//! synvelis#432 (AUTX-04-F01): `ProjectOp::Composite::validate` runs a
//! sequential dry-run on a scratch clone, so a member may reference an
//! ID an earlier member mints (create-then-reference) — while the
//! early-validate order, atomicity, stale-revision, and single-undo
//! contracts stay intact.

use synvelis_project::*;

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: ProjectOp) -> Command<ProjectOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

/// One committed op makes `live.id_watermark` equal the generator
/// position — the invariant every post-commit state satisfies — so the
/// watermark-seeded validate dry-run issues the same ID series the
/// real apply will.
fn synced_controller() -> ProjectController {
    let mut c = controller();
    c.apply(cmd(
        &c,
        ProjectOp::AddTrack {
            name: "seed".into(),
        },
    ))
    .unwrap();
    c
}

// ── create-then-reference commits through the controller ─────────

#[test]
fn composite_create_then_reference_commits_as_one_undo_step() {
    let mut c = synced_controller();
    // The next issued raw is what member 0's `AddTrack` mints — in
    // validate's watermark-seeded dry-run and in the real apply alike
    // — so member 1 can name it up front. This is the same shape the
    // gateway `bindings` seam resolves placeholders into.
    let predicted = TrackId::new(c.peek_raw_id());
    let rev = c.live_revision();
    c.apply(cmd(
        &c,
        ProjectOp::Composite {
            ops: vec![
                ProjectOp::AddTrack { name: "t".into() },
                ProjectOp::AddLane {
                    track_id: predicted,
                },
                ProjectOp::AddLane {
                    track_id: predicted,
                },
            ],
        },
    ))
    .unwrap();

    // The minted ID is exactly the predicted raw: validate's dry-run
    // and apply issued the same deterministic series.
    assert!(c.live().tracks.contains_key(&predicted));
    assert_eq!(c.live().tracks[&predicted].name, "t");
    assert_eq!(c.live().tracks[&predicted].lane_ids.len(), 2);
    for lane in c.live().lanes.values() {
        assert_eq!(lane.track_id, predicted);
    }
    assert_eq!(c.live_revision(), ProjectRevision(rev.0 + 1));

    // The whole composite is a single undo step: one revert removes
    // the track and both lanes together, redo restores them.
    assert!(c.undo().unwrap());
    assert!(!c.live().tracks.contains_key(&predicted));
    assert!(c.live().lanes.is_empty());
    assert!(c.redo().unwrap());
    assert!(c.live().tracks.contains_key(&predicted));
    assert_eq!(c.live().lanes.len(), 2);
}

#[test]
fn composite_forward_reference_works_for_asset_source_pair() {
    let mut c = synced_controller();
    let predicted_asset = AssetId::new(c.peek_raw_id());
    c.apply(cmd(
        &c,
        ProjectOp::Composite {
            ops: vec![
                ProjectOp::RegisterAsset { digest: "d".into() },
                ProjectOp::RegisterSource {
                    asset_id: predicted_asset,
                    content_digest: "d".into(),
                },
            ],
        },
    ))
    .unwrap();
    assert!(c.live().assets.contains_key(&predicted_asset));
    let source = c.live().sources.values().next().unwrap();
    assert_eq!(source.asset_id, predicted_asset);
}

// ── early validate / atomicity / revision contracts unchanged ────

#[test]
fn composite_member_failure_never_reaches_later_members() {
    let mut c = controller();
    let rev = c.live_revision();
    // member 0 references a missing track — the command fails at
    // member 0 and member 1's `AddTrack` is never validated or applied.
    let err = c
        .apply(cmd(
            &c,
            ProjectOp::Composite {
                ops: vec![
                    ProjectOp::AddLane {
                        track_id: TrackId::new(999),
                    },
                    ProjectOp::AddTrack {
                        name: "never".into(),
                    },
                ],
            },
        ))
        .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
    assert_eq!(c.live_revision(), rev);
    assert!(c.live().tracks.is_empty());
    assert!(c.live().lanes.is_empty());
    // The rejected command pushed no undo entry.
    assert!(!c.undo().unwrap());
}

#[test]
fn composite_mid_member_failure_discards_the_whole_dry_run() {
    let mut c = controller();
    let rev = c.live_revision();
    // member 0 mints a track in the dry-run; member 1 references a
    // track that exists nowhere — validate fails and the real apply
    // never runs, so nothing commits.
    let err = c
        .apply(cmd(
            &c,
            ProjectOp::Composite {
                ops: vec![
                    ProjectOp::AddTrack { name: "t".into() },
                    ProjectOp::AddLane {
                        track_id: TrackId::new(4242),
                    },
                ],
            },
        ))
        .unwrap_err();
    assert!(matches!(err, Error::NotFound(_)));
    assert_eq!(c.live_revision(), rev);
    // No partial composite state reached the live project.
    assert!(c.live().tracks.is_empty());
    assert!(c.live().lanes.is_empty());
    // The rejected command pushed no undo entry.
    assert!(!c.undo().unwrap());
}

#[test]
fn composite_rejects_empty_and_nested_with_stable_vocabulary() {
    let mut c = controller();
    let rev = c.live_revision();
    for (op, want) in [
        (ProjectOp::Composite { ops: vec![] }, "empty composite"),
        (
            ProjectOp::Composite {
                ops: vec![ProjectOp::Composite {
                    ops: vec![ProjectOp::AddTrack { name: "n".into() }],
                }],
            },
            "nested composite",
        ),
    ] {
        let err = c.apply(cmd(&c, op)).unwrap_err();
        match err {
            Error::Validation(message) => assert_eq!(message, want),
            other => panic!("expected Validation({want:?}), got {other:?}"),
        }
        assert_eq!(c.live_revision(), rev);
    }
    assert!(c.live().tracks.is_empty());
    assert!(!c.undo().unwrap());
}

#[test]
fn composite_stale_revision_is_rejected_before_validation() {
    let mut c = controller();
    let stale = Command {
        expected_revision: ProjectRevision(7),
        op: ProjectOp::Composite {
            ops: vec![ProjectOp::AddTrack { name: "t".into() }],
        },
    };
    assert!(matches!(c.apply(stale), Err(Error::StaleRevision { .. })));
    assert!(c.live().tracks.is_empty());
    assert!(!c.undo().unwrap());
}
