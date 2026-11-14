//! AUTX-01 (synvelis#260) — headless tests for the read-only Action
//! Catalog projection (`synvelis_workspace::action_catalog`).
//!
//! Coverage map:
//! - `ActionCatalog::project`/`project_gateway`: presentation metadata
//!   joins the *current* registered `CapabilityDescriptor` — never a
//!   second registry, never copied authority fields.
//! - `list`/`describe`: distinct verdicts for unknown id, unsupported
//!   version, disabled/planned/retired, orphan presentations, and
//!   schema-coherence states; machine-readable display values are the
//!   descriptor's own.
//! - No-authority: enumeration produces no caller/grant/approval; a
//!   listed capability still fails closed at Gateway policy.
//! - Shared fixture: one CapabilityRef/version + explicit stable target
//!   per operation domain (`Query`/`ProjectMutation`/`RuntimeControl`/
//!   `AsyncOperation`) drives `Gateway::bind`/`invoke` identically — the
//!   contract GUI lowering (#266) and CLI (#262) share.
//! - Consumer neutrality: `catalog.descriptors()` feeds the AUTX-07
//!   `project_action_catalog` view projection unchanged.

use std::sync::Arc;

use serde_json::json;
use synvelis_gateway::fixture::{
    EchoHandler, FixtureAdapter, PermitSetPolicy, fixture_descriptors, fixture_schemas,
    install as install_base,
};
use synvelis_gateway::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityOperation, CapabilityRef,
    CapabilityRegistry, CapabilityVersion, Gateway, GatewayError, OperationDomain, PermissionKind,
    PermissionSet, SchemaRef, SchemaRegistry, TargetContract, WireConsistency, WireRequest,
    WireTarget,
};
use synvelis_project::{ProjectId, ProjectRevision, TrackId};
use synvelis_workspace::action_catalog::fixture as cat_fixture;
use synvelis_workspace::action_catalog::{
    ActionCatalog, ActionCatalogError, ActionEntry, ActionFilter, ActionPresentation, ActionStatus,
    ExecutorState, SchemaConsistency as CatalogSchemaConsistency,
};
use synvelis_workspace::{
    AutomationViewAction, GrantState, automation_requests, project_action_catalog,
    resource_kind_tag,
};

// ── helpers ────────────────────────────────────────────────────────────

fn cap(id: &str, minor: u32) -> CapabilityRef {
    CapabilityRef {
        id: CapabilityId::new(id).expect("fixture id"),
        version: CapabilityVersion::new(1, minor),
    }
}

fn all_capabilities() -> Vec<CapabilityDescriptor> {
    let mut v = fixture_descriptors();
    v.extend(cat_fixture::capabilities());
    v
}

fn all_schemas() -> SchemaRegistry {
    let mut store = SchemaRegistry::new();
    for (name, version, schema) in fixture_schemas().into_iter().chain(cat_fixture::schemas()) {
        store.register(name, version, schema).expect("schema");
    }
    store
}

fn catalog() -> ActionCatalog {
    ActionCatalog::project(
        &all_capabilities(),
        Some(&all_schemas()),
        None,
        &cat_fixture::presentations(),
    )
    .expect("catalog")
}

fn entry<'a>(cat: &'a ActionCatalog, capability: &CapabilityRef) -> &'a ActionEntry {
    cat.entries()
        .iter()
        .find(|e| e.capability == *capability)
        .expect("entry")
}

fn wire_for(cat: &ActionCatalog, capability: &CapabilityRef) -> WireRequest {
    let _ = cat;
    cat_fixture::invocations()
        .into_iter()
        .find(|i| i.wire.capability == *capability)
        .map(|i| i.wire)
        .expect("fixture invocation")
}

// ── join / list / describe ─────────────────────────────────────────────

#[test]
fn catalog_joins_presentations_with_current_descriptors() {
    let cat = catalog();
    let registered = all_capabilities().len();
    assert_eq!(cat.entries().len(), registered);

    // Every registered capability yields an entry whose descriptor is the
    // registry's own object — machine-readable display values cannot drift.
    for desc in &all_capabilities() {
        let capability = desc.capability_ref();
        let e = entry(&cat, &capability);
        assert_eq!(e.descriptor.as_ref(), Some(desc));
        assert_eq!(e.status, ActionStatus::Available);
        assert_eq!(e.schema, CatalogSchemaConsistency::Consistent);
        assert_eq!(e.executor, ExecutorState::Unknown);
    }

    // Presentation metadata joined by exact CapabilityRef.
    let track = entry(&cat, &cap(cat_fixture::TRACK_QUERY_ID, 0));
    let p = track.presentation.as_ref().expect("presentation");
    assert_eq!(p.display_key, "catalog.display.track-query");
    assert_eq!(p.category, "query");
    assert_eq!(
        p.parameter_hints.as_ref().unwrap()["detail"],
        "param.hint.detail.enum"
    );
}

#[test]
fn list_filters_query_category_and_visibility() {
    let cat = catalog();

    // Case-insensitive substring over id / display / description / category.
    let rows = cat.list(&ActionFilter {
        query: Some("TRACK"),
        ..ActionFilter::default()
    });
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].capability.id.to_string(),
        cat_fixture::TRACK_QUERY_ID
    );

    let rows = cat.list(&ActionFilter {
        query: Some("catalog.display.clip"),
        ..ActionFilter::default()
    });
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].capability.id.to_string(),
        cat_fixture::CLIP_QUERY_ID
    );

    // Category is a presentation-only grouping.
    let rows = cat.list(&ActionFilter {
        category: Some("query"),
        ..ActionFilter::default()
    });
    assert!(rows.iter().all(|e| {
        e.presentation
            .as_ref()
            .is_some_and(|p| p.category.eq_ignore_ascii_case("query"))
    }));
    assert!(
        rows.iter()
            .any(|e| e.capability.id.to_string() == cat_fixture::TRACK_QUERY_ID)
    );

    // Hidden presentation is excluded from browse listing by default but
    // remains describable.
    let hidden = cap("synvelis.midi.create-source", 0);
    assert!(
        cat.list(&ActionFilter::default())
            .iter()
            .all(|e| e.capability != hidden)
    );
    let rows = cat.list(&ActionFilter {
        include_hidden: true,
        ..ActionFilter::default()
    });
    assert!(rows.iter().any(|e| e.capability == hidden));
    assert!(cat.describe(&hidden).is_some());
}

#[test]
fn unpresented_capabilities_stay_enumerable_but_flagged() {
    // A registered capability without product metadata is still registry
    // truth — listed by default, excludable, never given invented display
    // data.
    let mut presentations = cat_fixture::presentations();
    presentations.retain(|p| p.capability.id.to_string() != cat_fixture::CLIP_QUERY_ID);
    let cat = ActionCatalog::project(
        &all_capabilities(),
        Some(&all_schemas()),
        None,
        &presentations,
    )
    .expect("catalog");
    let e = entry(&cat, &cap(cat_fixture::CLIP_QUERY_ID, 0));
    assert!(e.presentation.is_none());
    assert!(e.descriptor.is_some());
    assert!(
        cat.list(&ActionFilter::default())
            .iter()
            .any(|r| r.capability == cap(cat_fixture::CLIP_QUERY_ID, 0))
    );
    assert!(
        !cat.list(&ActionFilter {
            include_unpresented: false,
            ..ActionFilter::default()
        })
        .iter()
        .any(|r| r.capability == cap(cat_fixture::CLIP_QUERY_ID, 0))
    );
}

#[test]
fn describe_distinguishes_unknown_id_and_unsupported_version() {
    let cat = catalog();

    // Exact registered ref → full entry.
    let e = cat
        .describe(&cap(cat_fixture::TRACK_QUERY_ID, 0))
        .expect("registered");
    assert_eq!(e.status, ActionStatus::Available);
    assert!(e.descriptor.is_some());

    // Known id, unregistered version → typed verdict, not a bare miss.
    let e = cat
        .describe(&cap(cat_fixture::TRACK_QUERY_ID, 9))
        .expect("known id");
    assert_eq!(
        e.status,
        ActionStatus::UnsupportedVersion {
            supported: vec![CapabilityVersion::new(1, 0)],
        }
    );
    assert!(e.descriptor.is_none());

    // Entirely unknown id → no information at all.
    assert!(
        cat.describe(&cap("synvelis.no.such.capability", 0))
            .is_none()
    );
}

#[test]
fn orphan_presentations_surface_as_unknown_or_unsupported() {
    let mut presentations = cat_fixture::presentations();
    // Presentation for a capability id the registry does not know.
    presentations.push(ActionPresentation {
        capability: cap("synvelis.unregistered.ghost", 0),
        display_key: "catalog.display.ghost".to_string(),
        category: "query".to_string(),
        description_key: "catalog.desc.ghost".to_string(),
        parameter_hints: None,
        visibility_hint: None,
    });
    // Presentation pinned to a version the registry does not have.
    presentations.push(ActionPresentation {
        capability: cap(cat_fixture::TRACK_QUERY_ID, 7),
        display_key: "catalog.display.track-future".to_string(),
        category: "query".to_string(),
        description_key: "catalog.desc.track-future".to_string(),
        parameter_hints: None,
        visibility_hint: None,
    });

    let cat = ActionCatalog::project(
        &all_capabilities(),
        Some(&all_schemas()),
        None,
        &presentations,
    )
    .expect("catalog");

    let ghost = entry(&cat, &cap("synvelis.unregistered.ghost", 0));
    assert_eq!(ghost.status, ActionStatus::Unknown);
    assert!(ghost.descriptor.is_none());
    assert!(ghost.presentation.is_some());

    let future = entry(&cat, &cap(cat_fixture::TRACK_QUERY_ID, 7));
    assert_eq!(
        future.status,
        ActionStatus::UnsupportedVersion {
            supported: vec![CapabilityVersion::new(1, 0)],
        }
    );
    assert!(future.descriptor.is_none());
    assert!(future.presentation.is_some());

    // Both stay visible in listings — discovery never hides defects.
    let rows = cat.list(&ActionFilter::default());
    assert!(
        rows.iter()
            .any(|e| e.capability == cap("synvelis.unregistered.ghost", 0))
    );
    assert!(
        rows.iter()
            .any(|e| e.capability == cap(cat_fixture::TRACK_QUERY_ID, 7))
    );
}

#[test]
fn availability_states_project_verbatim_and_distinct() {
    let mut capabilities = all_capabilities();
    for (i, availability) in [
        Availability::Disabled {
            reason: "maintenance window".to_string(),
        },
        Availability::Planned,
        Availability::Retired,
    ]
    .into_iter()
    .enumerate()
    {
        capabilities[i].availability = availability;
    }
    let cat = ActionCatalog::project(
        &capabilities,
        Some(&all_schemas()),
        None,
        &cat_fixture::presentations(),
    )
    .expect("catalog");

    let disabled = entry(&cat, &capabilities[0].capability_ref());
    assert_eq!(
        disabled.status,
        ActionStatus::Disabled {
            reason: "maintenance window".to_string(),
        }
    );
    assert_eq!(
        entry(&cat, &capabilities[1].capability_ref()).status,
        ActionStatus::Planned
    );
    assert_eq!(
        entry(&cat, &capabilities[2].capability_ref()).status,
        ActionStatus::Retired
    );

    // Non-available rows remain listed — displayability ≠ invocability,
    // and the catalog never upgrades a state.
    assert!(cat.entries().iter().all(|e| e.descriptor.is_some()));
}

#[test]
fn schema_consistency_is_typed_consistent_mismatch_unknown() {
    // Incoherent registration (bypasses `register_capability`'s check):
    // declared schema absent from the store → Mismatch, not Available.
    let mut capabilities = all_capabilities();
    capabilities[0].input_schema = SchemaRef::new("synvelis.missing.input", 1, 0);
    let cat =
        ActionCatalog::project(&capabilities, Some(&all_schemas()), None, &[]).expect("catalog");
    let e = entry(&cat, &capabilities[0].capability_ref());
    assert!(matches!(
        e.schema,
        CatalogSchemaConsistency::Mismatch { .. }
    ));
    // Availability status is independent of schema coherence.
    assert_eq!(e.status, ActionStatus::Available);

    // No schema store supplied → typed Unknown, never guessed Consistent.
    let cat = ActionCatalog::project(&capabilities, None, None, &[]).expect("catalog");
    assert!(
        cat.entries()
            .iter()
            .all(|e| e.schema == CatalogSchemaConsistency::Unknown)
    );
}

#[test]
fn presentation_carries_only_display_metadata() {
    // The presentation type has no authority surface: serializing one
    // yields exactly the product-metadata keys — no schema/target/
    // effect/permission/availability/version copies that could drift.
    let p = &cat_fixture::presentations()[0];
    let value = serde_json::to_value(p).expect("serialize");
    let obj = value.as_object().expect("object");
    // Every serialized key is product metadata — optional fields absent
    // when unset, and no authority-shaped key can ever appear.
    for key in obj.keys() {
        assert!(
            [
                "capability",
                "category",
                "description_key",
                "display_key",
                "parameter_hints",
                "visibility_hint",
            ]
            .contains(&key.as_str()),
            "unexpected presentation key: {key}"
        );
    }
    for required in ["capability", "display_key", "category", "description_key"] {
        assert!(obj.contains_key(required), "missing {required}");
    }
    // And the wire form of `capability` is the exact versioned ref.
    assert_eq!(
        obj["capability"],
        json!({
            "id": p.capability.id.to_string(),
            "version": { "major": 1, "minor": 0 },
        })
    );
}

#[test]
fn display_name_change_preserves_capability_identity() {
    // Renaming display metadata must not change the registered
    // Capability/Invocation meaning — the 1:1 versioned reference holds.
    let mut renamed = cat_fixture::presentations();
    for p in &mut renamed {
        p.display_key = format!("{}.renamed", p.display_key);
        p.description_key = format!("{}.renamed", p.description_key);
    }
    let a = catalog();
    let b = ActionCatalog::project(&all_capabilities(), Some(&all_schemas()), None, &renamed)
        .expect("catalog");

    for (ea, eb) in a.entries().iter().zip(b.entries()) {
        assert_eq!(ea.capability, eb.capability);
        assert_eq!(ea.descriptor, eb.descriptor);
        assert_eq!(ea.status, eb.status);
    }
    assert_eq!(a.descriptors(), b.descriptors());

    // A request lowered against either catalog names the same ref/schema.
    let wire = wire_for(&a, &cap("synvelis.midi.add-note", 1));
    assert_eq!(wire.capability, cap("synvelis.midi.add-note", 1));
    assert_eq!(
        wire.schema,
        SchemaRef::new("synvelis.midi.add-note.input", 1, 1)
    );
}

#[test]
fn duplicate_inputs_fail_closed() {
    let caps = all_capabilities();
    let mut dup_caps = caps.clone();
    dup_caps.push(caps[0].clone());
    assert_eq!(
        ActionCatalog::project(&dup_caps, Some(&all_schemas()), None, &[]).unwrap_err(),
        ActionCatalogError::DuplicateCapability {
            capability: caps[0].capability_ref(),
        }
    );

    let presentations = cat_fixture::presentations();
    let mut dup_pres = presentations.clone();
    dup_pres.push(presentations[0].clone());
    assert_eq!(
        ActionCatalog::project(&caps, Some(&all_schemas()), None, &dup_pres).unwrap_err(),
        ActionCatalogError::DuplicatePresentation {
            capability: presentations[0].capability.clone(),
        }
    );
}

#[test]
fn catalog_is_registry_truth_not_a_second_registry() {
    let cat = catalog();
    // The entry set's registered refs are exactly the input set — the
    // catalog adds and drops nothing.
    let mut registered: Vec<CapabilityRef> = all_capabilities()
        .iter()
        .map(CapabilityDescriptor::capability_ref)
        .collect();
    registered.sort();
    let mut projected: Vec<CapabilityRef> = cat
        .entries()
        .iter()
        .filter(|e| e.descriptor.is_some())
        .map(|e| e.capability.clone())
        .collect();
    projected.sort();
    assert_eq!(registered, projected);

    // Deterministic: same inputs → identical projection.
    assert_eq!(cat.entries(), catalog().entries());
}

// ── no authority ───────────────────────────────────────────────────────

#[test]
fn enumeration_grants_no_authority_invoke_still_policy_gated() {
    // DenyAllPolicy gateway: the catalog lists everything, yet invoke
    // still fails closed — list/describe are discovery, not permission.
    let mut gateway = Gateway::new();
    install_base(&mut gateway).expect("base install");
    cat_fixture::install(&mut gateway).expect("catalog fixture install");
    let cat =
        ActionCatalog::project_gateway(&gateway, &cat_fixture::presentations()).expect("project");
    assert!(
        cat.entries()
            .iter()
            .any(|e| e.status == ActionStatus::Available)
    );

    let adapter = FixtureAdapter::for_testing();
    let caller = adapter.agent("autx01-enumerator").expect("caller");
    let wire = wire_for(&cat, &cap("synvelis.track.query", 0));
    let req = gateway
        .bind(wire, caller)
        .expect("bind is not authorization");
    let err = gateway.invoke(&req).expect_err("deny-all must deny");
    assert!(
        matches!(err.code, GatewayError::Denied { .. }),
        "expected Denied, got {err:?}"
    );

    // Self-declared authority fields never survive wire parsing.
    let mut json_req = json!({
        "request_id": "autx01-probe",
        "capability": { "id": "synvelis.track.query", "version": { "major": 1, "minor": 0 } },
        "schema": { "name": "synvelis.track.query.input", "version": { "major": 1, "minor": 0 } },
        "project": ProjectId::new(7).to_tagged(),
        "target": { "shape": "resource", "kind": "track", "id": TrackId::new(3).to_tagged() },
        "args": {},
        "consistency": { "kind": "latest" },
        "as_user": "admin",
    });
    assert!(Gateway::parse_wire(&json_req).is_err());
    json_req.as_object_mut().unwrap().remove("as_user");
    json_req["role"] = json!("admin");
    assert!(Gateway::parse_wire(&json_req).is_err());
}

#[test]
fn unregistered_ids_stay_unknown_and_unexecutable() {
    // A UI-namespace-looking id is just another unregistered id: the
    // catalog cannot mint it into an action — no descriptor, no schema,
    // no target truth, status Unknown.
    let mut presentations = cat_fixture::presentations();
    presentations.push(ActionPresentation {
        capability: cap("ui.palette.select", 0),
        display_key: "catalog.display.ui-select".to_string(),
        category: "ui".to_string(),
        description_key: "catalog.desc.ui-select".to_string(),
        parameter_hints: None,
        visibility_hint: None,
    });
    let cat = ActionCatalog::project(
        &all_capabilities(),
        Some(&all_schemas()),
        None,
        &presentations,
    )
    .expect("catalog");
    let e = entry(&cat, &cap("ui.palette.select", 0));
    assert_eq!(e.status, ActionStatus::Unknown);
    assert!(e.descriptor.is_none());
    assert_eq!(e.schema, CatalogSchemaConsistency::Unknown);
}

// ── shared GUI/CLI fixture ─────────────────────────────────────────────

#[test]
fn fixture_invocations_cover_all_domains_with_explicit_targets() {
    let cat = catalog();
    let mut gateway = Gateway::with_evaluator(Box::new(PermitSetPolicy::new(PermissionSet::of([
        PermissionKind::ReadContext,
        PermissionKind::DiscloseContext,
        PermissionKind::MutateProject,
        PermissionKind::ControlTransport,
        PermissionKind::RunJob,
        PermissionKind::ExternalAction,
    ]))));
    install_base(&mut gateway).expect("base install");
    cat_fixture::install(&mut gateway).expect("catalog fixture install");

    let adapter = FixtureAdapter::for_testing();
    let caller = adapter.agent("autx01-fixture").expect("caller");

    let invocations = cat_fixture::invocations();
    let mut domains = std::collections::BTreeSet::new();
    for inv in &invocations {
        domains.insert(inv.domain);

        // The wire names the exact catalog CapabilityRef — the same id +
        // version GUI lowering and CLI would send.
        let e = cat.describe(&inv.wire.capability).expect("catalog entry");
        assert_eq!(e.status, ActionStatus::Available, "{}", inv.name);
        let desc = e.descriptor.as_ref().expect("descriptor");
        assert_eq!(desc.domain, inv.domain, "{}", inv.name);
        assert_eq!(inv.wire.schema, desc.input_schema, "{}", inv.name);

        // Explicit stable target: Resource contracts carry a typed wire
        // target of the contracted kind; None contracts carry none.
        // Nothing is inferred from selection/focus.
        match desc.target {
            TargetContract::Resource { kind } => {
                let Some(WireTarget::Resource { kind: tk, .. }) = &inv.wire.target else {
                    panic!("{}: explicit resource target required", inv.name);
                };
                assert_eq!(*tk, resource_kind_tag(kind), "{}", inv.name);
            }
            TargetContract::None => assert!(inv.wire.target.is_none(), "{}", inv.name),
            other => panic!("{}: unhandled contract {other:?}", inv.name),
        }

        // Domain-correct consistency: query pins a revision, mutation
        // requires ExpectedRevision, runtime/async are Unconditional.
        match inv.domain {
            OperationDomain::Query => assert!(
                matches!(
                    inv.wire.consistency,
                    WireConsistency::Latest
                        | WireConsistency::ReadAt { .. }
                        | WireConsistency::FixedSnapshot { .. }
                ),
                "{}",
                inv.name
            ),
            OperationDomain::ProjectMutation => assert!(
                matches!(
                    inv.wire.consistency,
                    WireConsistency::ExpectedRevision { .. }
                ),
                "{}",
                inv.name
            ),
            OperationDomain::RuntimeControl | OperationDomain::AsyncOperation => assert!(
                matches!(inv.wire.consistency, WireConsistency::Unconditional),
                "{}",
                inv.name
            ),
            OperationDomain::Proposal => {
                panic!("{}: unhandled domain {:?}", inv.name, inv.domain)
            }
        }

        // The same wire drives bind+invoke through the real Gateway.
        let req = gateway
            .bind(inv.wire.clone(), caller.clone())
            .unwrap_or_else(|e| panic!("{}: bind failed: {e:?}", inv.name));
        let outcome = gateway
            .invoke(&req)
            .unwrap_or_else(|e| panic!("{}: invoke failed: {e:?}", inv.name));
        match inv.domain {
            OperationDomain::AsyncOperation => {
                assert!(matches!(
                    outcome,
                    synvelis_gateway::GatewayOutcome::Accepted { .. }
                ));
            }
            _ => {
                assert!(matches!(
                    outcome,
                    synvelis_gateway::GatewayOutcome::Completed { .. }
                ));
            }
        }
    }

    // Query (track + clip), ProjectMutation, RuntimeControl, AsyncOperation.
    assert_eq!(domains.len(), 4);
    assert!(domains.contains(&OperationDomain::Query));
    assert!(domains.contains(&OperationDomain::ProjectMutation));
    assert!(domains.contains(&OperationDomain::RuntimeControl));
    assert!(domains.contains(&OperationDomain::AsyncOperation));
}

#[test]
fn catalog_feeds_autx07_view_projection_unchanged() {
    // #266's consumer contract: `project_action_catalog` takes the
    // descriptor slice — the catalog supplies exactly that, no view-side
    // change required.
    let mut gateway = Gateway::new();
    install_base(&mut gateway).expect("base install");
    cat_fixture::install(&mut gateway).expect("catalog fixture install");
    let cat =
        ActionCatalog::project_gateway(&gateway, &cat_fixture::presentations()).expect("project");

    let view = project_action_catalog(&cat.descriptors(), |_| GrantState::Satisfied, "", 3);
    assert_eq!(view.rows.len(), cat.descriptors().len());
    let row = view
        .rows
        .iter()
        .find(|r| r.id == cat_fixture::TRACK_QUERY_ID && r.version == "1.0")
        .expect("track row");
    assert!(row.executable);
    assert_eq!(row.domain, OperationDomain::Query);

    // And the view's own lowering still produces the same wire ref the
    // fixture advertises — GUI and catalog agree on identity.
    let descriptors = cat.descriptors();
    let lowered = automation_requests(
        &AutomationViewAction::Preview {
            capability: format!("{}@1.0", cat_fixture::TRACK_QUERY_ID),
            target: Some(synvelis_workspace::ExplicitTarget::Resource {
                kind: "track".to_string(),
                id: TrackId::new(3).to_tagged(),
            }),
            args: json!({ "detail": "summary" }),
        },
        &descriptors,
        &[],
        &ProjectId::new(7).to_tagged(),
        "req-autx01",
        &WireConsistency::ReadAt {
            revision: ProjectRevision(4),
        },
    )
    .expect("lower");
    assert_eq!(lowered.len(), 1);
    assert_eq!(lowered[0].capability, cap(cat_fixture::TRACK_QUERY_ID, 0));
}

// ── executor state (AUTX-01-F01) ───────────────────────────────────────

#[test]
fn executor_state_distinguishes_installed_absent_unknown() {
    // A supplied registry answers per exact ref: handler-backed
    // registrations report Installed, the one handler-less
    // registration reports Absent, and descriptor-less entries stay
    // Unknown — the verdict is never guessed.
    let capabilities = all_capabilities();
    let mut registry = CapabilityRegistry::new();
    let mut absent_ref = None;
    for desc in &capabilities {
        let capability = desc.capability_ref();
        let handler: Option<Arc<dyn CapabilityOperation>> =
            if capability.id.to_string() == cat_fixture::TRACK_QUERY_ID && absent_ref.is_none() {
                absent_ref = Some(capability.clone());
                None
            } else {
                Some(Arc::new(EchoHandler))
            };
        registry.register(desc.clone(), handler).expect("register");
    }
    let absent_ref = absent_ref.expect("absent fixture");

    let mut presentations = cat_fixture::presentations();
    presentations.push(ActionPresentation {
        capability: cap("synvelis.unregistered.ghost", 0),
        display_key: "catalog.display.ghost".to_string(),
        category: "query".to_string(),
        description_key: "catalog.desc.ghost".to_string(),
        parameter_hints: None,
        visibility_hint: None,
    });

    let cat = ActionCatalog::project(
        &capabilities,
        Some(&all_schemas()),
        Some(&registry),
        &presentations,
    )
    .expect("catalog");

    for e in cat.entries().iter().filter(|e| e.descriptor.is_some()) {
        if e.capability == absent_ref {
            assert_eq!(e.executor, ExecutorState::Absent, "{}", e.capability.id);
        } else {
            assert_eq!(e.executor, ExecutorState::Installed, "{}", e.capability.id);
        }
    }
    assert_eq!(
        entry(&cat, &cap("synvelis.unregistered.ghost", 0)).executor,
        ExecutorState::Unknown
    );
}

#[test]
fn project_without_registry_keeps_executor_unknown() {
    // `executors: None` → every entry reports Unknown even though the
    // same descriptors could answer through a registry — the verdict
    // is never guessed.
    let cat = catalog();
    assert!(
        cat.entries()
            .iter()
            .all(|e| e.executor == ExecutorState::Unknown)
    );
}

#[test]
fn project_gateway_reports_executor_verdicts() {
    // project_gateway wires the live registry: handler-backed fixture
    // capabilities report Installed and a handler-less registration —
    // the real No-Handler shape PrepareEngine uses — reports Absent.
    // Absent is display metadata only: bind still succeeds and
    // invoke remains the sole NoExecutor authority.
    let mut gateway = Gateway::with_evaluator(Box::new(PermitSetPolicy::new(PermissionSet::of([
        PermissionKind::ReadContext,
        PermissionKind::DiscloseContext,
        PermissionKind::MutateProject,
        PermissionKind::ControlTransport,
        PermissionKind::RunJob,
        PermissionKind::ExternalAction,
    ]))));
    install_base(&mut gateway).expect("base install");
    cat_fixture::install(&mut gateway).expect("catalog fixture install");

    let mut no_handler = fixture_descriptors()
        .into_iter()
        .find(|d| d.id.to_string() == "synvelis.transport.set-position")
        .expect("transport descriptor");
    no_handler.id = CapabilityId::new("synvelis.test.no-handler").expect("id");
    let no_handler_ref = no_handler.capability_ref();
    let transport_ref = fixture_descriptors()
        .iter()
        .find(|d| d.id.to_string() == "synvelis.transport.set-position")
        .expect("transport descriptor")
        .capability_ref();
    gateway
        .register_capability(no_handler, None)
        .expect("register no-handler");

    let mut presentations = cat_fixture::presentations();
    presentations.push(ActionPresentation {
        capability: cap("synvelis.unregistered.ghost", 0),
        display_key: "catalog.display.ghost".to_string(),
        category: "query".to_string(),
        description_key: "catalog.desc.ghost".to_string(),
        parameter_hints: None,
        visibility_hint: None,
    });

    let cat = ActionCatalog::project_gateway(&gateway, &presentations).expect("project");

    assert_eq!(entry(&cat, &no_handler_ref).executor, ExecutorState::Absent);
    assert_eq!(
        entry(&cat, &cap(cat_fixture::TRACK_QUERY_ID, 0)).executor,
        ExecutorState::Installed
    );
    assert_eq!(
        entry(&cat, &cap("synvelis.unregistered.ghost", 0)).executor,
        ExecutorState::Unknown
    );

    // The catalog verdict matches runtime truth: the same ref that
    // projects Absent fails invoke with NoExecutor.
    let mut wire = cat_fixture::invocations()
        .into_iter()
        .find(|i| i.wire.capability == transport_ref)
        .expect("transport invocation")
        .wire;
    wire.capability = no_handler_ref;
    let adapter = FixtureAdapter::for_testing();
    let caller = adapter.agent("autx01-f01").expect("caller");
    let req = gateway.bind(wire, caller).expect("bind");
    let err = gateway.invoke(&req).expect_err("no executor must fail");
    assert!(
        matches!(err.code, GatewayError::NoExecutor { .. }),
        "expected NoExecutor, got {err:?}"
    );
}
