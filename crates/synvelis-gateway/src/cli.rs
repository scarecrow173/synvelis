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
//!   decision: this slice ships the contract and two store backends —
//!   [`MemoryCliSessionStore`] for process-local use and
//!   [`FileCliSessionStore`] for durable, restart-surviving sessions
//!   (synvelis#515). The OS credential path that supplies the secret
//!   lives in the `synvelis-cli` adapter (`session issue` /
//!   `--session`); an environment that cannot verify provenance
//!   simply never issues interactive sessions, which is the
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
use std::path::{Path, PathBuf};
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
/// Process restart loses sessions; durable, restart-surviving
/// sessions use [`FileCliSessionStore`] (synvelis#515).
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

/// File-backed session store — the durable [`CliSessionStore`]
/// backend (synvelis#515).
///
/// One JSON document (`{"sessions": [...]}`) written atomically on
/// every mutation (tmp + fsync + rename), mirroring
/// [`crate::mcp_permit::FilePermitStore`]. It stores digests, never
/// secrets — the file on disk is not a usable credential list, and a
/// process restart preserves expiry/revocation/one-time-return
/// semantics exactly. Opening re-validates the Human-kind invariant
/// on every record: a file that violates it is corrupt or forged and
/// fails closed rather than serving confused identities.
pub struct FileCliSessionStore {
    path: PathBuf,
    records: BTreeMap<String, CliSessionRecord>,
    seq: u64,
}

impl FileCliSessionStore {
    /// Opens (or creates) the store at `path`.
    ///
    /// # Errors
    /// [`io::Error`] when an existing file cannot be read or parsed,
    /// or when a record violates the Human-kind invariant.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut records = BTreeMap::new();
        let mut seq = 0u64;
        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let file: CliSessionFile = serde_json::from_str(&text)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            for record in file.sessions {
                // A record violating the kind invariant was never
                // issued by this store — the file is corrupt or
                // forged, and the safest answer is not to serve it.
                if !record_kind_ok(record.principal.kind()) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "cli session {} violates the human-kind invariant",
                            record.session
                        ),
                    ));
                }
                if let Some(hex) = record.session.strip_prefix("cli-")
                    && let Ok(n) = u64::from_str_radix(hex, 16)
                {
                    seq = seq.max(n);
                }
                records.insert(record.session.clone(), record);
            }
        }
        Ok(Self { path, records, seq })
    }

    /// Every stored record (introspection — never a usable secret).
    #[must_use]
    pub fn records(&self) -> Vec<CliSessionRecord> {
        self.records.values().cloned().collect()
    }

    /// Persists the current records (tmp + fsync + rename for
    /// atomicity — the tmp write is durable before it replaces the
    /// live file).
    fn flush(&self) -> io::Result<()> {
        use std::io::Write as _;
        let file = CliSessionFile {
            sessions: self.records.values().cloned().collect(),
        };
        let text = serde_json::to_string_pretty(&file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let tmp = self.path.with_extension("json.tmp");
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)
    }
}

#[derive(Serialize, Deserialize)]
struct CliSessionFile {
    sessions: Vec<CliSessionRecord>,
}

impl CliSessionStore for FileCliSessionStore {
    fn issue(&mut self, spec: CliSessionSpec, now: PolicyInstant) -> io::Result<CliSessionIssue> {
        check_session_kind(&spec).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        self.seq = self.seq.saturating_add(1);
        let mut record = mint_record(&spec, now, self.seq);
        while self.records.contains_key(&record.session) {
            self.seq = self.seq.saturating_add(1);
            record = mint_record(&spec, now, self.seq);
        }
        let secret = mint_secret(CLI_SECRET_PREFIX)?;
        record.secret_hash = sha256_hex(&secret);
        self.records.insert(record.session.clone(), record.clone());
        self.flush()?;
        Ok(CliSessionIssue { record, secret })
    }

    fn verify(&self, credential: &str, now: PolicyInstant) -> Option<CliSessionRecord> {
        verify_in(&self.records, credential, now)
    }

    fn revoke(&mut self, session: &str) -> io::Result<bool> {
        match self.records.get_mut(session) {
            Some(record) => {
                record.revoked = true;
                self.flush()?;
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
