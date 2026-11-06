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

/// A parsed DTCG token tree: the root JSON object minus the
/// `$extensions` envelope.
///
/// The tree is kept as raw `serde_json` so that the interpreter
/// (ruviform #385) can consume the same structure later without a
/// second parser. Validation only inspects it — it never resolves or
/// merges values.
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

    /// Return the group-inherited `$type` map: `group path -> $type`
    /// for every group that declares one (including tokens themselves).
    #[must_use]
    pub fn type_map(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut stack: Vec<(String, &Map<String, Value>)> = vec![(String::new(), &self.root)];
        while let Some((path, obj)) = stack.pop() {
            if let Some(Value::String(t)) = obj.get("$type") {
                out.push((path.clone(), t.clone()));
            }
            if obj.contains_key("$value") {
                continue;
            }
            for (k, v) in obj {
                if k.starts_with('$') {
                    continue;
                }
                if let Some(child) = v.as_object() {
                    let child_path = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    stack.push((child_path, child));
                }
            }
        }
        out
    }
}

/// Parse a JSON document into `(ProductMeta, TokenTree)`.
///
/// Fails closed: envelope problems produce `ThemeError::Schema` and
/// per-token problems produce `ThemeError::Validation` (via the
/// caller's `validate` step, not here).
///
/// # Errors
/// [`ThemeError::Schema`] when the root is not a JSON object;
/// [`ThemeError::Validation`] when the product-theme envelope is
/// missing or malformed.
pub fn parse_document(json: &str) -> Result<(ProductMeta, TokenTree), ThemeError> {
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

    let mut tree = root;
    tree.remove("$extensions");
    Ok((meta, TokenTree { root: tree }))
}
