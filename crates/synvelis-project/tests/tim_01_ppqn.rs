//! TIM-01 acceptance tests — configurable project PPQN (issue #125).
//!
//! Headless slice: `ProjectOp::SetProjectPpqn` re-projects every
//! persisted musical-tick value through exact rational beats under an
//! explicit conversion policy — one CAS, one undo entry, non-destructive
//! to source-native MIDI data, absolute media anchors, and recording
//! frame data. GUI/transport gating is app-layer scope and out of
//! scope here.
//!
//! Structure/recording entities are seeded as POD directly (same
//! convention as `md_a.rs` — recording-side ops live in
//! `synvelis-recording`, which this crate's tests may not depend on).

use ruviform_media_core::Rounding;
use synvelis_project::*;

const RATE: u32 = 48_000;

// ---------- fixtures ----------

fn controller(project: Project) -> ProjectController {
    ProjectController::new(project, Box::new(CounterIdGenerator::new(1_000)))
}

fn apply<O: Operation>(c: &mut ProjectController, op: O) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap()
}

fn try_apply<O: Operation>(c: &mut ProjectController, op: O) -> Result<ProjectRevision, Error> {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
}

/// Asserts rejection and that neither the live revision nor the live
/// project content moved.
fn rejected<O: Operation>(c: &mut ProjectController, op: O) {
    let rev = c.live_revision();
    let live = c.live().clone();
    assert!(try_apply(c, op).is_err(), "op must be rejected");
    assert_eq!(
        c.live_revision(),
        rev,
        "rejected op must not advance revision"
    );
    assert_eq!(c.live(), &live, "rejected op must not mutate live state");
}

fn ppqn_op(old: u32, new: u32, policy: PpqnConversionPolicy) -> ProjectOp {
    ProjectOp::SetProjectPpqn {
        old_ppqn: old,
        new_ppqn: new,
        policy,
    }
}

fn mr(start: i64, end: i64) -> ProjectRange {
    ProjectRange::Musical(
        MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap(),
    )
}

fn tempo_map(p: &Project) -> TickTimeMapping {
    TickTimeMapping::new(&p.tempo_map).unwrap()
}

struct Seeded {
    lane_id: LaneId,
    clip_musical: ClipId,
    clip_absolute: ClipId,
    midi_source: SourceId,
    take_group: TakeGroupId,
    region: CompRegionId,
}

/// Project POD: one track/lane, one media source/asset, and the
/// minimal recording chain (session → pass → stream → segment →
/// take) a take group can reference.
#[allow(clippy::too_many_lines)] // flat POD fixture, one insert per entity
fn seeded_project() -> (Project, Seeded) {
    let mut p = Project::new(ProjectId::new(0), "tim-01");
    let lane_id = LaneId::new(2);
    let source_id = SourceId::new(5);
    let take_id = TakeId::new(15);

    let track_id = TrackId::new(1);
    p.tracks.insert(
        track_id,
        Track {
            id: track_id,
            name: "t".into(),
            lane_ids: vec![lane_id],
        },
    );
    p.lanes.insert(
        lane_id,
        Lane {
            id: lane_id,
            track_id,
            clip_ids: Vec::new(),
        },
    );
    let asset_id = AssetId::new(4);
    p.assets.insert(
        asset_id,
        Asset {
            id: asset_id,
            digest: "aa".into(),
        },
    );
    p.sources.insert(
        source_id,
        Source {
            id: source_id,
            asset_id,
            content_digest: "aa".into(),
            state: SourceState::Committed,
        },
    );

    // Recording chain — all session/source frame data, no musical
    // ticks: a PPQN change must leave every one of these untouched.
    let session_id = RecordingSessionId::new(6);
    let pass_id = RecordingPassId::new(7);
    let stream_id = CaptureStreamId::new(10);
    let seg_id = CaptureSegmentId::new(12);
    let epoch = ClockEpochId::new(18);
    p.clock_epochs.insert(
        epoch,
        ClockEpoch {
            id: epoch,
            description: "mock".into(),
        },
    );
    p.recording.sessions.insert(
        session_id,
        RecordingSession {
            id: session_id,
            name: "s".into(),
            state: SessionState::Finalized,
        },
    );
    p.recording.passes.insert(
        pass_id,
        RecordingPass {
            id: pass_id,
            session_id,
            index: 0,
        },
    );
    p.recording.streams.insert(
        stream_id,
        CaptureStream {
            id: stream_id,
            session_id,
            lane_id,
            clock_epoch_id: epoch,
        },
    );
    p.recording.segments.insert(
        seg_id,
        CaptureSegment {
            id: seg_id,
            stream_id,
            pass_id,
            session_range: SessionRange::try_new(
                SessionFrame::new(0).unwrap(),
                SessionFrame::new(30_000).unwrap(),
            )
            .unwrap(),
            source_range: SourceRange::try_new(
                SourceFrame::ZERO,
                SourceFrame::new(30_000).unwrap(),
            )
            .unwrap(),
            source_id,
        },
    );
    p.recording.takes.insert(
        take_id,
        Take {
            id: take_id,
            session_id,
            pass_id,
            stream_id,
            lane_id,
            source_id,
            segment_ids: vec![seg_id],
            state: TakeState::Verified,
        },
    );

    (
        p,
        Seeded {
            lane_id,
            clip_musical: ClipId::new(0),
            clip_absolute: ClipId::new(0),
            midi_source: SourceId::new(0),
            take_group: TakeGroupId::new(0),
            region: CompRegionId::new(0),
        },
    )
}

/// Applies the entity ops on top of the POD fixture: MIDI source +
/// note, a musical clip, an absolute clip, variable tempo events, and
/// a comp (group + one region over musical ranges).
#[allow(clippy::too_many_lines)] // flat op sequence, one apply per entity
fn seeded() -> (ProjectController, Seeded) {
    let (project, mut s) = seeded_project();
    let mut c = controller(project);
    let lane = s.lane_id;
    let source = SourceId::new(5);

    apply(
        &mut c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    s.midi_source = *c.live().midi_sources.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: s.midi_source,
            key: 60,
            velocity: 100,
            start: MidiTick::new(0).unwrap(),
            len: MidiTick::new(120).unwrap(),
        },
    );

    // Musical clip: anchor tick 3840 (= 4 beats @960), extent 1920
    // (= 2 beats) — whole-beat positions exact on every PPQN.
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(3_840)),
                extent: ClipExtent::Musical(MusicalDuration::new(1_920).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(96_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    s.clip_musical = *c.live().clips.keys().next().unwrap();

    // Absolute clip: media-domain anchor/extent — must be untouched.
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Absolute(MediaTimeValue::new(12_345, 1, RATE).unwrap()),
                extent: ClipExtent::Absolute(MediaDuration::new(24_000, 1, RATE).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(96_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    s.clip_absolute = *c.live().clips.keys().nth(1).unwrap();

    // Variable tempo: events at whole-beat ticks (960, 2880) plus a
    // signature change at 1920 (2 beats) — all exact on every PPQN,
    // including the odd 32767 and coarse 1 grids.
    apply(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 960,
            bpm_milli: 90_000,
        },
    );
    apply(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 2_880,
            bpm_milli: 140_000,
        },
    );
    apply(
        &mut c,
        ProjectOp::AddSignatureEvent {
            tick: 1_920,
            numerator: 3,
            denominator: 4,
        },
    );

    // Comp: one take group + one region, both musical ranges with
    // whole-beat bounds (group 0..2 beats, region 1..2 beats).
    let take = *c.live().recording.takes.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: lane,
            target_range: mr(0, 1_920),
            take_ids: vec![take],
            source_rate: RATE,
        },
    );
    s.take_group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: s.take_group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(960, 1_920),
            candidate: CompCandidateRef::ExistingSource {
                source_id: source,
                handle: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(96_000).unwrap())
                    .unwrap(),
                play_from: SourceFrame::new(2_000).unwrap(),
            },
            mapping: PlaybackMapping::Natural,
        },
    );
    s.region = *c.live().comp.regions.keys().next().unwrap();

    // Punch marks: project-wide musical ticks (1 and 3 beats) —
    // re-projected like every other persisted musical position.
    apply(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(960)),
            punch_out: Some(MusicalTick::new(2_880)),
        },
    );

    // Automation: a project-scope lane whose points live in the
    // rational beat domain — 4/3 beat is not representable on *any*
    // PPQN grid, so bit-identical survival proves the BeatPoint
    // domain is outside the re-projection walk.
    apply(
        &mut c,
        ProjectOp::AddParameterTarget {
            name: "p".into(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let target = *c
        .live()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap();
    apply(&mut c, ProjectOp::AddAutomationLane { target });
    let automation_lane = *c.live().control.lanes.keys().last().unwrap();
    apply(
        &mut c,
        ProjectOp::WriteAutomationRegion {
            lane: automation_lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(4, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: BeatPoint::new(4, 3).unwrap(),
                        value: 0.25,
                    },
                    AutomationPoint {
                        pos: BeatPoint::new(2, 1).unwrap(),
                        value: 0.75,
                    },
                ],
            },
            policy: ConflictPolicy::Reject,
        },
    );

    // Plugin instance identity: a resolved processor allocates a
    // StablePluginInstanceId — routing/plugin state is outside the
    // musical tick domain and must survive unchanged.
    apply(
        &mut c,
        ProjectOp::AddTrackStrip {
            track_id: TrackId::new(1),
        },
    );
    let strip = c.live().routing.strip_of_track(TrackId::new(1)).unwrap().id;
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: "vendor.tim01".into(),
                version: "1.0.0".into(),
                capability_revision: 1,
            }),
            sidechain_input: false,
        },
    );

    // SMF-backed source: division 480 ≠ project PPQN 960 — the
    // source-native document (events in its own ticks) and the note
    // projection are outside the re-projection walk.
    apply(
        &mut c,
        ProjectOp::ImportMidiSmf {
            document: SmfDocument {
                format: SmfFormat::Format1,
                division: 480,
                tracks: vec![
                    SmfTrack {
                        events: vec![
                            SmfEvent {
                                tick: MidiTick::ZERO,
                                kind: SmfEventKind::Tempo {
                                    us_per_quarter: 500_000,
                                },
                            },
                            SmfEvent {
                                tick: MidiTick::ZERO,
                                kind: SmfEventKind::EndOfTrack,
                            },
                        ],
                    },
                    SmfTrack {
                        events: vec![
                            SmfEvent {
                                tick: MidiTick::ZERO,
                                kind: SmfEventKind::NoteOn {
                                    channel: 0,
                                    key: 60,
                                    velocity: 100,
                                },
                            },
                            SmfEvent {
                                tick: MidiTick::new(120).unwrap(),
                                kind: SmfEventKind::NoteOff {
                                    channel: 0,
                                    key: 60,
                                    velocity: 0,
                                },
                            },
                            SmfEvent {
                                tick: MidiTick::new(120).unwrap(),
                                kind: SmfEventKind::EndOfTrack,
                            },
                        ],
                    },
                ],
            },
        },
    );

    (c, s)
}

/// The beat-domain truth of every persisted musical position, used to
/// prove PPQN changes preserve musical meaning exactly.
fn musical_positions(p: &Project) -> Vec<(String, i64)> {
    let mut out = Vec::new();
    for e in &p.tempo_map.tempo_events {
        out.push(("tempo".into(), e.tick));
    }
    for e in &p.tempo_map.signature_events {
        out.push(("sig".into(), e.tick));
    }
    for clip in p.clips.values() {
        if let TimeAnchor::Musical(t) = clip.placement.anchor {
            out.push((format!("anchor:{}", clip.id), t.value()));
        }
        if let ClipExtent::Musical(d) = clip.placement.extent {
            out.push((format!("extent:{}", clip.id), d.value()));
        }
    }
    for g in p.comp.take_groups.values() {
        if let ProjectRange::Musical(r) = g.target_range {
            out.push(("group-start".into(), r.start().value()));
            out.push(("group-end".into(), r.end().value()));
        }
    }
    for r in p.comp.regions.values() {
        if let ProjectRange::Musical(m) = r.project_range {
            out.push(("region-start".into(), m.start().value()));
            out.push(("region-end".into(), m.end().value()));
        }
    }
    for t in [&p.monitoring.punch.punch_in, &p.monitoring.punch.punch_out]
        .into_iter()
        .flatten()
    {
        out.push(("punch".into(), t.value()));
    }
    out
}

/// Lifts every `(kind, tick)` through `tick/ppqn` to a canonical
/// rational `(num, den)` — the PPQN-independent musical truth.
fn beats_of(tick: i64, ppqn: u32) -> (i128, u128) {
    let g = gcd(u128::from(tick.unsigned_abs()), u128::from(ppqn));
    (
        i128::from(tick) / i128::try_from(g).unwrap(),
        u128::from(ppqn) / g,
    )
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a.max(1)
}

// ---------- tests ----------

/// PPQN-01/02: changing PPQN preserves every musical position's
/// rational beat and re-projects all persisted tick fields.
#[test]
fn ppqn_change_preserves_beat_positions_and_reprojects_ticks() {
    let (mut c, s) = seeded();
    let before_positions = musical_positions(c.live());

    // Media-domain truth before the change: the musical clip's span
    // and every tempo-map sample must resolve identically after.
    let map_before = tempo_map(c.live());
    let clip = &c.live().clips[&s.clip_musical];
    let span_before = clip
        .placement
        .resolve_span(Some(&map_before), RATE)
        .unwrap();

    let rev = apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));
    assert!(rev.0 > 0);

    let project = c.live();
    assert_eq!(project.ppqn, ProjectPpqn(480));
    assert_eq!(project.tempo_map.ppqn, 480);
    // The tempo map content changed grids — its revision must move so
    // stale TempoMapRefs cannot keep pointing at it.
    assert_eq!(project.tempo_map.revision, 4);

    // Every stored tick halved exactly; beat positions identical.
    let after_positions = musical_positions(project);
    assert_eq!(before_positions.len(), after_positions.len());
    for ((_, bt), (_, at)) in before_positions.iter().zip(&after_positions) {
        assert_eq!(beats_of(*bt, 960), beats_of(*at, 480));
    }

    // Spot-checks: clip anchor 3840→1920, extent 1920→960,
    // tempo events 960→480 / 2880→1440, signature 1920→960,
    // group [0,1920)→[0,960), region [960,1920)→[480,960).
    let clip = &project.clips[&s.clip_musical];
    assert_eq!(
        clip.placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(1_920))
    );
    assert_eq!(
        clip.placement.extent,
        ClipExtent::Musical(MusicalDuration::new(960).unwrap())
    );
    assert_eq!(project.tempo_map.tempo_events[1].tick, 480);
    assert_eq!(project.tempo_map.tempo_events[2].tick, 1_440);
    assert_eq!(project.tempo_map.signature_events[1].tick, 960);
    let ProjectRange::Musical(group_range) = project.comp.take_groups[&s.take_group].target_range
    else {
        panic!("group range must stay musical");
    };
    assert_eq!(group_range.start(), MusicalTick::ZERO);
    assert_eq!(group_range.end(), MusicalTick::new(960));
    let ProjectRange::Musical(region_range) = project.comp.regions[&s.region].project_range else {
        panic!("region range must stay musical");
    };
    assert_eq!(region_range.start(), MusicalTick::new(480));
    assert_eq!(region_range.end(), MusicalTick::new(960));
    // Punch marks re-project too: 960→480 and 2880→1440.
    assert_eq!(
        project.monitoring.punch.punch_in,
        Some(MusicalTick::new(480))
    );
    assert_eq!(
        project.monitoring.punch.punch_out,
        Some(MusicalTick::new(1_440))
    );

    // Media truth invariant: same musical span resolves to the same
    // frames under the variable tempo map.
    let map_after = tempo_map(project);
    let span_after = clip.placement.resolve_span(Some(&map_after), RATE).unwrap();
    assert_eq!(span_before, span_after);
}

/// PPQN selectable range sweep — every boundary value works when all
/// stored positions are whole-beat (exact on any grid).
#[test]
fn ppqn_sweep_covers_selectable_range() {
    for new_ppqn in [1_u32, 480, 960, 1_000, 1_920, 32_767] {
        let (mut c, _s) = seeded();
        // Skip the identity case.
        if new_ppqn == 960 {
            continue;
        }
        apply(&mut c, ppqn_op(960, new_ppqn, PpqnConversionPolicy::Exact));
        let p = c.live();
        assert_eq!(p.ppqn, ProjectPpqn(new_ppqn), "ppqn {new_ppqn}");
        assert_eq!(p.tempo_map.ppqn, new_ppqn);
        // Anchor was 4 beats → 4 * new_ppqn ticks.
        let clip = p.clips.values().next().unwrap();
        let TimeAnchor::Musical(t) = clip.placement.anchor else {
            panic!("musical clip anchor");
        };
        assert_eq!(t.value(), 4 * i64::from(new_ppqn));
        // Beat positions still identical after the sweep step.
        let beats: Vec<_> = musical_positions(p)
            .iter()
            .map(|(_, t)| beats_of(*t, new_ppqn))
            .collect();
        let (base_c, _) = seeded();
        let expected: Vec<_> = musical_positions(base_c.live())
            .iter()
            .map(|(_, t)| beats_of(*t, 960))
            .collect();
        assert_eq!(beats, expected, "ppqn {new_ppqn} beat drift");
    }
}

/// Absolute media anchors, source-native MIDI ticks/divisions, and
/// recording session/source frames are never re-projected.
#[test]
fn ppqn_change_preserves_absolute_media_and_source_native_data() {
    let (mut c, s) = seeded();
    let abs_before = c.live().clips[&s.clip_absolute].placement;
    let midi_before = c.live().midi_sources.clone();
    let smf_before = c.live().smf_documents.clone();
    let recording_before = c.live().recording.clone();
    let control_before = c.live().control.clone();
    let routing_before = c.live().routing.clone();
    let plugin_before = c.live().plugin_instances.clone();

    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));

    let p = c.live();
    assert_eq!(p.clips[&s.clip_absolute].placement, abs_before);
    assert_eq!(&p.midi_sources, &midi_before);
    assert_eq!(
        &p.midi_sources[&s.midi_source],
        &midi_before[&s.midi_source]
    );
    // The SMF document keeps its source-native division (480) and
    // event ticks even as project.ppqn moves — 異Division import.
    assert_eq!(&p.smf_documents, &smf_before);
    assert!(p.smf_documents.values().all(|d| d.division == 480));
    // Automation BeatPoints, routing, and plugin-instance identities
    // are outside the musical tick domain — bit-identical.
    assert_eq!(&p.control, &control_before);
    assert_eq!(&p.routing, &routing_before);
    assert_eq!(&p.plugin_instances, &plugin_before);
    assert_eq!(&p.recording, &recording_before);
}

/// CAS: `old_ppqn` must match the live value and
/// `expected_revision` must match — both reject without mutation.
#[test]
fn ppqn_change_enforces_double_cas() {
    let (mut c, _s) = seeded();
    // Setting-level CAS mismatch.
    rejected(&mut c, ppqn_op(100, 480, PpqnConversionPolicy::Exact));
    // Revision-level CAS mismatch.
    let rev = c.live_revision();
    let res = c.apply(Command {
        expected_revision: ProjectRevision(rev.0 + 1),
        op: ppqn_op(960, 480, PpqnConversionPolicy::Exact),
    });
    assert!(matches!(res, Err(Error::StaleRevision { .. })));
    assert_eq!(c.live().ppqn, ProjectPpqn(960));
}

/// Undo restores the pre-change ticks/ppqn exactly; redo re-applies;
/// the live revision only ever advances.
#[test]
fn ppqn_change_undo_redo_round_trip() {
    let (mut c, s) = seeded();
    let before = c.live().clone();
    let rev0 = c.live_revision();

    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));
    let mid = c.live().clone();
    assert_eq!(mid.ppqn, ProjectPpqn(480));

    assert!(c.undo().unwrap());
    assert_eq!(c.live(), &before, "undo must restore exact content");
    assert!(c.live_revision().0 > rev0.0, "undo advances revision");

    assert!(c.redo().unwrap());
    assert_eq!(c.live(), &mid, "redo must restore the ppqn change");

    // Content-level anchors after the round trip.
    let clip = &c.live().clips[&s.clip_musical];
    assert_eq!(
        clip.placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(1_920))
    );
}

/// Save → reload preserves the new PPQN and re-projected ticks; the
/// pre-change snapshot bytes stay readable and immutable (the
/// non-destructive migration contract — no schema bump was needed
/// because `ppqn` already travels with every persisted tick).
#[test]
fn ppqn_change_survives_save_reload_and_keeps_old_snapshot() {
    let (mut c, _s) = seeded();
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let old_head = store.head().unwrap().unwrap();
    let old_bytes = store.read_snapshot(&old_head).unwrap();

    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));
    c.save(&mut store).unwrap();
    let new_head = store.head().unwrap().unwrap();
    assert_ne!(old_head, new_head, "save must publish a new snapshot");

    // Old snapshot bytes are byte-identical and still carry ppqn 960 —
    // the pre-change body remains a valid, untouched snapshot.
    assert_eq!(store.read_snapshot(&old_head).unwrap(), old_bytes);
    let env: serde_json::Value = serde_json::from_slice(&old_bytes).unwrap();
    assert_eq!(env["project"]["ppqn"], serde_json::json!(960));

    let mut c2 = controller(Project::new(ProjectId::new(0), "re"));
    c2.load(&store).unwrap();
    assert_eq!(c2.live().ppqn, ProjectPpqn(480));
    assert_eq!(c2.live().tempo_map.ppqn, 480);
    let clip = c2.live().clips.values().next().unwrap();
    assert_eq!(
        clip.placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(1_920))
    );
}

/// `Exact` rejects any stored value the new grid cannot represent —
/// the command fails atomically instead of silently quantizing.
#[test]
fn ppqn_exact_policy_rejects_inexact_positions() {
    let mut c = controller(Project::new(ProjectId::new(0), "inexact"));
    // Tick 320 @960 = 1/3 beat — representable at 480 (160) but not at
    // 1000 (1000/3 ticks).
    apply(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 320,
            bpm_milli: 120_000,
        },
    );
    // 960 → 480 is exact (320 → 160).
    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));
    assert_eq!(c.live().tempo_map.tempo_events[1].tick, 160);

    // Back to a grid where 1/3 beat is not representable: 480 → 1000
    // would need 1000/3 ticks — Exact must reject.
    let (mut c2, _s) = seeded_inexact();
    rejected(&mut c2, ppqn_op(960, 1_000, PpqnConversionPolicy::Exact));
    assert_eq!(c2.live().ppqn, ProjectPpqn(960));
}

/// `Round` applies the declared ruviform rounding — explicit,
/// bounded, recorded in the command — where `Exact` rejects.
#[test]
fn ppqn_round_policy_applies_declared_rounding() {
    let (mut c, _s) = seeded_inexact();
    // Anchor tick 1 @960 = 1/960 beat → @1000 = 1000/960 = 125/120
    // ticks — inexact. Floor → 1, Ceil → 2.
    apply(
        &mut c,
        ppqn_op(960, 1_000, PpqnConversionPolicy::Round(Rounding::Floor)),
    );
    let clip = c.live().clips.values().next().unwrap();
    let TimeAnchor::Musical(t) = clip.placement.anchor else {
        panic!("musical anchor");
    };
    assert_eq!(t.value(), 1);

    let (mut c2, _s2) = seeded_inexact();
    apply(
        &mut c2,
        ppqn_op(960, 1_000, PpqnConversionPolicy::Round(Rounding::Ceil)),
    );
    let clip = c2.live().clips.values().next().unwrap();
    let TimeAnchor::Musical(t) = clip.placement.anchor else {
        panic!("musical anchor");
    };
    assert_eq!(t.value(), 2);
}

/// A project seeded with an inexact musical position (anchor tick 1
/// = 1/960 beat) for the policy tests above.
fn seeded_inexact() -> (ProjectController, Seeded) {
    let (project, mut s) = seeded_project();
    let mut c = controller(project);
    let lane = s.lane_id;
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: SourceId::new(5),
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(1)),
                extent: ClipExtent::Musical(MusicalDuration::new(960).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(96_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    s.clip_musical = *c.live().clips.keys().next().unwrap();
    (c, s)
}

/// PPQN outside `1..=32767` is rejected before any conversion runs.
#[test]
fn ppqn_out_of_range_rejected() {
    let (mut c, _s) = seeded();
    for bad in [0_u32, 32_768, u32::MAX] {
        rejected(&mut c, ppqn_op(960, bad, PpqnConversionPolicy::Exact));
    }
}

/// An open recording session is the project-visible "recording in
/// progress" gate; closed/finalized sessions do not block.
#[test]
fn ppqn_change_blocked_by_open_recording_session() {
    let (project, _s) = seeded_project();
    // Flip the seeded Finalized session to Open.
    let mut p = project;
    for session in p.recording.sessions.values_mut() {
        session.state = SessionState::Open;
    }
    let mut c = controller(p);
    rejected(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));

    // Closed sessions do not gate.
    let (project, _s2) = seeded_project();
    let mut p = project;
    for session in p.recording.sessions.values_mut() {
        session.state = SessionState::Closed;
    }
    let mut c = controller(p);
    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));
    assert_eq!(c.live().ppqn, ProjectPpqn(480));
}

/// Overflow: a position whose re-projection exceeds `i64` is an
/// explicit rejection, never a wrap.
#[test]
fn ppqn_change_rejects_tick_overflow() {
    let mut c = controller(Project::new(ProjectId::new(0), "overflow"));
    // Seed minimal structure for a clip.
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(&mut c, ProjectOp::RegisterAsset { digest: "d".into() });
    let asset = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "d".into(),
        },
    );
    let source = *c.live().sources.keys().next().unwrap();
    // Anchor = 10^15 whole beats = 9.6e17 ticks @960: @32767 the tick
    // count overflows i64 → the checked conversion must reject; @480
    // the same beat stays representable.
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(960_000_000_000_000_000)),
                extent: ClipExtent::Musical(MusicalDuration::new(960).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(96_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    rejected(&mut c, ppqn_op(960, 32_767, PpqnConversionPolicy::Exact));
    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));
    assert_eq!(c.live().ppqn, ProjectPpqn(480));
    let clip = c.live().clips.values().next().unwrap();
    let TimeAnchor::Musical(t) = clip.placement.anchor else {
        panic!("musical anchor");
    };
    assert_eq!(t.value(), 480_000_000_000_000_000);
}

/// Long arrangements: large-but-representable tick values re-project
/// exactly (no truncation at scale).
#[test]
fn ppqn_change_handles_long_arrangements() {
    let mut c = controller(Project::new(ProjectId::new(0), "long"));
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(&mut c, ProjectOp::RegisterAsset { digest: "d".into() });
    let asset = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "d".into(),
        },
    );
    let source = *c.live().sources.keys().next().unwrap();
    // 10_000_000 beats = 9.6e9 ticks @960 — a very long arrangement.
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(9_600_000_000)),
                extent: ClipExtent::Musical(MusicalDuration::new(960).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(96_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    apply(&mut c, ppqn_op(960, 32_767, PpqnConversionPolicy::Exact));
    let clip = c.live().clips.values().next().unwrap();
    let TimeAnchor::Musical(t) = clip.placement.anchor else {
        panic!("musical anchor");
    };
    assert_eq!(t.value(), 10_000_000 * 32_767);
}

/// Multiple PPQN changes compose: each step re-projects from the
/// live grid, and undo walks the whole chain back.
#[test]
fn ppqn_changes_compose_across_steps() {
    let (mut c, _s) = seeded();
    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));
    apply(&mut c, ppqn_op(480, 1_920, PpqnConversionPolicy::Exact));
    assert_eq!(c.live().ppqn, ProjectPpqn(1_920));
    let clip = c.live().clips.values().next().unwrap();
    let TimeAnchor::Musical(t) = clip.placement.anchor else {
        panic!("musical anchor");
    };
    // 4 beats @960 → 4 beats @1920 = 7680 ticks.
    assert_eq!(t.value(), 7_680);
    assert!(c.undo().unwrap());
    assert_eq!(c.live().ppqn, ProjectPpqn(480));
    assert!(c.undo().unwrap());
    assert_eq!(c.live().ppqn, ProjectPpqn(960));
}

/// Identity change (`old == new`) is a no-op: no field churn, no
/// tempo-map revision bump.
#[test]
fn ppqn_identity_change_is_noop() {
    let (mut c, _s) = seeded();
    let before = c.live().clone();
    apply(&mut c, ppqn_op(960, 960, PpqnConversionPolicy::Exact));
    assert_eq!(c.live(), &before);
}

// ---------- preview_ppqn (non-destructive dry-run) ----------

/// The preview enumerates every musical-tick site and confirms an
/// exact change applies — without mutating the project.
#[test]
fn preview_ppqn_reports_sites_and_confirms_exact_change() {
    let (c, _s) = seeded();
    let before = c.live().clone();

    let preview = preview_ppqn(c.live(), 480, PpqnConversionPolicy::Exact).unwrap();
    assert_eq!(preview.old_ppqn, 960);
    assert_eq!(preview.new_ppqn, 480);
    // tempo(3) + signature(2) + musical clip anchor/extent(2)
    // + take-group range(2) + comp-region range(2) + punch marks(2)
    // = 13 sites.
    assert_eq!(preview.site_count, 13);
    assert_eq!(preview.inexact, Vec::new());
    assert_eq!(preview.overflow_sites, Vec::new());
    assert!(preview.exact_ok);
    assert!(preview.round_ok);
    assert!(preview.applies);
    assert_eq!(preview.rejection, None);
    assert_eq!(c.live(), &before, "preview must not mutate the project");
}

/// Inexact positions are listed with their site identity; `Exact`
/// reports infeasible while `Round` stays feasible and projects the
/// declared rounding.
#[test]
fn preview_ppqn_lists_inexact_sites_and_policy_feasibility() {
    let (c, _s) = seeded_inexact();

    let preview = preview_ppqn(c.live(), 1_000, PpqnConversionPolicy::Exact).unwrap();
    // tempo(1) + signature(1) + clip anchor/extent(2) = 4 sites.
    assert_eq!(preview.site_count, 4);
    assert_eq!(preview.inexact.len(), 1);
    let site = &preview.inexact[0];
    assert_eq!(site.kind, "clip_anchor");
    assert_eq!(
        site.instance,
        c.live().clips.keys().next().unwrap().to_tagged()
    );
    assert_eq!(site.part, None);
    assert_eq!(site.tick, 1);
    // Exact cannot represent the site — no projected tick.
    assert_eq!(site.projected, None);
    assert!(!preview.exact_ok);
    assert!(preview.round_ok);
    assert!(!preview.applies);
    assert!(preview.rejection.is_some());

    let preview = preview_ppqn(
        c.live(),
        1_000,
        PpqnConversionPolicy::Round(Rounding::Floor),
    )
    .unwrap();
    assert!(preview.applies);
    assert_eq!(preview.inexact.len(), 1);
    // 1/960 beat → 1000/960 = 25/24 ticks — Floor projects to 1.
    assert_eq!(preview.inexact[0].projected, Some(1));

    let preview =
        preview_ppqn(c.live(), 1_000, PpqnConversionPolicy::Round(Rounding::Ceil)).unwrap();
    assert_eq!(preview.inexact[0].projected, Some(2));

    assert_eq!(c.live().ppqn, ProjectPpqn(960));
}

/// Overflow sites are reported separately from inexact ones and
/// fail every policy; a representable target stays feasible.
#[test]
fn preview_ppqn_reports_overflow_sites() {
    let mut c = controller(Project::new(ProjectId::new(0), "preview-overflow"));
    // 10^15 beats @960 → @32767 the tick count overflows i64.
    apply(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 960_000_000_000_000_000,
            bpm_milli: 120_000,
        },
    );

    let preview = preview_ppqn(c.live(), 32_767, PpqnConversionPolicy::Exact).unwrap();
    // default tempo(1) + default signature(1) + added tempo(1) = 3.
    assert_eq!(preview.site_count, 3);
    assert_eq!(preview.overflow_sites.len(), 1);
    let site = &preview.overflow_sites[0];
    assert_eq!(site.kind, "tempo_event");
    assert_eq!(site.instance, "1");
    assert_eq!(site.tick, 960_000_000_000_000_000);
    assert_eq!(site.projected, None);
    assert_eq!(preview.inexact, Vec::new());
    assert!(!preview.exact_ok);
    assert!(!preview.round_ok);
    assert!(!preview.applies);

    // The same beat fits @480 — the preview confirms feasibility.
    let preview = preview_ppqn(c.live(), 480, PpqnConversionPolicy::Exact).unwrap();
    assert_eq!(preview.overflow_sites, Vec::new());
    assert!(preview.applies);
}

/// Out-of-range targets are rejected by the same resolution the
/// mutating command performs.
#[test]
fn preview_ppqn_rejects_out_of_range_ppqn() {
    let (c, _s) = seeded();
    for bad in [0_u32, 32_768, u32::MAX] {
        assert!(
            preview_ppqn(c.live(), bad, PpqnConversionPolicy::Exact).is_err(),
            "ppqn {bad} must be rejected"
        );
    }
}

/// An identity preview reports every site as exact and feasible —
/// matching the identity no-op the command performs.
#[test]
fn preview_ppqn_identity_reports_all_sites_exact() {
    let (c, _s) = seeded();
    let preview = preview_ppqn(c.live(), 960, PpqnConversionPolicy::Exact).unwrap();
    assert_eq!(preview.site_count, 13);
    assert_eq!(preview.inexact, Vec::new());
    assert_eq!(preview.overflow_sites, Vec::new());
    assert!(preview.applies);
}

/// RT evidence (issue #125 acceptance): the Beat→Frame resolution a
/// musical clip produces is identical before and after a PPQN
/// re-projection — the RT-side rounding reproduces exactly, and the
/// preview's `applies` verdict matches what the command does.
#[test]
fn rt_ppqn_beat_to_frame_rounding_reproduces() {
    let (mut c, s) = seeded();
    let map_before = tempo_map(c.live());
    let clip_before = &c.live().clips[&s.clip_musical];
    let span_before = clip_before
        .placement
        .resolve_span(Some(&map_before), RATE)
        .unwrap();

    let preview = preview_ppqn(c.live(), 480, PpqnConversionPolicy::Exact).unwrap();
    assert!(preview.applies, "preview must predict the exact change");
    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));

    let map_after = tempo_map(c.live());
    let clip_after = &c.live().clips[&s.clip_musical];
    let span_after = clip_after
        .placement
        .resolve_span(Some(&map_after), RATE)
        .unwrap();
    assert_eq!(span_before, span_after);
    // Concrete frame endpoints — the rounding reproduction is
    // observable, not just span-structural equality.
    assert_eq!(
        span_before.start().frames_at(RATE),
        span_after.start().frames_at(RATE)
    );
    assert_eq!(
        span_before.end().frames_at(RATE),
        span_after.end().frames_at(RATE)
    );
    assert!(span_before.end().frames_at(RATE) > span_before.start().frames_at(RATE));
}

// ---------- TIM-01 acceptance matrix (issue #125) ----------

/// AC1: every selectable PPQN — 1/480/960/1000/1920/32767 —
/// round-trips through apply → save → reload → undo → redo with
/// identical rational-beat positions and an identical reload.
#[test]
fn ppqn_sweep_save_reload_undo_redo_matrix() {
    for new_ppqn in [1_u32, 480, 960, 1_000, 1_920, 32_767] {
        let (mut c, s) = seeded();
        let before = c.live().clone();
        // Media truth before the change: the musical clip's span
        // under the variable tempo map — Beat→Frame RT rounding must
        // reproduce identically on every target grid.
        let map_before = tempo_map(c.live());
        let span_before = c.live().clips[&s.clip_musical]
            .placement
            .resolve_span(Some(&map_before), RATE)
            .unwrap();
        let mut store = InMemoryStore::new();

        apply(&mut c, ppqn_op(960, new_ppqn, PpqnConversionPolicy::Exact));
        let mid = c.live().clone();
        assert_eq!(mid.ppqn, ProjectPpqn(new_ppqn), "ppqn {new_ppqn}");
        assert_eq!(mid.tempo_map.ppqn, new_ppqn, "ppqn {new_ppqn}");

        // Save → reload: the persisted snapshot carries the new grid
        // and every re-projected tick verbatim.
        c.save(&mut store).unwrap();
        let mut c2 = controller(Project::new(ProjectId::new(0), "re"));
        c2.load(&store).unwrap();
        assert_eq!(c2.live(), &mid, "ppqn {new_ppqn} reload mismatch");

        // Beat positions survive the whole round trip identically.
        let expected: Vec<_> = musical_positions(&before)
            .iter()
            .map(|(_, t)| beats_of(*t, 960))
            .collect();
        let got: Vec<_> = musical_positions(c2.live())
            .iter()
            .map(|(_, t)| beats_of(*t, new_ppqn))
            .collect();
        assert_eq!(got, expected, "ppqn {new_ppqn} beat drift after reload");

        // Beat→Frame: the reloaded project resolves the same musical
        // span to the same media frames — RT rounding reproduces.
        let map_re = tempo_map(c2.live());
        let span_re = c2.live().clips[&s.clip_musical]
            .placement
            .resolve_span(Some(&map_re), RATE)
            .unwrap();
        assert_eq!(span_before, span_re, "ppqn {new_ppqn} beat→frame drift");

        // Undo restores the pre-change project exactly; redo
        // re-applies the change — one undo entry per command.
        assert!(c.undo().unwrap(), "ppqn {new_ppqn} undo");
        assert_eq!(c.live(), &before, "ppqn {new_ppqn} undo mismatch");
        assert!(c.redo().unwrap(), "ppqn {new_ppqn} redo");
        assert_eq!(c.live(), &mid, "ppqn {new_ppqn} redo mismatch");
    }
}

/// The real v5 snapshot bytes (`schema_version` 5, last v5 writer —
/// provenance in `evidence/issue-86-v5-migration/`).
const V5_SNAPSHOT: &[u8] = include_bytes!("fixtures/alib02-v5-legacy.json");
const V5_PROJECT_ID: &str = "project-00000000000000000000000000000086";
const V5_REVISION: u64 = 22;

/// AC2: a real v5 snapshot migrates to current schema and then
/// accepts a PPQN change: migrated musical ticks re-project through
/// rational beats while source-native MIDI data, digests, and the
/// v5 source bytes stay untouched.
#[test]
fn ppqn_change_on_v5_snapshot_preserves_migrated_data() {
    let mut store = InMemoryStore::new();
    let v5_head = store
        .save(V5_PROJECT_ID, ProjectRevision(V5_REVISION), V5_SNAPSHOT)
        .unwrap();

    let mut c = controller(Project::new(ProjectId::new(0), "v5"));
    c.load(&store).unwrap();
    assert_eq!(c.live().ppqn, ProjectPpqn(960), "v5 project PPQN");

    let before = c.live().clone();
    // The migrated musical positions are all whole-beat — Exact is
    // feasible on every target grid.
    let before_positions = musical_positions(&before);

    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));
    let mid = c.live().clone();
    assert_eq!(mid.ppqn, ProjectPpqn(480));
    assert_eq!(mid.tempo_map.ppqn, 480);

    // Migrated musical ticks re-projected: same beats, halved ticks.
    let after_positions = musical_positions(&mid);
    assert_eq!(before_positions.len(), after_positions.len());
    for ((_, bt), (_, at)) in before_positions.iter().zip(&after_positions) {
        assert_eq!(beats_of(*bt, 960), beats_of(*at, 480));
    }
    // Concrete migrated values: clip extent 7680→3840 (8 beats),
    // tempo event 3840→1920 (4 beats), signature 1920→960 (2 beats).
    let clip = mid.clips.values().next().unwrap();
    assert_eq!(
        clip.placement.extent,
        ClipExtent::Musical(MusicalDuration::new(3_840).unwrap())
    );

    // Source-native and digest-bearing state is outside the walk.
    assert_eq!(mid.midi_sources, before.midi_sources);
    assert_eq!(mid.applied_imports, before.applied_imports);
    assert_eq!(mid.assets, before.assets);
    assert_eq!(mid.sources, before.sources);
    assert_eq!(mid.control, before.control);

    // Save publishes a new snapshot; the v5 bytes stay immutable and
    // reloadable; the reloaded project equals the migrated+changed one.
    let v5_before = store.read_snapshot(&v5_head).unwrap();
    c.save(&mut store).unwrap();
    assert_ne!(store.head().unwrap().unwrap(), v5_head);
    assert_eq!(store.read_snapshot(&v5_head).unwrap(), v5_before);
    assert_eq!(store.read_snapshot(&v5_head).unwrap(), V5_SNAPSHOT);

    let mut c2 = controller(Project::new(ProjectId::new(0), "v5-re"));
    c2.load(&store).unwrap();
    assert_eq!(c2.live(), &mid);
}

/// AC3 denominator boundary: tick 1 @960 is 1/960 beat — on the
/// 32767 grid that is 32767/960 ticks, far from integral. `Exact`
/// rejects; `Round` applies only the declared rounding, and the
/// boundary PPQN 32767 itself is exercised.
#[test]
fn ppqn_denominator_boundary_is_explicit() {
    let (c, _s) = seeded_inexact();

    let preview = preview_ppqn(c.live(), 32_767, PpqnConversionPolicy::Exact).unwrap();
    assert_eq!(preview.site_count, 4);
    assert_eq!(preview.inexact.len(), 1);
    assert_eq!(preview.inexact[0].kind, "clip_anchor");
    assert_eq!(preview.inexact[0].tick, 1);
    assert_eq!(preview.inexact[0].projected, None);
    assert!(!preview.exact_ok);
    assert!(preview.round_ok);
    assert!(!preview.applies);

    // 1/960 beat → 32767/960 = 34.13… ticks: Floor 34, Ceil 35.
    let preview = preview_ppqn(
        c.live(),
        32_767,
        PpqnConversionPolicy::Round(Rounding::Floor),
    )
    .unwrap();
    assert_eq!(preview.inexact[0].projected, Some(34));
    let preview = preview_ppqn(
        c.live(),
        32_767,
        PpqnConversionPolicy::Round(Rounding::Ceil),
    )
    .unwrap();
    assert_eq!(preview.inexact[0].projected, Some(35));

    // The mutating command honors the same declared rounding.
    let (mut c2, _s2) = seeded_inexact();
    apply(
        &mut c2,
        ppqn_op(960, 32_767, PpqnConversionPolicy::Round(Rounding::Floor)),
    );
    assert_eq!(c2.live().ppqn, ProjectPpqn(32_767));
    let clip = c2.live().clips.values().next().unwrap();
    let TimeAnchor::Musical(t) = clip.placement.anchor else {
        panic!("musical anchor");
    };
    assert_eq!(t.value(), 34);
}

/// AC3 same-tick sites: a tempo event, a signature event, a clip
/// anchor/extent, and a punch-in mark all sitting on tick 960
/// re-project to the same target tick — no collision, no drift.
#[test]
fn ppqn_change_handles_same_tick_sites() {
    let mut c = controller(Project::new(ProjectId::new(0), "same-tick"));
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(&mut c, ProjectOp::RegisterAsset { digest: "d".into() });
    let asset = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "d".into(),
        },
    );
    let source = *c.live().sources.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(960)),
                extent: ClipExtent::Musical(MusicalDuration::new(960).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(96_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    apply(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 960,
            bpm_milli: 90_000,
        },
    );
    apply(
        &mut c,
        ProjectOp::AddSignatureEvent {
            tick: 960,
            numerator: 3,
            denominator: 4,
        },
    );
    apply(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(960)),
            punch_out: Some(MusicalTick::new(1_920)),
        },
    );

    apply(&mut c, ppqn_op(960, 480, PpqnConversionPolicy::Exact));

    // Every site that sat on tick 960 (1 beat) lands on 480; the
    // punch-out at 2 beats lands on 960.
    assert_eq!(c.live().tempo_map.tempo_events[1].tick, 480);
    assert_eq!(c.live().tempo_map.signature_events[1].tick, 480);
    let clip = c.live().clips.values().next().unwrap();
    assert_eq!(
        clip.placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(480))
    );
    assert_eq!(
        clip.placement.extent,
        ClipExtent::Musical(MusicalDuration::new(480).unwrap())
    );
    assert_eq!(
        c.live().monitoring.punch.punch_in,
        Some(MusicalTick::new(480))
    );
    assert_eq!(
        c.live().monitoring.punch.punch_out,
        Some(MusicalTick::new(960))
    );
}

/// The preview enumerates punch marks as first-class sites: an
/// inexact punch position is reported with its own site kind and
/// the same feasibility verdict the command reaches.
#[test]
fn preview_ppqn_reports_inexact_punch_marks() {
    let mut c = controller(Project::new(ProjectId::new(0), "punch-preview"));
    // punch_in tick 1 @960 = 1/960 beat — inexact @1000 (25/24);
    // punch_out tick 960 = 1 beat — exact.
    apply(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(1)),
            punch_out: Some(MusicalTick::new(960)),
        },
    );

    let preview = preview_ppqn(c.live(), 1_000, PpqnConversionPolicy::Exact).unwrap();
    // default tempo(1) + signature(1) + punch_in(1) + punch_out(1).
    assert_eq!(preview.site_count, 4);
    assert_eq!(preview.inexact.len(), 1);
    let site = &preview.inexact[0];
    assert_eq!(site.kind, "punch_in");
    assert_eq!(site.instance, "project");
    assert_eq!(site.tick, 1);
    assert_eq!(site.projected, None);
    assert!(!preview.applies);

    let preview = preview_ppqn(
        c.live(),
        1_000,
        PpqnConversionPolicy::Round(Rounding::Floor),
    )
    .unwrap();
    // 25/24 ticks → Floor 1.
    assert_eq!(preview.inexact[0].projected, Some(1));
    assert!(preview.applies);
    assert_eq!(c.live().ppqn, ProjectPpqn(960), "preview is read-only");
}
