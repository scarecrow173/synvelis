//! Contract tests for `synvelis-theme`: the product-theme envelope is
//! validated fail-closed, token contents stay opaque, the catalog
//! selects layered documents without merging, and the adapter hands
//! the original documents to the real upstream `rvf-dtcg-profile.v1`
//! parser/resolver (`ruviform-ui-theme`).
//!
//! Generic DTCG token semantics (`$type`, value shape, alias,
//! cycles) are owned by ruviform #385 (`ruviform-ui-theme` /
//! `rvf-dtcg-profile.v1`) and are intentionally NOT checked here —
//! the `delegated-*` fixtures pin that delegation boundary down.

use std::path::PathBuf;

use synvelis_theme::{
    DocumentLayer, ProblemKind, ResolveError, ThemeCatalog, ThemeError, ThemePreferences,
    ThemeSelection, TokenDocument, TokenError, load_dir, load_document, resolve_documents,
};

fn fixture(name: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures");
    p.push(name);
    std::fs::read_to_string(p).expect("fixture readable")
}

fn assets_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("assets/themes");
    p
}

fn shipped_catalog() -> ThemeCatalog {
    let docs = load_dir(&assets_dir()).expect("all shipped theme documents valid");
    assert_eq!(docs.len(), 5);
    let mut catalog = ThemeCatalog::new();
    for doc in docs {
        assert!(catalog.insert(doc).is_none(), "no duplicate keys");
    }
    catalog
}

fn selection(mode: &str, density: &str) -> ThemeSelection {
    ThemeSelection {
        theme: "synvelis".into(),
        mode: mode.into(),
        density: density.into(),
    }
}

#[test]
fn shipped_assets_are_valid() {
    let catalog = shipped_catalog();
    // Layered resolution: a (theme, mode, density) selection expands
    // to base + mode + density documents.
    let sel = selection("dark", "comfortable");
    assert!(catalog.is_complete(&sel));
    assert_eq!(catalog.documents_for(&sel).len(), 3);
    // A missing mode drops its layer only.
    let odd = selection("sepia", "compact");
    assert_eq!(catalog.documents_for(&odd).len(), 2);
    assert!(!catalog.is_complete(&odd));
}

#[test]
fn shipped_assets_pass_the_upstream_profile_parser() {
    // Every shipped document must carry both envelopes: the Synvelis
    // product metadata (validated by `load_dir`) and the upstream
    // `dev.ruviform.dtcg-profile` block consumed by
    // `TokenDocument::from_json`.
    for entry in std::fs::read_dir(assets_dir()).expect("assets dir readable") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let json = std::fs::read_to_string(&path).expect("asset readable");
        let doc =
            TokenDocument::from_json(&json).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(doc.layer(), DocumentLayer::Product);
        assert_ne!(doc.id(), "");
    }
}

#[test]
fn shipped_assets_resolve_for_every_mode_density_pair() {
    let catalog = shipped_catalog();
    let mut seen_bg = Vec::new();
    for mode in ["dark", "light"] {
        for density in ["compact", "comfortable"] {
            let sel = selection(mode, density);
            let theme = catalog
                .resolve(&sel)
                .unwrap_or_else(|e| panic!("{mode}/{density}: {e}"));
            assert_eq!(theme.mode(), mode);
            assert_eq!(theme.schema_version(), "rvf-dtcg-profile.v1");
            // base + mode + density product documents.
            assert_eq!(theme.sources().len(), 3);
            assert!(
                theme
                    .sources()
                    .iter()
                    .all(|s| s.layer == DocumentLayer::Product)
            );
            // Every layer contributes tokens to the merged result.
            assert!(theme.color("color.bg").is_some(), "{mode}: color.bg");
            assert!(
                theme.length("control.height").is_some(),
                "{density}: control.height"
            );
            let height = theme.length("control.height").unwrap();
            let expected = if density == "compact" { 22.0 } else { 28.0 };
            assert_eq!(height.value, expected, "{density}: control.height");
            // Base layer survives merge: motion/fast is a typed duration.
            let fast = theme.duration("motion.fast").unwrap();
            assert_eq!(fast.value, 100.0);
            seen_bg.push(theme.color("color.bg").unwrap());
        }
    }
    // Modes must produce different backgrounds (dark vs light);
    // density must not recolor within a mode.
    assert_eq!(seen_bg[0], seen_bg[1], "density must not recolor");
    assert_eq!(seen_bg[2], seen_bg[3], "density must not recolor");
    assert_ne!(seen_bg[0], seen_bg[2], "dark and light differ");
}

#[test]
fn adapter_surfaces_upstream_document_errors() {
    // `delegated-cycle` has a valid Synvelis envelope but no
    // `dev.ruviform.dtcg-profile` block — the consumer loads it,
    // the upstream parser rejects it at resolve time.
    let doc = load_document(&fixture("delegated-cycle.dtcg.json")).unwrap();
    let err = resolve_documents(&[&doc], "dark").unwrap_err();
    match err {
        ThemeError::Resolve(ResolveError::Document(TokenError::MissingProfile)) => {}
        other => panic!("expected Resolve(Document(MissingProfile)), got {other:?}"),
    }
}

#[test]
fn valid_document_with_alias_loads() {
    let doc = load_document(&fixture("valid-alias.dtcg.json")).unwrap();
    assert_eq!(doc.meta().theme, "test");
    assert_eq!(doc.meta().mode.as_deref(), Some("dark"));
    // token count: color.bg, color.fg, color.fg-dim(alias), spacing.md
    assert_eq!(doc.tree().tokens().len(), 4);
    // The original JSON is retained for the upstream parser.
    assert!(doc.source().contains("dev.ruviform.dtcg-profile"));
}

#[test]
fn delegated_token_semantics_load_without_consumer_validation() {
    // These documents carry token-level problems the upstream
    // `rvf-dtcg-profile.v1` validator would reject (unknown `$type`,
    // bad value shape, malformed alias, same-document cycle). The
    // consumer crate does not re-validate generic DTCG semantics, so
    // they load fine here — rejection happens upstream at resolve
    // time, not twice.
    for name in [
        "delegated-type.dtcg.json",
        "delegated-value.dtcg.json",
        "delegated-ref.dtcg.json",
        "delegated-cycle.dtcg.json",
    ] {
        load_document(&fixture(name))
            .unwrap_or_else(|e| panic!("{name} must load at consumer level: {e}"));
    }
}

#[test]
fn schema_mismatch_fails_closed() {
    let err = load_document(&fixture("invalid-schema.dtcg.json")).unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::Schema));
        }
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[test]
fn missing_envelope_fails_closed() {
    let json = r##"{"color": {"bg": {"$type": "color", "$value": "#000"}}}"##;
    let err = load_document(json).unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::MissingMeta));
        }
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[test]
fn envelope_profile_violations_fail_closed() {
    // Empty theme.
    let err = load_document(
        r##"{
          "$extensions": {"synvelis.productTheme": {"schema": 1, "theme": "", "layer": "product"}},
          "color": {"$type": "color", "x": {"$value": "#fff"}}}
        "##,
    )
    .unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::MissingMeta));
        }
        other => panic!("expected Validation, got {other:?}"),
    }
    // Empty mode and wrong layer tag produce two problems.
    let err = load_document(
        r##"{
          "$extensions": {"synvelis.productTheme": {"schema": 1, "theme": "t", "mode": "", "layer": "system"}},
          "color": {"$type": "color", "x": {"$value": "#fff"}}}
        "##,
    )
    .unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert_eq!(ps.len(), 2);
            assert!(ps.iter().all(|p| p.kind == ProblemKind::MissingMeta));
        }
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[test]
fn preferences_roundtrip_and_schema_guard() {
    let prefs = ThemePreferences {
        schema: synvelis_theme::PREFS_SCHEMA,
        selection: ThemeSelection {
            theme: "synvelis".into(),
            mode: "dark".into(),
            density: "comfortable".into(),
        },
    };
    let json = prefs.to_json();
    let back = ThemePreferences::from_json(&json).unwrap();
    assert_eq!(prefs, back);
    assert_eq!(back.selection.style_hint(), "synvelis/dark/comfortable");

    let bad = json.replace("\"schema\": 1", "\"schema\": 99");
    assert!(matches!(
        ThemePreferences::from_json(&bad),
        Err(ThemeError::Schema(_))
    ));
}
