//! Adoption-seam tests — `adopt_window`/`adopt_view` register
//! driver/composition-root-created windows and already-mounted views
//! without ever invoking host window creation.

use ruviform_ui::dock::{
    DockError, DockTarget, ViewInstanceId, WindowCloseDisposition, WindowGeometry,
};
use ruviform_ui::event::HostEvent;
use ruviform_ui::window::{MonitorInfo, WindowHandle, WindowId};
use synvelis_project::{CounterIdGenerator, Project, ProjectController, ProjectId};
use synvelis_workspace::{
    FakeWindowHost, ProjectSession, ViewType, WorkspaceError, WorkspaceManager,
};

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xA11CE), "adopt-test");
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

fn geom() -> WindowGeometry {
    WindowGeometry {
        x: 10,
        y: 20,
        width: 1280,
        height: 800,
        monitor: None,
    }
}

fn win(raw: u64) -> WindowId {
    WindowId::new(raw).expect("nonzero window id")
}

fn view(raw: u64) -> ViewInstanceId {
    ViewInstanceId::new(raw).expect("nonzero view id")
}

#[test]
fn adopt_window_registers_without_host_creation() {
    let mut ws = fresh_ws();
    let id = win(7);

    ws.adopt_window(id, "driven", geom()).expect("adopt");

    let record = ws
        .windows()
        .iter()
        .find(|w| w.id == id)
        .expect("window record");
    assert_eq!(record.title, "driven");
    assert_eq!(record.geometry.width, 1280);
    assert_eq!(record.geometry.x, 10);
    assert!(!record.veto_close);
    assert_eq!(record.disposition, WindowCloseDisposition::DetachViews);
    assert!(ws.dock().is_registered(id));
    // Adoption is a ledger operation only — the host never created
    // (or even knows about) this window.
    assert_eq!(ws.host.windows().len(), 0);
}

#[test]
fn adopt_window_rejects_duplicate() {
    let mut ws = fresh_ws();
    let id = win(3);
    ws.adopt_window(id, "first", geom()).expect("adopt");

    let err = ws
        .adopt_window(id, "second", geom())
        .expect_err("duplicate adopt must fail");
    assert!(matches!(err, WorkspaceError::DuplicateWindow(w) if w == id));

    // A host-created window occupies the same id space — adopting
    // its id is a duplicate too.
    let created = ws.create_window("host", 640, 480).expect("create");
    let err = ws
        .adopt_window(created, "again", geom())
        .expect_err("adopting a created id must fail");
    assert!(matches!(err, WorkspaceError::DuplicateWindow(w) if w == created));
}

#[test]
fn adopt_view_attaches_to_window_target() {
    let mut ws = fresh_ws();
    let w = win(1);
    ws.adopt_window(w, "main", geom()).expect("adopt window");

    let v = view(4);
    ws.adopt_view(v, ViewType::Arrangement, "arr", Some(DockTarget::Window(w)))
        .expect("adopt view");

    assert!(ws.registry().contains(v));
    let desc = ws.descriptor(v).expect("descriptor");
    assert_eq!(desc.kind, ViewType::Arrangement);
    assert_eq!(desc.title, "arr");
    assert_eq!(ws.dock().owner_of(v), Some(w));
    assert!(!ws.dock().is_detached(v));
    assert!(ws.states().get(v).is_some());
}

#[test]
fn adopt_view_without_target_lands_in_pool() {
    let mut ws = fresh_ws();
    let v = view(2);

    ws.adopt_view(v, ViewType::Mixer, "mix", None)
        .expect("adopt pooled view");

    assert!(ws.registry().contains(v));
    assert!(ws.dock().is_detached(v));
    assert_eq!(ws.dock().owner_of(v), None);
    assert!(ws.states().get(v).is_some());
}

#[test]
fn adopt_view_rejects_duplicates() {
    let mut ws = fresh_ws();
    let w = win(1);
    ws.adopt_window(w, "main", geom()).expect("adopt window");

    let v = view(5);
    ws.adopt_view(v, ViewType::Inspector, "insp", Some(DockTarget::Window(w)))
        .expect("first adopt");

    // Second adopt of the same id — whether to a window or the pool —
    // is a dock-level DuplicateView.
    for target in [Some(DockTarget::Window(w)), None] {
        let err = ws
            .adopt_view(v, ViewType::Inspector, "insp", target)
            .expect_err("duplicate view adopt must fail");
        assert!(matches!(
            err,
            WorkspaceError::Dock(DockError::DuplicateView { view, .. }) if view == v
        ));
    }

    // An id already minted by spawn_view collides the same way.
    let spawned = ws
        .spawn_view(ViewType::Mixer, "mix", DockTarget::Window(w))
        .expect("spawn");
    let err = ws
        .adopt_view(spawned, ViewType::Mixer, "mix", None)
        .expect_err("adopting a spawned id must fail");
    assert!(matches!(
        err,
        WorkspaceError::Dock(DockError::DuplicateView { view, .. }) if view == spawned
    ));
}

#[test]
fn adopt_view_rejects_unadopted_window_target() {
    let mut ws = fresh_ws();
    let err = ws
        .adopt_view(
            view(9),
            ViewType::Arrangement,
            "arr",
            Some(DockTarget::Window(win(99))),
        )
        .expect_err("target window must be adopted first");
    assert!(matches!(
        err,
        WorkspaceError::Dock(DockError::UnknownWindow { .. })
    ));
    // The failed adopt leaves no registry/state residue.
    assert!(!ws.registry().contains(view(9)));
}

#[test]
fn adopt_view_advances_next_view_past_adopted_ids() {
    let mut ws = fresh_ws();
    ws.adopt_view(view(41), ViewType::Arrangement, "arr", None)
        .expect("adopt");

    let spawned = ws
        .spawn_detached_view(ViewType::Mixer, "mix")
        .expect("spawn");
    assert!(spawned.get() > 41, "spawned id {spawned} must pass 41");
}

#[test]
fn adopted_window_destroyed_detaches_views_to_pool() {
    let mut ws = fresh_ws();
    let w = win(2);
    ws.adopt_window(w, "temp", geom()).expect("adopt window");
    let v = view(6);
    ws.adopt_view(v, ViewType::SignalFlow, "flow", Some(DockTarget::Window(w)))
        .expect("adopt view");

    ws.on_window_event(w, &HostEvent::Destroyed)
        .expect("destroyed");

    // Default DetachViews: the view survives in the pool, registry
    // and state intact; the window record is gone.
    assert!(ws.dock().is_detached(v));
    assert!(ws.registry().contains(v));
    assert!(ws.states().get(v).is_some());
    assert!(ws.windows().iter().all(|r| r.id != w));
    assert!(!ws.dock().is_registered(w));
}
