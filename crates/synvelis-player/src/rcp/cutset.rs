//! Typed cutset validation — decides whether a candidate
//! [`RenderCandidate`] may substitute for a node region without
//! violating the spec's boundary contract.
//!
//! The rules implemented here are the checkable subset of spec
//! §Subgraph cutset rules that a headless oracle can verify:
//!
//! - every enabled boundary output edge must be covered by an
//!   artifact output keyed at the *same region-side source port*,
//!   with matching signal domain, payload format, and effective
//!   latency/PDC;
//! - every enabled boundary input edge must be pinned into the
//!   artifact (`pinned_inputs`), which is what prevents double-fire
//!   of send/sidechain taps and double-add of summed paths;
//! - required boundary outputs (device/record/audition taps) must
//!   remain supplied after the cut;
//! - live input ports whose multiplicity minimum relied on
//!   region-produced feeds must not be left dangling;
//! - Control-domain boundary edges can only be covered by
//!   Control-domain artifact outputs — audio never fabricates
//!   control;
//! - a source port with parallel edges into the same destination
//!   port would double the signal — rejected as `DoubleFeed`.
//!
//! Cycle/zero-delay rules are upstream `ruviform_graph` validator
//! concerns; this module assumes the snapshot is already a valid
//! typed graph.

use std::collections::{BTreeMap, BTreeSet};

use ruviform_graph::SignalDomain;

use super::model::{ArtifactOutput, PortEndpoint, RenderCandidate, ResolvedGraphSnapshot};

/// Typed rejection reason for a candidate cutset. Every variant
/// names the violated boundary rule so fault-matrix evidence can
/// distinguish "no artifact" from "artifact exists but wrong".
#[derive(Clone, Debug, PartialEq)]
pub enum CutsetRejection {
    /// Region contains a `Sink` node — sinks are terminals, not
    /// substitutable content.
    SinkInRegion {
        /// The offending node.
        node: super::model::RcpNodeId,
    },
    /// Region references a node id absent from the snapshot.
    UnknownNode {
        /// The offending node id.
        node: super::model::RcpNodeId,
    },
    /// A boundary output edge has no artifact output at its
    /// region-side source port.
    UncoveredOutput {
        /// Region-side source endpoint that must be reproduced.
        source: PortEndpoint,
        /// Live-side destination the edge feeds.
        destination: PortEndpoint,
        /// Signal domain that must be reproduced.
        domain: SignalDomain,
    },
    /// Artifact output exists at the source port but reproduces a
    /// different signal domain.
    DomainMismatch {
        /// Region-side source endpoint.
        source: PortEndpoint,
        /// Domain the live graph expects.
        expected: SignalDomain,
        /// Domain the artifact declares.
        found: SignalDomain,
    },
    /// Audio artifact output format differs from the replaced
    /// port's format — channel maps cannot repair rate/layout/
    /// encoding changes.
    FormatMismatch {
        /// Region-side source endpoint.
        source: PortEndpoint,
        /// Expected payload format.
        expected: String,
        /// Artifact-declared payload format.
        found: String,
    },
    /// Artifact latency/PDC does not match the boundary edge's
    /// expected effective latency.
    LatencyMismatch {
        /// Region-side source endpoint.
        source: PortEndpoint,
        /// Expected effective latency in frames.
        expected: u64,
        /// Artifact-declared latency in frames.
        found: u64,
    },
    /// A Control-domain boundary edge would be covered by a
    /// non-Control artifact output — control misdelivery.
    ControlMisdelivery {
        /// Region-side source endpoint.
        source: PortEndpoint,
    },
    /// Boundary input edge whose signal was not pinned into the
    /// artifact — reusing the render would double-fire or
    /// double-add against the live edge.
    UnpinnedInput {
        /// Live-side source endpoint feeding the region.
        source: PortEndpoint,
        /// Region-side port that consumed it.
        destination: PortEndpoint,
    },
    /// Same as [`Self::UnpinnedInput`] but the edge kind is
    /// `Sidechain` — reported distinctly because unpinned
    /// sidechain reuse is explicitly forbidden even when other
    /// inputs are pinned.
    SidechainUnpinned {
        /// Live-side source endpoint.
        source: PortEndpoint,
        /// Region-side port.
        destination: PortEndpoint,
    },
    /// A required boundary output port would lose all feeds after
    /// the cut.
    RequiredOutputUncovered {
        /// The required input port.
        port: PortEndpoint,
    },
    /// A live input port's enforced multiplicity minimum relied on
    /// region-produced feeds the artifact does not reproduce.
    DanglingInput {
        /// The live input port left under-fed.
        port: PortEndpoint,
        /// Minimum feeds required.
        required: u16,
        /// Feeds remaining after substitution.
        found: usize,
    },
    /// The same source port feeds the same destination port via
    /// multiple edges — substitution would sum the signal twice.
    DoubleFeed {
        /// Source endpoint.
        source: PortEndpoint,
        /// Destination endpoint.
        destination: PortEndpoint,
    },
}

/// Result of validating one candidate cutset.
#[derive(Clone, Debug)]
pub struct CutsetReport {
    /// Candidate artifact id (evidence).
    pub artifact_id: String,
    /// Region node set evaluated (== `candidate.node_set`).
    pub region: BTreeSet<super::model::RcpNodeId>,
    /// `true` iff `rejections` is empty.
    pub accepted: bool,
    /// Typed rejections, deterministically ordered.
    pub rejections: Vec<CutsetRejection>,
}

fn expected_latency(
    snapshot: &ResolvedGraphSnapshot,
    edge_source: PortEndpoint,
    edge_pdc: u64,
) -> u64 {
    let node_latency = snapshot
        .node(edge_source.node)
        .map_or(0, |n| n.latency_frames);
    edge_pdc.saturating_add(node_latency)
}

fn output_covers(candidate: &RenderCandidate, source: PortEndpoint) -> Option<&ArtifactOutput> {
    candidate.outputs.iter().find(|o| o.source == source)
}

/// Validates `candidate.node_set` as a substitution region against
/// `candidate`'s declared outputs/pins. Pure; the same inputs always
/// produce the same report.
#[must_use]
pub fn validate_cutset(
    snapshot: &ResolvedGraphSnapshot,
    candidate: &RenderCandidate,
) -> CutsetReport {
    let region = &candidate.node_set;
    let mut rejections: Vec<CutsetRejection> = Vec::new();

    // Region sanity: no sinks, region nodes must exist.
    for node_id in region {
        match snapshot.node(*node_id) {
            Some(node) if node.class == super::model::NodeClass::Sink => {
                rejections.push(CutsetRejection::SinkInRegion { node: *node_id });
            }
            None => {
                rejections.push(CutsetRejection::UnknownNode { node: *node_id });
            }
            _ => {}
        }
    }

    let (boundary_outputs, boundary_inputs) = snapshot.cut_boundary(region);
    let covered_sources =
        check_boundary_outputs(snapshot, candidate, &boundary_outputs, &mut rejections);
    check_boundary_inputs(candidate, &boundary_inputs, &mut rejections);
    check_live_feeds(snapshot, region, &covered_sources, &mut rejections);
    check_double_feeds(snapshot, region, &mut rejections);

    // Deterministic rejection order: sort by Debug for stable
    // evidence (variants carry endpoint data; Debug is total).
    rejections.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));
    rejections.dedup();

    CutsetReport {
        artifact_id: candidate.artifact_id.clone(),
        region: region.clone(),
        accepted: rejections.is_empty(),
        rejections,
    }
}

/// Boundary outputs: every edge leaving the region must be covered
/// by an artifact output at the same source port with matching
/// domain, payload format, and effective latency. Returns the set
/// of source endpoints whose coverage fully validated.
fn check_boundary_outputs(
    snapshot: &ResolvedGraphSnapshot,
    candidate: &RenderCandidate,
    boundary_outputs: &[&super::model::ResolvedEdge],
    rejections: &mut Vec<CutsetRejection>,
) -> BTreeSet<PortEndpoint> {
    let mut covered = BTreeSet::new();
    for edge in boundary_outputs {
        let source_port = snapshot
            .node(edge.from.node)
            .and_then(|n| n.port(edge.from.port))
            .map_or(SignalDomain::Audio, super::model::ResolvedPort::domain);
        let Some(output) = output_covers(candidate, edge.from) else {
            rejections.push(CutsetRejection::UncoveredOutput {
                source: edge.from,
                destination: edge.to,
                domain: source_port,
            });
            continue;
        };
        if check_output_contract(snapshot, edge, source_port, output, rejections) {
            covered.insert(edge.from);
        }
    }
    covered
}

/// Per-output contract checks shared by the boundary loop — pushes
/// rejections and returns whether the output fully matched.
fn check_output_contract(
    snapshot: &ResolvedGraphSnapshot,
    edge: &super::model::ResolvedEdge,
    source_port: SignalDomain,
    output: &ArtifactOutput,
    rejections: &mut Vec<CutsetRejection>,
) -> bool {
    let mut ok = true;
    if output.domain != source_port {
        ok = false;
        if source_port == SignalDomain::Control {
            rejections.push(CutsetRejection::ControlMisdelivery { source: edge.from });
        } else {
            rejections.push(CutsetRejection::DomainMismatch {
                source: edge.from,
                expected: source_port,
                found: output.domain,
            });
        }
    }
    let expected_format = snapshot
        .node(edge.from.node)
        .and_then(|n| n.port(edge.from.port))
        .map(|p| format!("{:?}", p.format));
    let found_format = format!("{:?}", output.format);
    if source_port == SignalDomain::Audio
        && expected_format.as_deref() != Some(found_format.as_str())
    {
        ok = false;
        rejections.push(CutsetRejection::FormatMismatch {
            source: edge.from,
            expected: expected_format.unwrap_or_default(),
            found: found_format,
        });
    }
    let expected = expected_latency(snapshot, edge.from, edge.pdc_frames);
    if output.latency_frames != expected {
        ok = false;
        rejections.push(CutsetRejection::LatencyMismatch {
            source: edge.from,
            expected,
            found: output.latency_frames,
        });
    }
    ok
}

/// Boundary inputs: every enabled edge entering the region must be
/// pinned into the artifact — otherwise the live edge would
/// double-fire alongside the baked signal.
fn check_boundary_inputs(
    candidate: &RenderCandidate,
    boundary_inputs: &[&super::model::ResolvedEdge],
    rejections: &mut Vec<CutsetRejection>,
) {
    for edge in boundary_inputs {
        if candidate.pinned_inputs.contains(&edge.to) {
            continue;
        }
        rejections.push(match edge.kind {
            super::model::EdgeKind::Sidechain => CutsetRejection::SidechainUnpinned {
                source: edge.from,
                destination: edge.to,
            },
            _ => CutsetRejection::UnpinnedInput {
                source: edge.from,
                destination: edge.to,
            },
        });
    }
}

/// Required outputs + multiplicity/dangling: declared boundary
/// requirements plus ports whose enforced multiplicity minimum is
/// non-zero must retain enough feeds after the cut. Optional
/// `min == 0` ports may legitimately have no feeds.
fn check_live_feeds(
    snapshot: &ResolvedGraphSnapshot,
    region: &BTreeSet<super::model::RcpNodeId>,
    covered_sources: &BTreeSet<PortEndpoint>,
    rejections: &mut Vec<CutsetRejection>,
) {
    let mut required_ports: BTreeMap<PortEndpoint, u16> = BTreeMap::new();
    for node in snapshot.nodes.values() {
        if region.contains(&node.id) {
            continue;
        }
        for port in &node.ports {
            if port.direction == ruviform_graph::Direction::Input
                && port.requirement_enforced()
                && port.multiplicity.min > 0
            {
                required_ports.insert(
                    PortEndpoint {
                        node: node.id,
                        port: port.id,
                    },
                    port.multiplicity.min,
                );
            }
        }
    }
    for req in &snapshot.required_outputs {
        // Keep any stricter multiplicity minimum already recorded —
        // a required boundary port is never *less* demanding than
        // its declared port contract.
        required_ports.entry(req.port).or_insert(1);
    }

    for (port, minimum) in &required_ports {
        let feeds = snapshot.feeds_after_cut(region, covered_sources, *port);
        if feeds == 0 {
            rejections.push(CutsetRejection::RequiredOutputUncovered { port: *port });
        } else if usize::from(*minimum) > feeds {
            rejections.push(CutsetRejection::DanglingInput {
                port: *port,
                required: *minimum,
                found: feeds,
            });
        }
    }
}

/// Double-feed: the same (source port → destination port) pair via
/// multiple enabled boundary edges would be summed twice by the
/// artifact output.
fn check_double_feeds(
    snapshot: &ResolvedGraphSnapshot,
    region: &BTreeSet<super::model::RcpNodeId>,
    rejections: &mut Vec<CutsetRejection>,
) {
    let mut seen: BTreeSet<(PortEndpoint, PortEndpoint)> = BTreeSet::new();
    for edge in snapshot
        .edges
        .iter()
        .filter(|e| region.contains(&e.from.node) && !region.contains(&e.to.node))
    {
        if !seen.insert((edge.from, edge.to)) {
            rejections.push(CutsetRejection::DoubleFeed {
                source: edge.from,
                destination: edge.to,
            });
        }
    }
}
