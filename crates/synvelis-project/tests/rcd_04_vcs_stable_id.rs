//! RCD-04 residual (synvelis#118): `CompTransition` — and the rest of
//! the comp registry — keeps its independent stable ID across every
//! headless VCS surface: save → saved-snapshot resolution → semantic
//! diff, diff → selected apply (fail-closed), and snapshot →
//! `restore_as_copy` publication.
//!
//! What is pinned here:
//!
//! - A comp transition's add/remove is expressed in the `ChangeSet`
//!   *solely* by the `CompTransition` object's own stable ID — the
//!   owning `Comp`'s `transition_ids` membership list is diff-skipped
//!   (child entities carry membership), so no comp-level churn hides
//!   or duplicates the transition's identity.
//! - The tagged stable ID (`comp-transition-<32hex>`) is identical in
//!   both diff directions and survives save/reload, undo/redo, and a
//!   published restore copy byte-for-byte.
//! - The VCS apply surface replays comp kinds (synvelis#513):
//!   `target_fingerprint`/`apply_selected` accept `CompTransition`
//!   `Added` as a typed entity transplant, while `Removed` stays
//!   fail-closed `Unsupported` (cascade deferred).
//!
//! Headless fixtures only: no real provider, OS filesystem
//! durability certification, or GUI claim.

use std::collections::BTreeMap;
use std::path::PathBuf;

use synvelis_project::diff::{
    ChangeKind, ChangeSet, ComparisonCompleteness, ComparisonEquality, ComparisonSpec, ObjectKind,
    SemanticChange, compare_snapshots,
};
use synvelis_project::*;

// ---------------------------------------------------------------------
// Comp fixture — same POD-seeded shape as `rcd_04_comp_cas.rs`, with a
// real sha256 asset digest so the restore leg can verify blob content.
// ---------------------------------------------------------------------

const RATE: u32 = 48_000;

fn controller(project: Project) -> ProjectController {
    ProjectController::new(project, Box::new(CounterIdGenerator::new(1_000)))
}

fn apply_op<O: Operation>(c: &mut ProjectController, op: O) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap()
}

fn mr(start: i64, end: i64) -> ProjectRange {
    ProjectRange::Musical(
        MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap(),
    )
}

/// A project seeded like `rcd_04_comp_cas.rs`: one track, two lanes,
/// one asset+source (real sha256 digest), three takes — `take_a0`/
/// `take_a1` on `lane_a`, `take_b` on `lane_b`, all reading the same
/// `Source`.
struct Seeded {
    lane_a: LaneId,
    lane_b: LaneId,
    take_a0: TakeId,
    take_a1: TakeId,
    take_b: TakeId,
    source_id: SourceId,
    asset_digest: String,
    asset_bytes: Vec<u8>,
}

fn seeded() -> (Project, Seeded) {
    let asset_bytes = b"rcd-04-vcs-stable-id-raw".to_vec();
    let asset_digest = ContentDigest::sha256(&asset_bytes).to_string();
    let mut p = Project::new(ProjectId::new(0), "rcd-04-vcs-stable-id");
    let s = Seeded {
        lane_a: LaneId::new(2),
        lane_b: LaneId::new(3),
        take_a0: TakeId::new(15),
        take_a1: TakeId::new(16),
        take_b: TakeId::new(17),
        source_id: SourceId::new(5),
        asset_digest: asset_digest.clone(),
        asset_bytes,
    };
    seed_structure(&mut p, &s);
    seed_recording(&mut p, &s);
    (p, s)
}

fn seed_structure(p: &mut Project, s: &Seeded) {
    let track_id = TrackId::new(1);
    let asset_id = AssetId::new(4);
    p.tracks.insert(
        track_id,
        Track {
            id: track_id,
            name: "t".into(),
            lane_ids: vec![s.lane_a, s.lane_b],
        },
    );
    for (id, track) in [(s.lane_a, track_id), (s.lane_b, track_id)] {
        p.lanes.insert(
            id,
            Lane {
                id,
                track_id: track,
                clip_ids: Vec::new(),
            },
        );
    }
    p.assets.insert(
        asset_id,
        Asset {
            id: asset_id,
            digest: s.asset_digest.clone(),
        },
    );
    p.sources.insert(
        s.source_id,
        Source {
            id: s.source_id,
            asset_id,
            content_digest: s.asset_digest.clone(),
            state: SourceState::Committed,
        },
    );
    p.clock_epochs.insert(
        ClockEpochId::new(18),
        ClockEpoch {
            id: ClockEpochId::new(18),
            description: "mock".into(),
        },
    );
}

fn seed_recording(p: &mut Project, s: &Seeded) {
    let session_id = RecordingSessionId::new(6);
    let pass0 = RecordingPassId::new(7);
    let pass1 = RecordingPassId::new(8);
    let pass2 = RecordingPassId::new(9);
    let stream_a = CaptureStreamId::new(10);
    let stream_b = CaptureStreamId::new(11);
    let seg0 = CaptureSegmentId::new(12);
    let seg1 = CaptureSegmentId::new(13);
    let seg2 = CaptureSegmentId::new(14);
    let epoch = ClockEpochId::new(18);

    p.recording.sessions.insert(
        session_id,
        RecordingSession {
            id: session_id,
            name: "s".into(),
            state: SessionState::Finalized,
        },
    );
    for (i, id) in [pass0, pass1, pass2].into_iter().enumerate() {
        p.recording.passes.insert(
            id,
            RecordingPass {
                id,
                session_id,
                index: u32::try_from(i).unwrap(),
            },
        );
    }
    for (id, lane) in [(stream_a, s.lane_a), (stream_b, s.lane_b)] {
        p.recording.streams.insert(
            id,
            CaptureStream {
                id,
                session_id,
                lane_id: lane,
                clock_epoch_id: epoch,
            },
        );
    }
    for (id, stream, pass, s_lo, s_hi) in [
        (seg0, stream_a, pass0, 0_i64, 30_000_i64),
        (seg1, stream_a, pass1, 40_000, 70_000),
        (seg2, stream_b, pass2, 0, 1_000),
    ] {
        p.recording.segments.insert(
            id,
            CaptureSegment {
                id,
                stream_id: stream,
                pass_id: pass,
                session_range: SessionRange::try_new(
                    SessionFrame::new(s_lo).unwrap(),
                    SessionFrame::new(s_hi).unwrap(),
                )
                .unwrap(),
                source_range: SourceRange::try_new(
                    SourceFrame::new(s_lo).unwrap(),
                    SourceFrame::new(s_hi).unwrap(),
                )
                .unwrap(),
                source_id: s.source_id,
            },
        );
    }
    for (id, pass, stream, lane, segs) in [
        (s.take_a0, pass0, stream_a, s.lane_a, vec![seg0]),
        (s.take_a1, pass1, stream_a, s.lane_a, vec![seg1]),
        (s.take_b, pass2, stream_b, s.lane_b, vec![seg2]),
    ] {
        p.recording.takes.insert(
            id,
            Take {
                id,
                session_id,
                pass_id: pass,
                stream_id: stream,
                lane_id: lane,
                source_id: s.source_id,
                segment_ids: segs,
                state: TakeState::Verified,
            },
        );
    }
}

/// Shared comp build: group → comp → 3 regions → active. When
/// `with_transition` is set, the closing `AddCompTransition` runs —
/// the two variants mint identical IDs for every shared entity
/// (same op sequence, same `CounterIdGenerator` seed), so snapshots
/// of them diff cleanly except for the transition itself.
fn build_comp(
    c: &mut ProjectController,
    s: &Seeded,
    with_transition: bool,
) -> (
    TakeGroupId,
    CompId,
    Vec<CompRegionId>,
    Option<CompTransitionId>,
) {
    apply_op(
        c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_a0, s.take_a1],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply_op(
        c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    for (range, candidate) in [
        (
            mr(0, 960),
            CompCandidateRef::Take {
                take_id: s.take_a0,
                source_offset: SourceFrame::ZERO,
            },
        ),
        (
            mr(960, 1_920),
            CompCandidateRef::ExistingSource {
                source_id: s.source_id,
                handle: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(96_000).unwrap())
                    .unwrap(),
                play_from: SourceFrame::new(2_000).unwrap(),
            },
        ),
        (mr(1_920, 2_000), CompCandidateRef::Silence),
    ] {
        apply_op(
            c,
            CompOp::AddCompRegion {
                comp_id: comp,
                project_range: range,
                candidate,
                mapping: PlaybackMapping::Natural,
            },
        );
    }
    apply_op(
        c,
        CompOp::SetActiveComp {
            take_group_id: group,
            comp_id: Some(comp),
        },
    );
    let region_ids = c.live().comp.comps[&comp].region_ids.clone();
    let transition = with_transition.then(|| {
        apply_op(
            c,
            CompOp::AddCompTransition {
                left_region_id: region_ids[0],
                right_region_id: region_ids[1],
                overlap_range: mr(900, 1_020),
                fade_out: FadeShape::EqualPower,
                fade_in: FadeShape::Linear,
            },
        );
        *c.live().comp.transitions.keys().next().unwrap()
    });
    (group, comp, region_ids, transition)
}

// ---------------------------------------------------------------------
// Snapshot / diff / apply helpers (vcs02/vcs05 conventions).
// ---------------------------------------------------------------------

/// Reports every probed digest as present — keeps live captures free
/// of `UnresolvedSource` noise so `Equal` is reachable.
struct AllPresentProbe;

impl BlobProbe for AllPresentProbe {
    fn probe(&self, _digest: &str) -> BlobRead {
        BlobRead::Present(vec![0])
    }
}

fn snap_live(c: &ProjectController) -> ImmutableSnapshot {
    SnapshotResolver::new()
        .capture_live(
            c,
            c.live_revision(),
            &SnapshotCancel::new(),
            Some(&AllPresentProbe),
        )
        .unwrap()
}

fn spec_for(before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> ComparisonSpec {
    ComparisonSpec::new(before.identity.clone(), after.identity.clone())
}

fn compare(before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> ChangeSet {
    compare_snapshots(
        before,
        after,
        &spec_for(before, after),
        &SnapshotCancel::new(),
    )
    .unwrap()
}

fn comp_key_sets(p: &Project) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    (
        p.comp.take_groups.keys().map(|id| id.to_tagged()).collect(),
        p.comp.comps.keys().map(|id| id.to_tagged()).collect(),
        p.comp.regions.keys().map(|id| id.to_tagged()).collect(),
        p.comp.transitions.keys().map(|id| id.to_tagged()).collect(),
    )
}

// ---------------------------------------------------------------------
// apply helpers (vcs05_apply.rs conventions)
// ---------------------------------------------------------------------

struct Src {
    resolved: ResolvedChangeSet,
}

impl ChangeSetSource for Src {
    fn resolve(&self, id: &ContentDigest) -> Option<ResolvedChangeSet> {
        (id == &self.resolved.changeset.key.digest()).then(|| self.resolved.clone())
    }
}

fn sources_for(cs: &ChangeSet, before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> Src {
    Src {
        resolved: ResolvedChangeSet {
            changeset: cs.clone(),
            before: before.clone(),
            after: after.clone(),
        },
    }
}

// ---------------------------------------------------------------------
// restore helpers (vcs05_restore.rs conventions)
// ---------------------------------------------------------------------

/// Blob source serving `digest → bytes` (real sha256-verified by the
/// restore materializer).
struct MapBlobs {
    map: BTreeMap<String, Vec<u8>>,
}

impl RestoreBlobSource for MapBlobs {
    fn read_blob(&self, digest: &str, _cancel: &SnapshotCancel) -> Result<BlobRead, String> {
        Ok(match self.map.get(digest) {
            Some(b) => BlobRead::Present(b.clone()),
            None => BlobRead::Absent,
        })
    }
    fn stat_blob(&self, digest: &str) -> Result<Option<u64>, String> {
        Ok(self.map.get(digest).map(|b| b.len() as u64))
    }
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-rcd04-vcs-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

/// RCD-04 stable-ID: save → saved-HEAD snapshot resolution → semantic
/// diff sees the comp registry byte-identical — every stable ID
/// (take group, comp, regions, transition) survives the VCS save
/// boundary with zero diff noise.
#[test]
fn comp_stable_ids_survive_save_reload_and_diff() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (_group, _comp, _regions, transition) = build_comp(&mut c, &s, true);
    let transition = transition.expect("transition built");

    let live_keys = comp_key_sets(c.live());
    assert_eq!(live_keys.3, vec![transition.to_tagged()]);

    // Save → resolve the saved HEAD as an ImmutableSnapshot — the
    // same surface a VCS provider reads.
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let saved = SnapshotResolver::new()
        .resolve_saved_head(
            &store,
            c.live().project_id,
            &SnapshotCancel::new(),
            Some(&AllPresentProbe),
        )
        .unwrap();

    // Every comp stable ID is identical on the resolved snapshot.
    assert_eq!(comp_key_sets(saved.projection.project()), live_keys);

    // Live vs saved-snapshot diff: identical content → Equal, no
    // changes — the stable IDs carry the identity across the boundary.
    let live_snap = snap_live(&c);
    let cs = compare(&live_snap, &saved);
    assert_eq!(cs.overall, ComparisonEquality::Equal);
    assert!(cs.changes.is_empty(), "comp diff must be clean: {cs:?}");
}

/// RCD-04 stable-ID: a transition add/remove is expressed in the
/// `ChangeSet` solely by the `CompTransition` object's own stable ID.
/// The owning comp's `transition_ids` membership list is diff-skipped
/// (child entities carry membership), so no parent-level change
/// duplicates or obscures the transition's independent identity — in
/// either diff direction.
#[test]
fn comp_transition_diff_is_expressed_by_its_own_stable_id() {
    // State A: full comp WITH transition.
    let (pa, sa) = seeded();
    let mut ca = controller(pa);
    let (_g, _c, _r, transition) = build_comp(&mut ca, &sa, true);
    let transition = transition.expect("transition built");
    let snap_a = snap_live(&ca);

    // State B: identical build WITHOUT the transition op — every
    // shared entity minted the same ID (same op prefix, same id
    // generator seed).
    let (pb, sb) = seeded();
    let mut cb = controller(pb);
    build_comp(&mut cb, &sb, false);
    let snap_b = snap_live(&cb);

    // Sanity: the two states differ ONLY by the transition entity.
    let comp_b = &cb.live().comp;
    assert!(comp_b.transitions.is_empty());
    assert!(ca.live().comp.transitions.contains_key(&transition));

    // A → B: the transition's removal is one `CompTransition`
    // `Removed` change whose object identity IS its stable ID.
    let ab = compare(&snap_a, &snap_b);
    assert_eq!(ab.changes.len(), 1, "only the transition may diff: {ab:?}");
    assert!(ab.unresolved.is_empty(), "no unresolved issues: {ab:?}");
    assert_eq!(ab.completeness, ComparisonCompleteness::Complete);
    let removed = &ab.changes[0];
    assert_eq!(removed.object.kind, ObjectKind::CompTransition);
    assert_eq!(removed.kind, ChangeKind::Removed);
    assert_eq!(removed.object.stable_id, transition.to_tagged());
    assert!(removed.object.stable_id.starts_with("comp-transition-"));

    // B → A: symmetric — the same stable ID re-appears as `Added`.
    let ba = compare(&snap_b, &snap_a);
    assert_eq!(ba.changes.len(), 1, "only the transition may diff: {ba:?}");
    assert!(ba.unresolved.is_empty(), "no unresolved issues: {ba:?}");
    assert_eq!(ba.completeness, ComparisonCompleteness::Complete);
    let added = &ba.changes[0];
    assert_eq!(added.object.kind, ObjectKind::CompTransition);
    assert_eq!(added.kind, ChangeKind::Added);
    assert_eq!(added.object.stable_id, transition.to_tagged());

    // The transition ID is minted independently of its comp/region
    // parents: distinct kind prefix AND a distinct raw id portion —
    // a derivation bug that reused a parent's id bits would fail here.
    let tid_hex = added
        .object
        .stable_id
        .strip_prefix("comp-transition-")
        .expect("transition tagged id");
    for comp_id in ca.live().comp.comps.keys() {
        let cid_tagged = comp_id.to_tagged();
        let cid_hex = cid_tagged.strip_prefix("comp-").expect("comp tagged id");
        assert_ne!(tid_hex, cid_hex, "transition and comp share raw id");
    }
    for region_id in ca.live().comp.regions.keys() {
        let rid_tagged = region_id.to_tagged();
        let rid_hex = rid_tagged
            .strip_prefix("comp-region-")
            .expect("region tagged id");
        assert_ne!(tid_hex, rid_hex, "transition and region share raw id");
    }
}

/// Single-change `apply_selected` with a real target fingerprint —
/// the shared intent/journal boilerplate for the replay pins.
fn apply_one(
    c: &mut ProjectController,
    cs: &ChangeSet,
    before: &ImmutableSnapshot,
    after: &ImmutableSnapshot,
    change: &SemanticChange,
) -> Result<ApplyReceipt, ApplyError> {
    let fp = target_fingerprint(c.live(), std::slice::from_ref(&change.object))?;
    let intent = ApplyChangeIntent {
        change_set_id: cs.key.digest(),
        source_snapshot_id: after.identity.digest,
        target_object: change.object.clone(),
        expected_project_revision: c.live_revision(),
        selected_change_ids: vec![change.id.clone()],
        expected_target_fingerprint: fp,
        approval: ApplyApproval {
            ticket: "user-approved".into(),
        },
    };
    let src = sources_for(cs, before, after);
    let mut journal = InMemoryApplyJournal::new();
    apply_selected(
        c,
        &intent,
        &ApplySources {
            changesets: &src,
            assets: Some(&AllPresentProbe),
            plugins: None,
        },
        &mut journal,
        &SnapshotCancel::new(),
    )
}

/// RCD-04 stable-ID boundary (sv#513): comp kinds are diff-visible
/// AND on the apply whitelist — a `CompTransition` `Added` change
/// replays through `apply_selected` as a typed entity transplant, while
/// the reverse `Removed` change stays fail-closed `Unsupported`.
#[test]
fn apply_selected_replays_comp_transition_removed_stays_fail_closed() {
    // Live state = B (no transition); changeset wants B → A (adds it).
    let (pb, sb) = seeded();
    let mut live = controller(pb);
    build_comp(&mut live, &sb, false);
    let before = snap_live(&live);

    let (pa, sa) = seeded();
    let mut after_c = controller(pa);
    build_comp(&mut after_c, &sa, true);
    let after = snap_live(&after_c);
    let cs = compare(&before, &after);
    let change = cs
        .changes
        .iter()
        .find(|c| c.object.kind == ObjectKind::CompTransition)
        .expect("transition change present");
    assert_eq!(change.kind, ChangeKind::Added);

    // The fingerprint surface resolves the kind — comp entities are
    // apply-whitelisted now. End-to-end: the single-transition
    // selection applies as one CAS.
    target_fingerprint(live.live(), std::slice::from_ref(&change.object))
        .expect("comp kind is on the apply surface");
    let rev_before = live.live_revision();
    let receipt = apply_one(&mut live, &cs, &before, &after, change)
        .expect("comp transition apply must succeed");
    assert_eq!(receipt.outcome, apply::ApplyOutcome::Applied);
    assert_eq!(live.live_revision().0, rev_before.0 + 1);
    let transition = CompTransitionId::from_tagged(&change.object.stable_id).unwrap();
    assert!(live.live().comp.transitions.contains_key(&transition));

    // Reverse direction: the `Removed` transition change stays
    // fail-closed — removal cascade is still deferred for every kind.
    let cs_rm = compare(&after, &before);
    let removed = cs_rm
        .changes
        .iter()
        .find(|c| c.object.kind == ObjectKind::CompTransition && c.kind == ChangeKind::Removed)
        .expect("reverse transition removal present");
    // Live now HAS the transition (just applied) — reset to state A
    // semantics by applying on a fresh A-state controller instead.
    let (pa2, sa2) = seeded();
    let mut live_a = controller(pa2);
    build_comp(&mut live_a, &sa2, true);
    // Live state A already holds the transition — the Removed change
    // must still refuse before any mutation.
    let rev_a = live_a.live_revision();
    // `apply_one` fingerprints the live (present) entity, then the
    // plan layer rejects `Removed` for every kind.
    let err = apply_one(&mut live_a, &cs_rm, &after, &before, removed)
        .expect_err("comp transition removal must stay fail-closed");
    assert!(
        matches!(
            err,
            ApplyError::Unsupported {
                kind: ObjectKind::CompTransition,
                ..
            }
        ),
        "expected Unsupported, got {err:?}"
    );
    assert_eq!(live_a.live_revision(), rev_a);
    assert!(live_a.live().comp.transitions.contains_key(&transition));
}
/// RCD-04 stable-ID: `restore_as_copy` publishes a new project whose
/// comp registry carries the SAME stable IDs — take group, comp,
/// regions, and the independent transition ID all survive the
/// snapshot → restore → published-envelope round-trip, and the
/// receipt accounts for them.
#[test]
fn comp_stable_ids_survive_restore_as_copy() {
    let (p, s) = seeded();
    let c = {
        let mut c = controller(p);
        build_comp(&mut c, &s, true);
        c
    };
    let snap = snap_live(&c);
    let live_keys = comp_key_sets(c.live());

    let parent = tmp_dir("restore");
    let dest = parent.join("copy");
    let blobs = MapBlobs {
        map: [(s.asset_digest.clone(), s.asset_bytes.clone())]
            .into_iter()
            .collect(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
        branch_content: None,
    };
    let receipt = restore_as_copy(
        &RestoreRequest {
            restore_id: "rcd-04-restore".into(),
            snapshot: snap,
            output_dir: dest.clone(),
            new_project_id: ProjectId::new(777),
            missing_assets: MissingAssetsPolicy::Fail,
        },
        &ports,
        &SnapshotCancel::new(),
    )
    .expect("restore publishes");

    assert_eq!(receipt.outcome, RestoreOutcome::Published);
    assert!(receipt.effects.source.project_unchanged);
    // The receipt accounts for every comp stable-ID family.
    for key in ["take-group", "comp", "comp-region", "comp-transition"] {
        assert!(
            receipt.stable_ids.counts.get(key).is_some_and(|n| *n >= 1),
            "receipt must count {key} stable ids"
        );
    }

    // The published snapshot envelope carries identical comp IDs.
    let head = std::fs::read_to_string(dest.join("HEAD")).unwrap();
    let snap_bytes = std::fs::read(dest.join("snapshots").join(head.trim())).unwrap();
    let envelope: SnapshotEnvelope = serde_json::from_slice(&snap_bytes).unwrap();
    assert_eq!(comp_key_sets(&envelope.project), live_keys);

    let _ = std::fs::remove_dir_all(&parent);
}
