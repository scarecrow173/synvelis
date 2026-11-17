//! CB-06 residual slice (synvelis#283) — the gateway commit-gate
//! exercise for the `strip-fields` variant scope.
//!
//! `StripFieldsAdapter` and its `VariantPorts::standard` registration
//! landed earlier (`synvelis#283` PR #447 / CB-10 PR #483); what this
//! suite pins is the *wire path*: a `strip-fields` scope driven through
//! the same `prepare → proposal → approval → apply → CommitGate`
//! pipeline every other project mutation rides.
//!
//! What is proven here:
//!
//! - **Fork** — `synvelis.branch.fork` over a `strip-fields` scope
//!   resolves the registered adapter, captures the candidate payload
//!   through the shared content store, and commits as one CAS with
//!   domain provenance.
//! - **Typed edit boundary** — every ordinary op the gateway table can
//!   lower (`synvelis.clip.*`, `synvelis.track.*`, …) is *not*
//!   strip-domain surface: each candidate edit fails
//!   closed `PreparationRejected`/`variant-unsupported` before any
//!   mutation. No wire capability lowers to `SetStripMute`,
//!   `SetParameterBase`, `InsertProcessor` or any other mask-gated
//!   strip op in this slice, so the honest boundary evidence is the
//!   uniform typed rejection — never a silent accept or a guessed
//!   mapping. Routing/send/sidechain/bus ops have no capability at
//!   all; external wiring stays shared live context by construction
//!   (CB-STRIP-03).
//! - **Adopt CAS + approval + scope-digest pins** — `adopt` consumes
//!   the approval receipt as the domain ticket, re-verifies the live
//!   scope fingerprint against `expected_scope_digest`, and commits
//!   `Applied` (one undo step) even when the candidate is
//!   scope-identical: the adopted-pointer swap and frozen previous
//!   branch are real project truth. A stale `expected_scope_digest`
//!   or `expected_branch_revision` fails `variant-stale-scope` /
//!   `variant-stale-branch-revision` at prepare simulation — the pin
//!   is verified, not advisory.
//! - **Queries** — `synvelis.branch.{list,describe,compare}` decode
//!   the strip scope and its candidate payloads through the typed
//!   contract: `compare` recomputes the captured payload's scope
//!   fingerprint through the shared store (`verified: true`) and
//!   reports `scope_digests_equal` honestly.
//! - **Domain boundary evidence** — `boundary_dependencies` reports
//!   exactly the strip→master `Main` connection as scope-affiliated
//!   boundary (never a scope object), and `capabilities` reports the
//!   declared mask truthfully (`Supported` per declared field,
//!   `Unsupported` for routing/entity-remove/preset-select).
//!
//! Real PCM/RT/plugin lanes stay `NotRun` — they belong to external
//! gates, not this headless slice. See `evidence/sv283/`.

mod rig;

use std::collections::BTreeSet;
use std::path::Path;

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::*;
use synvelis_project::routing::{ConnectionKind, ProcessorBinding};
use synvelis_project::snapshot::ContentDigest;
use synvelis_project::variant::{
    DomainAdapter, ProcessingCapability, StripDomainMask, StripField, StripFieldsAdapter,
    VariantScope, scope_digest,
};
use synvelis_project::{
    BranchId, ClipId, Command, CounterIdGenerator, DirectoryBundleStore, Project,
    ProjectController, ProjectOp, ProjectRevision, ProjectStore, StripId, TrackId, VariantSetId,
};

// ── Strip fixture ────────────────────────────────────────────

/// The CB-06 fixture: `demo_project()` plus a track-owned strip with
/// one unresolved insert. `AddTrackStrip` mints the strip, its chain,
/// main ports, seeded `Fader`/`Mute`/`Solo` channel targets and the
/// deterministic main route into the master input; `InsertProcessor`
/// gives the `Inserts` mask real owned objects (processor + ports).
/// Two controller ops → the saved head sits at revision 2.
fn strip_controller() -> ProjectController {
    let mut c = ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(9_000)));
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op: ProjectOp::AddTrackStrip {
            track_id: TrackId::new(3),
        },
    })
    .unwrap();
    let strip = c
        .live()
        .routing
        .strip_of_track(TrackId::new(3))
        .expect("track strip exists")
        .id;
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op: ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".to_owned(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "plug-demo".to_owned(),
            },
            sidechain_input: false,
        },
    })
    .unwrap();
    c
}

/// The strip-containing project at the fixture's live revision (2).
fn strip_project() -> Project {
    strip_controller().live().clone()
}

/// The track strip's stable id — discovered, never hard-coded.
fn strip_id() -> StripId {
    strip_project()
        .routing
        .strip_of_track(TrackId::new(3))
        .expect("track strip exists")
        .id
}

/// `strip-fields` scope over the fixture strip, all five field
/// families declared — the widest honest mask.
fn strip_scope() -> Value {
    json!({
        "kind": "strip-fields",
        "strip": strip_id().to_tagged(),
        "fields": ["fader", "pan", "mute", "solo", "inserts"],
    })
}

/// The typed domain scope matching [`strip_scope`] — for the
/// domain-level boundary/capability assertions.
fn strip_domain_scope() -> VariantScope {
    VariantScope::StripFields {
        strip: strip_id(),
        fields: StripDomainMask(BTreeSet::from([
            StripField::Fader,
            StripField::Pan,
            StripField::Mute,
            StripField::Solo,
            StripField::Inserts,
        ])),
    }
}

/// [`seed_store`] for the strip fixture — saves the strip project at
/// its own head revision instead of `demo_project()`.
fn seed_strip_store(dir: &Path) -> DirectoryBundleStore {
    let mut store = DirectoryBundleStore::create(dir).unwrap();
    if store.head().unwrap().is_none() {
        let mut c = strip_controller();
        c.save(&mut store).unwrap();
    }
    store
}

// ── Request builders (cb10/cb11 conventions) ─────────────────

fn fork_op(scope: &Value) -> Value {
    op(
        "synvelis.branch.fork",
        create_target("project", &project_ref(), "variant-set"),
        json!({"scope": scope}),
    )
}

fn set_target(set: &VariantSetId) -> Value {
    json!({"shape": "resource", "kind": "variant-set", "id": set.to_tagged()})
}

fn edit_op(set: &VariantSetId, branch: &BranchId, revision: u64, inner: &Value) -> Value {
    op(
        "synvelis.branch.edit",
        set_target(set),
        json!({
            "branch": branch.to_tagged(),
            "expected_branch_revision": revision,
            "op": inner,
        }),
    )
}

fn adopt_op(
    set: &VariantSetId,
    branch: &BranchId,
    project_rev: u64,
    branch_rev: u64,
    digest: &ContentDigest,
) -> Value {
    op(
        "synvelis.branch.adopt",
        set_target(set),
        json!({
            "branch": branch.to_tagged(),
            "expected_project_revision": project_rev,
            "expected_branch_revision": branch_rev,
            "expected_scope_digest": digest.to_string(),
        }),
    )
}

fn discard_op(set: &VariantSetId, branch: &BranchId) -> Value {
    op(
        "synvelis.branch.discard",
        set_target(set),
        json!({"branch": branch.to_tagged()}),
    )
}

/// Representative ordinary ops — none is strip-domain surface, so
/// every one must fail the candidate confinement typed.
fn clip_mute_op() -> Value {
    op(
        "synvelis.clip.mute",
        json!({"shape": "resource", "kind": "clip", "id": ClipId::new(11).to_tagged()}),
        json!({"muted": true}),
    )
}

fn track_create_op() -> Value {
    op(
        "synvelis.track.create",
        create_target("project", &project_ref(), "track"),
        json!({"name": "extra"}),
    )
}

fn clip_move_op() -> Value {
    op(
        "synvelis.clip.move",
        json!({"shape": "resource", "kind": "clip", "id": ClipId::new(11).to_tagged()}),
        json!({"anchor": {"kind": "Musical", "value": 600}}),
    )
}

fn query_req(capability: &str, args: &Value, target: Option<Value>) -> Value {
    let mut req = json!({
        "request_id": "req-cb06-query",
        "capability": {"id": capability, "version": {"major": 1, "minor": 0}},
        "schema": {"name": format!("{capability}.input"), "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "args": args,
        "consistency": {"kind": "latest"},
    });
    if let Some(t) = target {
        req["target"] = t;
    }
    req
}

// ── Grants (cb10/cb11 `branch_grant` convention) ─────────────

/// Dynamic membership over every resource kind a strip-fields branch
/// mutation can touch — scope roots (`Strip`), scoped entities
/// (`Processor`, `ParameterTarget`, `AutomationLane`, …) and the
/// variant bookkeeping kinds.
fn branch_scope() -> ResourceScope {
    ResourceScope::Dynamic(DynamicScope {
        owners: OwnerConstraint::Any,
        kinds: BTreeSet::from([
            ResourceKind::Project,
            ResourceKind::VariantSet,
            ResourceKind::Branch,
            ResourceKind::Clip,
            ResourceKind::Track,
            ResourceKind::Lane,
            ResourceKind::Source,
            ResourceKind::Asset,
            ResourceKind::Note,
            ResourceKind::TakeGroup,
            ResourceKind::Comp,
            ResourceKind::ParameterTarget,
            ResourceKind::AutomationLane,
            ResourceKind::Strip,
            ResourceKind::Send,
            ResourceKind::Processor,
        ]),
        range: None,
        max_members: None,
        include_future: true,
        move_args: None,
    })
}

fn branch_grant(tag: &str, task: &str, caps: &[&str], policy: ApprovalPolicy) -> Grant {
    let mut grant = prep_grant(
        tag,
        task,
        caps,
        branch_scope(),
        Some(CreateWithin {
            parents: OwnerConstraint::Any,
            kinds: BTreeSet::from([ResourceKind::VariantSet, ResourceKind::Branch]),
            placement: None,
        }),
        &content_classes(),
    );
    let mut capabilities = grant.capabilities.clone();
    capabilities.insert(cap_ref(PREVIEW_ID));
    capabilities.insert(cap_ref(APPLY_ID));
    grant.capabilities = capabilities;
    grant.permissions =
        PermissionSet::of([PermissionKind::MutateProject, PermissionKind::ReadContext]);
    grant.approval_policy = policy;
    grant
}

// ── Flow helpers ─────────────────────────────────────────────

fn apply_human(rig: &mut Rig, proposal: &str, digest: &str, rev: u64) -> OperationId {
    let mut req = apply_req(proposal, digest, None, None, None);
    req["consistency"] = expected(rev);
    let outcome = rig.gw.handle(&req, human()).unwrap();
    let GatewayOutcome::Accepted { operation } = outcome else {
        panic!("expected Accepted, got {outcome:?}");
    };
    operation
}

fn apply_agent(
    rig: &mut Rig,
    proposal: &str,
    digest: &str,
    approval: Option<&str>,
    task: &str,
    grant: &str,
    rev: u64,
) -> GatewayOutcome<Value> {
    let mut req = apply_req(proposal, digest, approval, Some(task), Some(grant));
    req["consistency"] = expected(rev);
    rig.gw.handle(&req, agent()).unwrap()
}

/// The single variant set on the gate's live project.
fn only_set(gate: &CommitGate) -> (VariantSetId, BranchId, BranchId) {
    gate.inspect(|c| {
        let (set_id, set) = c
            .live()
            .variant_sets
            .iter()
            .next()
            .expect("one variant set");
        let adopted = set.adopted;
        let candidate = *set
            .branches
            .keys()
            .find(|b| **b != adopted)
            .expect("one candidate branch");
        (*set_id, adopted, candidate)
    })
}

fn branch_revision(gate: &CommitGate, set: &VariantSetId, branch: &BranchId) -> u64 {
    gate.inspect(|c| {
        c.live()
            .variant_sets
            .get(set)
            .and_then(|s| s.branches.get(branch))
            .map(|b| b.content.revision().raw())
            .expect("branch exists")
    })
}

fn live_rev(gate: &CommitGate) -> u64 {
    gate.live_revision().0
}

/// The live scope fingerprint under the gate — the value
/// `expected_scope_digest` pins.
fn live_scope_digest(gate: &CommitGate, set: &VariantSetId) -> ContentDigest {
    gate.inspect(|c| scope_digest(c.live(), &c.live().variant_sets[set].scope).unwrap())
}

fn publish(gate: &CommitGate, rig: &Rig) {
    let live = gate.inspect(|c| c.live().clone());
    rig.source.commit(project(), gate.live_revision(), live);
}

/// Boots a variant-wired gate over the strip fixture — the bundle
/// store holds the strip project at revision 2 and the rig's snapshot
/// source pins the same state, so `expected(2)` commits land.
fn boot_strip(vrig: &VariantRig, dir: &Path) -> CommitGate {
    let store_dir = dir.join("project");
    let mut store = seed_strip_store(&store_dir);
    let (gate, _report) = CommitGate::open(
        strip_project(),
        Box::new(CounterIdGenerator::new(9_000)),
        &mut store,
        dir.join("commit.journal"),
        gate_deps_variant(
            vrig.rig.store.clone(),
            vrig.rig.clock.clone(),
            vrig.rig.source.clone(),
            vrig.rig.proposals.clone(),
            apply_desc(&vrig.rig),
            vrig.ports.clone(),
        ),
        MutexFence::new(),
    )
    .expect("gate opens");
    gate
}

/// prepare (human) → apply (human) → commit. Returns the receipt.
fn commit_human(
    vrig: &mut VariantRig,
    gate: &CommitGate,
    operation: Value,
    rev: u64,
) -> CommitReceipt {
    let mut req = prepare_req(None, None, operation, expected(rev));
    req["request_id"] = json!(format!("req-cb06-{}", rev));
    let result = complete(vrig.rig.gw.handle(&req, human()).unwrap());
    let proposal = result["proposal"].as_str().unwrap().to_owned();
    let digest = result["digest"].as_str().unwrap().to_owned();
    apply_human(&mut vrig.rig, &proposal, &digest, rev);
    let pref = ProposalRef::new(&proposal).unwrap();
    gate.commit(&pref).expect("variant commit must land")
}

/// Asserts a candidate edit prepare fails `PreparationRejected` with
/// the typed `variant-*` reason — nothing is retained or mutated.
fn expect_edit_rejected(
    vrig: &mut VariantRig,
    gate: &CommitGate,
    set: &VariantSetId,
    branch: &BranchId,
    inner: &Value,
    needle: &str,
) {
    let rev = live_rev(gate);
    let brev = branch_revision(gate, set, branch);
    let err = vrig
        .rig
        .gw
        .handle(
            &prepare_req(None, None, edit_op(set, branch, brev, inner), expected(rev)),
            human(),
        )
        .expect_err("out-of-scope candidate edit must reject");
    match err.code {
        GatewayError::PreparationRejected { reason } => {
            assert!(
                reason.contains(needle),
                "expected {needle:?} in rejection, got {reason:?}"
            );
        }
        other => panic!("expected PreparationRejected, got {other:?}"),
    }
}

// ── Commit-gate lifecycle ────────────────────────────────────

#[test]
#[allow(clippy::too_many_lines)] // sequential lifecycle assertions read top-to-bottom (cb10 convention)
fn cb06_strip_fields_gateway_commit_gate_lifecycle() {
    let mut vrig = variant_rig();
    // The rig seeds `demo_project()` at revision 1; the strip fixture
    // is the same project plus `AddTrackStrip` + `InsertProcessor`,
    // whose controller head sits at revision 2 — pin it explicitly so
    // prepare/query see the strip state the gate commits against.
    vrig.rig
        .source
        .commit(project(), ProjectRevision(2), strip_project());
    let dir = tmpdir("cb06-strip-gateway");
    let gate = boot_strip(&vrig, &dir);
    assert_eq!(live_rev(&gate), 2, "strip fixture head is revision 2");

    // ── Fork: strip-fields scope through the commit gate ──────
    let receipt = commit_human(&mut vrig, &gate, fork_op(&strip_scope()), 2);
    assert_eq!(receipt.revision, ProjectRevision(3));
    publish(&gate, &vrig.rig);
    let (set, adopted, candidate) = only_set(&gate);
    assert_ne!(adopted, candidate);
    let prov: Value = serde_json::from_str(
        &receipt
            .provenance
            .expect("variant commit carries provenance"),
    )
    .unwrap();
    assert_eq!(prov["variant_set"], set.to_tagged());
    assert_eq!(prov["adopted"], adopted.to_tagged());
    assert_eq!(prov["candidate"], candidate.to_tagged());
    // The set's scope round-trips the declared strip + mask.
    gate.inspect(|c| {
        let set = &c.live().variant_sets[&set];
        assert_eq!(
            set.scope,
            VariantScope::StripFields {
                strip: strip_id(),
                fields: StripDomainMask(BTreeSet::from([
                    StripField::Fader,
                    StripField::Pan,
                    StripField::Mute,
                    StripField::Solo,
                    StripField::Inserts,
                ])),
            }
        );
    });

    // ── Queries on the pinned post-fork state ─────────────────
    let list = complete(
        vrig.rig
            .gw
            .handle(
                &query_req("synvelis.branch.list", &json!({}), None),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(list["variant_sets"].as_array().unwrap().len(), 1);
    assert_eq!(list["variant_sets"][0]["scope_kind"], "strip-fields");
    // The scope-kind filter is honest: strip-fields matches, others don't.
    let filtered = complete(
        vrig.rig
            .gw
            .handle(
                &query_req(
                    "synvelis.branch.list",
                    &json!({"scope_kind": "strip-fields"}),
                    None,
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(filtered["variant_sets"].as_array().unwrap().len(), 1);
    let other_kind = complete(
        vrig.rig
            .gw
            .handle(
                &query_req(
                    "synvelis.branch.list",
                    &json!({"scope_kind": "audio-clip"}),
                    None,
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(other_kind["variant_sets"].as_array().unwrap().len(), 0);

    let describe = complete(
        vrig.rig
            .gw
            .handle(
                &query_req(
                    "synvelis.branch.describe",
                    &json!({}),
                    Some(set_target(&set)),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(describe["scope_kind"], "strip-fields");
    assert_eq!(describe["scope"]["kind"], "strip-fields");
    assert_eq!(describe["scope"]["strip"], strip_id().to_tagged());
    assert_eq!(describe["adopted"], adopted.to_tagged());

    // Identical scoped content → honest digest equality, both sides
    // verified (the candidate payload is decoded + re-fingerprinted
    // through the shared store, never trusted from its claim).
    let digest_before = live_scope_digest(&gate, &set);
    let compare = complete(
        vrig.rig
            .gw
            .handle(
                &query_req(
                    "synvelis.branch.compare",
                    &json!({
                        "branch_a": adopted.to_tagged(),
                        "branch_b": candidate.to_tagged(),
                    }),
                    Some(set_target(&set)),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(compare["scope_digests_equal"], json!(true));
    assert_eq!(compare["branch_b"]["content"]["verified"], json!(true));
    assert_eq!(
        compare["branch_a"]["content"]["verified_scope_digest"].as_str(),
        Some(digest_before.to_string().as_str())
    );

    // ── Candidate edit boundary: every wire op rejects typed ──
    // The gateway table carries no strip-domain capability, so the
    // mask-gated surface (`SetStripMute`, `SetParameterBase`,
    // `InsertProcessor`, …) is unreachable — and every ordinary op
    // the table *can* lower fails the strip candidate confinement
    // `variant-unsupported` before any mutation. Routing/send/bus
    // ops have no capability at all: external wiring is never
    // candidate-owned (CB-STRIP-03).
    for inner in [clip_mute_op(), track_create_op(), clip_move_op()] {
        expect_edit_rejected(
            &mut vrig,
            &gate,
            &set,
            &candidate,
            &inner,
            "variant-unsupported",
        );
    }
    // Rejections consumed nothing: live is untouched and the branch
    // content revision did not advance.
    assert_eq!(live_rev(&gate), 3);
    assert_eq!(branch_revision(&gate, &set, &candidate), 1);
    assert!(!gate.inspect(|c| c.live().clips[&ClipId::new(11)].muted));

    // ── Adopt CAS pins: stale digest / branch revision reject ──
    let rev = live_rev(&gate);
    let brev = branch_revision(&gate, &set, &candidate);
    let bad_digest = ContentDigest::sha256(b"cb06-not-the-live-scope");
    let err = vrig
        .rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                adopt_op(&set, &candidate, rev, brev, &bad_digest),
                expected(rev),
            ),
            human(),
        )
        .expect_err("stale scope digest must reject");
    match err.code {
        GatewayError::PreparationRejected { reason } => {
            assert!(
                reason.contains("variant-stale-scope"),
                "expected variant-stale-scope, got {reason:?}"
            );
        }
        other => panic!("expected PreparationRejected, got {other:?}"),
    }
    let err = vrig
        .rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                adopt_op(&set, &candidate, rev, brev + 1, &digest_before),
                expected(rev),
            ),
            human(),
        )
        .expect_err("stale branch revision must reject");
    match err.code {
        GatewayError::PreparationRejected { reason } => {
            assert!(
                reason.contains("variant-stale-branch-revision"),
                "expected variant-stale-branch-revision, got {reason:?}"
            );
        }
        other => panic!("expected PreparationRejected, got {other:?}"),
    }

    // ── Adopt through the granted-agent approval path ─────────
    // The domain contract requires an explicit approval ticket the
    // gateway cannot mint — ApproveToApply consumes the human
    // receipt and its id becomes the adopt ticket (cb10 convention).
    issue_agent(
        &vrig.rig.store,
        "t1",
        branch_grant(
            "g1",
            "t1",
            &["synvelis.branch.adopt"],
            ApprovalPolicy::ApproveToApply,
        ),
    );
    let result = complete(
        vrig.rig
            .gw
            .handle(
                &prepare_req(
                    Some("t1"),
                    Some("g1"),
                    adopt_op(&set, &candidate, rev, brev, &digest_before),
                    expected(rev),
                ),
                agent(),
            )
            .unwrap(),
    );
    let proposal = result["proposal"].as_str().unwrap().to_owned();
    let digest_s = result["digest"].as_str().unwrap().to_owned();
    let outcome = apply_agent(&mut vrig.rig, &proposal, &digest_s, None, "t1", "g1", rev);
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    complete(
        vrig.rig
            .gw
            .handle(&approve_req(&proposal, &digest_s), human())
            .unwrap(),
    );
    let outcome = apply_agent(
        &mut vrig.rig,
        &proposal,
        &digest_s,
        Some(approval.as_str()),
        "t1",
        "g1",
        rev,
    );
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));
    let receipt = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect("adopt commits");
    publish(&gate, &vrig.rig);

    // The candidate is now adopted; the scope-identical content still
    // commits `Applied` — the adopted-pointer swap and the frozen
    // previous branch are real project truth (one undo step).
    let (_set, new_adopted, frozen) = only_set(&gate);
    assert_eq!(new_adopted, candidate);
    assert_eq!(frozen, adopted);
    let prov: Value =
        serde_json::from_str(&receipt.provenance.expect("adopt commit carries provenance"))
            .unwrap();
    assert_eq!(prov["branch"], candidate.to_tagged());
    assert_eq!(prov["outcome"], "applied");
    assert_eq!(prov["objects"], json!(0));
    assert_eq!(prov["project"]["committed"], json!(true));
    assert_eq!(prov["project"]["undo_steps"], json!(1));
    assert_eq!(prov["runtime"]["state"]["state"], "not-requested");
    // Scoped content is identical → the live scope fingerprint did
    // not move across the adopt.
    assert_eq!(live_scope_digest(&gate, &set), digest_before);
    // Live truth is unchanged: the strip, its chain and the seeded
    // channel targets are exactly what the fixture committed.
    gate.inspect(|c| {
        let p = c.live();
        assert_eq!(p.routing.strips.len(), 2, "master + track strip");
        assert!(p.routing.strips.contains_key(&strip_id()));
        assert!(!p.clips[&ClipId::new(11)].muted);
    });

    // ── Post-adopt compare: frozen Captured vs Live candidate ──
    let compare = complete(
        vrig.rig
            .gw
            .handle(
                &query_req(
                    "synvelis.branch.compare",
                    &json!({
                        "branch_a": adopted.to_tagged(),
                        "branch_b": candidate.to_tagged(),
                    }),
                    Some(set_target(&set)),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(compare["scope_digests_equal"], json!(true));
    assert_eq!(compare["branch_a"]["content"]["kind"], "captured");
    assert_eq!(compare["branch_a"]["content"]["verified"], json!(true));
    assert_eq!(compare["branch_b"]["content"]["kind"], "live");

    // ── Discard the frozen previous branch ────────────────────
    commit_human(&mut vrig, &gate, discard_op(&set, &frozen), live_rev(&gate));
    publish(&gate, &vrig.rig);
    gate.inspect(|c| {
        let set = &c.live().variant_sets[&set];
        assert!(!set.branches.contains_key(&frozen));
        assert_eq!(set.branches.len(), 1);
        assert_eq!(set.adopted, candidate);
    });
}

// ── Domain boundary + capability evidence ────────────────────

#[test]
fn cb06_strip_fields_scope_boundary_and_capabilities() {
    let project = strip_project();
    let scope = strip_domain_scope();
    let adapter = StripFieldsAdapter::new();

    // Scope validation resolves through the registered adapter path.
    adapter.validate_scope(&project, &scope).unwrap();

    // The routing boundary is exactly the strip→master `Main`
    // connection: strip-affiliated on the source side only, never a
    // scope object — adopt cannot touch it and candidate edits cannot
    // rewire it (CB-STRIP-03).
    let deps = StripFieldsAdapter::boundary_dependencies(&project, &scope).unwrap();
    assert_eq!(deps.len(), 1, "only the main route crosses the boundary");
    assert!(matches!(deps[0].kind, ConnectionKind::Main));
    assert!(deps[0].source_in_scope);
    assert!(!deps[0].destination_in_scope);

    // The capability report is mask-relative truth: every declared
    // field family is Supported; routing, entity removal and preset
    // selection stay Unsupported — display truth equals enforcement
    // truth (CB-STRIP-02).
    let VariantScope::StripFields { fields, .. } = &scope else {
        unreachable!()
    };
    let caps = adapter.capabilities(fields);
    for cap in [
        caps.fader,
        caps.pan,
        caps.mute,
        caps.solo,
        caps.inserts,
        caps.automation_edit,
        caps.modulation_edit,
    ] {
        assert_eq!(cap, ProcessingCapability::Supported);
    }
    for cap in [caps.entity_remove, caps.routing, caps.preset_select] {
        assert_eq!(cap, ProcessingCapability::Unsupported);
    }
    assert_eq!(caps.plugin_rebind, ProcessingCapability::Supported);

    // The scope digest is a stable fingerprint of the declared
    // ownership set — identical projects digest identically, and the
    // digest the gateway pins at adopt is this same function.
    let d1 = scope_digest(&project, &scope).unwrap();
    let d2 = scope_digest(&strip_project(), &scope).unwrap();
    assert_eq!(d1, d2);
}
