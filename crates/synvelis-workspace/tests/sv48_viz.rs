//! SV-48 acceptance tests — Ruviform `ui/viz` + `ui/binding` headless
//! contract adopted into the Synvelis DAW view layer (issue #48).
//!
//! The headless contract is verified end to end against real session
//! semantics: bounded waveform reads + LRU, stale/partial/gap/unavailable
//! data-state semantics, snapshot pump (sequence loss + staleness),
//! project-backed timeline items, view-link resync on dock moves, and
//! the guarantee that display operations never touch project/engine
//! truth. No GUI, no GPU, no real device.

use std::collections::BTreeMap;
use std::sync::Arc;

use synvelis_project::{
    Asset, AssetId, Clip, ClipExtent, ClipId, ClipPlacement, ClipPlayback, CounterIdGenerator,
    Lane, LaneId, MediaDuration, MediaTimeValue, MidiDivision, MidiSource, MusicalDuration,
    MusicalTick, PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp, Source,
    SourceFrame, SourceId, SourceRange, SourceState, TimeAnchor, Track, TrackId,
};
use synvelis_workspace::viz_ui::{MeterPort, WaveformPort};
use synvelis_workspace::*;

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

/// Project with one track, two lanes and two musically-anchored clips
/// at distinct positions (clip11 @ beat 0.25–0.75, clip13 @ beat 1–2
/// at PPQN 480).
fn viz_session() -> ProjectSession {
    let mut p = Project::new(ProjectId::new(0x548), "sv48");
    let ppqn = i64::from(p.ppqn.0); // 480
    let track = TrackId::new(3);
    let lane = LaneId::new(4);
    let lane5 = LaneId::new(5);
    let src9 = SourceId::new(9);
    p.tracks.insert(
        track,
        Track {
            id: track,
            name: "t3".into(),
            lane_ids: vec![lane, lane5],
        },
    );
    p.lanes.insert(
        lane,
        Lane {
            id: lane,
            track_id: track,
            clip_ids: vec![ClipId::new(11)],
        },
    );
    p.lanes.insert(
        lane5,
        Lane {
            id: lane5,
            track_id: track,
            clip_ids: vec![ClipId::new(13)],
        },
    );
    p.assets.insert(
        AssetId::new(5),
        Asset {
            id: AssetId::new(5),
            digest: "sha256:a5".into(),
        },
    );
    p.sources.insert(
        src9,
        Source {
            id: src9,
            asset_id: AssetId::new(5),
            content_digest: "sha256:m9".into(),
            state: SourceState::Verified,
        },
    );
    p.midi_sources.insert(
        SourceId::new(10),
        MidiSource {
            id: SourceId::new(10),
            division: MidiDivision::ppqn(480).unwrap(),
            notes: BTreeMap::new(),
        },
    );
    for (clip, lane, source, start, len) in [
        (ClipId::new(11), lane, src9, ppqn / 4, ppqn / 2),
        (ClipId::new(13), lane5, SourceId::new(10), ppqn, ppqn),
    ] {
        p.clips.insert(
            clip,
            Clip {
                id: clip,
                lane_id: lane,
                source_id: source,
                placement: ClipPlacement {
                    anchor: TimeAnchor::Musical(MusicalTick::new(start)),
                    extent: ClipExtent::Musical(MusicalDuration::new(len).unwrap()),
                },
                playback: ClipPlayback {
                    source_region: SourceRange::try_new(
                        SourceFrame::new(0).unwrap(),
                        SourceFrame::new(48_000).unwrap(),
                    )
                    .unwrap(),
                    mapping: PlaybackMapping::Natural,
                },
                muted: false,
            },
        );
    }
    ProjectSession::new(ProjectController::new(
        p,
        Box::new(CounterIdGenerator::new(1)),
    ))
}

/// Mono PCM of `len` frames with a NaN run (a media gap) at `[gap,
/// gap+gap_len)`.
#[allow(clippy::cast_precision_loss)] // `i * 7 % 100` < 100
fn pcm(len: usize, gap: usize, gap_len: usize) -> Vec<Vec<f64>> {
    let mut ch = Vec::with_capacity(len);
    for i in 0..len {
        if (gap..gap + gap_len).contains(&i) {
            ch.push(f64::NAN);
        } else {
            ch.push(((i * 7 % 100) as f64 - 50.0) / 100.0);
        }
    }
    vec![ch]
}

#[allow(clippy::cast_precision_loss)] // fixture sequences stay far under 2^53
fn meter_frame(seq: u64, dropped: Option<u64>) -> MeterFrame {
    MeterFrame {
        sequence: seq,
        channels: vec![
            MeterChannel {
                peak: 0.8,
                rms: 0.5,
                held_peak: Some(0.9),
                clip: Some(false),
            },
            MeterChannel {
                peak: f64::NAN,
                rms: f64::NAN,
                held_peak: None,
                clip: None,
            },
        ],
        dropped,
        timestamp: Some(seq as f64 * 20.0),
    }
}

/// A waveform port that stamps replies with a *stale* revision —
/// exercises the discard-and-retry path deterministically.
struct StaleStampWaveform {
    inner: MemoryWaveform,
    stamp: SourceRevision,
}

impl WaveformPort for StaleStampWaveform {
    fn meta(&self) -> WaveformMeta {
        self.inner.meta()
    }
    fn query(&self, q: &WaveformQuery) -> Result<WaveformView, VizQueryError> {
        let mut v = self.inner.query(q)?;
        match &mut v {
            WaveformView::Bins { revision, .. } | WaveformView::Samples { revision, .. } => {
                *revision = self.stamp.clone();
            }
        }
        Ok(v)
    }
}

/// A meter port whose snapshot always fails — exercises the
/// "source error" status path while other surfaces keep pumping.
struct FailingMeter;
impl MeterPort for FailingMeter {
    fn meta(&self) -> MeterMeta {
        MeterMeta {
            id: "meter:fail".into(),
            revision: SourceRevision::Num(0),
            channels: 2,
        }
    }
    fn snapshot(&self) -> Result<Option<MeterFrame>, String> {
        Err("bridge dead".into())
    }
}

// ---------------------------------------------------------------------------
// arrangement + mixer + inspector share one gateway
// ---------------------------------------------------------------------------

#[test]
fn three_views_share_gateway_and_project_stays_untouched() {
    let session = viz_session();
    let mut ui = VizUi::new();
    let rev0 = session.revision();
    let builds0 = session.engine().graph_builds;

    // Arrangement: timeline + a clip-window waveform.
    ui.attach_view("arr", "w1");
    ui.bind_timeline("arr", "items", TimelineAxis::Beats, 48_000)
        .unwrap();
    let clip_target = waveform_clip_target(&ClipId::new(11));
    ui.register_waveform(
        &clip_target,
        Arc::new(MemoryWaveform::window(
            clip_target.clone(),
            48_000,
            WaveformAxis::Seconds,
            &pcm(48_000, 12_000, 4_000),
            1_000,
            8_000,
        )),
    );
    ui.bind_waveform("arr", "clip11", clip_target, 256).unwrap();
    ui.sync_timeline(&session, "arr", "items");

    // Mixer: master meter.
    ui.attach_view("mix", "w2");
    let meter = Arc::new(ManualMeter::new(meter_master_target(), 2));
    ui.register_meter(&meter_master_target(), meter.clone());
    ui.bind_meter("mix", "master", meter_master_target())
        .unwrap();

    // Inspector: probe spectrum.
    ui.attach_view("insp", "w2");
    let spectrum = Arc::new(ManualSpectrum::new(
        spectrum_probe_target("probe-7"),
        32,
        0.0,
        100.0,
    ));
    ui.register_spectrum(&spectrum_probe_target("probe-7"), spectrum.clone());
    ui.bind_spectrum("insp", "probe", spectrum_probe_target("probe-7"))
        .unwrap();

    // Waveform resolved under bounded bins == plot px.
    let row = ui.waveform_row("arr", "clip11").unwrap();
    assert_eq!(row.status, VizStatus::Ok);
    let WaveformView::Bins { channels, .. } = row.view.clone().unwrap() else {
        panic!("expected bins view");
    };
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0].len(), 256);

    // Timeline resolved from project truth.
    let t = ui.timeline_row("arr", "items").unwrap();
    assert_eq!(t.status, VizStatus::Ok);
    assert_eq!(t.items.len(), 2);
    assert_eq!(t.unresolved, 0);
    assert!((t.items[0].start - 0.25).abs() < 1e-9);
    assert!((t.items[1].start - 1.0).abs() < 1e-9);
    assert!(t.items[0].payload.clip.starts_with("clip-"));

    // Snapshot surfaces land through the pump only.
    assert_eq!(
        ui.meter_row("mix", "master").unwrap().status,
        VizStatus::AwaitingData
    );
    meter.push(meter_frame(1, None));
    spectrum.push(SpectrumFrame {
        sequence: 1,
        magnitudes: (0..32).map(|i| f64::from(i) / 32.0).collect(),
        timestamp: Some(1.0),
    });
    ui.pump(100);
    assert_eq!(
        ui.meter_row("mix", "master")
            .unwrap()
            .frame
            .unwrap()
            .sequence,
        1
    );
    assert_eq!(
        ui.spectrum_row("insp", "probe")
            .unwrap()
            .frame
            .unwrap()
            .sequence,
        1
    );

    // Display work never touched project/engine truth.
    assert_eq!(session.revision(), rev0);
    assert_eq!(session.engine().graph_builds, builds0);
}

// ---------------------------------------------------------------------------
// waveform contract
// ---------------------------------------------------------------------------

#[test]
fn waveform_bounded_query_and_lru_cache() {
    let mut ui = VizUi::new();
    // 5 minutes at 48 kHz — long-form source.
    let target = waveform_source_target(&SourceId::new(9));
    ui.register_waveform(
        &target,
        Arc::new(MemoryWaveform::new(
            target.clone(),
            48_000,
            WaveformAxis::Seconds,
            &pcm(48_000 * 300, 0, 0),
        )),
    );
    ui.attach_view("arr", "w1");
    ui.bind_waveform("arr", "src", target, 512).unwrap();
    assert_eq!(ui.diagnostics().queries_issued, 1);
    assert_eq!(ui.diagnostics().cache_misses, 1);

    // Same span re-serve: cache hit, no new query.
    ui.ensure_waveform("arr", "src");
    assert_eq!(ui.diagnostics().queries_issued, 1);
    assert_eq!(ui.diagnostics().cache_hits, 1);

    // Pan → miss → new bounded query; pan back → hit.
    ui.pan("arr", "src", 1.0).unwrap();
    assert_eq!(ui.diagnostics().queries_issued, 2);
    ui.pan("arr", "src", -1.0).unwrap();
    assert_eq!(ui.diagnostics().queries_issued, 2);
    assert_eq!(ui.diagnostics().cache_hits, 2);

    // Zoom keeps the anchor: span halves around the anchor's position.
    ui.set_span("arr", "src", 0.0, 4.0).unwrap();
    ui.zoom("arr", "src", 2.0, 1.0).unwrap();
    let row = ui.waveform_row("arr", "src").unwrap();
    let span = row.span.unwrap();
    assert!((span.end - span.start - 2.0).abs() < 1e-9);
    // Anchor 1.0 stays at 25% of the span.
    let t = (1.0 - span.start) / (span.end - span.start);
    assert!((t - 0.25).abs() < 1e-9);
    // Reply stayed bounded: 512 buckets per channel.
    let WaveformView::Bins { channels, .. } = row.view.unwrap() else {
        panic!("bins view");
    };
    assert_eq!(channels[0].len(), 512);
}

#[test]
fn waveform_gaps_and_partial_coverage_stay_gaps() {
    let mut ui = VizUi::new();
    let target = waveform_source_target(&SourceId::new(9));
    // 48k frames, a 960-frame media gap at 500..1460.
    ui.register_waveform(
        &target,
        Arc::new(MemoryWaveform::new(
            target.clone(),
            48_000,
            WaveformAxis::Samples,
            &pcm(48_000, 500, 960),
        )),
    );
    ui.attach_view("arr", "w1");
    ui.bind_waveform("arr", "src", target.clone(), 480).unwrap();
    let row = ui.waveform_row("arr", "src").unwrap();
    let WaveformView::Bins { channels, .. } = row.view.unwrap() else {
        panic!("bins view");
    };
    let buckets = &channels[0];
    let gaps = buckets.iter().filter(|b| b.count == 0).count();
    assert!(gaps > 0, "NaN run must surface as count==0 buckets");
    for b in buckets.iter().filter(|b| b.count == 0) {
        assert!(b.min.is_nan() && b.max.is_nan() && b.rms.is_nan());
    }
    // Coverage is never zero-filled: finite buckets carry real stats.
    assert!(buckets.iter().any(|b| b.count > 0 && b.max > 0.0));

    // Panning beyond the source end → tail buckets are gaps (partial
    // coverage is surfaced, not clamped or errored).
    ui.set_span("arr", "src", 47_500.0, 49_000.0).unwrap();
    let row = ui.waveform_row("arr", "src").unwrap();
    assert_eq!(row.status, VizStatus::Ok);
    let WaveformView::Bins { channels, .. } = row.view.unwrap() else {
        panic!("bins view");
    };
    let tail_gaps = channels[0]
        .iter()
        .skip(400)
        .filter(|b| b.count == 0)
        .count();
    assert!(tail_gaps > 0, "out-of-range tail must be gap buckets");
}

#[test]
fn waveform_stale_revision_responses_are_rejected() {
    let mut ui = VizUi::new();
    let target = waveform_source_target(&SourceId::new(9));
    ui.register_waveform(
        &target,
        Arc::new(StaleStampWaveform {
            inner: MemoryWaveform::new(
                target.clone(),
                48_000,
                WaveformAxis::Samples,
                &pcm(4_800, 0, 0),
            ),
            stamp: SourceRevision::Num(999),
        }),
    );
    ui.attach_view("arr", "w1");
    ui.bind_waveform("arr", "src", target, 100).unwrap();
    // Issued under rev 0, replied under 999 → discarded both times.
    let d = ui.diagnostics();
    assert_eq!(d.queries_issued, 2);
    assert_eq!(d.queries_stale, 2);
    let row = ui.waveform_row("arr", "src").unwrap();
    assert!(row.view_stale);
    assert_eq!(row.status, VizStatus::Stale);
}

#[test]
fn waveform_source_change_triggers_resubscribe() {
    let mut ui = VizUi::new();
    let target = waveform_source_target(&SourceId::new(9));
    let port = Arc::new(ManualWaveform::new(
        target.clone(),
        48_000,
        WaveformAxis::Samples,
        &pcm(4_800, 0, 0),
    ));
    ui.register_waveform(&target, port.clone());
    ui.attach_view("arr", "w1");
    ui.bind_waveform("arr", "src", target.clone(), 100).unwrap();
    let rev0 = ui
        .waveform_row("arr", "src")
        .unwrap()
        .meta
        .unwrap()
        .revision;
    assert_eq!(rev0, SourceRevision::Num(0));

    // Data append → revision bump → notify → re-read + fresh view.
    let rev1 = port.set_data(&pcm(9_600, 0, 0));
    assert_eq!(rev1, SourceRevision::Num(1));
    ui.notify_source_changed(&target);
    let row = ui.waveform_row("arr", "src").unwrap();
    assert_eq!(row.meta.unwrap().revision, SourceRevision::Num(1));
    assert_eq!(row.status, VizStatus::Ok);
    assert!(!row.view_stale);
    // The span stretched to the new extent on resync? No — span is a
    // view-local choice; the view refreshed under the new revision.
    let WaveformView::Bins { revision, .. } = row.view.unwrap() else {
        panic!("bins view");
    };
    assert_eq!(revision, SourceRevision::Num(1));
}

#[test]
fn waveform_reply_over_budget_is_an_outcome_not_a_throw() {
    let mut ui = VizUi::new().with_max_query_bytes(1024);
    let target = waveform_source_target(&SourceId::new(9));
    ui.register_waveform(
        &target,
        Arc::new(MemoryWaveform::new(
            target.clone(),
            48_000,
            WaveformAxis::Samples,
            &pcm(48_000, 0, 0),
        )),
    );
    ui.attach_view("arr", "w1");
    // 512 buckets × 32 B = 16 KiB > 1 KiB budget.
    ui.bind_waveform("arr", "src", target, 512).unwrap();
    let row = ui.waveform_row("arr", "src").unwrap();
    assert!(matches!(row.status, VizStatus::SourceError { .. }));
    assert_eq!(ui.diagnostics().queries_failed, 1);
}

// ---------------------------------------------------------------------------
// snapshot pump — meter + spectrum
// ---------------------------------------------------------------------------

#[test]
fn meter_seq_gaps_dropped_and_stale_semantics() {
    let mut ui = VizUi::new().with_stale_after_ms(500);
    let meter = Arc::new(ManualMeter::new(meter_master_target(), 2));
    ui.register_meter(&meter_master_target(), meter.clone());
    ui.attach_view("mix", "w1");
    ui.bind_meter("mix", "master", meter_master_target())
        .unwrap();

    meter.push(meter_frame(1, None));
    ui.pump(100);
    let row = ui.meter_row("mix", "master").unwrap();
    assert_eq!(row.dropped, 0);
    assert!(!row.stale);
    assert_eq!(row.status, VizStatus::Ok);

    // seq 1 → 4 skips 2,3; producer also reports 2 dropped.
    meter.push(meter_frame(4, Some(2)));
    ui.pump(120);
    let row = ui.meter_row("mix", "master").unwrap();
    assert_eq!(row.dropped, 4, "2 producer + 2 seq-gap");
    let d = ui.diagnostics();
    assert_eq!(d.producer_dropped, 2);
    assert_eq!(d.seq_gaps, 2);

    // Age past staleAfter → stale once; a fresh frame clears it.
    ui.pump(700);
    let row = ui.meter_row("mix", "master").unwrap();
    assert!(row.stale);
    assert_eq!(row.status, VizStatus::Stale);
    ui.pump(900);
    assert_eq!(ui.diagnostics().stale_transitions, 1, "stale fires once");
    meter.push(meter_frame(5, None));
    ui.pump(901);
    let row = ui.meter_row("mix", "master").unwrap();
    assert!(!row.stale);
    assert_eq!(row.dropped, 4);
    // NaN channel stays NaN — unavailable, not zeroed.
    assert!(row.frame.unwrap().channels[1].peak.is_nan());
}

#[test]
fn meter_awaiting_data_and_source_error_coexist() {
    let mut ui = VizUi::new();
    let ok = Arc::new(ManualMeter::new("meter:ok".into(), 2));
    ui.register_meter("meter:ok", ok.clone());
    ui.register_meter("meter:fail", Arc::new(FailingMeter));
    ui.attach_view("mix", "w1");
    ui.bind_meter("mix", "a", "meter:ok".into()).unwrap();
    ui.bind_meter("mix", "b", "meter:fail".into()).unwrap();
    ui.pump(100);
    // No frame yet → awaiting data (not stale, not an error).
    assert_eq!(
        ui.meter_row("mix", "a").unwrap().status,
        VizStatus::AwaitingData
    );
    assert!(matches!(
        ui.meter_row("mix", "b").unwrap().status,
        VizStatus::SourceError { .. }
    ));
    // The pump survives source errors and keeps pulling the good one.
    ok.push(meter_frame(1, None));
    ui.pump(120);
    assert_eq!(ui.meter_row("mix", "a").unwrap().status, VizStatus::Ok);
    assert!(matches!(
        ui.meter_row("mix", "b").unwrap().status,
        VizStatus::SourceError { .. }
    ));
}

#[test]
fn spectrum_frames_land_with_nan_bins_intact() {
    let mut ui = VizUi::new().with_stale_after_ms(100);
    let spectrum = Arc::new(ManualSpectrum::new(
        spectrum_probe_target("p"),
        8,
        0.0,
        100.0,
    ));
    ui.register_spectrum(&spectrum_probe_target("p"), spectrum.clone());
    ui.attach_view("insp", "w1");
    ui.bind_spectrum("insp", "probe", spectrum_probe_target("p"))
        .unwrap();
    let mut mags = vec![0.1, 0.2, f64::NAN, 0.4, 0.5, f64::NAN, 0.7, 0.8];
    spectrum.push(SpectrumFrame {
        sequence: 1,
        magnitudes: mags.clone(),
        timestamp: Some(5.0),
    });
    ui.pump(10);
    let row = ui.spectrum_row("insp", "probe").unwrap();
    let frame = row.frame.unwrap();
    assert_eq!(frame.sequence, 1);
    assert!(frame.magnitudes[2].is_nan() && frame.magnitudes[5].is_nan());
    // Stale after the window; frames are complete-or-missing.
    ui.pump(200);
    let row = ui.spectrum_row("insp", "probe").unwrap();
    assert!(row.stale && row.status == VizStatus::Stale);
    mags[0] = 0.9;
    spectrum.push(SpectrumFrame {
        sequence: 3,
        magnitudes: mags,
        timestamp: Some(6.0),
    });
    ui.pump(210);
    let row = ui.spectrum_row("insp", "probe").unwrap();
    assert!(!row.stale);
    assert_eq!(row.dropped, 1, "seq 1→3 gap");
}

// ---------------------------------------------------------------------------
// timeline — project-backed items, selection/playhead, revision guard
// ---------------------------------------------------------------------------

#[test]
fn timeline_sync_select_playhead_and_project_revision_guard() {
    let mut session = viz_session();
    let mut ui = VizUi::new();
    ui.attach_view("arr", "w1");
    ui.bind_timeline("arr", "items", TimelineAxis::Beats, 48_000)
        .unwrap();
    ui.sync_timeline(&session, "arr", "items");

    // Fitted span covers content (≥ the 16-beat floor).
    let row = ui.timeline_row("arr", "items").unwrap();
    let span = row.span.unwrap();
    assert!(span.end >= 16.0);

    // Bounded read: narrow the span and only overlapping items show.
    ui.set_span("arr", "items", 0.9, 2.0).unwrap();
    let row = ui.timeline_row("arr", "items").unwrap();
    assert_eq!(row.items.len(), 1);
    assert_eq!(row.items[0].payload.clip, ClipId::new(13).to_tagged());

    // Selection gesture → outbound event; committed selection stays
    // consumer-side until the host pushes overlays.
    ui.select("arr", "items", 1.5, 0.5).unwrap();
    let events = ui.drain_events();
    let sel = events.iter().find_map(|e| match e {
        VizViewEvent::Select { start, end, .. } => Some((*start, *end)),
        VizViewEvent::Viewport { .. } => None,
    });
    assert_eq!(sel, Some((0.5, 1.5)));
    assert!(
        ui.timeline_row("arr", "items")
            .unwrap()
            .overlays
            .selection
            .is_none()
    );
    ui.set_overlays(
        "arr",
        "items",
        OverlayState {
            selection: Some(TimeRange {
                start: 0.5,
                end: 1.5,
            }),
            playhead: Some(0.75),
            markers: vec![OverlayMarker {
                at: 4.0,
                label: Some("cue".into()),
                color: None,
            }],
        },
    )
    .unwrap();
    let row = ui.timeline_row("arr", "items").unwrap();
    assert_eq!(row.overlays.playhead, Some(0.75));

    // A project mutation makes the synced items stale until resync —
    // the project-source revision guard.
    assert!(!ui.timeline_stale(&session, "arr", "items"));
    let rev = session.revision();
    session
        .apply(
            ProjectOp::AddTrack {
                name: "t-new".into(),
            },
            rev,
        )
        .unwrap();
    assert!(ui.timeline_stale(&session, "arr", "items"));
    ui.sync_timeline(&session, "arr", "items");
    assert!(!ui.timeline_stale(&session, "arr", "items"));
    assert_eq!(
        ui.timeline_row("arr", "items").unwrap().synced_rev,
        Some(session.revision().0)
    );
}

// ---------------------------------------------------------------------------
// view isolation, dock moves, detach semantics
// ---------------------------------------------------------------------------

#[test]
fn two_windows_hold_independent_view_state() {
    let mut ui = VizUi::new();
    let target = waveform_source_target(&SourceId::new(9));
    ui.register_waveform(
        &target,
        Arc::new(MemoryWaveform::new(
            target.clone(),
            48_000,
            WaveformAxis::Seconds,
            &pcm(48_000 * 4, 0, 0),
        )),
    );
    ui.attach_view("arr-a", "w1");
    ui.attach_view("arr-b", "w2");
    ui.bind_waveform("arr-a", "src", target.clone(), 256)
        .unwrap();
    ui.bind_waveform("arr-b", "src", target.clone(), 256)
        .unwrap();

    ui.set_span("arr-a", "src", 0.0, 1.0).unwrap();
    ui.set_span("arr-b", "src", 2.0, 4.0).unwrap();
    ui.select("arr-a", "src", 0.2, 0.8).unwrap();
    ui.set_overlays(
        "arr-a",
        "src",
        OverlayState {
            selection: Some(TimeRange {
                start: 0.2,
                end: 0.8,
            }),
            ..OverlayState::default()
        },
    )
    .unwrap();
    let a = ui.waveform_row("arr-a", "src").unwrap();
    let b = ui.waveform_row("arr-b", "src").unwrap();
    assert_eq!(a.span.unwrap().start, 0.0);
    assert_eq!(b.span.unwrap().start, 2.0);
    assert!(a.overlays.selection.is_some());
    assert!(b.overlays.selection.is_none());
    // Caches are per-surface and keyed by span/rev/buckets: each view
    // holds its own entries (initial extent + the set span).
    assert_eq!(a.cache_entries, 2);
    assert_eq!(b.cache_entries, 2);
}

#[test]
fn dock_move_resyncs_and_generation_invalidates() {
    let mut ui = VizUi::new();
    let target = waveform_source_target(&SourceId::new(9));
    let port = Arc::new(ManualWaveform::new(
        target.clone(),
        48_000,
        WaveformAxis::Samples,
        &pcm(4_800, 0, 0),
    ));
    ui.register_waveform(&target, port.clone());
    let meter = Arc::new(ManualMeter::new(meter_master_target(), 2));
    ui.register_meter(&meter_master_target(), meter.clone());

    ui.attach_view("arr", "w1");
    let gen0 = ui.generation("arr").unwrap();
    ui.bind_waveform("arr", "src", target.clone(), 100).unwrap();
    ui.bind_meter("arr", "m", meter_master_target()).unwrap();
    meter.push(meter_frame(1, None));
    ui.pump(10);
    assert_eq!(ui.meter_row("arr", "m").unwrap().frame.unwrap().sequence, 1);
    let issued0 = ui.diagnostics().queries_issued;

    // Dock transfer: attach to a different window performs the
    // implicit detach first — generation bumps twice, in-flight query
    // bookkeeping dies under the old epoch, surfaces re-read truth.
    ui.attach_view("arr", "w2");
    let gen1 = ui.generation("arr").unwrap();
    assert!(gen1 > gen0 + 1, "detach+attach bump the epoch");
    assert_eq!(ui.waveform_row("arr", "src").unwrap().status, VizStatus::Ok);
    // The resync re-read is a cache hit when nothing changed — that is
    // the LRU doing its job, not a skipped resubscribe.
    assert_eq!(ui.diagnostics().queries_issued, issued0);
    assert!(ui.diagnostics().cache_hits > 0);
    // Re-attach to the same window is idempotent.
    let gen2 = ui.generation("arr").unwrap();
    ui.attach_view("arr", "w2");
    assert_eq!(ui.generation("arr").unwrap(), gen2);

    // While detached the pump does not pull this view's snapshots.
    meter.push(meter_frame(2, None));
    ui.detach_view("arr");
    ui.pump(20);
    assert!(ui.meter_row("arr", "m").is_some());
    assert_eq!(
        ui.meter_row("arr", "m").unwrap().frame.unwrap().sequence,
        1,
        "detached views are not pumped"
    );
    // A source revision moved while the view was away: the re-attach
    // resync must land the *new* truth, not the cached old revision.
    port.set_data(&pcm(9_600, 0, 0));
    ui.attach_view("arr", "w2");
    let row = ui.waveform_row("arr", "src").unwrap();
    assert_eq!(row.meta.unwrap().revision, SourceRevision::Num(1));
    let WaveformView::Bins { revision, .. } = row.view.unwrap() else {
        panic!("bins view");
    };
    assert_eq!(revision, SourceRevision::Num(1));
    ui.pump(30);
    assert_eq!(ui.meter_row("arr", "m").unwrap().frame.unwrap().sequence, 2);

    // Close drops the surfaces and the theme entry.
    ui.close_view("arr");
    assert!(ui.waveform_row("arr", "src").is_none());
    assert!(ui.meter_row("arr", "m").is_none());
    assert!(!ui.is_attached("arr"));
}

#[test]
fn display_only_ops_never_requery_and_never_touch_project() {
    let session = viz_session();
    let mut ui = VizUi::new();
    let target = waveform_source_target(&SourceId::new(9));
    let wtarget = target.clone();
    ui.register_waveform(
        &wtarget,
        Arc::new(MemoryWaveform::new(
            wtarget.clone(),
            48_000,
            WaveformAxis::Seconds,
            &pcm(48_000, 0, 0),
        )),
    );
    let meter = Arc::new(ManualMeter::new(meter_track_target(&TrackId::new(3)), 2));
    ui.register_meter(&meter_track_target(&TrackId::new(3)), meter);
    ui.attach_view("arr", "w1");
    ui.bind_waveform("arr", "src", target, 100).unwrap();
    ui.bind_meter("arr", "m", meter_track_target(&TrackId::new(3)))
        .unwrap();
    let issued = ui.diagnostics().queries_issued;
    let rev0 = session.revision();
    let builds0 = session.engine().graph_builds;

    // Theme swap, overlays, scales, range, labels — repaint only.
    ui.set_theme(
        "arr",
        VizStyleInput {
            theme: Some("dark".into()),
            waveform: Some("#0af".into()),
            ..VizStyleInput::default()
        },
    )
    .unwrap();
    ui.set_overlays(
        "arr",
        "src",
        OverlayState {
            selection: Some(TimeRange {
                start: 0.1,
                end: 0.2,
            }),
            playhead: Some(0.15),
            markers: vec![],
        },
    )
    .unwrap();
    ui.set_meter_scale(
        "arr",
        "m",
        MeterScale {
            min_db: -48.0,
            max_db: 6.0,
            segments: vec![-12.0],
            clip_db: 0.0,
        },
    )
    .unwrap();
    ui.set_waveform_range("arr", "src", -0.5, 0.5).unwrap();
    ui.set_channel_labels("arr", "src", Some(vec![Some("L".into())]))
        .unwrap();

    let d = ui.diagnostics();
    assert_eq!(d.queries_issued, issued, "repaint must not re-query");
    assert_eq!(d.repaints, 5);
    assert_eq!(ui.theme("arr").unwrap().0, 1);
    assert_eq!(session.revision(), rev0);
    assert_eq!(session.engine().graph_builds, builds0);
}

// ---------------------------------------------------------------------------
// wire contract + errors
// ---------------------------------------------------------------------------

#[test]
fn bind_and_ops_validate_view_and_surface() {
    let mut ui = VizUi::new();
    // Unknown view.
    assert!(matches!(
        ui.bind_waveform("nope", "s", "waveform:x".into(), 100),
        Err(VizError::UnknownView(_))
    ));
    ui.attach_view("v", "w1");
    // Unregistered source binds but degrades to NoSource.
    ui.bind_waveform("v", "s", "waveform:x".into(), 100)
        .unwrap();
    assert_eq!(
        ui.waveform_row("v", "s").unwrap().status,
        VizStatus::NoSource
    );
    // Kind mismatch.
    ui.bind_meter("v", "m", "meter:x".into()).unwrap();
    assert!(matches!(
        ui.set_span("v", "m", 0.0, 1.0),
        Err(VizError::KindMismatch { .. })
    ));
    // Invalid spans.
    assert_eq!(ui.set_span("v", "s", 1.0, 1.0), Err(VizError::InvalidSpan));
    assert_eq!(
        ui.set_span("v", "s", 0.0, f64::NAN),
        Err(VizError::InvalidSpan)
    );
    assert_eq!(ui.set_plot_px("v", "s", 0), Err(VizError::InvalidPixels));
    // Detached view rejects display ops.
    ui.detach_view("v");
    assert!(matches!(
        ui.set_span("v", "s", 0.0, 1.0),
        Err(VizError::DetachedView(_))
    ));
}

#[test]
fn wire_contract_roundtrips() {
    let q = ViewportQuery {
        query_id: "q1".into(),
        source_id: "waveform:clip-abc".into(),
        source_revision: SourceRevision::Num(7),
        view_id: "v1".into(),
        generation: 2,
        start: 0.0,
        end: 1.5,
        buckets: Some(256),
        channels: Some(vec![0, 1]),
        max_bytes: Some(1024),
        encoding: None,
    };
    let json = serde_json::to_string(&q).unwrap();
    let back: ViewportQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(q, back);

    let view = WaveformView::Bins {
        revision: SourceRevision::Num(3),
        start: 0.0,
        units_per_bucket: 0.01,
        channels: vec![vec![WaveformBucket {
            min: -0.5,
            max: 0.5,
            rms: 0.3,
            count: 100,
        }]],
    };
    let json = serde_json::to_string(&view).unwrap();
    assert!(json.contains("\"kind\":\"bins\""));
    let back: WaveformView = serde_json::from_str(&json).unwrap();
    assert_eq!(view, back);

    let mut frame = meter_frame(9, Some(2));
    // JSON carries no NaN literal: serde_json emits `null` for
    // non-finite values. The live-path types keep NaN lossless; on the
    // wire the unavailable-channel cue must ride `null`/count fields —
    // asserted here explicitly so the narrowing is visible.
    frame.channels[1].peak = 0.4;
    frame.channels[1].rms = 0.2;
    let json = serde_json::to_string(&frame).unwrap();
    let back: MeterFrame = serde_json::from_str(&json).unwrap();
    assert_eq!(frame, back);
    frame.channels[1].peak = f64::NAN;
    let json = serde_json::to_string(&frame).unwrap();
    assert!(json.contains("\"peak\":null"), "NaN encodes as null");

    let ev = VizViewEvent::Select {
        view_id: "v".into(),
        surface_id: "s".into(),
        start: 0.0,
        end: 1.0,
    };
    let json = serde_json::to_string(&ev).unwrap();
    assert!(json.contains("\"kind\":\"select\""));
    let back: VizViewEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(ev, back);

    // SourceRevision carries both wire shapes.
    let n: SourceRevision = serde_json::from_str("4").unwrap();
    let t: SourceRevision = serde_json::from_str("\"rev-a\"").unwrap();
    assert_eq!(n, SourceRevision::Num(4));
    assert_eq!(t, SourceRevision::Text("rev-a".into()));
}

// ---------------------------------------------------------------------------
// upstream-fidelity corner cases
// ---------------------------------------------------------------------------

#[test]
fn unregister_source_degrades_bound_surfaces() {
    let mut ui = VizUi::new();
    let target = waveform_source_target(&SourceId::new(9));
    ui.register_waveform(
        &target,
        Arc::new(MemoryWaveform::new(
            target.clone(),
            48_000,
            WaveformAxis::Samples,
            &pcm(4_800, 0, 0),
        )),
    );
    let meter = Arc::new(ManualMeter::new(meter_master_target(), 1));
    ui.register_meter(&meter_master_target(), meter);
    ui.attach_view("v", "w1");
    ui.bind_waveform("v", "w", target.clone(), 100).unwrap();
    ui.bind_meter("v", "m", meter_master_target()).unwrap();
    ui.unregister_source(&target);
    ui.unregister_source(&meter_master_target());
    assert_eq!(
        ui.waveform_row("v", "w").unwrap().status,
        VizStatus::NoSource
    );
    assert_eq!(ui.meter_row("v", "m").unwrap().status, VizStatus::NoSource);
}

#[test]
fn clip_window_waveform_reads_only_the_source_region() {
    // The clip's `source_region` window restricts what the waveform
    // port may read — DAW range semantics enforced at the adapter.
    let window = MemoryWaveform::window(
        "waveform:clip-x".into(),
        48_000,
        WaveformAxis::Samples,
        &pcm(10_000, 0, 0),
        2_000,
        4_000,
    );
    let meta = synvelis_workspace::viz_ui::WaveformPort::meta(&window);
    assert_eq!(meta.extent, 4_000.0);
    let v = synvelis_workspace::viz_ui::WaveformPort::query(
        &window,
        &WaveformQuery {
            start: 0.0,
            end: 4_000.0,
            buckets: 4,
            channels: None,
        },
    )
    .unwrap();
    let WaveformView::Bins { channels, .. } = v else {
        panic!("bins view");
    };
    assert_eq!(channels[0].len(), 4);
    assert!(channels[0].iter().all(|b| b.count > 0));
}

// ---------------------------------------------------------------------------
// review-fix regression coverage — contract violations found in review
// ---------------------------------------------------------------------------

/// `VizStatus` error variants cross the wire (internally-tagged serde
/// needs struct variants — newtype-string variants cannot tag).
#[test]
fn viz_status_error_variants_roundtrip() {
    for status in [
        VizStatus::Ok,
        VizStatus::NoSource,
        VizStatus::AwaitingData,
        VizStatus::Stale,
        VizStatus::SourceError {
            detail: "bridge gone".into(),
        },
        VizStatus::Unsupported {
            detail: "no backend".into(),
        },
    ] {
        let json = serde_json::to_string(&status).unwrap();
        let back: VizStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, status);
    }
    let err = serde_json::to_string(&VizStatus::SourceError { detail: "x".into() }).unwrap();
    assert!(err.contains("source-error"));
}

/// Gap payloads (`NaN` stats, `NaN` channel/magnitude data) serialize
/// as `null` and deserialize back to `NaN` — the narrowing is
/// bidirectional, never a hard failure and never a silent finite zero.
#[test]
fn nan_gap_payloads_roundtrip_as_null() {
    let bucket = WaveformBucket {
        min: f64::NAN,
        max: f64::NAN,
        rms: f64::NAN,
        count: 0,
    };
    let json = serde_json::to_string(&bucket).unwrap();
    assert!(json.contains("null"));
    let back: WaveformBucket = serde_json::from_str(&json).unwrap();
    assert!(back.min.is_nan() && back.max.is_nan() && back.rms.is_nan());
    assert_eq!(back.count, 0);

    let mut frame = meter_frame(7, None);
    frame.channels[1].peak = f64::NAN;
    let back: MeterFrame = serde_json::from_str(&serde_json::to_string(&frame).unwrap()).unwrap();
    assert!(back.channels[1].peak.is_nan());
    assert_eq!(back.sequence, 7);

    let spectrum = SpectrumFrame {
        sequence: 3,
        magnitudes: vec![1.0, f64::NAN, 2.0],
        timestamp: None,
    };
    let json = serde_json::to_string(&spectrum).unwrap();
    assert!(json.contains("null"));
    let back: SpectrumFrame = serde_json::from_str(&json).unwrap();
    assert!(back.magnitudes[1].is_nan());
    assert_eq!(back.magnitudes[0], 1.0);
}

/// Wire names follow the camelCase contract (`queryId`, `sourceId`,
/// `sourceRevision`, `viewId`, `maxBytes`) like the sibling modules.
#[test]
fn wire_names_are_camel_case() {
    let q = ViewportQuery {
        query_id: "q1".into(),
        source_id: "s".into(),
        source_revision: SourceRevision::Num(1),
        view_id: "v".into(),
        generation: 2,
        start: 0.0,
        end: 1.0,
        buckets: Some(8),
        channels: None,
        max_bytes: Some(16),
        encoding: None,
    };
    let json = serde_json::to_string(&q).unwrap();
    for name in [
        "queryId",
        "sourceId",
        "sourceRevision",
        "viewId",
        "maxBytes",
    ] {
        assert!(json.contains(&format!("\"{name}\"")), "missing {name}");
    }
    let back: ViewportQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(back, q);
    // Unknown fields are rejected at the boundary.
    assert!(
        serde_json::from_str::<ViewportQuery>(&json.replace("\"start\"", "\"bogus\"")).is_err()
    );
}

/// Musical anchor + Absolute extent composes anchor position *and*
/// duration — the canonical `resolve_musical_range` rule (at 120 BPM /
/// PPQN 960, 0.5 s @48 kHz == 1 beat == 960 ticks).
#[test]
fn timeline_absolute_extent_composes_anchor() {
    let mut p = Project::new(ProjectId::new(0x999), "abs");
    let track = TrackId::new(1);
    let lane = LaneId::new(2);
    let src = SourceId::new(3);
    p.tracks.insert(
        track,
        Track {
            id: track,
            name: "t".into(),
            lane_ids: vec![lane],
        },
    );
    p.lanes.insert(
        lane,
        Lane {
            id: lane,
            track_id: track,
            clip_ids: vec![ClipId::new(1), ClipId::new(2)],
        },
    );
    p.assets.insert(
        AssetId::new(1),
        Asset {
            id: AssetId::new(1),
            digest: "sha256:a".into(),
        },
    );
    p.sources.insert(
        src,
        Source {
            id: src,
            asset_id: AssetId::new(1),
            content_digest: "sha256:s".into(),
            state: SourceState::Verified,
        },
    );
    let region = || {
        SourceRange::try_new(
            SourceFrame::new(0).unwrap(),
            SourceFrame::new(96_000).unwrap(),
        )
        .unwrap()
    };
    // Clip 1: Musical anchor @ beat 1.0 (tick 960) + Absolute 0.5 s →
    // beats [1.0, 2.0]. The old math produced [1.0, 1.0].
    p.clips.insert(
        ClipId::new(1),
        Clip {
            id: ClipId::new(1),
            lane_id: lane,
            source_id: src,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(960)),
                extent: ClipExtent::Absolute(MediaDuration::new(24_000, 1, 48_000).unwrap()),
            },
            playback: ClipPlayback {
                source_region: region(),
                mapping: PlaybackMapping::Natural,
            },
            muted: false,
        },
    );
    // Clip 2: Absolute anchor @ 1.0 s (48 000 f) + Absolute 0.5 s →
    // beats [2.0, 3.0].
    p.clips.insert(
        ClipId::new(2),
        Clip {
            id: ClipId::new(2),
            lane_id: lane,
            source_id: src,
            placement: ClipPlacement {
                anchor: TimeAnchor::Absolute(MediaTimeValue::new(48_000, 1, 48_000).unwrap()),
                extent: ClipExtent::Absolute(MediaDuration::new(24_000, 1, 48_000).unwrap()),
            },
            playback: ClipPlayback {
                source_region: region(),
                mapping: PlaybackMapping::Natural,
            },
            muted: false,
        },
    );
    let session = ProjectSession::new(ProjectController::new(
        p,
        Box::new(CounterIdGenerator::new(1)),
    ));
    let mut ui = VizUi::new();
    ui.attach_view("arr", "w1");
    ui.bind_timeline("arr", "items", TimelineAxis::Beats, 48_000)
        .unwrap();
    ui.sync_timeline(&session, "arr", "items");
    let row = ui.timeline_row("arr", "items").unwrap();
    assert_eq!(row.unresolved, 0);
    assert_eq!(row.items.len(), 2);
    let spans: Vec<(f64, f64)> = row.items.iter().map(|i| (i.start, i.end)).collect();
    assert_eq!(spans, vec![(1.0, 2.0), (2.0, 3.0)]);
}

/// A waveform surface bound before its source registers must serve
/// data once the source arrives — meta refresh applies the extent
/// default instead of sitting at `NoSource` forever.
#[test]
fn bind_before_register_serves_data_after_notify() {
    let mut ui = VizUi::new();
    ui.attach_view("arr", "w1");
    let target = waveform_source_target(&SourceId::new(9));
    ui.bind_waveform("arr", "w", target.clone(), 64).unwrap();
    assert_eq!(
        ui.waveform_row("arr", "w").unwrap().status,
        VizStatus::NoSource
    );
    ui.register_waveform(
        &target,
        Arc::new(MemoryWaveform::new(
            target.clone(),
            48_000,
            WaveformAxis::Samples,
            &pcm(4_800, 0, 0),
        )),
    );
    let row = ui.waveform_row("arr", "w").unwrap();
    assert_eq!(row.status, VizStatus::Ok);
    let WaveformView::Bins { channels, .. } = row.view.unwrap() else {
        panic!("bins view");
    };
    assert_eq!(channels[0].len(), 64);
}

/// A port swap under a colliding revision token must not serve the
/// previous source's cached truth — the swap invalidates the surface.
#[test]
fn source_swap_never_serves_cached_truth() {
    let mut ui = VizUi::new();
    ui.attach_view("arr", "w1");
    let target = waveform_source_target(&SourceId::new(9));
    let mut positive = pcm(4_800, 0, 0);
    positive[0].fill(1.0);
    ui.register_waveform(
        &target,
        Arc::new(MemoryWaveform::new(
            target.clone(),
            48_000,
            WaveformAxis::Samples,
            &positive,
        )),
    );
    ui.bind_waveform("arr", "w", target.clone(), 64).unwrap();
    let WaveformView::Bins { channels, .. } = ui.waveform_row("arr", "w").unwrap().view.unwrap()
    else {
        panic!("bins view");
    };
    assert!(channels[0].iter().all(|b| b.max > 0.0));

    // Swap in a different port still stamping revision 0 — a cache
    // keyed on the colliding token alone would resurrect old truth.
    let mut negative = pcm(4_800, 0, 0);
    negative[0].fill(-1.0);
    ui.register_waveform(
        &target,
        Arc::new(MemoryWaveform::new(
            target.clone(),
            48_000,
            WaveformAxis::Samples,
            &negative,
        )),
    );
    ui.ensure_waveform("arr", "w");
    let row = ui.waveform_row("arr", "w").unwrap();
    assert_eq!(row.status, VizStatus::Ok);
    let WaveformView::Bins { channels, .. } = row.view.unwrap() else {
        panic!("bins view");
    };
    assert!(channels[0].iter().all(|b| b.max < 0.0));
    assert_eq!(row.cache_entries, 1);
}

/// An unregistered meter must stay `NoSource` — liveness bookkeeping
/// dies with the port and can never age a source-less surface stale.
#[test]
fn unregister_meter_stays_no_source() {
    let mut ui = VizUi::new();
    ui.attach_view("mix", "w1");
    let target = meter_master_target();
    let meter = Arc::new(ManualMeter::new(target.clone(), 2));
    ui.register_meter(&target, meter.clone());
    ui.bind_meter("mix", "master", target.clone()).unwrap();
    meter.push(meter_frame(1, None));
    ui.pump(0);
    assert_eq!(ui.meter_row("mix", "master").unwrap().status, VizStatus::Ok);

    ui.unregister_source(&target);
    assert_eq!(
        ui.meter_row("mix", "master").unwrap().status,
        VizStatus::NoSource
    );
    // Pump well past the stale window — no phantom stale transition.
    ui.pump(60_000);
    assert_eq!(
        ui.meter_row("mix", "master").unwrap().status,
        VizStatus::NoSource
    );
    assert_eq!(ui.diagnostics().stale_transitions, 0);
}

/// Re-attach resync derives status from the truth actually held — a
/// landed frame reports `Ok`, never flaps back to `AwaitingData`.
#[test]
fn reattach_preserves_landed_frame_status() {
    let mut ui = VizUi::new();
    ui.attach_view("mix", "w1");
    let target = meter_master_target();
    let meter = Arc::new(ManualMeter::new(target.clone(), 2));
    ui.register_meter(&target, meter.clone());
    ui.bind_meter("mix", "master", target.clone()).unwrap();
    meter.push(meter_frame(1, None));
    ui.pump(100);

    ui.detach_view("mix");
    ui.attach_view("mix", "w1");
    let row = ui.meter_row("mix", "master").unwrap();
    assert_eq!(row.status, VizStatus::Ok);
    assert!(row.frame.is_some());
    // An idle producer ages the landed frame to stale as usual.
    ui.pump(100 + DEFAULT_STALE_AFTER_MS + 1);
    let row = ui.meter_row("mix", "master").unwrap();
    assert_eq!(row.status, VizStatus::Stale);
    assert!(row.stale);
}

/// `u64::MAX` sequences and `u64::MAX` dropped counts saturate rather
/// than overflow — producer-supplied arithmetic is always checked.
#[test]
fn sequence_and_dropped_extremes_saturate() {
    let mut ui = VizUi::new();
    ui.attach_view("mix", "w1");
    let target = meter_master_target();
    let meter = Arc::new(ManualMeter::new(target.clone(), 2));
    ui.register_meter(&target, meter.clone());
    ui.bind_meter("mix", "master", target.clone()).unwrap();

    meter.push(meter_frame(u64::MAX, Some(u64::MAX)));
    ui.pump(0);
    meter.push(meter_frame(u64::MAX, Some(u64::MAX)));
    ui.pump(1);
    // A regression after MAX is a producer epoch restart — lands,
    // no bogus gap.
    meter.push(meter_frame(5, None));
    ui.pump(2);
    let row = ui.meter_row("mix", "master").unwrap();
    assert_eq!(row.status, VizStatus::Ok);
    assert_eq!(row.frame.unwrap().sequence, 5);
    assert_eq!(ui.diagnostics().seq_gaps, 0);
    assert!(ui.diagnostics().producer_dropped > 0);
}

/// Computed spans that escape finite space are rejected — pan/zoom
/// never land non-finite or inverted spans in surface state.
#[test]
fn pan_zoom_reject_non_finite_spans() {
    let mut ui = VizUi::new();
    ui.attach_view("arr", "w1");
    ui.bind_timeline("arr", "items", TimelineAxis::Beats, 48_000)
        .unwrap();
    ui.set_span("arr", "items", 0.0, 4.0).unwrap();

    // Panning past f64 range → the computed end is infinite.
    assert_eq!(ui.pan("arr", "items", f64::MAX), Err(VizError::InvalidSpan));
    let span = ui.timeline_row("arr", "items").unwrap().span.unwrap();
    assert_eq!((span.start, span.end), (0.0, 4.0));

    // Zooming to subnormal collapse → the computed span inverts/zeroes.
    ui.set_span("arr", "items", 0.0, f64::MAX).unwrap();
    assert_eq!(
        ui.zoom("arr", "items", 1e-300, 0.0),
        Err(VizError::InvalidSpan)
    );
    assert_eq!(
        ui.zoom("arr", "items", f64::MAX, f64::MAX / 2.0),
        Err(VizError::InvalidSpan)
    );
}

/// An over-budget reply is a terminal outcome — `last_query`
/// bookkeeping clears like every other terminal path.
#[test]
fn over_budget_reply_clears_in_flight_bookkeeping() {
    let mut ui = VizUi::new().with_max_query_bytes(16);
    ui.attach_view("arr", "w1");
    let target = waveform_source_target(&SourceId::new(9));
    ui.register_waveform(
        &target,
        Arc::new(MemoryWaveform::new(
            target.clone(),
            48_000,
            WaveformAxis::Samples,
            &pcm(4_800, 0, 0),
        )),
    );
    ui.bind_waveform("arr", "w", target.clone(), 256).unwrap();
    let row = ui.waveform_row("arr", "w").unwrap();
    assert!(matches!(row.status, VizStatus::SourceError { .. }));
    assert!(ui.last_query("arr", "w").is_none());
    assert_eq!(ui.diagnostics().queries_failed, 1);
    assert_eq!(ui.diagnostics().queries_cancelled, 0);
}

/// `plot_px`/`buckets` are capped — the bounded-read contract is
/// enforced before allocation, not just on the reply bytes.
#[test]
fn plot_px_and_buckets_are_capped() {
    let mut ui = VizUi::new();
    ui.attach_view("arr", "w1");
    let target = waveform_source_target(&SourceId::new(9));
    ui.register_waveform(
        &target,
        Arc::new(MemoryWaveform::new(
            target.clone(),
            48_000,
            WaveformAxis::Samples,
            &pcm(4_800, 0, 0),
        )),
    );
    assert_eq!(
        ui.bind_waveform("arr", "w", target.clone(), u32::MAX),
        Err(VizError::InvalidPixels)
    );
    ui.bind_waveform("arr", "w", target.clone(), 64).unwrap();
    assert_eq!(
        ui.set_plot_px("arr", "w", u32::MAX),
        Err(VizError::InvalidPixels)
    );
    assert_eq!(
        ui.set_plot_px("arr", "w", MAX_BUCKETS + 1),
        Err(VizError::InvalidPixels)
    );
    // Port-level guard too — direct `query` calls obey the same cap.
    let port = MemoryWaveform::new(
        target.clone(),
        48_000,
        WaveformAxis::Samples,
        &pcm(480, 0, 0),
    );
    let err = port
        .query(&WaveformQuery {
            start: 0.0,
            end: 1.0,
            buckets: MAX_BUCKETS + 1,
            channels: None,
        })
        .unwrap_err();
    assert_eq!(err.kind, QueryErrorKind::OverBudget);
}

/// A frame that disagrees with meta channel count is a producer
/// error — partial frames never land as `Ok` truth.
#[test]
fn partial_meter_frame_is_a_source_error() {
    let mut ui = VizUi::new();
    ui.attach_view("mix", "w1");
    let target = meter_master_target();
    let meter = Arc::new(ManualMeter::new(target.clone(), 2));
    ui.register_meter(&target, meter.clone());
    ui.bind_meter("mix", "master", target.clone()).unwrap();
    let partial = MeterFrame {
        sequence: 1,
        channels: vec![MeterChannel {
            peak: 0.5,
            rms: 0.3,
            held_peak: None,
            clip: None,
        }],
        dropped: None,
        timestamp: None,
    };
    meter.push(partial);
    ui.pump(0);
    let row = ui.meter_row("mix", "master").unwrap();
    assert!(matches!(row.status, VizStatus::SourceError { .. }));
    assert!(row.frame.is_none());
    // Recovery: a complete frame still lands and clears the error.
    meter.push(meter_frame(2, None));
    ui.pump(1);
    let row = ui.meter_row("mix", "master").unwrap();
    assert_eq!(row.status, VizStatus::Ok);
    assert!(row.frame.is_some());
}

/// The outbound event queue is bounded — an undrained host cannot
/// grow memory without bound; drops are counted.
#[test]
fn event_queue_is_bounded() {
    let mut ui = VizUi::new();
    ui.attach_view("arr", "w1");
    ui.bind_timeline("arr", "items", TimelineAxis::Beats, 48_000)
        .unwrap();
    ui.set_span("arr", "items", 0.0, 4.0).unwrap();
    ui.drain_events();
    for _ in 0..MAX_EVENTS + 64 {
        ui.select("arr", "items", 1.0, 0.5).unwrap();
    }
    let drained = ui.drain_events();
    assert_eq!(drained.len(), MAX_EVENTS);
    assert_eq!(ui.diagnostics().events_dropped, 64);
    // First-dropped ordering: the newest events survive.
    assert!(matches!(drained.last(), Some(VizViewEvent::Select { .. })));
}

// ---------------------------------------------------------------------------
// second-pass review regression coverage
// ---------------------------------------------------------------------------

/// Meter/spectrum status re-derives when meta resolves: a port swap or
/// late-registered source never keeps the old producer's status
/// (`Ok`/`Stale`/`SourceError`/`NoSource` clinging to the new epoch).
#[test]
fn meter_swap_and_late_register_rederive_status() {
    let mut ui = VizUi::new();
    ui.attach_view("mix", "w1");
    let target = meter_master_target();

    // Bind before register: `NoSource` while absent, `AwaitingData`
    // once meta resolves — the surface knows its source exists.
    ui.bind_meter("mix", "master", target.clone()).unwrap();
    assert_eq!(
        ui.meter_row("mix", "master").unwrap().status,
        VizStatus::NoSource
    );
    ui.register_meter(&target, Arc::new(ManualMeter::new(target.clone(), 2)));
    assert_eq!(
        ui.meter_row("mix", "master").unwrap().status,
        VizStatus::AwaitingData
    );

    // Land a frame → Ok. Swap in a fresh idle port → the reset frame
    // re-derives to `AwaitingData`, never clinging `Ok`.
    let meter = Arc::new(ManualMeter::new(target.clone(), 2));
    ui.register_meter(&target, meter.clone());
    meter.push(meter_frame(1, None));
    ui.pump(0);
    assert_eq!(ui.meter_row("mix", "master").unwrap().status, VizStatus::Ok);
    ui.register_meter(&target, Arc::new(ManualMeter::new(target.clone(), 2)));
    assert_eq!(
        ui.meter_row("mix", "master").unwrap().status,
        VizStatus::AwaitingData
    );
    assert!(ui.meter_row("mix", "master").unwrap().frame.is_none());
}

/// Same re-derivation for spectrum, plus a stale status that must
/// not survive a swap.
#[test]
fn spectrum_swap_clears_stale_status() {
    let mut ui = VizUi::new().with_stale_after_ms(100);
    ui.attach_view("probe", "w1");
    let target = spectrum_probe_target("p");
    let spectrum = Arc::new(ManualSpectrum::new(target.clone(), 4, 0.0, 100.0));
    ui.register_spectrum(&target, spectrum.clone());
    ui.bind_spectrum("probe", "sp", target.clone()).unwrap();
    spectrum.push(SpectrumFrame {
        sequence: 1,
        magnitudes: vec![0.5; 4],
        timestamp: None,
    });
    ui.pump(0);
    ui.pump(60_000);
    assert_eq!(
        ui.spectrum_row("probe", "sp").unwrap().status,
        VizStatus::Stale
    );

    ui.register_spectrum(
        &target,
        Arc::new(ManualSpectrum::new(target.clone(), 4, 0.0, 100.0)),
    );
    assert_eq!(
        ui.spectrum_row("probe", "sp").unwrap().status,
        VizStatus::AwaitingData
    );
}

/// A surface bound to a port of the wrong kind reports a source
/// error, not absence — same rule as `*_pull` kind mismatches.
#[test]
fn wrong_kind_port_is_a_source_error() {
    let mut ui = VizUi::new();
    ui.attach_view("mix", "w1");
    let target = meter_master_target();
    // A *waveform* surface bound to a *meter* port.
    ui.register_meter(&target, Arc::new(ManualMeter::new(target.clone(), 2)));
    ui.bind_waveform("mix", "w", target.clone(), 64).unwrap();
    ui.ensure_waveform("mix", "w");
    let row = ui.waveform_row("mix", "w").unwrap();
    assert!(
        matches!(&row.status, VizStatus::SourceError { detail } if detail.contains("waveform")),
        "expected wrong-kind SourceError, got {:?}",
        row.status
    );
}

/// Overlay geometry is axis geometry — non-finite or inverted
/// positions are rejected at the boundary, never stored.
#[test]
fn overlays_reject_non_finite_geometry() {
    let mut ui = VizUi::new();
    ui.attach_view("arr", "w1");
    ui.bind_timeline("arr", "items", TimelineAxis::Beats, 48_000)
        .unwrap();
    let base = OverlayState::default();
    let cases = [
        OverlayState {
            selection: Some(TimeRange {
                start: f64::NAN,
                end: 1.0,
            }),
            ..base.clone()
        },
        OverlayState {
            selection: Some(TimeRange {
                start: 2.0,
                end: 1.0,
            }),
            ..base.clone()
        },
        OverlayState {
            playhead: Some(f64::INFINITY),
            ..base.clone()
        },
        OverlayState {
            markers: vec![OverlayMarker {
                at: f64::NEG_INFINITY,
                label: None,
                color: None,
            }],
            ..base.clone()
        },
    ];
    for bad in cases {
        assert_eq!(
            ui.set_overlays("arr", "items", bad),
            Err(VizError::InvalidSpan)
        );
    }
    // A valid set still lands.
    let good = OverlayState {
        selection: Some(TimeRange {
            start: 1.0,
            end: 1.0,
        }),
        playhead: Some(2.0),
        markers: vec![OverlayMarker {
            at: 0.0,
            label: Some("m".into()),
            color: None,
        }],
    };
    ui.set_overlays("arr", "items", good.clone()).unwrap();
    assert_eq!(ui.timeline_row("arr", "items").unwrap().overlays, good);
}

/// Every scale field is validated — `clip_db`/`segments` could
/// previously carry NaN into state and break the wire roundtrip.
#[test]
fn meter_scale_validates_all_fields() {
    let mut ui = VizUi::new();
    ui.attach_view("mix", "w1");
    let target = meter_master_target();
    ui.register_meter(&target, Arc::new(ManualMeter::new(target.clone(), 2)));
    ui.bind_meter("mix", "master", target.clone()).unwrap();
    for bad in [
        MeterScale {
            clip_db: f64::NAN,
            ..MeterScale::default()
        },
        MeterScale {
            segments: vec![-18.0, f64::INFINITY],
            ..MeterScale::default()
        },
    ] {
        assert!(matches!(
            ui.set_meter_scale("mix", "master", bad),
            Err(VizError::InvalidConfig(_))
        ));
    }
    ui.set_meter_scale("mix", "master", MeterScale::default())
        .unwrap();
}
