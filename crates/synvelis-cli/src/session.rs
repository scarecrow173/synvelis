//! OS credential session supply (synvelis#515).
//!
//! The durable [`synvelis_gateway::cli::FileCliSessionStore`] keeps
//! session *records* (digest-only); the usable secret lives in the OS
//! credential store, bound to the OS user account. This module is the
//! seam between the two:
//!
//! - [`store_path`] resolves the durable store location —
//!   `--session-store` when given, else the platform user-state
//!   directory (environment variables locate the directory only;
//!   they are never authority evidence).
//! - [`resolve_credential`] reads the secret for `--session <id>`
//!   in-process, so the secret never touches argv/env/logs.
//! - `vault_*` is the OS credential store: Windows Credential
//!   Manager via the `keyring` crate. Other platforms report
//!   [`CliError::Unsupported`] — fail-closed, honestly recorded.
//!
//! `session issue` is operator provisioning (the same relationship
//! `mcp-poc permit` has to `authorized_keys`): any local process can
//! run it, but minting a *usable* credential requires read access to
//! the OS user account's credential store. Interactive-presence
//! attestation (credential prompt / Windows Hello) is deferred to the
//! #114 lane; this slice binds the secret to the OS user account.

use std::path::{Path, PathBuf};

use synvelis_gateway::cli::{CliSessionSpec, CliSessionStore, FileCliSessionStore};
use synvelis_gateway::{PolicyClock, Principal, PrincipalKind, SystemClock};

use crate::error::CliError;

/// Credential-store service name every synvelis CLI session secret is
/// filed under (per-OS-user on Windows Credential Manager).
#[cfg(windows)]
const VAULT_SERVICE: &str = "synvelis-cli";

/// The vault username for a session: session ids are per-store
/// sequence values, so two different `--session-store` files can mint
/// the same id. Scoping the vault entry by a hash of the resolved
/// store path keeps credentials store-local — `issue`/`revoke`/
/// `--session` all derive the same scope from the same flag, and a
/// credential can never be claimed through a different store file.
/// (The hash is a namespace discriminator, not a secret.)
#[cfg(windows)]
fn vault_user(store_path_arg: Option<&Path>, session_id: &str) -> Result<String, CliError> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let path = store_path(store_path_arg)?;
    let mut h = DefaultHasher::new();
    path.to_string_lossy().as_bytes().hash(&mut h);
    Ok(format!("{:016x}-{session_id}", h.finish()))
}

/// Resolves the durable session-store path: `--session-store` when
/// given, else the platform user-state location.
///
/// # Errors
/// [`CliError::Setup`] when no platform user-state directory can be
/// resolved.
pub fn store_path(explicit: Option<&Path>) -> Result<PathBuf, CliError> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }
    default_store_path()
}

/// The platform user-state default (`cli-sessions.json` under the
/// per-user state directory). Environment variables are used for
/// *location* only — never as authority evidence.
fn default_store_path() -> Result<PathBuf, CliError> {
    let dir = user_state_dir().ok_or_else(|| {
        CliError::Setup(
            "cannot resolve a user-state directory for the CLI session \
             store — pass --session-store <path> explicitly"
                .to_owned(),
        )
    })?;
    Ok(dir.join("cli-sessions.json"))
}

#[cfg(target_os = "windows")]
fn user_state_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("synvelis"))
}

#[cfg(target_os = "macos")]
fn user_state_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|p| {
        PathBuf::from(p)
            .join("Library")
            .join("Application Support")
            .join("synvelis")
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn user_state_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_STATE_HOME") {
        return Some(PathBuf::from(xdg).join("synvelis"));
    }
    std::env::var_os("HOME").map(|p| {
        PathBuf::from(p)
            .join(".local")
            .join("state")
            .join("synvelis")
    })
}

/// Opens the durable store at the resolved path. Read paths never
/// write — the parent directory is created lazily by the store's own
/// flush on first mutation (dry-run/read-only boots stay write-free).
///
/// # Errors
/// [`CliError::Setup`] on path/IO/parse failure — a corrupt or forged
/// store file fails closed rather than serving sessions.
pub fn open_store(explicit: Option<&Path>) -> Result<FileCliSessionStore, CliError> {
    let path = store_path(explicit)?;
    FileCliSessionStore::open(&path).map_err(|e| {
        CliError::Setup(format!(
            "cannot open session store '{}': {e}",
            path.display()
        ))
    })
}

/// The default principal id `session issue` binds when `--user` is
/// absent — the OS account name (a label chosen by the operator at
/// issuance, not authority evidence).
#[must_use]
pub fn default_user() -> String {
    #[cfg(windows)]
    {
        std::env::var("USERNAME").unwrap_or_else(|_| "operator".to_owned())
    }
    #[cfg(not(windows))]
    {
        std::env::var("USER").unwrap_or_else(|_| "operator".to_owned())
    }
}

/// `session issue`: mints a human session in the durable store and
/// files the one-time secret in the OS credential store under the
/// session id. The returned record never contains the secret; the
/// secret itself is handed to the vault and dropped.
///
/// # Errors
/// [`CliError::Setup`] on store/vault failure — a store record whose
/// vault write failed is revoked again before returning (no orphan
/// live sessions).
pub fn issue(
    store_path_arg: Option<&Path>,
    ttl_ms: Option<u64>,
    user: Option<&str>,
) -> Result<synvelis_gateway::cli::CliSessionIssue, CliError> {
    if !vault_supported() {
        return Err(CliError::Unsupported(
            "OS credential store is implemented for Windows only in this              slice (Credential Manager); macOS/Linux report Unsupported"
                .to_owned(),
        ));
    }
    let mut store = open_store(store_path_arg)?;
    let clock = SystemClock;
    let now = PolicyClock::now(&clock);
    let spec = CliSessionSpec {
        principal: Principal::new(PrincipalKind::Human, user.unwrap_or(&default_user()))
            .map_err(|e| CliError::InvalidInput(format!("invalid --user principal: {e}")))?,
        ttl_ms,
    };
    let issued = store
        .issue(spec, now)
        .map_err(|e| CliError::Setup(format!("cannot issue session: {e}")))?;
    if let Err(e) = vault_set(store_path_arg, &issued.record.session, &issued.secret) {
        // Compensation: never leave a live record whose secret was
        // never provisioned (it could never verify anyway, but the
        // store should not accumulate dead sessions).
        let _ = store.revoke(&issued.record.session);
        return Err(e);
    }
    Ok(issued)
}

/// `session revoke`: records revocation in the durable store (the
/// authority) and best-effort deletes the OS vault entry.
///
/// # Errors
/// [`CliError::Setup`] on store failure; unknown ids are reported by
/// the caller via the returned `bool`.
pub fn revoke(store_path_arg: Option<&Path>, session_id: &str) -> Result<bool, CliError> {
    let mut store = open_store(store_path_arg)?;
    let revoked = store
        .revoke(session_id)
        .map_err(|e| CliError::Setup(format!("cannot revoke session: {e}")))?;
    if revoked {
        // Record revocation is authoritative; a vault delete failure
        // leaves an unusable orphan (the record already fails closed).
        let _ = vault_delete(store_path_arg, session_id);
    }
    Ok(revoked)
}

/// Lists every stored session record (digest-only — never a usable
/// secret).
///
/// # Errors
/// [`CliError::Setup`] on store failure.
pub fn list(
    store_path_arg: Option<&Path>,
) -> Result<Vec<synvelis_gateway::cli::CliSessionRecord>, CliError> {
    let store = open_store(store_path_arg)?;
    Ok(store.records())
}

/// Resolves `--session <id>` to the presentable credential by reading
/// the OS credential store in-process — the secret never appears on
/// argv, in env, or in logs.
///
/// `store_path_arg` must match the `--session-store` used at issue —
/// the vault entry is scoped to that store file.
///
/// # Errors
/// [`CliError::Unsupported`] on platforms without an OS credential
/// implementation; [`CliError::Setup`] on vault failures;
/// [`CliError::InvalidInput`] when the session has no vault entry
/// (unknown or already cleaned up — fail closed).
pub fn resolve_credential(
    store_path_arg: Option<&Path>,
    session_id: &str,
) -> Result<String, CliError> {
    vault_get(store_path_arg, session_id)
}

// ── OS credential vault ──────────────────────────────────────────

#[cfg(windows)]
fn vault_entry(user: &str) -> Result<keyring::Entry, CliError> {
    keyring::Entry::new(VAULT_SERVICE, user)
        .map_err(|e| CliError::Setup(format!("cannot open OS credential store: {e}")))
}

#[cfg(windows)]
fn vault_set(
    store_path_arg: Option<&Path>,
    session_id: &str,
    secret: &str,
) -> Result<(), CliError> {
    vault_entry(&vault_user(store_path_arg, session_id)?)?
        .set_password(secret)
        .map_err(|e| CliError::Setup(format!("cannot store session credential: {e}")))
}

#[cfg(windows)]
fn vault_get(store_path_arg: Option<&Path>, session_id: &str) -> Result<String, CliError> {
    match vault_entry(&vault_user(store_path_arg, session_id)?)?.get_password() {
        Ok(secret) => Ok(secret),
        Err(keyring::Error::NoEntry) => Err(CliError::InvalidInput(format!(
            "session '{session_id}' has no credential in the OS store"
        ))),
        Err(e) => Err(CliError::Setup(format!(
            "cannot read session credential: {e}"
        ))),
    }
}

#[cfg(windows)]
fn vault_delete(store_path_arg: Option<&Path>, session_id: &str) -> Result<(), CliError> {
    match vault_entry(&vault_user(store_path_arg, session_id)?)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(CliError::Setup(format!(
            "cannot delete session credential: {e}"
        ))),
    }
}

#[cfg(not(windows))]
fn vault_set(
    _store_path_arg: Option<&Path>,
    _session_id: &str,
    _secret: &str,
) -> Result<(), CliError> {
    Err(CliError::Unsupported(
        "OS credential store is implemented for Windows only in this \
         slice (Credential Manager); macOS/Linux report Unsupported"
            .to_owned(),
    ))
}

#[cfg(not(windows))]
fn vault_get(_store_path_arg: Option<&Path>, _session_id: &str) -> Result<String, CliError> {
    Err(CliError::Unsupported(
        "OS credential store is implemented for Windows only in this \
         slice (Credential Manager); macOS/Linux report Unsupported"
            .to_owned(),
    ))
}

#[cfg(not(windows))]
fn vault_delete(_store_path_arg: Option<&Path>, _session_id: &str) -> Result<(), CliError> {
    Err(CliError::Unsupported(
        "OS credential store is implemented for Windows only in this \
         slice (Credential Manager); macOS/Linux report Unsupported"
            .to_owned(),
    ))
}

/// Whether the platform has an OS credential implementation — used
/// for honest Unsupported reporting.
#[must_use]
pub fn vault_supported() -> bool {
    cfg!(windows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_path_prefers_explicit() {
        let p = Path::new("some/dir/store.json");
        assert_eq!(store_path(Some(p)).unwrap(), p);
    }
}
