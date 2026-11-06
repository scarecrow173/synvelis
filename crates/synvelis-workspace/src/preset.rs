//! Named, versioned workspace presets (SV177).
//!
//! A [`WorkspacePreset`] is a *template*: it describes windows, dock
//! trees, view slots, link-group assignments and a focus designation
//! using **logical slot ids** — never live `ViewInstanceId`s or
//! gesture/session state. Applying a preset binds slots to views:
//! compatible existing views are adopted (their state is preserved),
//! missing views are spawned, and residual existing views are parked
//! in the detached pool — never destroyed.
//!
//! The preset schema is deliberately separate from
//! [`crate::persist::WorkspaceSnapshot`] (`WORKSPACE_SCHEMA`): a
//! snapshot is a faithful session capture keyed by real view ids,
//! while a preset is a named, slot-based template with its own
//! version and restore path. A snapshot can be *converted* into a
//! preset ([`WorkspacePreset::from_snapshot`]), which is also how the
//! v1 → v2 migration works.

use std::collections::{BTreeMap, BTreeSet};

use ruviform_ui::dock::{
    Axis, DockNode, LAYOUT_SCHEMA_VERSION, LayoutSnapshot, MonitorHint, ViewInstanceId,
    WindowCloseDisposition, WindowSnapshot,
};
use ruviform_ui::event::HostEvent;
use ruviform_ui::window::{MonitorInfo, WindowHandle, WindowId};
use serde::{Deserialize, Serialize};

use crate::error::WorkspaceError;
use crate::link::LinkGroupId;
use crate::persist::WorkspaceSnapshot;
use crate::snap::{GridConfig, SnapConfig};
use crate::state::{InspectionFocus, InspectionMode, ViewState, Viewport};
use crate::tool::Tool;
use crate::view::ViewType;
use crate::workspace::WorkspaceManager;

/// Current preset schema version.
///
/// Schema history: `1` = legacy `WorkspaceSnapshot`-shaped data
/// (migrated on load); `2` = slot-based preset (this module).
pub const PRESET_SCHEMA: u32 = 2;

/// A named workspace preset: slot-based layout template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspacePreset {
    /// Envelope schema — must equal [`PRESET_SCHEMA`].
    pub schema: u32,
    /// Stable preset id (e.g. `"synvelis.editing"`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Windows to create, in order.
    #[serde(default)]
    pub windows: Vec<PresetWindow>,
    /// View slot definitions.
    #[serde(default)]
    pub views: Vec<PresetView>,
    /// Slots parked in the detached pool.
    #[serde(default)]
    pub detached: Vec<u32>,
    /// Link-group assignments (slot → group).
    #[serde(default)]
    pub links: Vec<PresetLink>,
    /// Initial focus designation.
    #[serde(default)]
    pub focus: Option<PresetFocus>,
}

/// One view slot: logical identity + initial content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetView {
    /// Logical slot id — unique within the preset, never a live
    /// `ViewInstanceId`.
    pub slot: u32,
    /// Surface kind (adoption matches by kind).
    pub kind: ViewType,
    /// Display title (adopted views keep their existing title).
    pub title: String,
    /// Optional initial state — applied to spawned views only.
    /// Adopted views always keep their existing `ViewState`
    /// (preset application must never clobber live view state).
    #[serde(default)]
    pub state: Option<PresetViewState>,
}

/// The template-meaningful subset of [`ViewState`].
///
/// A preset is a *template*, so it may carry view-local display and
/// tool defaults — but never session or entity state: no object/time
/// selections, `EditBinding`, pinned `SemanticTarget`s, link context
/// (which embeds live `ViewInstanceId`s), gesture state, or the
/// revision inbox. [`InspectionMode`] alone is included so the
/// Follow/Pinned *mode* (pin semantics) survives without the
/// session-scoped pinned entity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetViewState {
    /// Active editing tool.
    #[serde(default)]
    pub active_tool: Tool,
    /// Display grid configuration.
    #[serde(default)]
    pub grid: GridConfig,
    /// Snap configuration.
    #[serde(default)]
    pub snap: SnapConfig,
    /// Follow/Pinned mode (target is session-scoped — not part of
    /// the template).
    #[serde(default)]
    pub inspection_mode: InspectionMode,
    /// Viewport (scroll/zoom).
    #[serde(default)]
    pub viewport: Viewport,
}

impl PresetViewState {
    /// Projects a full [`ViewState`] onto the template subset —
    /// session and entity state are dropped.
    #[must_use]
    pub fn from_view_state(state: &ViewState) -> Self {
        Self {
            active_tool: state.active_tool,
            grid: state.grid,
            snap: state.snap,
            inspection_mode: state.inspection_focus.mode,
            viewport: state.viewport,
        }
    }

    /// Expands into a full [`ViewState`] with all session/entity
    /// fields at defaults.
    #[must_use]
    pub fn into_view_state(self) -> ViewState {
        ViewState {
            active_tool: self.active_tool,
            grid: self.grid,
            snap: self.snap,
            inspection_focus: InspectionFocus {
                target: None,
                mode: self.inspection_mode,
            },
            viewport: self.viewport,
            ..ViewState::default()
        }
    }
}

/// One preset window: geometry/monitor signature + dock tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetWindow {
    /// Display title.
    pub title: String,
    /// Captured geometry + monitor signature. `None` = host default.
    #[serde(default)]
    pub geometry: Option<PresetGeometry>,
    /// Dock tree expressed in slot ids.
    pub dock: PresetDock,
}

/// Window geometry + monitor signature (mirrors
/// `WindowSnapshot`/`MonitorHint`, which are restore inputs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresetGeometry {
    /// Left edge, virtual-desktop coordinates.
    pub x: i32,
    /// Top edge, virtual-desktop coordinates.
    pub y: i32,
    /// Width (physical px).
    pub width: u32,
    /// Height (physical px).
    pub height: u32,
    /// Monitor signature at save time (matched by value on restore).
    #[serde(default)]
    pub monitor: Option<MonitorHint>,
}

/// Recursive dock-tree node expressed in slot ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PresetDock {
    /// A tab leaf: slot ids + active tab index.
    Tabs {
        /// Slot ids — at least one, no duplicates.
        views: Vec<u32>,
        /// Active tab index (clamped like `DockNode::tabs`).
        #[serde(default)]
        active: usize,
    },
    /// A split: children (≥ 2) along `axis`, `ratio` = share of
    /// `children[0]` — identical to `DockNode::Split` invariants.
    Split {
        /// Layout direction.
        axis: Axis,
        /// Fraction for `children[0]`; finite, inside `(0, 1)`.
        ratio: f64,
        /// Child nodes.
        children: Vec<PresetDock>,
    },
}

/// One link-group assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetLink {
    /// View slot.
    pub view: u32,
    /// `LinkGroupId` raw value.
    pub group: u64,
}

/// Focus designation: which preset window + which slot inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetFocus {
    /// Index into `preset.windows`.
    pub window: usize,
    /// Slot to activate (optional — `None` = window's own active tab).
    #[serde(default)]
    pub view: Option<u32>,
}

/// What `apply_preset` actually did — for tests and callers that need
/// to show the user what changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PresetReport {
    /// Created window ids, in preset order.
    pub windows: Vec<WindowId>,
    /// Existing views adopted into slots (state preserved).
    pub adopted: Vec<ViewInstanceId>,
    /// Fresh views spawned for slots with no adoptable view.
    pub spawned: Vec<ViewInstanceId>,
    /// Residual existing views parked in the detached pool.
    pub detached: Vec<ViewInstanceId>,
    /// Whether the focus designation was applied.
    pub focused: bool,
}

impl WorkspacePreset {
    /// Serializes to JSON.
    ///
    /// # Errors
    /// [`WorkspaceError::Preset`] on serialization failure.
    pub fn to_json(&self) -> Result<String, WorkspaceError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| WorkspaceError::Preset(format!("cannot serialize: {e}")))
    }

    /// Parses a preset. A top-level `layout` key marks the legacy
    /// (v1) `WorkspaceSnapshot` shape, which is migrated via
    /// [`Self::from_snapshot`]; otherwise the v2 slot shape is
    /// parsed and its `schema` checked.
    ///
    /// # Errors
    /// [`WorkspaceError::Preset`] on malformed JSON, wrong schema, or
    /// failed migration.
    pub fn from_json(json: &str) -> Result<Self, WorkspaceError> {
        let value: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| WorkspaceError::Preset(format!("malformed preset: {e}")))?;
        if value.get("layout").is_some() {
            let snap = WorkspaceSnapshot::from_json(json)
                .map_err(|e| WorkspaceError::Preset(format!("legacy preset: {e}")))?;
            return Ok(Self::from_snapshot(
                &snap,
                "legacy".into(),
                "Legacy preset".into(),
            ));
        }
        let preset: Self = serde_json::from_value(value)
            .map_err(|e| WorkspaceError::Preset(format!("malformed preset: {e}")))?;
        if preset.schema != PRESET_SCHEMA {
            return Err(WorkspaceError::Preset(format!(
                "unsupported schema {} (expected {PRESET_SCHEMA})",
                preset.schema
            )));
        }
        Ok(preset)
    }

    /// Converts a captured [`WorkspaceSnapshot`] into a slot-based
    /// preset. This is also the v1 → v2 migration path.
    ///
    /// View slots are assigned in `views` order (1-based); every
    /// `ViewInstanceId` in dock trees / detached / links that has a
    /// `ViewRecord` maps to its slot. View ids referenced by the
    /// layout but missing from `views` are skipped (a template
    /// cannot resurrect a view it does not describe).
    #[must_use]
    pub fn from_snapshot(snap: &WorkspaceSnapshot, id: String, name: String) -> Self {
        let mut slot_of: BTreeMap<u64, u32> = BTreeMap::new();
        let views: Vec<PresetView> = snap
            .views
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let slot = u32::try_from(i + 1).unwrap_or(u32::MAX);
                slot_of.insert(v.id, slot);
                PresetView {
                    slot,
                    kind: v.kind,
                    title: v.title.clone(),
                    state: Some(PresetViewState::from_view_state(&v.state)),
                }
            })
            .collect();

        let windows: Vec<PresetWindow> = snap
            .layout
            .windows
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let title = snap
                    .window_titles
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("window {}", i + 1));
                PresetWindow {
                    title,
                    geometry: Some(PresetGeometry {
                        x: w.x,
                        y: w.y,
                        width: w.width,
                        height: w.height,
                        monitor: w.monitor.clone(),
                    }),
                    dock: w.tree.as_ref().map_or_else(
                        || PresetDock::Tabs {
                            views: vec![],
                            active: 0,
                        },
                        |t| dock_to_slots(t, &slot_of),
                    ),
                }
            })
            .collect();

        Self {
            schema: PRESET_SCHEMA,
            id,
            name,
            windows,
            views,
            detached: snap
                .layout
                .detached
                .iter()
                .filter_map(|raw| slot_of.get(raw).copied())
                .collect(),
            links: snap
                .links
                .iter()
                .filter_map(|l| {
                    slot_of.get(&l.view).map(|&slot| PresetLink {
                        view: slot,
                        group: l.group,
                    })
                })
                .collect(),
            focus: None,
        }
    }

    /// Structural validation: at least one window, unique slots,
    /// every referenced slot declared, docked slots unique and
    /// disjoint from `detached`, no nested empty tab leaves, link
    /// groups nonzero, and `focus.view` docked inside `focus.window`.
    /// Dock invariants (`ratio`, children counts, view uniqueness)
    /// are additionally verified pre-mutation by
    /// `LayoutSnapshot::restore` during apply — never after the
    /// workspace has been torn down.
    ///
    /// # Errors
    /// [`WorkspaceError::Preset`] describing the first problem.
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if self.windows.is_empty() {
            return Err(WorkspaceError::Preset(
                "preset must declare at least one window".into(),
            ));
        }
        let mut slots = BTreeSet::new();
        for v in &self.views {
            if !slots.insert(v.slot) {
                return Err(WorkspaceError::Preset(format!(
                    "duplicate view slot {}",
                    v.slot
                )));
            }
        }
        let mut docked = BTreeSet::new();
        for (wi, w) in self.windows.iter().enumerate() {
            w.dock.validate_structure(&slots, wi)?;
            for slot in w.dock.slots() {
                if !docked.insert(slot) {
                    return Err(WorkspaceError::Preset(format!(
                        "slot {slot} docked in more than one position"
                    )));
                }
            }
        }
        for slot in &self.detached {
            if !slots.contains(slot) {
                return Err(WorkspaceError::Preset(format!(
                    "detached references undeclared slot {slot}"
                )));
            }
            if docked.contains(slot) {
                return Err(WorkspaceError::Preset(format!(
                    "slot {slot} is both docked and detached"
                )));
            }
        }
        for l in &self.links {
            if !slots.contains(&l.view) {
                return Err(WorkspaceError::Preset(format!(
                    "link references undeclared slot {}",
                    l.view
                )));
            }
            if l.group == 0 {
                return Err(WorkspaceError::Preset(
                    "link group 0 is reserved (groups mint from 1)".into(),
                ));
            }
        }
        if let Some(f) = &self.focus {
            if f.window >= self.windows.len() {
                return Err(WorkspaceError::Preset(format!(
                    "focus window index {} out of range",
                    f.window
                )));
            }
            if let Some(slot) = f.view
                && !self.windows[f.window].dock.slots().contains(&slot)
            {
                return Err(WorkspaceError::Preset(format!(
                    "focus slot {slot} is not docked in window {}",
                    f.window
                )));
            }
        }
        Ok(())
    }
}

impl PresetDock {
    fn slots(&self) -> Vec<u32> {
        match self {
            Self::Tabs { views, .. } => views.clone(),
            Self::Split { children, .. } => children.iter().flat_map(Self::slots).collect(),
        }
    }

    /// Per-window structural checks: referenced slots declared,
    /// nested `Tabs` non-empty (a top-level empty `Tabs` = "window
    /// without views" — legal only at the root).
    fn validate_structure(
        &self,
        slots: &BTreeSet<u32>,
        window: usize,
    ) -> Result<(), WorkspaceError> {
        self.validate_node(slots, window, true)
    }

    fn validate_node(
        &self,
        slots: &BTreeSet<u32>,
        window: usize,
        is_root: bool,
    ) -> Result<(), WorkspaceError> {
        match self {
            Self::Tabs { views, .. } => {
                if views.is_empty() && !is_root {
                    return Err(WorkspaceError::Preset(format!(
                        "window[{window}] dock has a nested empty tab leaf"
                    )));
                }
                for s in views {
                    if !slots.contains(s) {
                        return Err(WorkspaceError::Preset(format!(
                            "window[{window}] dock references undeclared slot {s}"
                        )));
                    }
                }
                Ok(())
            }
            Self::Split { children, .. } => {
                for c in children {
                    c.validate_node(slots, window, false)?;
                }
                Ok(())
            }
        }
    }

    fn to_dock_node(
        &self,
        binding: &BTreeMap<u32, ViewInstanceId>,
    ) -> Result<Option<DockNode>, WorkspaceError> {
        match self {
            Self::Tabs { views, active } => {
                if views.is_empty() {
                    return Ok(None);
                }
                let ids: Vec<ViewInstanceId> = views
                    .iter()
                    .map(|s| {
                        binding
                            .get(s)
                            .copied()
                            .ok_or_else(|| WorkspaceError::Preset(format!("unbound slot {s}")))
                    })
                    .collect::<Result<_, _>>()?;
                DockNode::tabs(ids, *active)
                    .map(Some)
                    .map_err(|e| WorkspaceError::Preset(format!("dock tabs: {e}")))
            }
            Self::Split {
                axis,
                ratio,
                children,
            } => {
                let nodes: Vec<DockNode> = children
                    .iter()
                    .map(|c| c.to_dock_node(binding))
                    .collect::<Result<Vec<Option<DockNode>>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect();
                DockNode::split(*axis, *ratio, nodes)
                    .map(Some)
                    .map_err(|e| WorkspaceError::Preset(format!("dock split: {e}")))
            }
        }
    }
}

fn dock_to_slots(node: &DockNode, slot_of: &BTreeMap<u64, u32>) -> PresetDock {
    match node {
        DockNode::Tabs { views, active } => PresetDock::Tabs {
            views: views
                .iter()
                .filter_map(|v| slot_of.get(&v.get()).copied())
                .collect(),
            active: *active,
        },
        DockNode::Split {
            axis,
            ratio,
            children,
        } => PresetDock::Split {
            axis: *axis,
            ratio: *ratio,
            children: children.iter().map(|c| dock_to_slots(c, slot_of)).collect(),
        },
        _ => PresetDock::Tabs {
            views: vec![],
            active: 0,
        },
    }
}

impl<H: WindowHandle> WorkspaceManager<H> {
    /// Applies `preset` as the workspace's full layout.
    ///
    /// Steps: validate → bind slots (adopt-by-kind in registry order,
    /// else spawn) → build the [`LayoutSnapshot`] and run the
    /// *validating* [`LayoutSnapshot::restore`] — all structural
    /// errors surface before any window is destroyed → close existing
    /// windows (views park in the detached pool) → free bound views →
    /// create windows and attach restored trees → re-apply links and
    /// focus. Existing windows are always closed (preset apply is a
    /// full-layout replace; `veto_close` does not apply — views and
    /// their state are preserved, never destroyed).
    ///
    /// Residual views (existing views no preset slot adopted) stay in
    /// the detached pool with their state intact. Project/session
    /// state is never touched. Once step 4 begins mutating, a host or
    /// dock failure can still leave a partially applied layout; dock
    /// invariants are preserved at each step and homeless views are
    /// re-pooled, but window placement may be incomplete.
    ///
    /// # Errors
    /// [`WorkspaceError::Preset`] on validation or layout-structure
    /// failure (raised before any mutation);
    /// [`WorkspaceError::Ui`]/[`WorkspaceError::Dock`] on host/dock
    /// failures mid-apply.
    #[allow(clippy::too_many_lines)] // numbered apply steps kept linear for review
    pub fn apply_preset(
        &mut self,
        preset: &WorkspacePreset,
        monitors: &[MonitorInfo],
    ) -> Result<PresetReport, WorkspaceError> {
        preset.validate()?;
        let mut report = PresetReport::default();

        // 1. Slot → view binding: adopt compatible existing views
        //    (kind match, registry-id order), spawn the rest into the
        //    detached pool. Preset state applies to spawned views
        //    only — adopted views keep their live state.
        let mut binding: BTreeMap<u32, ViewInstanceId> = BTreeMap::new();
        let mut bound: BTreeSet<ViewInstanceId> = BTreeSet::new();
        for pv in &preset.views {
            let candidate = self.registry.ids().into_iter().find(|id| {
                !bound.contains(id)
                    && self
                        .registry
                        .descriptor(*id)
                        .is_some_and(|d| d.kind == pv.kind)
            });
            let id = if let Some(id) = candidate {
                report.adopted.push(id);
                id
            } else {
                let id = self.spawn_detached_view(pv.kind, &pv.title)?;
                report.spawned.push(id);
                if let Some(state) = &pv.state {
                    self.states.insert(id, state.clone().into_view_state());
                }
                id
            };
            binding.insert(pv.slot, id);
            bound.insert(id);
        }

        // 2. Build the LayoutSnapshot and run the validating restore
        //    *before* any destructive step: schema, geometry,
        //    dock-tree invariants, and global view dedup all fail
        //    here with no workspace damage. Monitor matching and
        //    off-screen correction happen inside `restore`.
        let docked_slots: BTreeSet<u32> =
            preset.windows.iter().flat_map(|w| w.dock.slots()).collect();
        let windows: Vec<WindowSnapshot> = preset
            .windows
            .iter()
            .map(|w| {
                let tree = w.dock.to_dock_node(&binding)?;
                let g = w.geometry.clone().unwrap_or(PresetGeometry {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                    monitor: None,
                });
                Ok(WindowSnapshot {
                    x: g.x,
                    y: g.y,
                    width: g.width,
                    height: g.height,
                    monitor: g.monitor,
                    tree,
                })
            })
            .collect::<Result<_, WorkspaceError>>()?;
        let layout = LayoutSnapshot {
            schema: LAYOUT_SCHEMA_VERSION,
            windows,
            detached: preset
                .detached
                .iter()
                .filter_map(|s| binding.get(s).map(|id| id.get()))
                .collect(),
        };
        let restored = layout
            .restore(monitors)
            .map_err(|e| WorkspaceError::Preset(format!("layout invalid: {e}")))?;

        // 3. Close every existing window: views land in the detached
        //    pool (DetachViews) — nothing is destroyed.
        let old: Vec<WindowId> = self.windows.iter().map(|w| w.id).collect();
        for w in old {
            if let Some(rec) = self.windows.iter_mut().find(|r| r.id == w) {
                rec.disposition = WindowCloseDisposition::DetachViews;
            }
            self.host.destroy_window(w)?;
            self.on_window_event(w, &HostEvent::Destroyed)?;
        }

        // 4. Views that the preset docks into windows must be *free*
        //    for `apply_restored_tree` — pull them out of the
        //    detached pool. Slots named in `preset.detached` stay.
        for slot in &docked_slots {
            if let Some(&id) = binding.get(slot)
                && self.dock.is_detached(id)
            {
                self.dock.remove_view(id)?;
            }
        }

        // 5. Create windows and attach restored trees.
        for (i, placement) in restored.placements.iter().enumerate() {
            let title = preset
                .windows
                .get(i)
                .map_or_else(|| format!("window {}", i + 1), |w| w.title.clone());
            let wid = self.create_window_at(
                &title,
                placement.x,
                placement.y,
                placement.width,
                placement.height,
            )?;
            report.windows.push(wid);
            if let Some(tree) = &placement.tree {
                self.dock.apply_restored_tree(wid, tree.clone())?;
            }
        }

        // 6. Safety net: any bound view left homeless returns to the
        //    detached pool.
        for &id in binding.values() {
            if self.dock.owner_of(id).is_none() && !self.dock.is_detached(id) {
                self.dock.add_detached(id)?;
            }
        }

        // 7. Links: bound views are under preset control — reset,
        //    then apply the preset's assignments.
        for &id in &bound {
            self.links.set_group(id, None);
        }
        for l in &preset.links {
            if let Some(&id) = binding.get(&l.view) {
                self.links.set_group(id, Some(LinkGroupId(l.group)));
            }
        }

        // 8. Focus: host-level focus for the designated window, then
        //    activate the designated tab (validation guarantees the
        //    slot is docked in this window).
        if let Some(f) = &preset.focus
            && let Some(&wid) = report.windows.get(f.window)
        {
            self.host.focus(wid, true)?;
            self.focus.native_window = Some(wid);
            self.focus.active_view = f
                .view
                .and_then(|s| binding.get(&s).copied())
                .or_else(|| self.active_view_in(wid));
            if let Some(id) = f.view.and_then(|s| binding.get(&s).copied()) {
                self.dock.activate_view(id)?;
            }
            report.focused = true;
        }

        // Residual existing views — existing views not bound to any
        // slot — remain in the detached pool by construction.
        report.detached = self
            .registry
            .ids()
            .into_iter()
            .filter(|id| !bound.contains(id) && self.dock.is_detached(*id))
            .collect();
        Ok(report)
    }
}
