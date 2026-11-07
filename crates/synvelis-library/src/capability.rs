//! Typed provider capabilities.
//!
//! Every provider operation is gated behind a declared capability.
//! A provider never answers "empty result" for a capability it does
//! not have: it declares `Unsupported` with a typed reason and the
//! operation returns [`ProviderErrorKind::UnsupportedCapability`].

use serde::{Deserialize, Serialize};

/// Why a capability is unavailable on this provider (or this asset).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UnsupportedReason {
    /// The provider backend cannot do this at all.
    NotSupportedByBackend,
    /// The capability could exist but is not implemented yet.
    NotImplemented,
    /// The capability needs an index that does not exist (e.g. a text
    /// search over a store with no searchable index).
    RequiresIndex,
    /// The capability needs a privilege level the principal lacks.
    RequiresPrivilege,
    /// Any other reason — details in `detail`.
    Other {
        /// Free-form detail.
        detail: String,
    },
}

/// Whether one operation is supported.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Capability {
    /// The operation works.
    Supported,
    /// The operation is unavailable for the typed reason.
    Unsupported {
        /// Why it is unavailable.
        reason: UnsupportedReason,
    },
}

impl Capability {
    /// `true` when the operation can be used.
    #[must_use]
    pub const fn is_supported(&self) -> bool {
        matches!(self, Self::Supported)
    }
}

/// The strongest pin a provider can produce.
///
/// Ordered weakest→strongest in documentation, not semantics: a
/// provider declares exactly what it can evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PinCapability {
    /// Pinning is unavailable.
    Unsupported {
        /// Why pinning is unavailable.
        reason: UnsupportedReason,
    },
    /// Can produce [`crate::PinProof::ConditionalStrong`] pins — a
    /// strong token enforced at fetch time (local files, `WebDAV` with
    /// strong validators).
    ConditionalStrong,
    /// Can produce [`crate::PinProof::ImmutableObject`] pins — the
    /// revision itself names an immutable object (CAS blob, Git
    /// commit/tree, SVN revision).
    ImmutableObject,
    /// Can produce either, depending on the asset.
    ConditionalOrImmutable,
}

impl PinCapability {
    /// `true` when `pin` can produce some proof.
    #[must_use]
    pub const fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported { .. })
    }
}

/// Whether the provider mutates anything through this API.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccessMode {
    /// Browsing and fetching only — nothing writes.
    ReadOnly,
    /// The provider may create or mutate provider-side objects.
    ReadWrite,
}

/// The full capability declaration of a provider.
///
/// `capabilities(None)` answers for the provider as a whole;
/// `capabilities(Some(asset))` may narrow it per asset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCapabilities {
    /// List a scope with paging.
    pub browse: Capability,
    /// Observe one asset's facts.
    pub stat: Capability,
    /// Optional query search — `Unsupported` is normal.
    pub search: Capability,
    /// Strongest pin evidence the provider can produce.
    pub pin: PinCapability,
    /// Byte-range reads.
    pub range_read: Capability,
    /// Bounded preview reads (unverified by definition).
    pub preview: Capability,
    /// Full-content fetch via `open_pinned`.
    pub fetch: Capability,
    /// Strong digest hints carried on pins/receipts.
    pub checksum_hint: Capability,
    /// Read/write access mode.
    pub access: AccessMode,
}
