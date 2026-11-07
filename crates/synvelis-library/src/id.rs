//! Provider-side identity types.
//!
//! `ProviderId` identifies an app's provider *connection configuration*.
//! It is deliberately a different type from the project aggregate's
//! stable IDs (`synvelis_project::AssetId` / `SourceId` / `ImportId`):
//! a provider locator is not a project entity and must not be assigned
//! to one by accident.
//!
//! `OpaqueLocator` is the provider-internal, credential-free locator
//! carried by [`crate::RemoteAssetRef`]. It is opaque to callers:
//! storable, comparable, and serializable, but its internal structure
//! is owned by each provider.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::error::{ProviderError, ProviderErrorKind};

/// Upper bound for locator/principal/credential-key text so a hostile
/// or corrupt provider cannot hand the library unbounded payloads.
pub(crate) const MAX_OPAQUE_LEN: usize = 4096;

/// Identifies a provider connection configuration inside the app.
///
/// Serialized form is `"provider-<32 lowercase hex>"`, matching the
/// tagged-ID convention used by `synvelis-project` stable IDs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderId(u128);

impl ProviderId {
    /// The serialization kind tag.
    pub const KIND: &'static str = "provider";

    /// Wraps a raw `u128` value.
    #[must_use]
    pub const fn new(raw: u128) -> Self {
        Self(raw)
    }

    /// The raw `u128` value.
    #[must_use]
    pub const fn raw(self) -> u128 {
        self.0
    }

    /// The canonical `"provider-<32hex>"` form.
    #[must_use]
    pub fn to_tagged(self) -> String {
        format!("{}-{:032x}", Self::KIND, self.0)
    }

    /// Parses the canonical tagged form, rejecting a mismatched kind.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when the tag or hex payload
    /// is malformed.
    pub fn from_tagged(text: &str) -> Result<Self, ProviderError> {
        let (kind, hex) = text
            .rsplit_once('-')
            .ok_or_else(|| invalid_id("provider id", text))?;
        if kind != Self::KIND || hex.len() != 32 {
            return Err(invalid_id("provider id", text));
        }
        let raw = u128::from_str_radix(hex, 16).map_err(|_| invalid_id("provider id", text))?;
        Ok(Self(raw))
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_tagged())
    }
}

impl Serialize for ProviderId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_tagged())
    }
}

impl<'de> Deserialize<'de> for ProviderId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::from_tagged(&text).map_err(D::Error::custom)
    }
}

fn invalid_id(what: &str, text: &str) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::InvalidRequest,
        format!("invalid {what}: {text:?}"),
    )
}

/// A credential-free, provider-internal locator.
///
/// Opaque to callers: it can be stored, compared, hashed and
/// serialized, but only the owning provider interprets its contents.
/// It never carries credentials or secret material.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpaqueLocator(String);

impl OpaqueLocator {
    /// Wraps a locator string after validation.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when the locator is empty,
    /// contains a NUL byte, or exceeds the opaque-length bound.
    pub fn new(text: impl Into<String>) -> Result<Self, ProviderError> {
        let text = text.into();
        if text.is_empty() {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "opaque locator is empty",
            ));
        }
        if text.len() > MAX_OPAQUE_LEN {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("opaque locator exceeds {MAX_OPAQUE_LEN} bytes"),
            ));
        }
        if text.contains('\0') {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "opaque locator contains NUL",
            ));
        }
        Ok(Self(text))
    }

    /// The raw locator text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the wrapper and returns the inner string.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for OpaqueLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for OpaqueLocator {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for OpaqueLocator {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::new(text).map_err(D::Error::custom)
    }
}

/// Principal scope key for provider access.
///
/// An app-internal scope identifier — not a secret. Browsing cursors
/// are bound to it so a cursor minted under one principal cannot be
/// replayed under another.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct PrincipalId(String);

impl PrincipalId {
    /// Wraps a principal scope key.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when the key is empty or
    /// over the opaque-length bound.
    pub fn new(text: impl Into<String>) -> Result<Self, ProviderError> {
        let text = text.into();
        if text.is_empty() || text.len() > MAX_OPAQUE_LEN {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "principal id is empty or over-long",
            ));
        }
        Ok(Self(text))
    }

    /// The raw scope key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PrincipalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PrincipalId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::new(text).map_err(D::Error::custom)
    }
}

/// A key naming a credential entry in an app-managed credential store.
///
/// The key travels through provider calls; the secret value does not.
/// Providers resolve it through [`crate::CredentialResolver`] at use
/// time. Keys are never persisted into the Project snapshot.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CredentialKey(String);

impl CredentialKey {
    /// Wraps a credential key.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when the key is empty or
    /// over the opaque-length bound.
    pub fn new(text: impl Into<String>) -> Result<Self, ProviderError> {
        let text = text.into();
        if text.is_empty() || text.len() > MAX_OPAQUE_LEN {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "credential key is empty or over-long",
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

impl<'de> Deserialize<'de> for CredentialKey {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::new(text).map_err(D::Error::custom)
    }
}

/// An opaque credential *handle* resolved at use time.
///
/// Carries the key it was resolved for plus provider-defined session
/// state (e.g. a store handle id). It intentionally does not model a
/// secret payload: baseline providers never inspect it, and remote
/// adapters treat it as an opaque authorization context.
#[derive(Clone)]
pub struct OpaqueCredential {
    key: CredentialKey,
    context: String,
}

// The context is provider-defined session state — never printed.
impl fmt::Debug for OpaqueCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpaqueCredential")
            .field("key", &self.key)
            .field("context", &"<redacted>")
            .finish()
    }
}

impl OpaqueCredential {
    /// Wraps a resolved handle for `key`.
    #[must_use]
    pub fn new(key: CredentialKey, context: impl Into<String>) -> Self {
        Self {
            key,
            context: context.into(),
        }
    }

    /// The key the handle was resolved for.
    #[must_use]
    pub fn key(&self) -> &CredentialKey {
        &self.key
    }

    /// The provider-defined opaque session context.
    #[must_use]
    pub fn context(&self) -> &str {
        &self.context
    }
}
