//! Frontends: YAML safe-subset source and canonical JSON decode.
//!
//! [`parse_workflow_yaml`] is the YAML entry point: the restricted
//! parser produces a [`YamlValue`] tree, which is lowered into the
//! canonical JSON form the typed [`WorkflowDefinition`] deserializes
//! from. Expression positions (`args`, `target`, `items`, `job`,
//! `value`, condition operands) accept the ergonomic YAML surface —
//! `$`-prefixed reference strings, plain scalars, block sequences and
//! mappings — and are rewritten into the tagged [`Expr`]/[`Condition`]
//! wire form here, so the serde types stay strict and
//! source-agnostic.
//!
//! [`decode_workflow_definition`] / [`decode_registry_snapshot`] accept
//! the canonical JSON form directly — this is the path programmatic
//! builders (synvelis#273) and JSON fixtures use.

use serde_json::{Map, Value};

use crate::definition::WorkflowDefinition;
use crate::error::AutomationError;
use crate::macros::AtomicMacroDefinition;
use crate::registry::RegistrySnapshot;
use crate::yaml::{self, YamlValue};

/// Parses a workflow definition from the YAML safe subset.
///
/// # Errors
/// [`AutomationError::Yaml`] on parser rejection,
/// [`AutomationError::Definition`] on lowering/semantic failure,
/// [`AutomationError::Json`] on canonical decode failure.
pub fn parse_workflow_yaml(source: &str) -> Result<WorkflowDefinition, AutomationError> {
    let yaml = yaml::parse(source).map_err(|e| AutomationError::Yaml {
        line: e.line,
        reason: e.reason,
    })?;
    let raw = yaml_to_json(&yaml)?;
    let lowered = lower_definition(&raw)?;
    decode_workflow_definition(lowered)
}

/// Decodes a [`WorkflowDefinition`] from its canonical JSON form
/// (tagged `expr`/`op`/`kind`/`type` forms — what [`Expr`],
/// [`Condition`], [`Step`], and [`crate::schema::SchemaNode`] expect).
///
/// # Errors
/// [`AutomationError::UnsupportedDefinitionVersion`] for `schema_version
/// != 1`, [`AutomationError::Json`] on decode failure (including unknown
/// fields), [`AutomationError::Definition`] on semantic validation.
pub fn decode_workflow_definition(value: Value) -> Result<WorkflowDefinition, AutomationError> {
    let def: WorkflowDefinition =
        serde_json::from_value(value).map_err(|e| AutomationError::Json {
            reason: e.to_string(),
        })?;
    def.validate()?;
    Ok(def)
}

/// Decodes a [`RegistrySnapshot`] from canonical JSON.
///
/// # Errors
/// [`AutomationError::Json`] on decode failure,
/// [`AutomationError::Registry`] on semantic validation.
pub fn decode_registry_snapshot(value: Value) -> Result<RegistrySnapshot, AutomationError> {
    let snap: RegistrySnapshot =
        serde_json::from_value(value).map_err(|e| AutomationError::Registry {
            reason: e.to_string(),
        })?;
    snap.validate()?;
    Ok(snap)
}

/// Parses a registry snapshot from the YAML safe subset (same parser
/// and budgets as workflow sources).
///
/// # Errors
/// Same as [`parse_workflow_yaml`] / [`decode_registry_snapshot`].
pub fn parse_registry_snapshot_yaml(source: &str) -> Result<RegistrySnapshot, AutomationError> {
    let yaml = yaml::parse(source).map_err(|e| AutomationError::Yaml {
        line: e.line,
        reason: e.reason,
    })?;
    decode_registry_snapshot(yaml_to_json(&yaml)?)
}

/// Parses an atomic macro definition from the YAML safe subset —
/// the same parser/budgets as workflow sources, with the ergonomic
/// `$`-reference surface extended by `$param.`/`$output.`.
///
/// # Errors
/// Same shape as [`parse_workflow_yaml`] / [`decode_macro_definition`].
pub fn parse_macro_yaml(source: &str) -> Result<AtomicMacroDefinition, AutomationError> {
    let yaml = yaml::parse(source).map_err(|e| AutomationError::Yaml {
        line: e.line,
        reason: e.reason,
    })?;
    let raw = yaml_to_json(&yaml)?;
    let lowered = lower_macro_definition(&raw)?;
    decode_macro_definition(lowered)
}

/// Decodes an [`AtomicMacroDefinition`] from its canonical JSON form
/// (tagged `expr` forms — `$param.`/`$output.` included).
///
/// # Errors
/// [`AutomationError::UnsupportedDefinitionVersion`] for `schema_version
/// != 1`, [`AutomationError::Json`] on decode failure (including unknown
/// fields — authority fields cannot be expressed),
/// [`AutomationError::Definition`] on semantic validation.
pub fn decode_macro_definition(value: Value) -> Result<AtomicMacroDefinition, AutomationError> {
    let def: AtomicMacroDefinition =
        serde_json::from_value(value).map_err(|e| AutomationError::Json {
            reason: e.to_string(),
        })?;
    def.validate()?;
    Ok(def)
}

// ---------------------------------------------------------------------------
// YAML value → raw JSON
// ---------------------------------------------------------------------------

fn yaml_to_json(value: &YamlValue) -> Result<Value, AutomationError> {
    Ok(match value {
        YamlValue::Null => Value::Null,
        YamlValue::Bool(b) => Value::Bool(*b),
        YamlValue::Int(i) => Value::Number((*i).into()),
        YamlValue::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .ok_or_else(|| AutomationError::Json {
                reason: format!("non-finite float literal: {f}"),
            })?,
        YamlValue::Str(s) => Value::String(s.clone()),
        YamlValue::Seq(items) => Value::Array(
            items
                .iter()
                .map(yaml_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        YamlValue::Map(map) => Value::Object(
            map.iter()
                .map(|(k, v)| Ok::<(String, Value), AutomationError>((k.clone(), yaml_to_json(v)?)))
                .collect::<Result<Map<_, _>, _>>()?,
        ),
    })
}

// ---------------------------------------------------------------------------
// Definition lowering: raw YAML-JSON → canonical expr-tagged JSON
// ---------------------------------------------------------------------------

fn def_err(reason: impl Into<String>) -> AutomationError {
    AutomationError::Definition {
        reason: reason.into(),
    }
}

/// Rewrites expression positions inside a raw definition JSON tree
/// into tagged `expr`/`op` form. Everything else passes through
/// untouched (serde then enforces the strict schema).
fn lower_definition(raw: &Value) -> Result<Value, AutomationError> {
    let Value::Object(map) = raw else {
        return Ok(raw.clone());
    };
    let mut out = map.clone();
    if let Some(Value::Array(steps)) = out.get("steps") {
        let lowered = steps
            .iter()
            .map(lower_step)
            .collect::<Result<Vec<_>, _>>()?;
        out.insert("steps".to_owned(), Value::Array(lowered));
    }
    Ok(Value::Object(out))
}

fn lower_step(step: &Value) -> Result<Value, AutomationError> {
    let Value::Object(map) = step else {
        return Ok(step.clone());
    };
    let mut out = map.clone();
    let kind = out.get("kind").and_then(Value::as_str).unwrap_or("");
    match kind {
        "query" | "invoke" | "async-submit" | "atomic-macro" => lower_step_exprs(&mut out)?,
        "await-job" => {
            if let Some(job) = out.get("job") {
                let lowered = lower_expr(job)?;
                out.insert("job".to_owned(), lowered);
            }
        }
        "if" => {
            if let Some(cond) = out.get("condition") {
                let lowered = lower_condition(cond)?;
                out.insert("condition".to_owned(), lowered);
            }
            for key in ["then", "else"] {
                if let Some(Value::Array(items)) = out.get(key) {
                    let lowered = items
                        .iter()
                        .map(lower_step)
                        .collect::<Result<Vec<_>, _>>()?;
                    out.insert(key.to_owned(), Value::Array(lowered));
                }
            }
        }
        "for-each-bounded" => {
            if let Some(items) = out.get("items") {
                let lowered = lower_expr(items)?;
                out.insert("items".to_owned(), lowered);
            }
            if let Some(Value::Array(body)) = out.get("body") {
                let lowered = body.iter().map(lower_step).collect::<Result<Vec<_>, _>>()?;
                out.insert("body".to_owned(), Value::Array(lowered));
            }
        }
        "emit-result" => {
            if let Some(value) = out.get("value") {
                let lowered = lower_expr(value)?;
                out.insert("value".to_owned(), lowered);
            }
        }
        _ => {}
    }
    Ok(Value::Object(out))
}

/// Lowers `args`/`target` expression positions inside a step object.
fn lower_step_exprs(out: &mut Map<String, Value>) -> Result<(), AutomationError> {
    for key in ["args", "target"] {
        if let Some(expr) = out.get(key) {
            let lowered = lower_expr(expr)?;
            out.insert(key.to_owned(), lowered);
        }
    }
    Ok(())
}

/// Rewrites expression positions inside a raw definition JSON tree
/// into tagged `expr`/`op` form. Everything else passes through
/// untouched (serde then enforces the strict schema).
fn lower_macro_definition(raw: &Value) -> Result<Value, AutomationError> {
    let Value::Object(map) = raw else {
        return Ok(raw.clone());
    };
    let mut out = map.clone();
    if let Some(Value::Array(steps)) = out.get("steps") {
        let lowered = steps
            .iter()
            .map(lower_macro_step)
            .collect::<Result<Vec<_>, _>>()?;
        out.insert("steps".to_owned(), Value::Array(lowered));
    }
    Ok(Value::Object(out))
}

/// Lowers one raw macro step — `{id, capability, args?, target?}` with
/// no `kind` tag — rewriting `args`/`target` expression positions.
fn lower_macro_step(step: &Value) -> Result<Value, AutomationError> {
    let Value::Object(map) = step else {
        return Ok(step.clone());
    };
    let mut out = map.clone();
    lower_step_exprs(&mut out)?;
    Ok(Value::Object(out))
}

pub(crate) fn lower_condition(cond: &Value) -> Result<Value, AutomationError> {
    let Value::Object(map) = cond else {
        return Ok(cond.clone());
    };
    let mut out = map.clone();
    let op = out.get("op").and_then(Value::as_str).unwrap_or("");
    match op {
        "eq" | "ne" | "lt" | "le" | "gt" | "ge" => {
            for key in ["left", "right"] {
                if let Some(e) = out.get(key) {
                    let lowered = lower_expr(e)?;
                    out.insert(key.to_owned(), lowered);
                }
            }
        }
        "exists" => {
            if let Some(e) = out.get("expr") {
                let lowered = lower_expr(e)?;
                out.insert("expr".to_owned(), lowered);
            }
        }
        "and" | "or" => {
            let key = if op == "and" { "all" } else { "any" };
            if let Some(Value::Array(items)) = out.get(key) {
                let lowered = items
                    .iter()
                    .map(lower_condition)
                    .collect::<Result<Vec<_>, _>>()?;
                out.insert(key.to_owned(), Value::Array(lowered));
            }
        }
        "not" => {
            if let Some(c) = out.get("cond") {
                let lowered = lower_condition(c)?;
                out.insert("cond".to_owned(), lowered);
            }
        }
        _ => {}
    }
    Ok(Value::Object(out))
}

// ---------------------------------------------------------------------------
// Expression lowering
// ---------------------------------------------------------------------------

/// Converts a raw YAML-JSON value into tagged [`Expr`] wire form.
pub(crate) fn lower_expr(value: &Value) -> Result<Value, AutomationError> {
    Ok(match value {
        Value::String(s) if s.starts_with('$') => lower_reference(s)?,
        Value::String(s) => expr_literal(&Value::String(s.clone())),
        Value::Number(_) | Value::Bool(_) | Value::Null => expr_literal(value),
        Value::Array(items) => {
            let lowered = items
                .iter()
                .map(lower_expr)
                .collect::<Result<Vec<_>, _>>()?;
            serde_json::json!({"expr": "array", "items": lowered})
        }
        Value::Object(map) => {
            let mut fields = Map::new();
            for (k, v) in map {
                fields.insert(k.clone(), lower_expr(v)?);
            }
            serde_json::json!({"expr": "object", "fields": Value::Object(fields)})
        }
    })
}

fn expr_literal(value: &Value) -> Value {
    serde_json::json!({"expr": "literal", "value": value})
}

/// Parses a `$`-reference string into tagged expr form.
///
/// Grammar: `$input.<name>[.<field>...]`, `$steps.<step>[.<field>...]`,
/// `$item[.<field>...]`, plus the atomic-macro forms `$param.<name>`
/// and `$output.<name>` (AUTX-04). Anything else starting with `$` is
/// rejected — `$` is reserved for references.
fn lower_reference(text: &str) -> Result<Value, AutomationError> {
    let body = &text[1..];
    let segs: Vec<&str> = body.split('.').collect();
    // `$` alone, or any empty segment (`$input..a`, `$input.a.`), is
    // malformed. A bare `$input`/`$item` (no segments) lowers to an
    // empty path — the compiler rejects it with a typed error.
    if segs.iter().any(|s| s.is_empty()) {
        return Err(def_err(format!("malformed reference '{text}'")));
    }
    let head = segs[0];
    let rest = &segs[1..];
    let strings = |v: &[&str]| -> Vec<String> { v.iter().map(|s| (*s).to_owned()).collect() };
    match head {
        "input" => Ok(serde_json::json!({
            "expr": "input",
            "path": strings(rest),
        })),
        "steps" => {
            let Some((step, field)) = rest.split_first() else {
                return Err(def_err(format!("malformed reference '{text}'")));
            };
            Ok(serde_json::json!({
                "expr": "step-output",
                "step_output": { "step": step, "field": strings(field) },
            }))
        }
        "item" => Ok(serde_json::json!({
            "expr": "item",
            "path": strings(rest),
        })),
        "param" => Ok(serde_json::json!({
            "expr": "param",
            "path": strings(rest),
        })),
        "output" => {
            let [name] = rest else {
                return Err(def_err(format!("malformed reference '{text}'")));
            };
            Ok(serde_json::json!({
                "expr": "output",
                "name": name,
            }))
        }
        _ => Err(def_err(format!("unknown reference '{text}'"))),
    }
}
