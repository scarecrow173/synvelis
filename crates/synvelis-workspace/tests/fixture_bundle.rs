//! Evidence fixture for SV175 real-window verification — ignored by
//! default. Writes a clip-bearing project bundle through the same
//! `DirectoryBundleStore` + `ProjectController` save path the app uses,
//! so `synvelis-native` can `syn.project.reload` it and observe the
//! Seek range follow the project length.
//!
//! Run manually:
//!
//! ```sh
//! SV175_FIXTURE_BUNDLE=<bundle-dir> \
//!   cargo test -p synvelis-workspace --test fixture_bundle -- --ignored
//! ```
//!
//! The bundle at `<bundle-dir>` is loaded when present (mirroring
//! `syn.project.open`), two musical clips ending at tick 2880 (1.5 s at
//! the default 120 BPM / PPQN 960 tempo map) are appended, and the
//! result is saved as the next revision.

use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, CounterIdGenerator, DirectoryBundleStore,
    MusicalDuration, MusicalTick, PlaybackMapping, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, SourceFrame, SourceRange, TimeAnchor,
};
use synvelis_workspace::ProjectSession;

fn apply(session: &mut ProjectSession, op: ProjectOp) {
    let rev: ProjectRevision = session.revision();
    session.apply(op, rev).expect("fixture op");
}

fn add_musical_clip(session: &mut ProjectSession, anchor: i64, extent: i64) {
    let lane_id = *session.project().lanes.keys().next().expect("lane");
    let source_id = *session.project().sources.keys().next().expect("source");
    apply(
        session,
        ProjectOp::AddClip {
            lane_id,
            source_id,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(anchor)),
                extent: ClipExtent::Musical(MusicalDuration::new(extent).expect("extent")),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_000_000).expect("frame"),
                )
                .expect("region"),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
}

#[test]
#[ignore = "manual evidence fixture — set SV175_FIXTURE_BUNDLE to a bundle dir"]
fn write_clip_bundle() {
    let dir = std::env::var("SV175_FIXTURE_BUNDLE").expect("SV175_FIXTURE_BUNDLE not set");
    let mut store = DirectoryBundleStore::create(&dir).expect("bundle store");
    let mut session = ProjectSession::new(ProjectController::new(
        Project::new(ProjectId::new(0), "fixture"),
        Box::new(CounterIdGenerator::new(1)),
    ));
    session
        .controller_mut()
        .load(&store)
        .expect("load existing bundle");

    apply(
        &mut session,
        ProjectOp::RegisterAsset {
            digest: "cafebabe".to_string(),
        },
    );
    let asset_id = *session.project().assets.keys().next().expect("asset");
    apply(
        &mut session,
        ProjectOp::RegisterSource {
            asset_id,
            content_digest: "cafebabe".to_string(),
        },
    );
    apply(
        &mut session,
        ProjectOp::AddTrack {
            name: "fixture".to_string(),
        },
    );
    let track_id = *session.project().tracks.keys().next().expect("track");
    apply(&mut session, ProjectOp::AddLane { track_id });
    add_musical_clip(&mut session, 0, 960);
    add_musical_clip(&mut session, 960, 1920);

    session
        .controller_mut()
        .save(&mut store)
        .expect("save bundle");
    eprintln!("fixture bundle written: {dir}");
}
