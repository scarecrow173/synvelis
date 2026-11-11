//! Emits a genuine schema-v5 project snapshot produced by this
//! checkout's own serializer (commit a401e5c, the last v5 build).
//! Usage: `cargo run -p synvelis-recording --example emit_v5_fixture -- <out.json>`
//! The companion media blob is written to `<out>.blob` and its real
//! forward+reverse CRC32-fold digest is embedded in the snapshot.

use std::{env, fs};

use synvelis_project::*;
use synvelis_recording::crc32::crc32;

fn content_digest(bytes: &[u8]) -> String {
    let fwd = u64::from(crc32(bytes));
    let mut rev = bytes.to_vec();
    rev.reverse();
    let bwd = u64::from(crc32(&rev));
    format!("{fwd:08x}{bwd:08x}")
}

fn main() {
    let out = env::args().nth(1).expect("usage: emit_v5_fixture <out.json>");
    let blob_bytes: Vec<u8> = (0u32..9600).map(|i| (i * 7 % 251) as u8).collect();
    let digest = content_digest(&blob_bytes);
    println!("blob digest: {digest}");

    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0x86), "legacy-v5"),
        Box::new(CounterIdGenerator::new(1)),
    );
    macro_rules! apply {
        ($op:expr) => {
            c.apply(Command {
                expected_revision: c.live_revision(),
                op: $op,
            })
            .unwrap()
        };
    }

    apply!(ProjectOp::SetPpqn { ppqn: 960 });
    apply!(ProjectOp::AddTempoEvent {
        tick: 3_840,
        bpm_milli: 140_000
    });
    apply!(ProjectOp::AddSignatureEvent {
        tick: 1_920,
        numerator: 3,
        denominator: 4
    });
    apply!(ProjectOp::AddTrack {
        name: "gtr".into()
    });
    let track = *c.live().tracks.keys().next().unwrap();
    apply!(ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();

    // media asset+source carrying the flat-blob CRC fold digest
    apply!(ProjectOp::RegisterAsset {
        digest: digest.clone()
    });
    let asset = *c.live().assets.keys().next().unwrap();
    apply!(ProjectOp::RegisterSource {
        asset_id: asset,
        content_digest: digest.clone()
    });
    let source = *c.live().sources.keys().next().unwrap();
    apply!(ProjectOp::SetSourceState {
        source_id: source,
        state: SourceState::Verified
    });

    // clip + clip-local note
    apply!(ProjectOp::AddClip {
        lane_id: lane,
        source_id: source,
        placement: ClipPlacement {
            anchor: TimeAnchor::Musical(MusicalTick::new(0)),
            extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
        },
        playback: ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::ZERO,
                SourceFrame::new(384_000).unwrap()
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    });
    let clip = *c.live().clips.keys().next().unwrap();
    apply!(ProjectOp::AddNote {
        clip,
        key: 64,
        start: BeatPoint::new(1, 4).unwrap(),
        len: BeatPoint::new(1, 2).unwrap(),
    });

    // MIDI source + midi-anchored second lane clip
    apply!(ProjectOp::AddMidiSource {
        division: MidiDivision::ppqn(480).unwrap()
    });
    let midi_source = *c.live().midi_sources.keys().next().unwrap();
    apply!(ProjectOp::AddMidiNote {
        source_id: midi_source,
        key: 60,
        velocity: 100,
        start: MidiTick::new(0).unwrap(),
        len: MidiTick::new(240).unwrap(),
    });
    apply!(ProjectOp::AddMidiNote {
        source_id: midi_source,
        key: 67,
        velocity: 90,
        start: MidiTick::new(240).unwrap(),
        len: MidiTick::new(240).unwrap(),
    });
    apply!(ProjectOp::AddLane { track_id: track });
    let lane2 = *c.live().lanes.keys().next_back().unwrap();
    apply!(ProjectOp::AddClip {
        lane_id: lane2,
        source_id: midi_source,
        placement: ClipPlacement {
            anchor: TimeAnchor::Musical(MusicalTick::new(0)),
            extent: ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap()),
        },
        playback: ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::ZERO,
                SourceFrame::new(480).unwrap()
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    });

    // control model: scope-tagged owners (track / clip / project),
    // automation lane with a written region, modulation source+assignment
    apply!(ProjectOp::AddParameterTarget {
        name: "track-vol".into(),
        owner: ControlScope::Track(track),
        kind: ValueKind::Continuous,
        mapping: ParamMapping::Linear,
        capability: None,
        base: 0.75,
    });
    let target = *c.live().control.targets.keys().next().unwrap();
    apply!(ProjectOp::AddParameterTarget {
        name: "clip-trim".into(),
        owner: ControlScope::Clip(clip),
        kind: ValueKind::Continuous,
        mapping: ParamMapping::Linear,
        capability: Some(TargetCapability {
            param_id: 7,
            flags_bits: 0,
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.5,
        }),
        base: 0.5,
    });
    apply!(ProjectOp::AddAutomationLane { target });
    let aut = *c.live().control.lanes.keys().next().unwrap();
    apply!(ProjectOp::WriteAutomationRegion {
        lane: aut,
        region: AutomationRegion {
            range: BeatRange::try_new(
                BeatPoint::new(0, 1).unwrap(),
                BeatPoint::new(4, 1).unwrap()
            )
            .unwrap(),
            interpolation: Interpolation::Linear,
            points: vec![
                AutomationPoint {
                    pos: BeatPoint::new(0, 1).unwrap(),
                    value: 0.25,
                },
                AutomationPoint {
                    pos: BeatPoint::new(3, 1).unwrap(),
                    value: 0.75,
                },
            ],
        },
        policy: ConflictPolicy::Reject,
    });
    apply!(ProjectOp::AddModulationSource {
        owner: ControlScope::Project,
        kind: SourceKind::Lfo {
            rate: BeatPoint::new(1, 1).unwrap(),
            shape: LfoShape::Sine,
            bipolar: true,
        },
    });
    let msrc = *c.live().control.sources.keys().next().unwrap();
    apply!(ProjectOp::AddModulationAssignment {
        source: msrc,
        target,
        depth: 0.1,
        depth_source: None,
        polarity: Polarity::Bipolar,
        transform: Transform::Identity,
        enabled: true,
    });

    // clock epoch + recording registry (session/pass/stream/segment/take)
    apply!(ProjectOp::RegisterClockEpoch {
        description: "v5-era device clock".into()
    });
    let epoch = *c.live().clock_epochs.keys().next().unwrap();

    let mut project = c.live().clone();
    let session = RecordingSessionId::new(0x5000);
    let pass = RecordingPassId::new(0x5001);
    let stream = CaptureStreamId::new(0x5002);
    let segment = CaptureSegmentId::new(0x5003);
    let take = TakeId::new(0x5004);
    project.recording.sessions.insert(
        session,
        RecordingSession {
            id: session,
            name: "v5-session".into(),
            state: SessionState::Closed,
        },
    );
    project.recording.passes.insert(
        pass,
        RecordingPass {
            id: pass,
            session_id: session,
            index: 1,
        },
    );
    project.recording.streams.insert(
        stream,
        CaptureStream {
            id: stream,
            session_id: session,
            lane_id: lane,
            clock_epoch_id: epoch,
        },
    );
    project.recording.segments.insert(
        segment,
        CaptureSegment {
            id: segment,
            stream_id: stream,
            pass_id: pass,
            session_range: SessionRange::try_new(
                SessionFrame::ZERO,
                SessionFrame::new(96_000).unwrap(),
            )
            .unwrap(),
            source_range: SourceRange::try_new(
                SourceFrame::ZERO,
                SourceFrame::new(96_000).unwrap(),
            )
            .unwrap(),
            source_id: source,
        },
    );
    project.recording.takes.insert(
        take,
        Take {
            id: take,
            session_id: session,
            pass_id: pass,
            stream_id: stream,
            lane_id: lane,
            source_id: source,
            segment_ids: vec![segment],
            state: TakeState::Raw,
        },
    );

    // L1-era applied-import receipt pointing at the live asset/source
    let import_id = ImportId::new(0x6000);
    project.applied_imports.insert(
        import_id,
        AppliedImport {
            content_hash: digest.clone(),
            asset_id: asset,
            source_id: source,
        },
    );
    project.id_watermark = 0x7000;
    project.validate_references().unwrap();

    let envelope = SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        project_id: project.project_id,
        revision: c.live_revision(),
        project,
    };
    assert_eq!(SCHEMA_VERSION, 5);
    let bytes = serde_json::to_vec(&envelope).unwrap();
    fs::write(&out, &bytes).unwrap();
    fs::write(format!("{out}.blob"), &blob_bytes).unwrap();
    println!("wrote {} ({} bytes)", out, bytes.len());
}
