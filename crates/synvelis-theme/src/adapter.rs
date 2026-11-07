//! Adapter: catalog-selected product documents → the upstream
//! `rvf-dtcg-profile.v1` resolver (`ruviform-ui-theme`).
//!
//! This is the single point where Synvelis product assets meet the
//! ruviform #385 interpreter contract. The adapter interprets nothing
//! itself: it feeds each document's original JSON (which carries both
//! the `synvelis.productTheme` and `dev.ruviform.dtcg-profile`
//! extension blocks) into `TokenDocument::from_json` — the upstream
//! strict parser — and calls `ThemeResolver::resolve`. All token
//! semantics (`$type`, value shape, alias resolution, layer merge,
//! mode selection, cycle detection) are decided upstream; failures
//! propagate as [`ThemeError::Resolve`].

use ruviform_ui_theme::{ResolveError, ResolvedTheme, ThemeResolver, TokenDocument};

use crate::document::ThemeDocument;
use crate::error::ThemeError;

/// Resolve catalog-selected product documents for `mode` via the
/// upstream `rvf-dtcg-profile.v1` pipeline.
///
/// `documents` must be given in intended product-precedence order
/// ([`ThemeCatalog::documents_for`][crate::catalog::ThemeCatalog::documents_for]
/// order: base → mode → density). All Synvelis product documents
/// share the upstream `product` layer, so the upstream resolver's
/// same-layer caller order (later wins) applies within it; aliases
/// resolve over the merged token set afterwards.
///
/// # Errors
/// [`ThemeError::Resolve`] wrapping an upstream `TokenError`
/// (document parse/validate — `MissingProfile`, `InvalidValue`,
/// `ReservedKey`, …) or `ResolveError` (merge semantics —
/// `MissingMode`, `TypeMismatch`, `UnknownReference`, `AliasCycle`,
/// `DuplicateSource`, `EmptyDocuments`).
pub fn resolve_documents(
    documents: &[&ThemeDocument],
    mode: &str,
) -> Result<ResolvedTheme, ThemeError> {
    let parsed: Vec<TokenDocument> = documents
        .iter()
        .map(|d| TokenDocument::from_json(d.source()))
        .collect::<Result<_, _>>()
        .map_err(ResolveError::from)
        .map_err(ThemeError::Resolve)?;
    ThemeResolver::resolve(&parsed, mode).map_err(ThemeError::Resolve)
}
