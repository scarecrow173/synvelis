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
fn resolve_rejects_modes_and_densities_outside_the_catalog_vocabulary() {
    // The theme declares modes {dark, light} and densities
    // {compact, comfortable}. Asking for anything else must fail
    // loudly — silently resolving the remaining layers would return
    // a ResolvedTheme labelled with a mode no document provides.
    let catalog = shipped_catalog();
    for bad in [
        selection("sepia", "compact"),
        selection("dark", "huge"),
        selection("sepia", "huge"),
    ] {
        match catalog.resolve(&bad) {
            Err(ThemeError::Selection(msg)) => {
                assert!(msg.contains(&bad.theme), "{msg}");
            }
            other => panic!("expected Selection error for {bad:?}, got {other:?}"),
        }
    }
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
    // A document with a valid Synvelis envelope but no
    // `dev.ruviform.dtcg-profile` block loads at the consumer level —
    // the upstream parser rejects it at resolve time (MissingProfile
    // short-circuits before any token check).
    let doc = load_document(
        r##"{
          "$extensions": {"synvelis.productTheme": {"schema": 1, "theme": "t", "layer": "product"}},
          "color": {"$type": "color", "x": {"$value": "#ffffff"}}
        }"##,
    )
    .unwrap();
    let err = resolve_documents(&[&doc], "dark").unwrap_err();
    match err {
        ThemeError::Resolve(ResolveError::Document(TokenError::MissingProfile)) => {}
        other => panic!("expected Resolve(Document(MissingProfile)), got {other:?}"),
    }
}

#[test]
fn delegated_fixtures_surface_their_named_upstream_rejections() {
    // Each `delegated-*` fixture carries both envelopes plus one
    // token-level problem, so the upstream pipeline reaches — and
    // names — the specific violation instead of stopping at
    // MissingProfile.
    let cases: &[(&str, &str)] = &[
        ("delegated-type.dtcg.json", "UnknownTokenType"),
        ("delegated-value.dtcg.json", "InvalidValue"),
        ("delegated-ref.dtcg.json", "InvalidValue"),
    ];
    for (name, variant) in cases {
        let doc = load_document(&fixture(name)).unwrap();
        let err = resolve_documents(&[&doc], "dark").unwrap_err();
        match err {
            ThemeError::Resolve(ResolveError::Document(e)) => {
                assert!(
                    format!("{e:?}").contains(variant),
                    "{name}: expected Document({variant}), got {e:?}"
                );
            }
            other => panic!("{name}: expected Resolve(Document({variant})), got {other:?}"),
        }
    }
    // Alias cycles are checked on the merged token set (upstream
    // resolve step, not document parse).
    let doc = load_document(&fixture("delegated-cycle.dtcg.json")).unwrap();
    match resolve_documents(&[&doc], "dark").unwrap_err() {
        ThemeError::Resolve(ResolveError::AliasCycle { .. }) => {}
        other => panic!("expected Resolve(AliasCycle), got {other:?}"),
    }
}

#[test]
fn overridden_cycle_members_make_the_merged_set_legal() {
    // P1 regression: the pre-delegation validator rejected
    // `delegated-cycle` up front, which wrongly rejected sets that
    // become legal once a later layer replaces the cyclic tokens.
    // Upstream checks cycles on the merged set only, so a cycle in a
    // lower document overridden by a later document is legal.
    let cyclic = load_document(&fixture("delegated-cycle.dtcg.json")).unwrap();
    let override_doc = load_document(
        r##"{
          "$extensions": {
            "dev.ruviform.dtcg-profile": {
              "profile": "rvf-dtcg-profile.v1",
              "layer": "product",
              "id": "test-override"
            },
            "synvelis.productTheme": {
              "schema": 1,
              "theme": "test",
              "mode": "dark",
              "layer": "product"
            }
          },
          "color": {
            "$type": "color",
            "a": { "$value": "#112233" },
            "b": { "$value": "#445566" }
          }
        }"##,
    )
    .unwrap();
    let theme = resolve_documents(&[&cyclic, &override_doc], "dark")
        .expect("overridden cycle members must resolve");
    assert_eq!(theme.mode(), "dark");
    assert!(theme.color("color.a").is_some());
}

#[test]
fn valid_document_with_alias_loads_and_resolves() {
    let doc = load_document(&fixture("valid-alias.dtcg.json")).unwrap();
    assert_eq!(doc.meta().theme, "test");
    assert_eq!(doc.meta().mode.as_deref(), Some("dark"));
    // token count: color.bg, color.fg, color.fg-dim(alias), spacing.md
    assert_eq!(doc.tree().tokens().len(), 4);
    // The original JSON is retained for the upstream parser.
    assert!(doc.source().contains("dev.ruviform.dtcg-profile"));
    // Real upstream alias resolution: {color.fg} resolves to the
    // literal #ffffff token, so fg-dim == fg in the merged theme.
    let theme = resolve_documents(&[&doc], "dark").expect("alias doc resolves");
    assert_eq!(theme.color("color.fg-dim"), theme.color("color.fg"));
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
fn dual_envelope_contradictions_fail_closed() {
    // The two extension envelopes must agree — the checks below are
    // coherence, not upstream re-validation.
    // (a) Upstream layer demotes the document: upstream resolver
    // would merge it at `semantic` while SV believes `product`.
    let err = load_document(
        r##"{
          "$extensions": {
            "synvelis.productTheme": {"schema": 1, "theme": "t", "layer": "product"},
            "dev.ruviform.dtcg-profile": {"profile": "rvf-dtcg-profile.v1", "layer": "semantic", "id": "x"}
          },
          "color": {"$type": "color", "x": {"$value": "#ffffff"}}
        }"##,
    )
    .unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::Contract));
        }
        other => panic!("expected Validation(Contract), got {other:?}"),
    }
    // (b) Upstream `modes` excludes the SV-declared mode.
    let err = load_document(
        r##"{
          "$extensions": {
            "synvelis.productTheme": {"schema": 1, "theme": "t", "mode": "light", "layer": "product"},
            "dev.ruviform.dtcg-profile": {"profile": "rvf-dtcg-profile.v1", "layer": "product", "id": "x", "modes": ["dark"]}
          },
          "color": {"$type": "color", "x": {"$value": "#ffffff"}}
        }"##,
    )
    .unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::Contract));
        }
        other => panic!("expected Validation(Contract), got {other:?}"),
    }
    // (c) Upstream `modes` on a mode-free SV base document.
    let err = load_document(
        r##"{
          "$extensions": {
            "synvelis.productTheme": {"schema": 1, "theme": "t", "layer": "product"},
            "dev.ruviform.dtcg-profile": {"profile": "rvf-dtcg-profile.v1", "layer": "product", "id": "x", "modes": ["dark"]}
          },
          "color": {"$type": "color", "x": {"$value": "#ffffff"}}
        }"##,
    )
    .unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::Contract));
        }
        other => panic!("expected Validation(Contract), got {other:?}"),
    }
    // (d) A document declaring both mode and density is unreachable
    // through the layered catalog selection.
    let err = load_document(
        r##"{
          "$extensions": {"synvelis.productTheme": {"schema": 1, "theme": "t", "mode": "dark", "density": "compact", "layer": "product"}},
          "color": {"$type": "color", "x": {"$value": "#ffffff"}}
        }"##,
    )
    .unwrap_err();
    match err {
        ThemeError::Validation(ps) => {
            assert!(ps.iter().any(|p| p.kind == ProblemKind::Contract));
        }
        other => panic!("expected Validation(Contract), got {other:?}"),
    }
    // A coherent pair still loads (shipped assets pin this in
    // `shipped_assets_are_valid` / `..._resolve_for_every_*`).
    let doc = load_document(&fixture("valid-alias.dtcg.json")).unwrap();
    assert_eq!(doc.meta().mode.as_deref(), Some("dark"));
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
