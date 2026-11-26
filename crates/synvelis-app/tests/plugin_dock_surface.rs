//! synvelis#176 (SV-GPUI-02) slice S2 — shared-authority invariants
//! for the `PluginDock` slot/lease seam.
//!
//! Deterministic regression tests for the authority asymmetry the S2
//! refactor exists to guarantee: every panel is lent ONE injected
//! [`NativeSlotAuthority`] ledger, so a lease is validated by the same
//! authority that minted it — never by a panel-local ledger, never by
//! a foreign one.
//!
//! **Headless/test-only evidence**: the authority here is the upstream
//! `FakeNativeSlot` fixture (no OS window, no real native parent, no
//! plugin binary). These tests pin ledger/generation semantics only;
//! they are never evidence of real native docking — that stays with
//! the OS-backed lanes (`evidence/sv176/`).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use ruviform_plugin_core::{ParentSlotLease, PluginEditorSessionId, PluginHandle};
use ruviform_plugin_host::{
    AttachMode, EditorCapabilities, EditorOpError, EditorSessionOps, EditorSize, LeaseRedeemer,
    PluginEditorSessions,
};
use ruviform_ui::WindowId;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::native_slot::{
    LeaseInfo, LogicalRect, NativeSurfaceSlot, SlotCapabilities, SlotError, SlotEvent,
    SlotGeometry, SlotLease, SlotSpec, SlotState, SlotVisibility, SurfaceSlotId,
};
use ruviform_ui_test::FakeNativeSlot;

use synvelis_app::plugin_dock::{NativeSlotAuthority, SharedNativeSlotAuthority};
use synvelis_app::{PluginDock, PluginDockError, PluginEditorHost};
use synvelis_player::StablePluginInstanceId;

// ---------------------------------------------------------------------------
// Shared test authority fixture (headless `FakeNativeSlot` wrapper)
// ---------------------------------------------------------------------------

/// High tag bits of the synthetic parent tokens the test authority
/// mints at redemption. **Test-only vocabulary** — obviously not an
/// OS handle; it exists so tests can prove which slot a redeemed
/// lease belonged to.
const TEST_PARENT_TOKEN_TAG: u64 = 0xFA17_E000_0000_0000;

fn test_parent_token(seq: u32) -> u64 {
    TEST_PARENT_TOKEN_TAG | u64::from(seq)
}

/// The same fixture shape `tests/plugin_dock.rs` uses, duplicated
/// here because integration-test binaries cannot share private
/// helpers: upstream [`FakeNativeSlot`] plus spec mirroring and
/// issuer-side redemption against the same ledger.
///
/// **Headless/test-only** — no OS window, no real native parent.
struct TestSlotAuthority {
    inner: FakeNativeSlot,
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
        let lease = SlotLease::new(payload);
        self.inner.validate_lease(lease)?;
        Ok(test_parent_token(lease.seq()))
    }
}

// ---------------------------------------------------------------------------
// Minimal scripted host — enough to drive `PluginDock::open`
// ---------------------------------------------------------------------------

/// Redeems through the panel-supplied issuer hook, exactly like a
/// real driver's adapter — a rejected lease fails the op.
struct FakeOpsAdapter<'a> {
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
        _session: PluginEditorSessionId,
        lease: &ParentSlotLease,
        _size: EditorSize,
        _mode: AttachMode,
    ) -> Result<EditorCapabilities, EditorOpError> {
        (self.redeem)(lease)?;
        Ok(EditorCapabilities::default())
    }

    fn close(&mut self, _session: PluginEditorSessionId) -> Result<(), EditorOpError> {
        Ok(())
    }

    fn set_size(
        &mut self,
        _session: PluginEditorSessionId,
        _size: EditorSize,
    ) -> Result<(), EditorOpError> {
        Ok(())
    }

    fn set_visibility(
        &mut self,
        _session: PluginEditorSessionId,
        _visible: bool,
    ) -> Result<(), EditorOpError> {
        Ok(())
    }

    fn reattach(
        &mut self,
        _session: PluginEditorSessionId,
        lease: &ParentSlotLease,
    ) -> Result<(), EditorOpError> {
        (self.redeem)(lease)?;
        Ok(())
    }

    fn set_focus(
        &mut self,
        _session: PluginEditorSessionId,
        _focused: bool,
    ) -> Result<(), EditorOpError> {
        Ok(())
    }

    fn send_key(
        &mut self,
        _session: PluginEditorSessionId,
        _event: ruviform_plugin_core::GuiKeyEvent,
    ) -> Result<(), EditorOpError> {
        Ok(())
    }
}

struct FakeEditorHost {
    handle: PluginHandle,
}

impl FakeEditorHost {
    fn new() -> Self {
        Self {
            handle: PluginHandle::new(0, 1),
        }
    }
}

impl PluginEditorHost for FakeEditorHost {
    fn resolve(&mut self, _instance: StablePluginInstanceId) -> Option<PluginHandle> {
        Some(self.handle)
    }

    fn ops<'a>(
        &'a mut self,
        _handle: PluginHandle,
        redeem: Box<LeaseRedeemer<'a>>,
    ) -> Result<Box<dyn EditorSessionOps + 'a>, EditorOpError> {
        Ok(Box::new(FakeOpsAdapter { redeem }))
    }
}

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

fn view(n: u64) -> ViewInstanceId {
    ViewInstanceId::new(n).expect("view id non-zero")
}

fn win(n: u64) -> WindowId {
    WindowId::new(n).expect("window id non-zero")
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

fn open_editor(
    dock: &mut PluginDock,
    sessions: &mut PluginEditorSessions,
    host: &mut FakeEditorHost,
) {
    dock.open(
        sessions,
        host,
        StablePluginInstanceId::new(7),
        size(),
        AttachMode::Docked,
    )
    .expect("open editor");
}

// ---------------------------------------------------------------------------
// (a) One shared authority — not a panel-local ledger
// ---------------------------------------------------------------------------

/// Two panels lent the same authority: a lease minted through panel
/// A's `open` validates through the shared authority — and a second,
/// independent authority (what a panel-local ledger would be)
/// rejects the very same payload as foreign. This is the authority
/// asymmetry the S2 refactor pins: validation always happens against
/// the ledger that minted the lease, never a panel-private one.
#[test]
fn lease_validates_through_the_shared_authority_only() {
    let authority = test_authority();
    let mut sessions = PluginEditorSessions::new();
    let mut host = FakeEditorHost::new();
    let mut a = PluginDock::declare(view(9), win(1), geom(), authority.clone()).unwrap();
    let b = PluginDock::declare(view(10), win(1), geom(), authority.clone()).unwrap();

    open_editor(&mut a, &mut sessions, &mut host);
    let lease = a.binding().unwrap().lease().unwrap();

    // The shared authority validates the lease it minted and redeems
    // it to the slot's (synthetic, test-only) parent token.
    authority
        .borrow()
        .validate_lease(lease)
        .expect("live lease validates in the minting authority");
    let token = authority
        .borrow()
        .redeem_parent(lease.raw())
        .expect("redeem through the same authority");
    assert_eq!(token, test_parent_token(SlotLease::new(lease.raw()).seq()));

    // A second authority instance — the shape a panel-private ledger
    // would be — never minted this payload: validation and
    // redemption both refuse it as foreign.
    let foreign = test_authority();
    assert!(
        foreign.borrow().validate_lease(lease).is_err(),
        "a foreign authority must reject the lease"
    );
    assert!(
        foreign.borrow().redeem_parent(lease.raw()).is_err(),
        "a foreign authority must not yield a parent token"
    );

    // Panel B, lent the SAME authority, declared its slot in the same
    // ledger — the two panels share one slot-index namespace, which
    // is itself proof there is no per-panel ledger.
    assert_ne!(
        a.slot().unwrap(),
        b.slot().unwrap(),
        "two panels declare distinct slots in the shared ledger"
    );
    authority
        .borrow()
        .validate_lease(lease)
        .expect("same authority, cross-panel visibility");
}

// ---------------------------------------------------------------------------
// (b) Released/revoked leases are rejected
// ---------------------------------------------------------------------------

/// After `close` revokes the lease, the same payload no longer
/// validates and no longer redeems — a revoked parent seat is never
/// reinterpreted.
#[test]
fn released_lease_is_rejected_at_validate_and_redeem() {
    let authority = test_authority();
    let mut sessions = PluginEditorSessions::new();
    let mut host = FakeEditorHost::new();
    let mut dock = PluginDock::declare(view(9), win(1), geom(), authority.clone()).unwrap();

    open_editor(&mut dock, &mut sessions, &mut host);
    let lease = dock.binding().unwrap().lease().unwrap();
    authority
        .borrow()
        .validate_lease(lease)
        .expect("live before close");

    dock.close(&mut sessions, Some(&mut host)).expect("close");

    assert!(
        authority.borrow().validate_lease(lease).is_err(),
        "revoked lease must not validate"
    );
    assert!(
        authority.borrow().redeem_parent(lease.raw()).is_err(),
        "revoked lease must not redeem"
    );
}

/// Double release is itself a refusal — the first release retires
/// the lease, and a second release of the same payload is an error,
/// never a silent no-op (no double-free of the parent seat).
#[test]
fn double_release_is_refused() {
    let authority = test_authority();
    let mut sessions = PluginEditorSessions::new();
    let mut host = FakeEditorHost::new();
    let mut dock = PluginDock::declare(view(9), win(1), geom(), authority.clone()).unwrap();
    open_editor(&mut dock, &mut sessions, &mut host);
    let lease = dock.binding().unwrap().lease().unwrap();

    authority
        .borrow_mut()
        .release_lease(lease)
        .expect("first release retires the lease");
    assert!(
        authority.borrow_mut().release_lease(lease).is_err(),
        "second release of the same lease must refuse"
    );
}

// ---------------------------------------------------------------------------
// (c) Index recycling — stale generation and stale leases die
// ---------------------------------------------------------------------------

/// Releasing a slot frees its index for recycling under a bumped
/// generation: the recycled index must not resurrect the old
/// generation's lease, and the old slot id is stale at the ledger.
/// Exercised through `PluginDock`'s window-loss + re-declare path
/// (the product flow that recycles indices) with direct authority
/// assertions as the independent oracle.
#[test]
fn recycled_index_rejects_stale_generation_and_lease() {
    let authority = test_authority();
    let mut sessions = PluginEditorSessions::new();
    let mut host = FakeEditorHost::new();
    let mut dock = PluginDock::declare(view(9), win(1), geom(), authority.clone()).unwrap();

    open_editor(&mut dock, &mut sessions, &mut host);
    let old_slot = dock.slot().unwrap();
    let old_lease = dock.binding().unwrap().lease().unwrap();

    // Window loss releases the slot (and its lease) — the index is
    // now recyclable.
    dock.notify_window_lost(&mut sessions);
    assert_eq!(
        authority.borrow().state(old_slot).ok().flatten(),
        Some(SlotState::Released)
    );

    // Re-dock on a new window re-declares — the registry recycles
    // the freed index under a bumped generation.
    dock.relocate(&mut sessions, Some(&mut host), win(2))
        .expect("re-declare on new window");
    let new_slot = dock.slot().unwrap();
    assert_ne!(new_slot, old_slot);
    if new_slot.index() == old_slot.index() {
        assert_ne!(
            new_slot.generation(),
            old_slot.generation(),
            "recycled index must carry a bumped generation"
        );
    }

    // The pre-loss lease is dead — recycling the index must not
    // resurrect it — and the old generation's id is stale for every
    // ledger operation.
    assert!(
        authority.borrow().validate_lease(old_lease).is_err(),
        "stale lease must stay rejected after index recycling"
    );
    assert!(
        authority.borrow().redeem_parent(old_lease.raw()).is_err(),
        "stale lease must not redeem after index recycling"
    );
    assert!(
        authority
            .borrow_mut()
            .set_visibility(old_slot, SlotVisibility::Hidden)
            .is_err(),
        "the old generation's slot id is stale"
    );
}

// ---------------------------------------------------------------------------
// (d) Busy mark refuses new leases until cleared
// ---------------------------------------------------------------------------

/// `prepare_move` marks the slot busy: `open` cannot mint a lease and
/// refuses honestly (retiring the half-minted session — no binding);
/// `abort_move` clears the mark and `open` succeeds again. Direct
/// authority assertions pin the ledger state as the oracle.
#[test]
fn busy_slot_refuses_lease_until_cleared() {
    let authority = test_authority();
    let mut sessions = PluginEditorSessions::new();
    let mut host = FakeEditorHost::new();
    let mut dock = PluginDock::declare(view(9), win(1), geom(), authority.clone()).unwrap();
    let slot = dock.slot().unwrap();

    dock.prepare_move().expect("prepare move");
    assert!(
        authority.borrow_mut().lease_parent(slot).is_err(),
        "busy slot refuses a direct lease mint"
    );
    let err = dock
        .open(
            &mut sessions,
            &mut host,
            StablePluginInstanceId::new(7),
            size(),
            AttachMode::Docked,
        )
        .expect_err("open on a busy slot must refuse");
    assert!(
        matches!(err, PluginDockError::Slot(_)),
        "expected a slot refusal, got {err}"
    );
    assert!(
        dock.binding().is_none(),
        "a lease-refused open leaves no half-minted binding"
    );

    dock.abort_move().expect("abort move");
    let lease = authority
        .borrow_mut()
        .lease_parent(slot)
        .expect("cleared slot leases again");
    authority
        .borrow_mut()
        .release_lease(lease)
        .expect("the probe lease releases cleanly");
}

/// Busy-mark cleanup: after `abort_move`, `open` through the panel
/// mints a fresh lease and lands attached — the busy episode leaves
/// no residue.
#[test]
fn open_after_busy_clear_lands_attached() {
    let authority = test_authority();
    let mut sessions = PluginEditorSessions::new();
    let mut host = FakeEditorHost::new();
    let mut dock = PluginDock::declare(view(9), win(1), geom(), authority.clone()).unwrap();

    dock.prepare_move().expect("prepare");
    dock.abort_move().expect("abort");
    open_editor(&mut dock, &mut sessions, &mut host);
    assert_eq!(
        authority
            .borrow()
            .state(dock.slot().unwrap())
            .ok()
            .flatten(),
        Some(SlotState::Leased)
    );
}

// ---------------------------------------------------------------------------
// (e) Forged leases are rejected
// ---------------------------------------------------------------------------

/// A payload no ledger ever minted — hand-packed bits — is forged:
/// validation and redemption both refuse it. The seam is trusted
/// same-process vocabulary, so the test pins the rejection of
/// enumerable-but-never-issued payloads.
#[test]
fn forged_lease_payload_is_rejected() {
    let authority = test_authority();
    let mut sessions = PluginEditorSessions::new();
    let mut host = FakeEditorHost::new();
    let mut dock = PluginDock::declare(view(9), win(1), geom(), authority.clone()).unwrap();
    open_editor(&mut dock, &mut sessions, &mut host);
    let slot = dock.slot().unwrap();

    // Hand-packed forgery: the right index with a sequence the ledger
    // never issued (far above any minted seq).
    let forged = SlotLease::encode_parts(slot.index(), u32::MAX - 1);
    assert!(
        authority.borrow().validate_lease(forged).is_err(),
        "forged lease must not validate"
    );
    assert!(
        authority.borrow().redeem_parent(forged.raw()).is_err(),
        "forged lease must not redeem"
    );

    // Garbage payload not even shaped like a minted lease.
    let garbage = SlotLease::new(0xDEAD_DEAD_DEAD_DEAD);
    assert!(authority.borrow().validate_lease(garbage).is_err());
    assert!(authority.borrow().redeem_parent(garbage.raw()).is_err());

    // The genuine lease still validates — the forgery probes did not
    // disturb the ledger.
    let lease = dock.binding().unwrap().lease().unwrap();
    authority
        .borrow()
        .validate_lease(lease)
        .expect("the genuine lease still validates");
}
