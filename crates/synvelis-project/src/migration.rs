//! The single versioned migration path for project snapshots.
//!
//! `STEPS` is an ordered registry of `v → v+1` migrations: schema
//! upgrades from SF-01 (this step), ALIB-02 (#86) and VCS-01 (#92) plug
//! into the same ladder instead of growing parallel readers/stores.
//! Every migration is a *read-only projection*: the source snapshot
//! bytes, HEAD and assets are never rewritten — the migrated project is
//! validated and published as a *new* snapshot by the next save.

use crate::{
    control::{AutomationTimeScope, ControlOwner, ParameterRole, master_fader_target},
    error::Error,
    id::{ChainId, RoutingConnectionId, SemanticPortId, StripId, TrackId},
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
