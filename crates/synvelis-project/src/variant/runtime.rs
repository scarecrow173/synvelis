//! Production [`RuntimePublish`] port (synvelis#517).
//!
//! The adopt contract keeps project commit and runtime activation as
//! separate evidence: [`RuntimePublish`] runs *after* the project CAS
//! and only reports what the runtime did. Within `synvelis-project`
//! there is no live engine to activate — engine-side rebuild on the
//! committed revision is the session layer's job (RCP-BR-04 last-good
//! semantics live there). What this crate *can* honestly do in
//! production is:
//!
//! - durably journal every publish request so a pending activation is
//!   visible across process restarts instead of being lost, and
//! - report typed [`RuntimePublishState::Pending`] — never `Published`
//!   — because no runtime confirmed activation, and typed `Failed`
//!   when even the journal write fails.
//!
//! `Pending` is the truthful state for a headless host: the project is
//! committed, activation is deferred to the runtime owner, and the
//! last-good plan is *not* reported as current. A future runtime
//! backend plugs in beside the journal without changing receipts.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::snapshot::ContentDigest;
use crate::variant::types::{
    RuntimePublish, RuntimePublishEvidence, RuntimePublishRequest, RuntimePublishState,
};

/// Journaled runtime-publish port — see the module docs.
///
/// Each request is written as one immutable JSON record under
/// `<journal_dir>/<request-digest>.json` via staged publish, so the
/// pending set survives restarts and duplicate requests are
/// idempotent by construction (same request → same digest → same
/// record).
#[derive(Clone, Debug)]
pub struct JournaledRuntimePublish {
    journal_dir: PathBuf,
}

/// One durable journal record — the publish request plus the evidence
/// state it was accepted under. `pending` records are the restart-
/// visible last-good ledger: nothing here claims runtime activation.
#[derive(Serialize)]
struct PublishRecord<'a> {
    project_id: &'a crate::id::ProjectId,
    revision: crate::controller::ProjectRevision,
    variant_set: &'a crate::id::VariantSetId,
    branch: &'a crate::id::BranchId,
    scope_digest: &'a ContentDigest,
    state: &'static str,
    detail: &'static str,
}

impl JournaledRuntimePublish {
    /// Port journaling into `journal_dir` (created on first publish).
    #[must_use]
    pub fn new(journal_dir: impl Into<PathBuf>) -> Self {
        Self {
            journal_dir: journal_dir.into(),
        }
    }

    /// The directory publish records live under.
    #[must_use]
    pub fn journal_dir(&self) -> &Path {
        &self.journal_dir
    }

    fn record_path(&self, request: &RuntimePublishRequest) -> Result<PathBuf, String> {
        let body = serde_json::to_vec(&serde_json::json!({
            "project_id": request.project_id,
            "revision": request.revision.0,
            "variant_set": request.variant_set,
            "branch": request.branch,
            "scope_digest": request.scope_digest,
        }))
        .map_err(|e| format!("journal key encode: {e}"))?;
        let digest = ContentDigest::sha256(&body);
        Ok(self
            .journal_dir
            .join(crate::plugin_state::digest_hex(&digest))
            .with_extension("json"))
    }
}

impl RuntimePublish for JournaledRuntimePublish {
    fn publish(&self, request: &RuntimePublishRequest) -> RuntimePublishEvidence {
        const PENDING_REASON: &str =
            "runtime activation is engine-owned; publish request journaled, activation pending";
        const RECORD_DETAIL: &str = "project committed; runtime activation deferred to the session engine (last-good retained)";
        let fail = |reason: String| RuntimePublishEvidence {
            state: RuntimePublishState::Failed { reason },
            detail: None,
        };
        let path = match self.record_path(request) {
            Ok(p) => p,
            Err(e) => return fail(e),
        };
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return fail(format!("create journal dir {}: {e}", parent.display()));
        }
        let record = PublishRecord {
            project_id: &request.project_id,
            revision: request.revision,
            variant_set: &request.variant_set,
            branch: &request.branch,
            scope_digest: &request.scope_digest,
            state: "pending",
            detail: RECORD_DETAIL,
        };
        let bytes = match serde_json::to_vec(&record) {
            Ok(b) => b,
            Err(e) => return fail(format!("journal encode: {e}")),
        };
        let mut staged = match ruviform_io_file::StagedFile::create(&path) {
            Ok(s) => s,
            Err(e) => return fail(format!("stage {}: {e}", path.display())),
        };
        if let Err(e) = staged.write_all(&bytes) {
            return fail(format!("write {}: {e}", path.display()));
        }
        let mut receipt = match staged.seal().publish(ruviform_io_file::PublishOptions::new(
            ruviform_io_file::PublishMode::Replace,
            ruviform_io_file::Durability::FileAndDirectorySync,
        )) {
            Ok(r) => r,
            Err(e) => return fail(format!("publish {}: {e}", path.display())),
        };
        if let Some(handle) = receipt.take_recovery_handle() {
            let _ = handle.cleanup();
        }
        RuntimePublishEvidence {
            state: RuntimePublishState::Pending {
                reason: PENDING_REASON.to_owned(),
            },
            detail: Some(path.display().to_string()),
        }
    }
}
