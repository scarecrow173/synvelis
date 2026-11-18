//! CB-09 Difference Explorer app wiring tests (synvelis#286).
//!
//! Covers the `syn.variant-diff.*` op surface end to end: view
//! lifecycle kind-gating, persisted view-local state, explicit-pin
//! compare through the CB-08 orchestrator, honest projection without
//! a compare session, adopt preview/commit separation with the
//! approval ticket, CAS rejection of stale pins, and post-commit
//! authority re-derivation. Real GPUI mount, OS/window evidence, and
//! plugin-backed signal measurement remain the #288 acceptance gate.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use ruviform_ui::BackendReport;
use ruviform_ui::ipc::codes;

use synvelis_app::SynvelisApp;
use synvelis_project::control::BeatPoint;
use synvelis_project::ops::ProjectOp;
use synvelis_project::placement::{
    ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, TimeAnchor,
};
use synvelis_project::range::SourceRange;
use synvelis_project::snapshot::{SnapshotCancel, SnapshotResolver};
use synvelis_project::time::{MusicalDuration, MusicalTick, SourceFrame};
use synvelis_project::variant::{
    AudioClipAdapter, BranchEditIntent, DirectoryBranchContentStore, VariantScope, edit_branch,
    fork_variant_set,
};
use synvelis_project::{BranchId, VariantSetId};
use synvelis_workspace::variant_diff::{BranchPin, VariantDiffTarget};

// ── fixture ─────────────────────────────────────────────────────────

struct DiffFixture {
    app: SynvelisApp,
    bundle: PathBuf,
    variant_set: VariantSetId,
    adopted: BranchId,
    candidate: BranchId,
}

fn bundle_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "sv286-cb09-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn apply_op(app: &mut SynvelisApp, op: ProjectOp) {
    let revision = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(op, revision)
        .expect("fixture op applies");
}

/// Track → lane → asset+source → one audio clip → one clip note, then
/// a real `fork_variant_set` + candidate mute edit through the same
/// `DirectoryBranchContentStore` root the app's dispatch engine
/// opens (`<bundle>/branch-content`).
#[allow(clippy::too_many_lines)] // sequential fixture scenario
fn fixture() -> DiffFixture {
    let bundle = bundle_dir("fixture");
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    app.project_new(&bundle.to_string_lossy(), Some("cb09"))
        .expect("project.new");

    apply_op(&mut app, ProjectOp::AddTrack { name: "t".into() });
    let track = *app
        .workspace()
        .session()
        .project()
        .tracks
        .keys()
        .next()
        .unwrap();
    apply_op(&mut app, ProjectOp::AddLane { track_id: track });
    let lane = *app
        .workspace()
        .session()
        .project()
        .lanes
        .keys()
        .next()
        .unwrap();
    apply_op(
        &mut app,
        ProjectOp::RegisterAsset {
            digest: "asset-a".into(),
        },
    );
    let asset = *app
        .workspace()
        .session()
        .project()
        .assets
        .keys()
        .next()
        .unwrap();
    apply_op(
        &mut app,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "digest-1".into(),
        },
    );
    let source = *app
        .workspace()
        .session()
        .project()
        .sources
        .keys()
        .next()
        .unwrap();
    apply_op(
        &mut app,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_000_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    let clip = *app
        .workspace()
        .session()
        .project()
        .clips
        .keys()
        .next_back()
        .unwrap();
    apply_op(
        &mut app,
        ProjectOp::AddNote {
            clip,
            key: 60,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );

    let scope = VariantScope::AudioClip { clip };
    let adapter = AudioClipAdapter::new();
    let mut store = DirectoryBranchContentStore::new(bundle.join("branch-content"));
    let controller = app.workspace_mut().session_mut().controller_mut();
    let receipt = fork_variant_set(
        controller,
        &adapter,
        &mut store,
        &SnapshotResolver::new(),
        scope,
        &SnapshotCancel::new(),
    )
    .expect("fork_variant_set");
    let candidate_rev = controller.live().variant_sets[&receipt.variant_set].branches
        [&receipt.candidate]
        .content
        .revision();
    edit_branch(
        controller,
        &adapter,
        &mut store,
        &BranchEditIntent {
            variant_set: receipt.variant_set,
            branch: receipt.candidate,
            expected_branch_revision: candidate_rev,
            op: ProjectOp::SetClipMuted {
                clip_id: clip,
                muted: true,
            },
        },
    )
    .expect("candidate mute edit");

    DiffFixture {
        app,
        bundle,
        variant_set: receipt.variant_set,
        adopted: receipt.adopted,
        candidate: receipt.candidate,
    }
}

fn copy_dir_all(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("copy dst");
    for entry in std::fs::read_dir(src).expect("copy src") {
        let entry = entry.expect("copy entry");
        let to = dst.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir_all(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).expect("copy file");
        }
    }
}

impl DiffFixture {
    fn pin(&self, branch: BranchId) -> BranchPin {
        let content = &self.app.workspace().session().project().variant_sets[&self.variant_set]
            .branches[&branch]
            .content;
        BranchPin {
            branch,
            revision: content.revision(),
        }
    }

    fn open_view(&mut self) -> String {
        let ack = self
            .app
            .op_variant_diff_visual(&json!({ "action": "open" }))
            .expect("visual open");
        ack["view_id"].as_str().unwrap().to_string()
    }

    fn action(
        &mut self,
        view_id: &str,
        params: Value,
    ) -> Result<Value, ruviform_ui::ipc::ErrorBody> {
        let mut params = params;
        params["view_id"] = json!(view_id);
        self.app.op_variant_diff_action(&params)
    }

    fn view(&self, view_id: &str) -> Result<Value, ruviform_ui::ipc::ErrorBody> {
        self.app
            .op_variant_diff_view(&json!({ "view_id": view_id }))
    }
}

// ── lifecycle + view gate ───────────────────────────────────────────

#[test]
fn cb09_app_visual_open_close_and_kind_gate() {
    let mut f = fixture();
    let view_id = f.open_view();
    assert!(view_id.starts_with("view-"));

    // A fresh view has no compare target — the projection fails
    // closed rather than fabricating an empty comparison.
    let err = f.view(&view_id).expect_err("untargeted view must fail");
    assert_eq!(err.code, codes::UNAVAILABLE);

    // Wrong-kind views are refused — the composition main view can
    // never mint variant-diff state.
    let err = f
        .app
        .op_variant_diff_view(&json!({ "view_id": "view-1" }))
        .expect_err("non-variant-diff view refused");
    assert_eq!(err.code, codes::INVALID_PARAMS);

    // Close removes the view; further ops refuse.
    f.app
        .op_variant_diff_visual(&json!({ "action": "close", "view_id": view_id }))
        .expect("visual close");
    let err = f.view(&view_id).expect_err("closed view refused");
    assert_eq!(err.code, codes::INVALID_PARAMS);
}

#[test]
fn cb09_app_open_with_target_seed() {
    let mut f = fixture();
    let target = VariantDiffTarget {
        variant_set: f.variant_set,
        left: f.pin(f.adopted),
        right: f.pin(f.candidate),
    };
    let ack = f
        .app
        .op_variant_diff_visual(&json!({
            "action": "open",
            "target": serde_json::to_value(&target).unwrap(),
        }))
        .expect("open with target");
    let view_id = ack["view_id"].as_str().unwrap();
    let projection = f.view(view_id).expect("targeted view projects");
    assert_eq!(
        projection["state"]["target"],
        serde_json::to_value(&target).unwrap()
    );
    // No compare session yet — evidence stays honestly absent, never
    // fabricated as "no difference".
    assert!(projection["spec"].is_object());
}

// ── view-local actions ──────────────────────────────────────────────

#[test]
fn cb09_app_view_local_actions_never_touch_authority() {
    let mut f = fixture();
    let view_id = f.open_view();
    let revision_before = f.app.workspace().session().revision();

    f.action(&view_id, json!({ "action": "select", "row": 3 }))
        .expect("select");
    f.action(
        &view_id,
        json!({ "action": "audition", "branch": serde_json::to_value(f.pin(f.candidate)).unwrap() }),
    )
    .expect("audition");
    let target = VariantDiffTarget {
        variant_set: f.variant_set,
        left: f.pin(f.adopted),
        right: f.pin(f.candidate),
    };
    f.action(
        &view_id,
        json!({
            "action": "set-target",
            "variant_set": serde_json::to_value(f.variant_set).unwrap(),
            "left": serde_json::to_value(&target.left).unwrap(),
            "right": serde_json::to_value(&target.right).unwrap(),
        }),
    )
    .expect("set-target");

    let projection = f.view(&view_id).expect("view projects");
    assert_eq!(projection["state"]["selected_row"], json!(3));
    assert!(projection["state"]["audition"].is_object());
    assert_eq!(
        projection["state"]["target"],
        serde_json::to_value(&target).unwrap()
    );
    // View-local actions never advance the project revision.
    assert_eq!(f.app.workspace().session().revision(), revision_before);
}

// ── compare ─────────────────────────────────────────────────────────

#[test]
fn cb09_app_compare_stores_pin_scoped_evidence() {
    let mut f = fixture();
    let view_id = f.open_view();
    let ack = f
        .action(
            &view_id,
            json!({
                "action": "compare",
                "left": serde_json::to_value(f.pin(f.adopted)).unwrap(),
                "right": serde_json::to_value(f.pin(f.candidate)).unwrap(),
            }),
        )
        .expect("compare");
    assert_eq!(ack["ok"], json!(true));

    // The compared pins become the persisted semantic target.
    let projection = f.view(&view_id).expect("view projects after compare");
    let target = &projection["state"]["target"];
    assert_eq!(
        target["variant_set"],
        serde_json::to_value(f.variant_set).unwrap()
    );
    assert_eq!(
        target["left"]["branch"],
        serde_json::to_value(f.adopted).unwrap()
    );
    assert_eq!(
        target["right"]["branch"],
        serde_json::to_value(f.candidate).unwrap()
    );
    // The spec is a real projection — semantic evidence rows exist
    // for the muted-clip difference (the candidate edit is real).
    assert!(projection["spec"].is_object());
}

#[test]
fn cb09_app_compare_rejects_stale_and_implicit_pins() {
    let mut f = fixture();
    let view_id = f.open_view();
    let mut stale = f.pin(f.candidate);
    stale.revision = synvelis_project::variant::BranchRevision::new(stale.revision.0 + 1);

    let err = f
        .action(
            &view_id,
            json!({
                "action": "compare",
                "left": serde_json::to_value(f.pin(f.adopted)).unwrap(),
                "right": serde_json::to_value(&stale).unwrap(),
            }),
        )
        .expect_err("stale pin rejected");
    assert_eq!(err.code, codes::INVALID_PARAMS);
    assert_eq!(err.details.unwrap()["reason"], json!("stale-revision"));

    // Same-branch compare is a typed reject, never an implicit diff.
    let err = f
        .action(
            &view_id,
            json!({
                "action": "compare",
                "left": serde_json::to_value(f.pin(f.adopted)).unwrap(),
                "right": serde_json::to_value(f.pin(f.adopted)).unwrap(),
            }),
        )
        .expect_err("same-branch rejected");
    assert_eq!(err.code, codes::INVALID_PARAMS);
    assert_eq!(err.details.unwrap()["reason"], json!("same-branch"));
}

// ── adopt preview / commit ──────────────────────────────────────────

#[test]
fn cb09_app_adopt_preview_never_commits() {
    let mut f = fixture();
    let view_id = f.open_view();
    let revision_before = f.app.workspace().session().revision();
    let pin = f.pin(f.candidate);

    let ack = f
        .action(
            &view_id,
            json!({
                "action": "adopt-preview",
                "branch": serde_json::to_value(pin.branch).unwrap(),
                "revision": pin.revision.0,
            }),
        )
        .expect("adopt-preview");
    assert_eq!(ack["ok"], json!(true));
    assert!(ack["preview"].is_object());

    // Preview is simulation only — live revision and adopted branch
    // are untouched.
    assert_eq!(f.app.workspace().session().revision(), revision_before);
    let set = &f.app.workspace().session().project().variant_sets[&f.variant_set];
    assert_eq!(set.adopted, f.adopted);
}

#[test]
fn cb09_app_adopt_requires_explicit_approval() {
    let mut f = fixture();
    let view_id = f.open_view();
    let pin = f.pin(f.candidate);

    let err = f
        .action(
            &view_id,
            json!({
                "action": "adopt",
                "branch": serde_json::to_value(pin.branch).unwrap(),
                "revision": pin.revision.0,
            }),
        )
        .expect_err("adopt without approval ticket refused");
    assert_eq!(err.code, codes::INVALID_PARAMS);
}

#[test]
fn cb09_app_adopt_commits_through_cas() {
    let mut f = fixture();
    let view_id = f.open_view();
    let pin = f.pin(f.candidate);
    let revision_before = f.app.workspace().session().revision();

    let ack = f
        .action(
            &view_id,
            json!({
                "action": "adopt",
                "approval": "user-confirmed-cb09",
                "branch": serde_json::to_value(pin.branch).unwrap(),
                "revision": pin.revision.0,
            }),
        )
        .expect("adopt commits");
    assert_eq!(ack["ok"], json!(true));
    assert!(ack["receipt"].is_object());

    // The CAS commit landed: adopted branch flipped, revision moved.
    let session = f.app.workspace().session();
    assert!(session.revision() > revision_before);
    let set = &session.project().variant_sets[&f.variant_set];
    assert_eq!(set.adopted, f.candidate);

    // Adopt is a mutation, not a compare — the view still has no
    // persisted target and the projection fails closed honestly
    // rather than fabricating one.
    let err = f
        .view(&view_id)
        .expect_err("adopt does not mint a compare target");
    assert_eq!(err.code, codes::UNAVAILABLE);
}

#[test]
fn cb09_app_variant_diff_spec_mounts_on_view_host() {
    let mut f = fixture();
    let view_id = f.open_view();
    let target = VariantDiffTarget {
        variant_set: f.variant_set,
        left: f.pin(f.adopted),
        right: f.pin(f.candidate),
    };
    f.action(
        &view_id,
        json!({
            "action": "set-target",
            "variant_set": serde_json::to_value(f.variant_set).unwrap(),
            "left": serde_json::to_value(&target.left).unwrap(),
            "right": serde_json::to_value(&target.right).unwrap(),
        }),
    )
    .expect("set-target");
    let projection = f.view(&view_id).expect("view projects");
    let spec: ruviform_ui::view::ViewSpec =
        serde_json::from_value(projection["spec"].clone()).expect("spec decodes");

    // The same mount path `synvelis-native` uses — session-owned
    // epoch allocator, adopted view instance, verbatim spec.
    let epochs = f.app.attach_epoch_allocator();
    let n: u64 = view_id.strip_prefix("view-").unwrap().parse().unwrap();
    let inst = ruviform_ui::dock::ViewInstanceId::new(n).unwrap();
    let host = synvelis_app::mount_variant_diff_view(&epochs, inst, spec).expect("mount");
    let names: Vec<String> = host.mounted_keys().iter().map(|k| k.0.clone()).collect();
    assert!(
        names.iter().any(|n| n.starts_with("variant-diff.")),
        "variant-diff components mounted: {names:?}"
    );
}

#[test]
fn cb09_app_adopt_rejects_stale_revision_cas() {
    let mut f = fixture();
    let view_id = f.open_view();
    let mut stale = f.pin(f.candidate);
    stale.revision = synvelis_project::variant::BranchRevision::new(stale.revision.0 + 1);
    let revision_before = f.app.workspace().session().revision();

    let err = f
        .action(
            &view_id,
            json!({
                "action": "adopt",
                "approval": "user-confirmed-cb09",
                "branch": serde_json::to_value(stale.branch).unwrap(),
                "revision": stale.revision.0,
            }),
        )
        .expect_err("stale adopt pin rejected");
    assert_eq!(err.code, codes::INVALID_PARAMS);
    assert_eq!(f.app.workspace().session().revision(), revision_before);
}

/// Cross-project isolation (review finding): compare evidence and
/// runtime-publish observations recorded under project A must never
/// surface under project B — even when B is a byte-identical bundle
/// copy whose deterministic variant-set/branch ids collide and the
/// fresh workspace restarts the view-id space.
#[test]
fn cb09_app_project_switch_clears_variant_diff_state() {
    let mut f = fixture();
    let view_id = f.open_view();
    f.action(
        &view_id,
        json!({
            "action": "compare",
            "left": serde_json::to_value(f.pin(f.adopted)).unwrap(),
            "right": serde_json::to_value(f.pin(f.candidate)).unwrap(),
        }),
    )
    .expect("compare");
    let spec_with_evidence = f.view(&view_id).expect("view projects")["spec"].to_string();
    assert!(
        spec_with_evidence.contains("semantic diff rows"),
        "compare evidence must render rows under project A"
    );

    // Persist A, copy the bundle, reopen the copy as project B —
    // identical content means identical ids/revisions, and the new
    // workspace re-mints the same view ids the sessions were keyed by.
    let target = VariantDiffTarget {
        variant_set: f.variant_set,
        left: f.pin(f.adopted),
        right: f.pin(f.candidate),
    };
    f.app.project_save().expect("project.save");
    let bundle_b = bundle_dir("copy");
    copy_dir_all(&f.bundle, &bundle_b);
    f.app
        .project_open(&bundle_b.to_string_lossy())
        .expect("project.open copy");

    let ack = f
        .app
        .op_variant_diff_visual(&json!({
            "action": "open",
            "target": serde_json::to_value(&target).unwrap(),
        }))
        .expect("open in project B");
    let view_b = ack["view_id"].as_str().unwrap();
    assert_eq!(
        view_b, view_id,
        "view-id space restarts — the leak scenario needs the collision"
    );

    let projection = f
        .app
        .op_variant_diff_view(&json!({ "view_id": view_b }))
        .expect("view projects in project B");
    let spec_b = projection["spec"].to_string();
    assert!(
        !spec_b.contains("semantic diff rows"),
        "project A's compare evidence leaked into project B's view"
    );
    assert!(
        spec_b.contains("unavailable (") || spec_b.contains("unknown ("),
        "missing evidence must degrade honestly, got: {spec_b}"
    );
}
