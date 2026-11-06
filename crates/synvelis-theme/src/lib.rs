//! `synvelis-theme` — Synvelis-owned DTCG product-theme documents,
//! structural validation, catalog/preferences, and the explicit
//! `ResolvedTheme` consumption seam.
//!
//! This crate is driver-neutral and headless: it has no GUI, toolkit,
//! or OS dependencies (`serde/serde_json` only). It deliberately does
//! **not** implement a token interpreter — alias resolution, mode
//! merge, override precedence, and `ResolvedTheme` construction are
//! owned by ruviform #385. See
//! `docs/designs/sv177-product-theme-workspace-preset-design.md`.

pub mod catalog;
pub mod document;
pub mod error;
pub mod resolve;
pub mod validate;

pub use catalog::{PREFS_SCHEMA, ThemeCatalog, ThemeKey, ThemePreferences, ThemeSelection};
pub use document::{ProductMeta, SV_THEME_SCHEMA, TokenTree, parse_document};
pub use error::{ProblemKind, ThemeError, TokenProblem};
pub use resolve::{ResolvedThemeSink, UnresolvedThemeResolver};
pub use validate::{load_dir, load_document, validate_document};
