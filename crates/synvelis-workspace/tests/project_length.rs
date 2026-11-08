//! Project length in seconds — the seek range source (SV175-02 slice 2).
//!
//! Fixture-only mutations go straight to `ProjectSession::apply`; the
//! function under test is `viz_ui::project_length_seconds`.

use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, CounterIdGenerator, MusicalDuration, MusicalTick,
    PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    SourceFrame, SourceRange, TimeAnchor,
};
use synvelis_workspace::ProjectSession;
use synvelis_workspace::viz_ui::project_length_seconds;

const MEDIA_RATE: u32 = 48_000;

fn fresh_session() -> ProjectSession {
    let project = Project::new(ProjectId::new(0x5EE), "length-test");
    ProjectSession::new(ProjectController::new(
        project,
        Box::new(CounterIdGenerator::new(1)),
    ))
}

fn apply(session: &mut ProjectSession, op: ProjectOp) {
    let rev: ProjectRevision = session.revision();
    session.apply(op, rev).expect("fixture op");
}

/// A session with one lane on one source; returns nothing — the
/// clips are added with [`add_musical_clip`].
fn session_with_lane() -> ProjectSession {
    let mut session = fresh_session();
    apply(
        &mut session,
        ProjectOp::RegisterAsset {
            digest: "deadbeef".to_string(),
        },
    );
    let asset_id = *session.project().assets.keys().next().expect("asset");
    apply(
        &mut session,
        ProjectOp::RegisterSource {
            asset_id,
            content_digest: "deadbeef".to_string(),
        },
    );
    apply(
        &mut session,
        ProjectOp::AddTrack {
            name: "t1".to_string(),
        },
    );
    let track_id = *session.project().tracks.keys().next().expect("track");
    apply(&mut session, ProjectOp::AddLane { track_id });
    session
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
fn empty_project_has_zero_length() {
    assert_eq!(project_length_seconds(&fresh_session(), MEDIA_RATE), 0.0);
}

#[test]
fn length_is_the_latest_clip_end_in_seconds() {
    // Default tempo map: 120 BPM at PPQN 960 → 960 ticks = 1 beat = 0.5 s.
    // Clips end at tick 960 (0.5 s) and tick 2880 (1.5 s).
    let mut session = session_with_lane();
    add_musical_clip(&mut session, 0, 960);
    add_musical_clip(&mut session, 960, 1920);
    let length = project_length_seconds(&session, MEDIA_RATE);
    assert!((length - 1.5).abs() < 1e-9, "length was {length}");
}
