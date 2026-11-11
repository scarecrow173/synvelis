//! The single versioned migration path for project snapshots.
//!
//! `STEPS` is an ordered registry of `v → v+1` migrations: schema
//! upgrades from SF-01 (this step), ALIB-02 (#86) and VCS-01 (#92) plug
//! into the same ladder instead of growing parallel readers/stores.
//! Every migration is a *read-only projection*: the source snapshot
//! bytes, HEAD and assets are never rewritten — the migrated project is
//! validated and published as a *new* snapshot by the next save.

use crate::{
    control::{
        AutomationTimeScope, ControlOwner, ParameterRole, STRIP_SEED_ROLES, master_channel_raw,
        master_fader_target, strip_channel_target,
    },
    error::Error,
    id::{ChainId, ParameterTargetId, RoutingConnectionId, SemanticPortId, StripId, TrackId},
    project::Project,
    routing::{
        ChannelStrip, ConnectionKind, PortBinding, PortDirection, PortOwner, PortRole,
        ProcessorChain, RoutingConnection, RoutingModel, SemanticPort, SignalDomain, StripOwner,
        is_issuable_raw,
    },
    store::SCHEMA_VERSION,
};

/// One ordered `v → v+1` schema upgrade. `migrate` must be a pure,
/// deterministic projection of the source project.
struct MigrationStep {
    /// The source schema version this step upgrades.
    from: u32,
    /// The projection itself.
    migrate: fn(Project) -> Result<Project, Error>,
}

/// The ordered migration ladder. New schema work adds steps here —
/// #17 / #86 / #92 share this registry and ordering.
const STEPS: &[MigrationStep] = &[
    MigrationStep {
        from: 5,
        migrate: migrate_v5_routing,
    },
    MigrationStep {
        from: 6,
        migrate: migrate_v6_control,
    },
    MigrationStep {
        from: 7,
        migrate: migrate_v7_bootstrap_fader,
    },
    MigrationStep {
        from: 8,
        migrate: migrate_v8_smf_documents,
    },
    MigrationStep {
        from: 9,
        migrate: migrate_v9_strip_params,
    },
    MigrationStep {
        from: 10,
        migrate: migrate_v10_asset_digests,
    },
    MigrationStep {
        from: 11,
        migrate: migrate_v11_monitoring,
    },
    MigrationStep {
        from: 12,
        migrate: migrate_v12_plugin_instances,
    },
];

/// Runs the ordered ladder from `version` to [`SCHEMA_VERSION`].
///
/// # Errors
/// [`Error::UnsupportedSchema`] when no step exists for `version`;
/// any error a step's projection raises.
pub(crate) fn migrate_to_current(version: u32, mut project: Project) -> Result<Project, Error> {
    let mut version = version;
    while version != SCHEMA_VERSION {
        let step = STEPS
            .iter()
            .find(|s| s.from == version)
            .ok_or(Error::UnsupportedSchema { found: version })?;
        project = (step.migrate)(project)?;
        version += 1;
    }
    Ok(project)
}

/// v5 → v6: project the legacy "every clip reaches master" routing into
/// the semantic model — one strip (+chain+main ports) per track and a
/// `Main` connection from each track strip's output into the master's
/// main input. External `insert_count` FX are call-context only and are
/// *not* fabricated as persisted processors.
///
/// Determinism: new stable IDs are allocated from
/// `max(live issuable max, id_watermark) + 1` in `TrackId` order, so the
/// same v5 snapshot always projects to the same v6 project. The
/// watermark advances past every issued ID.
///
/// # Errors
/// [`Error::Validation`] when the v5 body carries foreign routing state
/// or ID allocation would enter the reserved well-known range.
fn migrate_v5_routing(mut project: Project) -> Result<Project, Error> {
    if project.routing != RoutingModel::default() {
        return Err(Error::Validation(
            "v5 snapshot carries routing state it cannot own".into(),
        ));
    }
    let mut next_raw = project
        .max_issuable_id_raw()
        .unwrap_or(0)
        .max(project.id_watermark);
    let mut draw = || -> Result<u128, Error> {
        next_raw += 1;
        // The watermark must sit *above* the last drawn ID, so the last
        // issuable value still needs one free slot after it.
        if !is_issuable_raw(next_raw) || !is_issuable_raw(next_raw + 1) {
            return Err(Error::Validation(
                "migration ID allocation reached the reserved range".into(),
            ));
        }
        Ok(next_raw)
    };

    let master = project.routing.master;
    let master_strip = project.routing.buses[&master].strip;
    let master_in = project.routing.strips[&master_strip]
        .port_ids
        .iter()
        .copied()
        .find(|p| {
            project.routing.ports[p].direction == PortDirection::Input
                && project.routing.ports[p].role == PortRole::Main
        })
        .ok_or_else(|| Error::Validation("master strip lacks a main input".into()))?;

    let track_ids: Vec<TrackId> = project.tracks.keys().copied().collect();
    for track_id in track_ids {
        let strip = StripId::new(draw()?);
        let chain = ChainId::new(draw()?);
        let port_in = SemanticPortId::new(draw()?);
        let port_out = SemanticPortId::new(draw()?);
        let conn = RoutingConnectionId::new(draw()?);
        project.routing.chains.insert(
            chain,
            ProcessorChain {
                id: chain,
                processors: Vec::new(),
            },
        );
        project.routing.strips.insert(
            strip,
            ChannelStrip {
                id: strip,
                owner: StripOwner::Track(track_id),
                chain,
                port_ids: vec![port_in, port_out],
            },
        );
        project.routing.ports.insert(
            port_in,
            SemanticPort {
                id: port_in,
                owner: PortOwner::Strip(strip),
                direction: PortDirection::Input,
                role: PortRole::Main,
                domain: SignalDomain::Audio,
                binding: PortBinding::Internal,
                channels: crate::routing::DEFAULT_PORT_CHANNELS,
            },
        );
        project.routing.ports.insert(
            port_out,
            SemanticPort {
                id: port_out,
                owner: PortOwner::Strip(strip),
                direction: PortDirection::Output,
                role: PortRole::Main,
                domain: SignalDomain::Audio,
                binding: PortBinding::Internal,
                channels: crate::routing::DEFAULT_PORT_CHANNELS,
            },
        );
        project.routing.connections.insert(
            conn,
            RoutingConnection {
                id: conn,
                source: port_out,
                destination: master_in,
                kind: ConnectionKind::Main,
                enabled: true,
                channel_map: None,
            },
        );
    }
    // `id_watermark` names the *next* ID the generator will issue —
    // one past the last draw (the oracle rejects a watermark that does
    // not strictly exceed every live ID).
    project.id_watermark = (next_raw + 1).max(project.id_watermark);
    Ok(project)
}

/// v6 → v7: project the legacy `ControlScope` field into the split
/// `ControlOwner` + `AutomationTimeScope` + `ParameterRole` binding
/// (issue #121). The v6 body deserializes the owner through the
/// legacy `scope` tag into `ControlOwner`; this step then derives the
/// evaluation coordinate strictly: clip/note owners keep their local
/// coordinates, every other owner gets `ProjectMusical`, and every
/// role is `Generic`. Deterministic — allocates no IDs and leaves
/// `id_watermark` untouched.
///
/// # Errors
/// [`Error::Validation`] when the v6 body carries a `Strip`/`Send`/
/// `Processor` owner — those owners did not exist in v6, so the body
/// is foreign state this projection cannot own (same rule the v5→v6
/// step applies to pre-existing routing state).
fn migrate_v6_control(mut project: Project) -> Result<Project, Error> {
    let foreign = |owner: ControlOwner| {
        matches!(
            owner,
            ControlOwner::Strip(_) | ControlOwner::Send(_) | ControlOwner::Processor(_)
        )
    };
    for target in project.control.targets.values_mut() {
        if foreign(target.owner) {
            return Err(Error::Validation(
                "v6 snapshot carries a non-legacy control owner".into(),
            ));
        }
        target.role = ParameterRole::Generic;
        target.time_scope = match target.owner {
            ControlOwner::Clip(_) => AutomationTimeScope::ClipLocal,
            ControlOwner::Note(_) => AutomationTimeScope::NoteLocal,
            _ => AutomationTimeScope::ProjectMusical,
        };
    }
    for source in project.control.sources.values_mut() {
        if foreign(source.owner) {
            return Err(Error::Validation(
                "v6 snapshot carries a non-legacy control owner".into(),
            ));
        }
    }
    Ok(project)
}

/// v7 → v8: seed the bootstrap master-strip fader `ParameterTarget`
/// (synvelis#185). v7 bodies predate the target, so the well-known ID
/// is absent; `entry().or_insert` makes the step idempotent for a body
/// that already carries it. Allocates no IDs — the target's raw value
/// sits in the reserved well-known range — and leaves `id_watermark`
/// untouched. The inserted entity is the same canonical target
/// [`crate::ControlModel::bootstrap`] writes, so a migrated project is
/// indistinguishable from a fresh one.
///
/// A v7 body may already carry an *issued* [`ParameterRole::Fader`]
/// target on the master strip — legal then, since the singleton rule
/// allowed the first one and nothing seeded a bootstrap target. That
/// target is the project's mixer gain, so the step re-keys it onto
/// the well-known ID — preserving its base value, automation lanes,
/// modulation assignments, and send bindings — instead of writing a
/// second `Fader` the singleton rule would reject at validation.
///
/// # Errors
/// [`Error::Validation`] when the v7 body lacks the bootstrap master
/// bus — every v6+ project carries it, so its absence is foreign
/// state this projection cannot own.
fn migrate_v7_bootstrap_fader(mut project: Project) -> Result<Project, Error> {
    let master_strip = project
        .routing
        .buses
        .get(&project.routing.master)
        .ok_or_else(|| Error::Validation("v7 snapshot lacks the master bus".into()))?
        .strip;
    let canonical = master_fader_target(master_strip);
    if project.control.targets.contains_key(&canonical.id) {
        return Ok(project);
    }
    let issued = project
        .control
        .targets
        .iter()
        .find(|(_, t)| {
            t.owner == ControlOwner::Strip(master_strip) && t.role == ParameterRole::Fader
        })
        .map(|(id, _)| *id);
    if let Some(old) = issued
        && let Some(mut target) = project.control.targets.remove(&old)
    {
        target.id = canonical.id;
        project.control.targets.insert(canonical.id, target);
        for lane in project.control.lanes.values_mut() {
            if lane.target == old {
                lane.target = canonical.id;
            }
        }
        for assignment in project.control.assignments.values_mut() {
            if assignment.target == old {
                assignment.target = canonical.id;
            }
            if assignment.depth_source == Some(old) {
                assignment.depth_source = Some(canonical.id);
            }
        }
        for send in project.routing.sends.values_mut() {
            if send.gain_target == Some(old) {
                send.gain_target = Some(canonical.id);
            }
        }
        return Ok(project);
    }
    project.control.targets.insert(canonical.id, canonical);
    Ok(project)
}

/// v8 → v9: add `Project::smf_documents` (issue #126). The field is
/// `#[serde(default)]`, so a v8 body already deserializes with an
/// empty map — no projection is needed and none is fabricated.
/// Deterministic — allocates no IDs and leaves `id_watermark`
/// untouched.
///
/// # Errors
/// Never fails: the step is the identity projection. The `Result`
/// shape keeps the ladder uniform.
#[allow(clippy::unnecessary_wraps)]
fn migrate_v8_smf_documents(project: Project) -> Result<Project, Error> {
    Ok(project)
}

/// v9 → v10: seed the strip channel parameter set (issue #122) —
/// every strip carries the bit-exact-safe `Fader`/`Mute`/`Solo`
/// [`ParameterTarget`]s under the canonical defaults of
/// [`strip_channel_target`] (`Pan` registers on first use — see
/// [`STRIP_SEED_ROLES`]). Strips created before registration lack
/// them; the step seeds only the *missing* roles so a user-authored
/// target of the same role is never duplicated (the singleton rule
/// would reject it anyway).
///
/// The master strip's channel targets live in the reserved
/// well-known range — the step inserts those directly without
/// drawing IDs; every other strip's targets draw from
/// `max(live issuable max, id_watermark) + 1` in `StripId` then
/// [`STRIP_SEED_ROLES`] order, so the same v9 snapshot always
/// projects to the same v10 project. `id_watermark` advances past
/// every drawn ID.
///
/// # Errors
/// [`Error::Validation`] when ID allocation would enter the reserved
/// well-known range.
fn migrate_v9_strip_params(mut project: Project) -> Result<Project, Error> {
    let master_strip = project
        .routing
        .buses
        .get(&project.routing.master)
        .map(|bus| bus.strip);
    let mut next_raw = project
        .max_issuable_id_raw()
        .unwrap_or(0)
        .max(project.id_watermark);
    let strip_ids: Vec<StripId> = project.routing.strips.keys().copied().collect();
    let first_raw = next_raw;
    for strip in strip_ids {
        for role in STRIP_SEED_ROLES {
            let exists = project
                .control
                .targets
                .values()
                .any(|t| t.owner == ControlOwner::Strip(strip) && t.role == role);
            if exists {
                continue;
            }
            if Some(strip) == master_strip
                && let Some(raw) = master_channel_raw(role)
            {
                let target = strip_channel_target(ParameterTargetId::new(raw), strip, role);
                project.control.targets.insert(target.id, target);
                continue;
            }
            next_raw += 1;
            // The watermark must sit *above* the last drawn ID, so the
            // last issuable value still needs one free slot after it.
            if !is_issuable_raw(next_raw) || !is_issuable_raw(next_raw + 1) {
                return Err(Error::Validation(
                    "migration ID allocation reached the reserved range".into(),
                ));
            }
            let id = ParameterTargetId::new(next_raw);
            let target = strip_channel_target(id, strip, role);
            project.control.targets.insert(id, target);
        }
    }
    // Advance the watermark only when IDs were actually drawn —
    // `next_raw` starts at `max(max_issuable, watermark)`, so an
    // unconditional `+1` would skip an ID on a project whose strips
    // are all well-known (e.g. master-only).
    if next_raw > first_raw {
        project.id_watermark = (next_raw + 1).max(project.id_watermark);
    }
    Ok(project)
}

/// v10 → v11: the typed asset-digest contract (synvelis#86 / ALIB-02).
///
/// Persisted digest strings — `Asset.digest`, `Source.content_digest`,
/// `AppliedImport.content_hash` — are preserved **verbatim**: legacy
/// CRC-fold digests and opaque producer formats are never rewritten
/// and never promoted to SHA-256 without byte-level verification
/// (classification lives in [`crate::project::AssetDigest`]). What
/// the step enforces is the contract's integrity floor — every digest
/// is non-empty and every applied-import receipt names live asset and
/// source entities — the same checks [`Project::validate_references`]
/// applies to current bodies, so a migrated project can never carry
/// state the writer itself would reject. Deterministic — allocates
/// no IDs and leaves `id_watermark` untouched.
///
/// # Errors
/// [`Error::Validation`]/[`Error::NotFound`] from
/// [`Project::validate_digests`].
fn migrate_v10_asset_digests(project: Project) -> Result<Project, Error> {
    project.validate_digests()?;
    Ok(project)
}

/// v11 → v12: add `Project::monitoring` — authored Arm/Monitor/
/// channel-map/lane declarations and punch marks (synvelis#117).
/// v11 bodies predate the monitor projection and carry no such state,
/// so the serde `#[serde(default)]` on the field already yields the
/// correct empty value; the step exists to make the version advance
/// explicit and audited on the shared ladder. Deterministic —
/// allocates no IDs and leaves `id_watermark` untouched.
///
/// # Errors
/// Never fails: the step is the identity projection. The `Result`
/// shape keeps the ladder uniform.
#[allow(clippy::unnecessary_wraps)]
fn migrate_v11_monitoring(project: Project) -> Result<Project, Error> {
    Ok(project)
}

/// v12 → v13: add `Project::plugin_instances` — the persisted
/// plugin-instance identity store (Issue #101, PC-C4-05). v12 bodies
/// predate the store, so the serde `#[serde(default)]` yields the
/// empty watermark-1 state; this step backfills one identity per
/// `Resolved` processor so every migrated project satisfies the
/// v13 oracle (`Resolved` ⇔ exactly one entry).
///
/// Determinism: processors are visited in `ProcessorId` order
/// (`BTreeMap` iteration) and ids issue from watermark 1 upward, so
/// the same v12 body always projects to the same v13 project.
/// `Unresolved` processors get no entry; `id_watermark` (the entity
/// domain) is untouched — the instance watermark is an independent
/// u64 domain.
///
/// # Errors
/// [`Error::Validation`] only on u64 watermark exhaustion
/// (unreachable); propagated from
/// [`crate::plugin_instance::PluginInstanceIdentities::sync_processor`].
fn migrate_v12_plugin_instances(mut project: Project) -> Result<Project, Error> {
    // Disjoint field borrows: the store mutates while the processor
    // map is only read.
    for processor in project.routing.processors.values() {
        project.plugin_instances.sync_processor(processor)?;
    }
    Ok(project)
}
