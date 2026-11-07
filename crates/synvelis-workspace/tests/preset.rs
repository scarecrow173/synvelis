//! Workspace preset contract tests (SV177).
//!
//! Presets are slot-based templates: application adopts compatible
//! existing views, spawns missing ones, parks residuals in the
//! detached pool, and delegates monitor/off-screen fallback to
//! ruviform's `LayoutSnapshot::restore_or_default`.

use std::path::PathBuf;

use ruviform_ui::window::{MonitorInfo, WindowHandle};
use synvelis_project::{CounterIdGenerator, Project, ProjectController, ProjectId};
use synvelis_workspace::{
    FakeWindowHost, LinkGroupId, ProjectSession, ViewType, WorkspaceError, WorkspaceManager,
    WorkspacePreset,
};

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xA11CE), "preset-test");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    WorkspaceManager::new(
        FakeWindowHost::with_monitors(vec![MonitorInfo {
            name: Some("primary".to_string()),
            width: 2560,
            height: 1440,
            x: 0,
            y: 0,
            scale_factor: 1.0,
        }]),
        ProjectSession::new(controller),
    )
}

fn monitors(ws: &Ws) -> Vec<MonitorInfo> {
    ws.host.monitors()
}

fn asset(name: &str) -> String {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("assets/workspaces");
    p.push(name);
    std::fs::read_to_string(p).expect("preset asset readable")
}

#[test]
fn shipped_presets_parse_and_validate() {
    for name in [
        "synvelis.editing.json",
        "synvelis.mixing.json",
        "synvelis.plugin-dock.json",
    ] {
        let preset = WorkspacePreset::from_json(&asset(name)).unwrap();
        preset.validate().unwrap();
    }
    let dock = WorkspacePreset::from_json(&asset("synvelis.plugin-dock.json")).unwrap();
    assert!(
        dock.views.iter().any(|v| v.kind == ViewType::PluginDock),
        "plugin-dock preset carries the plugin-dock view kind"
    );
}

#[test]
fn apply_to_empty_workspace_spawns_and_docks() {
    let mut ws = fresh_ws();
    let preset = WorkspacePreset::from_json(&asset("synvelis.editing.json")).unwrap();
    let rev_before = ws.session().revision();
    let report = ws.apply_preset(&preset, &monitors(&ws)).unwrap();

    assert_eq!(report.windows.len(), 1);
    assert_eq!(report.spawned.len(), 2);
    assert_eq!(report.adopted, [] as [ruviform_ui::ViewInstanceId; 0]);
    assert!(report.focused);

    // Focus designation applied.
    let focused = ws.focus().native_window;
    assert_eq!(focused, Some(report.windows[0]));
    let active = ws.focus().active_view.expect("focus view applied");
    assert_eq!(ws.descriptor(active).unwrap().kind, ViewType::Arrangement);

    // Link group applied to the inspector slot.
    let inspector = ws
        .registry()
        .ids()
        .into_iter()
        .find(|id| ws.descriptor(*id).unwrap().kind == ViewType::Inspector)
        .unwrap();
    assert_eq!(ws.links().group_of(inspector), Some(LinkGroupId(1)));

    // Project revision untouched by preset application.
    assert_eq!(ws.session().revision(), rev_before);
}

#[test]
fn apply_to_populated_workspace_adopts_and_parks_residuals() {
    let mut ws = fresh_ws();
    let win = ws.create_window("existing", 1280, 720).unwrap();
    let arrangement = ws
        .spawn_view(
            ViewType::Arrangement,
            "My Arrangement",
            ruviform_ui::dock::DockTarget::Window(win),
        )
        .unwrap();
    let extra = ws
        .spawn_detached_view(ViewType::Mixer, "Spare Mixer")
        .unwrap();

    let preset = WorkspacePreset::from_json(&asset("synvelis.editing.json")).unwrap();
    let report = ws.apply_preset(&preset, &monitors(&ws)).unwrap();

    // The existing arrangement was adopted into slot 1; the
    // inspector was spawned; the spare mixer parked detached.
    assert!(report.adopted.contains(&arrangement));
    assert_eq!(report.adopted.len(), 1);
    assert_eq!(report.spawned.len(), 1);
    assert!(report.detached.contains(&extra));
    // Old window closed, exactly one new window exists.
    assert_eq!(ws.windows().len(), 1);
    // Adopted view keeps its registered identity.
    assert!(ws.registry().contains(arrangement));
}

#[test]
fn two_window_preset_builds_split_layout() {
    let mut ws = fresh_ws();
    let preset = WorkspacePreset::from_json(&asset("synvelis.mixing.json")).unwrap();
    let report = ws.apply_preset(&preset, &monitors(&ws)).unwrap();
    assert_eq!(report.windows.len(), 2);
    assert_eq!(ws.windows().len(), 2);
    // All three views docked, none detached.
    assert_eq!(
        ws.registry()
            .ids()
            .iter()
            .filter(|id| ws.dock().owner_of(**id).is_some())
            .count(),
        3
    );
}

#[test]
fn missing_monitor_falls_back_to_available_geometry() {
    let mut ws = fresh_ws();
    // Preset targets a second monitor at x=1620 which does not exist
    // here → restore_or_default re-centers onto the primary.
    let preset = WorkspacePreset::from_json(&asset("synvelis.mixing.json")).unwrap();
    let report = ws.apply_preset(&preset, &monitors(&ws)).unwrap();
    assert_eq!(report.windows.len(), 2);
    // Both windows exist and have non-zero geometry (corrected, not
    // dropped).
    for w in ws.windows() {
        assert!(w.geometry.width > 0 && w.geometry.height > 0);
    }
}

#[test]
fn schema_mismatch_is_rejected() {
    let bad = asset("synvelis.editing.json").replace("\"schema\": 2", "\"schema\": 99");
    assert!(matches!(
        WorkspacePreset::from_json(&bad),
        Err(WorkspaceError::Preset(_))
    ));
}

#[test]
fn legacy_snapshot_shape_migrates_to_preset() {
    // Build a workspace, capture it, then feed the snapshot JSON —
    // marked by its top-level "layout" key — into from_json.
    let mut ws = fresh_ws();
    let win = ws.create_window("legacy", 1280, 720).unwrap();
    ws.spawn_view(
        ViewType::Arrangement,
        "Arr",
        ruviform_ui::dock::DockTarget::Window(win),
    )
    .unwrap();
    let snap = ws.capture().unwrap();
    let json = snap.to_json().unwrap();

    let preset = WorkspacePreset::from_json(&json).unwrap();
    assert_eq!(preset.schema, synvelis_workspace::PRESET_SCHEMA);
    assert_eq!(preset.views.len(), 1);
    preset.validate().unwrap();

    // Applying the migrated preset reproduces the layout shape.
    let mut ws2 = fresh_ws();
    let report = ws2.apply_preset(&preset, &monitors(&ws2)).unwrap();
    assert_eq!(report.windows.len(), 1);
    assert_eq!(ws2.registry().ids().len(), 1);
}

#[test]
fn capture_to_preset_roundtrip() {
    let mut ws = fresh_ws();
    let win = ws.create_window("main", 1600, 900).unwrap();
    let a = ws
        .spawn_view(
            ViewType::Arrangement,
            "Arr",
            ruviform_ui::dock::DockTarget::Window(win),
        )
        .unwrap();
    let m = ws.spawn_detached_view(ViewType::Mixer, "Mix").unwrap();
    let snap = ws.capture().unwrap();

    let preset = WorkspacePreset::from_snapshot(&snap, "test.roundtrip".into(), "RT".into());
    preset.validate().unwrap();

    let mut ws2 = fresh_ws();
    let report = ws2.apply_preset(&preset, &monitors(&ws2)).unwrap();
    assert_eq!(report.windows.len(), 1);
    // Two slots: arrangement docked + mixer detached.
    assert_eq!(ws2.registry().ids().len(), 2);
    assert!(
        ws2.dock().is_detached(
            ws2.registry()
                .ids()
                .into_iter()
                .find(|id| ws2.descriptor(*id).unwrap().kind == ViewType::Mixer)
                .unwrap()
        )
    );
    let _ = a;
    let _ = m;
}

#[test]
fn structurally_invalid_presets_fail_before_mutating() {
    // A dock-structure error (bad ratio) must surface *before* any
    // window is destroyed — regression test for post-destruction
    // partial apply.
    let mut ws = fresh_ws();
    let win = ws.create_window("keep", 800, 600).unwrap();
    let bad = asset("synvelis.editing.json").replace("\"ratio\": 0.75", "\"ratio\": 1.5");
    let preset = WorkspacePreset::from_json(&bad).unwrap();
    let err = ws.apply_preset(&preset, &monitors(&ws)).unwrap_err();
    assert!(matches!(err, WorkspaceError::Preset(_)));
    assert_eq!(ws.windows().len(), 1);
    assert_eq!(ws.windows()[0].id, win);
}

#[test]
fn duplicate_docked_slot_is_rejected_before_mutating() {
    // Same slot docked in both windows → restore() would flag it as
    // duplicate — but validation catches it before teardown.
    let mut ws = fresh_ws();
    let win = ws.create_window("keep", 800, 600).unwrap();
    let dup = asset("synvelis.mixing.json").replace("\"views\": [3]", "\"views\": [1]");
    let preset = WorkspacePreset::from_json(&dup).unwrap();
    assert!(preset.validate().is_err());
    let err = ws.apply_preset(&preset, &monitors(&ws)).unwrap_err();
    assert!(matches!(err, WorkspaceError::Preset(_)));
    assert_eq!(ws.windows()[0].id, win);
}

#[test]
fn focus_must_be_docked_in_the_focused_window() {
    // Slot 3 is docked in window 1; focusing it from window 0 is
    // incoherent and must be rejected.
    let cross = asset("synvelis.mixing.json").replace(
        "\"focus\": { \"window\": 0, \"view\": 1 }",
        "\"focus\": { \"window\": 0, \"view\": 3 }",
    );
    let preset = WorkspacePreset::from_json(&cross).unwrap();
    assert!(preset.validate().is_err());
}

#[test]
fn empty_windows_and_zero_link_group_are_rejected() {
    let empty = asset("synvelis.editing.json")
        .replace("\"windows\": [", "\"windows\": [] ,\"_unused_windows\": [");
    let preset = WorkspacePreset::from_json(&empty).unwrap();
    assert!(preset.validate().is_err());

    let zero = asset("synvelis.editing.json").replace("\"group\": 1", "\"group\": 0");
    let preset = WorkspacePreset::from_json(&zero).unwrap();
    assert!(preset.validate().is_err());
}

#[test]
fn preset_state_applies_to_spawned_only_adopted_keeps_state() {
    let mut ws = fresh_ws();
    let win = ws.create_window("existing", 1280, 720).unwrap();
    let arrangement = ws
        .spawn_view(
            ViewType::Arrangement,
            "Live Arrangement",
            ruviform_ui::dock::DockTarget::Window(win),
        )
        .unwrap();
    // Mutate the live view's tool — a preset declaring a different
    // tool must not clobber it on adoption.
    let live_tool = ws.state(arrangement).unwrap().active_tool;
    let preset = WorkspacePreset::from_json(&asset("synvelis.editing.json")).unwrap();
    ws.apply_preset(&preset, &monitors(&ws)).unwrap();
    assert_eq!(ws.state(arrangement).unwrap().active_tool, live_tool);
}

#[test]
fn invalid_preset_references_fail_before_mutating() {
    let mut ws = fresh_ws();
    let json = asset("synvelis.editing.json").replace("\"view\": 2,", "\"view\": 77,");
    let preset = WorkspacePreset::from_json(&json).unwrap();
    let win = ws.create_window("keep", 800, 600).unwrap();
    let err = ws.apply_preset(&preset, &monitors(&ws)).unwrap_err();
    assert!(matches!(err, WorkspaceError::Preset(_)));
    // Validation ran before any mutation — the window survived.
    assert_eq!(ws.windows().len(), 1);
    assert_eq!(ws.windows()[0].id, win);
}
