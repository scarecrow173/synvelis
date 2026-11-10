//! `WorkspaceManager` — the WS-A orchestrator.
//!
//! Composes `DockWorkspace` (dock tree + move contract), a
//! [`WindowHandle`] host port, the view/state registries, focus state,
//! link groups and the single [`ProjectSession`]. All dock/window
//! operations are non-RT and never touch project history or the
//! engine stub's graph-build counter.

use ruviform_ui::dock::{
    DockError, DockTarget, DockTree, DockWorkspace, LayoutSnapshot, MoveReport, MoveToken,
    ViewInstanceId, WindowCloseDisposition, WindowGeometry,
};
use ruviform_ui::event::HostEvent;
use ruviform_ui::window::{MonitorInfo, WindowConfig, WindowHandle, WindowId};
use synvelis_project::{
    ControlEntityRef, ExplicitChannelMap, PortRole, ProjectOp, ProjectRevision, SemanticPortId,
    TimeAnchor, Transform, port_capability_generation,
};

use crate::command::{CommandContext, CommandKind, CommandOutcome, build_project_op, spec_for};
use crate::control_edge::{
    ControlEdgeFixation, ControlEdgeGesture, ControlEdgeOutcome, ControlEdgePreview,
};
use crate::edit::{self, CommitEffect};
use crate::error::WorkspaceError;
use crate::focus::{FocusState, InputContext};
use crate::gesture::{CancelReason, Gesture, GestureFixation, GestureOutcome, GesturePreview};
use crate::link::{LinkEvent, LinkGroupId, LinkGroupManager};
use crate::persist::{LinkRecord, ViewRecord, WORKSPACE_SCHEMA, WorkspaceSnapshot};
use crate::routing_gesture::{
    RoutingGesture, RoutingGestureFixation, RoutingGestureOutcome, RoutingGesturePreview,
    RoutingPortAssessment, assess_connection, routing_gesture_intent,
};
use crate::selection::{EditTimePoint, LaneScope, SelectionDomain, TimeSelection};
use crate::session::ProjectSession;
use crate::signal_flow_ui::{SignalFlowEntityRef, SignalFlowViewState};
use crate::snap::{SnapEnvironment, resolve_snap};
use crate::state::{
    EditingSelection, FollowSource, InspectionMode, SemanticTarget, ViewState, ViewStateRegistry,
};
use crate::tool::{EditOperation, HandleKind, HitProbe, Tool, ToolResolution, resolve_tool};
use crate::vcs_ui::VcsHistoryViews;
use crate::view::{ViewDescriptor, ViewRegistry, ViewType};

/// A window the manager knows about (creation order = capture order).
#[derive(Debug, Clone)]
pub struct WindowRecord {
    /// Host-issued id.
    pub id: WindowId,
    /// Display title.
    pub title: String,
    /// Last known geometry (for `LayoutSnapshot::capture`).
    pub geometry: WindowGeometry,
    /// When `true`, `CloseRequested` is answered `Veto`.
    pub veto_close: bool,
    /// How the dock disposes of this window's views on removal.
    pub disposition: WindowCloseDisposition,
}

/// The headless workspace orchestrator.
pub struct WorkspaceManager<H: WindowHandle> {
    /// The window host port (fake host in tests).
    pub host: H,
    pub(crate) dock: DockWorkspace,
    pub(crate) registry: ViewRegistry,
    pub(crate) states: ViewStateRegistry,
    pub(crate) vcs: VcsHistoryViews,
    pub(crate) links: LinkGroupManager,
    pub(crate) focus: FocusState,
    session: ProjectSession,
    pub(crate) windows: Vec<WindowRecord>,
    pub(crate) next_view: u64,
    /// Text-lane undo invocations — the stub standing in for text
    /// fields' own undo stacks (text input undo never reaches the
    /// DAW project history).
    text_lane_undos: u64,
}

impl<H: WindowHandle> WorkspaceManager<H> {
    /// An empty workspace over `session`, driven by `host`.
    #[must_use]
    pub fn new(host: H, session: ProjectSession) -> Self {
        Self {
            host,
            dock: DockWorkspace::new(),
            registry: ViewRegistry::default(),
            states: ViewStateRegistry::default(),
            vcs: VcsHistoryViews::new(),
            links: LinkGroupManager::default(),
            focus: FocusState::default(),
            session,
            windows: Vec::new(),
            next_view: 0,
            text_lane_undos: 0,
        }
    }

    // -- accessors ---------------------------------------------------------

    /// The dock workspace.
    #[must_use]
    pub fn dock(&self) -> &DockWorkspace {
        &self.dock
    }

    /// The view registry.
    #[must_use]
    pub fn registry(&self) -> &ViewRegistry {
        &self.registry
    }

    /// The view-state registry.
    #[must_use]
    pub fn states(&self) -> &ViewStateRegistry {
        &self.states
    }

    /// Mutable view-state registry.
    #[must_use]
    pub fn states_mut(&mut self) -> &mut ViewStateRegistry {
        &mut self.states
    }

    /// The VCS history/diff view-state registry (VCS-04).
    #[must_use]
    pub fn vcs(&self) -> &VcsHistoryViews {
        &self.vcs
    }

    /// Mutable VCS history/diff view-state registry.
    #[must_use]
    pub fn vcs_mut(&mut self) -> &mut VcsHistoryViews {
        &mut self.vcs
    }

    /// Runs `f` with split borrows of the VCS registry and the session.
    ///
    /// [`VcsHistoryViews`] ops need a [`crate::vcs_ui::VcsSession`]
    /// context built from `session()` — but `session()` borrows all of
    /// `self`, so callers cannot hold it across a `vcs_mut()` call.
    /// This accessor hands out `(&mut VcsHistoryViews, &ProjectSession)`
    /// simultaneously; the session side is shared, so nothing here can
    /// mutate project state.
    pub fn with_vcs<R>(&mut self, f: impl FnOnce(&mut VcsHistoryViews, &ProjectSession) -> R) -> R {
        f(&mut self.vcs, &self.session)
    }

    /// The link-group manager.
    #[must_use]
    pub fn links(&self) -> &LinkGroupManager {
        &self.links
    }

    /// Mutable link-group manager.
    #[must_use]
    pub fn links_mut(&mut self) -> &mut LinkGroupManager {
        &mut self.links
    }

    /// The focus state.
    #[must_use]
    pub fn focus(&self) -> &FocusState {
        &self.focus
    }

    /// Mutable focus state (input-context changes, tests).
    #[must_use]
    pub fn focus_mut(&mut self) -> &mut FocusState {
        &mut self.focus
    }

    /// The project session.
    #[must_use]
    pub fn session(&self) -> &ProjectSession {
        &self.session
    }

    /// Mutable project session (undo/save flows).
    #[must_use]
    pub fn session_mut(&mut self) -> &mut ProjectSession {
        &mut self.session
    }

    /// The window ledger, in creation order.
    #[must_use]
    pub fn windows(&self) -> &[WindowRecord] {
        &self.windows
    }

    /// How many undos landed on the text lane — the evidence that
    /// text-input undo stayed out of the project history.
    #[must_use]
    pub fn text_lane_undos(&self) -> u64 {
        self.text_lane_undos
    }

    fn window_record(&self, id: WindowId) -> Option<&WindowRecord> {
        self.windows.iter().find(|w| w.id == id)
    }

    fn window_record_mut(&mut self, id: WindowId) -> Option<&mut WindowRecord> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    // -- windows -----------------------------------------------------------

    /// Creates a host window and registers it with the dock.
    ///
    /// # Errors
    /// [`WorkspaceError::Ui`] when the host rejects creation.
    pub fn create_window(
        &mut self,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<WindowId, WorkspaceError> {
        self.create_window_at(title, 0, 0, width, height)
    }

    /// Like [`create_window`](Self::create_window) with an explicit
    /// initial position.
    ///
    /// # Errors
    /// [`WorkspaceError::Ui`] when the host rejects creation.
    pub fn create_window_at(
        &mut self,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<WindowId, WorkspaceError> {
        let config = WindowConfig {
            title: title.to_string(),
            width,
            height,
            position: Some((x, y)),
            ..WindowConfig::default()
        };
        let id = self.host.create_window(&config)?;
        self.dock.register_window(id);
        self.windows.push(WindowRecord {
            id,
            title: title.to_string(),
            geometry: WindowGeometry {
                x,
                y,
                width,
                height,
                monitor: self
                    .host
                    .monitors()
                    .first()
                    .map(ruviform_ui::dock::MonitorHint::from),
            },
            veto_close: false,
            disposition: WindowCloseDisposition::DetachViews,
        });
        Ok(id)
    }

    /// Adopts an already-created host window into the dock ledger.
    ///
    /// Unlike [`create_window_at`](Self::create_window_at) this never
    /// calls `host.create_window`: the driver/composition root owns
    /// window creation, and the workspace only records the result.
    /// The record defaults to `veto_close: false` and
    /// [`WindowCloseDisposition::DetachViews`] — the same defaults a
    /// created window gets.
    ///
    /// # Errors
    /// [`WorkspaceError::DuplicateWindow`] when `id` is already known.
    pub fn adopt_window(
        &mut self,
        id: WindowId,
        title: &str,
        geometry: WindowGeometry,
    ) -> Result<(), WorkspaceError> {
        if self.window_record(id).is_some() {
            return Err(WorkspaceError::DuplicateWindow(id));
        }
        self.dock.register_window(id);
        self.windows.push(WindowRecord {
            id,
            title: title.to_string(),
            geometry,
            veto_close: false,
            disposition: WindowCloseDisposition::DetachViews,
        });
        Ok(())
    }

    /// Sets whether `window` vetoes close requests.
    pub fn set_close_veto(&mut self, window: WindowId, veto: bool) {
        if let Some(rec) = self.window_record_mut(window) {
            rec.veto_close = veto;
        }
    }

    /// Sets the close disposition for `window`.
    pub fn set_close_disposition(&mut self, window: WindowId, d: WindowCloseDisposition) {
        if let Some(rec) = self.window_record_mut(window) {
            rec.disposition = d;
        }
    }

    /// Asks the host to close `window` (normal veto-able flow).
    pub fn request_close(&mut self, window: WindowId) {
        self.host.request_close(window);
    }

    // -- views ---------------------------------------------------------------

    fn mint_view(&mut self) -> ViewInstanceId {
        self.next_view += 1;
        // Wrap is unreachable in practice (u64); fall back to a scan.
        ViewInstanceId::new(self.next_view).unwrap_or_else(|| {
            (1..=u64::MAX)
                .find_map(ViewInstanceId::new)
                .expect("view id space exhausted")
        })
    }

    /// Spawns a view of `kind` attached at `target`.
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] when the target is invalid.
    pub fn spawn_view(
        &mut self,
        kind: ViewType,
        title: &str,
        target: DockTarget,
    ) -> Result<ViewInstanceId, WorkspaceError> {
        let id = self.mint_view();
        self.dock.attach_view(id, target)?;
        self.registry.register(id, kind, title.to_string());
        self.states.ensure(id);
        self.vcs.ensure(id);
        Ok(id)
    }

    /// Spawns a view into the detached pool (no dock location yet).
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] on the (unreachable) duplicate-id case.
    pub fn spawn_detached_view(
        &mut self,
        kind: ViewType,
        title: &str,
    ) -> Result<ViewInstanceId, WorkspaceError> {
        let id = self.mint_view();
        self.dock.add_detached(id)?;
        self.registry.register(id, kind, title.to_string());
        self.states.ensure(id);
        self.vcs.ensure(id);
        Ok(id)
    }

    /// Adopts an already-mounted view into the workspace.
    ///
    /// Unlike [`spawn_view`](Self::spawn_view) /
    /// [`spawn_detached_view`](Self::spawn_detached_view) this takes
    /// the caller-chosen `id` — the app/UI layer owns view identity —
    /// and never mints one. `Some(target)` attaches the view into the
    /// target window's dock (the window must already be adopted);
    /// `None` places it in the detached pool. The dock placement runs
    /// before the registry/state registration so a dock rejection
    /// leaves no partial view behind, and `next_view` advances past
    /// `id` so later `spawn_view` calls cannot collide with adopted
    /// ids.
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] when the view is already owned by the
    /// dock ([`DockError::DuplicateView`]) or `target` is invalid;
    /// the same variant is used when `id` is already registered in
    /// the view registry.
    pub fn adopt_view(
        &mut self,
        id: ViewInstanceId,
        kind: ViewType,
        title: &str,
        target: Option<DockTarget>,
    ) -> Result<(), WorkspaceError> {
        if self.registry.contains(id) {
            return Err(WorkspaceError::Dock(DockError::DuplicateView {
                view: id,
                owner: self.dock.owner_of(id),
            }));
        }
        match target {
            Some(target) => self.dock.attach_view(id, target)?,
            None => self.dock.add_detached(id)?,
        }
        self.registry.register(id, kind, title.to_string());
        self.states.ensure(id);
        self.vcs.ensure(id);
        self.next_view = self.next_view.max(id.get());
        Ok(())
    }

    /// Highest view id ever minted or adopted — a read-only peek at
    /// the id high-water mark. The next
    /// [`spawn_view`](Self::spawn_view) /
    /// [`spawn_detached_view`](Self::spawn_detached_view) mints
    /// `watermark + 1`; callers that allocate ids through
    /// [`adopt_view`](Self::adopt_view) must stay above it so later
    /// spawns cannot collide, and must not reuse ids at or below it
    /// so removed views' ids are never recycled.
    #[must_use]
    pub fn view_id_watermark(&self) -> u64 {
        self.next_view
    }

    /// Removes a view entirely (dock, registry, state, links).
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] when the view is busy or unknown.
    pub fn remove_view(&mut self, view: ViewInstanceId) -> Result<(), WorkspaceError> {
        self.dock.remove_view(view)?;
        self.registry.unregister(view)?;
        self.states.remove(view);
        self.vcs.remove(view);
        self.links.set_group(view, None);
        if self.focus.active_view == Some(view) {
            self.focus.active_view = None;
        }
        Ok(())
    }

    /// The view's state.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn state(&self, view: ViewInstanceId) -> Result<&ViewState, WorkspaceError> {
        self.states
            .get(view)
            .ok_or(WorkspaceError::UnknownView(view))
    }

    /// Mutable view state.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn state_mut(&mut self, view: ViewInstanceId) -> Result<&mut ViewState, WorkspaceError> {
        self.states
            .get_mut(view)
            .ok_or(WorkspaceError::UnknownView(view))
    }

    /// The view's descriptor.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn descriptor(&self, view: ViewInstanceId) -> Result<&ViewDescriptor, WorkspaceError> {
        self.registry
            .descriptor(view)
            .ok_or(WorkspaceError::UnknownView(view))
    }

    // -- dock lifecycle ------------------------------------------------------

    /// One-shot move (menu-move / in-window drop).
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] as `DockWorkspace::move_view`.
    pub fn move_view(
        &mut self,
        view: ViewInstanceId,
        target: DockTarget,
    ) -> Result<MoveReport, WorkspaceError> {
        Ok(self.dock.move_view(view, target)?)
    }

    /// Two-phase move: `begin` marks the view transferring.
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] as `DockWorkspace::begin_move`.
    pub fn begin_transfer(
        &mut self,
        view: ViewInstanceId,
        target: DockTarget,
    ) -> Result<MoveToken, WorkspaceError> {
        Ok(self.dock.begin_move(view, target)?)
    }

    /// Commits a pending transfer.
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] as `DockWorkspace::commit_move` —
    /// including the automatic rollback (`MoveAborted` +
    /// `TargetUnavailable`) when the destination vanished.
    pub fn commit_transfer(&mut self, token: MoveToken) -> Result<MoveReport, WorkspaceError> {
        Ok(self.dock.commit_move(token)?)
    }

    /// Aborts a pending transfer — the view keeps source ownership.
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] as `DockWorkspace::abort_move`.
    pub fn abort_transfer(&mut self, token: MoveToken) -> Result<(), WorkspaceError> {
        Ok(self.dock.abort_move(token)?)
    }

    /// Detaches `view` into a brand-new host window.
    ///
    /// Ordering: the window must exist and be registered before the
    /// move can target it, so creation comes first — a host failure
    /// leaves the workspace untouched. A failed move after creation
    /// cleans the window back up so no orphan remains.
    ///
    /// # Errors
    /// [`WorkspaceError::Ui`] on host failure;
    /// [`WorkspaceError::Dock`] when the move itself fails.
    pub fn detach_to_new_window(
        &mut self,
        view: ViewInstanceId,
        title: &str,
    ) -> Result<WindowId, WorkspaceError> {
        let window = self.create_window(title, 1280, 800)?;
        match self.dock.move_view(view, DockTarget::Window(window)) {
            Ok(_) => Ok(window),
            Err(e) => {
                // Clean up the freshly created, still-empty window.
                let _ = self.host.destroy_window(window);
                let _ = self
                    .dock
                    .remove_window(window, WindowCloseDisposition::ReleaseViews);
                self.windows.retain(|w| w.id != window);
                Err(WorkspaceError::Dock(e))
            }
        }
    }

    /// Reattaches `view` (from the pool or another window) to `window`.
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] as `DockWorkspace::move_view`.
    pub fn reattach(
        &mut self,
        view: ViewInstanceId,
        window: WindowId,
    ) -> Result<MoveReport, WorkspaceError> {
        self.move_view(view, DockTarget::Window(window))
    }

    /// Detaches `view` into the detached pool (kept, reattachable).
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] as `DockWorkspace::detach_view`.
    pub fn detach_to_pool(&mut self, view: ViewInstanceId) -> Result<(), WorkspaceError> {
        Ok(self.dock.detach_view(view)?)
    }

    /// The first leaf's active view in `window` — used to derive the
    /// active view when a window gains native focus.
    #[must_use]
    pub fn active_view_in(&self, window: WindowId) -> Option<ViewInstanceId> {
        fn first_tabs(node: &ruviform_ui::dock::DockNode) -> Option<ViewInstanceId> {
            match node {
                ruviform_ui::dock::DockNode::Tabs { views, active } => views
                    .get(*active)
                    .copied()
                    .or_else(|| views.first().copied()),
                ruviform_ui::dock::DockNode::Split { children, .. } => {
                    children.iter().find_map(first_tabs)
                }
                // DockNode is non_exhaustive; unknown future nodes
                // simply have no active tab here.
                _ => None,
            }
        }
        self.dock.tree(window).and_then(|t| first_tabs(t.root()))
    }

    /// Drains dock events (test observability).
    pub fn drain_dock_events(&mut self) -> Vec<ruviform_ui::dock::DockEvent> {
        self.dock.drain_events()
    }

    // -- window events -----------------------------------------------------

    /// Handles a host event for `window`.
    ///
    /// - `Focused(true)` sets native focus and derives the active view
    ///   from the window's first active tab.
    /// - `CloseRequested` consults the window's veto flag; `Approve`
    ///   asks the host to close (the `Destroyed` event then performs
    ///   the dock disposition).
    /// - `Destroyed` removes the window from the dock with its
    ///   recorded disposition (sub-window policy default:
    ///   `DetachViews` — views are kept, reattachable).
    ///
    /// # Errors
    /// [`WorkspaceError`] variants propagated from dock/host calls.
    pub fn on_window_event(
        &mut self,
        window: WindowId,
        event: &HostEvent,
    ) -> Result<(), WorkspaceError> {
        match event {
            HostEvent::Focused(focused) => {
                if *focused {
                    self.focus.native_window = Some(window);
                    self.focus.active_view = self.active_view_in(window);
                } else if self.focus.native_window == Some(window) {
                    self.focus.native_window = None;
                }
            }
            HostEvent::Resized { width, height } => {
                if let Some(rec) = self.window_record_mut(window) {
                    rec.geometry.width = *width;
                    rec.geometry.height = *height;
                }
            }
            HostEvent::Moved { x, y } => {
                if let Some(rec) = self.window_record_mut(window) {
                    rec.geometry.x = *x;
                    rec.geometry.y = *y;
                }
            }
            HostEvent::CloseRequested => {
                let veto = self.window_record(window).is_some_and(|r| r.veto_close);
                if veto {
                    self.host
                        .respond_close(window, ruviform_ui::window::CloseDecision::Veto)?;
                } else {
                    self.host
                        .respond_close(window, ruviform_ui::window::CloseDecision::Approve)?;
                }
            }
            HostEvent::Destroyed => {
                let disposition = self
                    .window_record(window)
                    .map_or(WindowCloseDisposition::DetachViews, |r| r.disposition);
                if self.dock.is_registered(window) {
                    let views = self.dock.remove_window(window, disposition)?;
                    if matches!(disposition, WindowCloseDisposition::ReleaseViews) {
                        for v in views {
                            let _ = self.registry.unregister(v);
                            self.states.remove(v);
                            self.vcs.remove(v);
                            self.links.set_group(v, None);
                        }
                    }
                }
                self.windows.retain(|w| w.id != window);
                if self.focus.native_window == Some(window) {
                    self.focus.native_window = None;
                }
            }
            _ => {}
        }
        Ok(())
    }

    // -- selection + link dispatch ------------------------------------------

    /// Sets `view`'s object selection (domain inferred from the
    /// targets) and publishes it through the view's link group.
    /// Returns the delivered `(receiver, event)` pairs that were
    /// actually applied.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when `view` is unregistered;
    /// [`WorkspaceError::MixedDomains`] when `selection` mixes
    /// domains.
    pub fn select(
        &mut self,
        view: ViewInstanceId,
        selection: EditingSelection,
    ) -> Result<Vec<(ViewInstanceId, LinkEvent)>, WorkspaceError> {
        if !self.registry.contains(view) {
            return Err(WorkspaceError::UnknownView(view));
        }
        self.states
            .ensure(view)
            .object_selections
            .assign(selection)?;
        let Some((event, receivers)) = self.links.publish(
            view,
            self.states
                .ensure(view)
                .object_selections
                .active_selection(),
            self.session.project(),
        ) else {
            return Ok(Vec::new());
        };
        let mut delivered = Vec::new();
        for receiver in receivers {
            if self.links.deliver(event, receiver, &mut self.states) {
                delivered.push((receiver, event));
            }
        }
        Ok(delivered)
    }

    // -- editing model (ED-A) ------------------------------------------------

    /// Sets `view`'s time selection (or clears it).
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn set_time_selection(
        &mut self,
        view: ViewInstanceId,
        time: Option<TimeSelection>,
    ) -> Result<(), WorkspaceError> {
        self.state_mut(view)?.time_selection = time;
        Ok(())
    }

    /// Sets `view`'s lane scope.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn set_lane_scope(
        &mut self,
        view: ViewInstanceId,
        scope: LaneScope,
    ) -> Result<(), WorkspaceError> {
        self.state_mut(view)?.lane_scope = scope;
        Ok(())
    }

    /// Sets `view`'s active tool.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn set_tool(&mut self, view: ViewInstanceId, tool: Tool) -> Result<(), WorkspaceError> {
        self.state_mut(view)?.active_tool = tool;
        Ok(())
    }

    /// Binds `clip` to `view`'s editor surface — a display/edit
    /// binding only. Neither selection nor link state changes
    /// (ED-A-05).
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] / missing clip.
    pub fn open_in_editor(
        &mut self,
        view: ViewInstanceId,
        clip_id: synvelis_project::ClipId,
    ) -> Result<(), WorkspaceError> {
        let clip = self.session.project().clips.get(&clip_id).ok_or_else(|| {
            WorkspaceError::Project(synvelis_project::Error::NotFound(format!("clip {clip_id}")))
        })?;
        let binding = crate::selection::EditBinding {
            clip: clip_id,
            source: clip.source_id,
        };
        self.state_mut(view)?.edit_binding = Some(binding);
        Ok(())
    }

    // -- inspector (Issue #5 / UI-C5-03) -------------------------------------

    /// Inspects `target` on `view`: enters [`InspectionMode::Explore`],
    /// pushes the previous target onto the explore back-stack and
    /// clears the forward-stack. This is observation only — the
    /// editing selection, project and undo history are never touched,
    /// so an inspect click can never become an edit (UI-C5-02).
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn inspect(
        &mut self,
        view: ViewInstanceId,
        target: SemanticTarget,
    ) -> Result<(), WorkspaceError> {
        let focus = &mut self.state_mut(view)?.inspection_focus;
        if let Some(prev) = focus.target
            && prev != target
        {
            focus.explore_back.push(prev);
        }
        focus.explore_fwd.clear();
        focus.target = Some(target);
        focus.mode = InspectionMode::Explore;
        Ok(())
    }

    /// Sets `view`'s inspection mode. Leaving Explore clears its
    /// session-only history; the follow source is preserved so a
    /// later return to Follow resumes the same tracking.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn set_inspection_mode(
        &mut self,
        view: ViewInstanceId,
        mode: InspectionMode,
    ) -> Result<(), WorkspaceError> {
        let focus = &mut self.state_mut(view)?.inspection_focus;
        focus.mode = mode;
        if mode != InspectionMode::Explore {
            focus.explore_back.clear();
            focus.explore_fwd.clear();
        }
        Ok(())
    }

    /// Sets (or clears) what `view`'s Follow mode tracks. `None`
    /// keeps the default: follow every event the view's own link
    /// group delivers.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn set_follow_source(
        &mut self,
        view: ViewInstanceId,
        source: Option<FollowSource>,
    ) -> Result<(), WorkspaceError> {
        self.state_mut(view)?.inspection_focus.follow_source = source;
        Ok(())
    }

    /// Steps `view`'s inspection target back through the explore
    /// history. Only meaningful in [`InspectionMode::Explore`];
    /// elsewhere or with an empty stack this is a no-op. The editing
    /// selection is never touched.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn explore_back(&mut self, view: ViewInstanceId) -> Result<(), WorkspaceError> {
        let focus = &mut self.state_mut(view)?.inspection_focus;
        if focus.mode != InspectionMode::Explore {
            return Ok(());
        }
        let Some(prev) = focus.explore_back.pop() else {
            return Ok(());
        };
        if let Some(cur) = focus.target {
            focus.explore_fwd.push(cur);
        }
        focus.target = Some(prev);
        Ok(())
    }

    /// Steps `view`'s inspection target forward through the explore
    /// history — the mirror of [`explore_back`](Self::explore_back).
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when unregistered.
    pub fn explore_forward(&mut self, view: ViewInstanceId) -> Result<(), WorkspaceError> {
        let focus = &mut self.state_mut(view)?.inspection_focus;
        if focus.mode != InspectionMode::Explore {
            return Ok(());
        }
        let Some(next) = focus.explore_fwd.pop() else {
            return Ok(());
        };
        if let Some(cur) = focus.target {
            focus.explore_back.push(cur);
        }
        focus.target = Some(next);
        Ok(())
    }

    // -- Control Layer edge gestures (UI-C5-02) ------------------------------

    /// Begins a Control Layer edge gesture on `view`: a drag from a
    /// [`ControlEntityRef::ModulationSource`] to a
    /// [`ControlEntityRef::Parameter`]. The UI anchor refs are
    /// resolved to typed command targets *once*, here — the fixation
    /// then carries ids, revision and window epoch, so nothing later
    /// (selection, focus, dock, link events) can retarget it.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`], [`WorkspaceError::ViewBusy`]
    /// mid-transfer, [`WorkspaceError::GestureActive`] when any
    /// gesture already runs on the view, or
    /// [`WorkspaceError::InvalidGesture`] for non connect endpoints.
    pub fn control_edge_begin(
        &mut self,
        view: ViewInstanceId,
        source: ControlEntityRef,
        target: ControlEntityRef,
    ) -> Result<(), WorkspaceError> {
        let state = self.state(view)?;
        if state.gesture.is_some()
            || state.control_edge_gesture.is_some()
            || state.routing_gesture.is_some()
        {
            return Err(WorkspaceError::GestureActive(view));
        }
        if self.dock.is_transferring(view) {
            return Err(WorkspaceError::ViewBusy(view));
        }
        let (ControlEntityRef::ModulationSource(source), ControlEntityRef::Parameter(target)) =
            (source, target)
        else {
            return Err(WorkspaceError::InvalidGesture(
                "control edge requires ModulationSource -> Parameter".to_string(),
            ));
        };
        let gesture = ControlEdgeGesture {
            fixation: ControlEdgeFixation {
                view,
                source,
                target,
                revision: self.session.revision(),
                window: self.dock.owner_of(view),
            },
            preview: ControlEdgePreview::default(),
        };
        self.state_mut(view)?.control_edge_gesture = Some(gesture);
        Ok(())
    }

    /// Updates the running edge gesture's preview values. Project
    /// state is never touched — a preview cannot create an undo
    /// boundary.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] / [`WorkspaceError::NoGesture`].
    pub fn control_edge_preview(
        &mut self,
        view: ViewInstanceId,
        preview: ControlEdgePreview,
    ) -> Result<(), WorkspaceError> {
        let state = self.state_mut(view)?;
        let Some(gesture) = state.control_edge_gesture.as_mut() else {
            return Err(WorkspaceError::NoGesture(view));
        };
        gesture.preview = preview;
        Ok(())
    }

    /// Commits `view`'s running edge gesture as one
    /// `AddModulationAssignment` at the fixated revision, then
    /// distributes the new revision. A moved window epoch (dock
    /// transfer/detach) or an advanced project revision drops the
    /// gesture as [`ControlEdgeOutcome::Stale`] — it never retargets
    /// to whatever is current now.
    ///
    /// # Errors
    /// [`WorkspaceError::NoGesture`] when nothing runs;
    /// [`WorkspaceError::Project`] on non-stale project rejection
    /// (the gesture is kept so the caller can inspect or cancel).
    pub fn control_edge_commit(
        &mut self,
        view: ViewInstanceId,
    ) -> Result<ControlEdgeOutcome, WorkspaceError> {
        let state = self.state(view)?;
        let Some(gesture) = state.control_edge_gesture.clone() else {
            return Err(WorkspaceError::NoGesture(view));
        };
        if self.dock.is_transferring(view) || self.dock.owner_of(view) != gesture.fixation.window {
            self.state_mut(view)?.control_edge_gesture = None;
            return Ok(ControlEdgeOutcome::Stale);
        }
        let op = ProjectOp::AddModulationAssignment {
            source: gesture.fixation.source,
            target: gesture.fixation.target,
            depth: gesture.preview.depth,
            depth_source: None,
            polarity: gesture.preview.polarity,
            transform: Transform::Identity,
            enabled: true,
        };
        match self.session.apply(op, gesture.fixation.revision) {
            Ok(revision) => {
                self.distribute_revision(revision);
                self.state_mut(view)?.control_edge_gesture = None;
                Ok(ControlEdgeOutcome::Applied { revision })
            }
            Err(synvelis_project::Error::StaleRevision { .. }) => {
                self.state_mut(view)?.control_edge_gesture = None;
                Ok(ControlEdgeOutcome::Stale)
            }
            Err(e) => Err(WorkspaceError::Project(e)),
        }
    }

    /// Cancels `view`'s running edge gesture — the preview is dropped
    /// and the project (and its undo history) is untouched.
    ///
    /// # Errors
    /// [`WorkspaceError::NoGesture`] when nothing runs.
    pub fn control_edge_cancel(
        &mut self,
        view: ViewInstanceId,
        reason: CancelReason,
    ) -> Result<ControlEdgeOutcome, WorkspaceError> {
        let state = self.state_mut(view)?;
        if state.control_edge_gesture.take().is_none() {
            return Err(WorkspaceError::NoGesture(view));
        }
        Ok(ControlEdgeOutcome::Cancelled { reason })
    }

    // -- Signal Flow inspection focus (SF-UX-04) -----------------------
    //
    // A routing entity inspected by a Signal Flow view — independent
    // of the arrangement `inspection_focus`/`SelectionDomain` pair.
    // The same Follow/Explore/Pinned semantics apply; Explore history
    // is session-only, and only Follow mode is retargeted by link
    // delivery (see [`crate::link::LinkGroupManager::deliver`]).

    fn signal_flow_state_mut(
        &mut self,
        view: ViewInstanceId,
    ) -> Result<&mut SignalFlowViewState, WorkspaceError> {
        Ok(self
            .state_mut(view)?
            .signal_flow
            .get_or_insert_with(SignalFlowViewState::default))
    }

    /// Inspects a Signal Flow routing entity — equivalent to the
    /// arrangement [`Self::inspect`] but writing the view-local
    /// Signal Flow focus, never `inspection_focus` or selection.
    /// Pushes the previous target onto the Explore back-stack.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] for an unregistered view.
    pub fn signal_flow_inspect(
        &mut self,
        view: ViewInstanceId,
        entity: SignalFlowEntityRef,
    ) -> Result<(), WorkspaceError> {
        let focus = &mut self.signal_flow_state_mut(view)?.inspection;
        if focus.target != Some(entity) {
            if let Some(prev) = focus.target.replace(entity) {
                focus.explore_back.push(prev);
            }
            focus.explore_fwd.clear();
        }
        focus.mode = InspectionMode::Explore;
        Ok(())
    }

    /// Sets the Signal Flow inspection mode. Leaving `Explore`
    /// clears its session history, matching [`Self::set_inspection_mode`].
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] for an unregistered view.
    pub fn signal_flow_set_inspection_mode(
        &mut self,
        view: ViewInstanceId,
        mode: InspectionMode,
    ) -> Result<(), WorkspaceError> {
        let focus = &mut self.signal_flow_state_mut(view)?.inspection;
        focus.mode = mode;
        if mode != InspectionMode::Explore {
            focus.explore_back.clear();
            focus.explore_fwd.clear();
        }
        Ok(())
    }

    /// Sets the Signal Flow Follow-mode source filter — `Some`
    /// narrows adoption to events carrying that source.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] for an unregistered view.
    pub fn signal_flow_set_follow_source(
        &mut self,
        view: ViewInstanceId,
        source: Option<FollowSource>,
    ) -> Result<(), WorkspaceError> {
        self.signal_flow_state_mut(view)?.inspection.follow_source = source;
        Ok(())
    }

    /// Steps the Signal Flow Explore history back one entity.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] for an unregistered view.
    pub fn signal_flow_explore_back(&mut self, view: ViewInstanceId) -> Result<(), WorkspaceError> {
        let focus = &mut self.signal_flow_state_mut(view)?.inspection;
        if let Some(prev) = focus.explore_back.pop()
            && let Some(current) = focus.target.replace(prev)
        {
            focus.explore_fwd.push(current);
        }
        Ok(())
    }

    /// Steps the Signal Flow Explore history forward one entity.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] for an unregistered view.
    pub fn signal_flow_explore_forward(
        &mut self,
        view: ViewInstanceId,
    ) -> Result<(), WorkspaceError> {
        let focus = &mut self.signal_flow_state_mut(view)?.inspection;
        if let Some(next) = focus.explore_fwd.pop()
            && let Some(current) = focus.target.replace(next)
        {
            focus.explore_back.push(current);
        }
        Ok(())
    }

    // -- Signal Flow routing gestures (SF-UX-03) ------------------------
    //
    // Port-drag connect: fixation at Begin, dry-run assessment on
    // hover, fixed target at drop, one `ProjectOp::Routing` at confirm.

    /// Begins a Signal Flow routing gesture fixated on `source` —
    /// view, project revision, port-capability generation and window
    /// epoch are all captured now and can never be retargeted.
    /// Refused while any gesture family is active or the view is
    /// mid-transfer; the port must exist in the routing model.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`], [`WorkspaceError::GestureActive`]
    /// when any gesture family already runs on this view,
    /// [`WorkspaceError::ViewBusy`] mid-transfer, or
    /// [`WorkspaceError::InvalidGesture`] when `source` is not a port
    /// in the routing model.
    pub fn routing_gesture_begin(
        &mut self,
        view: ViewInstanceId,
        source: SemanticPortId,
    ) -> Result<RoutingGestureFixation, WorkspaceError> {
        let state = self.state(view)?;
        if state.gesture.is_some()
            || state.control_edge_gesture.is_some()
            || state.routing_gesture.is_some()
        {
            return Err(WorkspaceError::GestureActive(view));
        }
        if self.dock.is_transferring(view) {
            return Err(WorkspaceError::ViewBusy(view));
        }
        if !self.session.project().routing.ports.contains_key(&source) {
            return Err(WorkspaceError::InvalidGesture(format!(
                "routing gesture source port {source} does not exist"
            )));
        }
        let fixation = RoutingGestureFixation {
            view,
            source,
            revision: self.session.revision(),
            capability_generation: port_capability_generation(self.session.project()),
            window: self.dock.owner_of(view),
        };
        self.state_mut(view)?.routing_gesture = Some(RoutingGesture {
            fixation: fixation.clone(),
            preview: RoutingGesturePreview {
                candidate: None,
                assessment: None,
                dropped_target: None,
            },
        });
        Ok(fixation)
    }

    /// Returns the running routing gesture's fixation, if any.
    #[must_use]
    pub fn routing_gesture(&self, view: ViewInstanceId) -> Option<&RoutingGestureFixation> {
        self.states
            .get(view)
            .and_then(|s| s.routing_gesture.as_ref())
            .map(|g| &g.fixation)
    }

    /// Returns the running routing gesture's preview, if any.
    #[must_use]
    pub fn routing_gesture_preview(&self, view: ViewInstanceId) -> Option<&RoutingGesturePreview> {
        self.states
            .get(view)
            .and_then(|s| s.routing_gesture.as_ref())
            .map(|g| &g.preview)
    }

    /// Hover update — recomputes the dry-run assessment for `target`
    /// (`None` = empty canvas). Rejected once the target is fixed by
    /// drop; the project is never mutated by preview.
    ///
    /// # Errors
    /// [`WorkspaceError::NoGesture`] when nothing runs,
    /// [`WorkspaceError::UnknownView`] for an unknown view.
    pub fn routing_gesture_hover(
        &mut self,
        view: ViewInstanceId,
        target: Option<SemanticPortId>,
    ) -> Result<Option<RoutingPortAssessment>, WorkspaceError> {
        let gesture = self
            .states
            .get(view)
            .ok_or(WorkspaceError::UnknownView(view))?
            .routing_gesture
            .clone()
            .ok_or(WorkspaceError::NoGesture(view))?;
        if gesture.preview.dropped_target.is_some() {
            return Err(WorkspaceError::InvalidGesture(
                "routing gesture target already fixed by drop".to_string(),
            ));
        }
        let assessment = target.map(|port| {
            let role = self
                .session
                .project()
                .routing
                .ports
                .get(&port)
                .map_or(PortRole::Main, |p| p.role);
            assess_connection(self.session.project(), &gesture.fixation, port, role)
        });
        if let Some(state) = self.states.get_mut(view)
            && let Some(g) = state.routing_gesture.as_mut()
        {
            g.preview.candidate = target;
            g.preview.assessment.clone_from(&assessment);
        }
        Ok(assessment)
    }

    /// Drop — fixes the previewed candidate as the immutable target.
    /// Dropping on empty canvas (no candidate) ends the gesture
    /// cancelled; the project is untouched either way. After a
    /// successful drop only [`Self::routing_gesture_confirm`] or
    /// [`Self::routing_gesture_cancel`] may end the gesture — no
    /// later hover, receipt, or focus event may swap the target.
    ///
    /// # Errors
    /// [`WorkspaceError::NoGesture`] when nothing runs,
    /// [`WorkspaceError::UnknownView`] for an unknown view.
    pub fn routing_gesture_drop(
        &mut self,
        view: ViewInstanceId,
    ) -> Result<Option<SemanticPortId>, WorkspaceError> {
        let gesture = self
            .states
            .get(view)
            .ok_or(WorkspaceError::UnknownView(view))?
            .routing_gesture
            .clone()
            .ok_or(WorkspaceError::NoGesture(view))?;
        if let Some(target) = gesture.preview.candidate {
            if let Some(g) = self.state_mut(view)?.routing_gesture.as_mut() {
                g.preview.dropped_target = Some(target);
            }
            return Ok(Some(target));
        }
        self.state_mut(view)?.routing_gesture = None;
        Ok(None)
    }

    /// Confirm — applies the fixated routing gesture as one
    /// `ProjectOp::Routing` at the fixated revision with the explicit
    /// channel-map consent (`None` = no conversion approved; a
    /// layout-mismatched pair then fails validation and the gesture
    /// stays alive for a corrected retry or cancel). A moved window
    /// epoch, an advanced project revision, or a moved port
    /// capability generation drops the gesture as
    /// [`RoutingGestureOutcome::Stale`] — never a retarget.
    ///
    /// # Errors
    /// [`WorkspaceError::NoGesture`] when nothing runs,
    /// [`WorkspaceError::InvalidGesture`] when no target was dropped,
    /// or [`WorkspaceError::Project`] on a validation rejection that
    /// is not staleness (the gesture stays alive).
    pub fn routing_gesture_confirm(
        &mut self,
        view: ViewInstanceId,
        channel_map: Option<&ExplicitChannelMap>,
    ) -> Result<RoutingGestureOutcome, WorkspaceError> {
        let gesture = self
            .states
            .get(view)
            .ok_or(WorkspaceError::UnknownView(view))?
            .routing_gesture
            .clone()
            .ok_or(WorkspaceError::NoGesture(view))?;
        let fixation = gesture.fixation.clone();
        let Some(target) = gesture.preview.dropped_target else {
            return Err(WorkspaceError::InvalidGesture(
                "routing gesture has no dropped target".to_string(),
            ));
        };
        // Stale checks — window epoch moved (dock transfer), project
        // revision advanced, or plugin port reconfigure moved the
        // capability generation. All drop the gesture; none retarget.
        if self.dock.is_transferring(view) || self.dock.owner_of(view) != fixation.window {
            self.state_mut(view)?.routing_gesture = None;
            return Ok(RoutingGestureOutcome::Stale);
        }
        if port_capability_generation(self.session.project()) != fixation.capability_generation {
            self.state_mut(view)?.routing_gesture = None;
            return Ok(RoutingGestureOutcome::Stale);
        }
        let Some(port) = self.session.project().routing.ports.get(&target) else {
            // The target port vanished — the project moved under the
            // gesture, so the honest outcome is stale, not a guess.
            self.state_mut(view)?.routing_gesture = None;
            return Ok(RoutingGestureOutcome::Stale);
        };
        let role = port.role;
        let intent = routing_gesture_intent(
            format!(
                "signal-flow-routing-{}-{}",
                fixation.view.get(),
                fixation.revision.0
            ),
            &fixation,
            target,
            role,
            channel_map,
        );
        match self
            .session
            .apply(ProjectOp::Routing { intent }, fixation.revision)
        {
            Ok(revision) => {
                self.distribute_revision(revision);
                self.state_mut(view)?.routing_gesture = None;
                Ok(RoutingGestureOutcome::Applied { revision })
            }
            Err(synvelis_project::Error::StaleRevision { .. }) => {
                self.state_mut(view)?.routing_gesture = None;
                Ok(RoutingGestureOutcome::Stale)
            }
            // Any other rejection — including RequiresUserDecision on
            // an unconsented channel-map — keeps the gesture alive so
            // the caller can retry with consent or cancel. The
            // capability generation was verified against the live
            // project immediately above, so StalePortCapability is
            // unreachable here.
            Err(e) => Err(WorkspaceError::Project(e)),
        }
    }

    /// Cancels the running routing gesture — preview discarded, the
    /// project untouched (no undo boundary).
    ///
    /// # Errors
    /// [`WorkspaceError::NoGesture`] when nothing runs,
    /// [`WorkspaceError::UnknownView`] for an unknown view.
    pub fn routing_gesture_cancel(
        &mut self,
        view: ViewInstanceId,
        reason: CancelReason,
    ) -> Result<RoutingGestureOutcome, WorkspaceError> {
        self.states
            .get_mut(view)
            .ok_or(WorkspaceError::UnknownView(view))?
            .routing_gesture
            .take()
            .map(|_| RoutingGestureOutcome::Cancelled { reason })
            .ok_or(WorkspaceError::NoGesture(view))
    }

    /// The outcome of a `pointer_down`.
    /// Resolves the active tool over `probe` and, on success, fixes
    /// the gesture: operation, target set, project revision and
    /// shared time domain are frozen for the gesture's lifetime.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`], [`WorkspaceError::GestureActive`]
    /// when a gesture is already running on this view, or
    /// [`WorkspaceError::MixedTimeDomains`] when the fixed targets
    /// disagree on their time domain.
    pub fn pointer_down(
        &mut self,
        view: ViewInstanceId,
        probe: &HitProbe,
    ) -> Result<PointerOutcome, WorkspaceError> {
        let state = self.state(view)?;
        if state.gesture.is_some()
            || state.control_edge_gesture.is_some()
            || state.routing_gesture.is_some()
        {
            return Err(WorkspaceError::GestureActive(view));
        }
        let selection = state.object_selections.active_selection().targets.clone();
        let resolution = resolve_tool(state.active_tool, probe, &selection);
        match resolution {
            ToolResolution::Ambiguous {
                candidates,
                suggested,
            } => Ok(PointerOutcome::Ambiguous {
                candidates,
                suggested,
            }),
            ToolResolution::NoGesture => Ok(PointerOutcome::NoGesture),
            ToolResolution::Resolved { operation, targets } => {
                let has_clip = targets.iter().any(|t| matches!(t, SemanticTarget::Clip(_)));
                let domain =
                    if matches!(operation, EditOperation::Marquee | EditOperation::DrawClip)
                        || !has_clip
                    {
                        // Target-free and clip-free operations take the
                        // probe's own domain — a media marquee is a media
                        // gesture, and the fixation's domain tag always
                        // matches its origin.
                        probe.position.domain()
                    } else {
                        edit::gesture_domain(self.session.project(), &targets)?
                    };
                // ED-B-06: when clip targets fix the domain, a probe
                // reporting another domain is a start-time rejection —
                // never a silent reinterpretation of its position.
                if has_clip && probe.position.domain() != domain {
                    return Err(WorkspaceError::MixedTimeDomains(
                        "probe position domain disagrees with the gesture domain".to_string(),
                    ));
                }
                let gesture = Gesture {
                    fixation: GestureFixation {
                        operation,
                        targets,
                        revision: self.session.revision(),
                        domain,
                        origin: probe.position,
                    },
                    preview: GesturePreview {
                        position: probe.position,
                        delta: 0,
                        snap: None,
                    },
                };
                self.state_mut(view)?.gesture = Some(gesture);
                Ok(PointerOutcome::GestureStarted)
            }
        }
    }

    /// Moves the running gesture's preview to `position`: the delta
    /// recomputes (through snap resolution on the primary anchor for
    /// move/trim gestures) while the fixation stays untouched.
    /// `position` must report the fixated domain — a mismatched probe
    /// is rejected, never reinterpreted.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] / [`WorkspaceError::NoGesture`] /
    /// [`WorkspaceError::MixedTimeDomains`] on a domain-mismatched
    /// position.
    pub fn drag_update(
        &mut self,
        view: ViewInstanceId,
        position: EditTimePoint,
        env: &SnapEnvironment,
    ) -> Result<(), WorkspaceError> {
        let state = self.state(view)?;
        let Some(gesture) = state.gesture.as_ref() else {
            return Err(WorkspaceError::NoGesture(view));
        };
        let fixation = gesture.fixation.clone();
        let prev_snap = gesture.preview.snap;
        let snap_cfg = state.snap;
        let raw_delta = position.delta_since(&fixation.origin)?;
        let mut delta = raw_delta;
        let mut winner = None;
        if matches!(
            fixation.operation,
            EditOperation::MoveClips | EditOperation::TrimClip { .. }
        ) && let Some(edge_start) = primary_snap_position(self.session.project(), &fixation)
        {
            let snapped = resolve_snap(
                &snap_cfg,
                env,
                edge_start
                    .checked_add(raw_delta)
                    .ok_or(WorkspaceError::Project(synvelis_project::Error::Overflow))?,
                fixation.domain,
                prev_snap,
            );
            if let Some(w) = snapped {
                delta = w
                    .position
                    .checked_sub(edge_start)
                    .ok_or(synvelis_project::Error::Overflow)?;
                winner = Some(w);
            }
        }
        let gesture = self
            .state_mut(view)?
            .gesture
            .as_mut()
            .ok_or(WorkspaceError::NoGesture(view))?;
        gesture.preview.position = position;
        gesture.preview.delta = delta;
        gesture.preview.snap = winner;
        Ok(())
    }

    /// Commits `view`'s running gesture: build the single effect,
    /// apply it through the session (expected revision = the fixated
    /// one — a project change meanwhile rejects stale), distribute
    /// the revision, drop the gesture.
    ///
    /// # Errors
    /// [`WorkspaceError::NoGesture`], validation/stale errors from the
    /// controller (gesture kept so the caller can inspect or cancel).
    pub fn commit_gesture(
        &mut self,
        view: ViewInstanceId,
    ) -> Result<GestureOutcome, WorkspaceError> {
        let state = self.state(view)?;
        let Some(gesture) = state.gesture.clone() else {
            return Err(WorkspaceError::NoGesture(view));
        };
        let lane_scope = state.lane_scope.clone();
        match edit::build_commit(self.session.project(), &gesture, &lane_scope)? {
            CommitEffect::ProjectOp(op) => {
                let expected = gesture.fixation.revision;
                match self.session.apply(op, expected) {
                    Ok(revision) => {
                        self.distribute_revision(revision);
                        self.state_mut(view)?.gesture = None;
                        Ok(GestureOutcome::Applied { revision })
                    }
                    Err(e) => Err(WorkspaceError::Project(e)),
                }
            }
            CommitEffect::Selection { domain, targets } => {
                let state = self.state_mut(view)?;
                state
                    .object_selections
                    .set(domain, EditingSelection { targets })?;
                state.gesture = None;
                Ok(GestureOutcome::SelectionChanged)
            }
            CommitEffect::NoChange => {
                self.state_mut(view)?.gesture = None;
                Ok(GestureOutcome::NoChange)
            }
        }
    }

    /// Cancels `view`'s running gesture — the preview is dropped and
    /// the project is untouched.
    ///
    /// # Errors
    /// [`WorkspaceError::NoGesture`].
    pub fn cancel_gesture(
        &mut self,
        view: ViewInstanceId,
        reason: CancelReason,
    ) -> Result<GestureOutcome, WorkspaceError> {
        let state = self.state_mut(view)?;
        if state.gesture.take().is_none() {
            return Err(WorkspaceError::NoGesture(view));
        }
        Ok(GestureOutcome::Cancelled { reason })
    }

    /// Switches the session input context. Leaving `View` drops every
    /// running gesture (`InputScopeLost` semantics — the preview is
    /// discarded, the project untouched).
    pub fn set_input_context(&mut self, input: InputContext) {
        if input != InputContext::View && self.focus.input == InputContext::View {
            for id in self.states.ids() {
                if let Some(state) = self.states.get_mut(id) {
                    state.gesture = None;
                    state.control_edge_gesture = None;
                    state.routing_gesture = None;
                }
            }
        }
        self.focus.input = input;
    }

    /// Replays `event` to `receiver` — tests proving dedup re-deliver
    /// the same event. Returns whether it applied.
    pub fn redeliver(&mut self, event: LinkEvent, receiver: ViewInstanceId) -> bool {
        self.links.deliver(event, receiver, &mut self.states)
    }

    // -- command routing -----------------------------------------------------

    /// Routes `ctx` per the WS-A contract: suppression → target
    /// resolution → execution. See [`crate::command`] for the rules.
    #[allow(clippy::too_many_lines)] // one lane per kind keeps the routing table flat
    pub fn dispatch(&mut self, ctx: CommandContext) -> CommandOutcome {
        let Some(spec) = spec_for(&ctx.id) else {
            return CommandOutcome::Rejected {
                error: WorkspaceError::UnknownCommand(ctx.id),
            };
        };
        if ctx.input.captures_input()
            && matches!(
                spec.kind,
                CommandKind::Project | CommandKind::Transport | CommandKind::Editing
            )
        {
            return CommandOutcome::Suppressed {
                reason: "input context captures daw commands",
            };
        }
        match spec.kind {
            CommandKind::Undo => self.dispatch_undo(&ctx),
            CommandKind::Application => {
                self.host.exit();
                CommandOutcome::Applied { revision: None }
            }
            CommandKind::Transport => {
                match ctx.id.as_str() {
                    "transport.play" => self.session.set_playing(true),
                    "transport.stop" => self.session.set_playing(false),
                    _ => {}
                }
                CommandOutcome::Applied { revision: None }
            }
            CommandKind::View => {
                if ctx.id == "tool.activate" {
                    let Some(tool) = ctx.arg.as_deref().and_then(Tool::parse) else {
                        return CommandOutcome::Unresolved {
                            reason: "tool.activate without a tool name".to_string(),
                        };
                    };
                    let Some(view) = ctx.origin_view else {
                        return CommandOutcome::Unresolved {
                            reason: "tool.activate without an origin view".to_string(),
                        };
                    };
                    return match self.set_tool(view, tool) {
                        Ok(()) => CommandOutcome::Applied { revision: None },
                        Err(e) => CommandOutcome::Rejected { error: e },
                    };
                }
                let Some(target) = self.resolve_target(&ctx) else {
                    return CommandOutcome::Unresolved {
                        reason: "view command without a resolvable view target".to_string(),
                    };
                };
                let ResolvedTarget::View(view) = target else {
                    return CommandOutcome::Unresolved {
                        reason: "view command target is not a view".to_string(),
                    };
                };
                match self.dock.activate_view(view) {
                    Ok(()) => CommandOutcome::Applied { revision: None },
                    Err(e) => CommandOutcome::Rejected {
                        error: WorkspaceError::Dock(e),
                    },
                }
            }
            CommandKind::Project => {
                let op = match ctx.id.as_str() {
                    "project.rename" => ProjectOp::SetProjectName {
                        name: ctx.arg.clone().unwrap_or_default(),
                    },
                    _ => {
                        return CommandOutcome::Rejected {
                            error: WorkspaceError::UnknownCommand(ctx.id),
                        };
                    }
                };
                self.apply_project_op(op, &ctx)
            }
            CommandKind::Editing => {
                // A mid-transfer view is edit-locked.
                if let Some(v) = ctx.origin_view
                    && self.dock.is_transferring(v)
                {
                    return CommandOutcome::Rejected {
                        error: WorkspaceError::ViewBusy(v),
                    };
                }
                // Range delete resolves from the origin view's time
                // selection + lane scope — an explicit command, never
                // implied by object selection.
                if ctx.id == "clips.delete-range" {
                    return self.dispatch_delete_range(&ctx);
                }
                // Selection-derived targets must come from the
                // command's required domain — a selection in another
                // domain leaves the command unresolved (ED-A-01).
                if let Some(domain) = spec.required_domain
                    && ctx.explicit_targets.is_empty()
                {
                    let matches_domain = ctx.selection.as_ref().map_or_else(
                        || {
                            ctx.origin_view
                                .and_then(|v| self.states.get(v))
                                .is_some_and(|s| s.object_selections.active == domain)
                        },
                        |sel| {
                            sel.targets
                                .first()
                                .is_some_and(|t| SelectionDomain::of(*t) == domain)
                        },
                    );
                    if !matches_domain {
                        return CommandOutcome::Unresolved {
                            reason: format!("{} requires a {domain:?} selection", ctx.id),
                        };
                    }
                }
                let Some(ResolvedTarget::Entity(entity)) = self.resolve_target(&ctx) else {
                    return CommandOutcome::Unresolved {
                        reason: format!("{} has no resolvable entity target", ctx.id),
                    };
                };
                let Some(op) = build_project_op(&ctx.id, entity) else {
                    return CommandOutcome::Unresolved {
                        reason: format!("{} cannot take target {entity:?}", ctx.id),
                    };
                };
                self.apply_project_op(op, &ctx)
            }
        }
    }

    /// Undo/redo lane: `View` goes through the session's project
    /// history (engine hook advances); `TextInput`/`Ime` land on the
    /// text lane stub without touching the project; `Modal`/
    /// `PluginGui` are swallowed.
    fn dispatch_undo(&mut self, ctx: &CommandContext) -> CommandOutcome {
        match ctx.input {
            InputContext::View => {
                let done = if ctx.id == "edit.undo" {
                    self.session.undo()
                } else {
                    self.session.redo()
                };
                match done {
                    Ok(true) => {
                        let revision = self.session.revision();
                        self.distribute_revision(revision);
                        CommandOutcome::Applied {
                            revision: Some(revision),
                        }
                    }
                    Ok(false) => CommandOutcome::Applied { revision: None },
                    Err(e) => CommandOutcome::Rejected {
                        error: WorkspaceError::Project(e),
                    },
                }
            }
            InputContext::TextInput | InputContext::Ime => {
                self.text_lane_undos += 1;
                CommandOutcome::Applied { revision: None }
            }
            _ => CommandOutcome::Suppressed {
                reason: "input context captures undo",
            },
        }
    }

    /// `clips.delete-range`: build [`ProjectOp::RemoveClipsInRange`]
    /// from the origin view's musical `TimeSelection` + `LaneScope`.
    fn dispatch_delete_range(&mut self, ctx: &CommandContext) -> CommandOutcome {
        let Some(view) = ctx.origin_view else {
            return CommandOutcome::Unresolved {
                reason: "clips.delete-range without an origin view".to_string(),
            };
        };
        let Some(state) = self.states.get(view) else {
            return CommandOutcome::Unresolved {
                reason: "clips.delete-range on an unknown view".to_string(),
            };
        };
        let Some(TimeSelection::Musical(range)) = state.time_selection else {
            return CommandOutcome::Unresolved {
                reason: "clips.delete-range requires a musical time selection".to_string(),
            };
        };
        if state.lane_scope.is_empty() {
            return CommandOutcome::Unresolved {
                reason: "clips.delete-range with an empty lane scope".to_string(),
            };
        }
        self.apply_project_op(
            ProjectOp::RemoveClipsInRange {
                lanes: state.lane_scope.lanes.clone(),
                range,
            },
            ctx,
        )
    }

    /// Applies `op` via the session and distributes the new revision
    /// to every registered view's `revision_inbox` + headless render.
    fn apply_project_op(&mut self, op: ProjectOp, ctx: &CommandContext) -> CommandOutcome {
        let expected = ctx
            .expected_revision
            .unwrap_or_else(|| self.session.revision());
        match self.session.apply(op, expected) {
            Ok(revision) => {
                self.distribute_revision(revision);
                CommandOutcome::Applied {
                    revision: Some(revision),
                }
            }
            Err(e) => CommandOutcome::Rejected {
                error: WorkspaceError::Project(e),
            },
        }
    }

    /// Pushes `revision` to every view — the single-source-of-truth
    /// broadcast that keeps all windows' projections in step.
    fn distribute_revision(&mut self, revision: ProjectRevision) {
        for view in self.registry.ids() {
            if let Some(state) = self.states.get_mut(view) {
                state.revision_inbox.push(revision);
            }
            if let Some(headless) = self.registry.headless_mut(view) {
                headless.render(revision);
            }
        }
    }

    /// Target resolution: explicit targets first, then the origin
    /// view's selection snapshot (or its current selection when no
    /// snapshot was captured). Never another view's selection.
    fn resolve_target(&self, ctx: &CommandContext) -> Option<ResolvedTarget> {
        if let Some(first) = ctx.explicit_targets.first() {
            return Some(ResolvedTarget::Entity(*first));
        }
        if let Some(selection) = &ctx.selection {
            return selection.primary().map(ResolvedTarget::Entity);
        }
        let view = ctx.origin_view?;
        if matches!(spec_for(&ctx.id)?.kind, CommandKind::View) {
            return Some(ResolvedTarget::View(view));
        }
        self.states
            .get(view)
            .and_then(|s| s.object_selections.primary())
            .map(ResolvedTarget::Entity)
    }

    // -- persistence ---------------------------------------------------------

    /// Captures the full workspace snapshot (layout + views + links).
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] when the capture inputs disagree.
    pub fn capture(&self) -> Result<WorkspaceSnapshot, WorkspaceError> {
        let ids: Vec<WindowId> = self.windows.iter().map(|w| w.id).collect();
        let geometry: Vec<WindowGeometry> =
            self.windows.iter().map(|w| w.geometry.clone()).collect();
        let layout = LayoutSnapshot::capture(&ids, &geometry, &self.dock)?;
        Ok(WorkspaceSnapshot {
            schema: WORKSPACE_SCHEMA,
            layout,
            window_titles: self.windows.iter().map(|w| w.title.clone()).collect(),
            views: self
                .registry
                .ids()
                .iter()
                .filter_map(|id| {
                    let d = self.registry.descriptor(*id)?;
                    Some(ViewRecord {
                        id: id.get(),
                        kind: d.kind,
                        title: d.title.clone(),
                        state: self.states.get(*id).cloned().unwrap_or_default(),
                    })
                })
                .collect(),
            links: self
                .links
                .assignments()
                .iter()
                .map(|(v, g)| LinkRecord {
                    view: v.get(),
                    group: g.0,
                })
                .collect(),
        })
    }

    /// Restores `snap` into this (empty) workspace. Window placements
    /// are applied in snapshot order to freshly created windows; view
    /// ids, state and link groups are re-registered as recorded. Layout
    /// corruption falls back to `LayoutSnapshot::restore_or_default`'s
    /// safe empty layout — views then land in the detached pool.
    ///
    /// # Errors
    /// [`WorkspaceError::Ui`] on host failure;
    /// [`WorkspaceError::Dock`] on inconsistent restored state.
    pub fn restore(
        &mut self,
        snap: &WorkspaceSnapshot,
        monitors: &[MonitorInfo],
    ) -> Result<(), WorkspaceError> {
        let layout = snap.layout.restore_or_default(monitors);
        for (i, placement) in layout.placements.iter().enumerate() {
            let title = snap
                .window_titles
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("window {}", i + 1));
            let window = self.create_window_at(
                &title,
                placement.x,
                placement.y,
                placement.width,
                placement.height,
            )?;
            if let Some(tree) = &placement.tree {
                self.dock.apply_restored_tree(window, tree.clone())?;
            }
        }
        for record in &snap.views {
            let id = ViewInstanceId::new(record.id)
                .ok_or_else(|| WorkspaceError::Snapshot("view id 0".to_string()))?;
            if !self.registry.contains(id) {
                self.registry
                    .register(id, record.kind, record.title.clone());
            }
            self.states.insert(id, record.state.clone());
            self.vcs.ensure(id);
        }
        for view in &layout.detached {
            if self.dock.owner_of(*view).is_none() && !self.dock.is_detached(*view) {
                self.dock.add_detached(*view)?;
            }
        }
        // Any registered view the (possibly defaulted) layout did not
        // place is parked in the detached pool — recoverable, never
        // homeless.
        for id in self.registry.ids() {
            if self.dock.owner_of(id).is_none() && !self.dock.is_detached(id) {
                self.dock.add_detached(id)?;
            }
        }
        for link in &snap.links {
            if let Some(id) = ViewInstanceId::new(link.view) {
                self.links.set_group(id, Some(LinkGroupId(link.group)));
            }
        }
        // Future view ids must not collide with restored ids.
        self.next_view = self
            .registry
            .ids()
            .iter()
            .map(|v| v.get())
            .max()
            .unwrap_or(0)
            .max(self.next_view);
        Ok(())
    }
}

/// What `pointer_down` resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerOutcome {
    /// A gesture started — its fixation is stored on the view.
    GestureStarted,
    /// The hit was ambiguous; candidates and suggested dedicated
    /// tools are reported instead of starting a gesture.
    Ambiguous {
        /// The competing operations.
        candidates: Vec<EditOperation>,
        /// Dedicated tools that disambiguate.
        suggested: Vec<Tool>,
    },
    /// The probe resolved to nothing (e.g. a dedicated tool with no
    /// compatible hit).
    NoGesture,
}

/// Internal resolution result — either a project entity or a view.
#[derive(Debug, Clone, Copy)]
enum ResolvedTarget {
    Entity(SemanticTarget),
    View(ViewInstanceId),
}

/// The position snap operates on during a move/trim preview, in the
/// fixation origin's declared base units: the primary clip's anchor
/// for a move or `StartTrim`, its media/musical *end* for `EndTrim`
/// (the dragged edge is what snaps, not the stationary anchor).
/// `None` for non-clip primaries or a base that cannot be expressed
/// (a domain mismatch is already rejected at `pointer_down`).
fn primary_snap_position(
    project: &synvelis_project::Project,
    fixation: &GestureFixation,
) -> Option<i64> {
    let SemanticTarget::Clip(clip_id) = fixation.primary()? else {
        return None;
    };
    let clip = project.clips.get(&clip_id)?;
    let anchor_start = match (clip.placement.anchor, fixation.origin) {
        (TimeAnchor::Musical(tick), EditTimePoint::Musical(_)) => tick.value(),
        (TimeAnchor::Absolute(anchor), EditTimePoint::Media(origin)) => {
            crate::selection::media_rescale(
                anchor.value(),
                (anchor.numerator(), anchor.denominator()),
                (origin.numerator(), origin.denominator()),
            )?
        }
        _ => return None,
    };
    let EditOperation::TrimClip {
        edge: HandleKind::EndTrim,
    } = fixation.operation
    else {
        return Some(anchor_start);
    };
    let extent_units = match (clip.placement.extent, fixation.origin) {
        (synvelis_project::ClipExtent::Musical(dur), EditTimePoint::Musical(_)) => dur.value(),
        (synvelis_project::ClipExtent::Absolute(dur), EditTimePoint::Media(origin)) => {
            crate::selection::media_rescale(
                dur.value(),
                (dur.numerator(), dur.denominator()),
                (origin.numerator(), origin.denominator()),
            )?
        }
        // A mixed-domain trim is rejected at commit; no snap edge.
        _ => return None,
    };
    anchor_start.checked_add(extent_units)
}

// keep DockTree/DockError referenced in the public surface docs
#[allow(dead_code)]
fn _assert_types(_: &DockTree, _: DockError) {}
