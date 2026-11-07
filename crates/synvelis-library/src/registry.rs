//! Provider registry: `ProviderId` → descriptor + provider instance.
//!
//! The registry is how the app resolves a [`RemoteAssetRef`]'s
//! provider into a live [`AssetProvider`]. IDs are unique; providers
//! are registered explicitly by the app at connection-configuration
//! time.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::ProviderId;
use crate::provider::AssetProvider;

/// The provider family a connection belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ProviderKind {
    /// A local filesystem folder.
    LocalFolder,
    /// The current project's asset catalog.
    ProjectAssets,
    /// A Git remote (future SV88+).
    GitRemote,
    /// A `WebDAV` / file server (future).
    WebDav,
    /// SVN (future).
    Svn,
    /// Perforce (future).
    Perforce,
    /// SMB / NAS (future).
    Smb,
    /// S3-compatible object store (future).
    S3,
    /// Anything else.
    Other {
        /// Provider-defined kind label.
        label: String,
    },
}

/// Registration metadata for one provider connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDescriptor {
    /// Connection identity.
    pub id: ProviderId,
    /// Provider family.
    pub kind: ProviderKind,
    /// Human-readable connection name.
    pub display_name: String,
}

/// The app's provider registry.
#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<ProviderId, (ProviderDescriptor, Arc<dyn AssetProvider>)>,
}

impl ProviderRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a provider under its descriptor id.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when the id is already
    /// registered, or when the provider's `id()` disagrees with the
    /// descriptor.
    pub fn register(
        &mut self,
        descriptor: ProviderDescriptor,
        provider: Arc<dyn AssetProvider>,
    ) -> Result<(), ProviderError> {
        if provider.id() != descriptor.id {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!(
                    "provider id {} does not match descriptor {}",
                    provider.id(),
                    descriptor.id
                ),
            ));
        }
        if provider.kind() != descriptor.kind {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!(
                    "provider kind {:?} does not match descriptor {:?}",
                    provider.kind(),
                    descriptor.kind
                ),
            ));
        }
        if self.providers.contains_key(&descriptor.id) {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("provider {} already registered", descriptor.id),
            ));
        }
        self.providers.insert(descriptor.id, (descriptor, provider));
        Ok(())
    }

    /// The provider registered for `id`.
    #[must_use]
    pub fn get(&self, id: &ProviderId) -> Option<Arc<dyn AssetProvider>> {
        self.providers.get(id).map(|(_, p)| Arc::clone(p))
    }

    /// The descriptor registered for `id`.
    #[must_use]
    pub fn descriptor(&self, id: &ProviderId) -> Option<&ProviderDescriptor> {
        self.providers.get(id).map(|(d, _)| d)
    }

    /// All registered descriptors in id order.
    #[must_use]
    pub fn list(&self) -> Vec<ProviderDescriptor> {
        self.providers.values().map(|(d, _)| d.clone()).collect()
    }
}
