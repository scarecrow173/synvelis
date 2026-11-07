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

use crate::document::{
    ProductMeta, SV_THEME_SCHEMA, ThemeDocument, UpstreamProfile, parse_document,
};
use crate::error::{ProblemKind, ThemeError, TokenProblem};

/// Validate the product envelope of one parsed document, plus the
/// coherence of the two extension envelopes. Returns all problems
/// found (empty = valid). Token contents are not inspected.
///
/// The coherence check is not upstream re-validation — upstream
/// internals (`profile`, unknown layer values, malformed `modes`)
/// stay upstream-rejected at resolve time. What *is* checked is the
/// Synvelis-owned invariant that the two envelopes in one file agree:
/// the upstream `layer` must equal the product `layer`, and an
/// upstream `modes` list must cover the SV-declared `mode`. Without
/// this the upstream resolver would silently merge the document at
/// a different layer or reject a mode the catalog believed
/// selectable.
#[must_use]
pub fn validate_document(
    meta: &ProductMeta,
    upstream: Option<&UpstreamProfile>,
) -> Vec<TokenProblem> {
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
    if meta.theme.trim().is_empty() {
        problems.push(TokenProblem {
            path: format!("{base}.theme"),
            kind: ProblemKind::MissingMeta,
            detail: "theme must not be empty".into(),
        });
    }
    for (field, value) in [("mode", &meta.mode), ("density", &meta.density)] {
        if let Some(v) = value
            && v.trim().is_empty()
        {
            problems.push(TokenProblem {
                path: format!("{base}.{field}"),
                kind: ProblemKind::MissingMeta,
                detail: "must not be empty when present".into(),
            });
        }
    }
    if meta.mode.is_some() && meta.density.is_some() {
        problems.push(TokenProblem {
            path: format!("{base}.density"),
            kind: ProblemKind::Contract,
            detail: "mode and density cannot both be declared (such a \
                     document is unreachable: the catalog selects \
                     mode and density layers separately)"
                .into(),
        });
    }
    if meta.layer != "product" {
        problems.push(TokenProblem {
            path: format!("{base}.layer"),
            kind: ProblemKind::MissingMeta,
            detail: format!("expected layer \"product\", got {:?}", meta.layer),
        });
    }

    // Dual-envelope coherence — only when the upstream block exists.
    if let Some(up) = upstream {
        let upbase = "$extensions.dev.ruviform.dtcg-profile";
        if let Some(ulayer) = &up.layer
            && *ulayer != meta.layer
        {
            problems.push(TokenProblem {
                path: format!("{upbase}.layer"),
                kind: ProblemKind::Contract,
                detail: format!(
                    "upstream layer {ulayer:?} disagrees with \
                     productTheme layer {:?} — the upstream resolver \
                     would merge this document at a different layer",
                    meta.layer
                ),
            });
        }
        if let Some(modes) = &up.modes
            && !modes.is_empty()
        {
            match &meta.mode {
                Some(m) if !modes.contains(m) => problems.push(TokenProblem {
                    path: format!("{upbase}.modes"),
                    kind: ProblemKind::Contract,
                    detail: format!(
                        "upstream modes {modes:?} do not cover \
                         productTheme mode {m:?} — resolve would fail \
                         with MissingMode for the declared mode"
                    ),
                }),
                None => problems.push(TokenProblem {
                    path: format!("{upbase}.modes"),
                    kind: ProblemKind::Contract,
                    detail: format!(
                        "upstream modes {modes:?} declared on a \
                         mode-free base document — resolves for \
                         unlisted modes would fail with MissingMode"
                    ),
                }),
                _ => {}
            }
        }
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
    let (meta, upstream, tree) = parse_document(json)?;
    let problems = validate_document(&meta, upstream.as_ref());
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
/// [`ThemeError::Io`] when the directory or a document file cannot be
/// read. A document that fails parsing or envelope validation
/// propagates its own [`ThemeError::Schema`]/[`ThemeError::Validation`]
/// error (with the offending file named in the message/detail) so
/// structured problems are not lost.
pub fn load_dir(dir: &Path) -> Result<Vec<ThemeDocument>, ThemeError> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| ThemeError::Io(format!("{}: {e}", dir.display())))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ThemeError::Io(format!("{}: {e}", dir.display())))?
        .into_iter()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let json = std::fs::read_to_string(&p)
                .map_err(|e| ThemeError::Io(format!("{}: {e}", p.display())))?;
            load_document(&json).map_err(|e| match e {
                ThemeError::Validation(problems) => ThemeError::Validation(
                    problems
                        .into_iter()
                        .map(|mut prob| {
                            prob.detail = format!("in {}: {}", p.display(), prob.detail);
                            prob
                        })
                        .collect(),
                ),
                ThemeError::Schema(detail) => {
                    ThemeError::Schema(format!("{}: {detail}", p.display()))
                }
                other => ThemeError::Io(format!("{}: {other}", p.display())),
            })
        })
        .collect()
}
