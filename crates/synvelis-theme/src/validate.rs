//! Envelope-level validation of product-theme documents.
//!
//! This crate validates **product metadata only**: the
//! `$extensions["synvelis.productTheme"]` envelope schema and the
//! product-profile rules (non-empty theme/mode/density, `layer` tag).
//! Generic DTCG token semantics — `$type` sets, value shapes, alias
//! syntax/resolution, mode merge — are owned by ruviform #385
//! (`ruviform-ui-theme` / `rvf-dtcg-profile.v1`) and are deliberately
//! not re-validated here. The token tree is kept opaque so the
//! upstream `TokenDocument` parser can consume the same bytes later.

use std::path::Path;

use crate::document::{ProductMeta, SV_THEME_SCHEMA, ThemeDocument, parse_document};
use crate::error::{ProblemKind, ThemeError, TokenProblem};

/// Validate the product envelope of one parsed document. Returns all
/// problems found (empty = valid). Token contents are not inspected.
#[must_use]
pub fn validate_document(meta: &ProductMeta) -> Vec<TokenProblem> {
    let mut problems = Vec::new();
    let base = "$extensions.synvelis.productTheme";

    if meta.schema != SV_THEME_SCHEMA {
        problems.push(TokenProblem {
            path: format!("{base}.schema"),
            kind: ProblemKind::Schema,
            detail: format!(
                "unsupported schema {} (expected {SV_THEME_SCHEMA})",
                meta.schema
            ),
        });
    }
    if meta.theme.is_empty() {
        problems.push(TokenProblem {
            path: format!("{base}.theme"),
            kind: ProblemKind::MissingMeta,
            detail: "theme must not be empty".into(),
        });
    }
    for (field, value) in [("mode", &meta.mode), ("density", &meta.density)] {
        if let Some(v) = value
            && v.is_empty()
        {
            problems.push(TokenProblem {
                path: format!("{base}.{field}"),
                kind: ProblemKind::MissingMeta,
                detail: "must not be empty when present".into(),
            });
        }
    }
    if meta.layer != "product" {
        problems.push(TokenProblem {
            path: format!("{base}.layer"),
            kind: ProblemKind::MissingMeta,
            detail: format!("expected layer \"product\", got {:?}", meta.layer),
        });
    }

    problems
}

/// Parse + envelope-validate a document from a JSON string.
/// Fail-closed: returns `ThemeError::Validation` when any envelope
/// problem exists. Token-level semantics are **not** checked — they
/// belong to the upstream `rvf-dtcg-profile.v1` validator. The
/// returned [`ThemeDocument`] retains the original JSON so the
/// upstream typed parser can consume it at resolve time.
///
/// # Errors
/// [`ThemeError::Schema`] on invalid JSON root;
/// [`ThemeError::Validation`] on any envelope problem.
pub fn load_document(json: &str) -> Result<ThemeDocument, ThemeError> {
    let (meta, tree) = parse_document(json)?;
    let problems = validate_document(&meta);
    if problems.is_empty() {
        Ok(ThemeDocument::new(meta, tree, json.to_owned()))
    } else {
        Err(ThemeError::Validation(problems))
    }
}

/// Load and envelope-validate every `*.json` document in a directory,
/// in filename order.
///
/// # Errors
/// [`ThemeError::Io`] when the directory or a document cannot be
/// read or fails validation (the error names the offending path).
pub fn load_dir(dir: &Path) -> Result<Vec<ThemeDocument>, ThemeError> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| ThemeError::Io(format!("{}: {e}", dir.display())))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let json = std::fs::read_to_string(&p)
                .map_err(|e| ThemeError::Io(format!("{}: {e}", p.display())))?;
            load_document(&json).map_err(|e| ThemeError::Io(format!("{}: {e}", p.display())))
        })
        .collect()
}
