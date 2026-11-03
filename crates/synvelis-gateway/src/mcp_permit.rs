//! GW-08 connection permits — the trusted-identity binding (GIA-004).
//!
//! A connection permit is how the *host* tells the adapter "a session
//! presenting this secret on this channel is this principal" — it is
//! the MCP-side analog of GW-02's grant issuance, scoped to transport
//! authentication rather than operation authority. Holding the secret
//! binds the session's trusted [`crate::CallerIdentity`]; it grants
//! **no** capability, scope, or data access — every invocation still
//! passes per-call policy evaluation (discovery is not execution
//! permission).
//!
//! Properties enforced here:
//!
//! - A permit is bound to one [`McpChannel`] — a client credential
//!   presented on the control channel (or vice versa) fails closed.
//! - Secrets are random 256-bit values; only their SHA-256 digest is
//!   stored, so the durable file never carries a usable credential.
//! - `verify` is **existence-blind**: unknown, wrong-channel, expired,
//!   and revoked secrets all return `None` — a probe learns nothing
//!   about which permits exist.
//! - `revoke` records the revocation rather than deleting the record,
//!   so a reissue under a recycled id can never silently resurrect a
//!   dead permit.
//!
//! Permit lifecycle is *host-side administration*: the `PoC` provisions
//! the first credential out-of-band (the `mcp-poc permit` CLI writing
//! the credential file — the same relationship `sshd` has to
//! `authorized_keys`), and an operator session can issue downstream
//! permits through the control channel. Neither path is a wire
//! capability an agent can reach.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use synvelis_project::ProjectId;

use crate::clock::PolicyInstant;
use crate::id::{GrantRef, TaskRef};
use crate::mcp::McpChannel;
use crate::outcome::GatewayError;
use crate::request::Principal;

/// The wire prefix every permit secret carries — its presence only
/// selects the verifier; the secret itself is the 64-hex digest
/// after it.
pub const SECRET_PREFIX: &str = "syv-";

/// What the host asks the store to mint — everything about a permit
/// except its secret, which the store generates.
#[derive(Clone, Debug)]
pub struct PermitSpec {
    /// The channel this credential may be presented on.
    pub channel: McpChannel,
    /// The trusted principal a session presenting it binds to.
    pub principal: Principal,
    /// The project the bound session may address.
    pub project: ProjectId,
    /// The task reference a client-channel session carries into
    /// every request (opaque — GW-02 evaluates it per call).
    pub task: Option<TaskRef>,
    /// The grant reference a client-channel session carries into
    /// every request (opaque — GW-02 evaluates it per call).
    pub grant: Option<GrantRef>,
    /// Time-to-live from issuance; `None` is a non-expiring permit
    /// (revocation is then the only kill switch).
    pub ttl_ms: Option<u64>,
}

/// A stored permit record. The `secret_hash` is private — a record
/// clone can never leak a usable credential.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermitRecord {
    /// The permit's stable id (`permit-<16hex>`).
    pub permit: String,
    /// The channel it may be presented on.
    pub channel: McpChannel,
    /// The principal it binds.
    pub principal: Principal,
    /// The project it scopes to.
    pub project: ProjectId,
    /// Bound task reference (client permits).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskRef>,
    /// Bound grant reference (client permits).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant: Option<GrantRef>,
    /// Issuance instant.
    pub issued_at: PolicyInstant,
    /// Expiry instant, when the permit was issued with a TTL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<PolicyInstant>,
    /// Recorded revocation — a revoked record stays so reissue
    /// cannot resurrect it.
    #[serde(default)]
    pub revoked: bool,
    secret_hash: String,
}

impl PermitRecord {
    /// Whether the record would bind at `now` (channel already
    /// matched by the caller of `PermitStore::verify`).
    #[must_use]
    pub fn live(&self, now: PolicyInstant) -> bool {
        !self.revoked && self.expires_at.is_none_or(|expiry| now < expiry)
    }
}

/// What `issue` hands back — the record plus the one-time secret.
/// The secret is returned exactly once; only its digest persists.
#[derive(Clone, Debug)]
pub struct PermitIssue {
    /// The minted record (id, binding, validity — no secret).
    pub record: PermitRecord,
    /// The presentable secret (`syv-<64hex>`), shown to the
    /// provisioner once and never stored.
    pub secret: String,
}

/// The credential-store seam the session host verifies against.
pub trait PermitStore: Send + Sync {
    /// Mints a permit for `spec` at `now`.
    ///
    /// # Errors
    /// [`io::Error`] on durable-write failure (file store); the in-memory
    /// store never fails.
    fn issue(&mut self, spec: PermitSpec, now: PolicyInstant) -> io::Result<PermitIssue>;

    /// The record `secret` binds to on `channel` at `now` — `None`
    /// for unknown, wrong-channel, expired, or revoked secrets (one
    /// blind verdict).
    fn verify(&self, secret: &str, channel: McpChannel, now: PolicyInstant)
    -> Option<PermitRecord>;

    /// Records revocation of `permit`; `false` when the id is unknown.
    ///
    /// # Errors
    /// [`io::Error`] on durable-write failure.
    fn revoke(&mut self, permit: &str) -> io::Result<bool>;

    /// The record for a permit id (control-plane introspection —
    /// never contains a usable secret).
    fn record(&self, permit: &str) -> Option<PermitRecord>;

    /// The number of stored records.
    fn len(&self) -> usize;

    /// Whether the store holds no records.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn sha256_hex(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn mint_secret() -> io::Result<String> {
    // 256 bits of OS entropy — the file's `secret_hash` must never be
    // a preimage oracle, so a CSPRNG is required here.
    let mut bytes = [0u8; 32];
    rand::RngCore::try_fill_bytes(&mut rand::rngs::OsRng, &mut bytes).map_err(io::Error::other)?;
    let mut hex = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(format!("{SECRET_PREFIX}{hex}"))
}

fn mint_record(spec: &PermitSpec, now: PolicyInstant, seq_hint: u64) -> PermitRecord {
    let permit_id = format!("permit-{seq_hint:016x}");
    PermitRecord {
        permit: permit_id,
        channel: spec.channel,
        principal: spec.principal.clone(),
        project: spec.project,
        task: spec.task.clone(),
        grant: spec.grant.clone(),
        issued_at: now,
        expires_at: spec
            .ttl_ms
            .map(|ttl| PolicyInstant(now.0.saturating_add(ttl))),
        revoked: false,
        secret_hash: String::new(),
    }
}

/// Constant-time hex equality — a `String ==` early-exit leaks
/// prefix length through timing.
fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// The channel/principal-kind invariant, checked against a stored
/// record — a `Client` record must bind an agent and a `Control`
/// record a human. `issue` enforces this at write time and `open`
/// re-validates it, so a forged or corrupted file record cannot
/// mint a confused identity.
fn record_kind_ok(channel: McpChannel, kind: crate::request::PrincipalKind) -> bool {
    use crate::request::PrincipalKind;
    let expected = match channel {
        McpChannel::Client => PrincipalKind::Agent,
        McpChannel::Control => PrincipalKind::Human,
    };
    kind == expected
}

fn verify_in(
    records: &BTreeMap<String, PermitRecord>,
    secret: &str,
    channel: McpChannel,
    now: PolicyInstant,
) -> Option<PermitRecord> {
    let hex = secret.strip_prefix(SECRET_PREFIX)?;
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let presented = sha256_hex(secret);
    let record = records
        .values()
        .find(|r| ct_eq(&r.secret_hash, &presented))?;
    if record.channel != channel
        || !record_kind_ok(record.channel, record.principal.kind())
        || !record.live(now)
    {
        return None;
    }
    Some(record.clone())
}

/// In-memory permit store — the seam's mock implementation and the
/// default for tests. Process restart loses permits, which is the
/// honest behavior for the in-crate variant (the file store exists
/// for the `PoC` transport).
#[derive(Default)]
pub struct MemoryPermitStore {
    records: BTreeMap<String, PermitRecord>,
    seq: u64,
}

impl MemoryPermitStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl PermitStore for MemoryPermitStore {
    fn issue(&mut self, spec: PermitSpec, now: PolicyInstant) -> io::Result<PermitIssue> {
        check_channel_kind(&spec).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        self.seq = self.seq.saturating_add(1);
        let mut record = mint_record(&spec, now, self.seq);
        // Single-issuance: an id collision keeps the existing record.
        while self.records.contains_key(&record.permit) {
            self.seq = self.seq.saturating_add(1);
            record = mint_record(&spec, now, self.seq);
        }
        let secret = mint_secret()?;
        record.secret_hash = sha256_hex(&secret);
        self.records.insert(record.permit.clone(), record.clone());
        Ok(PermitIssue { record, secret })
    }

    fn verify(
        &self,
        secret: &str,
        channel: McpChannel,
        now: PolicyInstant,
    ) -> Option<PermitRecord> {
        verify_in(&self.records, secret, channel, now)
    }

    fn revoke(&mut self, permit: &str) -> io::Result<bool> {
        match self.records.get_mut(permit) {
            Some(record) => {
                record.revoked = true;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn record(&self, permit: &str) -> Option<PermitRecord> {
        self.records.get(permit).cloned()
    }

    fn len(&self) -> usize {
        self.records.len()
    }
}

/// File-backed permit store — the `PoC`'s durable credential surface.
///
/// One JSON document (`{"permits": [...]}`) written atomically on
/// every mutation (tmp + rename). It stores digests, never secrets —
/// the file on disk is not a usable credential list. Provisioning the
/// first credential is deliberately an *offline file write* (the
/// `mcp-poc permit` CLI), not a wire capability.
pub struct FilePermitStore {
    path: PathBuf,
    records: BTreeMap<String, PermitRecord>,
    seq: u64,
}

impl FilePermitStore {
    /// Opens (or creates) the store at `path`.
    ///
    /// # Errors
    /// [`io::Error`] when an existing file cannot be read or parsed.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut records = BTreeMap::new();
        let mut seq = 0u64;
        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let file: PermitFile = serde_json::from_str(&text)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            for record in file.permits {
                // A record violating the channel/kind invariant was
                // never issued by this store — the file is corrupt or
                // forged, and the safest answer is not to serve it.
                if !record_kind_ok(record.channel, record.principal.kind()) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "permit {} violates the channel/kind invariant",
                            record.permit
                        ),
                    ));
                }
                if let Some(hex) = record.permit.strip_prefix("permit-")
                    && let Ok(n) = u64::from_str_radix(hex, 16)
                {
                    seq = seq.max(n);
                }
                records.insert(record.permit.clone(), record);
            }
        }
        Ok(Self { path, records, seq })
    }

    /// Persists the current records (tmp + fsync + rename for
    /// atomicity — the tmp write is durable before it replaces the
    /// live file).
    fn flush(&self) -> io::Result<()> {
        use std::io::Write as _;
        let file = PermitFile {
            permits: self.records.values().cloned().collect(),
        };
        let text = serde_json::to_string_pretty(&file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)
    }
}

#[derive(Serialize, Deserialize)]
struct PermitFile {
    permits: Vec<PermitRecord>,
}

impl PermitStore for FilePermitStore {
    fn issue(&mut self, spec: PermitSpec, now: PolicyInstant) -> io::Result<PermitIssue> {
        check_channel_kind(&spec).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        self.seq = self.seq.saturating_add(1);
        let mut record = mint_record(&spec, now, self.seq);
        while self.records.contains_key(&record.permit) {
            self.seq = self.seq.saturating_add(1);
            record = mint_record(&spec, now, self.seq);
        }
        let secret = mint_secret()?;
        record.secret_hash = sha256_hex(&secret);
        self.records.insert(record.permit.clone(), record.clone());
        self.flush()?;
        Ok(PermitIssue { record, secret })
    }

    fn verify(
        &self,
        secret: &str,
        channel: McpChannel,
        now: PolicyInstant,
    ) -> Option<PermitRecord> {
        verify_in(&self.records, secret, channel, now)
    }

    fn revoke(&mut self, permit: &str) -> io::Result<bool> {
        match self.records.get_mut(permit) {
            Some(record) => {
                record.revoked = true;
                self.flush()?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn record(&self, permit: &str) -> Option<PermitRecord> {
        self.records.get(permit).cloned()
    }

    fn len(&self) -> usize {
        self.records.len()
    }
}

/// Maps a presented permit record into the channel's attestation —
/// the only place a permit becomes adapter trust. `Client` attests
/// [`crate::OriginKind::ExternalAdapter`]; `Control` attests
/// [`crate::OriginKind::ManualUi`] — the `PoC`'s stand-in for the
/// Synvelis-side trusted approval surface GW-09's UI will own.
///
/// `seal` is `pub(crate)`-minted, so trusted identity can only be
/// produced inside this crate even though the record is serializable.
#[must_use]
pub(crate) fn caller_for(
    record: &PermitRecord,
    seal: &crate::request::AdapterSeal,
) -> crate::CallerIdentity {
    let origin = match record.channel {
        McpChannel::Client => crate::request::OriginKind::ExternalAdapter,
        McpChannel::Control => crate::request::OriginKind::ManualUi,
    };
    crate::CallerIdentity::new(record.principal.clone(), origin, seal)
}

/// Validates that a permit's declared principal is consistent with
/// its channel — a `Client` permit must bind an agent principal and a
/// `Control` permit a human one; cross-kind binds fail closed at
/// issuance rather than minting a confused identity.
///
/// # Errors
/// [`GatewayError::MalformedRequest`] on a channel/kind mismatch.
pub fn check_channel_kind(spec: &PermitSpec) -> Result<(), GatewayError> {
    use crate::request::PrincipalKind;
    let expected = match spec.channel {
        McpChannel::Client => PrincipalKind::Agent,
        McpChannel::Control => PrincipalKind::Human,
    };
    if spec.principal.kind() != expected {
        return Err(GatewayError::MalformedRequest {
            reason: format!(
                "{:?} channel permits bind {:?} principals only",
                spec.channel, expected
            ),
        });
    }
    Ok(())
}
