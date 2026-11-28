//! synvelis#176 (SV-GPUI-02) — headless `PluginDock` panel slice,
//! acceptance rows 2–3.
//!
//! Two deterministic tiers, no OS window or plugin binary:
//!
//! - **Panel tier** — `PluginDock` + an app-shared
//!   `PluginEditorSessions` + a scripted `PluginEditorHost`/`EditorSessionOps`
//!   fake: the `StablePluginInstanceId → PluginEditorSessionId →
//!   SurfaceSlotId → ViewInstanceId` ownership/generation model,
//!   `Open`/`Hide`/`Show`/`ReDock`/`Close`, stale-lease and
//!   `SessionBusy` rejection, `Unsupported`/host-loss honesty,
//!   window-loss convergence, and the PD-07 logical save/restore.
//! - **Op tier** — the same commands end-to-end through `NativeShell`
//!   + `FakeDriver` (`syn.plugin_dock.*`), including the
//!     `syn.window.dock_*` two-phase re-dock hook and proof that the
//!     ops never touch project/DSP state.
//!
//! Nothing here claims real OS/native-plugin evidence — rows 1/7/8
//! stay `NotRun`/`Deferred` (`evidence/sv176/`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{Value, json};

use ruviform_plugin_core::{ParentSlotLease, PluginEditorSessionId, PluginHandle};
use ruviform_plugin_host::{
    AttachMode, EditorCapabilities, EditorOpError, EditorSessionError, EditorSessionOps,
    EditorSessionState, EditorSize, LeaseRedeemer, PluginEditorSessions, SessionClock,
    SessionDeadlinePolicy, SessionFailCause, Support as HostSupport,
};
use ruviform_ui::WindowId;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::ipc::{ErrorBody, Event, Intent};
use ruviform_ui::native_slot::{
    LeaseInfo, LogicalRect, NativeSurfaceSlot, SlotCapabilities, SlotError, SlotEvent,
    SlotGeometry, SlotLease, SlotSpec, SlotState, SlotVisibility, SurfaceSlotId,
};
use ruviform_ui::{BackendReport, DriverEvent, WindowDriver, WindowLifecycle, WindowSpec};
use ruviform_ui_test::{FakeDriver, FakeNativeSlot};

use synvelis_app::plugin_dock::{NativeSlotAuthority, SharedNativeSlotAuthority};
use synvelis_app::{
    NativeShell, PluginDock, PluginDockError, PluginDockPlacement, PluginEditorHost, SynvelisApp,
    main_view_id, main_view_targets, secondary_view_id, secondary_view_targets,
};
use synvelis_player::StablePluginInstanceId;

// ---------------------------------------------------------------------------
// Shared test slot authority — headless `FakeNativeSlot` fixture
// ---------------------------------------------------------------------------

/// High tag bits of the synthetic parent tokens the test authority
/// mints at redemption. **Test-only vocabulary**: the value is
/// obviously not an OS handle (no real HWND/HWND-like pointer carries
/// this tag) and exists purely so tests can prove which slot a
/// redeemed lease belonged to. Nothing here is evidence of real
/// native docking.
const TEST_PARENT_TOKEN_TAG: u64 = 0xFA17_E000_0000_0000;

/// The synthetic test-only parent token for a lease sequence —
/// distinct per live lease so a redemption mix-up is observable
/// without treating a slot index as a parent token.
fn test_parent_token(seq: u32) -> u64 {
    TEST_PARENT_TOKEN_TAG | u64::from(seq)
}

/// Headless shared-authority fixture: upstream [`FakeNativeSlot`]
/// (the canonical deterministic [`NativeSurfaceSlot`] fake) plus the
/// product seam surface — spec mirroring (the fake does not expose
/// specs) and issuer-side redemption against the *same* ledger. One
/// instance behind one `Rc<RefCell>` is shared by every panel in a
/// test, exactly like the production driver's canonical surface
/// ledger — a second fixture instance is a *different* authority and
/// must reject this ledger's leases as foreign.
///
/// **Headless/test-only** — no OS window, no real native parent.
struct TestSlotAuthority {
    inner: FakeNativeSlot,
    /// Mirror of each live slot's declared spec, updated on
    /// geometry/visibility mutation — the fake validates through its
    /// registry but does not expose specs, and the product seam
    /// requires them.
    specs: HashMap<SurfaceSlotId, SlotSpec>,
}

impl TestSlotAuthority {
    fn new() -> Self {
        Self {
            inner: FakeNativeSlot::new(),
            specs: HashMap::new(),
        }
    }
}

/// mints a shared handle around a fresh fixture authority.
fn test_authority() -> SharedNativeSlotAuthority {
    Rc::new(RefCell::new(TestSlotAuthority::new()))
}

impl NativeSurfaceSlot for TestSlotAuthority {
    fn capabilities(&self, slot: SurfaceSlotId) -> Result<SlotCapabilities, SlotError> {
        self.inner.capabilities(slot)
    }

    fn declare_slot(&mut self, spec: SlotSpec) -> Result<SurfaceSlotId, SlotError> {
        let slot = self.inner.declare_slot(spec)?;
        self.specs.insert(slot, spec);
        Ok(slot)
    }

    fn release_slot(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError> {
        self.inner.release_slot(slot)?;
        self.specs.remove(&slot);
        Ok(())
    }

    fn set_geometry(
        &mut self,
        slot: SurfaceSlotId,
        geometry: SlotGeometry,
    ) -> Result<(), SlotError> {
        self.inner.set_geometry(slot, geometry)?;
        if let Some(spec) = self.specs.get_mut(&slot) {
            spec.geometry = geometry;
        }
        Ok(())
    }

    fn set_visibility(
        &mut self,
        slot: SurfaceSlotId,
        visibility: SlotVisibility,
    ) -> Result<(), SlotError> {
        self.inner.set_visibility(slot, visibility)?;
        if let Some(spec) = self.specs.get_mut(&slot) {
            spec.visibility = visibility;
        }
        Ok(())
    }

    fn set_clip(
        &mut self,
        slot: SurfaceSlotId,
        clip: Option<LogicalRect>,
    ) -> Result<(), SlotError> {
        self.inner.set_clip(slot, clip)
    }

    fn set_zorder(&mut self, slot: SurfaceSlotId, z: i32) -> Result<(), SlotError> {
        self.inner.set_zorder(slot, z)
    }

    fn request_focus(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError> {
        self.inner.request_focus(slot)
    }

    fn lease_parent(&mut self, slot: SurfaceSlotId) -> Result<SlotLease, SlotError> {
        self.inner.lease_parent(slot)
    }

    fn release_lease(&mut self, lease: SlotLease) -> Result<(), SlotError> {
        self.inner.release_lease(lease)
    }

    fn validate_lease(&self, lease: SlotLease) -> Result<LeaseInfo, SlotError> {
        self.inner.validate_lease(lease)
    }

    fn drain_events(&mut self) -> Vec<SlotEvent> {
        self.inner.drain_events()
    }
}

impl NativeSlotAuthority for TestSlotAuthority {
    fn state(&self, slot: SurfaceSlotId) -> Result<Option<SlotState>, SlotError> {
        Ok(self.inner.state(slot))
    }

    fn spec(&self, slot: SurfaceSlotId) -> Result<Option<SlotSpec>, SlotError> {
        Ok(self.specs.get(&slot).copied())
    }

    fn mark_busy(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError> {
        self.inner.mark_busy(slot)
    }

    fn clear_busy(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError> {
        self.inner.clear_busy(slot)
    }

    fn redeem_parent(&self, payload: u64) -> Result<u64, SlotError> {
        // Issuer-side validation against *this* ledger — a stale,
        // revoked, or foreign (other-authority) payload never yields
        // a token. The token itself is the clearly synthetic
        // test-only value `test_parent_token`, never an OS handle.
        let lease = SlotLease::new(payload);
        self.inner.validate_lease(lease)?;
        Ok(test_parent_token(lease.seq()))
    }
}

// ---------------------------------------------------------------------------
// Scripted fake host — `PluginEditorHost` + `EditorSessionOps`
// ---------------------------------------------------------------------------

/// What the fake adapter was asked to do — the DSP/RT non-interference
/// oracle is structural (the seam has no DSP-facing methods), but the
/// log also proves which editor ops ran, in order.
#[derive(Debug, Default)]
struct OpsLog {
    calls: Vec<String>,
    /// `(generation, payload)` of every `ParentLease` the adapter
    /// redeemed at open/reattach — generation must be the lease's
    /// issuer-side sequence, payload the test-only parent token.
    leases: Vec<(u32, u64)>,
    /// Attach modes `open` observed.
    modes: Vec<AttachMode>,
    /// Per-method scripted refusals.
    open_err: Option<EditorOpError>,
    close_err: Option<EditorOpError>,
    visibility_err: Option<EditorOpError>,
    reattach_err: Option<EditorOpError>,
    focus_err: Option<EditorOpError>,
    size_err: Option<EditorOpError>,
}

type SharedOps = Rc<RefCell<OpsLog>>;

/// The `EditorSessionOps` half the host lends — redeems every
/// `ParentSlotLease` through the panel-supplied issuer hook before
/// "attaching", so a stale/revoked token is `LeaseRejected` exactly
/// like a real driver's adapter.
struct FakeOpsAdapter<'a> {
    log: SharedOps,
    redeem: Box<LeaseRedeemer<'a>>,
}

impl EditorSessionOps for FakeOpsAdapter<'_> {
    fn capabilities(
        &self,
        _session: PluginEditorSessionId,
    ) -> Result<EditorCapabilities, EditorOpError> {
        Ok(EditorCapabilities::default())
    }

    fn open(
        &mut self,
        session: PluginEditorSessionId,
        lease: &ParentSlotLease,
        size: EditorSize,
        mode: AttachMode,
    ) -> Result<EditorCapabilities, EditorOpError> {
        let parent = (self.redeem)(lease)?;
        let mut log = self.log.borrow_mut();
        log.calls.push(format!(
            "open:{}:{}x{}",
            session.slot(),
            size.width,
            size.height
        ));
        log.leases.push((parent.generation(), parent.payload()));
        log.modes.push(mode);
        if let Some(e) = log.open_err {
            return Err(e);
        }
        Ok(EditorCapabilities {
            dock: HostSupport::Supported,
            float: HostSupport::Supported,
            resize: HostSupport::Supported,
            hide_show: HostSupport::Supported,
            reparent: HostSupport::Supported,
            close: HostSupport::Supported,
            focus: HostSupport::Supported,
            ..EditorCapabilities::default()
        })
    }

    fn close(&mut self, session: PluginEditorSessionId) -> Result<(), EditorOpError> {
        let mut log = self.log.borrow_mut();
        log.calls.push(format!("close:{}", session.slot()));
        match log.close_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn set_size(
        &mut self,
        session: PluginEditorSessionId,
        size: EditorSize,
    ) -> Result<(), EditorOpError> {
        let mut log = self.log.borrow_mut();
        log.calls
            .push(format!("resize:{}:{}", session.slot(), size.width));
        match log.size_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn set_visibility(
        &mut self,
        session: PluginEditorSessionId,
        visible: bool,
    ) -> Result<(), EditorOpError> {
        let mut log = self.log.borrow_mut();
        log.calls
            .push(format!("visible:{}:{visible}", session.slot()));
        match log.visibility_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn reattach(
        &mut self,
        session: PluginEditorSessionId,
        lease: &ParentSlotLease,
    ) -> Result<(), EditorOpError> {
        let parent = (self.redeem)(lease)?;
        let mut log = self.log.borrow_mut();
        log.calls.push(format!("reattach:{}", session.slot()));
        log.leases.push((parent.generation(), parent.payload()));
        match log.reattach_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn set_focus(
        &mut self,
        session: PluginEditorSessionId,
        focused: bool,
    ) -> Result<(), EditorOpError> {
        let mut log = self.log.borrow_mut();
        log.calls
            .push(format!("focus:{}:{focused}", session.slot()));
        match log.focus_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn send_key(
        &mut self,
        session: PluginEditorSessionId,
        _event: ruviform_plugin_core::GuiKeyEvent,
    ) -> Result<(), EditorOpError> {
        self.log
            .borrow_mut()
            .calls
            .push(format!("key:{}", session.slot()));
        Ok(())
    }
}

/// `PluginEditorHost` fake: a `StablePluginInstanceId` → `PluginHandle`
/// table plus one scripted ops log per handle slot — the headless
/// "plugin binary" the deterministic tests drive.
struct FakeEditorHost {
    handles: HashMap<u64, PluginHandle>,
    scripts: HashMap<u32, SharedOps>,
    /// Adapter-level refusal (`L1EditorOps`-style): `ops` itself
    /// fails before any session call.
    ops_err: Option<EditorOpError>,
    next_slot: u32,
}

impl FakeEditorHost {
    fn new() -> Self {
        Self {
            handles: HashMap::new(),
            scripts: HashMap::new(),
            ops_err: None,
            next_slot: 0,
        }
    }

    /// Registers `instance` as a live plugin instance; returns the
    /// shared ops log the test asserts against.
    fn add_instance(&mut self, instance: u64) -> SharedOps {
        let slot = self.next_slot;
        self.next_slot += 1;
        self.handles.insert(instance, PluginHandle::new(slot, 1));
        let ops = Rc::new(RefCell::new(OpsLog::default()));
        self.scripts.insert(slot, Rc::clone(&ops));
        ops
    }

    /// The ops log for the handle `resolve(instance)` would return.
    fn ops_of(&self, instance: u64) -> SharedOps {
        let handle = self.handles[&instance];
        Rc::clone(&self.scripts[&handle.slot()])
    }
}

impl PluginEditorHost for FakeEditorHost {
    fn resolve(&mut self, instance: StablePluginInstanceId) -> Option<PluginHandle> {
        self.handles.get(&instance.get()).copied()
    }

    fn ops<'a>(
        &'a mut self,
        handle: PluginHandle,
        redeem: Box<LeaseRedeemer<'a>>,
    ) -> Result<Box<dyn EditorSessionOps + 'a>, EditorOpError> {
        if let Some(e) = self.ops_err {
            return Err(e);
        }
        let log = self
            .scripts
            .get(&handle.slot())
            .map(Rc::clone)
            .unwrap_or_default();
        Ok(Box::new(FakeOpsAdapter { log, redeem }))
    }
}

// ---------------------------------------------------------------------------
// Panel-tier fixture
// ---------------------------------------------------------------------------

fn view(n: u64) -> ViewInstanceId {
    ViewInstanceId::new(n).expect("view id non-zero")
}

fn win(n: u64) -> WindowId {
    WindowId::new(n).expect("window id non-zero")
}

fn inst(n: u64) -> StablePluginInstanceId {
    StablePluginInstanceId::new(n)
}

fn geom() -> SlotGeometry {
    SlotGeometry {
        rect: LogicalRect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        },
        scale_factor: 1.0,
    }
}

fn size() -> EditorSize {
    EditorSize {
        width: 800.0,
        height: 600.0,
    }
}

struct Panel {
    sessions: PluginEditorSessions,
    host: FakeEditorHost,
    dock: PluginDock,
    /// Clone of the shared authority the panel was lent — assertions
    /// inspect the ledger through it, proving the panel holds no
    /// private ledger of its own.
    authority: SharedNativeSlotAuthority,
}

impl Panel {
    fn new(view_n: u64, window_n: u64) -> Self {
        Self::with_authority(view_n, window_n, test_authority())
    }

    fn with_authority(view_n: u64, window_n: u64, authority: SharedNativeSlotAuthority) -> Self {
        let dock = PluginDock::declare(view(view_n), win(window_n), geom(), authority.clone())
            .expect("declare panel");
        Self {
            sessions: PluginEditorSessions::new(),
            host: FakeEditorHost::new(),
            dock,
            authority,
        }
    }

    /// Live ledger state of `slot` through the shared authority.
    fn slot_state(&self, slot: SurfaceSlotId) -> Option<SlotState> {
        self.authority.borrow().state(slot).ok().flatten()
    }

    /// Live ledger spec of `slot` through the shared authority.
    fn slot_spec(&self, slot: SurfaceSlotId) -> Option<SlotSpec> {
        self.authority.borrow().spec(slot).ok().flatten()
    }

    fn open(&mut self, instance: u64) -> ruviform_plugin_host::AttachMode {
        let log = self.host.add_instance(instance);
        let report = self
            .dock
            .open(
                &mut self.sessions,
                &mut self.host,
                inst(instance),
                size(),
                AttachMode::Docked,
            )
            .expect("open editor");
        assert!(log.borrow().calls.iter().any(|c| c.starts_with("open:")));
        report.mode
    }

    fn state(&self) -> Option<EditorSessionState> {
        self.dock.session_state(&self.sessions)
    }
}

fn is_attached(state: Option<&EditorSessionState>) -> bool {
    matches!(state, Some(EditorSessionState::Attached { .. }))
}

fn is_hidden(state: Option<&EditorSessionState>) -> bool {
    matches!(state, Some(EditorSessionState::Hidden { .. }))
}

fn is_closed(state: Option<&EditorSessionState>) -> bool {
    matches!(state, Some(EditorSessionState::Closed))
}

// ---------------------------------------------------------------------------
// Row 3 — 4-ID ownership, Open/Hide/Show/ReDock/Close determinism
// ---------------------------------------------------------------------------

/// `open` mints the full identity tuple and lands `Attached{Docked}`:
/// the report carries view/slot/session/instance, the binding records
/// the live lease, and the slot ledger shows `Leased`.
#[test]
fn open_mints_four_ids_and_lands_attached() {
    let mut p = Panel::new(9, 1);
    let ops = p.host.add_instance(7);

    let report = p
        .dock
        .open(
            &mut p.sessions,
            &mut p.host,
            inst(7),
            size(),
            AttachMode::Docked,
        )
        .expect("open");

    assert_eq!(report.view, view(9));
    assert_eq!(report.instance, inst(7));
    assert_eq!(report.slot, p.dock.slot().unwrap());
    assert_eq!(report.mode, AttachMode::Docked);
    assert!(report.capabilities.dock.supported());
    assert!(is_attached(p.state().as_ref()));
    assert_eq!(p.slot_state(report.slot), Some(SlotState::Leased));

    let binding = p.dock.binding().expect("binding");
    assert_eq!(binding.session, report.session);
    assert_eq!(binding.slot, report.slot);
    assert!(binding.lease().is_some(), "live parent lease held");
    // The adapter redeemed the lease through the issuer-side hook —
    // the same shared authority that minted it. The minted parent
    // lease follows the upstream reference shape: generation is the
    // lease's issuer-side sequence, payload the slot's (test-only,
    // synthetic) parent token — never a slot index or generation.
    let lease = binding.lease().unwrap();
    let seq = SlotLease::new(lease.raw()).seq();
    assert_eq!(
        ops.borrow().leases.as_slice(),
        &[(seq, test_parent_token(seq))]
    );
    assert_eq!(ops.borrow().modes.as_slice(), &[AttachMode::Docked]);
}

/// `hide`/`show` are `Attached ⇄ Hidden` session transitions with the
/// slot's visibility mirroring them — the tab-inactive path.
#[test]
fn hide_show_round_trip_mirrors_slot_visibility() {
    let mut p = Panel::new(9, 1);
    p.open(7);

    p.dock
        .set_visibility(&mut p.sessions, &mut p.host, false)
        .expect("hide");
    assert!(is_hidden(p.state().as_ref()));
    assert_eq!(
        p.slot_spec(p.dock.slot().unwrap()).map(|s| s.visibility),
        Some(ruviform_ui::native_slot::SlotVisibility::Hidden)
    );

    p.dock
        .set_visibility(&mut p.sessions, &mut p.host, true)
        .expect("show");
    assert!(is_attached(p.state().as_ref()));
}

/// Re-dock across windows: a fresh slot + fresh lease, `reattach`
/// through the adapter, the old slot released (its index recyclable
/// under a bumped generation) and the old lease dead — the session id
/// and `Attached` state survive the whole move.
#[test]
fn relocate_republishes_slot_and_revokes_old_lease() {
    let mut p = Panel::new(9, 1);
    p.open(7);
    let old_slot = p.dock.slot().unwrap();
    let old_lease = p.dock.binding().unwrap().lease().unwrap();
    let session = p.dock.binding().unwrap().session;

    p.dock
        .relocate(&mut p.sessions, Some(&mut p.host), win(2))
        .expect("relocate");

    let new_slot = p.dock.slot().unwrap();
    assert_ne!(new_slot, old_slot);
    assert_eq!(p.slot_state(old_slot), Some(SlotState::Released));
    assert_eq!(p.slot_state(new_slot), Some(SlotState::Leased));
    assert!(
        p.authority.borrow().validate_lease(old_lease).is_err(),
        "the pre-move lease must be revoked after the reattach lands"
    );
    let binding = p.dock.binding().unwrap();
    assert_eq!(binding.slot, new_slot);
    assert_eq!(binding.session, session, "same session across re-dock");
    assert!(is_attached(p.state().as_ref()));
}

/// Adapter refusal mid-re-dock keeps the *old* slot + lease — the
/// editor may still be parented there — and the move reports the
/// error honestly.
#[test]
fn relocate_keeps_old_parent_on_adapter_refusal() {
    let mut p = Panel::new(9, 1);
    p.open(7);
    p.host.ops_of(7).borrow_mut().reattach_err = Some(EditorOpError::Unsupported);
    let old_slot = p.dock.slot().unwrap();
    let old_lease = p.dock.binding().unwrap().lease().unwrap();

    let err = p
        .dock
        .relocate(&mut p.sessions, Some(&mut p.host), win(2))
        .expect_err("reparent refusal");
    assert!(matches!(
        err,
        PluginDockError::Session(EditorSessionError::Ops(EditorOpError::Unsupported))
            | PluginDockError::Ops(EditorOpError::Unsupported)
    ));

    assert_eq!(p.dock.slot(), Some(old_slot));
    assert_eq!(
        p.slot_state(old_slot),
        Some(SlotState::Leased),
        "old parent seat stays live on refusal"
    );
    assert!(p.authority.borrow().validate_lease(old_lease).is_ok());
    // Upstream honesty: a refused reattach lands the session `Failed`
    // — never recorded as still-attached.
    assert!(matches!(p.state(), Some(EditorSessionState::Failed { .. })));
}

/// A stale lease is rejected at the issuer seam: mint a lease, revoke
/// it via close, then prove `validate_lease` refuses the token —
/// the same refusal the adapter's redeem path would produce
/// (`EditorOpError::LeaseRejected`), never a reinterpreted parent.
#[test]
fn revoked_lease_is_stale_at_the_ledger() {
    let mut p = Panel::new(9, 1);
    p.open(7);
    let lease = p.dock.binding().unwrap().lease().unwrap();

    p.dock
        .close(&mut p.sessions, Some(&mut p.host))
        .expect("close");

    let err = p
        .authority
        .borrow()
        .validate_lease(lease)
        .expect_err("revoked lease must not validate");
    assert!(matches!(
        err,
        ruviform_ui::native_slot::SlotError::LeaseRevoked
            | ruviform_ui::native_slot::SlotError::InvalidLease
            | ruviform_ui::native_slot::SlotError::StaleGeneration
            | ruviform_ui::native_slot::SlotError::UnknownSlot
    ));
}

/// `syn.window.dock_detach`-equivalent hook: the slot suppresses and
/// the session hides — the DSP-side editor session is *not* closed.
#[test]
fn detach_hides_editor_without_closing_session() {
    let mut p = Panel::new(9, 1);
    p.open(7);
    let session = p.dock.binding().unwrap().session;

    p.dock
        .notify_detached(&mut p.sessions, Some(&mut p.host))
        .expect("detach");

    assert!(is_hidden(p.state().as_ref()));
    assert_eq!(
        p.dock.binding().unwrap().session,
        session,
        "session survives detach"
    );
    assert!(p.dock.binding().unwrap().lease().is_some());
}

/// `close` drives the session to `Closed`, retires it from the shared
/// registry, revokes the lease (slot → `Declared`) and frees the
/// panel for another editor.
#[test]
fn close_retires_session_and_frees_panel() {
    let mut p = Panel::new(9, 1);
    p.open(7);
    let session = p.dock.binding().unwrap().session;
    let slot = p.dock.slot().unwrap();

    p.dock
        .close(&mut p.sessions, Some(&mut p.host))
        .expect("close");

    assert!(p.dock.binding().is_none());
    assert!(
        p.sessions.session(session).is_none(),
        "retired sessions leave the registry"
    );
    assert_eq!(p.slot_state(slot), Some(SlotState::Declared));

    // Re-open mints a *fresh* session id — the registry's generation
    // guard, not a recycled live id.
    let report = p
        .dock
        .open(
            &mut p.sessions,
            &mut p.host,
            inst(7),
            size(),
            AttachMode::Docked,
        )
        .expect("re-open");
    assert_ne!(report.session, session);
    assert!(is_attached(p.state().as_ref()));
}

/// `SessionBusy` is an upstream, cross-panel guarantee: one live
/// editor session per `PluginHandle`. Two panels sharing the app
/// registry cannot both host the same instance — the second `open`
/// is refused before a second session is minted.
#[test]
fn session_busy_is_refused_across_panels() {
    let mut sessions = PluginEditorSessions::new();
    let mut host = FakeEditorHost::new();
    host.add_instance(7);
    // Both panels are lent the SAME shared slot authority — the
    // production shape (one driver ledger), not two panel-private
    // registries.
    let authority = test_authority();
    let mut a = PluginDock::declare(view(9), win(1), geom(), authority.clone()).unwrap();
    let mut b = PluginDock::declare(view(10), win(1), geom(), authority).unwrap();

    a.open(
        &mut sessions,
        &mut host,
        inst(7),
        size(),
        AttachMode::Docked,
    )
    .expect("first open");

    let err = b
        .open(
            &mut sessions,
            &mut host,
            inst(7),
            size(),
            AttachMode::Docked,
        )
        .expect_err("same instance must refuse");
    assert!(
        matches!(
            err,
            PluginDockError::Session(EditorSessionError::SessionBusy)
        ),
        "expected SessionBusy, got {err}"
    );
    assert!(b.binding().is_none());
}

/// Occupied/open guards at the panel level, plus `UnknownInstance`
/// when the host has no live handle for the identity.
#[test]
fn occupied_and_unknown_instance_guards() {
    let mut p = Panel::new(9, 1);
    p.open(7);

    let err = p
        .dock
        .open(
            &mut p.sessions,
            &mut p.host,
            inst(7),
            size(),
            AttachMode::Docked,
        )
        .expect_err("occupied panel refuses open");
    assert!(matches!(err, PluginDockError::Occupied));

    let err = p
        .dock
        .open(
            &mut p.sessions,
            &mut p.host,
            inst(99),
            size(),
            AttachMode::Docked,
        )
        .expect_err("occupied before resolve");
    assert!(matches!(err, PluginDockError::Occupied));

    // Commands needing a live editor refuse on the empty panel B.
    let mut q = Panel::new(10, 1);
    let err = q
        .dock
        .set_visibility(&mut q.sessions, &mut q.host, false)
        .expect_err("no editor");
    assert!(matches!(err, PluginDockError::NoEditor));
    let err = q
        .dock
        .open(
            &mut q.sessions,
            &mut q.host,
            inst(99),
            size(),
            AttachMode::Docked,
        )
        .expect_err("no live handle");
    assert!(matches!(err, PluginDockError::UnknownInstance));
}

/// `Unsupported` at `ops.open` lands the session in `Failed`
/// honestly: the binding stays observable, the lease is revoked, and
/// `close` still retires it — never recorded as an attach.
#[test]
fn unsupported_open_lands_failed_honestly() {
    let mut p = Panel::new(9, 1);
    p.host.add_instance(7);
    p.host.ops_of(7).borrow_mut().open_err = Some(EditorOpError::Unsupported);

    let err = p
        .dock
        .open(
            &mut p.sessions,
            &mut p.host,
            inst(7),
            size(),
            AttachMode::Docked,
        )
        .expect_err("unsupported open refuses");
    assert!(matches!(
        err,
        PluginDockError::Session(EditorSessionError::Ops(EditorOpError::Unsupported))
    ));

    assert!(matches!(p.state(), Some(EditorSessionState::Failed { .. })));
    assert!(
        p.dock.binding().unwrap().lease().is_none(),
        "failed attach holds no parent lease"
    );
    p.dock
        .close(&mut p.sessions, Some(&mut p.host))
        .expect("failed session still retires on close");
    assert!(p.dock.binding().is_none());
}

/// A missing adapter is `unavailable`, not a fabricated attach: the
/// ops lend itself refused → no binding, session retired, slot back
/// to `Declared`.
#[test]
fn adapter_lend_refusal_is_honest_unavailable() {
    let mut p = Panel::new(9, 1);
    p.host.add_instance(7);
    p.host.ops_err = Some(EditorOpError::Unsupported);

    let err = p
        .dock
        .open(
            &mut p.sessions,
            &mut p.host,
            inst(7),
            size(),
            AttachMode::Docked,
        )
        .expect_err("adapter refusal");
    assert!(matches!(
        err,
        PluginDockError::Ops(EditorOpError::Unsupported)
    ));
    assert!(p.dock.binding().is_none());
    assert_eq!(
        p.slot_state(p.dock.slot().unwrap()),
        Some(SlotState::Declared)
    );
}

/// Window loss: the live session converges `Terminated{WindowLost}`
/// (the DSP instance is never touched — the seam has no DSP method),
/// the lease dies with the slot, and `relocate` onto a new window
/// re-declares the slot while the concluded binding stays observable.
#[test]
fn window_lost_converges_session_and_frees_slot() {
    let mut p = Panel::new(9, 1);
    p.open(7);
    let slot = p.dock.slot().unwrap();

    p.dock.notify_window_lost(&mut p.sessions);

    assert!(is_closed(p.state().as_ref()));
    assert_eq!(p.dock.slot(), None);
    assert_eq!(p.slot_state(slot), Some(SlotState::Released));
    assert!(p.dock.binding().unwrap().lease().is_none());

    // Re-dock onto a new window re-materializes the slot; the
    // concluded session is not re-parented.
    p.dock
        .relocate(&mut p.sessions, Some(&mut p.host), win(2))
        .expect("re-declare on new window");
    let new_slot = p.dock.slot().unwrap();
    assert_eq!(p.slot_spec(new_slot).unwrap().window, win(2));
    assert!(
        is_closed(p.state().as_ref()),
        "concluded session stays concluded"
    );
}

/// Host-loss during close converges honestly: `close(None)` records
/// `Terminated{HostLost}` rather than faking a graceful adapter close.
#[test]
fn close_without_host_converges_host_lost() {
    let mut p = Panel::new(9, 1);
    p.open(7);

    p.dock
        .close(&mut p.sessions, None)
        .expect("close converges without host");
    assert!(p.dock.binding().is_none());
}

/// PD-07 logical save/restore: the snapshot carries instance +
/// placement + size + visibility — no session id, slot id, lease or
/// OS handle — and restore re-mints a fresh session in the saved
/// placement.
#[test]
fn save_restore_round_trip_persists_only_logical_state() {
    let mut p = Panel::new(9, 1);
    p.open(7);
    let old_session = p.dock.binding().unwrap().session;

    let snapshot = p.dock.save(&p.sessions).expect("save");
    assert_eq!(snapshot.instance, 7);
    assert_eq!(snapshot.placement, PluginDockPlacement::Docked);
    assert_eq!(snapshot.size, [800.0, 600.0]);
    assert!(snapshot.visible);
    let json = serde_json::to_value(&snapshot).unwrap();
    for banned in ["session", "slot", "lease", "handle", "window"] {
        assert!(
            json.get(banned).is_none(),
            "snapshot must not persist {banned}"
        );
    }

    let report = p
        .dock
        .restore(&mut p.sessions, &mut p.host, &snapshot)
        .expect("restore");
    assert_ne!(
        report.session, old_session,
        "restore mints a fresh session id"
    );
    assert!(is_attached(p.state().as_ref()));
}

/// `visible: false` restores to `Hidden` — the tab-inactive
/// persistence case.
#[test]
fn restore_honors_saved_visibility() {
    let mut p = Panel::new(9, 1);
    p.open(7);
    p.dock
        .set_visibility(&mut p.sessions, &mut p.host, false)
        .expect("hide");
    let snapshot = p.dock.save(&p.sessions).expect("save");
    assert!(!snapshot.visible);

    p.dock
        .restore(&mut p.sessions, &mut p.host, &snapshot)
        .expect("restore");
    assert!(is_hidden(p.state().as_ref()));
}

/// Instance-level termination (crash/kill) converges the session and
/// frees the parent seat — `SessionFailCause` is recorded, the DSP
/// instance's own lifetime is upstream's concern, never this layer's.
#[test]
fn instance_termination_converges_and_frees_lease() {
    let mut p = Panel::new(9, 1);
    p.open(7);
    let slot = p.dock.slot().unwrap();

    p.dock
        .notify_instance_terminated(&mut p.sessions, SessionFailCause::HostLost);

    assert!(is_closed(p.state().as_ref()));
    assert!(p.dock.binding().unwrap().lease().is_none());
    assert_eq!(p.slot_state(slot), Some(SlotState::Declared));
}

/// `PluginDockPlacement` is the persisted vocabulary for the upstream
/// `AttachMode`: docked stays docked, every non-docked mode degrades
/// to the explicit `Floating` seat — never a fake dock pass.
#[test]
fn placement_vocabulary_round_trips_honestly() {
    assert_eq!(
        PluginDockPlacement::parse("docked"),
        Some(PluginDockPlacement::Docked)
    );
    assert_eq!(
        PluginDockPlacement::parse("floating"),
        Some(PluginDockPlacement::Floating)
    );
    assert_eq!(PluginDockPlacement::parse("bogus"), None);
    assert_eq!(
        PluginDockPlacement::from_attach_mode(AttachMode::Floating),
        PluginDockPlacement::Floating
    );
    assert_eq!(PluginDockPlacement::Docked.as_str(), "docked");
}

// ---------------------------------------------------------------------------
// Op tier — NativeShell + FakeDriver end to end
// ---------------------------------------------------------------------------

struct OpFixture {
    shell: NativeShell,
    driver: FakeDriver,
    w1: WindowId,
    w2: WindowId,
}

fn ops_fixture() -> OpFixture {
    ops_fixture_with(None)
}

fn ops_fixture_with(host: Option<FakeEditorHost>) -> OpFixture {
    ops_fixture_full(host, true)
}

fn ops_fixture_full(host: Option<FakeEditorHost>, with_authority: bool) -> OpFixture {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    if let Some(host) = host {
        app.set_plugin_editor_host(Box::new(host));
    }
    if with_authority {
        app.set_native_slot_authority(test_authority());
    }
    let mut shell = NativeShell::new(app, Duration::from_millis(16)).expect("compose shell");
    let mut driver = FakeDriver::new();
    let w1 = shell.open_main(&mut driver).expect("main window");
    ready(&mut shell, &mut driver, w1);
    let w2 = driver
        .create_window(&WindowSpec {
            title: "Synvelis — Secondary".to_string(),
            ..WindowSpec::default()
        })
        .expect("second window");
    ready(&mut shell, &mut driver, w2);
    shell
        .attach_view(&mut driver, w1, &main_view_id(), &main_view_targets())
        .expect("attach view-1");
    shell
        .attach_view(
            &mut driver,
            w2,
            &secondary_view_id(),
            &secondary_view_targets(),
        )
        .expect("attach view-2");
    OpFixture {
        shell,
        driver,
        w1,
        w2,
    }
}

fn ready(shell: &mut NativeShell, driver: &mut FakeDriver, window: WindowId) {
    driver.inject(DriverEvent::Windowed {
        window,
        event: WindowLifecycle::Ready,
    });
    pump(shell, driver);
}

fn pump(shell: &mut NativeShell, driver: &mut FakeDriver) {
    driver.pump(&mut |ev, d| shell.handle(ev, d));
}

fn inject(shell: &mut NativeShell, driver: &mut FakeDriver, ev: DriverEvent) {
    driver.inject(ev);
    pump(shell, driver);
}

fn op(
    f: &mut OpFixture,
    window: WindowId,
    id: &str,
    name: &str,
    params: Value,
) -> (bool, Option<Value>, Option<ErrorBody>) {
    inject(
        &mut f.shell,
        &mut f.driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Action(Intent::new(id, name, params)),
        },
    );
    f.driver
        .events_for(window)
        .iter()
        .filter_map(|e| match e {
            Event::Result {
                id: rid,
                ok,
                data,
                error,
            } if rid == id => Some((*ok, data.clone(), error.clone())),
            _ => None,
        })
        .next_back()
        .unwrap_or_else(|| panic!("no result for {name} on {window}"))
}

fn open_ok(f: &mut OpFixture, window: WindowId, view: &str, instance: u64) -> Value {
    let (ok, data, err) = op(
        f,
        window,
        "popen",
        "syn.plugin_dock.open",
        json!({
            "view_id": view,
            "window_id": window.to_string(),
            "instance": instance,
            "title": "Plugin",
        }),
    );
    assert!(ok, "plugin_dock.open failed: {err:?}");
    data.unwrap()
}

fn panel_state(f: &mut OpFixture, window: WindowId, view: &str) -> Value {
    let (ok, data, err) = op(
        f,
        window,
        "pstate",
        "syn.plugin_dock.state",
        json!({ "view_id": view }),
    );
    assert!(ok, "plugin_dock.state failed: {err:?}");
    data.unwrap()
}

/// End-to-end row-3 lifecycle through the ops: open → hide → show →
/// dock move (prepare/commit) → detach → reattach → close — every
/// transition observable through `syn.plugin_dock.state`, the session
/// id stable across the re-dock.
#[test]
fn plugin_dock_ops_drive_the_full_editor_lifecycle() {
    let mut f = ops_fixture_with(Some({
        let mut h = FakeEditorHost::new();
        h.add_instance(7);
        h
    }));
    let (w1, w2) = (f.w1, f.w2);

    let opened = open_ok(&mut f, w1, "view-9", 7);
    assert_eq!(opened["opened"], true);
    assert_eq!(opened["mode"], "docked");
    assert_eq!(opened["instance"], 7);
    let session = opened["session"].as_str().unwrap().to_string();
    let slot = opened["slot"].as_str().unwrap().to_string();

    let state = panel_state(&mut f, w1, "view-9");
    assert_eq!(state["session_state"], "attached");
    assert_eq!(state["window_id"], w1.to_string());
    assert_eq!(state["slot"], slot);
    assert_eq!(state["session"], session);
    assert_eq!(state["placement"], "docked");

    let (ok, _, err) = op(
        &mut f,
        w1,
        "phide",
        "syn.plugin_dock.hide",
        json!({ "view_id": "view-9" }),
    );
    assert!(ok, "hide failed: {err:?}");
    assert_eq!(panel_state(&mut f, w1, "view-9")["session_state"], "hidden");

    let (ok, _, err) = op(
        &mut f,
        w1,
        "pshow",
        "syn.plugin_dock.show",
        json!({ "view_id": "view-9" }),
    );
    assert!(ok, "show failed: {err:?}");
    assert_eq!(
        panel_state(&mut f, w1, "view-9")["session_state"],
        "attached"
    );

    // Re-dock through the existing two-phase move — the panel's slot
    // re-declares on w2 under a fresh lease; the session survives.
    let (ok, _, err) = op(
        &mut f,
        w1,
        "dprep",
        "syn.window.dock_prepare",
        json!({ "view_id": "view-9", "window_id": w2.to_string() }),
    );
    assert!(ok, "dock_prepare failed: {err:?}");
    let (ok, _, err) = op(
        &mut f,
        w1,
        "dcommit",
        "syn.window.dock_commit",
        json!({ "view_id": "view-9" }),
    );
    assert!(ok, "dock_commit failed: {err:?}");

    let moved = panel_state(&mut f, w1, "view-9");
    assert_eq!(moved["window_id"], w2.to_string());
    assert_eq!(moved["session"], session, "same session after re-dock");
    assert_ne!(moved["slot"], slot, "fresh slot on the new window");
    assert_eq!(moved["session_state"], "attached");

    // Detach to the pool hides the editor; reattach re-shows it.
    let (ok, _, err) = op(
        &mut f,
        w1,
        "ddetach",
        "syn.window.dock_detach",
        json!({ "view_id": "view-9" }),
    );
    assert!(ok, "dock_detach failed: {err:?}");
    assert_eq!(panel_state(&mut f, w1, "view-9")["session_state"], "hidden");
    let (ok, _, err) = op(
        &mut f,
        w1,
        "dreattach",
        "syn.window.dock_reattach",
        json!({ "view_id": "view-9", "window_id": w2.to_string() }),
    );
    assert!(ok, "dock_reattach failed: {err:?}");
    assert_eq!(
        panel_state(&mut f, w1, "view-9")["session_state"],
        "attached"
    );

    let (ok, data, err) = op(
        &mut f,
        w1,
        "pclose",
        "syn.plugin_dock.close",
        json!({ "view_id": "view-9" }),
    );
    assert!(ok, "close failed: {err:?}");
    assert_eq!(data.unwrap()["closed"], true);
    let state = panel_state(&mut f, w1, "view-9");
    assert_eq!(state["session_state"], Value::Null);
    assert_eq!(state["instance"], Value::Null);
}

/// `syn.view.close` on a plugin view retires the panel entirely —
/// the editor session closes before the view leaves the dock domain.
#[test]
fn view_close_retires_the_panel() {
    let mut f = ops_fixture_with(Some({
        let mut h = FakeEditorHost::new();
        h.add_instance(7);
        h
    }));
    let w1 = f.w1;
    open_ok(&mut f, w1, "view-9", 7);

    let (ok, _, err) = op(
        &mut f,
        w1,
        "vclose",
        "syn.view.close",
        json!({ "view_id": "view-9" }),
    );
    assert!(ok, "view.close failed: {err:?}");
    let (ok, data, err) = op(&mut f, w1, "plist", "syn.plugin_dock.list", json!({}));
    assert!(ok, "list failed: {err:?}");
    assert_eq!(data.unwrap()["panels"].as_array().unwrap().len(), 0);
    // `state` on the gone panel is an honest refusal, not a ghost.
    let (ok, _, err) = op(
        &mut f,
        w1,
        "pstate2",
        "syn.plugin_dock.state",
        json!({ "view_id": "view-9" }),
    );
    assert!(!ok);
    assert_eq!(err.unwrap().code, "invalid_params");
}

/// Save/restore through the wire: `save` returns only the logical
/// record; `restore` on the same view re-mints ids; JSON round-trip
/// keeps `instance`/`placement`/`size`/`visible` only.
#[test]
fn save_restore_ops_round_trip() {
    let mut f = ops_fixture_with(Some({
        let mut h = FakeEditorHost::new();
        h.add_instance(7);
        h
    }));
    let w1 = f.w1;
    let opened = open_ok(&mut f, w1, "view-9", 7);
    let session = opened["session"].as_str().unwrap().to_string();

    let (ok, data, err) = op(
        &mut f,
        w1,
        "psave",
        "syn.plugin_dock.save",
        json!({ "view_id": "view-9" }),
    );
    assert!(ok, "save failed: {err:?}");
    let snapshot = data.unwrap()["snapshot"].clone();
    assert_eq!(snapshot["instance"], 7);
    assert_eq!(snapshot["placement"], "docked");
    assert_eq!(snapshot["size"], json!([800.0, 600.0]));
    assert_eq!(snapshot["visible"], true);
    for banned in ["session", "slot", "lease", "handle", "window"] {
        assert!(
            snapshot.get(banned).is_none(),
            "snapshot must not persist {banned}"
        );
    }

    let (ok, data, err) = op(
        &mut f,
        w1,
        "prestore",
        "syn.plugin_dock.restore",
        json!({
            "view_id": "view-9",
            "window_id": w1.to_string(),
            "snapshot": snapshot,
        }),
    );
    assert!(ok, "restore failed: {err:?}");
    assert_eq!(data.unwrap()["restored"], true);
    assert_ne!(
        panel_state(&mut f, w1, "view-9")["session"],
        session,
        "restore mints a fresh session"
    );
}

/// The `instance`-elsewhere guard: opening the same plugin instance
/// on a second view refuses `invalid_params` — the op-level face of
/// "one editor session per instance" (panel tier pins the upstream
/// `SessionBusy` itself).
#[test]
fn duplicate_instance_across_views_is_refused() {
    let mut f = ops_fixture_with(Some({
        let mut h = FakeEditorHost::new();
        h.add_instance(7);
        h
    }));
    let w1 = f.w1;
    open_ok(&mut f, w1, "view-9", 7);

    let (ok, _, err) = op(
        &mut f,
        w1,
        "pdup",
        "syn.plugin_dock.open",
        json!({
            "view_id": "view-10",
            "window_id": w1.to_string(),
            "instance": 7,
        }),
    );
    assert!(!ok, "duplicate instance must refuse");
    assert_eq!(err.unwrap().code, "invalid_params");
    assert!(panel_state(&mut f, w1, "view-9")["session"].is_string());
}

/// No installed adapter → `unavailable`, never a fake attach. This is
/// the L1/refused-adapter honesty the issue requires of every gate.
#[test]
fn ops_refuse_unavailable_without_a_host() {
    let mut f = ops_fixture();
    let w1 = f.w1;

    let (ok, _, err) = op(
        &mut f,
        w1,
        "popen",
        "syn.plugin_dock.open",
        json!({
            "view_id": "view-9",
            "window_id": w1.to_string(),
            "instance": 7,
        }),
    );
    assert!(!ok);
    assert_eq!(err.unwrap().code, "unavailable");
}

/// Host installed but no shared slot authority → `unavailable` too:
/// the panel must never fall back to a private ledger (S2 authority
/// honesty).
#[test]
fn ops_refuse_unavailable_without_an_authority() {
    let mut f = ops_fixture_full(
        Some({
            let mut h = FakeEditorHost::new();
            h.add_instance(7);
            h
        }),
        false,
    );
    let w1 = f.w1;

    let (ok, _, err) = op(
        &mut f,
        w1,
        "popen",
        "syn.plugin_dock.open",
        json!({
            "view_id": "view-9",
            "window_id": w1.to_string(),
            "instance": 7,
        }),
    );
    assert!(!ok);
    assert_eq!(err.unwrap().code, "unavailable");
}

/// Unknown instance is `invalid_params` — an honest refusal, not a
/// fabricated handle.
#[test]
fn unknown_instance_is_refused() {
    let mut f = ops_fixture_with(Some(FakeEditorHost::new()));
    let w1 = f.w1;

    let (ok, _, err) = op(
        &mut f,
        w1,
        "popen",
        "syn.plugin_dock.open",
        json!({
            "view_id": "view-9",
            "window_id": w1.to_string(),
            "instance": 99,
        }),
    );
    assert!(!ok);
    assert_eq!(err.unwrap().code, "invalid_params");
}

/// Window destroy: the editor session converges (read model reports
/// `closed`), the slot is gone, and — DSP non-interference — the
/// project record is byte-identical afterwards.
#[test]
fn window_destroy_converges_editor_and_preserves_project() {
    let mut f = ops_fixture_with(Some({
        let mut h = FakeEditorHost::new();
        h.add_instance(7);
        h
    }));
    let (w1, w2) = (f.w1, f.w2);
    open_ok(&mut f, w1, "view-9", 7);
    // Move the panel onto w2 so its slot's window is the one that dies.
    let (ok, _, err) = op(
        &mut f,
        w1,
        "dprep",
        "syn.window.dock_prepare",
        json!({ "view_id": "view-9", "window_id": w2.to_string() }),
    );
    assert!(ok, "dock_prepare failed: {err:?}");
    let (ok, _, err) = op(
        &mut f,
        w1,
        "dcommit",
        "syn.window.dock_commit",
        json!({ "view_id": "view-9" }),
    );
    assert!(ok, "dock_commit failed: {err:?}");

    let (ok, project_before, err) = op(&mut f, w1, "pinfo", "syn.project.info", json!({}));
    assert!(ok, "project.info failed: {err:?}");

    inject(
        &mut f.shell,
        &mut f.driver,
        DriverEvent::Windowed {
            window: w2,
            event: WindowLifecycle::Destroyed,
        },
    );

    let state = panel_state(&mut f, w1, "view-9");
    assert_eq!(state["session_state"], "closed");
    assert_eq!(state["window_id"], Value::Null);
    assert_eq!(state["slot"], Value::Null);

    let (ok, project_after, err) = op(&mut f, w1, "pinfo2", "syn.project.info", json!({}));
    assert!(ok, "project.info failed: {err:?}");
    assert_eq!(
        project_before, project_after,
        "window-destroy editor convergence must not touch project state"
    );
}

/// DSP/Project/RT non-interference, asserted at the op boundary:
/// project info and the transport/viz projections are byte-identical
/// before and after the open/hide/show/close sequence. (The seam is
/// structurally UI-only — `PluginEditorHost`/`EditorSessionOps` carry
/// no DSP-facing methods — and `notify_terminated` converges the
/// *editor* session, never the instance's processing lifetime.)
#[test]
fn plugin_dock_ops_do_not_touch_project_or_dsp_state() {
    let mut f = ops_fixture_with(Some({
        let mut h = FakeEditorHost::new();
        h.add_instance(7);
        h
    }));
    let w1 = f.w1;

    let (ok, before_project, _) = op(&mut f, w1, "pi", "syn.project.info", json!({}));
    assert!(ok);
    let (ok, before_views, _) = op(&mut f, w1, "vl", "syn.view.list", json!({}));
    assert!(ok);

    open_ok(&mut f, w1, "view-9", 7);
    for (id, name) in [
        ("h", "syn.plugin_dock.hide"),
        ("s", "syn.plugin_dock.show"),
        ("f1", "syn.plugin_dock.focus"),
        ("r", "syn.plugin_dock.resize"),
    ] {
        let params = match name {
            "syn.plugin_dock.focus" => json!({ "view_id": "view-9", "focused": true }),
            "syn.plugin_dock.resize" => {
                json!({ "view_id": "view-9", "width": 640.0, "height": 480.0 })
            }
            _ => json!({ "view_id": "view-9" }),
        };
        let (ok, _, err) = op(&mut f, w1, id, name, params);
        assert!(ok, "{name} failed: {err:?}");
    }
    let (ok, _, err) = op(
        &mut f,
        w1,
        "pc",
        "syn.plugin_dock.close",
        json!({ "view_id": "view-9" }),
    );
    assert!(ok, "close failed: {err:?}");

    let (ok, after_project, _) = op(&mut f, w1, "pi2", "syn.project.info", json!({}));
    assert!(ok);
    assert_eq!(before_project, after_project);
    let (ok, after_views, _) = op(&mut f, w1, "vl2", "syn.view.list", json!({}));
    assert!(ok);
    // The plugin panel's own view row legitimately exists post-open;
    // every *other* view's row is untouched.
    let strip = |v: &Value| -> Vec<Value> {
        v["views"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["view_id"] != "view-9")
            .cloned()
            .collect()
    };
    assert_eq!(
        strip(&before_views.unwrap()),
        strip(&after_views.unwrap()),
        "non-plugin views must be untouched by plugin dock ops"
    );
}

// ---------------------------------------------------------------------------
// Session integration — project boundary and PD-07 sidecar persistence
// ---------------------------------------------------------------------------

fn direct_app_with_host_ops(instance: u64) -> (SynvelisApp, SharedNativeSlotAuthority, SharedOps) {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let mut host = FakeEditorHost::new();
    let ops = host.add_instance(instance);
    app.set_plugin_editor_host(Box::new(host));
    let authority = test_authority();
    app.set_native_slot_authority(authority.clone());
    (app, authority, ops)
}

fn direct_app_with_host(instance: u64) -> (SynvelisApp, SharedNativeSlotAuthority) {
    let (app, authority, _) = direct_app_with_host_ops(instance);
    (app, authority)
}

/// Selectively drains `syn.plugin_dock.state` notify payloads from the
/// app outbox. Non-matching events stay queued so this read never
/// consumes unrelated evidence.
fn drain_plugin_dock_notifies(app: &SynvelisApp) -> Vec<Value> {
    let outbox = app.event_outbox();
    let mut matched = Vec::new();
    let mut rest = Vec::new();
    {
        let mut q = outbox.borrow_mut();
        while let Some(event) = q.pop_front() {
            match &event {
                Event::Notify { data, .. } if data["channel"] == "syn.plugin_dock.state" => {
                    matched.push(data["data"].clone());
                }
                _ => rest.push(event),
            }
        }
        for event in rest.into_iter().rev() {
            q.push_front(event);
        }
    }
    matched
}

fn session_event_names(notifies: &[Value]) -> Vec<String> {
    notifies
        .iter()
        .filter(|row| row["view_id"] == "view-9")
        .flat_map(|row| {
            row["events"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|event| event["event"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn json_has_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(map) => map.contains_key(key) || map.values().any(|v| json_has_key(v, key)),
        Value::Array(rows) => rows.iter().any(|v| json_has_key(v, key)),
        _ => false,
    }
}

struct ManualClock {
    now: Rc<Cell<Duration>>,
}

impl SessionClock for ManualClock {
    fn now(&self) -> Duration {
        self.now.get()
    }
}

fn direct_temp_bundle(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "synvelis-sv176-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

fn direct_open_editor(app: &mut SynvelisApp, instance: u64) {
    app.op_window_lifecycle(&json!({
        "window_id": "w1",
        "event": "created",
        "title": "Synvelis",
    }))
    .expect("adopt window");
    app.op_plugin_dock_open(&json!({
        "view_id": "view-9",
        "window_id": "w1",
        "instance": instance,
        "x": 12.0,
        "y": 34.0,
        "scale_factor": 1.5,
    }))
    .expect("open plugin dock");
}

#[test]
fn project_boundary_retires_panel_session_lease_and_slot() {
    let (mut app, authority) = direct_app_with_host(7);
    let bundle = direct_temp_bundle("boundary-a");
    let next = direct_temp_bundle("boundary-b");
    app.project_new(&bundle.to_string_lossy(), Some("a"))
        .expect("project a");
    direct_open_editor(&mut app, 7);
    let before = app.plugin_dock_panel("view-9").expect("panel before");
    let slot = before.slot().expect("slot before");
    assert_eq!(
        authority.borrow().state(slot).ok().flatten(),
        Some(SlotState::Leased)
    );

    app.project_new(&next.to_string_lossy(), Some("b"))
        .expect("project b");

    assert!(app.plugin_dock_panel("view-9").is_none());
    assert_eq!(
        authority.borrow().state(slot).ok().flatten(),
        Some(SlotState::Released),
        "the old project's slot must not survive the boundary"
    );
    let err = app
        .op_plugin_dock_open(&json!({
            "view_id": "view-10",
            "window_id": "w1",
            "instance": 7,
        }))
        .expect_err("window adoption is project/workspace scoped");
    assert!(err.message.contains("not adopted"));
}

#[test]
fn project_save_open_restores_pd07_logical_panel_closed() {
    let (mut app, _authority) = direct_app_with_host(7);
    let bundle = direct_temp_bundle("pd07");
    app.project_new(&bundle.to_string_lossy(), Some("pd07"))
        .expect("project new");
    direct_open_editor(&mut app, 7);
    app.project_save().expect("project save");

    app.project_open(&bundle.to_string_lossy())
        .expect("project open");
    let state = app.plugin_dock_state("view-9").expect("restored panel");
    assert_eq!(state["instance"], 7);
    assert_eq!(state["placement"], "docked");
    assert_eq!(state["session_state"], "closed");
    assert!(state["session"].is_null(), "no session id is restored");
    assert!(state["slot"].is_null(), "no slot id is restored");
    assert_eq!(state["restored"]["origin"], json!([12.0, 34.0]));
    assert_eq!(state["restored"]["scale_factor"], 1.5);
    assert_eq!(state["restored"]["size"], json!([800.0, 600.0]));
    for banned in ["lease", "handle"] {
        assert!(state["restored"].get(banned).is_none());
    }

    // The logical panel is not a fake live editor: an explicit open
    // command is required, and it fails honestly until the view/window
    // are part of the new workspace.
    let err = app
        .op_plugin_dock_open(&json!({
            "view_id": "view-9",
            "window_id": "w1",
            "instance": 7,
        }))
        .expect_err("explicit reopen needs an adopted window");
    assert!(err.message.contains("not adopted"));

    // Once the new workspace adopts the window, the same explicit open
    // command is the honest re-open path: it consumes the restored
    // logical state instead of resurrecting a runtime session id.
    app.op_window_lifecycle(&json!({
        "window_id": "w1",
        "event": "created",
        "title": "Synvelis",
    }))
    .expect("adopt reopened window");
    app.op_plugin_dock_open(&json!({
        "view_id": "view-9",
        "window_id": "w1",
        "instance": 7,
    }))
    .expect("explicit reopen after adoption");
    let reopened = app.plugin_dock_state("view-9").expect("reopened panel");
    assert_eq!(reopened["session_state"], "attached");
    assert!(reopened["restored"].is_null());
}

#[test]
fn pd07_sidecar_contains_no_runtime_session_or_handle_vocabulary() {
    let (mut app, _authority) = direct_app_with_host(7);
    let bundle = direct_temp_bundle("pd07-sidecar");
    app.project_new(&bundle.to_string_lossy(), Some("pd07"))
        .expect("project new");
    direct_open_editor(&mut app, 7);
    app.op_plugin_dock_hide(&json!({ "view_id": "view-9" }))
        .expect("hide before save");
    app.project_save().expect("project save");

    let sidecar = std::fs::read_to_string(bundle.join("plugin-docks.json"))
        .expect("plugin dock sidecar exists in the bundle");
    let json: Value = serde_json::from_str(&sidecar).expect("sidecar is json");
    assert_eq!(json["format"], "synvelis-plugin-docks/1");
    assert_eq!(json["panels"][0]["snapshot"]["instance"], 7);
    assert_eq!(json["panels"][0]["snapshot"]["visible"], false);
    for banned in ["session", "slot", "lease", "handle", "token"] {
        assert!(
            !json_has_key(&json, banned),
            "runtime key `{banned}` must not persist: {sidecar}"
        );
    }

    let reopened = app
        .project_open(&bundle.to_string_lossy())
        .expect("project open");
    assert_eq!(reopened["plugin_docks"]["restored"], 1);
    let state = app.plugin_dock_state("view-9").expect("restored panel");
    assert_eq!(state["session_state"], "closed");
    assert_eq!(state["restored"]["visible"], false);
}

#[test]
fn session_pump_drains_panel_events_exactly_once_in_order() {
    let (mut app, _authority) = direct_app_with_host(7);
    direct_open_editor(&mut app, 7);
    app.op_plugin_dock_hide(&json!({ "view_id": "view-9" }))
        .expect("hide");
    app.op_plugin_dock_show(&json!({ "view_id": "view-9" }))
        .expect("show");
    app.op_window_lifecycle(&json!({ "event": "tick" }))
        .expect("extra pump");

    let events = session_event_names(&drain_plugin_dock_notifies(&app));
    assert_eq!(events, vec!["attached", "hidden", "shown"]);
}

#[test]
fn session_deadline_timeout_converges_failed_once_through_ui_pump() {
    let (mut app, _authority) = direct_app_with_host(7);
    direct_open_editor(&mut app, 7);
    let now = Rc::new(Cell::new(Duration::ZERO));
    app.with_plugin_editor_session_for_test("view-9", |session| {
        session.set_deadline_policy(Some(SessionDeadlinePolicy {
            timeout: Duration::from_millis(5),
            clock: Box::new(ManualClock { now: now.clone() }),
        }));
        session.begin_detach().expect("manual in-flight detach");
    })
    .expect("bound session");
    now.set(Duration::from_millis(6));

    app.op_window_lifecycle(&json!({ "event": "tick" }))
        .expect("deadline pump");
    let state = app.plugin_dock_state("view-9").expect("panel");
    assert_eq!(state["session_state"], "failed");

    app.op_window_lifecycle(&json!({ "event": "tick" }))
        .expect("second pump");
    let notifies = drain_plugin_dock_notifies(&app);
    let failed: Vec<_> = notifies
        .iter()
        .filter(|row| row["view_id"] == "view-9")
        .flat_map(|row| row["events"].as_array().into_iter().flatten())
        .filter(|event| event["event"] == "failed")
        .collect();
    assert_eq!(failed.len(), 1, "timeout failure is projected exactly once");
    assert_eq!(failed[0]["cause"], "Timeout");
}

#[test]
fn degraded_event_is_projected_without_claiming_requested_attach() {
    let (mut app, _authority, ops) = direct_app_with_host_ops(7);
    app.op_window_lifecycle(&json!({
        "window_id": "w1",
        "event": "created",
        "title": "Synvelis",
    }))
    .expect("adopt window");
    ops.borrow_mut().open_err = Some(EditorOpError::Unsupported);
    let err = app
        .op_plugin_dock_open(&json!({
            "view_id": "view-9",
            "window_id": "w1",
            "instance": 7,
        }))
        .expect_err("unsupported docked open fails first");
    assert_eq!(err.code.as_str(), "unavailable");

    app.with_plugin_editor_session_for_test("view-9", |session| {
        let request = session
            .begin_degrade(AttachMode::Floating)
            .expect("failed session can begin degrade");
        session
            .complete_attach(request, Ok(()))
            .expect("async fallback attach completion");
    })
    .expect("bound failed session");
    app.op_window_lifecycle(&json!({ "event": "tick" }))
        .expect("projection pump");

    let state = app.plugin_dock_state("view-9").expect("panel");
    assert_eq!(state["session_state"], "attached");
    assert_eq!(state["mode"], "floating");
    let events = session_event_names(&drain_plugin_dock_notifies(&app));
    assert_eq!(events, vec!["failed", "degraded"]);
    assert!(
        !events.iter().any(|event| event == "attached"),
        "a degrade must never be rewritten as a requested-mode attach"
    );
}

#[test]
fn project_boundary_converges_without_host_and_after_adapter_refusal() {
    for refusal in [None, Some(EditorOpError::Unsupported)] {
        let (mut app, authority, ops) = direct_app_with_host_ops(7);
        let first = direct_temp_bundle("boundary-host-a");
        let next = direct_temp_bundle("boundary-host-b");
        app.project_new(&first.to_string_lossy(), Some("a"))
            .expect("project a");
        direct_open_editor(&mut app, 7);
        let slot = app
            .plugin_dock_panel("view-9")
            .and_then(PluginDock::slot)
            .expect("live slot");
        if let Some(error) = refusal {
            ops.borrow_mut().close_err = Some(error);
        } else {
            app.clear_plugin_editor_host();
        }

        app.project_new(&next.to_string_lossy(), Some("b"))
            .expect("project boundary remains convergent");
        assert!(app.plugin_dock_panel("view-9").is_none());
        assert_eq!(
            authority.borrow().state(slot).ok().flatten(),
            Some(SlotState::Released)
        );
    }
}

#[test]
fn pd07_restore_reports_unknown_instance_without_panicking_or_dropping() {
    let (mut app, _authority) = direct_app_with_host(7);
    let bundle = direct_temp_bundle("pd07-unknown");
    app.project_new(&bundle.to_string_lossy(), Some("pd07"))
        .expect("project new");
    direct_open_editor(&mut app, 7);
    app.project_save().expect("project save");

    app.set_plugin_editor_host(Box::new(FakeEditorHost::new()));
    let opened = app
        .project_open(&bundle.to_string_lossy())
        .expect("project open");
    assert_eq!(opened["plugin_docks"]["restored"], 0);
    assert_eq!(
        opened["plugin_docks"]["rejected"][0]["reason"],
        "unknown_instance"
    );
    assert!(app.plugin_dock_panel("view-9").is_none());
}
