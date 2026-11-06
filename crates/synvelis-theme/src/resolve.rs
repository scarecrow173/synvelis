//! Seam for resolving a [`ThemeSelection`] to a resolved theme.
//!
//! The ruviform #385 contract (`ResolvedTheme` / token interpreter)
//! is not yet published, so Synvelis exposes an explicit
//! [`ResolvedThemeSink`] trait and an [`UnresolvedThemeResolver`] that
//! always returns [`ThemeError::Unsupported`]. Consumers must treat
//! `Unsupported` as a real error — not a silent fallback.

use crate::catalog::ThemeSelection;
use crate::document::TokenTree;
use crate::error::ThemeError;

/// Consumption point for a resolved theme. A future adapter will
/// implement this on top of the ruviform #385 `ResolvedTheme`
/// contract; `T` is deliberately generic so this crate stays
/// driver-neutral (no GPUI/toolkit types may appear here).
pub trait ResolvedThemeSink {
    /// The resolved-theme output type (adapter-defined; will be
    /// `ruviform_ui::ResolvedTheme` or equivalent once published).
    type Resolved;

    /// Resolve a selection against a document set. Implementations
    /// must not silently fall back on failure — errors propagate.
    ///
    /// # Errors
    /// [`ThemeError::Unsupported`] until the ruviform #385 contract
    /// is available; adapter-defined errors thereafter.
    fn resolve(
        &self,
        selection: &ThemeSelection,
        documents: &[TokenTree],
    ) -> Result<Self::Resolved, ThemeError>;
}

/// Stub resolver used until the ruviform #385 contract is available.
#[derive(Debug, Default)]
pub struct UnresolvedThemeResolver;

impl ResolvedThemeSink for UnresolvedThemeResolver {
    type Resolved = ();

    fn resolve(
        &self,
        _selection: &ThemeSelection,
        _documents: &[TokenTree],
    ) -> Result<Self::Resolved, ThemeError> {
        Err(ThemeError::Unsupported(
            "ruviform #385 ResolvedTheme contract not yet available".into(),
        ))
    }
}
