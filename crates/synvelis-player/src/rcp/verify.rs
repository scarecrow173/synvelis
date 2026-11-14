//! Plugin availability verification — turns authored
//! [`PluginDeclaration`]s plus [`PluginCapabilitySnapshot`] facts
//! into typed [`PluginVerdict`]s.
//!
//! This is the "plugin identity/availability/state restore/port
//! layout/host capability verification" stage of the spec pipeline.
//! It is deliberately *pure*: every verdict is derived from pinned
//! inputs, never from host probing, and every failure mode maps to
//! an explicit verdict so the selector can fail closed with a
//! machine-readable reason instead of guessing plugin state.

use std::collections::BTreeSet;

use ruviform_graph::{Direction, SignalDomain};

use super::model::{
    AccessState, DeclaredPort, PluginCapabilitySnapshot, PluginDeclaration, StateRestoreSupport,
};

/// Typed executability verdict for one plugin declaration.
///
/// Ordering matters for evidence: `Live` is the only verdict that
/// permits live execution. Every other variant names the concrete
/// missing/unavailable fact — the resolver must never collapse them
/// into a generic "missing plugin" because the remediation differs
/// (install vs. restore state vs. substitute render vs. block).
#[derive(Clone, Debug, PartialEq)]
pub enum PluginVerdict {
    /// Plugin verified usable live: installed, accessible, format
    /// supported, version/digest evidence consistent, port layout
    /// matches, state restore verified, automation identity
    /// complete.
    Live,
    /// No environment entry for the plugin key — binary not
    /// installed or never scanned.
    Missing,
    /// Plugin found but the environment denies access (ACL,
    /// quarantine, sandbox).
    Denied {
        /// Machine-readable denial detail.
        reason: String,
    },
    /// Host does not support the plugin format at all.
    UnsupportedFormat,
    /// Environment resolved a different version or binary digest
    /// than the project pinned — identity drift, fail closed.
    IdentityDrift {
        /// Which evidence field drifted.
        field: IdentityField,
        /// Authored expectation.
        expected: String,
        /// Environment-resolved value.
        found: String,
    },
    /// Advertised port layout differs from the authored
    /// declaration — substitution evidence cannot be trusted.
    PortLayoutMismatch {
        /// Declared ports absent from the environment layout.
        missing: Vec<DeclaredPort>,
        /// Environment ports not present in the declaration.
        extra: Vec<DeclaredPort>,
    },
    /// Opaque plugin state cannot be restored (status `Missing`/
    /// `Unknown`, capture quality insufficient, or environment
    /// reports no verified restore path).
    StateUnknown,
    /// State exists but the host cannot restore it — distinct from
    /// `StateUnknown` (evidence absent) because a substitute-render
    /// decision may treat verified-state plugins differently.
    StateUnsupported,
    /// Automation/parameter identities referenced by the project do
    /// not resolve inside the declared layout — semantic editing is
    /// also blocked for this node.
    AutomationIdentityIncomplete,
}

/// Which identity evidence field drifted.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IdentityField {
    /// Version string.
    Version,
    /// Binary content digest.
    BinaryDigest,
}

/// Verifies one plugin declaration against the capability
/// snapshot. Pure and deterministic; order of checks is fixed so
/// the *first* failing fact wins (documented evidence priority:
/// presence → access → format → identity → layout → automation
/// identity → state restore).
#[must_use]
pub fn verify_plugin(decl: &PluginDeclaration, caps: &PluginCapabilitySnapshot) -> PluginVerdict {
    let Some(env) = caps.get(&decl.identity) else {
        return PluginVerdict::Missing;
    };
    if !env.installed {
        return PluginVerdict::Missing;
    }
    if let AccessState::Denied { reason } = &env.access {
        return PluginVerdict::Denied {
            reason: reason.clone(),
        };
    }
    if !env.format_supported {
        return PluginVerdict::UnsupportedFormat;
    }
    if let Some(version) = &env.resolved_version
        && *version != decl.identity.version
    {
        return PluginVerdict::IdentityDrift {
            field: IdentityField::Version,
            expected: decl.identity.version.clone(),
            found: version.clone(),
        };
    }
    if let (Some(expected), Some(found)) =
        (&decl.identity.binary_digest, &env.resolved_binary_digest)
        && expected != found
    {
        return PluginVerdict::IdentityDrift {
            field: IdentityField::BinaryDigest,
            expected: expected.clone(),
            found: found.clone(),
        };
    }
    let layout_ok = match &env.advertised_ports {
        Some(advertised) => layouts_match(&decl.declared_ports, advertised),
        None => false,
    };
    if !layout_ok {
        let (missing, extra) = match &env.advertised_ports {
            Some(advertised) => layout_diff(&decl.declared_ports, advertised),
            None => (decl.declared_ports.clone(), Vec::new()),
        };
        return PluginVerdict::PortLayoutMismatch { missing, extra };
    }
    if !decl.automation_identity_complete {
        return PluginVerdict::AutomationIdentityIncomplete;
    }
    match env.state_restore {
        StateRestoreSupport::Verified => {
            if decl.state.status != synvelis_project::OpaqueStateStatus::Known
                || decl.state.digest.is_none()
                || !decl.state.quality.exact_eligible()
            {
                return PluginVerdict::StateUnknown;
            }
            PluginVerdict::Live
        }
        StateRestoreSupport::Unsupported => PluginVerdict::StateUnsupported,
        StateRestoreSupport::Unknown => PluginVerdict::StateUnknown,
    }
}

/// Ordinal layout key — ruviform port enums deliberately lack `Ord`
/// (they are contract enums, not sort keys), so comparison runs on
/// this normalized projection instead.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct LayoutKey {
    direction: u8,
    domain: u8,
    name: String,
}

fn port_key(port: &DeclaredPort) -> LayoutKey {
    LayoutKey {
        direction: match port.direction {
            Direction::Input => 0,
            Direction::Output => 1,
            _ => 2,
        },
        domain: match port.domain {
            SignalDomain::Audio => 0,
            SignalDomain::TimedEvent => 1,
            SignalDomain::Control => 2,
            _ => 3,
        },
        name: port.name.clone(),
    }
}

fn layouts_match(declared: &[DeclaredPort], advertised: &[DeclaredPort]) -> bool {
    let mut a: Vec<_> = declared.iter().map(port_key).collect();
    let mut b: Vec<_> = advertised.iter().map(port_key).collect();
    a.sort();
    b.sort();
    a == b
}

fn layout_diff(
    declared: &[DeclaredPort],
    advertised: &[DeclaredPort],
) -> (Vec<DeclaredPort>, Vec<DeclaredPort>) {
    let advertised_keys: BTreeSet<_> = advertised.iter().map(port_key).collect();
    let declared_keys: BTreeSet<_> = declared.iter().map(port_key).collect();
    let missing = declared
        .iter()
        .filter(|p| !advertised_keys.contains(&port_key(p)))
        .cloned()
        .collect();
    let extra = advertised
        .iter()
        .filter(|p| !declared_keys.contains(&port_key(p)))
        .cloned()
        .collect();
    (missing, extra)
}

/// Verifies every plugin node of a snapshot, returning verdicts
/// keyed by node id. Non-plugin nodes are not included — callers
/// treat their absence as implicitly `Live`.
#[must_use]
pub fn verify_snapshot(
    nodes: &std::collections::BTreeMap<super::model::RcpNodeId, super::model::ResolvedNode>,
    caps: &PluginCapabilitySnapshot,
) -> std::collections::BTreeMap<super::model::RcpNodeId, PluginVerdict> {
    nodes
        .iter()
        .filter_map(|(id, node)| {
            node.plugin
                .as_ref()
                .map(|decl| (*id, verify_plugin(decl, caps)))
        })
        .collect()
}
