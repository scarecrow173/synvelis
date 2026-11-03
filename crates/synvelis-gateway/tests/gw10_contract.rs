// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! GW-10 contract evidence — Issue #62, unit GW10-A / gate G0.
//!
//! The versioned capability / schema / typed stable id / typed
//! result+error contract, evidenced at L1/L2. The deep contract tests
//! live in `contract.rs`; these cases re-run the matrix-relevant
//! assertions and emit the evidence records G0 gates on.

mod rig;

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::fixture::{self, PermitSetPolicy};
use synvelis_gateway::*;
use synvelis_project::SourceId;

fn suite() -> std::sync::Arc<Evidence> {
    Evidence::suite("gw10-contract")
}

fn agent() -> CallerIdentity {
    adapter().agent("mcp-client-1").unwrap()
}

fn all_permissions() -> PermissionSet {
    PermissionSet::of([
        PermissionKind::ReadContext,
        PermissionKind::DiscloseContext,
        PermissionKind::MutateProject,
        PermissionKind::ControlTransport,
        PermissionKind::RunJob,
        PermissionKind::ExternalAction,
    ])
}

/// Fixture gateway with a policy that permits everything — isolates
/// bind/validation behavior from authorization.
fn permissive_gateway() -> Gateway {
    let mut gw = Gateway::with_evaluator(Box::new(PermitSetPolicy::new(all_permissions())));
    fixture::install(&mut gw).unwrap();
    gw
}

fn midi_add_note_req(revision: u64) -> Value {
    json!({
        "request_id": "req-m1",
        "capability": {"id": "synvelis.midi.add-note", "version": {"major": 1, "minor": 1}},
        "schema": {"name": "synvelis.midi.add-note.input", "version": {"major": 1, "minor": 1}},
        "project": project().to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()},
        "args": {
            "key": 60,
            "velocity": 100,
            "position": {"domain": "source-native", "start": 240, "len": 480}
        },
        "consistency": {"kind": "expected-revision", "revision": revision},
    })
}

#[allow(clippy::result_large_err)]
fn bind(gw: &mut Gateway, value: &Value) -> Result<OperationRequest, ErrorEnvelope> {
    let wire = Gateway::parse_wire(value)?;
    gw.bind(wire, agent())
}

// ── GT-001: typed stable ids / explicit targets ──────────────

#[test]
fn typed_ids_enforce_domain_and_kind_on_the_wire() {
    let ev = suite();
    let trace = CausalTrace::new();

    // An explicit typed target binds and binds *to the typed ref*.
    let mut gw = permissive_gateway();
    let bound = bind(&mut gw, &midi_add_note_req(3)).unwrap();
    trace.mark(
        "driver",
        "target-bound",
        "resource target bound to typed ref",
    );
    assert!(matches!(bound.target(), OperationTarget::Resource { .. }));

    // A foreign kind tag is rejected at bind — typed ids are not
    // free-text: "clip:9" cannot stand in for "src:9".
    let mut wrong_kind = midi_add_note_req(3);
    wrong_kind["target"]["id"] = json!(synvelis_project::ClipId::new(9).to_tagged());
    let err = bind(&mut gw, &wrong_kind).unwrap_err();
    trace.mark(
        "driver",
        "foreign-kind-denied",
        "clip tag presented as source",
    );
    assert!(matches!(
        err.code,
        GatewayError::InvalidId { .. } | GatewayError::WrongResourceKind { .. }
    ));

    // An untagged id and an unknown kind are typed errors, not
    // silent coercions.
    let mut untagged = midi_add_note_req(3);
    untagged["target"]["id"] = json!("9");
    assert!(bind(&mut gw, &untagged).is_err());
    let mut bad_kind = midi_add_note_req(3);
    bad_kind["target"]["kind"] = json!("flurb");
    assert!(matches!(
        bind(&mut gw, &bad_kind).unwrap_err().code,
        GatewayError::UnknownResourceKind { .. }
    ));

    ev.case("GT-001")
        .meta(Layer::L1, RunMode::Mock)
        .fixture("fixture-permissive")
        .trace(&trace.events())
        .verify(
            "explicit typed target binds; foreign kind/id tag rejected",
            "explicit typed target binds; foreign kind/id tag rejected",
        );
}

// ── GT-002: versioned capability surface ─────────────────────

#[test]
fn unknown_capability_and_version_drift_are_typed_rejections() {
    let ev = suite();
    let mut gw = permissive_gateway();

    // Unknown capability → typed rejection naming it.
    let mut unknown = midi_add_note_req(3);
    unknown["capability"]["id"] = json!("synvelis.does.not-exist");
    assert!(matches!(
        bind(&mut gw, &unknown).unwrap_err().code,
        GatewayError::UnknownCapability { .. }
    ));

    // Registered id at an unsupported major → version rejection.
    let mut drift = midi_add_note_req(3);
    drift["capability"]["version"]["major"] = json!(99);
    assert!(matches!(
        bind(&mut gw, &drift).unwrap_err().code,
        GatewayError::UnsupportedCapabilityVersion { .. }
    ));

    ev.case("GT-002")
        .meta(Layer::L1, RunMode::Mock)
        .fixture("fixture-permissive")
        .verify(
            "unknown capability and version drift rejected as typed errors",
            "unknown capability and version drift rejected as typed errors",
        );
}

// ── GT-003: schema envelope + typed error taxonomy ───────────

#[test]
fn schema_drift_and_malformed_wire_are_classified_errors() {
    let ev = suite();
    let mut gw = permissive_gateway();

    // Unknown input schema name → typed rejection.
    let mut bad_schema = midi_add_note_req(3);
    bad_schema["schema"]["name"] = json!("synvelis.bogus.schema");
    let err = bind(&mut gw, &bad_schema).unwrap_err();
    assert!(matches!(err.code, GatewayError::UnknownSchema { .. }));
    // Every failure is classifiable — category + retry advice ride
    // the envelope (GTC-008).
    assert_eq!(err.code.category(), ErrorCategory::Capability);

    // A request missing required envelope fields is malformed.
    let malformed = json!({"capability": {"id": "synvelis.midi.add-note"}});
    assert!(matches!(
        Gateway::parse_wire(&malformed).unwrap_err().code,
        GatewayError::MalformedRequest { .. } | GatewayError::SchemaViolation { .. }
    ));

    ev.case("GT-003")
        .meta(Layer::L1, RunMode::Mock)
        .fixture("fixture-permissive")
        .verify(
            "schema drift + malformed wire produce classified errors",
            "schema drift + malformed wire produce classified errors",
        );
}
