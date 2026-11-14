//! Luau type projection and stub generation.
//!
//! [`ScriptSchema`] → Luau type is a **total, deterministic** mapping.
//! The stub is generated *from the [`ScriptApiModel`]* — never from the
//! raw snapshot — so binding and stub can never disagree about which
//! entries exist or which schema versions they pin.
//!
//! # Mapping rules
//!
//! | `ScriptSchema` | Luau type |
//! |---|---|
//! | `Bool` | `boolean` |
//! | `Number` | `number` |
//! | `Integer` in f64-exact range | `number` |
//! | `Integer` outside f64-exact range | `SynvelisI64` (decimal text; decoded + range-checked at invoke — never silently rounded) |
//! | `String` | `string` |
//! | `Id { kind }` | `Synvelis<Kind>Id` (string alias; tag format + kind re-validated at invoke) |
//! | `Enum` | string-literal union |
//! | `Array` | `{T}` |
//! | `Object` | table type; `required: false` → optional prop (`?`) |
//! | `Optional(inner)` | `inner | nil` (Null) — distinct from an absent optional field |
//! | `TimeRange` | `{ domain: "<literal>", start: SynvelisI64, len: SynvelisI64 }` |
//!
//! Constraints Luau cannot express — integer bounds, `max_len`, enum
//! membership beyond the literal union, `deny_unknown` exactness, id tag
//! format, array length, time-domain — are re-validated by the Gateway
//! schema check at invoke; the stub header states this explicitly.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use synvelis_gateway::{CapabilityRef, SchemaRef, TargetContract};

use crate::canon::sha256_hex;
use crate::error::ScriptApiError;
use crate::model::ScriptApiModel;
use crate::schema::ScriptSchema;

/// Stub document format version.
pub const STUB_SCHEMA_VERSION: u32 = 1;

/// f64 represents every integer in `[-2^53, 2^53]` exactly.
const F64_EXACT_INT: i64 = 1 << 53;

/// Luau type aliases collected while rendering — emitted once, before
/// the per-entry types that reference them.
#[derive(Default)]
struct LuauNamespace {
    /// Whether `SynvelisI64` is referenced anywhere.
    uses_i64: bool,
    /// `ResourceKind`s needing a `Synvelis<Kind>Id` alias.
    id_kinds: BTreeSet<String>,
}

/// The structured surface of one generated stub entry — the side the
/// binding↔stub consistency check compares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StubEntry {
    /// Script API name.
    pub name: String,
    /// Generated `declare function` name.
    pub function_name: String,
    /// Exact capability pin.
    pub capability: CapabilityRef,
    /// Declared input schema ref.
    pub input_schema: SchemaRef,
    /// Declared output schema ref.
    pub output_schema: SchemaRef,
}

/// A generated Luau type stub plus its structured surface and digests.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LuauStub {
    /// Stub document format version.
    pub schema_version: u32,
    /// Digest of the [`ScriptApiModel`] this stub was generated from.
    pub model_digest: String,
    /// Digest of the source registry snapshot.
    pub snapshot_digest: String,
    /// Digest of the surface spec (naming policy).
    pub surface_digest: String,
    /// SHA-256 of `text` — detects hand-editing or one-sided regen.
    pub stub_digest: String,
    /// The `.d.luau`-style definition text.
    pub text: String,
    /// Structured per-entry surface (consistency checks, consumers).
    pub entries: Vec<StubEntry>,
}

impl LuauStub {
    /// Generates the stub from the model's invocable public entries.
    ///
    /// Hidden and non-`Invocable` entries produce **no** type
    /// declarations — their existence never reaches the analyzer
    /// surface.
    ///
    /// # Errors
    /// [`ScriptApiError::Internal`] — an entry marked invocable lacks
    /// the descriptor/schema data its verdict guarantees (only
    /// reachable from a hand-edited serialized model);
    /// [`ScriptApiError::Json`] — canonicalization failure.
    pub fn generate(model: &ScriptApiModel) -> Result<Self, ScriptApiError> {
        let mut ns = LuauNamespace::default();
        let mut rendered_entries = Vec::new();

        for entry in model.invocable_entries() {
            let internal = |what: &str| ScriptApiError::Internal {
                reason: format!("invocable entry {:?} lacks {what}", entry.name),
            };
            let descriptor = entry
                .descriptor
                .as_ref()
                .ok_or_else(|| internal("a descriptor"))?;
            let input_binding = entry
                .input
                .as_ref()
                .ok_or_else(|| internal("an input schema"))?;
            let output_binding = entry
                .output
                .as_ref()
                .ok_or_else(|| internal("an output schema"))?;

            rendered_entries.push(RenderedEntry {
                name: entry.name.clone(),
                function_name: function_name(&entry.name),
                capability: entry.requested.clone(),
                input_schema: input_binding.schema.clone(),
                output_schema: output_binding.schema.clone(),
                input_ty: render_type(&input_binding.resolved, &mut ns),
                output_ty: render_type(&output_binding.resolved, &mut ns),
                target_ty: target_type(descriptor.target, &mut ns),
                description: entry.description.clone(),
                capability_label: entry.requested.to_string(),
            });
        }

        let text = render_stub(model, &ns, &rendered_entries);
        let stub_digest = sha256_hex(text.as_bytes());

        Ok(Self {
            schema_version: STUB_SCHEMA_VERSION,
            model_digest: model.model_digest.clone(),
            snapshot_digest: model.snapshot_digest.clone(),
            surface_digest: model.surface_digest.clone(),
            stub_digest,
            text,
            entries: rendered_entries
                .iter()
                .map(|e| StubEntry {
                    name: e.name.clone(),
                    function_name: e.function_name.clone(),
                    capability: e.capability.clone(),
                    input_schema: e.input_schema.clone(),
                    output_schema: e.output_schema.clone(),
                })
                .collect(),
        })
    }

    /// Verifies this stub was generated from `model` — the drift check.
    ///
    /// # Errors
    /// [`ScriptApiError::StaleArtifact`] — model digest mismatch.
    pub fn verify_against(&self, model: &ScriptApiModel) -> Result<(), ScriptApiError> {
        model.verify_artifact("luau-stub", &self.model_digest)
    }
}

struct RenderedEntry {
    name: String,
    function_name: String,
    capability: CapabilityRef,
    input_schema: SchemaRef,
    output_schema: SchemaRef,
    input_ty: String,
    output_ty: String,
    target_ty: Option<String>,
    description: Option<String>,
    capability_label: String,
}

/// `PascalCase` of a script name, used for type alias and function names.
fn pascal_case(name: &str) -> String {
    name.split('_')
        .map(|seg| {
            let mut chars = seg.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// `synvelis<PascalName>` — the generated global function name.
fn function_name(name: &str) -> String {
    format!("synvelis{}", pascal_case(name))
}

/// `Synvelis<Kind>Id` alias name for a resource kind (Debug variant name
/// is already `PascalCase` and stable for this closed enum).
fn kind_alias_name(kind: synvelis_gateway::ResourceKind) -> String {
    format!("Synvelis{kind:?}Id")
}

fn time_domain_literal(domain: synvelis_gateway::TimeDomain) -> &'static str {
    match domain {
        synvelis_gateway::TimeDomain::ProjectMusical => "project-musical",
        synvelis_gateway::TimeDomain::Media => "media",
        synvelis_gateway::TimeDomain::SourceNative => "source-native",
    }
}

fn is_simple_ty(ty: &str) -> bool {
    !ty.is_empty()
        && !ty
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '{' | '}' | '(' | ')' | '|'))
}

/// `T?` — optional/nullable suffix, parenthesized when `T` is compound.
fn optional_suffix(ty: &str) -> String {
    if is_simple_ty(ty) {
        format!("{ty}?")
    } else {
        format!("({ty})?")
    }
}

/// `T | nil` — the Null projection, parenthesized when `T` is compound.
fn nil_union(ty: &str) -> String {
    if is_simple_ty(ty) {
        format!("{ty} | nil")
    } else {
        format!("({ty}) | nil")
    }
}

fn luau_string_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:04X}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn is_luau_ident(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Renders a [`ScriptSchema`] as a Luau type expression, collecting
/// namespace requirements into `ns`.
fn render_type(schema: &ScriptSchema, ns: &mut LuauNamespace) -> String {
    match schema {
        ScriptSchema::Bool => "boolean".to_owned(),
        ScriptSchema::Number => "number".to_owned(),
        ScriptSchema::Integer { min, max } => {
            let lo = min.unwrap_or(i64::MIN);
            let hi = max.unwrap_or(i64::MAX);
            if lo >= -F64_EXACT_INT && hi <= F64_EXACT_INT {
                "number".to_owned()
            } else {
                ns.uses_i64 = true;
                "SynvelisI64".to_owned()
            }
        }
        ScriptSchema::String { .. } => "string".to_owned(),
        ScriptSchema::Id { kind } => {
            ns.id_kinds.insert(kind_alias_name(*kind));
            kind_alias_name(*kind)
        }
        ScriptSchema::Enum { values } => {
            let inner = values
                .iter()
                .map(|v| luau_string_literal(v))
                .collect::<Vec<_>>()
                .join(" | ");
            if inner.is_empty() {
                // Degenerate schema: the Gateway rejects every value, so
                // no honest Luau type exists. `nil` at least fails fast
                // at the binding layer rather than admitting `string`.
                "nil".to_owned()
            } else {
                inner
            }
        }
        ScriptSchema::Array { item, .. } => format!("{{{}}}", render_type(item, ns)),
        ScriptSchema::Object { fields, .. } => render_object(fields, ns),
        ScriptSchema::Optional(inner) => nil_union(&render_type(inner, ns)),
        ScriptSchema::TimeRange { domain } => {
            ns.uses_i64 = true;
            format!(
                "{{ domain: {}, start: SynvelisI64, len: SynvelisI64 }}",
                luau_string_literal(time_domain_literal(*domain))
            )
        }
    }
}

fn render_object(
    fields: &std::collections::BTreeMap<String, crate::schema::ScriptField>,
    ns: &mut LuauNamespace,
) -> String {
    let mut parts = Vec::with_capacity(fields.len());
    for (name, field) in fields {
        let ty = render_type(&field.schema, ns);
        let prop_ty = match (&field.schema, field.required) {
            // required + Optional(inner): key must be present, value may
            // be nil — `inner | nil`, never the absent-admitting `?`.
            (ScriptSchema::Optional(inner), true) => nil_union(&render_type(inner, ns)),
            // optional field: may be absent; `?` also admits nil which is
            // a harmless superset (re-validated at invoke).
            (_, false) => optional_suffix(&ty),
            (_, true) => ty.clone(),
        };
        let key = if is_luau_ident(name) {
            name.clone()
        } else {
            format!("[{}]", luau_string_literal(name))
        };
        parts.push(format!("{key}: {prop_ty}"));
    }
    format!("{{ {} }}", parts.join(", "))
}

/// The Luau type of the `target` parameter, or `None` when the contract
/// takes no target.
fn target_type(contract: TargetContract, ns: &mut LuauNamespace) -> Option<String> {
    match contract {
        TargetContract::None => None,
        TargetContract::Resource { kind } => {
            ns.id_kinds.insert(kind_alias_name(kind));
            Some(kind_alias_name(kind))
        }
        TargetContract::ResourceSet { kind } => {
            ns.id_kinds.insert(kind_alias_name(kind));
            Some(format!("{{{}}}", kind_alias_name(kind)))
        }
        TargetContract::Create { parent, creates } => {
            ns.id_kinds.insert(kind_alias_name(parent));
            ns.id_kinds.insert(kind_alias_name(creates));
            Some(format!("{{ parent: {} }}", kind_alias_name(parent)))
        }
    }
}

fn render_stub(model: &ScriptApiModel, ns: &LuauNamespace, entries: &[RenderedEntry]) -> String {
    let mut out = String::new();
    out.push_str("--!strict\n");
    out.push_str("-- DO NOT EDIT — synvelis-script generated definition\n");
    let _ = writeln!(out, "-- snapshot-digest: {}", model.snapshot_digest);
    let _ = writeln!(out, "-- surface-digest: {}", model.surface_digest);
    let _ = writeln!(out, "-- model-digest: {}", model.model_digest);
    out.push_str(
        "--\n\
         -- Static types grant NO authority: every call is re-authorized and\n\
         -- re-validated by the Synvelis Gateway at invoke time (schema\n\
         -- re-check, availability, policy, approval). Analyzer success never\n\
         -- implies execution permission.\n\
         --\n\
         -- Constraints Luau cannot express are re-validated by the Gateway\n\
         -- schema check at invoke: integer bounds/precision, string length,\n\
         -- enum membership, object deny-unknown exactness, id tag format and\n\
         -- kind, array length, and time-range domain.\n\
         -- Null is `nil`; an absent value is an omitted optional field.\n",
    );

    // ---- helper aliases ----
    out.push_str("\nexport type SynvelisConsistency =\n");
    out.push_str("    { kind: \"latest\" }\n");
    out.push_str("    | { kind: \"read-at\", revision: number }\n");
    out.push_str("    | { kind: \"fixed-snapshot\", snapshot: string }\n");
    out.push_str("    | { kind: \"expected-revision\", revision: number }\n");
    out.push_str("    | { kind: \"unconditional\" }\n");
    out.push_str("export type SynvelisProjectId = string\n");
    if ns.uses_i64 {
        out.push_str(
            "-- decimal i64 text; the host binding decodes and range-checks it\n\
             -- at invoke (values beyond f64 precision are never rounded)\n\
             export type SynvelisI64 = string\n",
        );
    }
    for kind in &ns.id_kinds {
        let _ = writeln!(
            out,
            "-- branded id alias; the \"<kind>-<32hex>\" tag is checked at invoke\n\
             export type {kind} = string"
        );
    }

    // ---- per-entry type aliases ----
    for entry in entries {
        if let Some(desc) = &entry.description {
            let _ = writeln!(out, "\n-- {} ({desc})", entry.name);
        } else {
            let _ = writeln!(out, "\n-- {} ({})", entry.name, entry.capability_label);
        }
        let pascal = pascal_case(&entry.name);
        let _ = writeln!(out, "export type {pascal}Input = {}", entry.input_ty);
        let _ = writeln!(out, "export type {pascal}Output = {}", entry.output_ty);
    }

    // ---- declare functions ----
    for entry in entries {
        let pascal = pascal_case(&entry.name);
        let mut params = vec!["project: SynvelisProjectId".to_owned()];
        if let Some(target_ty) = &entry.target_ty {
            params.push(format!("target: {target_ty}"));
        }
        params.push(format!("args: {pascal}Input"));
        params.push("consistency: SynvelisConsistency".to_owned());
        let _ = writeln!(
            out,
            "\ndeclare function {}({}): {pascal}Output",
            entry.function_name,
            params.join(", ")
        );
    }

    out
}

/// Both generated artifacts from one model, plus the cross-surface
/// consistency check.
#[derive(Clone, Debug)]
pub struct GeneratedArtifacts {
    /// Runtime binding set.
    pub bindings: crate::binding::BindingSet,
    /// Luau type stub.
    pub stub: LuauStub,
}

impl GeneratedArtifacts {
    /// Generates bindings and stub from the same model and verifies
    /// their surfaces agree.
    ///
    /// # Errors
    /// [`ScriptApiError::BindingStubMismatch`] — surfaces disagree;
    /// generator errors propagate.
    pub fn generate(model: &ScriptApiModel) -> Result<Self, ScriptApiError> {
        let bindings = crate::binding::BindingSet::generate(model)?;
        let stub = LuauStub::generate(model)?;
        let artifacts = Self { bindings, stub };
        artifacts.check_consistency()?;
        Ok(artifacts)
    }

    /// Cross-checks that bindings and stub describe the same invocable
    /// surface: same names, same capability pins, same declared
    /// input/output schema refs.
    ///
    /// # Errors
    /// [`ScriptApiError::BindingStubMismatch`] — any divergence.
    pub fn check_consistency(&self) -> Result<(), ScriptApiError> {
        let binding_map: std::collections::BTreeMap<_, _> = self
            .bindings
            .entries
            .iter()
            .map(|e| {
                (
                    e.name.clone(),
                    (
                        e.capability.clone(),
                        e.input_schema.clone(),
                        e.output_schema.clone(),
                    ),
                )
            })
            .collect();
        let stub_map: std::collections::BTreeMap<_, _> = self
            .stub
            .entries
            .iter()
            .map(|e| {
                (
                    e.name.clone(),
                    (
                        e.capability.clone(),
                        e.input_schema.clone(),
                        e.output_schema.clone(),
                    ),
                )
            })
            .collect();
        if binding_map != stub_map {
            return Err(ScriptApiError::BindingStubMismatch {
                detail: format!(
                    "binding entries {} vs stub entries {}",
                    binding_map.len(),
                    stub_map.len()
                ),
            });
        }
        Ok(())
    }
}
