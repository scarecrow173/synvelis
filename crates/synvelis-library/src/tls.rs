//! TLS trust configuration for provider HTTPS transports.
//!
//! [`TlsTrust`] wraps a fully-built `rustls::ClientConfig` so no
//! `rustls` type appears in the public API. The trust decision is
//! per-call through [`crate::ProviderContext::tls`]: `None` uses the
//! platform's native root store (loaded lazily, once per process);
//! `Some` uses the caller-supplied roots — the seam tests and
//! private-CA deployments use. Protocol versions, cipher suites, and
//! the `ring` crypto provider are fixed secure defaults; callers
//! choose *which roots to trust*, never how to weaken verification.
//!
//! Certificate contents, peer-supplied data, and URL secrets never
//! appear in error details — [`crate::ProviderErrorKind::TlsVerification`]
//! carries the classification, the detail carries only rustls's own
//! sanitized diagnostic text.

use std::sync::{Arc, OnceLock};

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, RootCertStore};

use crate::error::{ProviderError, ProviderErrorKind};
use crate::webdav::url::WebDavUrl;

/// TLS trust roots for provider HTTPS calls.
///
/// Construct via [`TlsTrust::native`] (platform root store) or
/// [`TlsTrust::from_pem`] (explicit CA certificates) and assign to
/// [`crate::ProviderContext::tls`]. Cloning is cheap (shared
/// `Arc<ClientConfig>`).
#[derive(Clone)]
pub struct TlsTrust {
    config: Arc<ClientConfig>,
}

impl TlsTrust {
    /// Trust the platform's native root certificate store (OS trust:
    /// schannel on Windows, Security.framework on macOS, the system
    /// store elsewhere).
    ///
    /// # Errors
    /// [`ProviderErrorKind::Internal`] when the platform store yields
    /// no usable CA certificate — an environment deficiency, not a
    /// peer failure.
    pub fn native() -> Result<Self, ProviderError> {
        let result = rustls_native_certs::load_native_certs();
        let mut roots = RootCertStore::empty();
        let (added, _ignored) = roots.add_parsable_certificates(result.certs);
        if added == 0 {
            return Err(ProviderError::new(
                ProviderErrorKind::Internal,
                "platform TLS root store yielded no usable CA certificates",
            ));
        }
        Ok(Self {
            config: Arc::new(build_config(roots)?),
        })
    }

    /// Trust the PEM-encoded CA certificates in `pem` — private CAs,
    /// enterprise roots, or test fixtures. Every `CERTIFICATE` PEM
    /// section is added; sections that fail to parse as trust anchors
    /// are skipped, and at least one must succeed.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when `pem` contains no
    /// parseable CA certificate — the caller supplied unusable input.
    pub fn from_pem(pem: &[u8]) -> Result<Self, ProviderError> {
        let invalid = |detail: &str| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("invalid tls trust pem: {detail}"),
            )
        };
        let certs = CertificateDer::pem_slice_iter(pem)
            .collect::<Result<Vec<CertificateDer<'static>>, _>>()
            .map_err(|_| invalid("malformed PEM section"))?;
        if certs.is_empty() {
            return Err(invalid("no PEM certificates found"));
        }
        let mut roots = RootCertStore::empty();
        let (added, _ignored) = roots.add_parsable_certificates(certs);
        if added == 0 {
            return Err(invalid("no usable CA certificates"));
        }
        Ok(Self {
            config: Arc::new(build_config(roots)?),
        })
    }

    /// The shared client configuration.
    pub(crate) fn config(&self) -> Arc<ClientConfig> {
        self.config.clone()
    }
}

/// The lazily-built native-roots configuration — loaded at most once
/// per process, shared by every HTTPS call that does not override
/// [`crate::ProviderContext::tls`].
static NATIVE_CONFIG: OnceLock<Result<Arc<ClientConfig>, ProviderError>> = OnceLock::new();

/// Resolves the `ClientConfig` for one HTTPS call: the context's
/// explicit [`TlsTrust`] when present, else the shared native store.
pub(crate) fn client_config(trust: Option<&TlsTrust>) -> Result<Arc<ClientConfig>, ProviderError> {
    match trust {
        Some(t) => Ok(t.config()),
        None => NATIVE_CONFIG
            .get_or_init(|| TlsTrust::native().map(|t| t.config))
            .clone(),
    }
}

/// Per-connection TLS parameters for one HTTPS endpoint: the shared
/// client configuration plus the verified server name derived from
/// the endpoint's host. Carried on [`crate::webdav::http::HttpEndpoint`]
/// so the transport never re-derives trust per request.
#[derive(Clone)]
pub(crate) struct TlsSession {
    /// Shared client configuration (roots + protocol defaults).
    pub config: Arc<ClientConfig>,
    /// The name the peer certificate must verify against — an IP
    /// literal yields `ServerName::IpAddress`, never a DNS name.
    pub server_name: ServerName<'static>,
}

impl std::fmt::Debug for TlsSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The client config is opaque trust material — only the
        // verified server name is safe to expose.
        f.debug_struct("TlsSession")
            .field("server_name", &self.server_name)
            .finish_non_exhaustive()
    }
}

/// Builds the [`TlsSession`] for `url`'s origin under `trust`.
///
/// # Errors
/// [`ProviderErrorKind::InvalidRequest`] when the host cannot form a
/// TLS server name (unreachable — `WebDavUrl` already constrains the
/// grammar), or the trust configuration itself fails.
pub(crate) fn session_for(
    url: &WebDavUrl,
    trust: Option<&TlsTrust>,
) -> Result<TlsSession, ProviderError> {
    Ok(TlsSession {
        config: client_config(trust)?,
        server_name: server_name(url)?,
    })
}

/// The verified server name for `url`'s host. IP literals (IPv4
/// reg-name spellings like `127.0.0.1` and bracketed IPv6) become
/// `ServerName::IpAddress` so webpki checks `iPAddress` SANs;
/// everything else is a DNS name — `WebDavUrl`'s visible-ASCII
/// reg-name grammar is a strict subset of valid DNS names.
fn server_name(url: &WebDavUrl) -> Result<ServerName<'static>, ProviderError> {
    let host = url.host();
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return Ok(ServerName::from(ip));
    }
    ServerName::try_from(host.to_owned()).map_err(|_| {
        ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "tls server name cannot be derived from the remote host",
        )
    })
}

/// A `ClientConfig` with the workspace-pinned `ring` provider, TLS
/// 1.2 + 1.3, and the given root store. No client certificates, no
/// dangerous overrides — verification stays at webpki defaults. The
/// provider is named explicitly so a process-level default can never
/// silently substitute a different crypto backend.
fn build_config(roots: RootCertStore) -> Result<ClientConfig, ProviderError> {
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|e| {
                ProviderError::new(
                    ProviderErrorKind::Internal,
                    format!("tls protocol versions unavailable: {e}"),
                )
            })?
            .with_root_certificates(roots)
            .with_no_client_auth();
    Ok(config)
}
