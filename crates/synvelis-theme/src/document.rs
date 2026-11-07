//! DTCG product-theme document parsing: envelope metadata plus the raw
//! token tree.
//!
//! The Synvelis profile keeps tokens in the standard DTCG
//! group/token structure and stores product metadata under
//! `$extensions["synvelis.productTheme"]`. See the crate-level docs and
//! the design doc for the full profile description.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{ProblemKind, ThemeError, TokenProblem};

/// Current product-theme document schema version.
pub const SV_THEME_SCHEMA: u32 = 1;

/// Product metadata read from `$extensions["synvelis.productTheme"]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductMeta {
    /// Document schema version; must equal [`SV_THEME_SCHEMA`].
    pub schema: u32,
    /// Theme family name (e.g. `"synvelis"`).
    pub theme: String,
    /// Optional mode (e.g. `"dark"`, `"light"`). `None` for
    /// mode-independent base documents.
    #[serde(default)]
    pub mode: Option<String>,
    /// Optional density (e.g. `"compact"`, `"comfortable"`). `None` for
    /// density-independent documents.
    #[serde(default)]
    pub density: Option<String>,
    /// Layer tag (`"product"` for product overrides).
    pub layer: String,
}

/// The upstream `dev.ruviform.dtcg-profile` envelope fields needed
/// for the dual-envelope coherence check (layer tag and declared
/// modes). Everything else in the upstream block stays upstream-owned
/// and is not re-validated here — this struct only exists so the two
/// envelopes cannot silently contradict each other.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpstreamProfile {
    /// Upstream `layer` value, when present.
    pub layer: Option<String>,
    /// Upstream `modes` list, when present (an empty list means the
    /// document is mode-agnostic).
    pub modes: Option<Vec<String>>,
}

/// A parsed DTCG token tree: the root JSON object minus the
/// `$extensions` envelope.
///
/// The tree is kept as raw `serde_json` so the upstream
/// `ruviform-ui-theme` `TokenDocument` parser (`rvf-dtcg-profile.v1`)
/// can consume the same structure later without a second parser.
/// Token semantics are opaque to this crate — no `$type` resolution,
/// alias handling, or merging happens here.
#[derive(Debug, Clone)]
pub struct TokenTree {
    root: Map<String, Value>,
}

impl TokenTree {
    /// Iterate over `(path, token)` pairs in deterministic order.
    ///
    /// A "token" is any object containing a `$value` key. `path` is the
    /// dotted JSON path (group names joined with `.`).
    #[must_use]
    pub fn tokens(&self) -> Vec<(String, Map<String, Value>)> {
        let mut out = Vec::new();
        let mut stack: Vec<(String, &Map<String, Value>)> = self
            .root
            .iter()
            .map(|(k, v)| (k.clone(), v))
            .filter(|(k, _)| !k.starts_with('$')) // metadata keys are not groups
            .filter_map(|(k, v)| v.as_object().map(|o| (k, o)))
            .collect();
        while let Some((path, obj)) = stack.pop() {
            if obj.contains_key("$value") {
                out.push((path, obj.clone()));
            } else {
                for (k, v) in obj {
                    if k.starts_with('$') {
                        continue; // metadata keys are not groups
                    }
                    if let Some(child) = v.as_object() {
                        stack.push((format!("{path}.{k}"), child));
                    }
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

/// A loaded product-theme document: validated [`ProductMeta`]
/// envelope, opaque [`TokenTree`], and the original JSON source.
///
/// The original source is retained verbatim — including both the
/// `synvelis.productTheme` and `dev.ruviform.dtcg-profile` extension
/// blocks — because it is the authoritative input the upstream
/// `ruviform-ui-theme` `TokenDocument` parser consumes at resolve
/// time ([`crate::adapter::resolve_documents`]).
#[derive(Debug, Clone)]
pub struct ThemeDocument {
    meta: ProductMeta,
    tree: TokenTree,
    source: String,
}

impl ThemeDocument {
    /// Assemble a document from its parsed parts and original source.
    /// Crate-private: only [`crate::validate::load_document`] may
    /// construct a `ThemeDocument`, so an incoherent
    /// meta/tree/source triple cannot be built outside the load
    /// path.
    #[must_use]
    pub(crate) fn new(meta: ProductMeta, tree: TokenTree, source: String) -> Self {
        Self { meta, tree, source }
    }

    /// Product metadata from the `synvelis.productTheme` envelope.
    #[must_use]
    pub const fn meta(&self) -> &ProductMeta {
        &self.meta
    }

    /// Opaque token tree (metadata `$` keys stripped at the root).
    #[must_use]
    pub const fn tree(&self) -> &TokenTree {
        &self.tree
    }

    /// Original JSON source, retained for the upstream typed
    /// `TokenDocument::from_json` parse.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Parse a JSON document into `(ProductMeta, UpstreamProfile,
/// TokenTree)`. The upstream profile block is captured only for the
/// dual-envelope coherence check — `None` when the document carries
/// no `dev.ruviform.dtcg-profile` block (upstream `TokenDocument`
/// will reject it at resolve time; that is not re-validated here).
///
/// Fails closed: envelope problems produce `ThemeError::Schema` and
/// per-token problems produce `ThemeError::Validation` (via the
/// caller's `validate` step, not here).
///
/// # Errors
/// [`ThemeError::Schema`] when the root is not a JSON object;
/// [`ThemeError::Validation`] when the product-theme envelope is
/// missing or malformed.
pub fn parse_document(
    json: &str,
) -> Result<(ProductMeta, Option<UpstreamProfile>, TokenTree), ThemeError> {
    let root: Value =
        serde_json::from_str(json).map_err(|e| ThemeError::Schema(format!("invalid JSON: {e}")))?;
    let root = root
        .as_object()
        .cloned()
        .ok_or_else(|| ThemeError::Schema("document root must be a JSON object".into()))?;

    let meta = root
        .get("$extensions")
        .and_then(|e| e.get("synvelis.productTheme"))
        .ok_or_else(|| {
            ThemeError::Validation(vec![TokenProblem {
                path: "$extensions".into(),
                kind: ProblemKind::MissingMeta,
                detail: "missing $extensions[\"synvelis.productTheme\"] envelope".into(),
            }])
        })?;
    let meta: ProductMeta = serde_json::from_value(meta.clone()).map_err(|e| {
        ThemeError::Validation(vec![TokenProblem {
            path: "$extensions.synvelis.productTheme".into(),
            kind: ProblemKind::MissingMeta,
            detail: format!("malformed productTheme envelope: {e}"),
        }])
    })?;

    // Capture just the upstream fields needed for the envelope
    // coherence check; malformed upstream internals stay
    // upstream-rejected at resolve time.
    let upstream = root
        .get("$extensions")
        .and_then(|e| e.get("dev.ruviform.dtcg-profile"))
        .map(|p| UpstreamProfile {
            layer: p.get("layer").and_then(Value::as_str).map(str::to_owned),
            modes: p.get("modes").and_then(|m| {
                m.as_array().map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
            }),
        });

    let mut tree = root;
    tree.remove("$extensions");
    Ok((meta, upstream, TokenTree { root: tree }))
}
