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
    ColorSpace, DocumentLayer, LengthUnit, ProblemKind, ResolveError, ResolvedColor, StatusTone,
    ThemeCatalog, ThemeError, ThemePreferences, ThemeSelection, TokenDocument, TokenError,
    load_dir, load_document, resolve_documents,
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
fn shipped_assets_resolve_every_status_tone_for_every_mode_density_pair() {
    // AUTX-07 / SV266-NR-03: every `StatusTone` token path must resolve
    // as `color.{path}` through the shipped product documents for all
    // four mode×density selections. `StatusTone::ALL` is the vocabulary
    // authority — the scan follows future tone additions without a
    // hardcoded count.
    let catalog = shipped_catalog();
    let mut mode_ramps: Vec<Vec<ResolvedColor>> = Vec::new();
    for mode in ["dark", "light"] {
        let mut per_density: [Vec<ResolvedColor>; 2] = [Vec::new(), Vec::new()];
        for (d_idx, density) in ["compact", "comfortable"].iter().enumerate() {
            let theme = catalog
                .resolve(&selection(mode, density))
                .unwrap_or_else(|e| panic!("{mode}/{density}: {e}"));
            for tone in StatusTone::ALL {
                let path = format!("color.{}", tone.token_path());
                let color = theme
                    .color(&path)
                    .unwrap_or_else(|| panic!("{mode}/{density}: {path} unresolved"));
                per_density[d_idx].push(color);
            }
        }
        // Density documents carry no `color` group — the resolved
        // status ramp must be identical across densities within a mode.
        assert_eq!(
            per_density[0], per_density[1],
            "{mode}: density must not recolor status tokens"
        );
        mode_ramps.push(per_density[0].clone());
    }
    // The mode documents own mode-adapted status ramps: the resolved
    // value sets must differ between dark and light. Per-token equality
    // stays legal for genuinely mode-invariant values — the contract
    // is that the ramp as a whole is mode-owned, not that every tone
    // differs.
    assert_ne!(
        mode_ramps[0], mode_ramps[1],
        "dark and light status ramps differ"
    );
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

/// `#rrggbb` → the [`ResolvedColor`] the upstream parser produces
/// (sRGB components normalized to `0.0..=1.0`, alpha `1.0`).
fn srgb(hex: &str) -> ResolvedColor {
    let h = hex.strip_prefix('#').expect("hex color");
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).expect("hex digit");
    ResolvedColor {
        space: ColorSpace::Srgb,
        components: [
            f64::from(byte(0)) / 255.0,
            f64::from(byte(2)) / 255.0,
            f64::from(byte(4)) / 255.0,
        ],
        alpha: 1.0,
    }
}

/// Canonical color keys `ViewPalette::from_theme` reads, mapped to the
/// shipped product value each mode must resolve (design doc
/// `docs/designs/issue-177-design-tokens.md` mapping table).
const CANONICAL_COLORS: &[(&str, &str, &str)] = &[
    // (canonical key, dark value, light value)
    ("surface.bg", "#1b1d21", "#f5f6f8"),
    ("surface.raised", "#23262c", "#e8eaee"),
    ("surface.border", "#33373f", "#e8eaee"),
    ("text.primary", "#e8e8ea", "#1b1d21"),
    ("text.dim", "#9aa0a8", "#5a6068"),
    ("accent.primary", "#6aa9e8", "#2a6bc2"),
    ("state.error", "#e86a6a", "#c23a3a"),
    ("state.disabled", "#4e565f", "#9aa0a8"),
    ("control.track", "#191b1f", "#eaecf0"),
    ("control.fill", "#6aa9e8", "#2a6bc2"),
    ("control.thumb", "#e8e8ea", "#1b1d21"),
    ("viz.waveform", "#6aa9e8", "#2a6bc2"),
    ("viz.waveform_rms", "#8fb8e8", "#7aa8d8"),
    ("viz.baseline", "#33373f", "#e8eaee"),
    ("viz.cursor", "#e8e8ea", "#1b1d21"),
];

/// `ViewPalette::fallback` literals (ruviform pin `cbcc834`,
/// `render.rs` `color_or` defaults) as `#rrggbb`. Kept here so the
/// contract proves the resolved theme carries *product* values, not
/// palette fallbacks; an upstream vocabulary change surfaces as a
/// test update, not a silent drift.
const FALLBACK_COLORS: &[(&str, &str)] = &[
    ("surface.bg", "#14181d"),
    ("surface.raised", "#1d232a"),
    ("surface.border", "#2e3640"),
    ("text.primary", "#d7dde4"),
    ("text.dim", "#8a949f"),
    ("accent.primary", "#3d7be0"),
    ("state.error", "#e05555"),
    ("state.disabled", "#5a636d"),
    ("control.track", "#262d36"),
    ("control.fill", "#3d7be0"),
    ("control.thumb", "#e8edf2"),
    ("viz.waveform", "#4a90d9"),
    ("viz.waveform_rms", "#2b5f9e"),
    ("viz.baseline", "#39424d"),
    ("viz.cursor", "#e8edf2"),
];

/// Canonical → primitive alias pairs: the resolved values must be
/// identical, proving the whole-value alias (not a copied literal)
/// carries the product value.
const CANONICAL_ALIASES: &[(&str, &str)] = &[
    ("surface.bg", "color.bg"),
    ("surface.raised", "color.bg-panel"),
    ("surface.border", "color.border"),
    ("text.primary", "color.fg"),
    ("text.dim", "color.fg-dim"),
    ("accent.primary", "color.accent"),
    ("state.error", "color.err"),
    ("control.track", "color.bg-input"),
    ("control.fill", "color.accent"),
    ("control.thumb", "color.fg"),
    ("viz.waveform", "color.accent"),
    ("viz.baseline", "color.border"),
    ("viz.cursor", "color.fg"),
];

/// Canonical dimension aliases → `spacing.*` primitives, with the
/// expected px value per density.
const CANONICAL_SPACE: &[(&str, &str, f64, f64)] = &[
    // (canonical key, primitive key, compact px, comfortable px)
    ("space.sm", "spacing.sm", 4.0, 8.0),
    ("space.md", "spacing.md", 6.0, 12.0),
    ("space.page", "spacing.lg", 10.0, 16.0),
];

#[test]
fn shipped_canonical_color_vocabulary_resolves_product_values() {
    // synvelis#177: every canonical color key `ViewPalette::from_theme`
    // reads must resolve to the shipped product value for all four
    // mode×density selections — never absent, never the palette
    // fallback literal.
    let catalog = shipped_catalog();
    for mode in ["dark", "light"] {
        let mode_idx = usize::from(mode == "light");
        for density in ["compact", "comfortable"] {
            let theme = catalog
                .resolve(&selection(mode, density))
                .unwrap_or_else(|e| panic!("{mode}/{density}: {e}"));
            for &(key, dark_hex, light_hex) in CANONICAL_COLORS {
                let expected = srgb(if mode_idx == 0 { dark_hex } else { light_hex });
                let resolved = theme
                    .color(key)
                    .unwrap_or_else(|| panic!("{mode}/{density}: {key} unresolved"));
                assert_eq!(
                    resolved, expected,
                    "{mode}/{density}: {key} must resolve to the product value"
                );
            }
        }
    }
}

#[test]
fn shipped_canonical_colors_differ_from_palette_fallback_literals() {
    // The parity contract's core: resolved canonical colors must not
    // coincide with `ViewPalette::fallback` literals — that equality
    // would mean the product document contributes nothing and the
    // palette silently falls back.
    let catalog = shipped_catalog();
    let fallback: std::collections::BTreeMap<&str, ResolvedColor> = FALLBACK_COLORS
        .iter()
        .map(|&(k, hex)| (k, srgb(hex)))
        .collect();
    for mode in ["dark", "light"] {
        for density in ["compact", "comfortable"] {
            let theme = catalog
                .resolve(&selection(mode, density))
                .unwrap_or_else(|e| panic!("{mode}/{density}: {e}"));
            for &(key, _, _) in CANONICAL_COLORS {
                let resolved = theme.color(key).expect("canonical color resolves");
                assert_ne!(
                    resolved, fallback[key],
                    "{mode}/{density}: {key} equals the palette fallback literal"
                );
            }
        }
    }
}

#[test]
fn shipped_canonical_aliases_track_their_primitive_values() {
    // Whole-value aliases must carry the primitive's resolved value —
    // if a product edit changes `color.bg`, `surface.bg` must follow.
    let catalog = shipped_catalog();
    for mode in ["dark", "light"] {
        for density in ["compact", "comfortable"] {
            let theme = catalog
                .resolve(&selection(mode, density))
                .unwrap_or_else(|e| panic!("{mode}/{density}: {e}"));
            for &(canonical, primitive) in CANONICAL_ALIASES {
                assert_eq!(
                    theme.color(canonical),
                    theme.color(primitive),
                    "{mode}/{density}: {canonical} must alias {primitive}"
                );
            }
            for &(canonical, primitive, compact_px, comfortable_px) in CANONICAL_SPACE {
                let expected = if density == "compact" {
                    compact_px
                } else {
                    comfortable_px
                };
                let length = theme
                    .length(canonical)
                    .unwrap_or_else(|| panic!("{mode}/{density}: {canonical} unresolved"));
                assert_eq!(length.unit, LengthUnit::Px, "{canonical} unit");
                assert_eq!(
                    length.value, expected,
                    "{mode}/{density}: {canonical} must alias {primitive}"
                );
                assert_eq!(
                    theme.length(canonical),
                    theme.length(primitive),
                    "{mode}/{density}: {canonical} must alias {primitive}"
                );
            }
        }
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
