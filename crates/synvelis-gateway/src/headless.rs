//! AUTX-02 headless session host — GUI-free gateway composition.
//!
//! [`HeadlessSessionHost`] assembles the pieces a headless local
//! session needs — the typed [`Gateway`] boundary, the trusted local
//! CLI adapter ([`crate::cli::LocalCliAdapter`]), and the commit /
//! administration ports — without any GUI, windowing, or transport
//! dependency. It is the in-crate contract a future `synvelis-runtime`
//! CLI binary or session process embeds: project open/query/prepare/
//! commit/reopen semantics stay exactly the existing ones because the
//! host adds *no* second source of truth — every operation still flows
//! through `Gateway::handle` under a seal-minted [`CallerIdentity`],
//! and commits still flow through the same [`CommitPort`] /
//! [`crate::CommitGate`] fence, journal, and recovery path.
//!
//! Trust boundary (unchanged from GW-01/GW-02/GW-08):
//!
//! - A request enters only as wire JSON plus a [`CliSessionClaim`].
//!   `handle_claim` resolves the claim through
//!   [`LocalCliAdapter::attest`] — unattended claims always become
//!   agent-equivalent delegated callers, interactive claims become
//!   `Human` only with a store-verified session credential. TTY state,
//!   `--yes`, environment variables, script source, and JSON fields
//!   are never consulted.
//! - `handle` accepts an already-minted [`CallerIdentity`] for trusted
//!   in-process callers (e.g. the same session driving a prepared
//!   apply); it cannot mint one — only the crate's seal issuers can.
//! - The commit/admin ports are supplied by the host wiring. Wiring a
//!   [`crate::CommitGate`] (which implements both ports) keeps the
//!   single-writer, revision, journal, and `Indeterminate` contracts
//!   intact; [`NoCommitPort`]/[`NoAdminPort`] leave the boundary
//!   fail-closed.
//!
//! Deferred to later slices (recorded in `evidence/sv261/`): the
//! `synvelis-project` port adapter for real project open/lock/reopen,
//! a durable `CliSessionStore` with OS credential verification, the
//! `synvelis-runtime` CLI binary, and real multi-process commit
//! verification.

use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::Gateway;
use crate::cli::{CliSessionClaim, CliSessionStore, LocalCliAdapter};
use crate::clock::PolicyClock;
use crate::id::AdapterId;
use crate::mcp_server::{AdminPort, CommitPort, NoAdminPort, NoCommitPort};
use crate::outcome::{ErrorEnvelope, GatewayOutcome};
use crate::request::CallerIdentity;

/// The GUI-free session host — one gateway, one local CLI adapter,
/// one commit/admin seam set.
///
/// `Gateway` is behind a mutex because `bind`/`invoke` mutate the id
/// allocator and idempotency ledger; the adapter serializes its own
/// session store. Both locks are leaf-level — no lock ordering
/// concerns beyond "never hold one while calling into a reentrant
/// path", which this type does not create.
pub struct HeadlessSessionHost {
    gateway: Mutex<Gateway>,
    cli: LocalCliAdapter,
    committer: Arc<dyn CommitPort>,
    admin: Arc<dyn AdminPort>,
}

#[allow(clippy::result_large_err)] // typed envelopes are the contract
impl HeadlessSessionHost {
    /// Composes a headless session over an already-configured
    /// [`Gateway`] (capabilities, schemas, and policy evaluator are the
    /// caller's wiring — the host does not weaken them).
    ///
    /// `sessions` is the CLI session store (e.g.
    /// [`crate::cli::MemoryCliSessionStore`]); `committer`/`admin` are
    /// the commit and administration seams — a [`crate::CommitGate`]
    /// satisfies both, [`NoCommitPort`]/[`NoAdminPort`] leave them
    /// fail-closed. `clock` drives session expiry; `adapter` is the
    /// endpoint identity local-CLI callers attest through
    /// (conventionally [`crate::cli::CLI_ADAPTER_RAW`]).
    #[must_use]
    pub fn new(
        gateway: Gateway,
        sessions: Box<dyn CliSessionStore>,
        committer: Arc<dyn CommitPort>,
        admin: Arc<dyn AdminPort>,
        clock: Arc<dyn PolicyClock>,
        adapter: AdapterId,
    ) -> Self {
        Self {
            gateway: Mutex::new(gateway),
            cli: LocalCliAdapter::new(adapter, sessions, clock),
            committer,
            admin,
        }
    }

    /// A host with no commit boundary and no admin surface — the
    /// fail-closed default for wiring that has not (yet) attached a
    /// [`crate::CommitGate`].
    #[must_use]
    pub fn without_commit_boundary(
        gateway: Gateway,
        sessions: Box<dyn CliSessionStore>,
        clock: Arc<dyn PolicyClock>,
    ) -> Self {
        Self::new(
            gateway,
            sessions,
            Arc::new(NoCommitPort),
            Arc::new(NoAdminPort),
            clock,
            AdapterId::new(crate::cli::CLI_ADAPTER_RAW),
        )
    }

    /// The local CLI adapter — session issue/revoke and caller
    /// attestation live here.
    #[must_use]
    pub const fn cli(&self) -> &LocalCliAdapter {
        &self.cli
    }

    /// The commit boundary the host drives applies through.
    #[must_use]
    pub fn committer(&self) -> Arc<dyn CommitPort> {
        Arc::clone(&self.committer)
    }

    /// The administration surface (task/grant issuance and revocation,
    /// live revision probe).
    #[must_use]
    pub fn admin(&self) -> Arc<dyn AdminPort> {
        Arc::clone(&self.admin)
    }

    /// Runs one wire request under an already-minted trusted caller —
    /// the full parse → bind → invoke path. The caller must come from
    /// a crate seal issuer; this method cannot create one.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — any pipeline stage's typed failure.
    ///
    /// # Panics
    /// Only if the gateway lock is poisoned by a panic elsewhere.
    pub fn handle(
        &self,
        value: &Value,
        caller: CallerIdentity,
    ) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        self.gateway
            .lock()
            .expect("gateway poisoned")
            .handle(value, caller)
    }

    /// Resolves `claim` to a trusted caller, then runs the wire
    /// request — the single entry point an unattended or interactive
    /// local invocation takes.
    ///
    /// An unverifiable claim fails before the gateway is touched:
    /// `Unattended` always yields the delegated agent-equivalent
    /// caller, `Interactive` yields `Human` only for a live
    /// store-verified session credential. Once bound, the request
    /// faces the same capability/schema/consistency validation and
    /// per-call policy evaluation as every other adapter path.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — claim attestation failure
    /// ([`crate::outcome::GatewayError::Authentication`] for a bad
    /// credential, `MalformedRequest` for a bad principal id) or any
    /// gateway pipeline failure.
    pub fn handle_claim(
        &self,
        value: &Value,
        claim: &CliSessionClaim,
    ) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let caller = self.cli.attest(claim)?;
        self.handle(value, caller)
    }
}
