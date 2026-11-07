//! Theme catalog, selection, and persisted preferences.
//!
//! The catalog indexes validated product documents by
//! `(theme, mode, density)`. Documents are layered: a base document
//! (no mode/density), one per mode, one per density.
//! [`ThemeCatalog::documents_for`] collects the layers a
//! [`ThemeSelection`] needs — the document *set* the adapter
//! ([`crate::adapter::resolve_documents`]) feeds into the upstream
//! `ruviform-ui-theme` resolver. Mode application and merge
//! semantics belong upstream; this crate only selects documents,
//! never merges values.

use std::collections::BTreeMap;

use ruviform_ui_theme::ResolvedTheme;
use serde::{Deserialize, Serialize};

use crate::document::{ProductMeta, ThemeDocument};
use crate::error::ThemeError;

/// Preferences schema version (independent of project/workspace
/// schemas — theme state must not be silently repurposed).
pub const PREFS_SCHEMA: u32 = 1;

/// Key identifying one concrete theme document variant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ThemeKey {
    /// Theme family (e.g. `"synvelis"`).
    pub theme: String,
    /// Mode (e.g. `"dark"`, `"light"`); `None` = mode-independent.
    #[serde(default)]
    pub mode: Option<String>,
    /// Density (e.g. `"compact"`); `None` = density-independent.
    #[serde(default)]
    pub density: Option<String>,
}

impl ThemeKey {
    /// Build a key from document metadata.
    #[must_use]
    pub fn of(meta: &ProductMeta) -> Self {
        Self {
            theme: meta.theme.clone(),
            mode: meta.mode.clone(),
            density: meta.density.clone(),
        }
    }
}

/// A user's theme choice: `(theme, mode, density)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeSelection {
    /// Theme family.
    pub theme: String,
    /// Color mode.
    pub mode: String,
    /// Density.
    pub density: String,
}

impl ThemeSelection {
    /// Deterministic style-hint string (`"theme/mode/density"`) for
    /// repaint-only consumers such as `VizStyleInput.theme`.
    #[must_use]
    pub fn style_hint(&self) -> String {
        format!("{}/{}/{}", self.theme, self.mode, self.density)
    }
}

/// Persisted theme preferences — versioned independently of the
/// workspace snapshot schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePreferences {
    /// Schema version; must equal [`PREFS_SCHEMA`].
    pub schema: u32,
    /// The selected theme.
    pub selection: ThemeSelection,
}

impl ThemePreferences {
    /// Serialize to JSON.
    ///
    /// # Panics
    /// Never in practice: `ThemePreferences` is always serializable.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("ThemePreferences is serializable")
    }

    /// Parse from JSON; fails on schema mismatch (no silent reuse).
    ///
    /// # Errors
    /// [`ThemeError::Schema`] on malformed JSON or unsupported schema.
    pub fn from_json(json: &str) -> Result<Self, ThemeError> {
        let prefs: Self = serde_json::from_str(json)
            .map_err(|e| ThemeError::Schema(format!("preferences parse: {e}")))?;
        if prefs.schema != PREFS_SCHEMA {
            return Err(ThemeError::Schema(format!(
                "unsupported preferences schema {} (expected {PREFS_SCHEMA})",
                prefs.schema
            )));
        }
        Ok(prefs)
    }
}

/// Catalog of validated product theme documents, indexed by
/// [`ThemeKey`]. Each entry keeps its [`ThemeDocument`] so a
/// selection can be expanded into the document set the upstream
/// resolver consumes.
#[derive(Debug, Default)]
pub struct ThemeCatalog {
    themes: BTreeMap<ThemeKey, ThemeDocument>,
}

impl ThemeCatalog {
    /// Empty catalog.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a validated document. Returns the replaced entry when a
    /// document with the same key was already present (duplicate
    /// `(theme, mode, density)` declarations are suspicious — callers
    /// should treat a `Some` return as a load-order bug).
    pub fn insert(&mut self, document: ThemeDocument) -> Option<ThemeDocument> {
        self.themes.insert(ThemeKey::of(document.meta()), document)
    }

    /// The document layers a selection needs, in deterministic
    /// application order: base (mode/density-free) → mode → density.
    /// Missing layers are simply absent — merge semantics belong to
    /// the upstream resolver, not the catalog.
    #[must_use]
    pub fn documents_for(&self, sel: &ThemeSelection) -> Vec<&ThemeDocument> {
        let keys = [
            ThemeKey {
                theme: sel.theme.clone(),
                mode: None,
                density: None,
            },
            ThemeKey {
                theme: sel.theme.clone(),
                mode: Some(sel.mode.clone()),
                density: None,
            },
            ThemeKey {
                theme: sel.theme.clone(),
                mode: None,
                density: Some(sel.density.clone()),
            },
        ];
        keys.iter().filter_map(|k| self.themes.get(k)).collect()
    }

    /// Resolve a selection through the upstream adapter
    /// ([`crate::adapter::resolve_documents`]) — shorthand for
    /// `resolve_documents(&self.documents_for(sel), &sel.mode)`.
    ///
    /// # Errors
    /// [`ThemeError::Resolve`] on upstream parse/merge failure —
    /// including `EmptyDocuments` when the selection has no layers.
    pub fn resolve(&self, sel: &ThemeSelection) -> Result<ResolvedTheme, ThemeError> {
        crate::adapter::resolve_documents(&self.documents_for(sel), &sel.mode)
    }

    /// Whether the catalog has every layer a selection needs (base +
    /// mode + density).
    #[must_use]
    pub fn is_complete(&self, sel: &ThemeSelection) -> bool {
        self.documents_for(sel).len() == 3
    }

    /// All known keys (deterministic order).
    pub fn keys(&self) -> impl Iterator<Item = &ThemeKey> {
        self.themes.keys()
    }
}
