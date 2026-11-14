//! Execution selection — the RCP-04 resolver proper.
//!
//! [`ExecutionSelector::select`] runs the spec's non-RT pipeline on
//! pinned inputs: verify plugins → find verified render candidates
//! → solve typed cutsets → choose live regions / substitutions /
//! skips under the intent's policy → emit [`ExecutionSelection`]
//! plus a [`ReconciliationReceipt`] carrying every pin and every
//! evaluated candidate as evidence.
//!
//! Ordering is correctness-first, then editability, RT resources,
//! stability — implemented as: a candidate must pass *all* gates
//! (intent eligibility, access, snapshot pins, live-region
//! protection, cutset validation) before scoring, and scoring only
//! breaks ties among already-correct candidates.
//!
//! Nothing here touches the filesystem, allocates unboundedly per
//! node, or runs on the RT path — this is the async non-RT prepare
//! decision layer. Actual Ruviform compile/prepare/publish is the
//! caller's job via [`super::reconcile`].

use std::collections::{BTreeMap, BTreeSet};

use synvelis_project::{
    BuildMode, LOWERING_POLICY_VERSION, ProjectRevision, RoutingBuildKey, RoutingFingerprint,
};

use super::cutset::{CutsetReport, validate_cutset};
use super::intent::ExecutionIntent;
use super::model::{
    AccessState, ArtifactEligibility, RcpNodeId, RenderCandidate, RenderCandidateIndex,
    ReproducibilityClass, ResolvedGraphSnapshot,
};
use super::verify::{PluginVerdict, verify_snapshot};

/// Why an unexecutable node could not be resolved.
#[derive(Clone, Debug, PartialEq)]
pub enum UnresolvedReason {
    /// No verified artifact covers the node and skipping is not
    /// permitted (exact intent, required live region, or required
    /// output would dangle).
    NoEligibleSubstitution,
    /// Every covering candidate failed cutset validation; the
    /// reports are in the receipt's `candidate_attempts`.
    CutsetRejected,
    /// Plugin verdict detail carried for evidence.
    PluginVerdict(PluginVerdict),
}

/// One node the selection could not make executable.
#[derive(Clone, Debug, PartialEq)]
pub struct UnresolvedNode {
    /// The node.
    pub node: RcpNodeId,
    /// Why it is unresolved.
    pub reason: UnresolvedReason,
}

/// A node deliberately removed from execution (preview opt-in
/// only) — its outputs must have been verified non-required.
#[derive(Clone, Debug, PartialEq)]
pub struct SkippedNode {
    /// The node.
    pub node: RcpNodeId,
    /// The plugin verdict that made it unexecutable.
    pub verdict: PluginVerdict,
}

/// One accepted render substitution — a validated cutset plus the
/// artifact that fills it.
#[derive(Clone, Debug)]
pub struct Substitution {
    /// Exact node set replaced.
    pub nodes: BTreeSet<RcpNodeId>,
    /// Artifact identity.
    pub artifact_id: String,
    /// Content hash of the artifact.
    pub content_hash: String,
    /// Whether the artifact is preview-class (non-equivalence
    /// evidence — `EligiblePreview`/`PreviewOnly`/`ReplayOfVerifiedArtifact`
    /// under a preview intent).
    pub preview_class: bool,
}

/// One maximal connected region of live nodes — the editable/
/// executable remainder after substitutions and skips.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveRegion {
    /// Member node ids.
    pub nodes: BTreeSet<RcpNodeId>,
}

/// Top-level verdict of a selection.
#[derive(Clone, Debug, PartialEq)]
pub enum SelectionVerdict {
    /// Every required output is supplied by verified-exact means —
    /// the only verdict compatible with an Exact claim.
    Exact,
    /// Selection succeeded but carries explicit non-equivalence —
    /// preview-class artifacts, skipped nodes, or degraded
    /// unresolved nodes outside the required scope. Only reachable
    /// when the intent opted in.
    NonEquivalent,
    /// Fail-closed: required outputs cannot be supplied or a
    /// required-live region contains unexecutable nodes.
    Blocked,
}

/// The resolver output — `ExecutionSelection` of the Issue #293
/// contract.
#[derive(Clone, Debug)]
pub struct ExecutionSelection {
    /// The intent this selection answers.
    pub intent: ExecutionIntent,
    /// Live regions (executable remainder).
    pub live_regions: Vec<LiveRegion>,
    /// Accepted render substitutions.
    pub substitutions: Vec<Substitution>,
    /// Skipped nodes (preview opt-in only).
    pub skipped: Vec<SkippedNode>,
    /// Unresolved nodes.
    pub unresolved: Vec<UnresolvedNode>,
    /// Degraded-but-tolerated nodes (unresolved outside required
    /// scope under preview opt-in) — evidence for the non-
    /// equivalence display.
    pub degraded: Vec<RcpNodeId>,
    /// Verdict.
    pub verdict: SelectionVerdict,
    /// `true` iff `verdict == NonEquivalent` — the explicit display
    /// flag; `Exact` selections always carry `false`.
    pub non_equivalent: bool,
    /// Evidence receipt.
    pub receipt: ReconciliationReceipt,
}

/// Per-candidate evaluation evidence — every artifact considered,
/// with its eligibility/cutset outcome, so "why was this render not
/// used" is answerable from the receipt alone.
#[derive(Clone, Debug)]
pub struct CutsetAttempt {
    /// Artifact id.
    pub artifact_id: String,
    /// Region evaluated.
    pub region: BTreeSet<RcpNodeId>,
    /// Stage that rejected it, or `Accepted`.
    pub outcome: AttemptOutcome,
    /// Cutset rejections when validation ran.
    pub cutset: Option<CutsetReport>,
}

/// How far a candidate got.
#[derive(Clone, Debug, PartialEq)]
pub enum AttemptOutcome {
    /// Passed every gate.
    Accepted,
    /// Rejected by intent eligibility / access / pin checks before
    /// cutset validation.
    Ineligible {
        /// Machine-readable reason.
        reason: String,
    },
    /// Rejected by cutset validation.
    CutsetRejected,
}

/// The `ReconciliationReceipt` of the contract — pins every input
/// generation the selection depended on plus the evidence trail.
#[derive(Clone, Debug)]
pub struct ReconciliationReceipt {
    /// Snapshot identity.
    pub snapshot_id: u64,
    /// Project revision pinned.
    pub project_revision: ProjectRevision,
    /// Graph structure digest pinned.
    pub structure_digest: u64,
    /// Capability generation pinned.
    pub capability_generation: u64,
    /// Render index generation pinned.
    pub render_index_generation: u64,
    /// Plugin verdicts per plugin node.
    pub plugin_verdicts: BTreeMap<RcpNodeId, PluginVerdict>,
    /// Every candidate evaluated, in deterministic order.
    pub candidate_attempts: Vec<CutsetAttempt>,
    /// Build key for the downstream Ruviform prepare/publish step.
    pub build_key: RoutingBuildKey,
    /// Digest of the selection decisions (substitutions/skips/
    /// unresolved) — changes iff the plan shape changes.
    pub selection_digest: u64,
}

/// Stateless resolver — all inputs are explicit pins.
pub struct ExecutionSelector;

impl ExecutionSelector {
    /// Runs the full resolution pipeline. Deterministic: identical
    /// inputs produce identical selections and receipts.
    #[must_use]
    pub fn select(
        snapshot: &ResolvedGraphSnapshot,
        intent: &ExecutionIntent,
        capabilities: &super::model::PluginCapabilitySnapshot,
        renders: &RenderCandidateIndex,
    ) -> ExecutionSelection {
        let verdicts = verify_snapshot(&snapshot.nodes, capabilities);
        let unexecutable: BTreeSet<RcpNodeId> = verdicts
            .iter()
            .filter(|(_, v)| **v != PluginVerdict::Live)
            .map(|(id, _)| *id)
            .collect();

        let mut attempts: Vec<CutsetAttempt> = Vec::new();
        let (substitutions, covered, mut degraded_marks) =
            resolve_substitutions(snapshot, intent, &unexecutable, renders, &mut attempts);

        // Remaining unexecutable nodes: skip (preview opt-in) or
        // unresolved/degraded.
        let (skipped, final_unresolved, degraded) = resolve_leftovers(
            snapshot,
            intent,
            &unexecutable,
            &covered,
            &substitutions,
            &verdicts,
            &mut degraded_marks,
        );

        // Required outputs must remain fed — checked against live
        // executable nodes plus substitution coverage plus skips.
        let substituted_regions: Vec<BTreeSet<RcpNodeId>> =
            substitutions.iter().map(|s| s.nodes.clone()).collect();
        let required_ok = required_outputs_fed(
            snapshot,
            &covered,
            &substituted_regions,
            &skipped,
            &final_unresolved,
        );

        let verdict = if !final_unresolved.is_empty() || !required_ok {
            SelectionVerdict::Blocked
        } else if degraded_marks {
            SelectionVerdict::NonEquivalent
        } else {
            SelectionVerdict::Exact
        };
        let non_equivalent = verdict == SelectionVerdict::NonEquivalent;

        // Live regions: connected components of executable nodes.
        let live_regions =
            compute_live_regions(snapshot, &covered, &skipped, &final_unresolved, &degraded);

        let build_key = RoutingBuildKey {
            project_id: snapshot.project_id,
            project_revision: snapshot.project_revision,
            routing_fingerprint: RoutingFingerprint(snapshot.structure_digest),
            capability_generation: snapshot.capability_generation,
            engine_config_generation: snapshot.engine_config_generation,
            source_format_generation: snapshot.source_format_generation,
            lowering_policy_version: LOWERING_POLICY_VERSION,
            build_mode: match intent {
                ExecutionIntent::OfflineRender { .. } | ExecutionIntent::Export => {
                    BuildMode::Offline
                }
                _ => BuildMode::Realtime,
            },
            device_clock_epoch: snapshot.clock_epoch,
            runtime_instance: snapshot.runtime_instance,
        };

        let selection_digest = compute_selection_digest(
            snapshot,
            &substitutions,
            &skipped,
            &final_unresolved,
            &degraded,
            &verdict,
        );

        let receipt = ReconciliationReceipt {
            snapshot_id: snapshot.snapshot_id,
            project_revision: snapshot.project_revision,
            structure_digest: snapshot.structure_digest,
            capability_generation: snapshot.capability_generation,
            render_index_generation: renders.generation,
            plugin_verdicts: verdicts,
            candidate_attempts: attempts,
            build_key,
            selection_digest,
        };

        ExecutionSelection {
            intent: intent.clone(),
            live_regions,
            substitutions,
            skipped,
            unresolved: final_unresolved,
            degraded,
            verdict,
            non_equivalent,
            receipt,
        }
    }
}

/// Greedy set-cover substitution phase: repeatedly pick the
/// candidate covering the most still-unresolved unexecutable nodes
/// among those passing every correctness gate; ties break toward
/// the smaller region then the lower artifact id. Returns the
/// accepted substitutions, the covered node set, and whether any
/// accepted substitution is preview-class.
fn resolve_substitutions(
    snapshot: &ResolvedGraphSnapshot,
    intent: &ExecutionIntent,
    unexecutable: &BTreeSet<RcpNodeId>,
    renders: &RenderCandidateIndex,
    attempts: &mut Vec<CutsetAttempt>,
) -> (Vec<Substitution>, BTreeSet<RcpNodeId>, bool) {
    let mut substitutions: Vec<Substitution> = Vec::new();
    let mut covered: BTreeSet<RcpNodeId> = BTreeSet::new();
    let mut degraded_marks = false;
    if unexecutable.is_empty() {
        return (substitutions, covered, degraded_marks);
    }

    // Candidate pool: every artifact covering ≥1 unexecutable node.
    let mut pool: Vec<&RenderCandidate> = renders
        .candidates
        .iter()
        .filter(|c| c.node_set.iter().any(|n| unexecutable.contains(n)))
        .collect();
    pool.sort_by(|a, b| a.artifact_id.cmp(&b.artifact_id));

    // Evaluation is deterministic per candidate, but the greedy
    // loop reconsiders the pool each round — record each artifact's
    // first rejection once so the receipt lists every candidate
    // exactly once.
    let mut recorded: BTreeSet<String> = BTreeSet::new();

    loop {
        let remaining: BTreeSet<RcpNodeId> = unexecutable.difference(&covered).copied().collect();
        if remaining.is_empty() {
            break;
        }
        let mut best: Option<(&RenderCandidate, CutsetReport)> = None;
        for candidate in &pool {
            if !candidate.node_set.iter().any(|n| remaining.contains(n)) {
                continue;
            }
            match evaluate_candidate(
                snapshot,
                intent,
                candidate,
                &substitutions,
                attempts,
                &mut recorded,
            ) {
                CandidateEval::Accepted(report) => {
                    let covered_count = candidate
                        .node_set
                        .iter()
                        .filter(|n| remaining.contains(n))
                        .count();
                    let better = match &best {
                        None => true,
                        Some((inc, _)) => {
                            let inc_cov = inc
                                .node_set
                                .iter()
                                .filter(|n| remaining.contains(n))
                                .count();
                            (
                                covered_count,
                                std::cmp::Reverse(candidate.node_set.len()),
                                candidate.artifact_id.as_str(),
                            ) > (
                                inc_cov,
                                std::cmp::Reverse(inc.node_set.len()),
                                inc.artifact_id.as_str(),
                            )
                        }
                    };
                    if better {
                        best = Some((candidate, report));
                    }
                }
                CandidateEval::Rejected => {}
            }
        }
        let Some((candidate, report)) = best else {
            break;
        };
        let preview_class = is_preview_class(intent, candidate);
        substitutions.push(Substitution {
            nodes: candidate.node_set.clone(),
            artifact_id: candidate.artifact_id.clone(),
            content_hash: candidate.content_hash.clone(),
            preview_class,
        });
        covered.extend(candidate.node_set.iter().copied());
        attempts.push(CutsetAttempt {
            artifact_id: candidate.artifact_id.clone(),
            region: candidate.node_set.clone(),
            outcome: AttemptOutcome::Accepted,
            cutset: Some(report),
        });
        degraded_marks |= preview_class;
    }
    (substitutions, covered, degraded_marks)
}

/// Outcome of evaluating one candidate inside the greedy loop.
enum CandidateEval {
    /// Passed every gate — carries the accepted cutset report.
    Accepted(CutsetReport),
    /// Rejected; the reason was already recorded in `attempts`.
    Rejected,
}

/// Runs the correctness gates for one candidate and records the
/// rejection evidence. Live-region protection and overlap checks
/// are the caller's job.
fn evaluate_candidate(
    snapshot: &ResolvedGraphSnapshot,
    intent: &ExecutionIntent,
    candidate: &RenderCandidate,
    substitutions: &[Substitution],
    attempts: &mut Vec<CutsetAttempt>,
    recorded: &mut BTreeSet<String>,
) -> CandidateEval {
    let mut reject = |outcome: AttemptOutcome, cutset: Option<CutsetReport>| {
        if recorded.insert(candidate.artifact_id.clone()) {
            attempts.push(CutsetAttempt {
                artifact_id: candidate.artifact_id.clone(),
                region: candidate.node_set.clone(),
                outcome,
                cutset,
            });
        }
        CandidateEval::Rejected
    };
    if let Err(reason) = gate_candidate(snapshot, intent, candidate) {
        return reject(AttemptOutcome::Ineligible { reason }, None);
    }
    if intent
        .required_live_regions()
        .iter()
        .any(|r| r.iter().any(|n| candidate.node_set.contains(n)))
        || substitutions
            .iter()
            .any(|s| s.nodes.iter().any(|n| candidate.node_set.contains(n)))
    {
        return reject(
            AttemptOutcome::Ineligible {
                reason: "covers required live region or overlaps".to_string(),
            },
            None,
        );
    }
    let report = validate_cutset(snapshot, candidate);
    if report.accepted {
        CandidateEval::Accepted(report)
    } else {
        reject(AttemptOutcome::CutsetRejected, Some(report))
    }
}

/// Leftover phase: unexecutable nodes not covered by any accepted
/// substitution are skipped (preview opt-in only, outputs verified
/// fed) or recorded unresolved/degraded.
fn resolve_leftovers(
    snapshot: &ResolvedGraphSnapshot,
    intent: &ExecutionIntent,
    unexecutable: &BTreeSet<RcpNodeId>,
    covered: &BTreeSet<RcpNodeId>,
    substitutions: &[Substitution],
    verdicts: &BTreeMap<RcpNodeId, PluginVerdict>,
    degraded_marks: &mut bool,
) -> (Vec<SkippedNode>, Vec<UnresolvedNode>, Vec<RcpNodeId>) {
    let substituted_regions: Vec<BTreeSet<RcpNodeId>> =
        substitutions.iter().map(|s| s.nodes.clone()).collect();
    let mut skipped: Vec<SkippedNode> = Vec::new();
    let mut unresolved: Vec<UnresolvedNode> = Vec::new();
    let mut degraded: Vec<RcpNodeId> = Vec::new();

    for node_id in unexecutable {
        if covered.contains(node_id) {
            continue;
        }
        let verdict = verdicts
            .get(node_id)
            .cloned()
            .unwrap_or(PluginVerdict::Missing);
        let can_skip = intent.is_preview()
            && intent.non_equivalent_opted_in()
            && !intent
                .required_live_regions()
                .iter()
                .any(|r| r.contains(node_id))
            && skip_keeps_outputs_fed(snapshot, *node_id, &substituted_regions);
        if can_skip {
            skipped.push(SkippedNode {
                node: *node_id,
                verdict,
            });
            *degraded_marks = true;
        } else {
            unresolved.push(UnresolvedNode {
                node: *node_id,
                reason: UnresolvedReason::NoEligibleSubstitution,
            });
        }
    }

    // Degraded unresolved: preview opt-in tolerates unresolved
    // nodes outside required scope — marked, not blocking. Exact
    // intents and required-live-region members always block.
    let mut final_unresolved: Vec<UnresolvedNode> = Vec::new();
    for u in unresolved {
        let in_required_live = intent
            .required_live_regions()
            .iter()
            .any(|r| r.contains(&u.node));
        if intent.is_preview()
            && intent.non_equivalent_opted_in()
            && !in_required_live
            && !feeds_required_output(snapshot, u.node, &substituted_regions, &skipped)
        {
            degraded.push(u.node);
            *degraded_marks = true;
        } else {
            final_unresolved.push(u);
        }
    }
    (skipped, final_unresolved, degraded)
}

// ---------------------------------------------------------------------------
// Gates
// ---------------------------------------------------------------------------

fn reproducibility_rank(class: ReproducibilityClass) -> u8 {
    match class {
        ReproducibilityClass::BitExactCertified => 4,
        ReproducibilityClass::ReplayOfVerifiedArtifact => 3,
        ReproducibilityClass::VerifiedWithinTolerance => 2,
        ReproducibilityClass::PreviewOnly => 1,
        ReproducibilityClass::Unknown => 0,
    }
}

fn is_preview_class(intent: &ExecutionIntent, candidate: &RenderCandidate) -> bool {
    candidate.eligibility == ArtifactEligibility::EligiblePreview
        || candidate.reproducibility == ReproducibilityClass::PreviewOnly
        || (intent.is_preview()
            && candidate.reproducibility == ReproducibilityClass::ReplayOfVerifiedArtifact)
}

/// Intent/eligibility/access/pin gate — runs before cutset
/// validation so cheap rejections don't pay for boundary analysis.
/// Returns `Err(reason)` with evidence.
fn gate_candidate(
    snapshot: &ResolvedGraphSnapshot,
    intent: &ExecutionIntent,
    candidate: &RenderCandidate,
) -> Result<(), String> {
    if matches!(candidate.access, AccessState::Denied { .. }) {
        return Err("access denied".to_string());
    }
    // Snapshot pins: an artifact rendered against a different
    // revision or structure can never substitute — stale by
    // definition regardless of declared eligibility.
    if candidate.snapshot_revision != snapshot.project_revision
        || candidate.snapshot_digest != snapshot.structure_digest
    {
        return Err("stale snapshot pin".to_string());
    }
    match intent {
        ExecutionIntent::MainPlayback | ExecutionIntent::BranchAudition { .. } => {
            if candidate.eligibility != ArtifactEligibility::EligibleExact {
                return Err(format!("eligibility {:?}", candidate.eligibility));
            }
            if matches!(
                candidate.reproducibility,
                ReproducibilityClass::PreviewOnly | ReproducibilityClass::Unknown
            ) {
                return Err("preview-class reproducibility".to_string());
            }
        }
        ExecutionIntent::EditingPreview {
            allow_non_equivalent,
            ..
        } => {
            match candidate.eligibility {
                ArtifactEligibility::EligibleExact => {}
                ArtifactEligibility::EligiblePreview if *allow_non_equivalent => {}
                other => return Err(format!("eligibility {other:?}")),
            }
            if matches!(
                candidate.reproducibility,
                ReproducibilityClass::PreviewOnly | ReproducibilityClass::Unknown
            ) && !*allow_non_equivalent
            {
                return Err("preview-class reproducibility without opt-in".to_string());
            }
        }
        ExecutionIntent::OfflineRender {
            min_reproducibility,
        } => {
            if candidate.eligibility != ArtifactEligibility::EligibleExact {
                return Err(format!("eligibility {:?}", candidate.eligibility));
            }
            if reproducibility_rank(candidate.reproducibility)
                < reproducibility_rank(*min_reproducibility)
            {
                return Err("reproducibility below intent minimum".to_string());
            }
        }
        ExecutionIntent::Export => {
            if candidate.eligibility != ArtifactEligibility::EligibleExact {
                return Err(format!("eligibility {:?}", candidate.eligibility));
            }
            if candidate.reproducibility != ReproducibilityClass::BitExactCertified {
                return Err("export requires bit-exact certified".to_string());
            }
        }
    }
    Ok(())
}

/// `true` when removing `node` keeps every destination port it
/// feeds sufficiently supplied — live alternate feeds, enforced
/// multiplicity minimums, and required outputs all accounted for.
fn skip_keeps_outputs_fed(
    snapshot: &ResolvedGraphSnapshot,
    node: RcpNodeId,
    substituted_regions: &[BTreeSet<RcpNodeId>],
) -> bool {
    for edge in &snapshot.edges {
        if edge.from.node != node {
            continue;
        }
        let dest_node = snapshot.node(edge.to.node);
        let Some(dest_port) = dest_node.and_then(|n| n.port(edge.to.port)) else {
            continue;
        };
        let required = if snapshot.required_outputs.iter().any(|r| r.port == edge.to) {
            1
        } else if dest_port.requirement_enforced() {
            usize::from(dest_port.multiplicity.min)
        } else {
            0
        };
        if required == 0 {
            continue;
        }
        // Count feeds into edge.to excluding edges from `node`.
        let feeds = snapshot
            .edges
            .iter()
            .filter(|e| {
                e.to == edge.to
                    && e.from.node != node
                    && !substituted_regions
                        .iter()
                        .any(|r| r.contains(&e.from.node))
            })
            .count()
            // Substitution coverage: boundary edges from region
            // nodes count as feeds when their source port is
            // covered — approximated here by counting region edges
            // whose destination is this port (the cutset validator
            // already guaranteed coverage for accepted candidates).
            + snapshot
                .edges
                .iter()
                .filter(|e| {
                    e.to == edge.to
                        && substituted_regions
                            .iter()
                            .any(|r| r.contains(&e.from.node))
                })
                .count();
        if feeds < required {
            return false;
        }
    }
    true
}

/// `true` when `node` can reach a required output through edges —
/// unresolved nodes on such paths cannot be silently degraded.
fn feeds_required_output(
    snapshot: &ResolvedGraphSnapshot,
    node: RcpNodeId,
    substituted_regions: &[BTreeSet<RcpNodeId>],
    skipped: &[SkippedNode],
) -> bool {
    let skip_set: BTreeSet<RcpNodeId> = skipped.iter().map(|s| s.node).collect();
    let mut stack = vec![node];
    let mut seen = BTreeSet::new();
    while let Some(current) = stack.pop() {
        if !seen.insert(current) {
            continue;
        }
        for edge in &snapshot.edges {
            if edge.from.node != current {
                continue;
            }
            if substituted_regions
                .iter()
                .any(|r| r.contains(&edge.to.node))
            {
                // Path enters a substitution — the artifact's
                // boundary outputs were validated to cover it, so
                // the signal still reaches downstream live nodes.
                continue;
            }
            if skip_set.contains(&edge.to.node) {
                continue;
            }
            if snapshot
                .required_outputs
                .iter()
                .any(|r| r.port.node == edge.to.node)
            {
                return true;
            }
            stack.push(edge.to.node);
        }
    }
    false
}

/// Final required-output check: every declared boundary output
/// must retain ≥1 feed from live executable nodes or accepted
/// substitution coverage.
fn required_outputs_fed(
    snapshot: &ResolvedGraphSnapshot,
    covered: &BTreeSet<RcpNodeId>,
    substituted_regions: &[BTreeSet<RcpNodeId>],
    skipped: &[SkippedNode],
    unresolved: &[UnresolvedNode],
) -> bool {
    let removed: BTreeSet<RcpNodeId> = skipped
        .iter()
        .map(|s| s.node)
        .chain(unresolved.iter().map(|u| u.node))
        .collect();
    for req in &snapshot.required_outputs {
        let feeds = snapshot.edges.iter().filter(|e| {
            if e.to != req.port {
                return false;
            }
            if removed.contains(&e.from.node) {
                return false;
            }
            // Sources inside a substitution are fine — the cutset
            // validator guaranteed their outputs are covered.
            true
        });
        if feeds.count() == 0 {
            // Also accept coverage via substitution: the required
            // port's producer inside a region is covered iff the
            // region's cutset passed (already enforced).
            let producer_covered = covered.contains(&req.port.node)
                || substituted_regions
                    .iter()
                    .any(|r| r.contains(&req.port.node));
            if !producer_covered {
                return false;
            }
        }
    }
    true
}

/// Connected components of executable nodes → live regions.
fn compute_live_regions(
    snapshot: &ResolvedGraphSnapshot,
    covered: &BTreeSet<RcpNodeId>,
    skipped: &[SkippedNode],
    unresolved: &[UnresolvedNode],
    degraded: &[RcpNodeId],
) -> Vec<LiveRegion> {
    let removed: BTreeSet<RcpNodeId> = covered
        .iter()
        .copied()
        .chain(skipped.iter().map(|s| s.node))
        .chain(unresolved.iter().map(|u| u.node))
        .chain(degraded.iter().copied())
        .collect();
    let mut regions: Vec<BTreeSet<RcpNodeId>> = Vec::new();
    for node_id in snapshot.nodes.keys() {
        if removed.contains(node_id) {
            continue;
        }
        // BFS over edges whose both endpoints are live.
        let mut region = BTreeSet::new();
        let mut stack = vec![*node_id];
        while let Some(current) = stack.pop() {
            if removed.contains(&current) || !region.insert(current) {
                continue;
            }
            for edge in &snapshot.edges {
                if edge.from.node == current && !removed.contains(&edge.to.node) {
                    stack.push(edge.to.node);
                }
                if edge.to.node == current && !removed.contains(&edge.from.node) {
                    stack.push(edge.from.node);
                }
            }
        }
        if !region.is_empty() && !regions.iter().any(|r| r == &region) {
            regions.push(region);
        }
    }
    regions.sort_by_key(|r| r.iter().next().copied());
    regions
        .into_iter()
        .map(|nodes| LiveRegion { nodes })
        .collect()
}

fn compute_selection_digest(
    snapshot: &ResolvedGraphSnapshot,
    substitutions: &[Substitution],
    skipped: &[SkippedNode],
    unresolved: &[UnresolvedNode],
    degraded: &[RcpNodeId],
    verdict: &SelectionVerdict,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    mix(format!("{:?}", snapshot.snapshot_id).as_bytes());
    mix(format!("{verdict:?}").as_bytes());
    for s in substitutions {
        mix(s.artifact_id.as_bytes());
        for n in &s.nodes {
            mix(format!("{n:?}").as_bytes());
        }
    }
    for s in skipped {
        mix(format!("{:?}", s.node).as_bytes());
    }
    for u in unresolved {
        mix(format!("{:?}", u.node).as_bytes());
    }
    for d in degraded {
        mix(format!("{d:?}").as_bytes());
    }
    hash
}
