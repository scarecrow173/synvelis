//! Arrangement read model rows (SV-GUI-03 slice 2, Issue #112).
//!
//! Fixture-only mutations go straight to `ProjectSession::apply`; the
//! function under test is `arrangement_rows`.

use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, CounterIdGenerator, MusicalDuration, MusicalTick,
    PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    SourceFrame, SourceRange, TimeAnchor,
};
use synvelis_workspace::ProjectSession;
use synvelis_workspace::ArrangementRow;
use synvelis_workspace::arrangement_rows;

const MEDIA_RATE: u32 = 48_000;

fn fresh_session() -> ProjectSession {
    let project = Project::new(ProjectId::new(0x5EE), "arrangement-test");
    ProjectSession::new(ProjectController::new(
        project,
        Box::new(CounterIdGenerator::new(1)),
    ))
}

fn apply(session: &mut ProjectSession, op: ProjectOp) {
    let rev: ProjectRevision = session.revision();
    session.apply(op, rev).expect("fixture op");
}

/// A session with one asset+source, two tracks (second with two
/// lanes), and returns it — clips are added per-test.
fn session_with_structure() -> ProjectSession {
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
    apply(
        &mut session,
        ProjectOp::AddTrack {
            name: "t2".to_string(),
        },
    );
    // t2 gets two lanes so lane order is observable; t1 keeps one.
    let track_ids: Vec<_> = session.project().tracks.keys().copied().collect();
    let (t1, t2) = (track_ids[0], track_ids[1]);
    apply(&mut session, ProjectOp::AddLane { track_id: t1 });
    apply(&mut session, ProjectOp::AddLane { track_id: t2 });
    apply(&mut session, ProjectOp::AddLane { track_id: t2 });
    session
}

fn lane_ids(session: &ProjectSession, track_pos: usize) -> Vec<synvelis_project::LaneId> {
    let track = session
        .project()
        .tracks
        .values()
        .nth(track_pos)
        .expect("track");
    track.lane_ids.clone()
}

fn add_clip(
    session: &mut ProjectSession,
    lane_id: synvelis_project::LaneId,
    anchor: i64,
    len: i64,
) {
    let source_id = *session.project().sources.keys().next().expect("source");
    apply(
        session,
        ProjectOp::AddClip {
            lane_id,
            source_id,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(anchor)),
                extent: ClipExtent::Musical(MusicalDuration::new(len).expect("extent")),
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
fn empty_project_has_no_rows() {
    assert_eq!(
        arrangement_rows(&fresh_session(), MEDIA_RATE),
        Vec::<ArrangementRow>::new()
    );
}

#[test]
fn rows_walk_tracks_lanes_clips_in_persisted_order() {
    let mut session = session_with_structure();
    // t1/lane1: one clip; t2/lane1: one clip; t2/lane2: one clip.
    let t1_lane = lane_ids(&session, 0)[0];
    let t2_lanes = lane_ids(&session, 1);
    add_clip(&mut session, t1_lane, 0, 960);
    add_clip(&mut session, t2_lanes[0], 0, 480);
    add_clip(&mut session, t2_lanes[1], 480, 480);

    let rows = arrangement_rows(&session, MEDIA_RATE);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].track, "t1");
    assert_eq!(rows[1].track, "t2");
    assert_eq!(rows[2].track, "t2");
    assert_eq!(rows[1].lane, t2_lanes[0].to_tagged());
    assert_eq!(rows[2].lane, t2_lanes[1].to_tagged());
    // Default tempo map: 960 ticks = 1 beat = 0.5 s at 120 BPM.
    // Clip 1: [0, 960) ticks → 0.0–0.5 s; clip 2: [0, 480) → 0.0–0.25 s;
    // clip 3: [480, 960) → 0.25–0.5 s.
    assert_eq!(rows[0].start_seconds, Some(0.0));
    assert!((rows[0].end_seconds.expect("end") - 0.5).abs() < 1e-9);
    assert!((rows[2].start_seconds.expect("start") - 0.25).abs() < 1e-9);
    assert!(rows.iter().all(|r| !r.muted));
    assert!(rows.iter().all(|r| r.source.starts_with("source-")));
    assert!(rows.iter().all(|r| r.clip.starts_with("clip-")));
}

#[test]
fn muted_flag_is_carried() {
    let mut session = session_with_structure();
    let lane = lane_ids(&session, 0)[0];
    add_clip(&mut session, lane, 0, 960);
    let clip_id = *session.project().clips.keys().next().expect("clip");
    apply(
        &mut session,
        ProjectOp::SetClipMuted {
            clip_id,
            muted: true,
        },
    );
    let rows = arrangement_rows(&session, MEDIA_RATE);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].muted);
}
