//! Canonical [`ScriptSurfaceSpec`] generation — the script naming
//! policy authority (synvelis#262).
//!
//! [`synvelis_script`] is deliberately naming-agnostic: it consumes a
//! surface spec but never invents one. This module owns the *one*
//! deterministic projection from a live registry snapshot onto script
//! API names so every host (CLI today, GUI/analyzer integration under
//! synvelis#274) exposes the same names for the same registry.
//!
//! Naming rule (fixed by this module — do not hand-copy test fixture
//! surfaces into production):
//!
//! 1. Take the capability id verbatim (e.g. `synvelis.clip.mute`).
//! 2. Strip a leading `synvelis.` prefix when present; a foreign
//!    prefix is kept (the whole id still transforms deterministically).
//! 3. Split the remainder on `.`, `-`, and `_`; drop empty segments.
//! 4. camelCase: first segment lowercased, each later segment
//!    capitalized. `synvelis.context.query` → `contextQuery`,
//!    `synvelis.project.add-note` → `projectAddNote`.
//!
//! Determinism + fail-closed collision rule: the derived name is a
//! pure function of the capability id. Two *different* capability ids
//! deriving the same name is ambiguous — generation fails with
//! [`ScriptApiError::DuplicateApiName`] rather than guessing a
//! disambiguator. A name that is not a usable Luau identifier is *not*
//! fatal here: the entry is still emitted and the model records a
//! per-entry `InvalidName` verdict (discovery honesty — one bad name
//! cannot hide the rest of the registry).

use std::collections::BTreeMap;

use synvelis_script::snapshot::RegistrySnapshot;
use synvelis_script::{
    SURFACE_SCHEMA_VERSION, ScriptApiError, ScriptSurfaceEntry, ScriptSurfaceSpec, ScriptVisibility,
};

/// Derives the script API name for one capability id — see the module
/// docs for the fixed rule.
#[must_use]
pub fn script_name_for(capability_id: &str) -> String {
    let rest = capability_id
        .strip_prefix("synvelis.")
        .unwrap_or(capability_id);
    let mut name = String::new();
    for (index, segment) in rest
        .split(['.', '-', '_'])
        .filter(|s| !s.is_empty())
        .enumerate()
    {
        let lowered: String = segment.chars().flat_map(char::to_lowercase).collect();
        let mut chars = lowered.chars();
        if index == 0 {
            name.push_str(&lowered);
        } else if let Some(first) = chars.next() {
            name.extend(first.to_uppercase());
            name.extend(chars);
        }
    }
    name
}

/// Builds the canonical surface for `snapshot`: every registered
/// capability descriptor becomes one `Public` entry pinned to its
/// exact `(id, version)` — the model/binding layers decide
/// invocability from the snapshot's availability, never from this
/// naming layer.
///
/// Entries are emitted in snapshot order (already sorted by
/// `(id, version)`); the model re-sorts canonically.
///
/// # Errors
/// [`ScriptApiError::DuplicateApiName`] — two distinct capability ids
/// derived the same script name (fail-closed: no silent disambiguation
/// policy exists).
pub fn surface_for_snapshot(
    snapshot: &RegistrySnapshot,
) -> Result<ScriptSurfaceSpec, ScriptApiError> {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let mut entries = Vec::with_capacity(snapshot.capabilities.len());
    for descriptor in &snapshot.capabilities {
        let name = script_name_for(descriptor.id.as_str());
        // Any second derivation of the same name is ambiguous —
        // including the same capability id at another version, which
        // would silently shadow one pin behind one script name.
        if seen.contains_key(name.as_str()) {
            return Err(ScriptApiError::DuplicateApiName { name });
        }
        seen.insert(name.clone(), descriptor.id.as_str().to_owned());
        // A derived name that is not a usable Luau identifier is
        // still emitted — the model's per-entry `InvalidName` verdict
        // is the authority, so one bad name cannot hide the registry.
        entries.push(ScriptSurfaceEntry {
            name,
            capability: descriptor.capability_ref(),
            visibility: ScriptVisibility::Public,
            description: None,
        });
    }
    Ok(ScriptSurfaceSpec {
        schema_version: SURFACE_SCHEMA_VERSION,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use synvelis_gateway::capability::{
        Availability, CapabilityDescriptor, CapabilityVersion, DataAccessSet, EffectSet,
        ExecutionContract, OperationDomain, PermissionSet, TargetContract,
    };
    use synvelis_gateway::{CapabilityId, SchemaRef, SchemaVersion, ValueSchema};
    use synvelis_script::snapshot::SchemaEntry;
    use synvelis_script::surface::luau_name_status;
    use synvelis_script::{SNAPSHOT_SCHEMA_VERSION, ScriptSchema, sha256_hex};

    fn descriptor(id: &str) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new(id).unwrap(),
            version: CapabilityVersion::new(1, 0),
            domain: OperationDomain::Query,
            input_schema: SchemaRef::new("synvelis.test.in", 1, 0),
            output_schema: SchemaRef::new("synvelis.test.out", 1, 0),
            target: TargetContract::None,
            effect_bound: EffectSet::new(),
            required_permissions: PermissionSet::new(),
            data_access: DataAccessSet::new(),
            execution: ExecutionContract::Sync,
            availability: Availability::Available,
        }
    }

    fn snapshot_of(ids: &[&str]) -> RegistrySnapshot {
        RegistrySnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            capabilities: ids.iter().map(|id| descriptor(id)).collect(),
            schemas: vec![
                SchemaEntry {
                    name: "synvelis.test.in".to_owned(),
                    version: SchemaVersion::new(1, 0),
                    schema: ScriptSchema::from_value_schema(&ValueSchema::Bool),
                },
                SchemaEntry {
                    name: "synvelis.test.out".to_owned(),
                    version: SchemaVersion::new(1, 0),
                    schema: ScriptSchema::from_value_schema(&ValueSchema::Bool),
                },
            ],
        }
    }

    #[test]
    fn names_derive_deterministically() {
        assert_eq!(script_name_for("synvelis.clip.mute"), "clipMute");
        assert_eq!(script_name_for("synvelis.context.query"), "contextQuery");
        assert_eq!(
            script_name_for("synvelis.project.add-note"),
            "projectAddNote"
        );
        assert_eq!(script_name_for("synvelis.job.get"), "jobGet");
        // Foreign prefix is kept, still deterministic.
        assert_eq!(script_name_for("mcp.tool.call"), "mcpToolCall");
        // Separator variants collapse to the same camelCase.
        assert_eq!(script_name_for("synvelis.a-b_c.d"), "aBCD");
    }

    #[test]
    fn surface_maps_every_capability_with_exact_pin() {
        let snapshot = snapshot_of(&["synvelis.context.query", "synvelis.clip.mute"]);
        let surface = surface_for_snapshot(&snapshot).unwrap();
        assert_eq!(surface.entries.len(), 2);
        let query = surface
            .entries
            .iter()
            .find(|e| e.name == "contextQuery")
            .unwrap();
        assert_eq!(query.capability.id.as_str(), "synvelis.context.query");
        assert_eq!(query.capability.version.major, 1);
        assert_eq!(query.visibility, ScriptVisibility::Public);
        for entry in &surface.entries {
            luau_name_status(&entry.name).unwrap();
        }
    }

    #[test]
    fn name_collision_fails_closed() {
        // `a-b` and `a_b` both derive `ab`… wait: a-b → ["a","b"] → "aB",
        // a_b → same → collision.
        let snapshot = snapshot_of(&["synvelis.a-b", "synvelis.a_b"]);
        let err = surface_for_snapshot(&snapshot).unwrap_err();
        assert!(matches!(err, ScriptApiError::DuplicateApiName { .. }));
    }

    #[test]
    fn same_id_multiple_versions_still_collide() {
        // Same derived name from the same id at two versions is still
        // ambiguous for callers — fail closed, never pick one.
        let mut snapshot = snapshot_of(&["synvelis.context.query"]);
        let mut second = descriptor("synvelis.context.query");
        second.version = CapabilityVersion::new(1, 1);
        snapshot.capabilities.push(second);
        let err = surface_for_snapshot(&snapshot).unwrap_err();
        assert!(matches!(err, ScriptApiError::DuplicateApiName { .. }));
    }

    #[test]
    fn invalid_identifier_names_are_emitted_for_model_verdicts() {
        // A name the Luau rules reject is still surfaced — the model
        // records InvalidName rather than the generator dropping it.
        let snapshot = snapshot_of(&["synvelis.9lives.x"]);
        let surface = surface_for_snapshot(&snapshot).unwrap();
        assert_eq!(surface.entries.len(), 1);
        assert!(luau_name_status(&surface.entries[0].name).is_err());
    }

    #[test]
    fn surface_digest_is_stable() {
        let snapshot = snapshot_of(&["synvelis.context.query"]);
        let a = surface_for_snapshot(&snapshot).unwrap();
        let b = surface_for_snapshot(&snapshot).unwrap();
        assert_eq!(
            sha256_hex(serde_json::to_string(&a).unwrap().as_bytes()),
            sha256_hex(serde_json::to_string(&b).unwrap().as_bytes())
        );
    }
}
