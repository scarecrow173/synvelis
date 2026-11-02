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
use synvelis_project::{MusicalTick, ProjectOp, ProjectRevision};

use crate::command::{CommandContext, CommandKind, CommandOutcome, build_project_op, spec_for};
use crate::edit::{self, CommitEffect};
use crate::error::WorkspaceError;
use crate::focus::{FocusState, InputContext};
use crate::gesture::{CancelReason, Gesture, GestureFixation, GestureOutcome, GesturePreview};
use crate::link::{LinkEvent, LinkGroupId, LinkGroupManager};
use crate::persist::{LinkRecord, ViewRecord, WORKSPACE_SCHEMA, WorkspaceSnapshot};
use crate::selection::{LaneScope, SelectionDomain, TimeSelection};
use crate::session::ProjectSession;
use crate::snap::{SnapEnvironment, resolve_snap};
use crate::state::{EditingSelection, SemanticTarget, ViewState, ViewStateRegistry};
use crate::tool::{EditOperation, HitProbe, Tool, ToolResolution, resolve_tool};
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
    dock: DockWorkspace,
    registry: ViewRegistry,
    states: ViewStateRegistry,
    links: LinkGroupManager,
    focus: FocusState,
    session: ProjectSession,
    windows: Vec<WindowRecord>,
    next_view: u64,
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
        Ok(id)
    }

    /// Removes a view entirely (dock, registry, state, links).
    ///
    /// # Errors
    /// [`WorkspaceError::Dock`] when the view is busy or unknown.
    pub fn remove_view(&mut self, view: ViewInstanceId) -> Result<(), WorkspaceError> {
        self.dock.remove_view(view)?;
        self.registry.unregister(view)?;
        self.states.remove(view);
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
        if state.gesture.is_some() {
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
                let domain =
                    if matches!(operation, EditOperation::Marquee | EditOperation::DrawClip) {
                        crate::selection::EditTimeDomain::Musical
                    } else {
                        edit::gesture_domain(self.session.project(), &targets)?
                    };
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
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] / [`WorkspaceError::NoGesture`].
    pub fn drag_update(
        &mut self,
        view: ViewInstanceId,
        position: MusicalTick,
        env: &SnapEnvironment,
    ) -> Result<(), WorkspaceError> {
        let state = self.state(view)?;
        let Some(gesture) = state.gesture.as_ref() else {
            return Err(WorkspaceError::NoGesture(view));
        };
        let fixation = gesture.fixation.clone();
        let prev_snap = gesture.preview.snap;
        let snap_cfg = state.snap;
        let raw_delta = position
            .value()
            .checked_sub(fixation.origin.value())
            .ok_or(synvelis_project::Error::Overflow)?;
        let mut delta = raw_delta;
        let mut winner = None;
        if matches!(
            fixation.operation,
            EditOperation::MoveClips | EditOperation::TrimClip { .. }
        ) && let Some(anchor_start) = primary_anchor_start(self.session.project(), &fixation)
        {
            let snapped = resolve_snap(
                &snap_cfg,
                env,
                anchor_start
                    .checked_add(raw_delta)
                    .map_err(WorkspaceError::Project)?,
                fixation.domain,
                prev_snap,
            );
            if let Some(w) = snapped {
                delta = w
                    .position
                    .value()
                    .checked_sub(anchor_start.value())
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

/// The primary clip's anchor start — the position snap operates on
/// during a move/trim preview.
fn primary_anchor_start(
    project: &synvelis_project::Project,
    fixation: &GestureFixation,
) -> Option<MusicalTick> {
    let SemanticTarget::Clip(clip_id) = fixation.primary()? else {
        return None;
    };
    let clip = project.clips.get(&clip_id)?;
    match clip.anchor {
        synvelis_project::TimeAnchor::Musical(range) => Some(range.start()),
        synvelis_project::TimeAnchor::Media(_) => None,
    }
}

// keep DockTree/DockError referenced in the public surface docs
#[allow(dead_code)]
fn _assert_types(_: &DockTree, _: DockError) {}
