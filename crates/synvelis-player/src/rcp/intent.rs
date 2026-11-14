//! `ExecutionIntent` — the typed evaluation/execution intent that
//! selects resolver behavior (Issue #293 contract).
//!
//! Each intent carries exactly the policy inputs the spec assigns
//! to it; the selector never infers intent from graph shape. The
//! non-equivalence contract is structural: preview intents must
//! explicitly opt in *and* the resulting [`super::select::ExecutionSelection`]
//! carries an explicit non-equivalence mark — there is no path that
//! silently returns preview audio under an Exact claim.

use std::collections::BTreeSet;

use super::model::{PortEndpoint, RcpNodeId, ReproducibilityClass};

/// What the session intends to do with the resolved plan.
#[derive(Clone, Debug, PartialEq)]
pub enum ExecutionIntent {
    /// Normal project playback — project-authoritative exactness.
    /// Every required output must be reproduced exactly; any
    /// unresolved plugin fails closed to `Blocked`.
    MainPlayback,
    /// Editing-oriented preview of a selected live region.
    ///
    /// `live_regions` are the node sets the user explicitly wants
    /// running live (editable); everything else may degrade to
    /// verified render substitution or skip. `allow_non_equivalent`
    /// is the explicit opt-in: without it, preview-class artifacts
    /// and degraded nodes still fail closed.
    EditingPreview {
        /// Node sets that must execute live for the preview to be
        /// meaningful — unresolved nodes inside these block the
        /// selection.
        live_regions: Vec<BTreeSet<RcpNodeId>>,
        /// Explicit opt-in to non-equivalent preview output.
        allow_non_equivalent: bool,
    },
    /// Branch audition — compare a candidate branch against the
    /// main line.
    ///
    /// `fixed_inputs` are the boundary inputs pinned to the *main*
    /// revision's signals so the audition cannot leak branch state
    /// into the comparison; `target_revision` is the branch
    /// revision being auditioned (evidence for receipts — the
    /// snapshot itself is already resolved at that revision).
    /// Writer isolation (main vs. recording) is a runtime concern
    /// outside this headless slice.
    BranchAudition {
        /// Branch revision under audition.
        target_revision: synvelis_project::ProjectRevision,
        /// Inputs held fixed to main-line signals.
        fixed_inputs: BTreeSet<PortEndpoint>,
        /// Explicit opt-in to non-equivalent audition output.
        allow_non_equivalent: bool,
    },
    /// Recipe-driven offline render — completeness and
    /// reproducibility fail closed; no preview relaxation.
    OfflineRender {
        /// Minimum reproducibility class the render job requires.
        min_reproducibility: ReproducibilityClass,
    },
    /// Recipe-driven export — strictest intent: every substitution
    /// must be `EligibleExact` with certified reproducibility.
    Export,
}

impl ExecutionIntent {
    /// `true` for intents that demand project-authoritative
    /// exactness (no preview relaxation, no skips of required
    /// outputs).
    #[must_use]
    pub fn requires_exact(&self) -> bool {
        match self {
            Self::MainPlayback | Self::OfflineRender { .. } | Self::Export => true,
            Self::EditingPreview { .. } | Self::BranchAudition { .. } => false,
        }
    }

    /// `true` when the intent permits non-equivalent output at all
    /// — preview intents still need their explicit opt-in checked
    /// separately via [`Self::non_equivalent_opted_in`].
    #[must_use]
    pub fn is_preview(&self) -> bool {
        !self.requires_exact()
    }

    /// `true` when the intent's explicit non-equivalence opt-in is
    /// present. Exact intents return `false` — they can never opt
    /// in.
    #[must_use]
    pub fn non_equivalent_opted_in(&self) -> bool {
        match self {
            Self::EditingPreview {
                allow_non_equivalent,
                ..
            }
            | Self::BranchAudition {
                allow_non_equivalent,
                ..
            } => *allow_non_equivalent,
            _ => false,
        }
    }

    /// Nodes that must execute live for this intent — preview
    /// intents name their editable regions; exact intents require
    /// every non-substituted node live, so this returns an empty
    /// set (the "must be live" constraint is expressed through
    /// `requires_exact` instead).
    #[must_use]
    pub fn required_live_regions(&self) -> &[BTreeSet<RcpNodeId>] {
        match self {
            Self::EditingPreview { live_regions, .. } => live_regions,
            _ => &[],
        }
    }
}
