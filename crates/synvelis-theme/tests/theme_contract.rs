//! Contract tests for `synvelis-theme`: valid documents load,
//! invalid documents fail closed with the right problem kinds, and
//! the resolution seam stays explicitly `Unsupported` until ruviform
//! #385 lands.

use std::path::PathBuf;

use synvelis_theme::{
    ProblemKind, ResolvedThemeSink, ThemeCatalog, ThemeError, ThemePreferences, ThemeSelection,
    UnresolvedThemeResolver, load_dir, load_document,
};

fn fixture(name: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures");
    p.push(name);
    std::fs::read_to_string(p).expect("fixture readable")
}

#[test]
fn shipped_assets_are_valid() {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("assets/themes");
    let docs = load_dir(&p).expect("all shipped theme documents valid");
    assert_eq!(docs.len(), 5);
    let mut catalog = ThemeCatalog::new();
    for (meta, tree) in docs {
        assert!(catalog.insert(meta, tree).is_none(), "no duplicate keys");
    }
    // Layered resolution: a (theme, mode, density) selection expands
    // to base + mode + density documents.
    let sel = ThemeSelection {
        theme: "synvelis".into(),
        mode: "dark".into(),
        density: "comfortable".into(),
    };
    assert!(catalog.is_complete(&sel));
    assert_eq!(catalog.documents_for(&sel).len(), 3);
    // A missing mode drops its layer only.
    let odd = ThemeSelection {
        theme: "synvelis".into(),
        mode: "sepia".into(),
        density: "compact".into(),
    };
    assert_eq!(catalog.documents_for(&odd).len(), 2);
    assert!(!catalog.is_complete(&odd));
}

#[test]
fn valid_document_with_alias_loads() {
    let (meta, tree) = load_document(&fixture("valid-alias.dtcg.json")).unwrap();
    assert_eq!(meta.theme, "test");
    assert_eq!(meta.mode.as_deref(), Some("dark"));
    // token count: color.bg, color.fg, color.fg-dim(alias), spacing.md
    assert_eq!(tree.tokens().len(), 4);
}

#[test]
fn unknown_type_fails_closed() {
    let err = load_document(&fixture("invalid-type.dtcg.json")).unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::UnknownType));
        }
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[test]
fn bad_value_fails_closed() {
    let err = load_document(&fixture("invalid-value.dtcg.json")).unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::BadValue));
        }
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[test]
fn malformed_ref_fails_closed() {
    let err = load_document(&fixture("invalid-ref.dtcg.json")).unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::BadRef));
        }
        other => panic!("expected Validation, got {other:?}"),
    }
}

#[test]
fn same_document_cycle_fails_closed() {
    let err = load_document(&fixture("invalid-cycle.dtcg.json")).unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::CyclicRef));
        }
        other => panic!("expected Validation, got {other:?}"),
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

#[test]
fn malformed_alias_edge_cases() {
    // Nested braces are a malformed reference, not a cross-doc alias.
    let err = load_document(
        r#"{
          "$extensions": {"synvelis.productTheme": {"schema": 1, "theme": "t", "layer": "product"}},
          "color": {"$type": "color", "x": {"$value": "{a{b}}"}}}
        "#,
    )
    .unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::BadRef));
        }
        other => panic!("expected Validation, got {other:?}"),
    }
    // Literal braces *inside* a string are not a reference at all.
    let (meta, _) = load_document(
        r#"{
          "$extensions": {"synvelis.productTheme": {"schema": 1, "theme": "t", "layer": "product"}},
          "font": {"fam": {"$type": "fontFamily", "$value": "Fira {Code}"}}}
        "#,
    )
    .unwrap();
    assert_eq!(meta.theme, "t");
}

#[test]
fn resolution_seam_is_explicitly_unsupported() {
    let sel = ThemeSelection {
        theme: "synvelis".into(),
        mode: "dark".into(),
        density: "comfortable".into(),
    };
    let err = UnresolvedThemeResolver.resolve(&sel, &[]).unwrap_err();
    assert!(matches!(err, ThemeError::Unsupported(_)));
}
