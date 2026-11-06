//! Structural validation of DTCG product-theme documents.
//!
//! This crate validates **structure only**: envelope schema, known
//! `$type`, value shape, alias syntax, and same-document alias
//! existence + cycle detection. It never resolves aliases, merges
//! modes, converts colors/units, or produces a `ResolvedTheme` — that
//! is ruviform #385's responsibility.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Map, Value};

use crate::document::{ProductMeta, SV_THEME_SCHEMA, TokenTree, parse_document};
use crate::error::{ProblemKind, ThemeError, TokenProblem};

/// Accepted `$type` values in the initial Synvelis profile — a
/// DTCG 2025.10 subset. Unknown types fail closed.
const KNOWN_TYPES: &[&str] = &[
    "color",
    "dimension",
    "fontFamily",
    "fontWeight",
    "duration",
    "cubicBezier",
    "number",
];

/// Validate one parsed document. Returns all problems found (empty =
/// valid).
///
/// # Panics
/// Never in practice: `TokenTree::tokens` only yields objects that
/// contain `$value`.
#[must_use]
pub fn validate_document(meta: &ProductMeta, tree: &TokenTree) -> Vec<TokenProblem> {
    let mut problems = Vec::new();

    if meta.schema != SV_THEME_SCHEMA {
        problems.push(TokenProblem {
            path: "$extensions.synvelis.productTheme.schema".into(),
            kind: ProblemKind::Schema,
            detail: format!(
                "unsupported schema {} (expected {SV_THEME_SCHEMA})",
                meta.schema
            ),
        });
    }
    for (field, value) in [("mode", &meta.mode), ("density", &meta.density)] {
        if let Some(v) = value
            && v.is_empty()
        {
            problems.push(TokenProblem {
                path: format!("$extensions.synvelis.productTheme.{field}"),
                kind: ProblemKind::MissingMeta,
                detail: "must not be empty when present".into(),
            });
        }
    }
    if meta.layer != "product" {
        problems.push(TokenProblem {
            path: "$extensions.synvelis.productTheme.layer".into(),
            kind: ProblemKind::MissingMeta,
            detail: format!("expected layer \"product\", got {:?}", meta.layer),
        });
    }

    // Resolved $type per token: token's own $type wins, otherwise the
    // nearest ancestor group's $type (DTCG group inheritance).
    let type_map: BTreeMap<String, String> = tree.type_map().into_iter().collect();
    let tokens = tree.tokens();
    let token_paths: BTreeSet<String> = tokens.iter().map(|(p, _)| p.clone()).collect();

    // Same-document alias graph for existence + cycle checks.
    let mut alias_graph: BTreeMap<String, String> = BTreeMap::new();

    for (path, token) in &tokens {
        let ty = resolve_type(path, token, &type_map);
        match &ty {
            None => problems.push(TokenProblem {
                path: path.clone(),
                kind: ProblemKind::UnknownType,
                detail: "token has no $type and no ancestor group declares one".into(),
            }),
            Some(t) if !KNOWN_TYPES.contains(&t.as_str()) => problems.push(TokenProblem {
                path: path.clone(),
                kind: ProblemKind::UnknownType,
                detail: format!("unknown $type {t:?}"),
            }),
            Some(_) => {}
        }

        let value = token.get("$value").expect("token has $value");

        // Alias references: a string value shaped exactly like
        // `{a.b.c}` is an alias — syntax checked here; cycle
        // detection for same-document targets (existence is not
        // checkable: unresolved targets may legitimately live in
        // another document/layer — resolution is the interpreter's
        // job). A string *starting* with `{` that is not a
        // well-formed alias is a malformed reference; braces
        // elsewhere are literal text.
        if let Some(s) = value.as_str() {
            if let Some(target) = alias_target(s) {
                alias_graph.insert(path.clone(), target);
            } else if s.starts_with('{') {
                problems.push(TokenProblem {
                    path: path.clone(),
                    kind: ProblemKind::BadRef,
                    detail: format!("malformed alias reference in value {s:?}"),
                });
            }
        }

        if let Some(t) = &ty
            && KNOWN_TYPES.contains(&t.as_str())
            && !value_is_valid(t, value)
        {
            problems.push(TokenProblem {
                path: path.clone(),
                kind: ProblemKind::BadValue,
                detail: format!("value {} is not valid for $type {t:?}", truncate(value)),
            });
        }
    }

    // Same-document alias existence + cycle detection. A target that
    // is not a token in this document is a cross-document reference —
    // the interpreter (ruviform #385) owns its resolution, so we do
    // not flag it here.
    for (path, target) in &alias_graph {
        if token_paths.contains(target) {
            // same-document ref — check for cycles through same-doc refs
            if same_doc_cycle(path, target, &alias_graph, &token_paths) {
                problems.push(TokenProblem {
                    path: path.clone(),
                    kind: ProblemKind::CyclicRef,
                    detail: format!("alias cycle via {target:?}"),
                });
            }
        }
        // else: cross-document ref — syntax-only, resolution delegated.
    }

    problems
}

/// Parse + validate a document from a JSON string. Fail-closed:
/// returns `ThemeError::Validation` when any problem exists.
///
/// # Errors
/// [`ThemeError::Schema`] on invalid JSON root;
/// [`ThemeError::Validation`] on any structural problem.
pub fn load_document(json: &str) -> Result<(ProductMeta, TokenTree), ThemeError> {
    let (meta, tree) = parse_document(json)?;
    let problems = validate_document(&meta, &tree);
    if problems.is_empty() {
        Ok((meta, tree))
    } else {
        Err(ThemeError::Validation(problems))
    }
}

/// Load and validate every `*.json` document in a directory, in
/// filename order.
///
/// # Errors
/// [`ThemeError::Io`] when the directory or a document cannot be
/// read or fails validation (the error names the offending path).
pub fn load_dir(dir: &Path) -> Result<Vec<(ProductMeta, TokenTree)>, ThemeError> {
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

fn resolve_type(
    path: &str,
    token: &Map<String, Value>,
    type_map: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some(Value::String(t)) = token.get("$type") {
        return Some(t.clone());
    }
    // Walk ancestors from nearest to root.
    let mut p = path.to_string();
    while let Some(idx) = p.rfind('.') {
        p.truncate(idx);
        if let Some(t) = type_map.get(&p) {
            return Some(t.clone());
        }
    }
    type_map.get("").cloned()
}

/// Returns the target path when `s` is a well-formed single-brace
/// alias (`{a.b.c}` — non-empty target, no nested braces).
fn alias_target(s: &str) -> Option<String> {
    let inner = s.strip_prefix('{').and_then(|x| x.strip_suffix('}'))?;
    if inner.is_empty() || inner.contains(['{', '}']) {
        return None;
    }
    Some(inner.to_string())
}

fn same_doc_cycle(
    from: &str,
    first_target: &str,
    graph: &BTreeMap<String, String>,
    same_doc: &BTreeSet<String>,
) -> bool {
    let mut seen: BTreeSet<&str> = [from].into_iter().collect();
    let mut cur = first_target;
    loop {
        if cur == from {
            return true;
        }
        if !seen.insert(cur) || !same_doc.contains(cur) {
            return false;
        }
        match graph.get(cur) {
            Some(next) if same_doc.contains(next) => cur = next,
            _ => return false,
        }
    }
}

fn value_is_valid(ty: &str, value: &Value) -> bool {
    // Alias strings are always shape-valid; resolution is the
    // interpreter's job.
    if value.as_str().is_some_and(|s| alias_target(s).is_some()) {
        return true;
    }
    match ty {
        "color" => valid_color(value),
        "dimension" => valid_dimension(value),
        "fontFamily" => {
            value.is_string()
                || value
                    .as_array()
                    .is_some_and(|a| a.iter().all(serde_json::Value::is_string))
        }
        "fontWeight" => {
            value.as_f64().is_some_and(|n| (1.0..=1000.0).contains(&n)) || value.is_string()
        }
        "duration" => {
            valid_unit_string(value, &["ms", "s"]) || valid_value_unit(value, &["ms", "s"])
        }
        "cubicBezier" => value
            .as_array()
            .is_some_and(|a| a.len() == 4 && a.iter().all(serde_json::Value::is_number)),
        "number" => value.is_number(),
        _ => true,
    }
}

fn valid_color(v: &Value) -> bool {
    if let Some(s) = v.as_str() {
        let h = s.strip_prefix('#');
        return h.is_some_and(|h| {
            matches!(h.len(), 3 | 6 | 8) && h.chars().all(|c| c.is_ascii_hexdigit())
        });
    }
    if let Some(o) = v.as_object() {
        return o.contains_key("colorSpace") && o.contains_key("components");
    }
    false
}

fn valid_dimension(v: &Value) -> bool {
    valid_unit_string(v, &["px", "rem"]) || valid_value_unit(v, &["px", "rem"])
}

fn valid_unit_string(v: &Value, units: &[&str]) -> bool {
    if let Some(s) = v.as_str() {
        return units.iter().any(|u| {
            s.strip_suffix(u)
                .is_some_and(|n| !n.is_empty() && n.parse::<f64>().is_ok())
        });
    }
    false
}

fn valid_value_unit(v: &Value, units: &[&str]) -> bool {
    if let Some(o) = v.as_object() {
        let ok_num = o.get("value").is_some_and(Value::is_number);
        let ok_unit = o
            .get("unit")
            .and_then(Value::as_str)
            .is_some_and(|u| units.contains(&u));
        return ok_num && ok_unit;
    }
    false
}

fn truncate(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 60 {
        format!("{}…", &s[..57])
    } else {
        s
    }
}
