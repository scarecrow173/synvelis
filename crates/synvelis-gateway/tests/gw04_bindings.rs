//! AUTX-04 GW-04 bindings seam tests — synvelis#263.
//!
//! A `synvelis.operation.batch` carrying a non-empty `bindings`
//! array is the create-then-reference wire form the atomic-macro
//! compiler emits (`docs/automation/design-atomic-macro.md` §3):
//! members reference `$output.<name>` through deterministic
//! placeholder ids, and the prepare engine resolves them against a
//! scratch apply seeded at the pinned base's `id_watermark` — the
//! same id series `ProjectOp::Composite` draws at commit.
//!
//! The contract under test: placeholders rewrite to real minted ids
//! (never guessed), one flattened `Composite` is retained, the whole
//! batch stays a single proposal / single commit / single undo, and
//! every malformed, premature, ambiguous, or unresolvable binding
//! fails closed with the typed taxonomy.

mod rig;

use std::collections::BTreeSet;

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::*;
use synvelis_project::{
    ClipId, Command, CounterIdGenerator, LaneId, ProjectController, ProjectOp, ProjectRevision,
    TrackId,
};

/// First raw id of the placeholder range — mirrors
/// `synvelis_automation::macros::OUTPUT_BINDING_ID_BASE` (the
/// automation crate is a dev-dependency; the wire constant is shared
/// verbatim, same as `prepare_engine.rs`).
const OUTPUT_BINDING_ID_BASE: u128 = (1u128 << 96) + (1u128 << 32);

/// The compiler placeholder for binding `slot` of `kind` —
/// `<kind>-<32hex>` over `BASE + slot`.
fn placeholder(kind: &str, slot: usize) -> String {
    format!("{kind}-{:032x}", OUTPUT_BINDING_ID_BASE + slot as u128)
}

/// A named-operation member triple (`capability`/`target`/`args`).
fn member(capability: &str, target: Value, args: &Value) -> Value {
    let mut v = json!({
        "capability": {"id": capability, "version": {"major": 1, "minor": 0}},
        "args": args,
    });
    if !target.is_null() {
        v["target"] = target;
    }
    v
}

/// `synvelis.track.create` under the demo project — mints one track.
fn track_create() -> Value {
    member(
        "synvelis.track.create",
        create_target("project", &project_ref(), "track"),
        &json!({"name": "t-new"}),
    )
}

/// `synvelis.lane.create` whose create-parent is the tagged id
/// `parent` (a real id or a compiler placeholder).
fn lane_create(parent: &str) -> Value {
    member(
        "synvelis.lane.create",
        json!({
            "shape": "create",
            "parent_kind": "track",
            "parent": parent,
            "creates": "lane",
        }),
        &json!({}),
    )
}

/// `synvelis.clip.add` on lane `parent` replaying midi source10 —
/// the same wire args `prepare_regressions` uses.
fn clip_add(parent: &str) -> Value {
    member(
        "synvelis.clip.add",
        json!({
            "shape": "create",
            "parent_kind": "lane",
            "parent": parent,
            "creates": "clip",
        }),
        &json!({
            "source": tagged(&source10()),
            "placement": {"anchor": {"kind": "Musical", "value": 0}, "extent": {"kind": "Musical", "value": 240}},
            "playback": {"source_region": {"start": 0, "end": 480}, "mapping": {"kind": "Natural"}},
        }),
    )
}

/// The batch operation carrying `ops` + `bindings`.
fn bound_batch(ops: &[Value], bindings: &[Value]) -> Value {
    op(
        "synvelis.operation.batch",
        Value::Null,
        json!({"ops": ops, "bindings": bindings}),
    )
}

/// The agent grant for create-then-reference: dynamic scope covering
/// the project and every creatable kind (`created_revision` is
/// unrecorded, so `include_future` is required — AHG-008), a create
/// axis naming the permitted parents, and the full GW-05 capability
/// floor so the same grant carries prepare → apply.
fn bindings_grant(
    tag: &str,
    task_tag: &str,
    ops: &[&str],
    parents: BTreeSet<ResourceRef>,
    kinds: &[ResourceKind],
) -> Grant {
    let mut grant = prep_grant(
        tag,
        task_tag,
        ops,
        ResourceScope::Dynamic(DynamicScope {
            owners: OwnerConstraint::Any,
            kinds: kinds.iter().copied().collect(),
            range: None,
            max_members: None,
            include_future: true,
            move_args: None,
        }),
        Some(CreateWithin {
            parents: OwnerConstraint::OneOf { refs: parents },
            kinds: kinds.iter().copied().collect(),
            placement: None,
        }),
        &mutate_classes(),
    );
    grant.capabilities.insert(cap_ref(PREVIEW_ID));
    grant.capabilities.insert(cap_ref(APPLY_ID));
    grant.permissions =
        PermissionSet::of([PermissionKind::MutateProject, PermissionKind::ReadContext]);
    grant
}

// ── Happy path ───────────────────────────────────────────────

#[test]
fn agent_create_then_reference_commits_as_one_undo_unit() {
    // The headline contract: `track.create` → `lane.create` with the
    // lane's parent carried as the slot-0 placeholder. The agent's
    // grant names the *deterministic* minted track (`id_watermark`
    // 1000 → raw 1000) as a permitted create parent — resolvable only
    // because the provisional gate sees the seam's scratch
    // post-state. One proposal, one composite commit, one undo.
    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        bindings_grant(
            "g1",
            "t1",
            &[
                "synvelis.operation.batch",
                "synvelis.track.create",
                "synvelis.lane.create",
            ],
            BTreeSet::from([project_ref(), ResourceRef::Track(TrackId::new(1000))]),
            &[
                ResourceKind::Project,
                ResourceKind::Track,
                ResourceKind::Lane,
            ],
        ),
    );
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    Some("t1"),
                    Some("g1"),
                    bound_batch(
                        &[track_create(), lane_create(&placeholder("track", 0))],
                        &[json!({"name": "new_track", "member": 0, "kind": "track"})],
                    ),
                    expected(1),
                ),
                agent(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);

    // One retained composite — the flattened member ops with the
    // placeholder already rewritten to the real minted ids.
    let Some(ProjectOp::Composite { ops }) = record.project_op() else {
        panic!(
            "bindings batch must retain a Composite, got {:?}",
            record.mutation
        );
    };
    assert_eq!(ops.len(), 2);
    assert_eq!(record.prepared.commands.len(), 2);

    // The candidate shows the deterministic ids — track raw 1000,
    // lane raw 1001 parented to it — and no placeholder survives.
    let track = TrackId::new(1000);
    let lane = LaneId::new(1001);
    assert!(record.prepared.candidate.tracks.contains_key(&track));
    assert_eq!(record.prepared.candidate.lanes[&lane].track_id, track);
    effect(&record, EffectKind::Create, &ResourceRef::Track(track));
    effect(&record, EffectKind::Create, &ResourceRef::Lane(lane));

    // The same grant carries the GW-05 apply gate — the proposal's
    // stored effects (including the transaction-created resources)
    // authorize against the real candidate.
    let (proposal, digest) = (
        result["proposal"].as_str().unwrap().to_owned(),
        result["digest"].as_str().unwrap().to_owned(),
    );
    accept_apply(&mut rig, &proposal, &digest, "t1", "g1");

    // Commit through the single-writer controller: one apply, one
    // undo removes *both* created resources — the composite is one
    // logical command, never two transactions.
    let mut controller =
        ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(1000)));
    controller
        .apply(Command {
            expected_revision: ProjectRevision(0),
            op: ProjectOp::SetProjectName {
                name: "demo".to_owned(),
            },
        })
        .unwrap();
    let rev = controller
        .apply(Command {
            expected_revision: record.prepared.base.revision,
            op: record.project_op().expect("project mutation").clone(),
        })
        .expect("prepared composite must apply");
    assert_eq!(rev, ProjectRevision(2));
    assert!(controller.live().tracks.contains_key(&track));
    assert!(controller.live().lanes.contains_key(&lane));
    assert!(controller.undo().unwrap());
    assert!(!controller.live().tracks.contains_key(&track));
    assert!(!controller.live().lanes.contains_key(&lane));
    assert_eq!(controller.live().tracks.len(), 1);
    assert_eq!(controller.live().lanes.len(), 2);
    assert!(controller.redo().unwrap());
}

#[test]
fn chained_bindings_resolve_across_three_members() {
    // track → lane → clip: two binding slots, the clip's create-parent
    // carried as the slot-1 (lane) placeholder. Proves positional
    // slot mapping and multi-hop resolution, not just a single hop.
    let mut rig = fixture_rig();
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    bound_batch(
                        &[
                            track_create(),
                            lane_create(&placeholder("track", 0)),
                            clip_add(&placeholder("lane", 1)),
                        ],
                        &[
                            json!({"name": "t", "member": 0, "kind": "track"}),
                            json!({"name": "l", "member": 1, "kind": "lane"}),
                        ],
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    let Some(ProjectOp::Composite { ops }) = record.project_op() else {
        panic!("expected Composite");
    };
    assert_eq!(ops.len(), 3);
    // track 1000 → lane 1001 → clip 1002, each parented to its
    // predecessor's real minted id.
    let track = TrackId::new(1000);
    let lane = LaneId::new(1001);
    let clip = ClipId::new(1002);
    assert_eq!(record.prepared.candidate.lanes[&lane].track_id, track);
    assert_eq!(record.prepared.candidate.clips[&clip].lane_id, lane);
    effect(&record, EffectKind::Create, &ResourceRef::Clip(clip));
}

#[test]
fn human_caller_bindings_batch_prepares() {
    // The trusted-user path needs no grant machinery — the same seam
    // resolves placeholders and retains the composite.
    let mut rig = fixture_rig();
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    bound_batch(
                        &[track_create(), lane_create(&placeholder("track", 0))],
                        &[json!({"name": "t", "member": 0, "kind": "track"})],
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    assert!(matches!(
        record.project_op(),
        Some(ProjectOp::Composite { .. })
    ));
    assert!(
        record
            .prepared
            .candidate
            .tracks
            .contains_key(&TrackId::new(1000))
    );
}

// ── Authorization stays authoritative ────────────────────────

#[test]
fn agent_without_create_axis_still_denies() {
    // Resolving placeholders never bypasses the create axis: the same
    // batch under a grant with no `create_within` denies at the
    // derived-effects stage.
    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        prep_grant(
            "g1",
            "t1",
            &[
                "synvelis.operation.batch",
                "synvelis.track.create",
                "synvelis.lane.create",
            ],
            ResourceScope::Dynamic(DynamicScope {
                owners: OwnerConstraint::Any,
                kinds: BTreeSet::from([
                    ResourceKind::Project,
                    ResourceKind::Track,
                    ResourceKind::Lane,
                ]),
                range: None,
                max_members: None,
                include_future: true,
                move_args: None,
            }),
            None,
            &mutate_classes(),
        ),
    );
    let err = rig
        .gw
        .handle(
            &prepare_req(
                Some("t1"),
                Some("g1"),
                bound_batch(
                    &[track_create(), lane_create(&placeholder("track", 0))],
                    &[json!({"name": "t", "member": 0, "kind": "track"})],
                ),
                expected(1),
            ),
            agent(),
        )
        .expect_err("no create axis must deny");
    assert!(matches!(err.code, GatewayError::Denied { .. }));
    assert_eq!(rig.engine.proposal_count(), 0);
}

#[test]
fn agent_scope_without_future_membership_denies() {
    // The created track resolves through the scratch oracle but is
    // still just a resource: `include_future: false` leaves it
    // uncovered (its creation revision is unrecorded → treated as
    // future), so the provisional gate denies identically to any
    // out-of-scope read.
    let mut rig = fixture_rig();
    let mut grant = bindings_grant(
        "g1",
        "t1",
        &[
            "synvelis.operation.batch",
            "synvelis.track.create",
            "synvelis.lane.create",
        ],
        BTreeSet::from([project_ref(), ResourceRef::Track(TrackId::new(1000))]),
        &[
            ResourceKind::Project,
            ResourceKind::Track,
            ResourceKind::Lane,
        ],
    );
    let ResourceScope::Dynamic(dynamic) = &mut grant.scope else {
        unreachable!()
    };
    dynamic.include_future = false;
    issue_agent(&rig.store, "t1", grant);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                Some("t1"),
                Some("g1"),
                bound_batch(
                    &[track_create(), lane_create(&placeholder("track", 0))],
                    &[json!({"name": "t", "member": 0, "kind": "track"})],
                ),
                expected(1),
            ),
            agent(),
        )
        .expect_err("future-created resources need include_future");
    assert!(matches!(err.code, GatewayError::Denied { .. }));
}

// ── Fail-closed binding contract ─────────────────────────────

#[test]
fn rejects_malformed_binding_declarations() {
    // Every malformed declaration shape is a typed schema violation —
    // non-object items, unknown/missing fields, out-of-range or
    // non-integer member indexes, unknown kind tags, and duplicate
    // (member, kind) pairs.
    let cases: Vec<(&str, Value)> = vec![
        ("non-object", json!("yes")),
        (
            "unknown field",
            json!({"name": "x", "member": 0, "kind": "track", "extra": 1}),
        ),
        ("missing name", json!({"member": 0, "kind": "track"})),
        ("missing member", json!({"name": "x", "kind": "track"})),
        ("missing kind", json!({"name": "x", "member": 0})),
        (
            "member out of range",
            json!({"name": "x", "member": 3, "kind": "track"}),
        ),
        (
            "member negative",
            json!({"name": "x", "member": -1, "kind": "track"}),
        ),
        (
            "member fractional",
            json!({"name": "x", "member": 0.5, "kind": "track"}),
        ),
        (
            "unknown kind",
            json!({"name": "x", "member": 0, "kind": "bogus"}),
        ),
    ];
    for (label, binding) in cases {
        let mut rig = fixture_rig();
        let Err(err) = rig.gw.handle(
            &prepare_req(
                None,
                None,
                bound_batch(&[track_create()], &[binding]),
                expected(1),
            ),
            human(),
        ) else {
            panic!("{label} must reject")
        };
        assert!(
            matches!(
                err.code,
                GatewayError::SchemaViolation { .. } | GatewayError::UnknownResourceKind { .. }
            ),
            "{label}: unexpected error {:?}",
            err.code
        );
    }

    // Duplicate (member, kind) declarations are ambiguous — reject.
    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                bound_batch(
                    &[track_create()],
                    &[
                        json!({"name": "a", "member": 0, "kind": "track"}),
                        json!({"name": "b", "member": 0, "kind": "track"}),
                    ],
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("duplicate (member, kind) must reject");
    assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));
}

#[test]
fn rejects_undeclared_and_kind_mismatched_placeholders() {
    // A placeholder naming a slot no binding declares, and one whose
    // kind tag disagrees with the declaration, are both typed
    // violations — the tag is part of the placeholder's identity.
    let cases: Vec<(&str, Vec<Value>, Vec<Value>)> = vec![
        (
            "undeclared slot",
            vec![track_create(), lane_create(&placeholder("track", 7))],
            vec![json!({"name": "t", "member": 0, "kind": "track"})],
        ),
        (
            "kind mismatch",
            vec![track_create(), lane_create(&placeholder("lane", 0))],
            vec![json!({"name": "t", "member": 0, "kind": "track"})],
        ),
    ];
    for (label, ops, bindings) in cases {
        let mut rig = fixture_rig();
        let Err(err) = rig.gw.handle(
            &prepare_req(None, None, bound_batch(&ops, &bindings), expected(1)),
            human(),
        ) else {
            panic!("{label} must reject")
        };
        assert!(
            matches!(err.code, GatewayError::SchemaViolation { .. }),
            "{label}: unexpected error {:?}",
            err.code
        );
    }
}

#[test]
fn rejects_backward_self_and_premature_references() {
    // `$output` may only name an *earlier* producer: a consumer in
    // member 0 referencing member 1's output (forward), member 0's
    // own output (self), and member 1 referencing member 1 (self at
    // a later index) all reject before lowering.
    let cases: Vec<(&str, Vec<Value>, Vec<Value>)> = vec![
        (
            "forward reference",
            vec![lane_create(&placeholder("track", 0)), track_create()],
            vec![json!({"name": "t", "member": 1, "kind": "track"})],
        ),
        (
            "self reference",
            vec![lane_create(&placeholder("track", 0))],
            vec![json!({"name": "t", "member": 0, "kind": "track"})],
        ),
        (
            "self reference at index 1",
            vec![track_create(), lane_create(&placeholder("lane", 1))],
            vec![
                json!({"name": "t", "member": 0, "kind": "track"}),
                json!({"name": "l", "member": 1, "kind": "lane"}),
            ],
        ),
    ];
    for (label, ops, bindings) in cases {
        let mut rig = fixture_rig();
        let Err(err) = rig.gw.handle(
            &prepare_req(None, None, bound_batch(&ops, &bindings), expected(1)),
            human(),
        ) else {
            panic!("{label} must reject")
        };
        assert!(
            matches!(err.code, GatewayError::SchemaViolation { .. }),
            "{label}: unexpected error {:?}",
            err.code
        );
    }
}

#[test]
fn rejects_unresolvable_producer_output() {
    // The producer's apply fails (clip 999 does not exist), so its
    // binding stays unresolved — a later placeholder naming it is a
    // typed rejection, never a guessed id. The real domain error
    // would also resurface at composite validation; the placeholder
    // failure is the earlier, more precise verdict.
    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                bound_batch(
                    &[
                        member(
                            "synvelis.clip.mute",
                            res_target("clip", &ResourceRef::Clip(ClipId::new(999))),
                            &json!({"muted": true}),
                        ),
                        lane_create(&placeholder("clip", 0)),
                    ],
                    &[json!({"name": "c", "member": 0, "kind": "clip"})],
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("unresolvable producer output must reject");
    let GatewayError::PreparationRejected { reason } = &err.code else {
        panic!("expected PreparationRejected, got {:?}", err.code);
    };
    assert!(reason.contains("could not be resolved"), "reason: {reason}");
}

#[test]
fn rejects_wrong_created_arity() {
    // A binding resolves iff its producer created *exactly one*
    // resource of the declared kind — zero (track.create declared as
    // lane) and several (detach-source mints a source plus two fresh
    // notes) are both typed rejections; ids are never guessed.
    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                bound_batch(
                    &[track_create()],
                    &[json!({"name": "wrong", "member": 0, "kind": "lane"})],
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("zero matching creations must reject");
    let GatewayError::PreparationRejected { reason } = &err.code else {
        panic!("expected PreparationRejected, got {:?}", err.code);
    };
    assert!(reason.contains("created 0"), "reason: {reason}");

    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                bound_batch(
                    &[member(
                        "synvelis.clip.detach-source",
                        res_target("clip", &clip12()),
                        &json!({}),
                    )],
                    &[json!({"name": "notes", "member": 0, "kind": "note"})],
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("two matching creations must reject");
    let GatewayError::PreparationRejected { reason } = &err.code else {
        panic!("expected PreparationRejected, got {:?}", err.code);
    };
    assert!(reason.contains("created 2"), "reason: {reason}");
}

#[test]
fn rejects_placeholder_object_keys_and_nested_batches() {
    // A placeholder may only appear as a *value* — an object key is a
    // typed violation. A batch member that is itself a batch is the
    // same nested-batch rejection the legacy path gives.
    let mut rig = fixture_rig();
    let mut args = json!({"name": "t-new"});
    args[placeholder("track", 0)] = json!(1);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                bound_batch(
                    &[member(
                        "synvelis.track.create",
                        create_target("project", &project_ref(), "track"),
                        &args,
                    )],
                    &[json!({"name": "t", "member": 0, "kind": "track"})],
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("placeholder object key must reject");
    assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));

    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                bound_batch(
                    &[op(
                        "synvelis.operation.batch",
                        Value::Null,
                        json!({"ops": [track_create()]}),
                    )],
                    &[json!({"name": "t", "member": 0, "kind": "track"})],
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("nested batch must reject");
    assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));
}

#[test]
fn empty_or_absent_bindings_keep_legacy_lowering() {
    // `bindings: []` and an absent `bindings` carry no
    // create-then-reference — the batch lowers through the legacy
    // member recursion with no scratch post-state.
    for bindings in [
        json!({"ops": [track_create(), lane_create(&TrackId::new(3).to_tagged())], "bindings": []}),
        json!({"ops": [track_create(), lane_create(&TrackId::new(3).to_tagged())]}),
    ] {
        let mut rig = fixture_rig();
        let result = complete(
            rig.gw
                .handle(
                    &prepare_req(
                        None,
                        None,
                        op("synvelis.operation.batch", Value::Null, bindings),
                        expected(1),
                    ),
                    human(),
                )
                .unwrap(),
        );
        let record = record_for(&rig, &result);
        assert!(matches!(
            record.project_op(),
            Some(ProjectOp::Composite { .. })
        ));
        // The lane parented to the *existing* track 3 — a real id,
        // never a placeholder. The new track drew raw 1000, so the
        // lane draws 1001 off the same watermark counter.
        assert_eq!(
            record.prepared.candidate.lanes[&LaneId::new(1001)].track_id,
            TrackId::new(3)
        );
    }
}
