//! Issue #601 slice 2 — `WireRequest::parse` property suite.
//!
//! Target contracts (product source of truth:
//! `crates/synvelis-gateway/src/wire.rs`):
//!
//! - Parse never panics on arbitrary `serde_json::Value`.
//! - Errors stay in the closed three-way set:
//!   `MalformedRequest` / `SelfDeclaredAuthority` /
//!   `ImplicitTargetRejected` (compile-time exhaustive match).
//! - Every `FORBIDDEN_AUTHORITY_FIELDS` top-level key is rejected as
//!   `SelfDeclaredAuthority` echoing the injected field; every
//!   `FORBIDDEN_IMPLICIT_TARGET_FIELDS` key as `ImplicitTargetRejected`;
//!   authority wins when both are present (scan order).
//! - The forbidden-key contract is top-level only: the same keys
//!   nested under `args` (not scanned) must not produce those errors.
//! - Accepted input satisfies the wire allow-list structure — a
//!   test-local independent model checker asserts the product contract
//!   rather than re-deriving it from product internals.
//! - Unknown keys inside `capability` / `capability.version` /
//!   `schema` / `schema.version` / `target` (known shape) /
//!   `consistency` (known kind) are `MalformedRequest` — never silently
//!   accepted through the internally tagged enums (serde#2123).
//!
//! Bounds: JSON depth ≤ 4, ≤ 8 children per container, strings ≤ 24
//! chars; the "valid base" generator emits structurally conforming
//! requests so accept-side structure is exercised, not just rejects.

use proptest::prelude::*;
use serde_json::{Map, Value, json};
use synvelis_gateway::outcome::GatewayError;
use synvelis_gateway::wire::{
    FORBIDDEN_AUTHORITY_FIELDS, FORBIDDEN_IMPLICIT_TARGET_FIELDS, WireRequest,
};

use crate::strategies::arb_ref_text;

// ---- independent model checker ----
//
// Re-implements the documented accept-side structure contract: parse
// may only accept values that satisfy this predicate. Divergence is
// evidence for the contract review, verified by the mutation run.

/// Struct-level fields serde accepts at the top level.
const TOP_LEVEL_KEYS: &[&str] = &[
    "request_id",
    "capability",
    "schema",
    "project",
    "target",
    "args",
    "consistency",
    "task",
    "grant",
    "idempotency_key",
];

/// Fields with no `#[serde(default)]` — must be present to parse.
const REQUIRED_KEYS: &[&str] = &[
    "request_id",
    "capability",
    "schema",
    "project",
    "consistency",
];

fn object_keys_within(obj: &Map<String, Value>, allowed: &[&str]) -> bool {
    obj.keys().all(|k| allowed.contains(&k.as_str()))
}

/// Mirrors `allowed_target_keys` / `allowed_consistency_keys` plus the
/// required fields serde enforces per variant.
fn nested_tag_object_allows(obj: &Map<String, Value>, tag_key: &str) -> bool {
    let Some(tag) = obj.get(tag_key).and_then(Value::as_str) else {
        return false;
    };
    let (allowed, required): (&[&str], &[&str]) = match (tag_key, tag) {
        ("shape", "resource") => (&["shape", "kind", "id"], &["kind", "id"]),
        ("shape", "resource-set") => (&["shape", "kind", "ids"], &["kind", "ids"]),
        ("shape", "create") => (
            &["shape", "parent_kind", "parent", "creates"],
            &["parent_kind", "parent", "creates"],
        ),
        ("kind", "latest" | "unconditional") => (&["kind"], &[]),
        ("kind", "read-at" | "expected-revision") => (&["kind", "revision"], &["revision"]),
        ("kind", "fixed-snapshot") => (&["kind", "snapshot"], &["snapshot"]),
        _ => return false,
    };
    object_keys_within(obj, allowed) && required.iter().all(|k| obj.contains_key(*k))
}

/// `true` iff `value` satisfies the wire allow-list structure —
/// every input `WireRequest::parse` may legitimately accept.
fn model_allows(value: &Value) -> bool {
    let Some(obj) = value.as_object() else {
        return false;
    };
    if !object_keys_within(obj, TOP_LEVEL_KEYS)
        || !REQUIRED_KEYS.iter().all(|k| obj.contains_key(*k))
    {
        return false;
    }
    for name in ["capability", "schema"] {
        let Some(sub) = obj.get(name).and_then(Value::as_object) else {
            return false;
        };
        let version_key: &[&str] = if name == "capability" {
            &["id", "version"]
        } else {
            &["name", "version"]
        };
        if !object_keys_within(sub, version_key) {
            return false;
        }
        if let Some(version) = sub.get("version") {
            let Some(version) = version.as_object() else {
                return false;
            };
            if !object_keys_within(version, &["major", "minor"]) {
                return false;
            }
        }
    }
    if let Some(target) = obj.get("target") {
        let Some(target) = target.as_object() else {
            return false;
        };
        if !nested_tag_object_allows(target, "shape") {
            return false;
        }
    }
    if let Some(consistency) = obj.get("consistency") {
        let Some(consistency) = consistency.as_object() else {
            return false;
        };
        if !nested_tag_object_allows(consistency, "kind") {
            return false;
        }
    }
    true
}

// ---- strategies ----

/// JSON object keys: mostly ordinary identifiers, with occasional
/// forbidden-looking keys so arbitrary inputs also reach the scans.
fn arb_json_key() -> impl Strategy<Value = String> {
    prop_oneof![
        8 => "[a-z_]{1,12}",
        1 => prop::sample::select(FORBIDDEN_AUTHORITY_FIELDS.to_vec()).prop_map(str::to_owned),
        1 => prop::sample::select(FORBIDDEN_IMPLICIT_TARGET_FIELDS.to_vec()).prop_map(str::to_owned),
    ]
}

/// Arbitrary JSON values, depth/size-bounded for lane speed.
fn arb_json_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|i| json!(i)),
        any::<f64>()
            .prop_filter("finite", |f| f.is_finite())
            .prop_map(|f| json!(f)),
        "[ -~]{0,24}".prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 48, 8, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..8).prop_map(Value::Array),
            prop::collection::btree_map(arb_json_key(), inner, 0..8)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

/// A structurally valid `consistency` object.
fn arb_consistency() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(json!({"kind": "latest"})),
        Just(json!({"kind": "unconditional"})),
        any::<u64>().prop_map(|rev| json!({"kind": "read-at", "revision": rev})),
        any::<u64>().prop_map(|rev| json!({"kind": "expected-revision", "revision": rev})),
        "[ -~]{1,24}".prop_map(|snap| json!({"kind": "fixed-snapshot", "snapshot": snap})),
    ]
}

/// A structurally valid `target` object (or absent).
fn arb_target() -> impl Strategy<Value = Option<Value>> {
    prop::option::of(prop_oneof![
        ("[a-z]{1,12}", "[ -~]{1,32}")
            .prop_map(|(kind, id)| json!({"shape": "resource", "kind": kind, "id": id})),
        ("[a-z]{1,12}", prop::collection::vec("[ -~]{1,16}", 1..4))
            .prop_map(|(kind, ids)| json!({"shape": "resource-set", "kind": kind, "ids": ids})),
        ("[a-z]{1,12}", "[ -~]{1,32}", "[a-z]{1,12}").prop_map(|(pk, parent, creates)| {
            json!({"shape": "create", "parent_kind": pk, "parent": parent, "creates": creates})
        }),
    ])
}

/// A near-valid wire request: structurally conforming, so parse
/// reaches serde and accepts unless a generated value violates a
/// string-ref rule. `args`/`task`/`grant`/`idempotency_key`/`target`
/// stay optional-arbitrary.
fn arb_wire_request() -> impl Strategy<Value = Value> {
    (
        "[a-zA-Z0-9_-]{1,64}",
        arb_ref_text(),
        (any::<u32>(), any::<u32>()),
        arb_ref_text(),
        (any::<u32>(), any::<u32>()),
        "[ -~]{0,32}",
        arb_consistency(),
        arb_target(),
        prop::option::of(arb_ref_text()),
        prop::option::of(arb_ref_text()),
        prop::option::of(arb_ref_text()),
    )
        .prop_map(
            |(
                request_id,
                cap_id,
                (cap_major, cap_minor),
                schema_name,
                (schema_major, schema_minor),
                project,
                consistency,
                target,
                task,
                grant,
                idempotency_key,
            )| {
                let mut req = json!({
                    "request_id": request_id,
                    "capability": {"id": cap_id, "version": {"major": cap_major, "minor": cap_minor}},
                    "schema": {"name": schema_name, "version": {"major": schema_major, "minor": schema_minor}},
                    "project": project,
                    "consistency": consistency,
                });
                let obj = req.as_object_mut().expect("request is an object");
                if let Some(target) = target {
                    obj.insert("target".into(), target);
                }
                if let Some(task) = task {
                    obj.insert("task".into(), Value::String(task));
                }
                if let Some(grant) = grant {
                    obj.insert("grant".into(), Value::String(grant));
                }
                if let Some(key) = idempotency_key {
                    obj.insert("idempotency_key".into(), Value::String(key));
                }
                req
            },
        )
}

/// Insert `key: value` into the top-level object of `req`.
fn inject_top_level(req: &Value, key: &str, value: Value) -> Value {
    let mut req = req.clone();
    req.as_object_mut()
        .expect("wire request is an object")
        .insert(key.into(), value);
    req
}

fn is_malformed(result: &Result<WireRequest, GatewayError>) -> bool {
    matches!(result, Err(GatewayError::MalformedRequest { .. }))
}

/// The closed three-way error contract — a helper so proptest
/// assertions carry no struct-pattern braces.
fn is_contract_error(err: &GatewayError) -> bool {
    matches!(
        err,
        GatewayError::MalformedRequest { .. }
            | GatewayError::SelfDeclaredAuthority { .. }
            | GatewayError::ImplicitTargetRejected { .. }
    )
}

fn is_self_declared(result: &Result<WireRequest, GatewayError>) -> bool {
    matches!(result, Err(GatewayError::SelfDeclaredAuthority { .. }))
}

/// Either forbidden-key rejection — the contract is top-level only.
fn is_forbidden_rejection(result: &Result<WireRequest, GatewayError>) -> bool {
    matches!(
        result,
        Err(GatewayError::SelfDeclaredAuthority { .. }
            | GatewayError::ImplicitTargetRejected { .. })
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any JSON value: parse never panics (proptest catches), and a
    /// rejection stays in the closed three-way error set.
    #[test]
    fn parse_never_panics_and_errors_are_closed(value in arb_json_value()) {
        match WireRequest::parse(&value) {
            Ok(_) => {}
            Err(err) => prop_assert!(
                is_contract_error(&err),
                "error outside the closed set: {:?}",
                err
            ),
        }
    }

    /// Accept side: parse may only accept values satisfying the
    /// independent allow-list model; reject side stays closed.
    #[test]
    fn accepted_values_satisfy_allowlist_model(req in arb_wire_request()) {
        match WireRequest::parse(&req) {
            Ok(_) => prop_assert!(
                model_allows(&req),
                "parse accepted a structure outside the allow-list model: {req}"
            ),
            Err(err) => prop_assert!(
                matches!(err, GatewayError::MalformedRequest { .. }),
                "unexpected error variant on forbidden-free input: {err:?}"
            ),
        }
    }

    /// Every authority-shaped top-level key is rejected as
    /// `SelfDeclaredAuthority` echoing the injected field.
    #[test]
    fn authority_fields_rejected(
        req in arb_wire_request(),
        field in prop::sample::select(FORBIDDEN_AUTHORITY_FIELDS.to_vec()),
        injected in arb_json_value(),
    ) {
        let req = inject_top_level(&req, field, injected);
        match WireRequest::parse(&req) {
            Err(GatewayError::SelfDeclaredAuthority { field: f }) => {
                prop_assert_eq!(f, field);
            }
            other => prop_assert!(false, "expected SelfDeclaredAuthority, got {:?}", other),
        }
    }

    /// Every implicit-target top-level key is rejected as
    /// `ImplicitTargetRejected` echoing the injected field.
    #[test]
    fn implicit_target_fields_rejected(
        req in arb_wire_request(),
        field in prop::sample::select(FORBIDDEN_IMPLICIT_TARGET_FIELDS.to_vec()),
        injected in arb_json_value(),
    ) {
        let req = inject_top_level(&req, field, injected);
        match WireRequest::parse(&req) {
            Err(GatewayError::ImplicitTargetRejected { field: f }) => {
                prop_assert_eq!(f, field);
            }
            other => prop_assert!(false, "expected ImplicitTargetRejected, got {:?}", other),
        }
    }

    /// The authority scan precedes the implicit-target scan: both
    /// present → `SelfDeclaredAuthority`.
    #[test]
    fn authority_precedes_implicit_target(
        req in arb_wire_request(),
        auth in prop::sample::select(FORBIDDEN_AUTHORITY_FIELDS.to_vec()),
        implicit in prop::sample::select(FORBIDDEN_IMPLICIT_TARGET_FIELDS.to_vec()),
    ) {
        let req = inject_top_level(&req, auth, Value::Null);
        let req = inject_top_level(&req, implicit, Value::Null);
        prop_assert!(is_self_declared(&WireRequest::parse(&req)));
    }

    /// Top-level-only contract: the same keys nested under `args`
    /// (a non-scanned field) must not produce the rejections.
    #[test]
    fn forbidden_keys_nested_under_args_not_rejected(
        req in arb_wire_request(),
        field in prop::sample::select(
            FORBIDDEN_AUTHORITY_FIELDS
                .iter()
                .chain(FORBIDDEN_IMPLICIT_TARGET_FIELDS)
                .copied()
                .collect::<Vec<_>>(),
        ),
        nested in arb_json_value(),
    ) {
        let mut req = req;
        req.as_object_mut()
            .expect("wire request is an object")
            .insert("args".into(), json!({field: nested}));
        let result = WireRequest::parse(&req);
        prop_assert!(!is_forbidden_rejection(&result));
    }

    /// Unknown keys inside the owned nested objects are
    /// `MalformedRequest` — never silently accepted.
    #[test]
    fn unknown_nested_keys_are_malformed(
        req in arb_wire_request(),
        site in prop::sample::select(vec![
            "capability",
            "capability.version",
            "schema",
            "schema.version",
        ]),
    ) {
        let mut req = req;
        let path: Vec<&str> = site.split('.').collect();
        let mut cursor = req.as_object_mut().expect("request is an object");
        for key in &path[..path.len() - 1] {
            cursor = cursor
                .get_mut(*key)
                .and_then(Value::as_object_mut)
                .expect("nested container is an object");
        }
        cursor
            .get_mut(path[path.len() - 1])
            .and_then(Value::as_object_mut)
            .expect("nested container is an object")
            .insert("zz-extra".into(), Value::Null);
        prop_assert!(is_malformed(&WireRequest::parse(&req)));
    }

    /// Dropping any required key → `MalformedRequest`.
    #[test]
    fn missing_required_keys_are_malformed(
        req in arb_wire_request(),
        key in prop::sample::select(REQUIRED_KEYS.to_vec()),
    ) {
        let mut req = req;
        req.as_object_mut()
            .expect("wire request is an object")
            .remove(key);
        prop_assert!(is_malformed(&WireRequest::parse(&req)));
    }
}
