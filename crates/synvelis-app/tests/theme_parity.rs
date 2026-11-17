//! synvelis#177 — canonical semantic vocabulary + `ResolvedTheme`→
//! `ViewPalette` parity evidence (gpui-test lane, `SV177
//! product theme canonical semantic vocabulary` slice).
//!
//! `ViewPalette::from_theme`/`fallback` は ruviform-ui-gpui の
//! private module にあり palette 内部値は公開観測面に出ない。
//! 本 lane が証明するのは (a) 出荷 DTCG 文書を `ThemeCatalog` で
//! resolve した `ResolvedTheme` が canonical 語彙
//! （`surface.*`/`text.*`/`accent.*`/`state.*`/`control.*`/`viz.*`/
//! `space.*` — `ViewPalette` の read key 集合）を製品値で保持する
//! こと、(b) その `ResolvedTheme` を `ViewMount.theme` に渡した
//! mount が gpui-test window で settle し observe/drawn plan を
//! 返すこと、(c) 画面 spec の semantic component key fixture が
//! mount 済み component と一致すること、に限定される。
//! palette 内部値の直接照合・pixel 差分は本 lane の証拠範囲外
//! （`DrawnPlan` は色を含まない — theme 有無で plan が同一になる
//! ことを `themed_and_fallback_mounts_draw_identical_plans` で
//! 正直に記録する）。実 OS window/pixel golden は RF#631+SV#114
//! gate — `NotRun`。

#![cfg(feature = "gpui-test")]

mod common;

use std::collections::BTreeSet;

use common::scenario::screen_state;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{AttachEpochAllocator, ComponentKey, ViewHost, ViewSpec};
use ruviform_ui::{WindowId, WindowSpec};
use ruviform_ui_gpui::{DrawnPlan, GpuiGuiTest, ViewMount};
use ruviform_ui_test::{SemanticGuiTest, plugin_dock_view_spec};
use synvelis_app::{
    main_screen_spec, main_view_instance, secondary_screen_spec, secondary_view_instance,
};
use synvelis_theme::{ResolvedTheme, ScaleContext, ThemeCatalog, ThemeSelection, load_document};

// The exact shipped documents `synvelis-native` embeds — the parity
// claim is about *these* assets, not fixtures.
const BASE: &str = include_str!("../../../assets/themes/synvelis-base.dtcg.json");
const DARK: &str = include_str!("../../../assets/themes/synvelis-dark.dtcg.json");
const LIGHT: &str = include_str!("../../../assets/themes/synvelis-light.dtcg.json");
const COMPACT: &str = include_str!("../../../assets/themes/synvelis-density-compact.dtcg.json");
const COMFORTABLE: &str =
    include_str!("../../../assets/themes/synvelis-density-comfortable.dtcg.json");

/// `ViewPalette::from_theme` (ruviform pin `cbcc834`, `render.rs`)
/// が read する canonical key の全集合 — color 15 + length 2。
const PALETTE_COLOR_KEYS: &[&str] = &[
    "surface.bg",
    "surface.raised",
    "surface.border",
    "text.primary",
    "text.dim",
    "accent.primary",
    "state.error",
    "state.disabled",
    "control.track",
    "control.fill",
    "control.thumb",
    "viz.waveform",
    "viz.waveform_rms",
    "viz.baseline",
    "viz.cursor",
];
const PALETTE_LENGTH_KEYS: &[&str] = &["space.sm", "space.md"];

/// Screen design fixture (synvelis#177): semantic component keys each
/// product screen mounts, grouped by region. `screens.rs` の spec が
/// 生成する key を stable fixture として固定し、canonical token
/// 語彙への対応は `docs/designs/issue-177-design-tokens.md` の
/// mapping 表が正本。`toggle.mute`/`toggle.solo` は state 依存の
/// 条件付き mount のため fixture 対象外（evidence に記録）。
const MAIN_SCREEN_FIXTURE: &[&str] = &[
    // chrome
    "screen.root",
    "screen.toolbar",
    "screen.tabs",
    "project.save",
    "project.reload",
    "project.undo",
    "project.redo",
    // tab strip
    "tab.arrangement",
    "tab.transport",
    "tab.mixer",
    "tab.inspector",
    "tab.signal_flow",
    "tab.project",
    // arrangement tab (active=0)
    "tab.arrangement.panel",
    "arrangement.table",
    "viz.waveform.panel",
    "viz.waveform.status",
    "viz.waveform",
    // transport tab
    "tab.transport.panel",
    "transport.status",
    "transport.buttons",
    "transport.play",
    "transport.stop",
    "seek.position",
    // mixer tab
    "tab.mixer.panel",
    "tab.mixer.strip",
    "fader.gain",
    "viz.meter.panel",
    "viz.meter.status",
    "viz.meter",
    // inspector tab
    "tab.inspector.panel",
    "inspector.target",
    "inspector.table",
    "inspector.relations.label",
    "inspector.relations.table",
    // signal flow tab
    "tab.signal_flow.panel",
    "signal_flow.status",
    "signal_flow.nodes",
    "signal_flow.edges",
    "signal_flow.probes",
    // project tab
    "tab.project.panel",
    "ppqn.status",
    "ppqn.preview",
    "ppqn.effects",
    "ppqn.sites",
];

const SECONDARY_SCREEN_FIXTURE: &[&str] = &[
    "screen2.root",
    "transport.status",
    "transport2.buttons",
    "transport2.play",
    "transport2.stop",
    "seek2.position",
];

/// Plugin Dock 外枠（`ruviform_ui_test::plugin_dock_view_spec` の
/// 共有 spec）— plugin GUI 内部 theme は SV#176/RF#630 領域で
/// 対象外。dock frame の semantic key のみ fixture に含める。
const PLUGIN_DOCK_FIXTURE: &[&str] = &["plugin.dock", "plugin.editor"];

fn shipped_catalog() -> ThemeCatalog {
    let mut catalog = ThemeCatalog::new();
    for json in [BASE, DARK, LIGHT, COMPACT, COMFORTABLE] {
        let doc = load_document(json).expect("shipped theme document parses");
        assert!(catalog.insert(doc).is_none(), "no duplicate document keys");
    }
    catalog
}

fn resolve(mode: &str, density: &str) -> ResolvedTheme {
    shipped_catalog()
        .resolve(&ThemeSelection {
            theme: "synvelis".to_string(),
            mode: mode.to_string(),
            density: density.to_string(),
        })
        .expect("shipped theme resolves")
}

/// `spec` を `view` の `ViewHost` に mount し、`theme` を
/// `ViewMount.theme` に渡して GPUI test window として立てる —
/// production の `synvelis-native` composition
/// （`ViewHost` + `create_view_window` + `ViewMount.theme`）と同じ形。
fn mount_with_theme(
    gui: &mut GpuiGuiTest,
    view: ViewInstanceId,
    spec: ViewSpec,
    theme: Option<ResolvedTheme>,
) -> WindowId {
    let mut host = ViewHost::new(view, AttachEpochAllocator::new().mint());
    host.mount(spec).expect("mount");
    host.drain_events();
    let window = gui
        .observer_mut()
        .driver_mut()
        .create_view_window(&WindowSpec::default(), ViewMount { host, theme })
        .expect("create_view_window");
    gui.settle().expect("settle mount");
    window
}

fn mounted_keys(gui: &GpuiGuiTest, window: WindowId) -> BTreeSet<String> {
    gui.observer()
        .observe(window)
        .expect("window hosts a view")
        .components
        .iter()
        .map(|c| c.key.0.clone())
        .collect()
}

/// 4 組合せ（dark/light × compact/comfortable）全てで、出荷文書を
/// resolve した `ResolvedTheme` が `ViewPalette::from_theme` の
/// read key 全集合を製品値で保持し、`theme: Some(..)` mount が
/// settle・observe/drawn plan を返す。fallback literal との非一致・
/// alias 追従の深い契約は `synvelis-theme` の headless contract
/// test が正本 — ここでは app 側の load→resolve→mount 経路を固定する。
#[test]
fn product_theme_reaches_gpui_mount_for_all_mode_density_pairs() {
    let state = screen_state();
    for mode in ["dark", "light"] {
        for density in ["compact", "comfortable"] {
            let theme = resolve(mode, density);
            for key in PALETTE_COLOR_KEYS {
                assert!(
                    theme.color(key).is_some(),
                    "{mode}/{density}: palette color key {key} unresolved"
                );
            }
            for key in PALETTE_LENGTH_KEYS {
                assert!(
                    theme.length(key).is_some(),
                    "{mode}/{density}: palette length key {key} unresolved"
                );
            }

            let mut gui = GpuiGuiTest::new();
            let window = mount_with_theme(
                &mut gui,
                main_view_instance(),
                main_screen_spec(&state),
                Some(theme),
            );
            let obs = gui
                .observer()
                .observe(window)
                .expect("mounted view observable");
            assert!(
                !obs.components.is_empty(),
                "{mode}/{density}: no components"
            );
            let drawn = gui.observer().drawn(window).expect("mounted view drawable");
            assert!(
                !drawn.components.is_empty(),
                "{mode}/{density}: no drawn plans"
            );
        }
    }
}

/// 2scale×2theme 意味論差分: `ScaleContext` は mapper 側変換
/// ヒントのみで token 値を変えない（px unit の本 theme では scale
/// 次元の token 差分は意味論上ゼロ）こと、dark/light で mode 値が
/// 分岐することを `ResolvedTheme` 上で固定する。device px 変換・実
/// DPI は RF#631/SV#114 gate — `NotRun`。
#[test]
fn scale_context_is_mapper_hint_only_and_modes_diverge() {
    let base = resolve("dark", "comfortable");
    for scale_factor in [1.0, 2.0] {
        let scaled = base.with_scale(ScaleContext {
            scale_factor,
            rem_base_px: 16.0,
        });
        for key in PALETTE_COLOR_KEYS {
            assert_eq!(
                base.color(key),
                scaled.color(key),
                "scale {scale_factor} must not change token {key}"
            );
        }
        for key in PALETTE_LENGTH_KEYS {
            assert_eq!(
                base.length(key),
                scaled.length(key),
                "scale {scale_factor} must not change token {key}"
            );
        }
    }
    let dark = resolve("dark", "comfortable");
    let light = resolve("light", "comfortable");
    assert_ne!(
        dark.color("surface.bg"),
        light.color("surface.bg"),
        "dark and light surface.bg must diverge"
    );
}

/// 画面別 design fixture: main/secondary/plugin-dock の semantic
/// component key が mount 済み component に全て存在することを
/// assert する — spec 側の key 変更・欠落を fixture が検出する。
#[test]
fn screen_fixture_keys_match_mounted_components() {
    let state = screen_state();
    let theme = resolve("dark", "comfortable");

    let mut gui = GpuiGuiTest::new();
    let main_window = mount_with_theme(
        &mut gui,
        main_view_instance(),
        main_screen_spec(&state),
        Some(theme.clone()),
    );
    let mounted = mounted_keys(&gui, main_window);
    for key in MAIN_SCREEN_FIXTURE {
        assert!(
            mounted.contains(*key),
            "main screen fixture key {key} not mounted"
        );
    }

    let secondary_window = mount_with_theme(
        &mut gui,
        secondary_view_instance(),
        secondary_screen_spec(&state),
        Some(theme),
    );
    let mounted = mounted_keys(&gui, secondary_window);
    for key in SECONDARY_SCREEN_FIXTURE {
        assert!(
            mounted.contains(*key),
            "secondary screen fixture key {key} not mounted"
        );
    }
}

/// Plugin Dock 外枠 fixture: `plugin_dock_view_spec`（ruviform
/// 共有 spec）の semantic key が mount されることを確認する。
/// plugin GUI 内部の theme 強制は SV#176/RF#630 領域で対象外。
#[test]
fn plugin_dock_frame_fixture_keys_mount() {
    let mut gui = GpuiGuiTest::new();
    let window = mount_with_theme(
        &mut gui,
        ViewInstanceId::new(1).expect("nonzero view id"),
        plugin_dock_view_spec(),
        None,
    );
    let mounted = mounted_keys(&gui, window);
    for key in PLUGIN_DOCK_FIXTURE {
        assert!(
            mounted.contains(*key),
            "plugin dock fixture key {key} not mounted"
        );
    }
}

/// 観測限界の正直な記録: `DrawnPlan` は色を含まないため、
/// `theme: Some(product)` と `theme: None`（fallback palette）の
/// mount は同一の drawn plan を返す。palette 内部値が製品 theme で
/// 変わること自体は本 lane では観測不能 — parity の保証は
/// `ResolvedTheme` 解決値（synvelis-theme contract test）と本 lane
/// の mount 受理の 2 層で構成される。
#[test]
fn themed_and_fallback_mounts_draw_identical_plans() {
    let state = screen_state();
    let theme = resolve("dark", "comfortable");

    let mut themed = GpuiGuiTest::new();
    let themed_window = mount_with_theme(
        &mut themed,
        main_view_instance(),
        main_screen_spec(&state),
        Some(theme),
    );

    let mut fallback = GpuiGuiTest::new();
    let fallback_window = mount_with_theme(
        &mut fallback,
        main_view_instance(),
        main_screen_spec(&state),
        None,
    );

    let themed_plans: Vec<(ComponentKey, DrawnPlan)> = themed
        .observer()
        .drawn(themed_window)
        .expect("themed drawn")
        .components
        .into_iter()
        .map(|c| (c.key, c.plan))
        .collect();
    let fallback_plans: Vec<(ComponentKey, DrawnPlan)> = fallback
        .observer()
        .drawn(fallback_window)
        .expect("fallback drawn")
        .components
        .into_iter()
        .map(|c| (c.key, c.plan))
        .collect();
    assert_eq!(
        themed_plans, fallback_plans,
        "DrawnPlan carries no color — theme and fallback palettes \
         must produce identical plans; palette internals are \
         unobservable in this lane by design"
    );
}
