//! `synvelis-theme` — Synvelis-owned DTCG product-theme documents,
//! product-envelope validation, catalog/preferences, and the adapter
//! to the upstream `ruviform-ui-theme` resolver.
//!
//! This crate is driver-neutral and headless: it has no GUI, toolkit,
//! or OS dependencies (`serde`/`serde_json` plus the dependency-free
//! `ruviform-ui-theme` interpreter). It deliberately does **not**
//! implement a token interpreter or generic DTCG validation —
//! `$type`/value/alias semantics, mode merge, override precedence, and
//! `ResolvedTheme` construction are owned by ruviform #385
//! (`ruviform-ui-theme`, `rvf-dtcg-profile.v1`) and reached through
//! [`adapter::resolve_documents`] (or [`ThemeCatalog::resolve`]).

pub mod adapter;
pub mod catalog;
pub mod document;
pub mod error;
pub mod validate;

pub use adapter::resolve_documents;
pub use catalog::{PREFS_SCHEMA, ThemeCatalog, ThemeKey, ThemePreferences, ThemeSelection};
pub use document::{ProductMeta, SV_THEME_SCHEMA, ThemeDocument, TokenTree, parse_document};
pub use error::{ProblemKind, ThemeError, TokenProblem};
// Re-export the upstream contract surface the adapter exposes so
// consumers do not need their own `ruviform-ui-theme` dependency.
pub use ruviform_ui_theme::{
    DocumentLayer, ResolveError, ResolvedTheme, ResolvedValue, TokenDocument, TokenError,
};
pub use validate::{load_dir, load_document, validate_document};
