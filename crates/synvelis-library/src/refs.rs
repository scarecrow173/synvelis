//! Provider asset references and pin proofs.
//!
//! A [`RemoteAssetRef`] names an asset *inside* a provider. A
//! [`PinnedAssetRef`] additionally carries evidence that a specific
//! object revision can be fetched later: either an immutable-object
//! reference (commit/tree/CAS blob) or a strong conditional token
//! (strong content hash or server validator enforced at fetch time).
//! Weak validators — weak `ETag`s, branch names, mtimes, sizes alone —
//! are never pin evidence.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::{OpaqueLocator, ProviderId};

/// A credential-free reference to an asset inside one provider.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteAssetRef {
    /// Owning provider connection.
    pub provider: ProviderId,
    /// Provider-internal, credential-free locator.
    pub locator: OpaqueLocator,
}

/// Names an immutable revision on the provider side.
///
/// Examples: `"commit:<git-oid>"`, `"tree:<git-oid>"`,
/// `"blob:<content-digest>"`, `"rev:<svn-revision>"`. The scheme prefix
/// is provider-owned; consumers only store and compare the string.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct RevisionKey(String);

impl RevisionKey {
    /// Wraps a revision key.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when empty.
    pub fn new(text: impl Into<String>) -> Result<Self, ProviderError> {
        let text = text.into();
        if text.is_empty() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "revision key is empty",
            ));
        }
        Ok(Self(text))
    }

    /// The raw key text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RevisionKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::new(text).map_err(serde::de::Error::custom)
    }
}

/// Names an object identity on the provider side.
///
/// Distinct from [`RevisionKey`]: a revision names *when/where* in
/// history, an object identity names *which object*. For a
/// content-addressed store they coincide; for a Git remote the object
/// is the blob/tree OID while the revision is the commit.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ObjectIdentity(String);

impl ObjectIdentity {
    /// Wraps an object identity.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when empty.
    pub fn new(text: impl Into<String>) -> Result<Self, ProviderError> {
        let text = text.into();
        if text.is_empty() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "object identity is empty",
            ));
        }
        Ok(Self(text))
    }

    /// The raw identity text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ObjectIdentity {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::new(text).map_err(serde::de::Error::custom)
    }
}

/// The digest algorithms the library can compute and verify.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DigestAlgorithm {
    /// SHA-256 over the raw bytes.
    Sha256,
}

impl DigestAlgorithm {
    /// The canonical algorithm tag (`"sha256"`).
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
        }
    }
}

/// An algorithm-tagged strong content digest (`"sha256:<64hex>"`).
///
/// This is the *new* digest namespace for provider verification. It is
/// deliberately not interchangeable with the legacy CRC32-pair digest
/// used by `synvelis_player::import` blobs: that scheme is kept as an
/// [`ObjectIdentity`] value, never promoted to a strong digest.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentDigest {
    algorithm: DigestAlgorithm,
    hex: String,
}

impl ContentDigest {
    /// Builds a digest from raw digest bytes.
    #[must_use]
    pub fn new(algorithm: DigestAlgorithm, digest_bytes: &[u8]) -> Self {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut hex = String::with_capacity(digest_bytes.len() * 2);
        for &b in digest_bytes {
            hex.push(HEX[usize::from(b >> 4)] as char);
            hex.push(HEX[usize::from(b & 0x0f)] as char);
        }
        Self { algorithm, hex }
    }

    /// Parses the `"<alg>:<hex>"` tagged form.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] on unknown algorithm,
    /// missing tag, or malformed hex.
    pub fn parse(text: &str) -> Result<Self, ProviderError> {
        let err = || {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("invalid content digest: {text:?}"),
            )
        };
        let (alg, hex) = text.split_once(':').ok_or_else(err)?;
        let algorithm = match alg {
            "sha256" => DigestAlgorithm::Sha256,
            _ => return Err(err()),
        };
        let expected = 64;
        if hex.len() != expected || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(err());
        }
        Ok(Self {
            algorithm,
            hex: hex.to_ascii_lowercase(),
        })
    }

    /// The digest algorithm.
    #[must_use]
    pub const fn algorithm(&self) -> DigestAlgorithm {
        self.algorithm
    }

    /// The lowercase hex payload.
    #[must_use]
    pub fn hex(&self) -> &str {
        &self.hex
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algorithm.tag(), self.hex)
    }
}

impl Serialize for ContentDigest {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ContentDigest {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// How a strong validator is enforced at fetch time.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreconditionKind {
    /// Fetch only when the current validator equals the pinned one.
    IfMatch,
    /// Fetch only when the object is unmodified since the pin instant.
    IfUnmodifiedSince,
}

/// A strong conditional pin token, enforced at fetch time.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum StrongRevisionToken {
    /// A strong content digest observed at pin time and re-verified
    /// byte-for-byte at fetch time (the local-file scheme).
    ContentHash {
        /// The strong digest evidence.
        digest: ContentDigest,
    },
    /// A server-side strong validator (e.g. strong `ETag`) enforced via
    /// the named fetch precondition.
    StrongValidator {
        /// The opaque validator value (never a weak tag).
        validator: String,
        /// The precondition applied when fetching.
        precondition: PreconditionKind,
    },
}

/// Evidence that fixes one fetchable object revision.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "proof", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PinProof {
    /// The revision names an immutable object: a Git commit/tree, a
    /// content-addressed blob, an SVN revision. Fetching this revision
    /// later always addresses the same object.
    ImmutableObject {
        /// The immutable revision reference.
        revision: RevisionKey,
    },
    /// A strong conditional token enforced at fetch time: the fetch
    /// either returns the pinned bytes or fails.
    ConditionalStrong {
        /// The strong token.
        token: StrongRevisionToken,
    },
}

/// A provider asset reference fixed to one fetchable object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedAssetRef {
    /// The asset that was pinned.
    pub asset: RemoteAssetRef,
    /// The pin evidence.
    pub proof: PinProof,
    /// The provider-side object identity this pin addresses.
    pub object_identity: ObjectIdentity,
    /// Observed byte length at pin time — a hint used as a fetch
    /// precondition, not a trusted verification result.
    pub expected_size: Option<u64>,
    /// Optional strong digest hint observed at pin time
    /// (`ChecksumHint` capability). When present, `open_pinned`
    /// verifies fetched bytes against it.
    pub digest_hint: Option<ContentDigest>,
}
