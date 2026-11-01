//! `recording-poc`: deterministic headless F0 scenario.
//!
//! Builds a project through `ProjectController`, registers a recording
//! session's entities, saves through `DirectoryBundleStore`, reloads, and
//! prints a deterministic summary. Two runs must produce identical
//! stdout (see `tests/poc_determinism.rs`).

use std::path::PathBuf;

use synvelis_project::{
    Command, CounterIdGenerator, DirectoryBundleStore, Project, ProjectId, ProjectOp, SessionFrame,
    SessionRange, SourceFrame, SourceRange,
};
use synvelis_recording::{RecordingOp, SynvelisOp};

fn cmd<C: synvelis_project::Operation>(
    controller: &synvelis_project::ProjectController,
    op: C,
) -> Command<C> {
    Command {
        expected_revision: controller.live_revision(),
        op,
    }
}

macro_rules! apply {
    ($c:expr, $op:expr) => {
        $c.apply(cmd(&$c, $op)).expect("command must apply")
    };
}

fn main() {
    if let Err(e) = run() {
        eprintln!("recording-poc failed: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), synvelis_project::Error> {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("poc-out");
    let _ = std::fs::remove_dir_all(&out_dir);
    let mut store = DirectoryBundleStore::create(&out_dir)?;

    let mut controller = synvelis_project::ProjectController::new(
        Project::new(ProjectId::new(0), "vs-a-poc"),
        Box::new(CounterIdGenerator::new(1)),
    );

    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::AddTrack {
            name: "drums".into(),
        })
    );
    let track_id = *controller.live().tracks.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::AddLane { track_id })
    );
    let lane_id = *controller.live().lanes.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::RegisterAsset {
            digest: "sha256:drums-take".into(),
        })
    );
    let asset_id = *controller.live().assets.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::RegisterSource {
            asset_id,
            content_digest: "sha256:drums-take".into(),
        })
    );
    let source_id = *controller.live().sources.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::RegisterClockEpoch {
            description: "audio-device-clock".into(),
        })
    );
    let clock_epoch_id = *controller.live().clock_epochs.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::CreateSession {
            name: "take-1".into(),
        })
    );
    let session_id = *controller.live().recording.sessions.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::OpenSession { session_id })
    );
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::RegisterPass {
            session_id,
            index: 0,
        })
    );
    let pass_id = *controller.live().recording.passes.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::RegisterStream {
            session_id,
            lane_id,
            clock_epoch_id,
        })
    );
    let stream_id = *controller.live().recording.streams.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::RegisterSegment {
            stream_id,
            pass_id,
            session_range: SessionRange::try_new(
                SessionFrame::new(0)?,
                SessionFrame::new(96_000)?,
            )?,
            source_range: SourceRange::try_new(SourceFrame::new(0)?, SourceFrame::new(96_000)?,)?,
            source_id,
        })
    );
    let segment_id = *controller.live().recording.segments.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::RegisterTake {
            session_id,
            pass_id,
            stream_id,
            lane_id,
            source_id,
            segment_ids: vec![segment_id],
        })
    );
    let take_id = *controller.live().recording.takes.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::MarkTakeVerified { take_id })
    );
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::CloseSession { session_id })
    );

    let saved = controller.save(&mut store)?;

    let mut reloaded = synvelis_project::ProjectController::new(
        Project::new(ProjectId::new(0), "placeholder"),
        Box::new(CounterIdGenerator::new(1)),
    );
    reloaded.load(&store)?;

    let same = serde_json::to_value(controller.live()).unwrap()
        == serde_json::to_value(reloaded.live()).unwrap();

    println!("project_id={}", controller.live().project_id);
    println!("saved_revision={}", saved.revision.0);
    println!("saved_head={}", saved.head);
    println!("reloaded_revision={}", reloaded.live_revision().0);
    println!("tracks={}", controller.live().tracks.len());
    println!("sessions={}", controller.live().recording.sessions.len());
    println!("takes={}", controller.live().recording.takes.len());
    println!("roundtrip_equal={same}");
    assert!(same, "reload mismatch");
    println!("result=OK");
    Ok(())
}
