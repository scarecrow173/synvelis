//! Library job ledger persistence/recovery (ALIB-05 residual,
//! synvelis#643).
//!
//! The app-owned [`synvelis_workspace::LibraryJobLedger`] is in-memory —
//! this module persists its wire projection (`jobs_snapshot`) as the
//! `library-jobs.json` bundle sidecar at `syn.project.save`/`new`, and
//! restores it at `syn.project.open` through the ledger's public API.
//! Workers cannot survive a process restart, so entries that were
//! in-flight land as *typed terminals* rather than fake in-flight
//! states: a parked commit → `recoverable` at `commit-pending` (the
//! durable manifest — not the ledger — carries the intent a resubmit
//! converges through), an import job whose durable manifest exists →
//! `recoverable` at the manifest's stage, a recorded user cancel →
//! `cancelled`, everything else → `failed` with reason `interrupted`.
//! `done` entries keep their recorded receipt verbatim.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use synvelis_library::ImportStage;
use synvelis_project::{ImportId, ProjectStore};
use synvelis_workspace::{
    LibraryJobBegin, LibraryJobLedger, LibraryJobState, LibraryJobTerminal, LibraryJobWire,
};

use crate::app::SynvelisApp;
use crate::error::AppError;
use crate::host::ShellWindowHost;

/// The bundle sidecar file — flat under the bundle root, beside `HEAD`.
const LIBRARY_JOBS_SIDECAR: &str = "library-jobs.json";

/// Versioned format tag — a file written by a newer writer fails
/// loudly instead of silently degrading (the snapshot envelope's
/// `deny_unknown_fields` honesty, applied to the sidecar).
const LIBRARY_JOBS_FORMAT: &str = "synvelis-library-jobs/1";

/// The persisted payload — a `jobs_snapshot()` verbatim plus the
/// format tag and the owning project's tagged id (a foreign project's
/// sidecar is never restored).
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryJobsSidecar {
    format: String,
    project_id: String,
    jobs: Vec<LibraryJobWire>,
}

impl SynvelisApp {
    /// Writes the ledger snapshot of `workspace` into `store` — called
    /// at every successful `syn.project.new`/`save` so the sidecar and
    /// HEAD describe the same point in time (project authority stays
    /// single-writer on the service thread).
    ///
    /// # Errors
    /// [`AppError::Store`] on serialization or publish failures.
    pub(crate) fn persist_library_jobs(
        workspace: &synvelis_workspace::WorkspaceManager<ShellWindowHost>,
        store: &mut dyn ProjectStore,
    ) -> Result<(), AppError> {
        let payload = LibraryJobsSidecar {
            format: LIBRARY_JOBS_FORMAT.to_string(),
            project_id: workspace.session().project().project_id.to_tagged(),
            jobs: workspace.library().jobs_snapshot(),
        };
        let bytes = serde_json::to_vec(&payload)
            .map_err(|e| AppError::store(format!("{LIBRARY_JOBS_SIDECAR}: {e}")))?;
        store
            .save_sidecar(LIBRARY_JOBS_SIDECAR, &bytes)
            .map_err(|e| AppError::store(e.to_string()))
    }

    /// Restores the persisted ledger onto the freshly-built workspace —
    /// `done` entries keep their receipts; every other state lands the
    /// typed restart terminal (see module docs). Reads the durable
    /// import manifests through the bound [`crate::app::LibraryImportServices`]
    /// so `recoverable`/`cancelled.resumable` answer the truth the
    /// store can prove, never an optimistic claim.
    ///
    /// # Errors
    /// [`AppError::Store`] on a corrupt or unknown-format sidecar — a
    /// foreign `project_id` degrades to "not restored" (never an
    /// error: a bundle dir may be reused).
    pub(crate) fn restore_library_jobs(&self) -> Result<(), AppError> {
        let Some(store) = self.store.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = store
            .read_sidecar(LIBRARY_JOBS_SIDECAR)
            .map_err(|e| AppError::store(e.to_string()))?
        else {
            return Ok(());
        };
        let sidecar: LibraryJobsSidecar = serde_json::from_slice(&bytes)
            .map_err(|e| AppError::store(format!("{LIBRARY_JOBS_SIDECAR}: {e}")))?;
        if sidecar.format != LIBRARY_JOBS_FORMAT {
            return Err(AppError::store(format!(
                "{LIBRARY_JOBS_SIDECAR}: unsupported format {}",
                sidecar.format
            )));
        }
        let project = self.workspace.session().project();
        if sidecar.project_id != project.project_id.to_tagged() {
            tracing::warn!("library jobs sidecar names a different project — not restored");
            return Ok(());
        }
        // Resumability evidence: which imports left a durable manifest.
        // `inspect_imports` failing degrades to "no manifest provable" —
        // entries then read `failed` rather than claiming a resume the
        // store cannot show.
        let manifests: BTreeMap<u128, ImportStage> = self
            .library_import
            .as_ref()
            .and_then(|services| {
                synvelis_library::inspect_imports(&services.store, project)
                    .map_err(
                        |e| tracing::warn!(error = %e, "inspect_imports failed during job restore"),
                    )
                    .ok()
            })
            .unwrap_or_default()
            .into_iter()
            .map(|j| (j.import_id.raw(), j.stage))
            .collect();
        let ledger = self.workspace.library().jobs();
        let Ok(mut ledger) = ledger.lock() else {
            return Ok(());
        };
        for wire in &sidecar.jobs {
            restore_job_entry(&mut ledger, wire, &manifests);
        }
        Ok(())
    }
}

/// Rebuilds one wire entry through the public ledger API — `begin`
/// mints a fresh token/epoch (both are process-local), `report` and
/// `request_cancel` replay the recorded flags, `finish` lands the
/// receipt. `runtime_job`, `pending_commit`, `intent`, and the cancel
/// token itself cannot be re-injected — the terminal records that
/// honestly.
fn restore_job_entry(
    ledger: &mut LibraryJobLedger,
    wire: &LibraryJobWire,
    manifests: &BTreeMap<u128, ImportStage>,
) {
    let Some(view) = wire
        .view
        .strip_prefix('v')
        .and_then(|n| n.parse::<u64>().ok())
        .and_then(ruviform_ui::dock::ViewInstanceId::new)
    else {
        tracing::warn!(job_key = %wire.job_key, "skipping job with unrestorable view id");
        return;
    };
    if let LibraryJobBegin::InFlight = ledger.begin(
        wire.job_key.clone(),
        wire.kind,
        view,
        wire.import_id.clone(),
        None,
    ) {
        return;
    }
    if let Some(progress) = &wire.progress {
        ledger.report(
            &wire.job_key,
            progress.done,
            progress.total,
            &progress.detail,
        );
    }
    if wire.cancel_requested {
        let _ = ledger.request_cancel(&wire.job_key);
    }
    let terminal = if wire.state == LibraryJobState::Done {
        wire.terminal
            .clone()
            .unwrap_or_else(|| LibraryJobTerminal::Failed {
                reason: "interrupted".to_string(),
                detail: format!(
                    "job {} recorded done without a terminal before restart",
                    wire.job_key
                ),
            })
    } else {
        recovery_terminal(wire, manifests)
    };
    ledger.finish(&wire.job_key, terminal);
}

/// The terminal an in-flight entry lands as — the worker did not
/// survive the restart, so the honest receipt is `cancelled` when a
/// user cancel was already recorded, `recoverable` while a durable
/// manifest exists for the import to converge through, `failed`
/// otherwise.
fn recovery_terminal(
    wire: &LibraryJobWire,
    manifests: &BTreeMap<u128, ImportStage>,
) -> LibraryJobTerminal {
    let detail = format!(
        "job {} was interrupted by a process restart before reaching a terminal",
        wire.job_key
    );
    // A parked commit proves the durable manifest reached `Published`
    // — the owed commit survives for a resubmit to land.
    let manifest_stage = if wire.pending_commit {
        Some(ImportStage::Published)
    } else {
        wire.import_id
            .as_deref()
            .and_then(|tagged| ImportId::from_tagged(tagged).ok())
            .and_then(|id| manifests.get(&id.raw()).copied())
    };
    if wire.cancel_requested {
        return LibraryJobTerminal::Cancelled {
            resumable: manifest_stage.is_some(),
            stage: manifest_stage.map(recovery_stage_name).map(str::to_string),
        };
    }
    match manifest_stage {
        Some(stage) => LibraryJobTerminal::Recoverable {
            reason: "interrupted".to_string(),
            stage: recovery_stage_name(stage).to_string(),
            detail,
        },
        None => LibraryJobTerminal::Failed {
            reason: "interrupted".to_string(),
            detail,
        },
    }
}

/// The `Recoverable.stage`/`Cancelled.stage` vocabulary — mirrors the
/// workspace's private `ux_stage` mapping verbatim (synvelis-workspace
/// is read-only for this slice; the strings are the durable-stage
/// wording `import.retry` receipts already report).
fn recovery_stage_name(stage: ImportStage) -> &'static str {
    match stage {
        ImportStage::Started => "fetching",
        ImportStage::Staged => "verifying",
        ImportStage::Verified => "materializing",
        ImportStage::Published | ImportStage::Committed => "commit-pending",
    }
}
