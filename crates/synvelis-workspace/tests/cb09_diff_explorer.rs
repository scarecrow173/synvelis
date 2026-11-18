//! CB-09 Difference Explorer headless scenario tests (synvelis#286).
//!
//! Covers the driver-neutral view-model contract: per-domain diff
//! projection with honest `Unsupported`/`Unavailable`/`Unknown` states,
//! independent Adopted/Auditioning/Editing/RT labels, explicit-target
//! action binding with typed rejects, stale/pinned-context handling,
//! Mock-vs-Pipeline provenance, deterministic `ViewSpec` output, and
//! view-local state serde tolerance. Real GPUI mount, OS/window/IME,
//! real plugin dock, PCM/RT signal measurement, and the CB-08
//! orchestrator are `NotRun` — recorded in `evidence/sv286/`.
//!
//! Semantic evidence in these tests is produced by the merged
//! `compare_snapshots` engine (real implementation consumption, not a
//! mock); signal/perceptual layers use the declared mock seam and are
//! labelled `Mock` — never real measurements.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_ui::view::{ButtonSpec, ComponentSpec};
use synvelis_project::control::BeatPoint;
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    ChangeEvidence, ChangeId, ChangeKind, ChangeSet, ChangeSetSummary, ComparisonCompleteness,
    ComparisonEquality, ComparisonIssue, ComparisonSpec, ObjectKind, SemanticChange,
    SemanticFieldDiff, SemanticObjectRef, compare_snapshots,
};
use synvelis_project::id::CounterIdGenerator;
use synvelis_project::ops::ProjectOp;
use synvelis_project::placement::{
    ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, TimeAnchor,
};
use synvelis_project::project::Project;
use synvelis_project::range::MusicalRange;
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, ContentDigest, FixedSnapshotLocator, ImmutableSnapshot,
    ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness, SnapshotIdentity,
    SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::SCHEMA_VERSION;
use synvelis_project::time::{MusicalDuration, MusicalTick, SourceFrame};
use synvelis_project::variant::{
    ArrangementAdapter, ArrangementDomainMask, AudioClipAdapter, BranchEditIntent, BranchRevision,
    CompareError, DirectoryBranchContentStore, DomainAdapter, EvidenceOrigin as Cb08EvidenceOrigin,
    FakeDomainAdapter, InMemoryBranchContentStore, LayerState as Cb08LayerState,
    ProcessingRangeRef, RuntimePublishState, SessionOptions,
    SignalAlignment as Cb08SignalAlignment, SignalCompareRequest, SignalComparisonProvider,
    SignalDomain as Cb08SignalDomain, SignalMetric, SignalProviderReport, StripDomainMask,
    StripField, TakeUsageTarget, VariantScope, decode_candidate, edit_branch, fork_variant_set,
    scope_digest,
};
use synvelis_project::{
    BranchId, ChainId, ClipId, LaneId, NoteId, ProcessorId, ProjectId, StripId, TakeGroupId,
    TrackId, VariantSetId,
};
use synvelis_workspace::state::ViewState;
use synvelis_workspace::variant_diff::{
    VariantDiffBranchObservations, VariantDiffDispatchError, VariantDiffEngine,
    variant_diff_authority, variant_diff_context,
};
use synvelis_workspace::{
    ACTION_VARIANT_DIFF_ADOPT, ACTION_VARIANT_DIFF_ADOPT_PREVIEW, ACTION_VARIANT_DIFF_AUDITION,
    ACTION_VARIANT_DIFF_COMPARE, ACTION_VARIANT_DIFF_SELECT, AlignmentState, AuxLayerEvidence,
    BranchAuthority, BranchPin, CapabilityEvidence, CompareAuthority, CompareContext, CompareSide,
    ContextState, DiffRow, EvidenceOrigin, EvidenceRow, EvidenceState, SemanticDiffEvidence,
    SemanticEvidence, VARIANT_DIFF_TARGET, VariantDiffActionKind, VariantDiffDomain,
    VariantDiffInput, VariantDiffIntent, VariantDiffReason, VariantDiffReject, VariantDiffTarget,
    VariantDiffViewAction, VariantDiffViewState, ViewType, apply_variant_diff_action,
    decode_variant_diff_action, project_variant_diff, variant_diff_spec,
};

// ── synthetic fixtures ──────────────────────────────────────────────

const SET: u128 = 100;
const REV: u64 = 7;

fn pin(branch: u128, revision: u64) -> BranchPin {
    BranchPin {
        branch: BranchId::new(branch),
        revision: BranchRevision::new(revision),
    }
}

fn scope_for(domain_case: usize) -> VariantScope {
    match domain_case {
        0 => VariantScope::AudioClip {
            clip: ClipId::new(1),
        },
        1 => VariantScope::TakeUsage {
            target: TakeUsageTarget {
                take_group: TakeGroupId::new(2),
            },
        },
        2 => VariantScope::MidiClip {
            clip: ClipId::new(3),
        },
        3 => VariantScope::InstrumentSlot {
            processor: ProcessorId::new(4),
        },
        4 => VariantScope::ProcessingRange {
            range: ProcessingRangeRef {
                chain: ChainId::new(5),
                start: ProcessorId::new(6),
                end: ProcessorId::new(7),
            },
        },
        5 => VariantScope::StripFields {
            strip: StripId::new(8),
            fields: StripDomainMask(BTreeSet::from([StripField::Fader])),
        },
        _ => VariantScope::Arrangement {
            tracks: vec![TrackId::new(9)],
            time: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(960)).unwrap(),
            fields: ArrangementDomainMask {
                clips: true,
                notes: true,
            },
        },
    }
}

fn domain_for(case: usize) -> VariantDiffDomain {
    match case {
        0 => VariantDiffDomain::AudioClip,
        1 => VariantDiffDomain::TakeUsage,
        2 => VariantDiffDomain::MidiClip,
        3 => VariantDiffDomain::InstrumentSlot,
        4 => VariantDiffDomain::ProcessingRange,
        5 => VariantDiffDomain::StripFields,
        _ => VariantDiffDomain::Arrangement,
    }
}

fn authority_for(scope: &VariantScope) -> CompareAuthority {
    let mut branches = BTreeMap::new();
    branches.insert(
        BranchId::new(1),
        BranchAuthority {
            revision: BranchRevision::new(1),
            editing: false,
            publish: RuntimePublishState::NotRequested,
        },
    );
    branches.insert(
        BranchId::new(2),
        BranchAuthority {
            revision: BranchRevision::new(1),
            editing: false,
            publish: RuntimePublishState::NotRequested,
        },
    );
    CompareAuthority {
        variant_set: VariantSetId::new(SET),
        scope: scope.clone(),
        adopted: BranchId::new(1),
        project_revision: ProjectRevision(REV),
        scope_digest: ContentDigest::sha256(b"scope-digest"),
        branches,
    }
}

fn input_for(scope: &VariantScope, authority: CompareAuthority) -> VariantDiffInput {
    VariantDiffInput {
        context: CompareContext {
            variant_set: VariantSetId::new(SET),
            scope: scope.clone(),
            left: pin(1, 1),
            right: pin(2, 1),
            scope_digest: authority.scope_digest,
            project_revision: authority.project_revision,
        },
        authority,
        semantic: SemanticEvidence {
            state: EvidenceState::Unavailable {
                reason: VariantDiffReason::EvidenceUnavailable,
            },
            diff: None,
        },
        signal: AuxLayerEvidence::default(),
        perceptual: AuxLayerEvidence::default(),
        // Editability available by default so adopt intents can be
        // exercised; exact audition stays honestly unavailable
        // (missing plugin) per CB-UI-RCP-01.
        capabilities: CapabilityEvidence {
            editability: EvidenceState::Available,
            exact_audition: EvidenceState::Unavailable {
                reason: VariantDiffReason::MissingPlugin,
            },
            reasons: vec![VariantDiffReason::MissingPlugin],
        },
    }
}

fn empty_diff() -> SemanticDiffEvidence {
    SemanticDiffEvidence {
        changes: vec![],
        unresolved: vec![],
        overall: ComparisonEquality::Equal,
        completeness: ComparisonCompleteness::Complete,
        summary: ChangeSetSummary::default(),
    }
}

fn available_semantic(diff: SemanticDiffEvidence) -> SemanticEvidence {
    SemanticEvidence {
        state: EvidenceState::Available,
        diff: Some(diff),
    }
}

// ── real variant-set fixture (merged CB-01 path) ────────────────────

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(synvelis_project::ProjectId::new(1), "cb09-fixture"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply_op(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

fn clip_placement(anchor_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
    }
}

fn clip_playback() -> ClipPlayback {
    ClipPlayback {
        source_region: synvelis_project::range::SourceRange::try_new(
            SourceFrame::ZERO,
            SourceFrame::new(1_000_000).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

/// Track → lane → asset+source → one audio clip → one clip note.
fn audio_fixture() -> (ProjectController, TrackId, LaneId, ClipId, NoteId) {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::RegisterAsset {
            digest: "asset-a".into(),
        },
    );
    let asset = *c.live().assets.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "digest-1".into(),
        },
    );
    let source = *c.live().sources.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: clip_placement(0),
            playback: clip_playback(),
        },
    );
    let clip = *c.live().clips.keys().next_back().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddNote {
            clip,
            key: 60,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    let note = *c.live().notes.keys().next_back().unwrap();
    (c, track, lane, clip, note)
}

struct RealFixture {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    variant_set: VariantSetId,
    candidate: BranchId,
    scope: VariantScope,
}

fn real_variant_fixture() -> RealFixture {
    let (mut c, _t, _l, clip, _n) = audio_fixture();
    let store = InMemoryBranchContentStore::default();
    let adapter = FakeDomainAdapter::new();
    let resolver = SnapshotResolver::new();
    let scope = VariantScope::AudioClip { clip };
    let mut store = store;
    let receipt = fork_variant_set(
        &mut c,
        &adapter,
        &mut store,
        &resolver,
        scope.clone(),
        &SnapshotCancel::new(),
    )
    .unwrap();
    // One scoped candidate edit: mute the clip.
    let candidate_rev = c.live().variant_sets[&receipt.variant_set].branches[&receipt.candidate]
        .content
        .revision();
    edit_branch(
        &mut c,
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
    .unwrap();
    RealFixture {
        controller: c,
        store,
        variant_set: receipt.variant_set,
        candidate: receipt.candidate,
        scope,
    }
}

fn snap_manual(project: Project, schema_version: u32, tag: &str) -> ImmutableSnapshot {
    let bytes = serde_json::to_vec(&project).unwrap();
    let mut material = tag.as_bytes().to_vec();
    material.extend_from_slice(&schema_version.to_le_bytes());
    material.extend_from_slice(&bytes);
    ImmutableSnapshot {
        identity: SnapshotIdentity {
            project_id: project.project_id,
            source: SnapshotOrigin::Live,
            locator: FixedSnapshotLocator::Live {
                project_id: project.project_id,
                live_revision: ProjectRevision(0),
            },
            digest: ContentDigest::sha256(&material),
            schema_version,
            projection_version: SCHEMA_VERSION,
            capture_generation: 0,
        },
        origin: SnapshotOrigin::Live,
        projection: ReadOnlyProjectProjection::new(schema_version, project),
        validation: SnapshotValidation::Valid,
        assets: AssetAvailabilityManifest::default(),
        opaque_state: Vec::new(),
        completeness: SnapshotCompleteness::Complete,
    }
}

fn compare_projects(before: &Project, after: &Project) -> ChangeSet {
    let a = snap_manual(before.clone(), SCHEMA_VERSION, "before");
    let b = snap_manual(after.clone(), SCHEMA_VERSION, "after");
    compare_snapshots(
        &a,
        &b,
        &ComparisonSpec::new(a.identity.clone(), b.identity.clone()),
        &SnapshotCancel::new(),
    )
    .unwrap()
}

fn real_authority(fx: &RealFixture) -> CompareAuthority {
    let set = &fx.controller.live().variant_sets[&fx.variant_set];
    CompareAuthority {
        variant_set: fx.variant_set,
        scope: fx.scope.clone(),
        adopted: set.adopted,
        project_revision: fx.controller.live_revision(),
        scope_digest: scope_digest(fx.controller.live(), &fx.scope).unwrap(),
        branches: set
            .branches
            .iter()
            .map(|(id, b)| {
                (
                    *id,
                    BranchAuthority {
                        revision: b.content.revision(),
                        editing: false,
                        publish: RuntimePublishState::NotRequested,
                    },
                )
            })
            .collect(),
    }
}

fn real_input(fx: &RealFixture) -> VariantDiffInput {
    let authority = real_authority(fx);
    let set = &fx.controller.live().variant_sets[&fx.variant_set];
    let candidate_project =
        decode_candidate(&fx.store, &set.branches[&fx.candidate].content).unwrap();
    let cs = compare_projects(&fx.controller.live().clone(), &candidate_project);
    VariantDiffInput {
        context: CompareContext {
            variant_set: fx.variant_set,
            scope: fx.scope.clone(),
            left: BranchPin {
                branch: set.adopted,
                revision: set.branches[&set.adopted].content.revision(),
            },
            right: BranchPin {
                branch: fx.candidate,
                revision: set.branches[&fx.candidate].content.revision(),
            },
            scope_digest: authority.scope_digest,
            project_revision: authority.project_revision,
        },
        authority,
        semantic: available_semantic(SemanticDiffEvidence::from(&cs)),
        signal: AuxLayerEvidence::default(),
        perceptual: AuxLayerEvidence::default(),
        capabilities: CapabilityEvidence {
            editability: EvidenceState::Available,
            exact_audition: EvidenceState::Unavailable {
                reason: VariantDiffReason::MissingPlugin,
            },
            reasons: vec![VariantDiffReason::MissingPlugin],
        },
    }
}

// ── spec helpers ────────────────────────────────────────────────────

fn collect_keys(spec: &ComponentSpec, out: &mut Vec<String>) {
    out.push(spec.key().0.clone());
    if let ComponentSpec::Panel(p) = spec {
        for child in &p.children {
            collect_keys(child, out);
        }
    }
}

fn collect_labels(spec: &ComponentSpec, out: &mut Vec<String>) {
    match spec {
        ComponentSpec::Label(l) => out.push(l.text.clone()),
        ComponentSpec::Panel(p) => {
            for child in &p.children {
                collect_labels(child, out);
            }
        }
        _ => {}
    }
}

fn count_buttons(spec: &ComponentSpec) -> usize {
    let mut n = 0;
    match spec {
        ComponentSpec::Button(_) => n += 1,
        ComponentSpec::Panel(p) => {
            for child in &p.children {
                n += count_buttons(child);
            }
        }
        _ => {}
    }
    n
}

// ── CB-UX-03: per-domain projection + honest unsupported ────────────

#[test]
fn cb09_domain_labels_cover_every_scope() {
    for case in 0..7 {
        let scope = scope_for(case);
        let authority = authority_for(&scope);
        let input = input_for(&scope, authority);
        let view = project_variant_diff(&input, &VariantDiffViewState::default());
        assert_eq!(view.domain, domain_for(case), "case {case}");
        // Every enumerated domain has a merged CB-0x adapter —
        // arrangement included (CB-07/#284). The
        // `DomainAdapterUnmerged` verdict stays typed for future
        // unmerged domains but no current scope triggers it.
        assert!(view.domain_state.is_available(), "case {case}");
    }
}

#[test]
fn cb09_arrangement_domain_projects_supplied_evidence() {
    let scope = scope_for(6);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    // Arrangement is a supported domain now — caller-supplied diff
    // evidence projects like any other domain's rows.
    input.semantic = available_semantic(empty_diff());
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(view.semantic.state.is_available());
    assert_eq!(view.semantic.equality, Some(ComparisonEquality::Equal));
    assert_eq!(view.semantic.rows, Vec::new());
}

#[test]
fn cb09_supported_domain_projects_empty_diff_as_equal() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    input.semantic = available_semantic(empty_diff());
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(view.semantic.state.is_available());
    assert_eq!(view.semantic.equality, Some(ComparisonEquality::Equal));
    assert_eq!(view.semantic.rows, Vec::new());
}

// ── real merged-path semantic evidence ──────────────────────────────

#[test]
fn cb09_real_changeset_projects_domain_rows() {
    let fx = real_variant_fixture();
    let input = real_input(&fx);
    let view = project_variant_diff(&input, &VariantDiffViewState::default());

    assert_eq!(view.domain, VariantDiffDomain::AudioClip);
    assert!(matches!(view.context_state, ContextState::Current));
    assert!(view.semantic.state.is_available());
    assert_eq!(
        view.semantic.equality,
        Some(ComparisonEquality::Different),
        "muted candidate vs live must be Different"
    );
    // The scoped clip change is a content row; variant-set/branch
    // bookkeeping is counted, not shown as content.
    assert!(
        view.semantic
            .rows
            .iter()
            .any(|r| r.object_kind_label == "clip" && r.change_label == "modified"),
        "rows: {:?}",
        view.semantic
            .rows
            .iter()
            .map(|r| &r.object_kind_label)
            .collect::<Vec<_>>()
    );
    assert!(view.semantic.variants_bookkeeping >= 1);
    assert!(
        view.semantic
            .rows
            .iter()
            .all(|r| r.object_kind_label != "variant-branch")
    );
    // Field evidence is projected verbatim.
    let clip_row = view
        .semantic
        .rows
        .iter()
        .find(|r| r.object_kind_label == "clip")
        .unwrap();
    assert!(clip_row.field_count >= 1);
    assert_ne!(clip_row.fields, Vec::new());
}

// ── CB-UX-02: independent branch labels ─────────────────────────────

#[test]
fn cb09_branch_labels_are_independent_axes() {
    let scope = scope_for(0);
    let mut authority = authority_for(&scope);
    authority
        .branches
        .get_mut(&BranchId::new(1))
        .unwrap()
        .publish = RuntimePublishState::Pending {
        reason: "activating".into(),
    };
    authority
        .branches
        .get_mut(&BranchId::new(2))
        .unwrap()
        .editing = true;
    authority
        .branches
        .get_mut(&BranchId::new(2))
        .unwrap()
        .publish = RuntimePublishState::Published;
    let input = input_for(&scope, authority);

    let state = VariantDiffViewState {
        audition: Some(pin(2, 1)),
        ..Default::default()
    };
    let view = project_variant_diff(&input, &state);

    let left = &view.branches[0];
    let right = &view.branches[1];
    assert!(
        left.labels
            .contains(&synvelis_workspace::BranchLabel::Adopted)
    );
    assert!(
        left.labels
            .contains(&synvelis_workspace::BranchLabel::RtPending)
    );
    assert!(
        !left
            .labels
            .contains(&synvelis_workspace::BranchLabel::Auditioning)
    );
    // Right is simultaneously Auditioning + Editing + RT Active — the
    // axes never collapse into one another (CB-UX-02).
    assert!(
        right
            .labels
            .contains(&synvelis_workspace::BranchLabel::Auditioning)
    );
    assert!(
        right
            .labels
            .contains(&synvelis_workspace::BranchLabel::Editing)
    );
    assert!(
        right
            .labels
            .contains(&synvelis_workspace::BranchLabel::RtActive)
    );
    assert!(
        !right
            .labels
            .contains(&synvelis_workspace::BranchLabel::Adopted)
    );
}

// ── CB-UX-01/04: explicit-target binding ────────────────────────────

#[test]
fn cb09_compare_and_adopt_require_explicit_pins() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let input = input_for(&scope, authority.clone());
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    let mut state = VariantDiffViewState::default();

    // Compare: explicit left/right pins → typed intent.
    let params = serde_json::json!({ "left": pin(1, 1), "right": pin(2, 1) });
    let action = decode_variant_diff_action(ACTION_VARIANT_DIFF_COMPARE, Some(&params)).unwrap();
    let intent = apply_variant_diff_action(&mut state, &view, &authority, &action)
        .unwrap()
        .expect("compare yields an intent");
    assert!(matches!(intent, VariantDiffIntent::Compare { .. }));

    // Adopt: explicit target pin → intent carries the authority scope
    // digest as the CAS guard (client never asserts it).
    let params = serde_json::to_value(pin(2, 1)).unwrap();
    let action = decode_variant_diff_action(ACTION_VARIANT_DIFF_ADOPT, Some(&params)).unwrap();
    let intent = apply_variant_diff_action(&mut state, &view, &authority, &action)
        .unwrap()
        .expect("adopt yields an intent");
    match intent {
        VariantDiffIntent::Adopt {
            target,
            scope_digest,
        } => {
            assert_eq!(target, pin(2, 1));
            assert_eq!(scope_digest, authority.scope_digest);
        }
        other => panic!("expected Adopt intent, got {other:?}"),
    }

    // Adopt preview shares the same validation.
    let action =
        decode_variant_diff_action(ACTION_VARIANT_DIFF_ADOPT_PREVIEW, Some(&params)).unwrap();
    let intent = apply_variant_diff_action(&mut state, &view, &authority, &action)
        .unwrap()
        .expect("adopt-preview yields an intent");
    assert!(matches!(intent, VariantDiffIntent::AdoptPreview { .. }));
}

#[test]
fn cb09_implicit_malformed_and_stale_targets_fail_closed() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let input = input_for(&scope, authority.clone());
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    let mut state = VariantDiffViewState::default();

    // Unknown action name → reject (no implicit target can be built).
    assert_eq!(
        decode_variant_diff_action("bogus-action", None),
        Err(VariantDiffReject::ImplicitTarget)
    );
    // Pin-carrying action without params → ImplicitTarget.
    assert_eq!(
        decode_variant_diff_action(ACTION_VARIANT_DIFF_COMPARE, None),
        Err(VariantDiffReject::ImplicitTarget)
    );
    assert_eq!(
        decode_variant_diff_action(ACTION_VARIANT_DIFF_ADOPT, None),
        Err(VariantDiffReject::ImplicitTarget)
    );
    // Missing half of the compare pair → ImplicitTarget.
    let partial = serde_json::json!({ "left": pin(1, 1) });
    assert_eq!(
        decode_variant_diff_action(ACTION_VARIANT_DIFF_COMPARE, Some(&partial)),
        Err(VariantDiffReject::ImplicitTarget)
    );
    // Wrong payload shape → Malformed.
    let bad = serde_json::json!("not-a-pin");
    assert_eq!(
        decode_variant_diff_action(ACTION_VARIANT_DIFF_ADOPT, Some(&bad)),
        Err(VariantDiffReject::Malformed)
    );
    let bad_select = serde_json::json!({ "row": "x" });
    assert_eq!(
        decode_variant_diff_action(ACTION_VARIANT_DIFF_SELECT, Some(&bad_select)),
        Err(VariantDiffReject::Malformed)
    );

    // Same-branch compare → typed reject.
    let same = serde_json::json!({ "left": pin(1, 1), "right": pin(1, 1) });
    let action = decode_variant_diff_action(ACTION_VARIANT_DIFF_COMPARE, Some(&same)).unwrap();
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &authority, &action),
        Err(VariantDiffReject::SameBranch)
    );

    // Stale revision pin → typed reject.
    let stale_params = serde_json::to_value(pin(2, 99)).unwrap();
    let action =
        decode_variant_diff_action(ACTION_VARIANT_DIFF_ADOPT, Some(&stale_params)).unwrap();
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &authority, &action),
        Err(VariantDiffReject::StaleRevision)
    );

    // Unknown branch → typed reject.
    let ghost = serde_json::to_value(pin(77, 1)).unwrap();
    let action = decode_variant_diff_action(ACTION_VARIANT_DIFF_ADOPT, Some(&ghost)).unwrap();
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &authority, &action),
        Err(VariantDiffReject::UnknownBranch)
    );
}

#[test]
fn cb09_arrangement_domain_mints_authority_intents() {
    let scope = scope_for(6);
    let authority = authority_for(&scope);
    let input = input_for(&scope, authority.clone());
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    let mut state = VariantDiffViewState::default();

    // Arrangement's adapter is merged — compare and adopt candidates
    // mint typed intents like every other supported domain.
    let params = serde_json::json!({ "left": pin(1, 1), "right": pin(2, 1) });
    let action = decode_variant_diff_action(ACTION_VARIANT_DIFF_COMPARE, Some(&params)).unwrap();
    assert!(matches!(
        apply_variant_diff_action(&mut state, &view, &authority, &action),
        Ok(Some(VariantDiffIntent::Compare { .. }))
    ));
    let params = serde_json::to_value(pin(2, 1)).unwrap();
    let action = decode_variant_diff_action(ACTION_VARIANT_DIFF_ADOPT, Some(&params)).unwrap();
    assert!(matches!(
        apply_variant_diff_action(&mut state, &view, &authority, &action),
        Ok(Some(VariantDiffIntent::Adopt { .. }))
    ));
    assert!(view.actions.iter().any(|a| a.executable));
}

// ── CB-CMP-01: stale / rejected context ─────────────────────────────

#[test]
fn cb09_stale_and_rejected_contexts_are_honest() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);

    // Stale branch revision.
    let mut input = input_for(&scope, authority.clone());
    input.context.left.revision = BranchRevision::new(2);
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(matches!(
        &view.context_state,
        ContextState::Stale { reasons }
            if reasons.contains(&VariantDiffReason::StaleRevision)
    ));
    assert!(view.actions.iter().all(|a| !a.executable));

    // Stale scope digest.
    let mut input = input_for(&scope, authority.clone());
    input.context.scope_digest = ContentDigest::sha256(b"other");
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(matches!(
        &view.context_state,
        ContextState::Stale { reasons }
            if reasons.contains(&VariantDiffReason::StaleScope)
    ));

    // Stale project revision.
    let mut input = input_for(&scope, authority.clone());
    input.context.project_revision = ProjectRevision(REV + 1);
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(matches!(
        &view.context_state,
        ContextState::Stale { reasons }
            if reasons.contains(&VariantDiffReason::StaleProject)
    ));

    // Unknown set.
    let mut input = input_for(&scope, authority.clone());
    input.context.variant_set = VariantSetId::new(999);
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(matches!(
        view.context_state,
        ContextState::Rejected {
            reason: VariantDiffReason::UnknownSet
        }
    ));

    // Unknown branch.
    let mut input = input_for(&scope, authority.clone());
    input.context.right.branch = BranchId::new(88);
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(matches!(
        view.context_state,
        ContextState::Rejected {
            reason: VariantDiffReason::UnknownBranch
        }
    ));
}

// ── CB-CMP-02: unknown/unavailable never becomes zero-diff ──────────

#[test]
fn cb09_unavailable_and_unknown_layers_are_not_zero_diff() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    input.semantic = SemanticEvidence {
        state: EvidenceState::Unknown {
            reason: VariantDiffReason::EvidenceUnknown,
        },
        diff: None,
    };
    input.signal = AuxLayerEvidence::unavailable(VariantDiffReason::EvidenceUnavailable);
    input.perceptual = AuxLayerEvidence {
        state: EvidenceState::Unknown {
            reason: VariantDiffReason::EvidenceUnknown,
        },
        ..AuxLayerEvidence::default()
    };
    let view = project_variant_diff(&input, &VariantDiffViewState::default());

    assert!(matches!(view.semantic.state, EvidenceState::Unknown { .. }));
    assert!(
        view.semantic.equality.is_none(),
        "Unknown must not claim Equal"
    );
    assert!(matches!(
        view.signal.state,
        EvidenceState::Unavailable { .. }
    ));
    assert_eq!(view.signal.alignment, AlignmentState::Unavailable);
    assert_eq!(view.signal.rows, Vec::new());
    assert!(matches!(
        view.perceptual.state,
        EvidenceState::Unknown { .. }
    ));
    assert_eq!(view.perceptual.alignment, AlignmentState::Unavailable);
}

// ── CB-CMP-03 / CB-UI-RCP-01: editability vs exact audition ─────────

#[test]
fn cb09_missing_plugin_splits_editability_from_audition() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    input.capabilities = CapabilityEvidence {
        editability: EvidenceState::Available,
        exact_audition: EvidenceState::Unavailable {
            reason: VariantDiffReason::MissingPlugin,
        },
        reasons: vec![VariantDiffReason::MissingPlugin],
    };
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(view.capabilities.editability.is_available());
    assert!(matches!(
        view.capabilities.exact_audition,
        EvidenceState::Unavailable {
            reason: VariantDiffReason::MissingPlugin
        }
    ));
    assert_eq!(
        view.capabilities.reasons,
        vec![VariantDiffReason::MissingPlugin]
    );
}

// ── mock provenance is labelled, never measured ─────────────────────

#[test]
fn cb09_mock_evidence_is_labelled_not_measured() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    input.signal = AuxLayerEvidence::available(
        EvidenceOrigin::Mock,
        AlignmentState::Approximate,
        vec![EvidenceRow {
            label: "fake-loudness-delta".into(),
            value: Some("+0.8 LU (fake)".into()),
            detail: None,
        }],
    );
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert_eq!(view.signal.origin, Some(EvidenceOrigin::Mock));
    assert_eq!(view.signal.alignment, AlignmentState::Approximate);
    assert_eq!(view.signal.rows.len(), 1);

    let spec = variant_diff_spec(&view, &VariantDiffViewState::default());
    let mut texts = Vec::new();
    collect_labels(&spec.root, &mut texts);
    assert!(
        texts.iter().any(|t| t.contains("mock")),
        "spec must label mock provenance: {texts:?}"
    );
}

// ── ViewSpec determinism + executable-only controls ─────────────────

#[test]
fn cb09_viewspec_is_deterministic_with_stable_keys() {
    let fx = real_variant_fixture();
    let input = real_input(&fx);
    let state = VariantDiffViewState::default();
    let view = project_variant_diff(&input, &state);

    let spec_a = variant_diff_spec(&view, &state);
    let spec_b = variant_diff_spec(&view, &state);
    assert_eq!(
        serde_json::to_value(&spec_a).unwrap(),
        serde_json::to_value(&spec_b).unwrap(),
        "same input must produce identical ViewSpec"
    );

    let mut keys = Vec::new();
    collect_keys(&spec_a.root, &mut keys);
    let unique: std::collections::BTreeSet<_> = keys.iter().collect();
    assert_eq!(keys.len(), unique.len(), "component keys must be unique");
    assert!(keys.iter().any(|k| k == "variant-diff"));
    assert!(keys.iter().any(|k| k == "variant-diff.semantic.rows"));

    // Executable candidates → buttons; the adopted branch gets no
    // adopt buttons; audition toggles are always present (view-local).
    let buttons = count_buttons(&spec_a.root);
    // compare(1) + adopt-preview/adopt for candidate(2) + audition(2) = 5
    assert_eq!(buttons, 5, "buttons={buttons}");
}

#[test]
fn cb09_blocked_actions_never_become_buttons() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    input.context.left.revision = BranchRevision::new(2); // stale
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    let spec = variant_diff_spec(&view, &VariantDiffViewState::default());
    // Only the two view-local audition toggles remain executable.
    assert_eq!(count_buttons(&spec.root), 2);
    let mut texts = Vec::new();
    collect_labels(&spec.root, &mut texts);
    assert!(texts.iter().any(|t| t.contains("blocked: stale-revision")));
}

// ── view-local state + tolerant serde ───────────────────────────────

#[test]
fn cb09_viewstate_roundtrip_and_tolerant_decode() {
    let target = VariantDiffTarget {
        variant_set: VariantSetId::new(SET),
        left: pin(1, 1),
        right: pin(2, 1),
    };
    let vs = ViewState {
        variant_diff: Some(VariantDiffViewState {
            target: Some(target),
            audition: Some(pin(2, 1)),
            selected_row: Some(3),
            show_signal: false,
            show_perceptual: true,
        }),
        ..Default::default()
    };
    let json = serde_json::to_string(&vs).unwrap();
    let back: ViewState = serde_json::from_str(&json).unwrap();
    assert_eq!(vs.variant_diff, back.variant_diff);

    // Malformed payload degrades to None — never rejects the snapshot.
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    v["variant_diff"] = serde_json::json!("bogus");
    let back2: ViewState = serde_json::from_value(v).unwrap();
    assert!(back2.variant_diff.is_none());

    // Absent field (pre-CB-09 snapshot shape) defaults to None.
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    v.as_object_mut().unwrap().remove("variant_diff");
    let back3: ViewState = serde_json::from_value(v).unwrap();
    assert!(back3.variant_diff.is_none());
}

#[test]
fn cb09_view_local_actions_never_touch_authority() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let input = input_for(&scope, authority.clone());
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    let mut state = VariantDiffViewState::default();

    // SelectRow / SetAudition / SetTarget are view-local: Ok(None),
    // state mutates, no intent is minted.
    let action = VariantDiffViewAction::SelectRow { row: 4 };
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &authority, &action).unwrap(),
        None
    );
    assert_eq!(state.selected_row, Some(4));

    let action = VariantDiffViewAction::SetAudition {
        branch: Some(pin(2, 1)),
    };
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &authority, &action).unwrap(),
        None
    );
    assert_eq!(state.audition, Some(pin(2, 1)));

    let action = VariantDiffViewAction::SetTarget {
        target: VariantDiffTarget {
            variant_set: VariantSetId::new(SET),
            left: pin(1, 1),
            right: pin(2, 1),
        },
    };
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &authority, &action).unwrap(),
        None
    );
    assert!(state.target.is_some());

    // Audition decode round-trips through the wire contract.
    let params = serde_json::json!({ "branch": serde_json::to_value(pin(1, 1)).unwrap() });
    let action = decode_variant_diff_action(ACTION_VARIANT_DIFF_AUDITION, Some(&params)).unwrap();
    assert!(matches!(
        action,
        VariantDiffViewAction::SetAudition { branch: Some(_) }
    ));
    let params = serde_json::json!({ "branch": null });
    let action = decode_variant_diff_action(ACTION_VARIANT_DIFF_AUDITION, Some(&params)).unwrap();
    assert!(matches!(
        action,
        VariantDiffViewAction::SetAudition { branch: None }
    ));
}

// ── registration ────────────────────────────────────────────────────

#[test]
fn cb09_view_type_registers_as_variant_diff() {
    let json = serde_json::to_value(ViewType::VariantDiff).unwrap();
    assert_eq!(json, serde_json::json!("variant-diff"));
    let back: ViewType = serde_json::from_value(json).unwrap();
    assert_eq!(back, ViewType::VariantDiff);
    // The logical target is a single stable binding point.
    assert_eq!(VARIANT_DIFF_TARGET, "cb.variant-diff");
}

// ── review-protocol regression coverage ─────────────────────────────
//
// The cases below pin defects found by the sv#286 review pass:
// duplicate component keys (unmountable spec), undecodable
// stop-audition params, apply-side context/domain/capability gates,
// provenance-less aux evidence, scope-mismatch reason, row caps, and
// pinned-context visibility.

fn collect_buttons(spec: &ComponentSpec, out: &mut Vec<ButtonSpec>) {
    match spec {
        ComponentSpec::Button(b) => out.push(b.clone()),
        ComponentSpec::Panel(p) => {
            for child in &p.children {
                collect_buttons(child, out);
            }
        }
        _ => {}
    }
}

fn synthetic_change(i: usize) -> SemanticChange {
    SemanticChange {
        id: ChangeId(format!("chg-{i}")),
        object: SemanticObjectRef {
            project_id: ProjectId::new(1),
            kind: ObjectKind::Clip,
            stable_id: format!("clip-{i:04}"),
            lineage: None,
        },
        kind: ChangeKind::Modified,
        before: None,
        after: None,
        affected_range: None,
        evidence: ChangeEvidence { fields: vec![] },
        dependencies: vec![],
    }
}

fn assert_unique_keys(spec: &ComponentSpec) {
    let mut keys = Vec::new();
    collect_keys(spec, &mut keys);
    let unique: BTreeSet<_> = keys.iter().collect();
    assert_eq!(
        keys.len(),
        unique.len(),
        "duplicate component keys: {keys:?}"
    );
}

#[test]
fn cb09_spec_keys_unique_with_two_non_adopted_sides() {
    // adopted is a THIRD branch — both compared sides are candidates,
    // so adopt-preview/adopt candidates exist twice. Keys must still
    // be unique or ViewHost::validate_spec rejects the whole spec.
    let scope = scope_for(0);
    let mut authority = authority_for(&scope);
    authority.adopted = BranchId::new(3);
    authority.branches.insert(
        BranchId::new(3),
        BranchAuthority {
            revision: BranchRevision::new(1),
            editing: false,
            publish: RuntimePublishState::NotRequested,
        },
    );
    let mut input = input_for(&scope, authority);
    input.context.left = pin(1, 1);
    input.context.right = pin(2, 1);
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    let spec = variant_diff_spec(&view, &VariantDiffViewState::default());

    assert_unique_keys(&spec.root);
    // compare + preview/adopt ×2 sides + audition ×2 = 7 buttons.
    assert_eq!(count_buttons(&spec.root), 7);
    let mut keys = Vec::new();
    collect_keys(&spec.root, &mut keys);
    assert!(
        keys.iter()
            .any(|k| k == "variant-diff.action.adopt-preview.left")
    );
    assert!(
        keys.iter()
            .any(|k| k == "variant-diff.action.adopt-preview.right")
    );
    assert!(keys.iter().any(|k| k == "variant-diff.action.adopt.left"));
    assert!(keys.iter().any(|k| k == "variant-diff.action.adopt.right"));
}

#[test]
fn cb09_spec_keys_unique_with_many_issues_and_reasons() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    input.semantic = available_semantic(SemanticDiffEvidence {
        changes: vec![],
        unresolved: vec![
            ComparisonIssue::InvalidReference {
                detail: "missing asset".into(),
            },
            ComparisonIssue::InvalidReference {
                detail: "dangling ref".into(),
            },
            ComparisonIssue::InvalidReference {
                detail: "missing asset".into(),
            },
        ],
        overall: ComparisonEquality::Unknown(ComparisonIssue::InvalidReference {
            detail: "unresolved".into(),
        }),
        completeness: ComparisonCompleteness::Partial,
        summary: ChangeSetSummary::default(),
    });
    input.capabilities.reasons = vec![
        VariantDiffReason::MissingPlugin,
        VariantDiffReason::EvidenceUnknown,
        VariantDiffReason::MissingPlugin,
    ];
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    let spec = variant_diff_spec(&view, &VariantDiffViewState::default());
    assert_unique_keys(&spec.root);
    assert_eq!(view.semantic.issues.len(), 3);
}

#[test]
fn cb09_every_button_params_roundtrips_decode() {
    // Regression: the spec's own ButtonSpec.params must decode through
    // decode_variant_diff_action — the stop-audition double-wrap bug
    // made {"branch": {"branch": null}} undecodable.
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let input = input_for(&scope, authority.clone());
    let state = VariantDiffViewState {
        audition: Some(pin(2, 1)),
        ..Default::default()
    };
    let view = project_variant_diff(&input, &state);
    let spec = variant_diff_spec(&view, &state);

    let mut buttons = Vec::new();
    collect_buttons(&spec.root, &mut buttons);
    assert_ne!(buttons, Vec::new());
    let mut saw_stop = false;
    for button in &buttons {
        let decoded = decode_variant_diff_action(&button.action, button.params.as_ref());
        assert!(
            decoded.is_ok(),
            "button {:?} action {} params {} failed decode: {decoded:?}",
            button.key,
            button.action,
            button
                .params
                .as_ref()
                .map_or_else(|| "none".to_string(), ToString::to_string)
        );
        if button.label == "Stop Audition" {
            saw_stop = true;
            assert!(matches!(
                decoded.unwrap(),
                VariantDiffViewAction::SetAudition { branch: None }
            ));
        }
    }
    assert!(saw_stop, "auditioning state must offer a stop button");

    // And the decoded stop action actually clears the selection.
    let mut state2 = state.clone();
    let stop = decode_variant_diff_action(
        ACTION_VARIANT_DIFF_AUDITION,
        Some(&serde_json::json!({ "branch": null })),
    )
    .unwrap();
    apply_variant_diff_action(&mut state2, &view, &authority, &stop).unwrap();
    assert_eq!(state2.audition, None);
}

#[test]
fn cb09_apply_rejects_noncurrent_context() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority.clone());
    input.context.left.revision = BranchRevision::new(2); // stale pin
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(!matches!(view.context_state, ContextState::Current));

    let mut state = VariantDiffViewState::default();
    // Even with fresh, valid pins the intent is refused — apply
    // enforces the same gate the candidates advertised.
    let action = VariantDiffViewAction::Compare {
        left: pin(1, 1),
        right: pin(2, 1),
    };
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &authority, &action),
        Err(VariantDiffReject::StaleContext)
    );
    let action = VariantDiffViewAction::Adopt { target: pin(2, 1) };
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &authority, &action),
        Err(VariantDiffReject::StaleContext)
    );
}

#[test]
fn cb09_apply_domain_comes_from_authority_scope() {
    // View projected from an AudioClip context, but apply is handed an
    // authority for a *different* scope — the scope mismatch must fail
    // closed before any intent is minted. With every current domain
    // adapter merged the domain gate no longer rejects arrangement;
    // the context check still does.
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let input = input_for(&scope, authority.clone());
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(view.domain_state.is_available());

    let mut arrangement_authority = authority_for(&scope_for(6));
    arrangement_authority.variant_set = authority.variant_set;
    arrangement_authority.scope_digest = authority.scope_digest;
    arrangement_authority.project_revision = authority.project_revision;
    let mut state = VariantDiffViewState::default();
    let action = VariantDiffViewAction::Adopt { target: pin(2, 1) };
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &arrangement_authority, &action),
        Err(VariantDiffReject::StaleContext)
    );
}

#[test]
fn cb09_adopt_gated_by_editability_and_already_adopted() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority.clone());
    input.capabilities.editability = EvidenceState::Unavailable {
        reason: VariantDiffReason::EvidenceUnavailable,
    };
    let view = project_variant_diff(&input, &VariantDiffViewState::default());

    // Adopt candidate blocked with the capability reason; preview and
    // compare stay available (read-only paths).
    let adopt = view
        .actions
        .iter()
        .find(|a| a.kind == synvelis_workspace::VariantDiffActionKind::Adopt)
        .unwrap();
    assert!(!adopt.executable);
    assert_eq!(
        adopt.block_reason,
        Some(VariantDiffReason::EvidenceUnavailable)
    );
    assert!(view.actions.iter().any(|a| a.kind
        == synvelis_workspace::VariantDiffActionKind::AdoptPreview
        && a.executable));

    let mut state = VariantDiffViewState::default();
    let action = VariantDiffViewAction::Adopt { target: pin(2, 1) };
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &authority, &action),
        Err(VariantDiffReject::CapabilityUnavailable)
    );
    // Preview is not the mutating intent — still allowed.
    let action = VariantDiffViewAction::AdoptPreview { target: pin(2, 1) };
    assert!(
        apply_variant_diff_action(&mut state, &view, &authority, &action)
            .unwrap()
            .is_some()
    );

    // Adopt-of-adopted is rejected even with editability available.
    let mut input2 = input_for(&scope, authority.clone());
    input2.capabilities.editability = EvidenceState::Available;
    let view2 = project_variant_diff(&input2, &VariantDiffViewState::default());
    let action = VariantDiffViewAction::Adopt { target: pin(1, 1) }; // branch 1 = adopted
    assert_eq!(
        apply_variant_diff_action(&mut state, &view2, &authority, &action),
        Err(VariantDiffReject::AlreadyAdopted)
    );
}

#[test]
fn cb09_aux_available_without_origin_is_unknown() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    input.signal = AuxLayerEvidence {
        state: EvidenceState::Available,
        origin: None, // provenance missing — contract violation
        alignment: AlignmentState::Aligned,
        rows: vec![EvidenceRow {
            label: "unattributed".into(),
            value: Some("1.0".into()),
            detail: None,
        }],
    };
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(matches!(
        view.signal.state,
        EvidenceState::Unknown {
            reason: VariantDiffReason::EvidenceUnknown
        }
    ));
    assert_eq!(view.signal.rows, Vec::new());
    assert_eq!(view.signal.alignment, AlignmentState::Unavailable);
}

#[test]
fn cb09_scope_mismatch_reports_unknown_scope() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    input.context.scope = scope_for(1); // different scope identity
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(matches!(
        view.context_state,
        ContextState::Rejected {
            reason: VariantDiffReason::UnknownScope
        }
    ));
}

#[test]
fn cb09_audition_decode_requires_object_and_branch_key() {
    assert_eq!(
        decode_variant_diff_action(ACTION_VARIANT_DIFF_AUDITION, None),
        Err(VariantDiffReject::ImplicitTarget)
    );
    assert_eq!(
        decode_variant_diff_action(ACTION_VARIANT_DIFF_AUDITION, Some(&serde_json::json!(5))),
        Err(VariantDiffReject::Malformed)
    );
    assert_eq!(
        decode_variant_diff_action(ACTION_VARIANT_DIFF_AUDITION, Some(&serde_json::json!({}))),
        Err(VariantDiffReject::ImplicitTarget)
    );
    assert_eq!(
        decode_variant_diff_action(
            ACTION_VARIANT_DIFF_AUDITION,
            Some(&serde_json::json!({ "branch": "x" })),
        ),
        Err(VariantDiffReject::Malformed)
    );
    // Explicit null clears; a pin sets.
    let action = decode_variant_diff_action(
        ACTION_VARIANT_DIFF_AUDITION,
        Some(&serde_json::json!({ "branch": null })),
    )
    .unwrap();
    assert!(matches!(
        action,
        VariantDiffViewAction::SetAudition { branch: None }
    ));
}

#[test]
fn cb09_rows_issues_and_aux_rows_are_capped_honestly() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    let changes: Vec<SemanticChange> = (0..300).map(synthetic_change).collect();
    let unresolved: Vec<ComparisonIssue> = (0..200)
        .map(|i| ComparisonIssue::InvalidReference {
            detail: format!("dangling-{i}"),
        })
        .collect();
    input.semantic = available_semantic(SemanticDiffEvidence {
        changes,
        unresolved,
        overall: ComparisonEquality::Different,
        completeness: ComparisonCompleteness::Complete,
        summary: ChangeSetSummary::default(),
    });
    input.signal = AuxLayerEvidence::available(
        EvidenceOrigin::Mock,
        AlignmentState::Approximate,
        (0..300)
            .map(|i| EvidenceRow {
                label: format!("row-{i}"),
                value: None,
                detail: None,
            })
            .collect(),
    );
    let view = project_variant_diff(&input, &VariantDiffViewState::default());

    assert_eq!(view.semantic.rows.len(), 256);
    assert_eq!(view.semantic.omitted_rows, 44);
    assert_eq!(view.semantic.issues.len(), 128);
    assert_eq!(view.semantic.omitted_issues, 72);
    assert_eq!(view.signal.rows.len(), 256);
    assert_eq!(view.signal.omitted_rows, 44);

    // The spec stays mountable (well under the 4096-row driver cap)
    // and says what was omitted.
    let spec = variant_diff_spec(&view, &VariantDiffViewState::default());
    let mut texts = Vec::new();
    collect_labels(&spec.root, &mut texts);
    assert!(texts.iter().any(|t| t.contains("of 300 content change")));
    assert!(texts.iter().any(|t| t.contains("more issue(s) omitted")));
    assert!(texts.iter().any(|t| t.contains("of 300 evidence row")));
}

#[test]
fn cb09_pinned_context_identity_is_visible_in_spec() {
    let fx = real_variant_fixture();
    let input = real_input(&fx);
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    let spec = variant_diff_spec(&view, &VariantDiffViewState::default());
    let mut texts = Vec::new();
    collect_labels(&spec.root, &mut texts);
    assert!(texts.iter().any(|t| t.contains("variant set:")));
    assert!(texts.iter().any(|t| t.contains("project revision:")));
    assert!(texts.iter().any(|t| t.contains("scope digest: sha256:")));
    // Bookkeeping count is surfaced when present.
    if view.semantic.variants_bookkeeping > 0 {
        assert!(texts.iter().any(|t| t.contains("variant bookkeeping:")));
    }
}

// ── review-protocol round 2 regression coverage ─────────────────────

#[test]
fn cb09_apply_revalidates_context_against_apply_time_authority() {
    // The projection-time `context_state` verdict is display state;
    // apply must re-derive it against the authority it is actually
    // validating pins for, or a drift window mints intents a fresh
    // projection would block.
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let input = input_for(&scope, authority.clone());
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert!(matches!(view.context_state, ContextState::Current));

    // Authority drifts after projection: the live project revision
    // advances while branch revisions stay put — action pins still
    // validate, so only the apply-time context re-check catches it.
    let mut drifted = authority.clone();
    drifted.project_revision = ProjectRevision(REV + 1);
    let mut state = VariantDiffViewState::default();
    let adopt = VariantDiffViewAction::Adopt { target: pin(2, 1) };
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &drifted, &adopt),
        Err(VariantDiffReject::StaleContext)
    );
    let compare = VariantDiffViewAction::Compare {
        left: pin(1, 1),
        right: pin(2, 1),
    };
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &drifted, &compare),
        Err(VariantDiffReject::StaleContext)
    );

    // Set/scope identity drift is likewise re-validated.
    let mut foreign = authority.clone();
    foreign.variant_set = VariantSetId::new(SET + 1);
    assert_eq!(
        apply_variant_diff_action(&mut state, &view, &foreign, &adopt),
        Err(VariantDiffReject::StaleContext)
    );

    // Sanity: the same actions succeed against the undrifted authority.
    assert!(
        apply_variant_diff_action(&mut state, &view, &authority, &compare)
            .unwrap()
            .is_some()
    );
}

#[test]
fn cb09_selected_row_projects_field_detail() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    let mut change = synthetic_change(0);
    change.evidence.fields = vec![SemanticFieldDiff {
        field: "mute".into(),
        before: serde_json::json!(false),
        after: serde_json::json!(true),
    }];
    input.semantic = available_semantic(SemanticDiffEvidence {
        changes: vec![change],
        unresolved: vec![],
        overall: ComparisonEquality::Different,
        completeness: ComparisonCompleteness::Complete,
        summary: ChangeSetSummary {
            total: 1,
            modified: 1,
            ..ChangeSetSummary::default()
        },
    });
    let view = project_variant_diff(&input, &VariantDiffViewState::default());

    // No selection → no detail labels.
    let state = VariantDiffViewState::default();
    let spec = variant_diff_spec(&view, &state);
    let mut texts = Vec::new();
    collect_labels(&spec.root, &mut texts);
    assert!(!texts.iter().any(|t| t.starts_with("selected:")));

    // SelectRow is view-local; the next spec surfaces the bounded
    // field evidence for exactly that row.
    let mut state = VariantDiffViewState::default();
    let action = VariantDiffViewAction::SelectRow { row: 0 };
    assert!(
        apply_variant_diff_action(&mut state, &view, &input.authority, &action)
            .unwrap()
            .is_none()
    );
    let spec = variant_diff_spec(&view, &state);
    let mut texts = Vec::new();
    collect_labels(&spec.root, &mut texts);
    assert!(texts.iter().any(|t| t.contains("selected: clip clip-0000")));
    assert!(texts.iter().any(|t| t.contains("mute: false -> true")));

    // An out-of-range selection (crafted payload) shows nothing.
    state.selected_row = Some(99);
    let spec = variant_diff_spec(&view, &state);
    let mut texts = Vec::new();
    collect_labels(&spec.root, &mut texts);
    assert!(!texts.iter().any(|t| t.starts_with("selected:")));
}

#[test]
fn cb09_same_branch_context_dedupes_adopt_candidates() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    // Both sides pin the same non-adopted branch — Compare is
    // SameBranch-blocked, and adopt must not mint duplicate A/B
    // candidates for the identical target.
    input.context.left = pin(2, 1);
    input.context.right = pin(2, 1);
    let view = project_variant_diff(&input, &VariantDiffViewState::default());

    let adopts: Vec<_> = view
        .actions
        .iter()
        .filter(|a| {
            matches!(
                a.kind,
                VariantDiffActionKind::Adopt | VariantDiffActionKind::AdoptPreview
            )
        })
        .collect();
    assert_eq!(adopts.len(), 2, "one preview + one adopt, left side only");
    assert!(adopts.iter().all(|a| a.side == Some(CompareSide::Left)));
    let compare = view
        .actions
        .iter()
        .find(|a| a.kind == VariantDiffActionKind::Compare)
        .unwrap();
    assert!(!compare.executable);
    assert_eq!(compare.block_reason, Some(VariantDiffReason::SameBranch));
}

#[test]
fn cb09_capability_reasons_display_is_capped() {
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    input.capabilities.reasons = (0..40).map(|_| VariantDiffReason::MissingPlugin).collect();
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    // The view model keeps every reason verbatim — only the spec
    // projection truncates, with an explicit omitted count.
    assert_eq!(view.capabilities.reasons.len(), 40);
    let spec = variant_diff_spec(&view, &VariantDiffViewState::default());
    let mut texts = Vec::new();
    collect_labels(&spec.root, &mut texts);
    let reason_labels = texts
        .iter()
        .filter(|t| t.starts_with("reason: missing-plugin"))
        .count();
    assert_eq!(reason_labels, 32);
    assert!(texts.iter().any(|t| t.contains("8 more not shown")));
}

#[test]
fn cb09_summary_label_reports_every_change_kind() {
    // A moved-only diff must not render as "+0 -0 ~0" — every
    // ChangeSetSummary counter reaches the spec label.
    let scope = scope_for(0);
    let authority = authority_for(&scope);
    let mut input = input_for(&scope, authority);
    input.semantic = available_semantic(SemanticDiffEvidence {
        changes: vec![synthetic_change(0)],
        unresolved: vec![],
        overall: ComparisonEquality::Different,
        completeness: ComparisonCompleteness::Complete,
        summary: ChangeSetSummary {
            total: 5,
            moved: 3,
            reordered: 1,
            reconnected: 1,
            ..ChangeSetSummary::default()
        },
    });
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    let spec = variant_diff_spec(&view, &VariantDiffViewState::default());
    let mut texts = Vec::new();
    collect_labels(&spec.root, &mut texts);
    let summary = texts
        .iter()
        .find(|t| t.starts_with("changes:"))
        .expect("summary label");
    assert!(summary.contains("changes: 5"), "{summary}");
    assert!(summary.contains("3 moved"), "{summary}");
    assert!(summary.contains("1 reordered"), "{summary}");
    assert!(summary.contains("1 reconnected"), "{summary}");
}

// ── CB-08 provider adapter + dispatch facade (synvelis#286 wiring) ──
//
// `VariantDiffEngine` is the workspace-side seam connecting the CB-09
// view-model to the merged CB-07/CB-08/CB-10 machinery: real domain
// adapters, the comparison orchestrator, and the gateway's
// simulate/execute commit gate — the app never names gateway types.
// Fixtures fork through the *same* directory store the engine's
// standard ports open, so captured candidate payloads resolve through
// the real read path.

static ENGINE_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

fn engine_root() -> PathBuf {
    let seq = ENGINE_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("synvelis-cb09-engine-{}-{seq}", std::process::id()))
}

struct EngineFixture {
    root: PathBuf,
    controller: ProjectController,
    variant_set: VariantSetId,
    adopted: BranchId,
    candidate: BranchId,
    scope: VariantScope,
}

impl EngineFixture {
    /// Live pins for adopted (left) vs candidate (right) plus the
    /// current project revision — the explicit targets every facade
    /// call requires.
    fn pins(&self) -> (BranchPin, BranchPin, ProjectRevision) {
        let set = &self.controller.live().variant_sets[&self.variant_set];
        (
            BranchPin {
                branch: self.adopted,
                revision: set.branches[&self.adopted].content.revision(),
            },
            BranchPin {
                branch: self.candidate,
                revision: set.branches[&self.candidate].content.revision(),
            },
            self.controller.live_revision(),
        )
    }

    fn authority(&self) -> CompareAuthority {
        variant_diff_authority(
            self.controller.live(),
            self.variant_set,
            self.controller.live_revision(),
            &VariantDiffBranchObservations::default(),
        )
        .unwrap()
    }
}

/// Forks a real variant set through `adapter` into the directory store
/// under `root/branch-content` — the same path `VariantDiffEngine`'s
/// standard ports open — then mutes the clip on the candidate so the
/// semantic layer has a confirmed change to project.
fn engine_fixture(
    scope: impl FnOnce(TrackId, ClipId) -> VariantScope,
    adapter: &dyn DomainAdapter,
) -> EngineFixture {
    let root = engine_root();
    let (mut controller, track, _lane, clip, _note) = audio_fixture();
    let mut store = DirectoryBranchContentStore::new(root.join("branch-content"));
    let scope = scope(track, clip);
    let receipt = fork_variant_set(
        &mut controller,
        adapter,
        &mut store,
        &SnapshotResolver::new(),
        scope.clone(),
        &SnapshotCancel::new(),
    )
    .unwrap();
    let candidate_rev = controller.live().variant_sets[&receipt.variant_set].branches
        [&receipt.candidate]
        .content
        .revision();
    edit_branch(
        &mut controller,
        adapter,
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
    .unwrap();
    EngineFixture {
        root,
        controller,
        variant_set: receipt.variant_set,
        adopted: receipt.adopted,
        candidate: receipt.candidate,
        scope,
    }
}

fn audio_engine_fixture() -> EngineFixture {
    engine_fixture(
        |_track, clip| VariantScope::AudioClip { clip },
        &AudioClipAdapter::new(),
    )
}

/// Layers → view input, reusing the facade-built authority/context so
/// the projection sees the same pins the orchestrator evaluated.
fn engine_input(
    fx: &EngineFixture,
    layers: &synvelis_workspace::variant_diff::VariantDiffLayers,
) -> VariantDiffInput {
    let (left, right, rev) = fx.pins();
    VariantDiffInput {
        context: variant_diff_context(fx.controller.live(), fx.variant_set, left, right, rev)
            .unwrap(),
        authority: fx.authority(),
        semantic: layers.semantic.clone(),
        signal: layers.signal.clone(),
        perceptual: layers.perceptual.clone(),
        capabilities: CapabilityEvidence {
            editability: EvidenceState::Available,
            exact_audition: EvidenceState::Unavailable {
                reason: VariantDiffReason::MissingPlugin,
            },
            reasons: vec![VariantDiffReason::MissingPlugin],
        },
    }
}

#[test]
fn cb09_engine_compare_projects_cb08_layers() {
    let fx = audio_engine_fixture();
    let mut engine = VariantDiffEngine::new(Some(&fx.root));
    let (left, right, rev) = fx.pins();
    let layers = engine
        .compare(
            fx.controller.live(),
            fx.variant_set,
            &left,
            &right,
            rev,
            &SessionOptions::default(),
            None,
        )
        .unwrap();

    // Semantic: the real orchestrator diff — mute is a confirmed
    // change, never collapsed into equality.
    assert!(layers.semantic.state.is_available());
    let semantic = layers.semantic.diff.as_ref().expect("semantic diff");
    assert_eq!(semantic.overall, ComparisonEquality::Different);
    assert!(!layers.freshness.is_stale());

    // Signal: no provider configured → honest typed gap, not a
    // zero-difference claim.
    assert!(matches!(
        layers.signal.state,
        EvidenceState::Unavailable {
            reason: VariantDiffReason::EvidenceUnavailable
        }
    ));
    assert!(layers.signal.origin.is_none());
    assert_eq!(layers.signal.rows.len(), 0);

    // Perceptual: headless audition stays an explicit gap.
    assert!(matches!(
        layers.perceptual.state,
        EvidenceState::Unavailable {
            reason: VariantDiffReason::EvidenceUnavailable
        }
    ));

    // The same bundle projects into the view unchanged.
    let input = engine_input(&fx, &layers);
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert_eq!(view.domain, VariantDiffDomain::AudioClip);
    assert!(view.domain_state.is_available());
    assert!(view.semantic.state.is_available());
    assert_eq!(view.semantic.equality, Some(ComparisonEquality::Different));
    assert_ne!(view.semantic.rows, [] as [DiffRow; 0]);
    assert!(matches!(
        view.signal.state,
        EvidenceState::Unavailable { .. }
    ));
    assert_eq!(view.signal.rows.len(), 0);
}

/// Provider seam: Mock stays visibly Mock, Pipeline/Derived report as
/// pipeline provenance, alignment and metric rows carry verbatim.
struct StubSignalProvider {
    report: SignalProviderReport,
}

impl SignalComparisonProvider for StubSignalProvider {
    fn compare_signal(&self, _request: &SignalCompareRequest<'_>) -> SignalProviderReport {
        self.report.clone()
    }
}

#[test]
fn cb09_engine_compare_maps_signal_provider_provenance() {
    let fx = audio_engine_fixture();
    let mut engine = VariantDiffEngine::new(Some(&fx.root));
    let (left, right, rev) = fx.pins();

    for (origin, expected) in [
        (Cb08EvidenceOrigin::Mock, EvidenceOrigin::Mock),
        (Cb08EvidenceOrigin::Pipeline, EvidenceOrigin::Pipeline),
        (Cb08EvidenceOrigin::Derived, EvidenceOrigin::Pipeline),
    ] {
        let provider = StubSignalProvider {
            report: SignalProviderReport {
                state: Cb08LayerState::Available,
                origin,
                alignment: Cb08SignalAlignment::Aligned,
                metrics: vec![SignalMetric {
                    name: "pcm.rms".into(),
                    domain: Cb08SignalDomain::Musical,
                    before: 0.5,
                    after: 0.7,
                    delta: 0.2,
                    unit: "linear".into(),
                }],
                provenance: None,
            },
        };
        let layers = engine
            .compare(
                fx.controller.live(),
                fx.variant_set,
                &left,
                &right,
                rev,
                &SessionOptions::default(),
                Some(&provider),
            )
            .unwrap();
        assert!(layers.signal.state.is_available(), "{origin:?}");
        assert_eq!(layers.signal.origin, Some(expected), "{origin:?}");
        assert_eq!(layers.signal.alignment, AlignmentState::Aligned);
        assert_eq!(layers.signal.rows.len(), 1);
        assert!(layers.signal.rows[0].label.contains("pcm.rms"));
    }
}

/// The merged CB-07 arrangement adapter resolves through the facade's
/// port table — arrangement compare produces real semantic evidence,
/// not the stale `DomainAdapterUnmerged` verdict.
#[test]
fn cb09_engine_compare_resolves_arrangement_adapter() {
    let fx = engine_fixture(
        |track, _clip| VariantScope::Arrangement {
            tracks: vec![track],
            time: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(16 * 960)).unwrap(),
            fields: ArrangementDomainMask {
                clips: true,
                notes: true,
            },
        },
        &ArrangementAdapter::new(),
    );
    let mut engine = VariantDiffEngine::new(Some(&fx.root));
    let (left, right, rev) = fx.pins();
    let layers = engine
        .compare(
            fx.controller.live(),
            fx.variant_set,
            &left,
            &right,
            rev,
            &SessionOptions::default(),
            None,
        )
        .unwrap();
    assert!(layers.semantic.state.is_available());

    let input = engine_input(&fx, &layers);
    let view = project_variant_diff(&input, &VariantDiffViewState::default());
    assert_eq!(view.domain, VariantDiffDomain::Arrangement);
    assert!(view.domain_state.is_available());
    assert!(view.semantic.state.is_available());
    assert_eq!(view.semantic.equality, Some(ComparisonEquality::Different));
    assert_ne!(view.semantic.rows, [] as [DiffRow; 0]);
}

#[test]
fn cb09_engine_authority_and_context_reflect_live_state() {
    let fx = audio_engine_fixture();
    let authority = fx.authority();
    let set = &fx.controller.live().variant_sets[&fx.variant_set];
    assert_eq!(authority.variant_set, fx.variant_set);
    assert_eq!(authority.scope, set.scope);
    assert_eq!(authority.adopted, set.adopted);
    assert_eq!(authority.project_revision, fx.controller.live_revision());
    assert_eq!(
        authority.scope_digest,
        scope_digest(fx.controller.live(), &fx.scope).unwrap()
    );
    for (id, branch) in &set.branches {
        assert_eq!(authority.branches[id].revision, branch.content.revision());
        assert!(!authority.branches[id].editing);
        assert_eq!(
            authority.branches[id].publish,
            RuntimePublishState::NotRequested
        );
    }

    let (left, right, rev) = fx.pins();
    let context = variant_diff_context(
        fx.controller.live(),
        fx.variant_set,
        left.clone(),
        right.clone(),
        rev,
    )
    .unwrap();
    assert_eq!(context.left, left);
    assert_eq!(context.right, right);
    assert_eq!(context.scope_digest, authority.scope_digest);

    // Unknown sets fail closed — never a fabricated authority.
    assert!(
        variant_diff_authority(
            fx.controller.live(),
            VariantSetId::new(999),
            rev,
            &VariantDiffBranchObservations::default(),
        )
        .is_err()
    );
}

#[test]
fn cb09_engine_compare_rejects_stale_and_unknown_pins() {
    let fx = audio_engine_fixture();
    let mut engine = VariantDiffEngine::new(Some(&fx.root));
    let (left, right, rev) = fx.pins();

    // Stale asserted pin revision → typed CAS reject before any
    // session opens.
    let stale_right = BranchPin {
        branch: right.branch,
        revision: BranchRevision::new(right.revision.raw() + 1),
    };
    assert!(matches!(
        engine.compare(
            fx.controller.live(),
            fx.variant_set,
            &left,
            &stale_right,
            rev,
            &SessionOptions::default(),
            None,
        ),
        Err(CompareError::StaleRevision { .. })
    ));

    // Unknown branch → typed reject.
    let ghost = BranchPin {
        branch: BranchId::new(9999),
        revision: BranchRevision::new(1),
    };
    assert!(matches!(
        engine.compare(
            fx.controller.live(),
            fx.variant_set,
            &left,
            &ghost,
            rev,
            &SessionOptions::default(),
            None,
        ),
        Err(CompareError::UnknownBranch { .. })
    ));
}

#[test]
fn cb09_engine_adopt_preview_and_commit_keep_cas() {
    let mut fx = audio_engine_fixture();
    let engine = VariantDiffEngine::new(Some(&fx.root));
    let authority = fx.authority();
    let candidate_rev = authority.branches[&fx.candidate].revision;
    let target = BranchPin {
        branch: fx.candidate,
        revision: candidate_rev,
    };

    // Preview: the simulated post-state adopts the candidate; live is
    // untouched.
    let preview_intent = VariantDiffIntent::AdoptPreview {
        target: target.clone(),
        scope_digest: authority.scope_digest,
    };
    let preview = engine
        .adopt_preview(fx.controller.live(), &authority, &preview_intent)
        .unwrap();
    assert_eq!(preview.variant_sets[&fx.variant_set].adopted, fx.candidate);
    assert_ne!(
        fx.controller.live().variant_sets[&fx.variant_set].adopted,
        fx.candidate
    );

    // Commit: the same CAS guards run through the gateway execute
    // path; live flips to the candidate only now.
    let adopt_intent = VariantDiffIntent::Adopt {
        target,
        scope_digest: authority.scope_digest,
    };
    engine
        .adopt(
            &mut fx.controller,
            &authority,
            &adopt_intent,
            "cb09-approval",
        )
        .unwrap();
    assert_eq!(
        fx.controller.live().variant_sets[&fx.variant_set].adopted,
        fx.candidate
    );
}

#[test]
fn cb09_engine_adopt_rejects_typed() {
    let mut fx = audio_engine_fixture();
    let engine = VariantDiffEngine::new(Some(&fx.root));
    let authority = fx.authority();
    let candidate_rev = authority.branches[&fx.candidate].revision;
    let good = BranchPin {
        branch: fx.candidate,
        revision: candidate_rev,
    };

    // Stale asserted branch revision.
    let stale = VariantDiffIntent::Adopt {
        target: BranchPin {
            branch: fx.candidate,
            revision: BranchRevision::new(candidate_rev.raw() + 1),
        },
        scope_digest: authority.scope_digest,
    };
    assert!(matches!(
        engine.adopt(&mut fx.controller, &authority, &stale, "t"),
        Err(VariantDiffDispatchError::Reject(
            VariantDiffReject::StaleRevision
        ))
    ));

    // Unknown branch.
    let unknown = VariantDiffIntent::Adopt {
        target: BranchPin {
            branch: BranchId::new(9999),
            revision: BranchRevision::new(1),
        },
        scope_digest: authority.scope_digest,
    };
    assert!(matches!(
        engine.adopt(&mut fx.controller, &authority, &unknown, "t"),
        Err(VariantDiffDispatchError::Reject(
            VariantDiffReject::UnknownBranch
        ))
    ));

    // Scope fingerprint drift.
    let drifted = VariantDiffIntent::Adopt {
        target: good.clone(),
        scope_digest: ContentDigest::sha256(b"cb09-wrong-scope"),
    };
    assert!(matches!(
        engine.adopt(&mut fx.controller, &authority, &drifted, "t"),
        Err(VariantDiffDispatchError::Reject(
            VariantDiffReject::StaleContext
        ))
    ));

    // Already-adopted target is a typed no-op reject, never a commit.
    let adopted_pin = BranchPin {
        branch: fx.adopted,
        revision: authority.branches[&fx.adopted].revision,
    };
    let noop = VariantDiffIntent::Adopt {
        target: adopted_pin,
        scope_digest: authority.scope_digest,
    };
    assert!(matches!(
        engine.adopt(&mut fx.controller, &authority, &noop, "t"),
        Err(VariantDiffDispatchError::Reject(
            VariantDiffReject::AlreadyAdopted
        ))
    ));

    // A preview intent must never reach the commit path, and an adopt
    // intent must never be silently downgraded to a preview.
    let preview = VariantDiffIntent::AdoptPreview {
        target: good.clone(),
        scope_digest: authority.scope_digest,
    };
    assert!(matches!(
        engine.adopt(&mut fx.controller, &authority, &preview, "t"),
        Err(VariantDiffDispatchError::Reject(
            VariantDiffReject::Malformed
        ))
    ));
    let commit = VariantDiffIntent::Adopt {
        target: good,
        scope_digest: authority.scope_digest,
    };
    assert!(matches!(
        engine.adopt_preview(fx.controller.live(), &authority, &commit),
        Err(VariantDiffDispatchError::Reject(
            VariantDiffReject::Malformed
        ))
    ));
}
