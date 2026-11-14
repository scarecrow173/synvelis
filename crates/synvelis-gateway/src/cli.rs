//! AUTX-02 trusted local CLI adapter — headless session identity.
//!
//! A local CLI, script runner, CI job, or scheduler must never gain
//! Human authority merely because it runs on the same machine, owns a
//! TTY, was passed `--yes`, reads an environment variable, or receives
//! JSON arguments. This module is the seam that keeps that boundary:
//! [`LocalCliAdapter`] is the crate's third [`AdapterSeal`] issuer and
//! the *only* way a local-CLI-origin [`CallerIdentity`] comes into
//! existence.
//!
//! Two attestation paths exist, and they are deliberately asymmetric:
//!
//! - **Delegated / unattended** — [`CliSessionClaim::Unattended`] and
//!   [`LocalCliAdapter::caller_delegated`] always mint an `Agent`
//!   principal on [`OriginKind::LocalCli`]. There is no argument, flag,
//!   environment variable, TTY state, or JSON field that can move this
//!   path to `Human`. The resulting caller is agent-equivalent: every
//!   operation still needs task/grant authorization evaluated by
//!   [`crate::GrantPolicy`] per call — automation without a grant has
//!   no authority at all.
//! - **Verified interactive** — a [`CliSessionStore`] session binds a
//!   `Human` principal to a random 256-bit credential whose SHA-256
//!   digest is all the store keeps. [`LocalCliAdapter::caller_verified`]
//!   resolves a presented credential and mints `Human` +
//!   `OriginKind::LocalCli` *only* when the record verifies (unknown,
//!   expired, revoked, or non-human records all fail closed with one
//!   blind verdict). Issuing a session is the host's provenance
//!   decision: this slice ships the contract and the in-memory store —
//!   OS session/credential verification (Windows logon session,
//!   macOS/Linux console session) and a durable store are deferred to
//!   the runtime slice, and an environment that cannot verify
//!   provenance simply never issues interactive sessions, which is the
//!   fail-closed answer.
//!
//! What this module deliberately does **not** do: read `std::env`,
//! inspect TTY state, parse CLI flags, or accept any caller-supplied
//! claim of humanness. [`CliSessionClaim`] is a *claim* — `attest`
//! resolves it against the session store and fails closed, so script
//! source, `--yes`, or a doctored argument vector cannot mint or
//! elevate authority (AUTX-02 AC).

use std::collections::BTreeMap;
use std::io;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::clock::{PolicyClock, PolicyInstant};
use crate::id::AdapterId;
use crate::mcp_permit::{ct_eq, mint_secret, sha256_hex};
use crate::outcome::{ErrorEnvelope, GatewayError};
use crate::request::{AdapterSeal, CallerIdentity, OriginKind, Principal, PrincipalKind};

/// The adapter endpoint identity every local-CLI caller attests
/// through — parallel to [`crate::mcp::MCP_ADAPTER_RAW`].
pub const CLI_ADAPTER_RAW: u128 = 0x434C_4902;

/// The wire prefix every CLI session credential carries — its
/// presence only selects the verifier; the secret itself is the
/// 64-hex digest after it.
pub const CLI_SECRET_PREFIX: &str = "syv-cli-";

/// What the host asks the session store to mint — everything about a
/// session except its secret, which the store generates.
///
/// The principal must be `Human`: a session credential is the *only*
/// path to human authority on the local CLI origin, so binding one to
/// an agent (or anything else) is rejected at issuance and again at
/// verification. Unattended execution never gets a session — it uses
/// [`LocalCliAdapter::caller_delegated`].
#[derive(Clone, Debug)]
pub struct CliSessionSpec {
    /// The verified human principal the session binds.
    pub principal: Principal,
    /// Time-to-live from issuance; `None` is a non-expiring session
    /// (revocation is then the only kill switch).
    pub ttl_ms: Option<u64>,
}

/// A stored session record. The `secret_hash` is private — a record
/// clone can never leak a usable credential.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliSessionRecord {
    /// The session's stable id (`cli-<16hex>`).
    pub session: String,
    /// The human principal it binds.
    pub principal: Principal,
    /// Issuance instant.
    pub issued_at: PolicyInstant,
    /// Expiry instant, when issued with a TTL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<PolicyInstant>,
    /// Recorded revocation — a revoked record stays so reissue cannot
    /// resurrect it.
    #[serde(default)]
    pub revoked: bool,
    secret_hash: String,
}

impl CliSessionRecord {
    /// Whether the record would bind at `now`.
    #[must_use]
    pub fn live(&self, now: PolicyInstant) -> bool {
        !self.revoked && self.expires_at.is_none_or(|expiry| now < expiry)
    }
}

/// What `issue` hands back — the record plus the one-time secret.
/// The secret is returned exactly once; only its digest persists.
#[derive(Clone, Debug)]
pub struct CliSessionIssue {
    /// The minted record (id, binding, validity — no secret).
    pub record: CliSessionRecord,
    /// The presentable credential (`syv-cli-<64hex>`), shown to the
    /// provisioner once and never stored.
    pub secret: String,
}

/// The session-store seam the local CLI adapter verifies against.
///
/// Mirrors [`crate::mcp_permit::PermitStore`]: verification is
/// existence-blind (unknown, expired, and revoked credentials share
/// one `None` verdict) and revocation is a recorded state, not a
/// delete.
pub trait CliSessionStore: Send + Sync {
    /// Mints a session for `spec` at `now`.
    ///
    /// # Errors
    /// [`io::Error`] on durable-write failure (a future file store);
    /// the in-memory store never fails.
    fn issue(&mut self, spec: CliSessionSpec, now: PolicyInstant) -> io::Result<CliSessionIssue>;

    /// The record `credential` resolves to at `now` — `None` for
    /// unknown, expired, or revoked credentials (one blind verdict).
    fn verify(&self, credential: &str, now: PolicyInstant) -> Option<CliSessionRecord>;

    /// Records revocation of `session`; `false` when the id is unknown.
    ///
    /// # Errors
    /// [`io::Error`] on durable-write failure.
    fn revoke(&mut self, session: &str) -> io::Result<bool>;

    /// The record for a session id (introspection — never contains a
    /// usable secret).
    fn record(&self, session: &str) -> Option<CliSessionRecord>;

    /// The number of stored records.
    fn len(&self) -> usize;

    /// Whether the store holds no records.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// In-memory session store — the seam's default and the test fixture.
/// Process restart loses sessions, which is the honest behavior for
/// the in-crate variant (a durable store is deferred with the runtime
/// slice).
#[derive(Default)]
pub struct MemoryCliSessionStore {
    records: BTreeMap<String, CliSessionRecord>,
    seq: u64,
}

impl MemoryCliSessionStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// The session/principal-kind invariant, checked against a stored
/// record — a session record must bind a `Human` principal. `issue`
/// enforces this at write time and `verify` re-checks it, so a
/// corrupted or forged record cannot mint a confused identity.
fn record_kind_ok(kind: PrincipalKind) -> bool {
    kind == PrincipalKind::Human
}

/// Validates that a session spec binds a human principal — sessions
/// are the only human path on this origin, so anything else fails
/// closed at issuance.
///
/// # Errors
/// [`GatewayError::MalformedRequest`] on a non-human principal.
pub fn check_session_kind(spec: &CliSessionSpec) -> Result<(), GatewayError> {
    if spec.principal.kind() != PrincipalKind::Human {
        return Err(GatewayError::MalformedRequest {
            reason: "cli sessions bind human principals only — unattended \
                     execution uses the delegated caller path"
                .to_owned(),
        });
    }
    Ok(())
}

fn mint_record(spec: &CliSessionSpec, now: PolicyInstant, seq_hint: u64) -> CliSessionRecord {
    CliSessionRecord {
        session: format!("cli-{seq_hint:016x}"),
        principal: spec.principal.clone(),
        issued_at: now,
        expires_at: spec
            .ttl_ms
            .map(|ttl| PolicyInstant(now.0.saturating_add(ttl))),
        revoked: false,
        secret_hash: String::new(),
    }
}

fn verify_in(
    records: &BTreeMap<String, CliSessionRecord>,
    credential: &str,
    now: PolicyInstant,
) -> Option<CliSessionRecord> {
    let hex = credential.strip_prefix(CLI_SECRET_PREFIX)?;
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let presented = sha256_hex(credential);
    let record = records
        .values()
        .find(|r| ct_eq(&r.secret_hash, &presented))?;
    if !record_kind_ok(record.principal.kind()) || !record.live(now) {
        return None;
    }
    Some(record.clone())
}

impl CliSessionStore for MemoryCliSessionStore {
    fn issue(&mut self, spec: CliSessionSpec, now: PolicyInstant) -> io::Result<CliSessionIssue> {
        check_session_kind(&spec).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        self.seq = self.seq.saturating_add(1);
        let mut record = mint_record(&spec, now, self.seq);
        // Single-issuance: an id collision keeps the existing record.
        while self.records.contains_key(&record.session) {
            self.seq = self.seq.saturating_add(1);
            record = mint_record(&spec, now, self.seq);
        }
        let secret = mint_secret(CLI_SECRET_PREFIX)?;
        record.secret_hash = sha256_hex(&secret);
        self.records.insert(record.session.clone(), record.clone());
        Ok(CliSessionIssue { record, secret })
    }

    fn verify(&self, credential: &str, now: PolicyInstant) -> Option<CliSessionRecord> {
        verify_in(&self.records, credential, now)
    }

    fn revoke(&mut self, session: &str) -> io::Result<bool> {
        match self.records.get_mut(session) {
            Some(record) => {
                record.revoked = true;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn record(&self, session: &str) -> Option<CliSessionRecord> {
        self.records.get(session).cloned()
    }

    fn len(&self) -> usize {
        self.records.len()
    }
}

/// What a local CLI invocation *claims* about its execution context.
///
/// A claim is not evidence — [`LocalCliAdapter::attest`] resolves it
/// against the session store and fails closed. Nothing in this type
/// (or anywhere else reachable from a CLI argument vector, script
/// source, environment, or TTY state) can assert humanness: the
/// `Interactive` variant carries only an opaque credential whose
/// binding was decided at issuance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CliSessionClaim {
    /// Unattended execution (CI job, scheduler, script runner) under
    /// the named agent-equivalent principal. Always resolves to
    /// `Agent` + [`OriginKind::LocalCli`] — never `Human`.
    Unattended {
        /// The delegated principal id (e.g. `"ci-nightly"`).
        principal: String,
    },
    /// Interactive execution presenting a session credential issued
    /// after verified human provenance. The stored session decides
    /// the principal; the claim carries nothing else that could
    /// influence identity.
    Interactive {
        /// The `syv-cli-<64hex>` credential to verify.
        credential: String,
    },
}

/// The trusted local CLI adapter — AUTX-02's third `AdapterSeal`
/// issuer, beside [`crate::fixture::FixtureAdapter`] and
/// [`crate::mcp_server::McpHost`].
///
/// Holds the crate-internal seal, the session store, and the policy
/// clock. Callers reach it only inside the host process (the
/// [`crate::headless::HeadlessSessionHost`] composition or a trusted
/// transport shim); there is no wire method that mints a caller.
pub struct LocalCliAdapter {
    seal: AdapterSeal,
    sessions: Mutex<Box<dyn CliSessionStore>>,
    clock: Arc<dyn PolicyClock>,
    adapter: AdapterId,
}

#[allow(clippy::result_large_err)] // typed envelopes are the contract
impl LocalCliAdapter {
    /// Builds the adapter over its seams. `adapter` is the endpoint
    /// identity every minted caller attests through — conventionally
    /// [`CLI_ADAPTER_RAW`].
    #[must_use]
    pub fn new(
        adapter: AdapterId,
        sessions: Box<dyn CliSessionStore>,
        clock: Arc<dyn PolicyClock>,
    ) -> Self {
        Self {
            seal: AdapterSeal::issue(adapter),
            sessions: Mutex::new(sessions),
            clock,
            adapter,
        }
    }

    /// The adapter endpoint identity.
    #[must_use]
    pub const fn adapter(&self) -> AdapterId {
        self.adapter
    }

    /// The policy clock instant used for session validity.
    #[must_use]
    pub fn clock(&self) -> PolicyInstant {
        self.clock.now()
    }

    /// Mints an interactive-verified human session (host-side
    /// provenance decision — see the module docs).
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — non-human principal, or store failure.
    ///
    /// # Panics
    /// Only if the session-store lock is poisoned by a panic elsewhere.
    pub fn issue_session(&self, spec: CliSessionSpec) -> Result<CliSessionIssue, ErrorEnvelope> {
        check_session_kind(&spec)?;
        self.sessions
            .lock()
            .expect("cli session store poisoned")
            .issue(spec, self.clock.now())
            .map_err(|e| {
                GatewayError::Internal {
                    reason: format!("cli session store failure: {e}"),
                }
                .into()
            })
    }

    /// Records session revocation — interaction cancellation, host
    /// shutdown, or operator action. Never deletes the record.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — unknown id or store failure.
    ///
    /// # Panics
    /// Only if the session-store lock is poisoned by a panic elsewhere.
    pub fn revoke_session(&self, session: &str) -> Result<bool, ErrorEnvelope> {
        self.sessions
            .lock()
            .expect("cli session store poisoned")
            .revoke(session)
            .map_err(|e| {
                GatewayError::Internal {
                    reason: format!("cli session store failure: {e}"),
                }
                .into()
            })
    }

    /// A stored session record by id (never carries a usable secret).
    ///
    /// # Panics
    /// Only if the session-store lock is poisoned by a panic elsewhere.
    #[must_use]
    pub fn session_record(&self, session: &str) -> Option<CliSessionRecord> {
        self.sessions
            .lock()
            .expect("cli session store poisoned")
            .record(session)
    }

    /// The delegated/unattended caller — CI, scheduler, or script
    /// execution. **Always** `Agent` + [`OriginKind::LocalCli`]: this
    /// path has no parameter, flag, or environment input that can
    /// produce `Human`, so unattended execution can never reach the
    /// unconditional human authorization path (AUTX-02 AC).
    ///
    /// The returned caller carries no authority by itself — task and
    /// grant references on each wire request are evaluated by
    /// [`crate::GrantPolicy`] exactly as for any other agent.
    ///
    /// # Errors
    /// [`GatewayError::MalformedRequest`] when `principal` is empty or
    /// over-long.
    pub fn caller_delegated(&self, principal: &str) -> Result<CallerIdentity, GatewayError> {
        let principal = Principal::new(PrincipalKind::Agent, principal)?;
        Ok(CallerIdentity::new(
            principal,
            OriginKind::LocalCli,
            &self.seal,
        ))
    }

    /// The verified-interactive caller — resolves `credential`
    /// against the session store at the current policy instant.
    ///
    /// Fails closed with [`GatewayError::Authentication`] for unknown,
    /// expired, revoked, malformed, or non-human records — one blind
    /// verdict that discloses nothing about which sessions exist.
    ///
    /// # Errors
    /// [`GatewayError::Authentication`] when the credential does not
    /// resolve to a live human session.
    ///
    /// # Panics
    /// Only if the session-store lock is poisoned by a panic elsewhere.
    pub fn caller_verified(&self, credential: &str) -> Result<CallerIdentity, GatewayError> {
        let record = self
            .sessions
            .lock()
            .expect("cli session store poisoned")
            .verify(credential, self.clock.now());
        let Some(record) = record else {
            return Err(GatewayError::Authentication);
        };
        Ok(CallerIdentity::new(
            record.principal,
            OriginKind::LocalCli,
            &self.seal,
        ))
    }

    /// Resolves a [`CliSessionClaim`] to a trusted caller — the single
    /// entry point a headless transport uses per invocation.
    ///
    /// `Unattended` always yields the delegated agent-equivalent
    /// caller; `Interactive` yields `Human` only when the credential
    /// verifies. There is no claim variant that asserts humanness
    /// without a store-verified credential.
    ///
    /// # Errors
    /// [`GatewayError`] — malformed principal id, or
    /// [`GatewayError::Authentication`] for an unverifiable
    /// interactive claim.
    pub fn attest(&self, claim: &CliSessionClaim) -> Result<CallerIdentity, GatewayError> {
        match claim {
            CliSessionClaim::Unattended { principal } => self.caller_delegated(principal),
            CliSessionClaim::Interactive { credential } => self.caller_verified(credential),
        }
    }
}
