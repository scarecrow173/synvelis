//! Product `PluginDock` panel — driver-neutral view/session model
//! (synvelis#176 SV-GPUI-02 headless slice, acceptance rows 2–3; S2
//! shared-authority refactor).
//!
//! One [`PluginDock`] is the product-side record of a single
//! [`ViewType::PluginDock`](synvelis_workspace::ViewType::PluginDock)
//! dock view hosting at most one plugin editor at a time. It binds the
//! four identity axes the issue requires —
//! [`StablePluginInstanceId`] (project-side instance vocabulary) →
//! [`PluginEditorSessionId`] (host session) → [`SurfaceSlotId`] (native
//! surface slot) → [`ViewInstanceId`] (dock view) — over two shared
//! authorities it is *lent*, never owns:
//!
//! - [`NativeSlotAuthority`] — the single injected slot/lease ledger
//!   behind every panel's `plugin.editor` [`NativeSlotSpec`]. Panels
//!   hold only a clone of the shared handle: slot declare/lease/
//!   release/validate/geometry/visibility/busy/event-drain all go
//!   through the one authority the composition root installed (the
//!   driver's canonical surface ledger in production), so a lease is
//!   always validated by the same ledger that minted it. A slot is
//!   declared against the view's owning window, mints at most one
//!   live [`SlotLease`] at a time, and recycles freed indices under a
//!   bumped generation so a stale lease is always rejected.
//! - [`PluginEditorSessions`] — the editor-session state machine
//!   (`Closed → Creating → Attached/Hidden → Closing → Closed`,
//!   `Failed`, `Terminated`) driven through the [`EditorSessionOps`]
//!   port — never an OS handle, never `gpui`.
//!
//! The adapter seam is [`PluginEditorHost`]: the composition root (or
//! a test fixture) injects an implementation that resolves a stable
//! instance identity to a live [`PluginHandle`] and lends an
//! [`EditorSessionOps`] adapter per call, redeeming opaque
//! [`ParentSlotLease`]s through the issuer-side
//! [`redeem_lease`] hook — which validates against the panel's shared
//! authority, the same seam a real GUI driver's ledger validates
//! before yielding an OS parent. With no host or no authority
//! installed the op layer reports `unavailable` — an honest refusal,
//! never a simulated editor.
//!
//! Dock moves connect to the existing `syn.window.dock_*` two-phase
//! contract instead of re-implementing it: the op layer calls
//! [`PluginDock::prepare_move`] on prepare (the slot refuses new leases
//! while busy), [`PluginDock::relocate`] on commit/reattach (cross-
//! window moves re-parent through `ops.reattach` under a freshly
//! minted lease), and [`PluginDock::abort_move`] on abort.
//!
//! **RT contract:** non-RT only — everything here is UI-thread state.
//! Editor sessions never touch the DSP engine, project session, or
//! any RT path; the `SessionFailCause` vocabulary exists precisely so
//! a dead editor leaves the audio instance alive.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use ruviform_plugin_core::{
    GuiKeyEvent, ParentLease, ParentSlotLease, PluginEditorSessionId, PluginHandle,
};
use ruviform_plugin_host::{
    AttachMode, EditorCapabilities, EditorOpError, EditorSessionError, EditorSessionOps,
    EditorSessionState, EditorSize, LeaseRedeemer, PluginEditorSession, PluginEditorSessions,
    SessionFailCause,
};
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::native_slot::{
    LogicalRect, NativeSurfaceSlot, SlotCapabilities, SlotError, SlotGeometry, SlotLease, SlotSpec,
    SlotState, SlotVisibility, Support, SurfaceSlotId,
};
use ruviform_ui::view::{
    A11ySpec, AttachEpochAllocator, ComponentKey, ComponentSpec, ComponentVisualState, LabelSpec,
    NativeSlotSpec, PanelSpec, ViewHost, ViewSpec,
};
use ruviform_ui::{UiError, WindowId};
use serde::{Deserialize, Serialize};
use synvelis_player::StablePluginInstanceId;

/// Root panel [`ComponentKey`] — the vocabulary the upstream
/// `plugin_dock_view_spec` contract test pins (`plugin.dock`).
pub const PLUGIN_DOCK_PANEL_KEY: &str = "plugin.dock";

/// [`ComponentKey`] of the panel's native-slot region — identical to
/// upstream `PLUGIN_DOCK_SLOT_KEY` (`plugin.editor`); the driver binds
/// `key → SurfaceSlotId` under this name.
pub const PLUGIN_DOCK_SLOT_KEY: &str = "plugin.editor";

/// [`ComponentKey`] of the panel's status line — a product addition
/// the upstream declaration-only spec does not carry.
pub const PLUGIN_DOCK_STATUS_KEY: &str = "plugin.dock.status";

/// Capabilities the product's `plugin.editor` slot honestly declares.
///
/// The headless model performs attach/re-parent/cross-window/geometry/
/// hide-show bookkeeping itself, so those flags are `Supported`; every
/// capability the pure model does not perform (focus routing, keyboard,
/// IME, popup, clip, z-order, out-of-process parenting) stays
/// `Unsupported` — a driver may upgrade them, the model never fakes them.
const PANEL_SLOT_CAPABILITIES: SlotCapabilities = SlotCapabilities {
    attach_parent: Support::Supported,
    reparent: Support::Supported,
    cross_window_move: Support::Supported,
    resize_negotiate: Support::Supported,
    hide_show: Support::Supported,
    focus: Support::Unsupported,
    keyboard_input: Support::Unsupported,
    ime: Support::Unsupported,
    popup: Support::Unsupported,
    clip: Support::Unsupported,
    z_order: Support::Unsupported,
    out_of_process_parent: Support::Unsupported,
};

/// The attach mode the user asked for — persisted vocabulary.
///
/// Upstream [`AttachMode`] is `#[non_exhaustive]` and not serde, so the
/// logical save/restore record (PD-07: no OS handle is ever persisted)
/// keeps this product enum instead.
///
/// **RT contract:** non-RT only — UI-thread only.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginDockPlacement {
    /// Editor docked inside the panel's `plugin.editor` slot.
    #[default]
    Docked,
    /// Editor in a host-owned floating window (explicit fallback —
    /// never counted as a dock pass).
    Floating,
}

impl PluginDockPlacement {
    /// The upstream session vocabulary for this placement.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub const fn to_attach_mode(self) -> AttachMode {
        match self {
            Self::Docked => AttachMode::Docked,
            Self::Floating => AttachMode::Floating,
        }
    }

    /// Parses `"docked"`/`"floating"` (op params).
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "docked" => Some(Self::Docked),
            "floating" => Some(Self::Floating),
            _ => None,
        }
    }

    /// `"docked"`/`"floating"` — the wire name.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Docked => "docked",
            Self::Floating => "floating",
        }
    }

    /// Maps an upstream [`AttachMode`] back onto the product
    /// vocabulary; future upstream modes degrade to `Floating` (the
    /// non-dock seat).
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub fn from_attach_mode(mode: AttachMode) -> Self {
        match mode {
            AttachMode::Docked => Self::Docked,
            _ => Self::Floating,
        }
    }
}

/// The logical, persistable state of a panel's editor — the PD-07
/// record. Carries the stable instance identity, requested placement,
/// logical size and visibility; no session id, slot id, lease or OS
/// handle survives a restore.
///
/// **RT contract:** non-RT only — UI-thread only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginDockSnapshot {
    /// [`StablePluginInstanceId::get`] of the hosted instance.
    pub instance: u64,
    /// Requested attach placement.
    pub placement: PluginDockPlacement,
    /// `[width, height]` logical editor size.
    pub size: [f64; 2],
    /// Logical editor origin inside the panel (`[x, y]`). Defaults to
    /// the panel origin for snapshots written before geometry persistence.
    #[serde(default)]
    pub origin: [f64; 2],
    /// Logical scale factor recorded with the geometry.
    #[serde(default = "default_snapshot_scale_factor")]
    pub scale_factor: f64,
    /// Whether the editor was visible when saved.
    pub visible: bool,
}

fn default_snapshot_scale_factor() -> f64 {
    1.0
}

/// The live four-identity binding of an occupied panel.
///
/// `instance` is the project-side stable identity; `handle` is the
/// host-runtime handle the session opened against; `session`/`slot`/
/// `view` are the three runtime ids the issue's row 3 requires to be
/// owned and generation-checked together.
///
/// **RT contract:** non-RT only — UI-thread only.
#[derive(Copy, Clone, Debug)]
pub struct PluginDockBinding {
    /// Project-side plugin instance identity.
    pub instance: StablePluginInstanceId,
    /// Live host handle the session opened against.
    pub handle: PluginHandle,
    /// Live (or terminally recorded) editor session id.
    pub session: PluginEditorSessionId,
    /// Slot the session was last parented under.
    pub slot: SurfaceSlotId,
    /// The dock view this panel is.
    pub view: ViewInstanceId,
    /// Requested placement (persisted intent, not adapter-reported).
    pub placement: PluginDockPlacement,
    /// Last negotiated logical editor size.
    pub size: EditorSize,
    /// The slot's live lease while an editor may still be parented;
    /// `None` once the lease was revoked (termination, window loss).
    lease: Option<SlotLease>,
}

impl PluginDockBinding {
    /// The live parent lease, if still held.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub const fn lease(&self) -> Option<SlotLease> {
        self.lease
    }
}

/// What [`PluginDock::open`] minted — the full identity tuple plus the
/// adapter-reported capabilities of the attach.
///
/// **RT contract:** non-RT only — UI-thread only.
#[derive(Copy, Clone, Debug)]
pub struct PluginDockReport {
    /// Dock view hosting the panel.
    pub view: ViewInstanceId,
    /// Slot the editor is parented under.
    pub slot: SurfaceSlotId,
    /// Minted editor session.
    pub session: PluginEditorSessionId,
    /// Hosted instance identity.
    pub instance: StablePluginInstanceId,
    /// Mode the session landed in.
    pub mode: AttachMode,
    /// Adapter-reported capabilities.
    pub capabilities: EditorCapabilities,
}

/// Read-model projection of one panel — what the `state` op serializes
/// and [`plugin_dock_screen_spec`] renders.
///
/// **RT contract:** non-RT only — UI-thread only.
#[derive(Clone, Debug)]
pub struct PluginDockStatus {
    /// The panel's dock view.
    pub view: ViewInstanceId,
    /// Window the slot is declared in (`None` after window loss).
    pub window: Option<WindowId>,
    /// The live `plugin.editor` slot id.
    pub slot: Option<SurfaceSlotId>,
    /// Ledger state of the slot (`None` when no slot is declared).
    pub slot_state: Option<SlotState>,
    /// Hosted instance identity, if any binding exists.
    pub instance: Option<StablePluginInstanceId>,
    /// Editor session id, if a binding exists.
    pub session: Option<PluginEditorSessionId>,
    /// Upstream session state (`Attached`/`Hidden`/`Failed`/…).
    pub session_state: Option<EditorSessionState>,
    /// Requested placement of the current binding.
    pub placement: Option<PluginDockPlacement>,
    /// Last negotiated editor size.
    pub size: Option<EditorSize>,
}

/// Errors returned by [`PluginDock`] commands.
///
/// The enum is `#[non_exhaustive]` per the unified public-enum API
/// policy.
///
/// **RT contract:** non-RT only — UI-thread only.
#[non_exhaustive]
#[derive(Debug)]
pub enum PluginDockError {
    /// The command needs a live binding (`hide`/`focus`/`close`/`save`
    /// on an empty panel).
    NoEditor,
    /// The panel already hosts an editor — `close` or `restore` first.
    Occupied,
    /// The panel's surface slot is gone (window lost or never declared)
    /// — `ensure_slot`/`relocate` re-materializes it.
    NoSlot,
    /// The stable instance identity resolved to no live host handle.
    UnknownInstance,
    /// The surface-slot ledger declined.
    Slot(SlotError),
    /// The editor-session layer declined.
    Session(EditorSessionError),
    /// The ops adapter declined before a session call was made.
    Ops(EditorOpError),
}

impl fmt::Display for PluginDockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEditor => f.write_str("panel hosts no editor"),
            Self::Occupied => f.write_str("panel already hosts an editor"),
            Self::NoSlot => f.write_str("panel has no declared surface slot"),
            Self::UnknownInstance => f.write_str("instance has no live host handle"),
            Self::Slot(e) => write!(f, "surface slot: {e}"),
            Self::Session(e) => write!(f, "editor session: {e}"),
            Self::Ops(e) => write!(f, "editor ops adapter: {e}"),
        }
    }
}

impl std::error::Error for PluginDockError {}

impl From<SlotError> for PluginDockError {
    fn from(e: SlotError) -> Self {
        Self::Slot(e)
    }
}

impl From<EditorSessionError> for PluginDockError {
    fn from(e: EditorSessionError) -> Self {
        Self::Session(e)
    }
}

impl From<EditorOpError> for PluginDockError {
    fn from(e: EditorOpError) -> Self {
        Self::Ops(e)
    }
}

/// The driver-neutral slot/lease authority seam (synvelis#176 S2):
/// one shared ledger every `PluginDock` panel is *lent* — the panel
/// never owns a private registry. The trait extends the upstream
/// [`NativeSurfaceSlot`] port with the product surface the panel and
/// the lease-redemption path need:
///
/// - [`NativeSlotAuthority::state`]/[`NativeSlotAuthority::spec`] —
///   ledger reads the port does not carry. `Err` means the ledger
///   could not be consulted this call (a driver-internal borrow
///   refusal), `Ok(None)` means the id is unknown — the two are never
///   conflated.
/// - [`NativeSlotAuthority::mark_busy`]/[`NativeSlotAuthority::clear_busy`]
///   — the dock-move two-phase lease guard.
/// - [`NativeSlotAuthority::redeem_parent`] — the issuer-side
///   redemption step: validate the raw opaque lease payload against
///   *this* ledger and yield the issuer's parent token, or refuse.
///   The token is backend vocabulary (an OS-container token upstream,
///   a clearly synthetic value in test fixtures) — never reinterpreted
///   product-side.
///
/// Implementations are UI-thread only; the shared handle is
/// `Rc<RefCell<dyn ..>>`, matching the driver's own shared-surface
/// shape.
///
/// **RT contract:** non-RT only — UI-thread only.
pub trait NativeSlotAuthority: NativeSurfaceSlot {
    /// The ledger state of `slot` — see the trait docs for the
    /// `Err`/`Ok(None)` distinction.
    ///
    /// # Errors
    /// Returns [`SlotError`] when the ledger cannot be consulted.
    fn state(&self, slot: SurfaceSlotId) -> Result<Option<SlotState>, SlotError>;

    /// The declared spec of `slot` — same `Err`/`Ok(None)`
    /// distinction as [`Self::state`].
    ///
    /// # Errors
    /// Returns [`SlotError`] when the ledger cannot be consulted.
    fn spec(&self, slot: SurfaceSlotId) -> Result<Option<SlotSpec>, SlotError>;

    /// Marks `slot` busy — new leases are refused until
    /// [`Self::clear_busy`] (dock-move in flight).
    ///
    /// # Errors
    /// Returns [`SlotError`] on a bad id.
    fn mark_busy(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError>;

    /// Clears the busy mark.
    ///
    /// # Errors
    /// Returns [`SlotError`] on a bad id.
    fn clear_busy(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError>;

    /// Validates the raw opaque lease payload against this ledger and
    /// returns the issuer-side parent token. A stale, revoked, forged,
    /// or backend-dead lease is always refused — never reinterpreted.
    ///
    /// # Errors
    /// Returns [`SlotError`] on validation failure or backend refusal.
    fn redeem_parent(&self, payload: u64) -> Result<u64, SlotError>;
}

/// The shared authority handle every panel (and the op layer) clones.
/// `Rc<RefCell<..>>` because all implementations are UI-thread
/// interior-mutable ledgers; panel code borrows per call and never
/// holds a guard across an adapter call, so the issuer-side redeem
/// hook can always consult the same ledger re-entrantly.
///
/// **RT contract:** non-RT only — UI-thread only.
pub type SharedNativeSlotAuthority = Rc<RefCell<dyn NativeSlotAuthority>>;

/// Issuer-side half of the lease seam, following the upstream real
/// reference shape (`ruviform-plugin-dock-test`'s `real_editor_ops`):
/// the session-carried [`ParentSlotLease`] is opaque — its raw payload
/// is validated/redeemed against the *same authority that minted it*,
/// and the minted [`ParentLease`] binds the lease's issuer-side
/// sequence (`SlotLease::new(raw).seq()`) to the redeemed parent
/// token. Neither the slot generation nor the slot index is ever
/// mapped into a [`ParentLease`]; a stale/revoked/forged token is
/// always [`EditorOpError::LeaseRejected`].
fn redeem_lease(
    authority: &SharedNativeSlotAuthority,
    lease: ParentSlotLease,
) -> Result<ParentLease, EditorOpError> {
    let raw = lease.raw();
    let token = authority
        .try_borrow()
        .map_err(|_| EditorOpError::LeaseRejected)?
        .redeem_parent(raw)
        .map_err(|_| EditorOpError::LeaseRejected)?;
    Ok(ParentLease::new(SlotLease::new(raw).seq(), token))
}

#[cfg(feature = "native")]
impl NativeSlotAuthority for ruviform_ui_gpui::SharedSurface {
    fn state(&self, slot: SurfaceSlotId) -> Result<Option<SlotState>, SlotError> {
        ruviform_ui_gpui::SharedSurface::state(self, slot)
    }

    fn spec(&self, slot: SurfaceSlotId) -> Result<Option<SlotSpec>, SlotError> {
        ruviform_ui_gpui::SharedSurface::spec(self, slot)
    }

    fn mark_busy(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError> {
        ruviform_ui_gpui::SharedSurface::mark_busy(self, slot)
    }

    fn clear_busy(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError> {
        ruviform_ui_gpui::SharedSurface::clear_busy(self, slot)
    }

    fn redeem_parent(&self, payload: u64) -> Result<u64, SlotError> {
        ruviform_ui_gpui::SharedSurface::redeem(self, payload)
    }
}

/// The composition seam: resolves a stable instance identity to a
/// live host handle and lends an [`EditorSessionOps`] adapter for one
/// command — the same role `InProcessEditorOps::with_redeemer` fills
/// for a real host, plus the project-side `StablePluginInstanceId` →
/// [`PluginHandle`] lookup.
///
/// `redeem` is the issuer-side validator the panel supplies (built on
/// [`redeem_lease`]); the adapter applies it inside `open`/`reattach`
/// before any OS parent is consumed. Implementations are UI-thread
/// only.
///
/// **RT contract:** non-RT only — UI-thread only.
pub trait PluginEditorHost {
    /// Resolves the project-side stable instance identity to its live
    /// host handle; `None` = no such live instance (an honest
    /// `UnknownInstance` refusal, not a fabricated handle).
    fn resolve(&mut self, instance: StablePluginInstanceId) -> Option<PluginHandle>;

    /// Lends the editor-ops adapter for `handle`; `redeem` is the
    /// issuer-side lease validator the panel supplies.
    ///
    /// # Errors
    /// Returns [`EditorOpError`] when the adapter cannot be provided
    /// (e.g. [`L1EditorOps`](ruviform_plugin_host::L1EditorOps) style
    /// defined refusal).
    fn ops<'a>(
        &'a mut self,
        handle: PluginHandle,
        redeem: Box<LeaseRedeemer<'a>>,
    ) -> Result<Box<dyn EditorSessionOps + 'a>, EditorOpError>;
}

/// The injected host object — the alias pins `dyn + 'static` so a
/// `&mut PluginEditorHostObj` parameter accepts the app-owned
/// `Box<dyn PluginEditorHost>`'s deref (a bare `&mut dyn
/// PluginEditorHost` would infer the *reference's* lifetime as the
/// object bound and reject it).
///
/// **RT contract:** non-RT only — UI-thread only.
pub type PluginEditorHostObj = dyn PluginEditorHost;

/// `EditorSessionOps` for the boxed adapter [`PluginEditorHost::ops`]
/// lends — `PluginEditorSession`'s `O: EditorSessionOps` generics
/// cannot name `dyn`, so commands bridge through this.
struct OpsBridge<'a>(Box<dyn EditorSessionOps + 'a>);

impl EditorSessionOps for OpsBridge<'_> {
    fn capabilities(
        &self,
        session: PluginEditorSessionId,
    ) -> Result<EditorCapabilities, EditorOpError> {
        self.0.capabilities(session)
    }

    fn open(
        &mut self,
        session: PluginEditorSessionId,
        lease: &ParentSlotLease,
        size: EditorSize,
        mode: AttachMode,
    ) -> Result<EditorCapabilities, EditorOpError> {
        self.0.open(session, lease, size, mode)
    }

    fn close(&mut self, session: PluginEditorSessionId) -> Result<(), EditorOpError> {
        self.0.close(session)
    }

    fn set_size(
        &mut self,
        session: PluginEditorSessionId,
        size: EditorSize,
    ) -> Result<(), EditorOpError> {
        self.0.set_size(session, size)
    }

    fn set_visibility(
        &mut self,
        session: PluginEditorSessionId,
        visible: bool,
    ) -> Result<(), EditorOpError> {
        self.0.set_visibility(session, visible)
    }

    fn reattach(
        &mut self,
        session: PluginEditorSessionId,
        lease: &ParentSlotLease,
    ) -> Result<(), EditorOpError> {
        self.0.reattach(session, lease)
    }

    fn set_focus(
        &mut self,
        session: PluginEditorSessionId,
        focused: bool,
    ) -> Result<(), EditorOpError> {
        self.0.set_focus(session, focused)
    }

    fn send_key(
        &mut self,
        session: PluginEditorSessionId,
        event: GuiKeyEvent,
    ) -> Result<(), EditorOpError> {
        self.0.send_key(session, event)
    }
}

/// One mounted `PluginDock` panel: a `ViewInstanceId`, its declared
/// `plugin.editor` surface slot, a clone of the shared slot/lease
/// authority handle, and a binding into the app-wide
/// [`PluginEditorSessions`] registry — the deterministic headless
/// state machine the issue's row 2 requires.
///
/// Neither authority is per-panel: the session registry is the
/// host-wide authority that mints unique [`PluginEditorSessionId`]s
/// and enforces "one live editor session per [`PluginHandle`]"
/// ([`EditorSessionError::SessionBusy`]) across panels — callers lend
/// `&mut PluginEditorSessions` to every session-touching method — and
/// the [`NativeSlotAuthority`] handle points at the one shared ledger
/// the composition root installed, so every lease validates against
/// the ledger that minted it regardless of which panel redeems it.
///
/// Invariants enforced (composed, not re-implemented):
/// - at most one live lease per slot and one live session per binding;
/// - stale `{index,generation}` slot ids and revoked leases are always
///   rejected by the shared ledger;
/// - a cross-window [`Self::relocate`] re-parents through a *fresh*
///   lease on a *fresh* slot — the old lease is revoked only after the
///   reattach lands, so the editor never sits on a dead parent;
/// - window loss converges a live session via `Terminated{WindowLost}`
///   and releases the slot — the DSP instance is untouched.
///
/// **RT contract:** non-RT only — UI-thread only.
pub struct PluginDock {
    /// The dock view this panel renders as.
    view: ViewInstanceId,
    /// The live `plugin.editor` slot — `None` after window loss until
    /// `ensure_slot`/`relocate` re-materializes it.
    slot: Option<SurfaceSlotId>,
    /// The shared slot/lease authority — a clone of the handle the
    /// composition root injected, never a panel-owned ledger.
    authority: SharedNativeSlotAuthority,
    /// The occupying editor, when any (including a terminally-failed
    /// session kept observable until `close`/`restore`).
    binding: Option<PluginDockBinding>,
    /// Last recorded slot geometry — survives `release_slot` (window
    /// loss) so a re-attach on a new window re-declares the same
    /// logical shape.
    last_geometry: SlotGeometry,
    /// Logical state restored from a project sidecar while no editor
    /// session is live. This is not an occupied binding: the panel
    /// starts honestly closed and an explicit open/restore command is
    /// required to mint runtime session/slot/lease identities.
    restored_logical: Option<PluginDockSnapshot>,
}

// `dyn NativeSlotAuthority` is not `Debug` (the driver's shared
// surface cannot implement foreign traits for it) — the projection
// prints the panel's own identity fields only.
impl fmt::Debug for PluginDock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginDock")
            .field("view", &self.view)
            .field("slot", &self.slot)
            .field("binding", &self.binding)
            .field("last_geometry", &self.last_geometry)
            .finish_non_exhaustive()
    }
}

impl PluginDock {
    /// Declares a panel for `view` with its `plugin.editor` slot on
    /// `window` at `geometry`, inside the shared slot/lease
    /// `authority` the composition root injected.
    ///
    /// # Errors
    /// [`PluginDockError::Slot`] on invalid geometry — the panel is not
    /// created.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn declare(
        view: ViewInstanceId,
        window: WindowId,
        geometry: SlotGeometry,
        authority: SharedNativeSlotAuthority,
    ) -> Result<Self, PluginDockError> {
        let slot = authority
            .borrow_mut()
            .declare_slot(Self::slot_spec(view, window, geometry))?;
        Ok(Self {
            view,
            slot: Some(slot),
            authority,
            binding: None,
            last_geometry: geometry,
            restored_logical: None,
        })
    }

    /// Declares a logical restored panel without a window, slot, lease,
    /// or editor session. Project-open persistence uses this honest
    /// shape: runtime identities are minted only by a later explicit
    /// open/restore command.
    #[must_use]
    pub fn declare_restored_logical(
        view: ViewInstanceId,
        authority: SharedNativeSlotAuthority,
        snapshot: PluginDockSnapshot,
    ) -> Self {
        let geometry = SlotGeometry {
            rect: LogicalRect {
                x: snapshot.origin[0],
                y: snapshot.origin[1],
                width: snapshot.size[0],
                height: snapshot.size[1],
            },
            scale_factor: snapshot.scale_factor,
        };
        Self {
            view,
            slot: None,
            authority,
            binding: None,
            last_geometry: geometry,
            restored_logical: Some(snapshot),
        }
    }

    /// The logical snapshot restored into an otherwise empty panel.
    #[must_use]
    pub fn restored_logical(&self) -> Option<&PluginDockSnapshot> {
        self.restored_logical.as_ref()
    }

    fn slot_spec(view: ViewInstanceId, window: WindowId, geometry: SlotGeometry) -> SlotSpec {
        SlotSpec {
            window,
            view,
            geometry,
            visibility: SlotVisibility::Visible,
            capabilities: PANEL_SLOT_CAPABILITIES,
        }
    }

    /// The dock view this panel renders as.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub const fn view(&self) -> ViewInstanceId {
        self.view
    }

    /// The live `plugin.editor` slot id (`None` after window loss).
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub const fn slot(&self) -> Option<SurfaceSlotId> {
        self.slot
    }

    /// A clone of the shared slot/lease authority handle — the same
    /// ledger this panel's slot lives in. Tests and the composition
    /// root inspect/validate through it; the panel itself borrows it
    /// per call.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub fn authority(&self) -> SharedNativeSlotAuthority {
        Rc::clone(&self.authority)
    }

    /// Ledger spec read, collapsing a consult failure to `None` —
    /// read-model use only (command paths propagate the error).
    fn spec_of(&self, slot: SurfaceSlotId) -> Option<SlotSpec> {
        self.authority.borrow().spec(slot).ok().flatten()
    }

    /// Ledger state read — same read-model-only collapse as
    /// [`Self::spec_of`].
    fn state_of(&self, slot: SurfaceSlotId) -> Option<SlotState> {
        self.authority.borrow().state(slot).ok().flatten()
    }

    /// The occupying editor binding, if any.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub const fn binding(&self) -> Option<&PluginDockBinding> {
        self.binding.as_ref()
    }

    /// The bound session's current upstream state.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub fn session_state(&self, sessions: &PluginEditorSessions) -> Option<EditorSessionState> {
        self.binding
            .and_then(|b| sessions.session(b.session))
            .map(PluginEditorSession::state)
    }

    /// The read-model projection the op layer serializes.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    #[must_use]
    pub fn status(&self, sessions: &PluginEditorSessions) -> PluginDockStatus {
        PluginDockStatus {
            view: self.view,
            window: self
                .slot
                .and_then(|s| self.spec_of(s))
                .map(|spec| spec.window),
            slot: self.slot,
            slot_state: self.slot.and_then(|s| self.state_of(s)),
            instance: self.binding.map(|b| b.instance),
            session: self.binding.map(|b| b.session),
            session_state: self.session_state(sessions),
            placement: self.binding.map(|b| b.placement),
            size: self.binding.map(|b| b.size),
        }
    }

    /// Guarantees a live slot declared against `window` — declares one
    /// when the panel has none, migrates (terminating any orphaned
    /// editor honestly via `Terminated{WindowLost}`) when the recorded
    /// owner differs, and refreshes recorded geometry otherwise.
    ///
    /// # Errors
    /// [`PluginDockError::Slot`] on invalid geometry or ledger refusal.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn ensure_slot(
        &mut self,
        sessions: &mut PluginEditorSessions,
        window: WindowId,
        geometry: SlotGeometry,
    ) -> Result<SurfaceSlotId, PluginDockError> {
        self.last_geometry = geometry;
        let owner = self
            .slot
            .and_then(|s| self.spec_of(s).map(|spec| spec.window));
        if owner == Some(window)
            && let Some(slot) = self.slot
        {
            self.authority.borrow_mut().set_geometry(slot, geometry)?;
            return Ok(slot);
        }
        if self.binding.is_some() {
            // The editor's recorded parent is gone — converge the
            // session honestly instead of re-parenting silently (the
            // hooked move paths use `relocate`, which re-parents).
            self.notify_window_lost(sessions);
            self.retire_binding(sessions)?;
        }
        if let Some(slot) = self.slot.take() {
            self.authority.borrow_mut().release_slot(slot)?;
        }
        let slot = self
            .authority
            .borrow_mut()
            .declare_slot(Self::slot_spec(self.view, window, geometry))?;
        self.slot = Some(slot);
        Ok(slot)
    }

    /// Lends the host adapter for `handle` with the issuer-side redeem
    /// hook bound to the shared authority that minted the lease. The
    /// closure owns a clone of the authority handle, so no panel
    /// borrow is held across the adapter lend.
    fn host_ops<'a>(
        host: &'a mut PluginEditorHostObj,
        authority: &SharedNativeSlotAuthority,
        handle: PluginHandle,
    ) -> Result<OpsBridge<'a>, EditorOpError> {
        let authority = Rc::clone(authority);
        let redeem: Box<LeaseRedeemer<'a>> =
            Box::new(move |lease| redeem_lease(&authority, *lease));
        host.ops(handle, redeem).map(OpsBridge)
    }

    /// `ops` lend + `session.open` contained in one call — the
    /// `&mut sessions` borrow ends with it so the caller can mutate
    /// the ledger on either outcome.
    fn open_with_ops(
        host: &mut PluginEditorHostObj,
        authority: &SharedNativeSlotAuthority,
        sessions: &mut PluginEditorSessions,
        binding: &PluginDockBinding,
        parent: ParentSlotLease,
        mode: AttachMode,
    ) -> Result<EditorCapabilities, PluginDockError> {
        let mut ops = Self::host_ops(host, authority, binding.handle)?;
        sessions
            .session_mut(binding.session)
            .expect("freshly minted session")
            .open(&mut ops, &parent, binding.size, mode)
            .map_err(Into::into)
    }

    /// `ops` lend + `session.reattach` — same contained-borrow shape
    /// as [`Self::open_with_ops`].
    fn reattach_with_ops(
        host: &mut PluginEditorHostObj,
        authority: &SharedNativeSlotAuthority,
        sessions: &mut PluginEditorSessions,
        binding: &PluginDockBinding,
        parent: ParentSlotLease,
    ) -> Result<(), PluginDockError> {
        let mut ops = Self::host_ops(host, authority, binding.handle)?;
        sessions
            .session_mut(binding.session)
            .ok_or(EditorSessionError::UnknownSession)?
            .reattach(&mut ops, &parent)
            .map_err(Into::into)
    }

    /// Opens `instance`'s editor in the panel: resolve handle →
    /// `begin_session` → `lease_parent` → `session.open` under the
    /// minted lease. On adapter refusal the session lands `Failed` and
    /// the binding records it (observable until `close`/`restore`) —
    /// on pre-open ledger refusal the half-minted session is retired.
    ///
    /// # Errors
    /// [`PluginDockError::Occupied`] while a binding exists;
    /// [`PluginDockError::NoSlot`] without a declared slot;
    /// [`PluginDockError::UnknownInstance`] when `resolve` fails;
    /// ledger/session/ops errors as wrapped.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn open(
        &mut self,
        sessions: &mut PluginEditorSessions,
        host: &mut PluginEditorHostObj,
        instance: StablePluginInstanceId,
        size: EditorSize,
        mode: AttachMode,
    ) -> Result<PluginDockReport, PluginDockError> {
        self.restored_logical = None;
        let slot = self.slot.ok_or(PluginDockError::NoSlot)?;
        if self.binding.is_some() {
            return Err(PluginDockError::Occupied);
        }
        let handle = host
            .resolve(instance)
            .ok_or(PluginDockError::UnknownInstance)?;
        let session = sessions.begin_session(handle)?;
        let leased = self.authority.borrow_mut().lease_parent(slot);
        let lease = match leased {
            Ok(lease) => lease,
            Err(e) => {
                // Never-opened sessions retire cleanly — no binding,
                // no pinned instance.
                let _ = sessions.end_session(session);
                return Err(e.into());
            }
        };
        let parent = ParentSlotLease::new(lease.raw());
        let binding = PluginDockBinding {
            instance,
            handle,
            session,
            slot,
            view: self.view,
            placement: PluginDockPlacement::from_attach_mode(mode),
            size,
            lease: Some(lease),
        };
        let opened = Self::open_with_ops(host, &self.authority, sessions, &binding, parent, mode);
        self.binding = Some(binding);
        match opened {
            Ok(capabilities) => Ok(PluginDockReport {
                view: self.view,
                slot,
                session,
                instance,
                mode,
                capabilities,
            }),
            // The adapter could not even be lent — the session never
            // saw `open`: drop the binding, revoke the lease, retire
            // the half-minted session.
            Err(e @ PluginDockError::Ops(_)) => {
                self.binding = None;
                let _ = self.authority.borrow_mut().release_lease(lease);
                let _ = sessions.end_session(session);
                Err(e)
            }
            // The binding keeps the failed session observable —
            // `close`/`restore` retire it; the lease is revoked so the
            // slot can host a retry.
            Err(e) => {
                let _ = self.authority.borrow_mut().release_lease(lease);
                if let Some(binding) = self.binding.as_mut() {
                    binding.lease = None;
                }
                Err(e)
            }
        }
    }

    /// Shows (`visible = true`) or hides the editor — the session
    /// transition plus the slot mirror.
    ///
    /// # Errors
    /// [`PluginDockError::NoEditor`] without a binding; session/slot
    /// errors as wrapped (a session refusal leaves both unchanged).
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn set_visibility(
        &mut self,
        sessions: &mut PluginEditorSessions,
        host: &mut PluginEditorHostObj,
        visible: bool,
    ) -> Result<(), PluginDockError> {
        let (session_id, handle, slot) = self.binding_ids()?;
        let mut ops = Self::host_ops(host, &self.authority, handle)?;
        sessions
            .session_mut(session_id)
            .ok_or(EditorSessionError::UnknownSession)?
            .set_visibility(&mut ops, visible)?;
        drop(ops);
        self.authority.borrow_mut().set_visibility(
            slot,
            if visible {
                SlotVisibility::Visible
            } else {
                SlotVisibility::Hidden
            },
        )?;
        Ok(())
    }

    /// Requests an editor resize and records the negotiated size.
    ///
    /// # Errors
    /// [`PluginDockError::NoEditor`] without a binding; session errors
    /// as wrapped (a plugin refusal is non-fatal upstream — the session
    /// is left unchanged).
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn set_size(
        &mut self,
        sessions: &mut PluginEditorSessions,
        host: &mut PluginEditorHostObj,
        size: EditorSize,
    ) -> Result<(), PluginDockError> {
        let (session_id, handle, _) = self.binding_ids()?;
        let mut ops = Self::host_ops(host, &self.authority, handle)?;
        sessions
            .session_mut(session_id)
            .ok_or(EditorSessionError::UnknownSession)?
            .set_size(&mut ops, size)?;
        if let Some(binding) = self.binding.as_mut() {
            binding.size = size;
        }
        Ok(())
    }

    /// Gives (`focused = true`) or removes keyboard focus on the
    /// editor — non-fatal upstream, no session transition.
    ///
    /// # Errors
    /// [`PluginDockError::NoEditor`] without a binding; session errors
    /// as wrapped.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn set_focus(
        &mut self,
        sessions: &mut PluginEditorSessions,
        host: &mut PluginEditorHostObj,
        focused: bool,
    ) -> Result<(), PluginDockError> {
        let (session_id, handle, _) = self.binding_ids()?;
        let mut ops = Self::host_ops(host, &self.authority, handle)?;
        sessions
            .session_mut(session_id)
            .ok_or(EditorSessionError::UnknownSession)?
            .set_focus(&mut ops, focused)?;
        Ok(())
    }

    /// Records a new slot geometry (layout/resize/DPI reconciliation).
    ///
    /// # Errors
    /// [`PluginDockError::NoSlot`]/`Slot` on bad id or geometry.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn set_slot_geometry(&mut self, geometry: SlotGeometry) -> Result<(), PluginDockError> {
        let slot = self.slot.ok_or(PluginDockError::NoSlot)?;
        self.authority.borrow_mut().set_geometry(slot, geometry)?;
        self.last_geometry = geometry;
        Ok(())
    }

    /// Prepare-half hook for `syn.window.dock_prepare*` — the slot
    /// refuses new leases while the dock move is in flight.
    ///
    /// # Errors
    /// [`PluginDockError::Slot`] on a stale slot id (a no-op without
    /// one — nothing exists to guard).
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn prepare_move(&mut self) -> Result<(), PluginDockError> {
        if let Some(slot) = self.slot {
            self.authority.borrow_mut().mark_busy(slot)?;
        }
        Ok(())
    }

    /// Abort-half hook for `syn.window.dock_abort` — clears the busy
    /// mark; the editor's parent never changed.
    ///
    /// # Errors
    /// [`PluginDockError::Slot`] on a stale slot id.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn abort_move(&mut self) -> Result<(), PluginDockError> {
        if let Some(slot) = self.slot {
            self.authority.borrow_mut().clear_busy(slot)?;
        }
        Ok(())
    }

    /// Commit-half hook for `syn.window.dock_commit`/`dock_reattach`
    /// and the re-attach path — clears the busy mark and migrates the
    /// slot when the view's owner changed. Same-window zone moves only
    /// refresh bookkeeping; cross-window moves mint a fresh slot +
    /// lease and re-parent the live session through `ops.reattach`.
    ///
    /// Failure honesty: a refused reattach leaves the old slot and its
    /// lease intact (the editor may still be parented there) and the
    /// session lands `Failed`; a missing host converges the session
    /// `Terminated{HostLost}` — the move already happened, so a fake
    /// still-parented success is never recorded.
    ///
    /// # Errors
    /// [`PluginDockError::Slot`]/`Session`/`Ops` on ledger/adapter
    /// refusal — the view already moved, so callers report the op's
    /// dock move as committed and surface this error separately.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn relocate(
        &mut self,
        sessions: &mut PluginEditorSessions,
        host: Option<&mut PluginEditorHostObj>,
        to_window: WindowId,
    ) -> Result<(), PluginDockError> {
        let Some(old_slot) = self.slot else {
            // The slot died with its window — re-declare on the new
            // owner with the last recorded geometry. A live session
            // cannot exist here (window loss converged it), so there
            // is nothing to re-parent.
            let slot = self.authority.borrow_mut().declare_slot(Self::slot_spec(
                self.view,
                to_window,
                self.last_geometry,
            ))?;
            self.slot = Some(slot);
            return Ok(());
        };
        let _ = self.authority.borrow_mut().clear_busy(old_slot);
        let spec = self
            .authority
            .borrow()
            .spec(old_slot)
            .map_err(PluginDockError::Slot)?
            .ok_or(PluginDockError::NoSlot)?;
        if spec.window == to_window {
            return self.reshow_hidden_on_same_window(sessions, host, old_slot);
        }
        let new_slot = self.authority.borrow_mut().declare_slot(Self::slot_spec(
            self.view,
            to_window,
            spec.geometry,
        ))?;
        self.slot = Some(new_slot);
        let Some(binding) = self.binding else {
            // Empty panel — the slot follows its view with no editor
            // to re-parent.
            self.authority.borrow_mut().release_slot(old_slot)?;
            return Ok(());
        };
        let live = sessions.session(binding.session).is_some_and(|s| {
            matches!(
                s.state(),
                EditorSessionState::Attached { .. } | EditorSessionState::Hidden { .. }
            )
        });
        if !live {
            // A terminally-failed/concluded session has no editor to
            // re-parent — the old slot (and its lease, if any) dies
            // with the old window.
            self.authority.borrow_mut().release_slot(old_slot)?;
            if let Some(b) = self.binding.as_mut() {
                b.lease = None;
            }
            return Ok(());
        }
        let Some(host) = host else {
            // No adapter can re-parent — converge the live session
            // honestly rather than record a fake still-parented one.
            sessions.notify_terminated(binding.handle, SessionFailCause::HostLost);
            self.authority.borrow_mut().release_slot(old_slot)?;
            if let Some(b) = self.binding.as_mut() {
                b.lease = None;
            }
            return Ok(());
        };
        let new_lease = self.authority.borrow_mut().lease_parent(new_slot)?;
        let parent = ParentSlotLease::new(new_lease.raw());
        let reattached = Self::reattach_with_ops(host, &self.authority, sessions, &binding, parent);
        match reattached {
            Ok(()) => {
                self.authority.borrow_mut().release_slot(old_slot)?;
                if let Some(b) = self.binding.as_mut() {
                    b.slot = new_slot;
                    b.lease = Some(new_lease);
                }
                // Mirror the resulting session state onto the new
                // slot — a hidden-before-move editor may land visible
                // (`reattach` converges to `Attached`).
                let visible = sessions
                    .session(binding.session)
                    .is_some_and(|s| matches!(s.state(), EditorSessionState::Attached { .. }));
                let _ = self.authority.borrow_mut().set_visibility(
                    new_slot,
                    if visible {
                        SlotVisibility::Visible
                    } else {
                        SlotVisibility::Hidden
                    },
                );
                Ok(())
            }
            Err(e) => {
                // Keep the old slot + lease — the editor may still be
                // parented under them; retire the unused new slot.
                let _ = self.authority.borrow_mut().release_lease(new_lease);
                let _ = self.authority.borrow_mut().release_slot(new_slot);
                self.slot = Some(old_slot);
                Err(e)
            }
        }
    }

    /// Same-window half of [`Self::relocate`]: a `Hidden` session
    /// (e.g. an editor a `detach_to_pool` suppressed) re-shows on
    /// re-attach — the slot's reserved region is rendered again, so
    /// the editor returns to `Attached`. A deliberately-hidden
    /// binding is not silently re-shown — only the detach-seated one.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    fn reshow_hidden_on_same_window(
        &mut self,
        sessions: &mut PluginEditorSessions,
        host: Option<&mut PluginEditorHostObj>,
        slot: SurfaceSlotId,
    ) -> Result<(), PluginDockError> {
        let Some(binding) = self.binding else {
            return Ok(());
        };
        let hidden = sessions
            .session(binding.session)
            .is_some_and(|s| matches!(s.state(), EditorSessionState::Hidden { .. }));
        if !hidden {
            return Ok(());
        }
        let Some(host) = host else {
            return Ok(());
        };
        let mut ops = Self::host_ops(host, &self.authority, binding.handle)?;
        sessions
            .session_mut(binding.session)
            .ok_or(EditorSessionError::UnknownSession)?
            .set_visibility(&mut ops, true)?;
        drop(ops);
        self.authority
            .borrow_mut()
            .set_visibility(slot, SlotVisibility::Visible)?;
        Ok(())
    }

    /// View-detach hook (`detach_to_pool` / window destroy under
    /// `DetachViews`): the slot's reserved region is unmounted — the
    /// surface records `Hidden` and a live session is hidden through
    /// the adapter when one is installed.
    ///
    /// # Errors
    /// [`PluginDockError::Session`] on an adapter refusal — the slot
    /// record already suppressed the surface, so the error reports
    /// session/model drift honestly.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn notify_detached(
        &mut self,
        sessions: &mut PluginEditorSessions,
        host: Option<&mut PluginEditorHostObj>,
    ) -> Result<(), PluginDockError> {
        if let Some(slot) = self.slot {
            let _ = self
                .authority
                .borrow_mut()
                .set_visibility(slot, SlotVisibility::Hidden);
        }
        let Some(binding) = self.binding else {
            return Ok(());
        };
        let live = sessions.session(binding.session).is_some_and(|s| {
            matches!(
                s.state(),
                EditorSessionState::Attached { .. } | EditorSessionState::Hidden { .. }
            )
        });
        if !live {
            return Ok(());
        }
        let Some(host) = host else {
            return Ok(());
        };
        let mut ops = Self::host_ops(host, &self.authority, binding.handle)?;
        sessions
            .session_mut(binding.session)
            .ok_or(EditorSessionError::UnknownSession)?
            .set_visibility(&mut ops, false)?;
        Ok(())
    }

    /// Window-destroy hook: the slot's window is gone — a live session
    /// converges `Terminated{WindowLost}` (the DSP instance survives),
    /// its lease is revoked with the slot, and the panel keeps the
    /// terminally-recorded binding observable until `close`.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn notify_window_lost(&mut self, sessions: &mut PluginEditorSessions) {
        if let Some(binding) = self.binding.as_mut() {
            sessions.notify_terminated(binding.handle, SessionFailCause::WindowLost);
            binding.lease = None;
        }
        if let Some(slot) = self.slot.take() {
            let _ = self.authority.borrow_mut().release_slot(slot);
        }
    }

    /// Instance-destroy hook (host notification): converges the live
    /// session on the terminal event and frees the parent seat — the
    /// binding stays observable until `close`.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn notify_instance_terminated(
        &mut self,
        sessions: &mut PluginEditorSessions,
        cause: SessionFailCause,
    ) {
        if let Some(binding) = self.binding.as_mut() {
            sessions.notify_terminated(binding.handle, cause);
            if let Some(lease) = binding.lease.take() {
                let _ = self.authority.borrow_mut().release_lease(lease);
            }
        }
    }

    /// Closes the occupying editor: drives the session to `Closed`
    /// through the adapter when one is installed (a `Failed` session
    /// whose open was refused concludes without an adapter call), or
    /// converges `Terminated{HostLost}` honestly when none is. The
    /// lease is revoked and the session retired; the panel and its
    /// slot stay, ready to host another editor.
    ///
    /// # Errors
    /// [`PluginDockError::NoEditor`] without a binding; session/ops
    /// errors as wrapped — on adapter refusal the session lands
    /// `Failed` and the binding is kept for retry/inspection.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn close(
        &mut self,
        sessions: &mut PluginEditorSessions,
        host: Option<&mut PluginEditorHostObj>,
    ) -> Result<(), PluginDockError> {
        let binding = self.binding.ok_or(PluginDockError::NoEditor)?;
        let concluded = sessions
            .session(binding.session)
            .is_none_or(|s| s.concluded() || matches!(s.state(), EditorSessionState::Closed));
        if !concluded {
            match host {
                Some(host) => {
                    let mut ops = Self::host_ops(host, &self.authority, binding.handle)?;
                    sessions
                        .session_mut(binding.session)
                        .ok_or(EditorSessionError::UnknownSession)?
                        .close(&mut ops)?;
                }
                None => sessions.notify_terminated(binding.handle, SessionFailCause::HostLost),
            }
        }
        self.retire_binding(sessions)
    }

    /// Retires a `Closed` session — revokes the lease and drops the
    /// binding. Only legal once the session converged; `Failed` or
    /// live sessions must go through [`Self::close`].
    fn retire_binding(
        &mut self,
        sessions: &mut PluginEditorSessions,
    ) -> Result<(), PluginDockError> {
        if let Some(binding) = self.binding.take() {
            if let Some(lease) = binding.lease {
                let _ = self.authority.borrow_mut().release_lease(lease);
            }
            sessions.end_session(binding.session)?;
        }
        Ok(())
    }

    /// The PD-07 logical save: instance identity + requested
    /// placement + logical size + visibility — no session id, slot id,
    /// lease or OS handle.
    ///
    /// # Errors
    /// [`PluginDockError::NoEditor`] on an empty panel — there is no
    /// editor state to persist.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn save(
        &self,
        sessions: &PluginEditorSessions,
    ) -> Result<PluginDockSnapshot, PluginDockError> {
        let Some(binding) = self.binding else {
            return self
                .restored_logical
                .clone()
                .ok_or(PluginDockError::NoEditor);
        };
        let visible = sessions
            .session(binding.session)
            .is_some_and(|s| matches!(s.state(), EditorSessionState::Attached { .. }));
        Ok(PluginDockSnapshot {
            instance: binding.instance.get(),
            placement: binding.placement,
            size: [binding.size.width, binding.size.height],
            origin: [self.last_geometry.rect.x, self.last_geometry.rect.y],
            scale_factor: self.last_geometry.scale_factor,
            visible,
        })
    }

    /// Restores a saved editor: closes any occupying session first,
    /// then re-opens the snapshot's instance in the saved placement
    /// and re-hides when the snapshot recorded `visible: false`.
    ///
    /// # Errors
    /// Propagates `close`/`open`/`set_visibility` errors — a failed
    /// restore leaves the panel in the honest state the last completed
    /// step produced (empty, `Failed` binding, or attached-but-visible).
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn restore(
        &mut self,
        sessions: &mut PluginEditorSessions,
        host: &mut PluginEditorHostObj,
        snapshot: &PluginDockSnapshot,
    ) -> Result<PluginDockReport, PluginDockError> {
        if self.binding.is_some() {
            self.close(sessions, Some(&mut *host))?;
        }
        let report = self.open(
            sessions,
            &mut *host,
            StablePluginInstanceId::new(snapshot.instance),
            EditorSize {
                width: snapshot.size[0],
                height: snapshot.size[1],
            },
            snapshot.placement.to_attach_mode(),
        )?;
        if !snapshot.visible {
            self.set_visibility(sessions, &mut *host, false)?;
        }
        Ok(report)
    }

    /// Evaluates every session's deadline policy once (driver-driven
    /// monitoring — a request never ticked stays in flight). Returns
    /// how many in-flight requests timed out.
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn tick(&mut self, sessions: &mut PluginEditorSessions) -> usize {
        sessions.tick_all()
    }

    /// Drains the bound session's pending events (empty when no
    /// binding exists).
    ///
    /// **RT contract:** non-RT only — UI-thread only.
    pub fn drain_session_events(
        &mut self,
        sessions: &mut PluginEditorSessions,
    ) -> Vec<ruviform_plugin_host::EditorSessionEvent> {
        self.binding
            .and_then(|b| sessions.session_mut(b.session))
            .map_or_else(Vec::new, PluginEditorSession::drain_events)
    }

    /// The `(session, handle, slot)` triple for commands needing the
    /// adapter — `NoEditor` without a binding.
    fn binding_ids(
        &self,
    ) -> Result<(PluginEditorSessionId, PluginHandle, SurfaceSlotId), PluginDockError> {
        self.binding
            .map(|b| (b.session, b.handle, b.slot))
            .ok_or(PluginDockError::NoEditor)
    }
}

/// Wire/JSON name for an [`EditorSessionState`] — the `state` op's
/// `session_state` field and the status label's vocabulary.
///
/// **RT contract:** non-RT only — UI-thread only.
#[must_use]
pub fn session_state_name(state: &EditorSessionState) -> &'static str {
    match state {
        EditorSessionState::Closed => "closed",
        EditorSessionState::Creating { .. } => "creating",
        EditorSessionState::Attached { .. } => "attached",
        EditorSessionState::Hidden { .. } => "hidden",
        EditorSessionState::Detaching { .. } => "detaching",
        EditorSessionState::Closing { .. } => "closing",
        EditorSessionState::Failed { .. } => "failed",
        EditorSessionState::Recovering { .. } => "recovering",
        _ => "unknown",
    }
}

/// The `AttachMode` a session state reports, when it carries one.
///
/// **RT contract:** non-RT only — UI-thread only.
#[must_use]
pub fn session_state_mode(state: &EditorSessionState) -> Option<AttachMode> {
    match state {
        EditorSessionState::Attached { mode } | EditorSessionState::Hidden { mode } => Some(*mode),
        _ => None,
    }
}

/// The product `PluginDock` `ViewSpec` — Panel `plugin.dock` carrying a
/// status `Label` projection plus the `plugin.editor`
/// [`NativeSlotSpec`] the upstream contract vocabulary pins.
///
/// The spec stays declarative: mounting it proves layout/semantics
/// only — the slot binds a runtime `SurfaceSlotId` through the shared
/// [`NativeSlotAuthority`] ledger, never through this tree.
///
/// **RT contract:** non-RT only — UI-thread only.
#[must_use]
pub fn plugin_dock_screen_spec(status: &PluginDockStatus) -> ViewSpec {
    ViewSpec {
        root: ComponentSpec::Panel(PanelSpec {
            key: ComponentKey(PLUGIN_DOCK_PANEL_KEY.to_string()),
            title: Some("Plugin Dock".to_string()),
            children: vec![
                ComponentSpec::Label(LabelSpec {
                    key: ComponentKey(PLUGIN_DOCK_STATUS_KEY.to_string()),
                    text: plugin_dock_status_text(status),
                    visual: ComponentVisualState::default(),
                    a11y: A11ySpec {
                        label: Some("Plugin editor status".to_string()),
                        ..A11ySpec::default()
                    },
                }),
                ComponentSpec::NativeSlot(NativeSlotSpec {
                    key: ComponentKey(PLUGIN_DOCK_SLOT_KEY.to_string()),
                    min_size: Some([64.0, 64.0]),
                    visual: ComponentVisualState::default(),
                    a11y: A11ySpec::default(),
                }),
            ],
            visual: ComponentVisualState::default(),
            a11y: A11ySpec::default(),
        }),
    }
}

/// The status line text — honest session vocabulary, never a claim
/// the editor is real.
fn plugin_dock_status_text(status: &PluginDockStatus) -> String {
    match (status.instance, status.session_state) {
        (None, _) => "no editor".to_string(),
        (Some(instance), Some(state)) => {
            let mode = session_state_mode(&state)
                .map(|m| format!(" ({m:?})").to_lowercase())
                .unwrap_or_default();
            format!(
                "instance {} — {}{}",
                instance.get(),
                session_state_name(&state),
                mode
            )
        }
        (Some(instance), None) => format!("instance {} — session retired", instance.get()),
    }
}

/// Mounts a `PluginDock` panel's screen spec onto a fresh [`ViewHost`]
/// under the session-owned `AttachEpochAllocator` — the same mint
/// contract `mount_screen_view` enforces for every product view.
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation — a code
/// defect.
///
/// **RT contract:** non-RT only — UI-thread only.
pub fn mount_plugin_dock_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    status: &PluginDockStatus,
) -> Result<ViewHost, UiError> {
    let mut host = ViewHost::new(status.view, attach_epochs.borrow_mut().mint());
    host.mount(plugin_dock_screen_spec(status))
        .map_err(|e| UiError::Internal {
            message: format!("plugin dock spec invalid: {e}"),
        })?;
    Ok(host)
}
