//! GC eligibility — mark/sweep over typed roots with grace period,
//! pending (unresolved remote) pins and tombstones
//! (synvelis#291, RCP-02; contract: `docs/specs/render-checkpoint.md`
//! §Retention/GC).
//!
//! Roots are the RCP-00 vocabulary materialized as
//! [`ArtifactReference`]s plus external render-manifest attachments
//! (commit-associated manifests). An artifact is collectable only when
//! *every* reference is released and the grace period has elapsed
//! since the last release (or since publish when never referenced).
//! `Pending` references — remote/offline pins whose reachability
//! cannot be confirmed — count as live: reachability unknown always
//! keeps, never deletes.
//!
//! Raw Source/Take/verified-recording bytes are structurally safe:
//! they live in a different store namespace and can never appear in
//! [`ArtifactStore::artifacts`], so this sweep cannot reach them.

use synvelis_project::snapshot::ContentDigest;

use crate::artifact::manifest::RenderArtifactId;
use crate::artifact::store::{ArtifactStore, RefState, StoreError};

/// GC policy — the grace window in store logical ticks.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GcPolicy {
    /// Ticks an unreferenced artifact is retained after its last
    /// reference release (or after publish when never referenced).
    /// `0` collects on the next sweep.
    pub grace_ticks: u64,
}

/// Sweep outcome — full evidence of what was kept and why.
#[derive(Clone, Debug, Default)]
pub struct GcReport {
    /// Artifacts scanned.
    pub scanned: usize,
    /// Kept — at least one live (`Active`/`Pending`) reference or an
    /// external manifest attachment exists.
    pub kept_reachable: Vec<RenderArtifactId>,
    /// Kept — unreferenced but still inside the grace window.
    pub kept_grace: Vec<RenderArtifactId>,
    /// Deleted — final reference lost and grace expired; tombstoned.
    pub collected: Vec<RenderArtifactId>,
    /// Quarantined artifacts deleted after their own grace elapsed.
    pub quarantined_collected: Vec<RenderArtifactId>,
    /// Blob digests freed because no remaining manifest referenced
    /// them.
    pub blobs_freed: Vec<ContentDigest>,
}

/// One sweep pass — deterministic given store state and clock.
///
/// # Errors
/// [`StoreError`] — enumeration/deletion failure; partial progress is
/// reported through the error, never silently skipped.
pub fn sweep(store: &mut dyn ArtifactStore, policy: &GcPolicy) -> Result<GcReport, StoreError> {
    let now = store.now();
    let records = store.artifacts()?;
    let mut report = GcReport {
        scanned: records.len(),
        ..GcReport::default()
    };
    for record in records {
        let id = record.id;
        // External manifest attachments are commit-associated roots —
        // an attached artifact is reachable regardless of references.
        if store.has_external_manifest(&id)? {
            report.kept_reachable.push(id);
            continue;
        }
        let refs = store.references(&id)?;
        let live = refs
            .iter()
            .any(|r| matches!(r.state, RefState::Active | RefState::Pending));
        if live {
            report.kept_reachable.push(id);
            continue;
        }
        // Grace base: latest release tick, or publish tick when the
        // artifact was never referenced.
        let base = refs
            .iter()
            .filter_map(|r| match r.state {
                RefState::Released { at } => Some(at),
                _ => None,
            })
            .max()
            .unwrap_or(record.published_tick);
        let eligible_at = base.saturating_add(policy.grace_ticks);
        if record.quarantined {
            // Quarantined records sweep on the same grace rule from
            // their quarantine/publish tick.
            if now >= eligible_at {
                let outcome = store.delete_artifact(&id)?;
                report.quarantined_collected.push(id);
                if let Some(digest) = outcome.blob_freed {
                    report.blobs_freed.push(digest);
                }
            } else {
                report.kept_grace.push(id);
            }
            continue;
        }
        if now >= eligible_at {
            let outcome = store.delete_artifact(&id)?;
            report.collected.push(id);
            if let Some(digest) = outcome.blob_freed {
                report.blobs_freed.push(digest);
            }
        } else {
            report.kept_grace.push(id);
        }
    }
    Ok(report)
}
