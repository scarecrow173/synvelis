//! Temporal invalidation + partial re-render classification
//! (synvelis#290, RCP-01; contract: `docs/specs/render-checkpoint.md`).
//!
//! [`classify_invalidation`] answers "which output frames must
//! re-render when these semantic objects changed?" for a resolved
//! [`RenderRecipeV1`]. The rules are deliberately conservative —
//! RCP-REC-04 requires dirty intervals to expand *without
//! under-approximation* (`過少区間再利用0`):
//!
//! - A changed object **outside the typed closure** affects nothing
//!   (unrelated subgraphs keep their keys — RCP-REC-03).
//! - A changed object **inside the closure** with a known dirty frame
//!   interval expands by the recipe's temporal margins:
//!   - DSP lookahead + reported PDC backwards,
//!   - declared tail forwards (reverb tail, plugin release,
//!     note-off/held-CC tails),
//!   - the loop region when the dirty signal overlaps it (every loop
//!     iteration re-renders),
//!   - forward-to-end when the dirty signal touches a seek boundary
//!     (post-seek state is not reusable).
//! - A change with **unknown temporal extent**, or a temporal-domain
//!   change (tempo map / PPQN / musical placement), invalidates the
//!   whole render interval — never a guessed sub-interval.
//! - Clip crossfade materializes as comp-transition/comp-region
//!   entities inside the closure; their dirty intervals carry the
//!   crossfade span, and the standard tail/lookahead margins apply.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use synvelis_project::diff::{ObjectKind, SemanticObjectRef};

use crate::recipe::{FrameInterval, RenderRecipeV1};

/// One semantic change to classify against a recipe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangedObject {
    /// The changed semantic object.
    pub object: SemanticObjectRef,
    /// Dirty interval in **render-output frames** when known. `None`
    /// means the change's temporal extent is unknown — full
    /// invalidation, never a guessed sub-interval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<FrameInterval>,
    /// `true` when the change is temporal-domain (tempo map, PPQN,
    /// musical anchor/extent) — musical-time consumers cannot be
    /// partially reused, so the whole interval invalidates.
    #[serde(default)]
    pub temporal: bool,
}

/// Why an interval grew — recorded for evidence/audit.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InvalidationReason {
    /// Direct dirty interval of a closure member.
    Direct,
    /// Expanded backwards by lookahead + PDC.
    LookaheadPdc,
    /// Expanded forwards by the declared tail.
    Tail,
    /// Expanded to the whole loop region (loop re-iteration).
    LoopRegion,
    /// Extended to the interval end across a seek boundary.
    SeekBoundary,
    /// Temporal-domain change — whole interval invalidated.
    TemporalDomain,
    /// Unknown temporal extent — whole interval invalidated
    /// (fail closed; partial reuse would under-approximate).
    UnknownExtent,
}

/// The partial re-render decision for one recipe.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvalidationPlan {
    /// Merged, sorted dirty output-frame intervals inside the render
    /// interval. When [`Self::full`] is `true` this is exactly the
    /// whole `temporal.interval`.
    pub dirty_intervals: Vec<FrameInterval>,
    /// Closure objects the changes touched (evidence).
    pub affected_objects: BTreeSet<SemanticObjectRef>,
    /// Per-interval expansion reasons (union of all applied rules).
    pub reasons: BTreeSet<InvalidationReason>,
    /// `true` = the whole render interval must re-render.
    pub full: bool,
}

/// Kinds whose changes are inherently temporal-domain even when the
/// caller did not flag them: the project singleton carries the tempo
/// map/PPQN; musical-time carriers are flagged `temporal` by callers
/// that know the change kind — the kind check here is the fail-closed
/// backstop for the tempo-map carrier itself.
fn is_temporal_carrier(kind: ObjectKind) -> bool {
    matches!(kind, ObjectKind::Project)
}

/// Classifies `changes` against `recipe`'s closure and temporal
/// domain.
///
/// Infallible by construction: every ambiguous input resolves to the
/// *larger* dirty region, never the smaller.
#[must_use]
pub fn classify_invalidation(
    recipe: &RenderRecipeV1,
    changes: &[ChangedObject],
) -> InvalidationPlan {
    let temporal = &recipe.temporal;
    let closure: BTreeSet<&SemanticObjectRef> =
        recipe.closure.nodes.iter().map(|n| &n.object).collect();

    let mut plan = InvalidationPlan::default();
    let mut dirty: Vec<FrameInterval> = Vec::new();
    let mut full = false;

    for change in changes {
        // Unrelated subgraph — cannot affect this boundary (RCP-REC-03).
        if !closure.contains(&change.object) {
            continue;
        }
        plan.affected_objects.insert(change.object.clone());

        if change.temporal || is_temporal_carrier(change.object.kind) {
            full = true;
            plan.reasons.insert(InvalidationReason::TemporalDomain);
            continue;
        }
        let Some(base) = change.interval else {
            // Unknown extent — fail closed to the whole interval.
            full = true;
            plan.reasons.insert(InvalidationReason::UnknownExtent);
            continue;
        };

        // Expand: lookahead + PDC backwards, tail forwards.
        let back = temporal
            .lookahead_frames
            .saturating_add(temporal.pdc_frames);
        let fwd = temporal.tail_frames;
        let mut start = base.start.saturating_sub(back);
        let mut end = base.end.saturating_add(fwd);
        plan.reasons.insert(InvalidationReason::Direct);
        if back > 0 {
            plan.reasons.insert(InvalidationReason::LookaheadPdc);
        }
        if fwd > 0 {
            plan.reasons.insert(InvalidationReason::Tail);
        }

        // Loop: a dirty signal inside the loop region re-renders every
        // iteration — expand to the whole loop coverage.
        if let Some(loop_region) = temporal.loop_region
            && loop_region.overlaps_or_touches(FrameInterval { start, end })
        {
            start = start.min(loop_region.start);
            end = end.max(loop_region.end);
            plan.reasons.insert(InvalidationReason::LoopRegion);
        }

        // Seek boundaries: state does not carry across a seek, but a
        // dirty signal touching a boundary can affect the post-seek
        // approach — extend conservatively to the interval end.
        if temporal
            .seek_boundaries
            .iter()
            .any(|b| start <= *b && *b < end)
        {
            end = temporal.interval.end;
            plan.reasons.insert(InvalidationReason::SeekBoundary);
        }

        // Clip to the render interval; empty after clipping = the
        // change lands outside what this render covers.
        start = start.max(temporal.interval.start);
        end = end.min(temporal.interval.end);
        if end > start
            && let Ok(iv) = FrameInterval::new(start, end)
        {
            dirty.push(iv);
        }
    }

    if full {
        plan.full = true;
        plan.dirty_intervals = vec![temporal.interval];
        return plan;
    }

    // Merge overlapping/touching intervals into a canonical sorted set.
    dirty.sort();
    let mut merged: Vec<FrameInterval> = Vec::with_capacity(dirty.len());
    for iv in dirty {
        match merged.last_mut() {
            Some(last) if last.overlaps_or_touches(iv) => *last = last.merge(iv),
            _ => merged.push(iv),
        }
    }
    plan.dirty_intervals = merged;
    plan
}
