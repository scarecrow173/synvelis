//! The product's minimal neutral screen spec (SV-GPUI-01 view-mount
//! slice, Issue #175 successor; mixer gain wired by Issue #185;
//! Transport actions + Inspector display wired by Issue #112 slice 1;
//! Arrangement tab + Undo/Redo wired by Issue #112 slice 2;
//! Mixer strip Mute/Solo toggles wired by Issue #112 slice 3).
//!
//! [`main_screen_spec`] is the smallest *product-shaped* `ViewSpec`
//! mounted through the RF387 seam: `Panel > [Toolbar, Tabs]` where the
//! toolbar holds Save/Reload `Button`s (the `project` actions), the
//! Transport tab carries Play/Stop `Button`s on the `transport` action
//! target plus a status `Label` readout and a `Seek` driving the real
//! `transport/position` command target (`Begin → Preview → Commit/
//! Cancel` gesture dispatch; the controlled value is the seconds
//! position derived from the authoritative projection), the Mixer tab
//! carries a `Fader` bound to the bootstrap master-strip
//! `ParameterRole::Fader` target ([`mixer_gain_target`]) — its
//! controlled display is the authoritative `host_normalized` projection
//! and commits land as `SetParameterBase` through the same gesture path
//! as every parameter target — the Inspector tab carries the
//! subscribed parameter target's `InspectorBreakdown` as a `Table`
//! plus its control-relation neighborhood as a second read-only
//! `Table` with a verbatim coverage readout (synvelis#102 UI-C5-01),
//! the Arrangement tab carries the project-backed track/lane/clip
//! read model as a read-only `Table` (synvelis#112 slice 2), the
//! toolbar carries Undo/Redo `Button`s on the `project` action target
//! (`syn.project.undo`/`syn.project.redo`), and the Mixer tab's strip
//! additionally carries `Toggle`s bound to the master strip's seeded
//! `ParameterRole::Mute`/`Solo` targets discovered from the project's
//! `ControlModel` (synvelis#112 slice 3).
//! The Seek range follows the project length
//! ([`seek_range_seconds`]).
//!
//! Dynamic display values (transport status/position/loop, inspector
//! rows) are not projections — they are consumer-resolved text baked
//! into the spec from [`MainScreenState`], which the composition layer
//! captures out of the `ControlsUi` projection rows after every UI
//! pump. The spec is rebuilt on every state change and applied with
//! `ViewHost::update` (through `GpuiDriver::update_view`), so the
//! mounted controls keep their identity and no remount happens.
//!
//! Contract notes baked into the spec:
//!
//! - `control_id` equals the binding `target` on driven controls.
//!   `syn.control.projection` rows translate to the view wire as
//!   `control_id: <target>` and `ViewHost::apply_projection`
//!   correlates by `control_id`, so a control whose `control_id`
//!   differs from its `target` would silently drop every projection
//!   (`crate::binding::notify_to_view_event`).
//! - Every control carries a distinct `control_id` (RF410 shared-
//!   `control_id` under-determination is avoided). A control whose
//!   target does not resolve in `syn.control.targets` mints commands
//!   that come back `rejected` — the Fader's well-known target is
//!   seeded by `Project::new` and restored by the v7→v8 migration, so
//!   every loadable project resolves it; only a corrupt bundle fails
//!   honestly instead of writing phantom state.
//! - The logical view id is `view-1`: `ViewHost` stamps
//!   `origin_view = "view-{ViewInstanceId}"` on minted commands, and
//!   the shell validates that against `syn.view.attach` bookkeeping —
//!   [`main_view_id`] and [`main_view_instance`] must stay in lockstep.
//!
//! **Dependency direction:** `ruviform_ui::view` types only — no
//! GPUI/OS/DOM. **RT contract:** non-RT only.

use std::cell::RefCell;
use std::rc::Rc;

use ruviform_ui::UiError;
use ruviform_ui::behavior::{DetachPolicy, ValueDescriptor};
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{
    A11yRole, A11ySpec, AttachEpochAllocator, ButtonSpec, CellValue, ComponentKey, ComponentSpec,
    ComponentVisualState, ControlSpec, LabelSpec, Orientation, PanelSpec, TabSpec, TableSpec,
    TabsSelectionSpec, TabsSpec, ViewHost, ViewSpec,
};
use synvelis_project::{
    ControlModel, MASTER_FADER_RAW, ModulationAssignmentId, ParameterRole, ParameterTargetId,
    RelationCoverage,
};
use synvelis_workspace::controls_ui::{TRANSPORT_POSITION_TARGET, TRANSPORT_TARGET};
use synvelis_workspace::{ArrangementRow, RelationRow, ViewType};

use crate::binding::{PROJECT_TARGET, VIEW_STATE_TARGET};

/// The single logical view this slice mounts — `ViewInstanceId(1)`
/// renders as `origin_view`/`view_id` `"view-1"` on the wire.
const MAIN_VIEW_INSTANCE_RAW: u64 = 1;

/// The second logical view (SV-GUI-04 slice 1, synvelis#113) —
/// `ViewInstanceId(2)` renders as `origin_view`/`view_id` `"view-2"`
/// on the wire. It shares the same `MainScreenState` cell and Project
/// session as the main view; only the mounted spec (a transport deck)
/// and its subscription targets differ.
const SECONDARY_VIEW_INSTANCE_RAW: u64 = 2;

/// Seek range used while the project has no resolvable content — the
/// boot range the slice shipped with, kept so an empty project still
/// scrubs.
pub const SEEK_DEFAULT_RANGE_SECONDS: f64 = 240.0;

/// The Seek range for a project of `project_length_seconds`: the real
/// project length, or [`SEEK_DEFAULT_RANGE_SECONDS`] when the project
/// has no content (length `0`).
#[must_use]
pub fn seek_range_seconds(project_length_seconds: f64) -> f64 {
    if project_length_seconds.is_finite() && project_length_seconds > 0.0 {
        project_length_seconds
    } else {
        SEEK_DEFAULT_RANGE_SECONDS
    }
}

/// Wire `view_id` of the product's main view — must equal
/// `format!("view-{}", MAIN_VIEW_INSTANCE_RAW)`.
#[must_use]
pub fn main_view_id() -> String {
    format!("view-{MAIN_VIEW_INSTANCE_RAW}")
}

/// The logical view instance for the main screen. The id is the
/// *persisted* identity (layout restoration re-attaches it); the
/// `AttachEpoch` minted per host is the *runtime* guard — kept
/// deliberately separate so a recreated host for the same logical
/// view is a fresh command namespace, never the old one resumed.
///
/// # Panics
/// Never: `MAIN_VIEW_INSTANCE_RAW` is a nonzero literal.
#[must_use]
pub fn main_view_instance() -> ViewInstanceId {
    ViewInstanceId::new(MAIN_VIEW_INSTANCE_RAW).expect("nonzero literal")
}

/// Wire `view_id` of the product's secondary view — the transport
/// deck mounted on the env-gated second window
/// ([`secondary_screen_spec`]). Must stay in lockstep with
/// [`secondary_view_instance`] exactly like [`main_view_id`] does
/// with [`main_view_instance`].
#[must_use]
pub fn secondary_view_id() -> String {
    format!("view-{SECONDARY_VIEW_INSTANCE_RAW}")
}

/// The logical view instance for the secondary screen — a distinct
/// persisted identity and command namespace from the main view (the
/// `AttachEpoch` minted per host is the runtime guard, same contract
/// as [`main_view_instance`]).
///
/// # Panics
/// Never: `SECONDARY_VIEW_INSTANCE_RAW` is a nonzero literal.
#[must_use]
pub fn secondary_view_instance() -> ViewInstanceId {
    ViewInstanceId::new(SECONDARY_VIEW_INSTANCE_RAW).expect("nonzero literal")
}

/// Wire target of the mixer gain fader — the tagged id of the
/// bootstrap master-strip [`synvelis_project::ParameterRole::Fader`]
/// target (`param-<32hex>`), resolved by `syn.control.subscribe`/
/// `dispatch` like any parameter target (synvelis#185).
#[must_use]
pub fn mixer_gain_target() -> String {
    ParameterTargetId::new(MASTER_FADER_RAW).to_tagged()
}

/// Mixer channel-strip state — the tagged `param-<hex>` ids of the
/// master strip's seeded channel toggles (`Mute`/`Solo`), resolved
/// from the project's [`ControlModel`] by [`mixer_strip_targets`]
/// (synvelis#112 slice 3). A `None` field means the target is absent
/// (a degenerate control model) and the strip omits that toggle
/// rather than binding a phantom target.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MixerScreenState {
    /// Tagged id of the master strip's `ParameterRole::Mute` target.
    pub mute_target: Option<String>,
    /// Tagged id of the master strip's `ParameterRole::Solo` target.
    pub solo_target: Option<String>,
}

/// Resolves the master strip's seeded channel-toggle targets from the
/// project's control model — the strip that owns the well-known
/// [`mixer_gain_target`] fader. Discovery is by role+owner, never by
/// well-known raw-ID arithmetic (those IDs are `pub(crate)` in
/// synvelis-project), so it stays correct if the numbering moves.
/// `Mute`/`Solo` are seeded by `ControlModel::bootstrap` and cannot
/// be removed, so every legal project resolves both; a degenerate
/// model resolves neither.
#[must_use]
pub fn mixer_strip_targets(control: &ControlModel) -> MixerScreenState {
    let Some(strip) = control
        .targets
        .get(&ParameterTargetId::new(MASTER_FADER_RAW))
        .map(|target| target.owner)
    else {
        return MixerScreenState::default();
    };
    let mut state = MixerScreenState::default();
    for target in control.targets.values() {
        if target.owner != strip {
            continue;
        }
        match target.role {
            ParameterRole::Mute => state.mute_target = Some(target.id.to_tagged()),
            ParameterRole::Solo => state.solo_target = Some(target.id.to_tagged()),
            _ => {}
        }
    }
    state
}

/// Transport readout state — captured from the authoritative
/// `TransportProjection` (`status`/`position`/`loop_range`) by the
/// composition layer and baked into the spec's status `Label`.
/// `position_seconds` is already tempo-map converted (seconds), the
/// same unit the Seek's controlled display uses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransportScreenState {
    /// Playback status display text (`"Stopped"`/`"Playing"`/`"Paused"`).
    pub status: Option<String>,
    /// Playhead position in seconds.
    pub position_seconds: Option<f64>,
    /// Whether a loop range is active.
    pub loop_enabled: bool,
}

/// One Inspector table row — a breakdown field and its resolved
/// display text.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectorRow {
    /// Field name (e.g. `"mode"`, `"arm"`).
    pub field: String,
    /// Resolved display text.
    pub value: String,
}

/// Inspector tab state — the subscribed parameter target's
/// `InspectorBreakdown` flattened to display rows. Slice 1 shows the
/// view's subscribed param target (the mixer gain bootstrap); target
/// selection UI is a later slice.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InspectorScreenState {
    /// Tagged id of the displayed parameter target, if any.
    pub target: Option<String>,
    /// Breakdown rows in display order.
    pub rows: Vec<InspectorRow>,
}

/// Relation table state — the subscribed parameter target's
/// control-relation neighborhood resolved by the composition layer
/// ([`synvelis_workspace::relation_rows`], synvelis#102 UI-C5-01).
/// `coverage` carries the query's verbatim [`RelationCoverage`]:
/// `Stale`/`Unavailable`/`Truncated` surface as text, never as an
/// empty table that reads like "no relations".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RelationScreenState {
    /// Tagged id of the anchor entity, when a query ran.
    pub anchor: Option<String>,
    /// Verbatim coverage of the last relation query.
    pub coverage: Option<RelationCoverage>,
    /// Relation rows in deterministic query order.
    pub rows: Vec<RelationRow>,
}

impl RelationScreenState {
    /// Projects a [`synvelis_workspace::RelationView`] into display
    /// state — anchor and coverage carried verbatim.
    #[must_use]
    pub fn from_view(view: synvelis_workspace::RelationView) -> Self {
        Self {
            anchor: Some(view.anchor_label()),
            coverage: Some(view.coverage),
            rows: view.rows,
        }
    }
}

/// Signal Flow display state — the latest
/// [`synvelis_workspace::SignalFlowView`] projected by the
/// composition layer (synvelis#108 SF-05 slice 1). The whole view is
/// carried verbatim: coverage (`Complete`/`Truncated`/`Stale`/
/// `Unavailable`/`Unsupported`), revisions, generations, per-node
/// runtime/probe state, and edges surface as text — never collapsed
/// into an empty graph that reads like "no routing".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SignalFlowScreenState {
    /// The last projected view (`None` before the first refresh).
    pub view: Option<synvelis_workspace::SignalFlowView>,
}

impl SignalFlowScreenState {
    /// Projects a [`synvelis_workspace::SignalFlowView`] verbatim.
    #[must_use]
    pub fn from_view(view: synvelis_workspace::SignalFlowView) -> Self {
        Self { view: Some(view) }
    }
}

/// The full display state [`main_screen_spec`] needs — project length
/// (Seek range), arrangement rows, transport readout, and inspector
/// rows. Owned by the composition root behind a shared cell; refreshed
/// after every service op (project length + arrangement rows +
/// relation rows here) and UI pump (transport/inspector capture in
/// `ui_ops`), then pushed to the mounted view through
/// `ViewHost::update`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MainScreenState {
    /// Live project length in seconds — the Seek range source.
    pub project_length_seconds: f64,
    /// Arrangement table rows — the project-backed track/lane/clip
    /// read model (synvelis#112 slice 2).
    pub arrangement_rows: Vec<ArrangementRow>,
    /// Transport status/position/loop readout.
    pub transport: TransportScreenState,
    /// Inspector breakdown display.
    pub inspector: InspectorScreenState,
    /// Inspector relation neighborhood display (synvelis#102
    /// UI-C5-01).
    pub relations: RelationScreenState,
    /// Signal Flow graph/status display (synvelis#108 SF-05 slice 1).
    pub signal_flow: SignalFlowScreenState,
    /// Mixer strip channel-toggle targets (synvelis#112 slice 3) —
    /// project-derived, refreshed by `publish_project_length`.
    pub mixer: MixerScreenState,
}

/// Targets the main view subscribes after attach — every driven
/// control's binding target plus the `transport` action surface whose
/// `TransportProjection` feeds the status readout. One entry per
/// unique target (the subscription is `(view, target)`, not per
/// control).
#[must_use]
pub fn main_view_targets() -> Vec<String> {
    vec![
        TRANSPORT_TARGET.to_string(),
        TRANSPORT_POSITION_TARGET.to_string(),
        mixer_gain_target(),
    ]
}

/// Targets the secondary view subscribes after attach — only the
/// transport surface its deck drives (the `transport` action target
/// for Play/Stop and `transport/position` for the Seek). The
/// subscription is `(view, target)`, so both views receive their own
/// projection rows for the shared targets.
#[must_use]
pub fn secondary_view_targets() -> Vec<String> {
    vec![
        TRANSPORT_TARGET.to_string(),
        TRANSPORT_POSITION_TARGET.to_string(),
    ]
}

/// Dock-domain metadata for a composition-root view — everything the
/// workspace adoption seam and the `syn.window.dock_*` op layer need
/// that the wire `view_id` alone cannot carry: the persisted
/// `ViewInstanceId`, the surface `ViewType`, the descriptor title,
/// and the subscription targets the window attach issues.
pub struct DockViewInfo {
    /// Persisted dock identity.
    pub instance: ViewInstanceId,
    /// Surface kind recorded in the view descriptor.
    pub kind: ViewType,
    /// Descriptor title (tab caption / window chrome).
    pub title: &'static str,
    /// Subscription targets issued on window attach.
    pub targets: Vec<String>,
}

/// Resolves `view_id` to its dock metadata — `Some` only for the
/// composition root's own views (`view-1`/`view-2`). Every other id
/// stays on the legacy UI-only path and is never adopted into the
/// workspace dock.
#[must_use]
pub fn dock_view_info(view_id: &str) -> Option<DockViewInfo> {
    if view_id == main_view_id() {
        Some(DockViewInfo {
            instance: main_view_instance(),
            kind: ViewType::Arrangement,
            title: "Main",
            targets: main_view_targets(),
        })
    } else if view_id == secondary_view_id() {
        Some(DockViewInfo {
            instance: secondary_view_instance(),
            kind: ViewType::Mixer,
            title: "Secondary",
            targets: secondary_view_targets(),
        })
    } else {
        None
    }
}

fn key(name: &str) -> ComponentKey {
    ComponentKey(name.to_string())
}

fn a11y_slider(label: &str, o: Orientation) -> A11ySpec {
    A11ySpec {
        role: Some(A11yRole::Slider),
        label: Some(label.to_string()),
        orientation: Some(o),
        value_min: Some(0.0),
        value_max: Some(1.0),
        value_text: None,
    }
}

fn panel(key_name: &str, title: Option<&str>, children: Vec<ComponentSpec>) -> ComponentSpec {
    ComponentSpec::Panel(PanelSpec {
        key: key(key_name),
        title: title.map(str::to_string),
        children,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec::default(),
    })
}

/// The driven control: a horizontal seek scrubbing `transport/position`
/// — the one continuous target the boot project always exposes. Its
/// range is the seek range for `range_seconds` ([`seek_range_seconds`]).
fn transport_seek(range_seconds: f64) -> ComponentSpec {
    transport_seek_keyed("seek.position", range_seconds)
}

/// [`transport_seek`] under an explicit `ComponentKey` — the
/// secondary screen's spec reuses the same driven control contract
/// (`control_id == target`) under its own key namespace.
fn transport_seek_keyed(key_name: &str, range_seconds: f64) -> ComponentSpec {
    ComponentSpec::Seek(ControlSpec {
        key: key(key_name),
        control_id: Some(TRANSPORT_POSITION_TARGET.to_string()),
        target: Some(TRANSPORT_POSITION_TARGET.to_string()),
        descriptor: ValueDescriptor {
            default_value: Some(0.0),
            step: Some(1.0),
            ..ValueDescriptor::ranged(0.0, range_seconds)
        },
        orientation: Orientation::Horizontal,
        detach_policy: DetachPolicy::Cancel,
        unsupported: false,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            role: Some(A11yRole::Slider),
            label: Some("Transport position".to_string()),
            orientation: Some(Orientation::Horizontal),
            value_min: Some(0.0),
            value_max: Some(range_seconds),
            value_text: None,
        },
    })
}

/// A product `project` action button — Save/Reload. The press emits one
/// `Action` on [`PROJECT_TARGET`]; the shell runs the matching service op.
fn project_button(key_name: &str, label: &str, action: &str) -> ComponentSpec {
    ComponentSpec::Button(ButtonSpec {
        key: key(key_name),
        label: label.to_string(),
        target: PROJECT_TARGET.to_string(),
        action: action.to_string(),
        params: None,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec::default(),
    })
}

/// A `transport` action button — Play/Stop. The press emits one
/// discrete `Action` on [`TRANSPORT_TARGET`]; `ControlsUi::route_action`
/// resolves `play`/`stop` into the transport ops and broadcasts the
/// resulting `TransportProjection` (synvelis#112 slice 1).
fn transport_button(key_name: &str, label: &str, action: &str) -> ComponentSpec {
    ComponentSpec::Button(ButtonSpec {
        key: key(key_name),
        label: label.to_string(),
        target: TRANSPORT_TARGET.to_string(),
        action: action.to_string(),
        params: None,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec::default(),
    })
}

/// The transport status readout — resolved text baked from
/// [`TransportScreenState`] (status + seconds position + loop flag).
/// Static display text: it is *not* a projection target, so it carries
/// no `control_id`; updates arrive through `ViewHost::update` spec
/// pushes, not `apply_projection`.
fn transport_status_label(t: &TransportScreenState) -> ComponentSpec {
    ComponentSpec::Label(LabelSpec {
        key: key("transport.status"),
        text: transport_status_text(t),
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Transport status".to_string()),
            ..A11ySpec::default()
        },
    })
}

fn transport_status_text(t: &TransportScreenState) -> String {
    let status = t.status.as_deref().unwrap_or("—");
    match t.position_seconds {
        Some(position) => {
            let loop_mark = if t.loop_enabled { " loop" } else { "" };
            format!("{status} {position:.2}s{loop_mark}")
        }
        None => status.to_string(),
    }
}

/// The Inspector target readout — which parameter target the table
/// below describes (`"no target"` before the first projection lands).
fn inspector_target_label(i: &InspectorScreenState) -> ComponentSpec {
    ComponentSpec::Label(LabelSpec {
        key: key("inspector.target"),
        text: i.target.clone().unwrap_or_else(|| "no target".to_string()),
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Inspector target".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The Inspector breakdown table — the subscribed parameter target's
/// `InspectorBreakdown` flattened to `Field`/`Value` rows by the
/// composition layer ([`InspectorScreenState`]). Read-only: no
/// `selection`, so it issues no intents.
fn inspector_table(i: &InspectorScreenState) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("inspector.table"),
        columns: vec!["Field".to_string(), "Value".to_string()],
        rows: i
            .rows
            .iter()
            .map(|row| {
                vec![
                    Some(CellValue::Text(row.field.clone())),
                    Some(CellValue::Text(row.value.clone())),
                ]
            })
            .collect(),
        selection: None,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Inspector breakdown".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The Inspector relation readout — which anchor entity the relation
/// table below describes plus the query's verbatim coverage
/// (`Complete`/`Truncated`/`Stale`/`Unavailable`). Static display
/// text like [`transport_status_label`]: updates arrive through
/// `ViewHost::update` spec pushes, not `apply_projection`.
fn relation_label(r: &RelationScreenState) -> ComponentSpec {
    let text = match (&r.anchor, &r.coverage) {
        (Some(anchor), Some(coverage)) => format!("Relations — {anchor} — {coverage:?}"),
        (Some(anchor), None) => format!("Relations — {anchor}"),
        (None, _) => "Relations — no target".to_string(),
    };
    ComponentSpec::Label(LabelSpec {
        key: key("inspector.relations.label"),
        text,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Inspector relations".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The Inspector relation table — the subscribed parameter target's
/// control-relation neighborhood ([`RelationScreenState`],
/// synvelis#102 UI-C5-01). Kind/source/target/assignment/state are
/// carried verbatim: `Disabled`/`Unresolved` edge states and
/// `Truncated`/`Stale`/`Unavailable` coverage are surfaced, never
/// collapsed into an empty or zeroed table. Read-only: `selection`
/// is `None`, so it issues no intents.
fn relation_table(r: &RelationScreenState) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("inspector.relations.table"),
        columns: vec![
            "Kind".to_string(),
            "Source".to_string(),
            "Target".to_string(),
            "Assignment".to_string(),
            "State".to_string(),
        ],
        rows: r
            .rows
            .iter()
            .map(|row| {
                vec![
                    Some(CellValue::Text(format!("{:?}", row.kind))),
                    Some(CellValue::Text(row.source_label())),
                    Some(CellValue::Text(row.target_label())),
                    Some(CellValue::Text(row.assignment.map_or_else(
                        || "—".to_string(),
                        ModulationAssignmentId::to_tagged,
                    ))),
                    Some(CellValue::Text(format!("{:?}", row.state))),
                ]
            })
            .collect(),
        selection: None,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Inspector relations".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The Signal Flow status readout — live project revision, view
/// coverage/scope, engine attachment, activation state and the
/// planned/published/observed/last-good generations, source-map
/// revision/generation, staleness, and probe count. `Published` and
/// `Active` stay distinct labels; an absent engine reads
/// `engine unattached`, never a fake empty-success (SF-UX-05).
fn signal_flow_status_label(s: &SignalFlowScreenState) -> ComponentSpec {
    let text = match &s.view {
        None => "Signal Flow — no projection".to_string(),
        Some(view) => {
            let status = &view.status;
            let opt_u64 = |v: Option<u64>| v.map_or_else(|| "—".to_string(), |g| g.to_string());
            let activation = status
                .activation
                .as_ref()
                .map_or_else(|| "none".to_string(), |a| format!("{a:?}"));
            let engine = if status.engine_attached {
                "attached"
            } else {
                "unattached"
            };
            let stale = if status.stale { " STALE" } else { "" };
            format!(
                "Signal Flow — rev {} — {:?} — engine {} — activation {} — gen planned/published/observed/last-good {}/{}/{}/{} — map rev {} gen {} — probes {}{}",
                view.project_revision.0,
                view.coverage,
                engine,
                activation,
                opt_u64(status.planned_generation),
                opt_u64(status.published_generation),
                opt_u64(status.observed_active_generation),
                opt_u64(status.last_good_generation),
                status
                    .source_map_revision
                    .map_or_else(|| "—".to_string(), |r| r.0.to_string()),
                opt_u64(status.source_map_generation),
                status.probes.len(),
                stale,
            )
        }
    };
    ComponentSpec::Label(LabelSpec {
        key: key("signal_flow.status"),
        text,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Signal Flow status".to_string()),
            ..A11ySpec::default()
        },
    })
}

fn signal_flow_node_label(node: &synvelis_workspace::SignalFlowNode) -> String {
    let kind = match &node.kind {
        synvelis_workspace::SignalFlowNodeKind::TrackStrip { .. } => "track",
        synvelis_workspace::SignalFlowNodeKind::BusStrip { kind, .. } => match kind {
            synvelis_project::BusKind::Master => "master",
            synvelis_project::BusKind::Group => "bus",
            synvelis_project::BusKind::Return => "return",
        },
        synvelis_workspace::SignalFlowNodeKind::Processor { .. } => "processor",
    };
    format!("{} ({kind})", node.label)
}

/// The Signal Flow node table — projected nodes with semantic state,
/// runtime resolution, probe state, mute/solo verbatim (SF-UX-01/05).
/// Read-only: `selection` is `None`.
fn signal_flow_nodes_table(s: &SignalFlowScreenState) -> ComponentSpec {
    let rows = s.view.as_ref().map_or_else(Vec::new, |view| {
        view.nodes
            .iter()
            .map(|node| {
                vec![
                    Some(CellValue::Text(signal_flow_node_label(node))),
                    Some(CellValue::Text(format!("{:?}", node.state))),
                    Some(CellValue::Text(format!("{:?}", node.runtime))),
                    Some(CellValue::Text(
                        node.probe
                            .map_or_else(|| "—".to_string(), |p| format!("{p:?}")),
                    )),
                    Some(CellValue::Text(
                        if node.muted { "yes" } else { "" }.to_string(),
                    )),
                    Some(CellValue::Text(
                        if node.soloed { "yes" } else { "" }.to_string(),
                    )),
                ]
            })
            .collect()
    });
    ComponentSpec::Table(TableSpec {
        key: key("signal_flow.nodes"),
        columns: vec![
            "Node".to_string(),
            "State".to_string(),
            "Runtime".to_string(),
            "Probe".to_string(),
            "Muted".to_string(),
            "Solo".to_string(),
        ],
        rows,
        selection: None,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Signal Flow nodes".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The Signal Flow edge table — one row per projected edge with role,
/// resolved endpoint labels, enabled flag and send tap verbatim.
fn signal_flow_edges_table(s: &SignalFlowScreenState) -> ComponentSpec {
    let rows = s.view.as_ref().map_or_else(Vec::new, |view| {
        let label_of = |id: synvelis_workspace::SignalFlowNodeId| {
            view.nodes
                .iter()
                .find(|n| n.id == id)
                .map_or_else(|| format!("{id:?}"), signal_flow_node_label)
        };
        view.edges
            .iter()
            .map(|edge| {
                vec![
                    Some(CellValue::Text(format!("{:?}", edge.role))),
                    Some(CellValue::Text(label_of(edge.source.node))),
                    Some(CellValue::Text(label_of(edge.target.node))),
                    Some(CellValue::Text(
                        if edge.enabled { "yes" } else { "no" }.to_string(),
                    )),
                    Some(CellValue::Text(
                        edge.tap
                            .map_or_else(|| "—".to_string(), |t| format!("{t:?}")),
                    )),
                ]
            })
            .collect()
    });
    ComponentSpec::Table(TableSpec {
        key: key("signal_flow.edges"),
        columns: vec![
            "Role".to_string(),
            "Source".to_string(),
            "Target".to_string(),
            "Enabled".to_string(),
            "Tap".to_string(),
        ],
        rows,
        selection: None,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Signal Flow edges".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The Arrangement table — the project-backed track/lane/clip read
/// model resolved by the composition layer
/// ([`synvelis_workspace::arrangement_rows`]). Read-only: `selection`
/// is `None`, so it issues no intents; row selection → Inspector is a
/// later slice (synvelis#112).
fn arrangement_table(rows: &[ArrangementRow]) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("arrangement.table"),
        columns: vec![
            "Track".to_string(),
            "Lane".to_string(),
            "Clip".to_string(),
            "Source".to_string(),
            "Start".to_string(),
            "End".to_string(),
            "Muted".to_string(),
        ],
        rows: rows
            .iter()
            .map(|row| {
                let seconds =
                    |v: Option<f64>| v.map_or_else(|| "—".to_string(), |s| format!("{s:.2}"));
                vec![
                    Some(CellValue::Text(row.track.clone())),
                    Some(CellValue::Text(row.lane.clone())),
                    Some(CellValue::Text(row.clip.clone())),
                    Some(CellValue::Text(row.source.clone())),
                    Some(CellValue::Text(seconds(row.start_seconds))),
                    Some(CellValue::Text(seconds(row.end_seconds))),
                    Some(CellValue::Text(
                        if row.muted { "yes" } else { "" }.to_string(),
                    )),
                ]
            })
            .collect(),
        selection: None,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Arrangement".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The `Fader` the slice names — the mixer gain, bound to the
/// bootstrap master-strip fader target ([`mixer_gain_target`]). Its
/// normalized `[0, 1]` domain is the domain contract: the controlled
/// display is the projection's `host_normalized` and commits write
/// `target.base` through `SetParameterBase`.
fn mixer_fader() -> ComponentSpec {
    let target = mixer_gain_target();
    ComponentSpec::Fader(ControlSpec {
        key: key("fader.gain"),
        control_id: Some(target.clone()),
        target: Some(target),
        descriptor: ValueDescriptor {
            default_value: Some(1.0),
            step: Some(0.05),
            ..ValueDescriptor::ranged(0.0, 1.0)
        },
        orientation: Orientation::Vertical,
        detach_policy: DetachPolicy::Cancel,
        unsupported: false,
        visual: ComponentVisualState::default(),
        a11y: a11y_slider("Gain", Orientation::Vertical),
    })
}

/// A strip channel `Toggle` — `Mute`/`Solo` on the master strip
/// (synvelis#112 slice 3). One click is one atomic `Set` (a
/// synthesized begin+commit pair marked `atomic`) through the same
/// `syn.control.dispatch` gesture path as the Fader; the controlled
/// display is the projection's `host_normalized` on the binary
/// `{0, 1}` descriptor the Toggle contract requires.
fn strip_toggle(key_name: &str, label: &str, target: &str) -> ComponentSpec {
    ComponentSpec::Toggle(ControlSpec {
        key: key(key_name),
        control_id: Some(target.to_string()),
        target: Some(target.to_string()),
        descriptor: ValueDescriptor {
            default_value: Some(0.0),
            step: Some(1.0),
            ..ValueDescriptor::ranged(0.0, 1.0)
        },
        orientation: Orientation::Horizontal,
        detach_policy: DetachPolicy::Cancel,
        unsupported: false,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            role: Some(A11yRole::Switch),
            label: Some(label.to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The Mixer tab's channel strip — the gain [`mixer_fader`] plus a
/// `Toggle` per discovered strip target ([`MixerScreenState`]). A
/// `None` target omits its toggle so a degenerate project never
/// binds a phantom target.
fn mixer_strip(state: &MixerScreenState) -> ComponentSpec {
    let mut children = vec![mixer_fader()];
    if let Some(target) = &state.mute_target {
        children.push(strip_toggle("toggle.mute", "Mute", target));
    }
    if let Some(target) = &state.solo_target {
        children.push(strip_toggle("toggle.solo", "Solo", target));
    }
    panel("tab.mixer.strip", Some("Master"), children)
}

/// The product's minimal screen spec — `Panel > [Toolbar, Tabs]`:
///
/// - **Toolbar**: Save, Reload, Undo and Redo `Button`s (the
///   `project` actions — Undo/Redo map to `syn.project.undo`/
///   `syn.project.redo`, synvelis#112 slice 2).
/// - **Arrangement** tab: the project-backed track/lane/clip
///   read-only `Table` ([`arrangement_table`]) resolved from
///   `state.arrangement_rows` — `selection: None`, so it issues no
///   intents.
/// - **Transport** tab: the status `Label` ([`transport_status_label`]),
///   Play/Stop `Button`s on the `transport` action target
///   ([`transport_button`]), and the `Seek` bound to
///   `transport/position` — `Begin → Preview → Commit/Cancel` gesture
///   dispatch through the real `syn.control.*` surface. The controlled
///   display is the seconds position of the authoritative projection
///   (see the adapter in `binding.rs`). Its range is
///   [`seek_range_seconds`] of `state.project_length_seconds`.
/// - **Mixer** tab: the master strip — the `Fader` bound to the
///   bootstrap master-strip fader target plus `Toggle`s for the
///   discovered `Mute`/`Solo` targets (see [`mixer_fader`],
///   [`mixer_strip`], [`mixer_strip_targets`]; synvelis#112 slice 3).
/// - **Inspector** tab: the target `Label` and breakdown `Table`
///   ([`inspector_target_label`]/[`inspector_table`]) resolved from
///   `state.inspector`, plus the control-relation readout `Label` and
///   relation `Table` ([`relation_label`]/[`relation_table`]) resolved
///   from `state.relations` (synvelis#102 UI-C5-01).
#[must_use]
pub fn main_screen_spec(state: &MainScreenState) -> ViewSpec {
    let seek_range = seek_range_seconds(state.project_length_seconds);
    ViewSpec {
        root: panel(
            "screen.root",
            Some("Synvelis"),
            vec![
                panel(
                    "screen.toolbar",
                    None,
                    vec![
                        project_button("project.save", "Save", "save"),
                        project_button("project.reload", "Reload", "reload"),
                        project_button("project.undo", "Undo", "undo"),
                        project_button("project.redo", "Redo", "redo"),
                    ],
                ),
                ComponentSpec::Tabs(TabsSpec {
                    key: key("screen.tabs"),
                    active: 0,
                    selection: Some(TabsSelectionSpec {
                        target: VIEW_STATE_TARGET.to_string(),
                    }),
                    tabs: vec![
                        TabSpec {
                            key: key("tab.arrangement"),
                            title: "Arrangement".to_string(),
                            child: Box::new(panel(
                                "tab.arrangement.panel",
                                None,
                                vec![arrangement_table(&state.arrangement_rows)],
                            )),
                        },
                        TabSpec {
                            key: key("tab.transport"),
                            title: "Transport".to_string(),
                            child: Box::new(panel(
                                "tab.transport.panel",
                                None,
                                vec![
                                    transport_status_label(&state.transport),
                                    panel(
                                        "transport.buttons",
                                        None,
                                        vec![
                                            transport_button("transport.play", "Play", "play"),
                                            transport_button("transport.stop", "Stop", "stop"),
                                        ],
                                    ),
                                    transport_seek(seek_range),
                                ],
                            )),
                        },
                        TabSpec {
                            key: key("tab.mixer"),
                            title: "Mixer".to_string(),
                            child: Box::new(panel(
                                "tab.mixer.panel",
                                None,
                                vec![mixer_strip(&state.mixer)],
                            )),
                        },
                        TabSpec {
                            key: key("tab.inspector"),
                            title: "Inspector".to_string(),
                            child: Box::new(panel(
                                "tab.inspector.panel",
                                None,
                                vec![
                                    inspector_target_label(&state.inspector),
                                    inspector_table(&state.inspector),
                                    relation_label(&state.relations),
                                    relation_table(&state.relations),
                                ],
                            )),
                        },
                        TabSpec {
                            key: key("tab.signal_flow"),
                            title: "Signal Flow".to_string(),
                            child: Box::new(panel(
                                "tab.signal_flow.panel",
                                None,
                                vec![
                                    signal_flow_status_label(&state.signal_flow),
                                    signal_flow_nodes_table(&state.signal_flow),
                                    signal_flow_edges_table(&state.signal_flow),
                                ],
                            )),
                        },
                    ],
                    visual: ComponentVisualState::default(),
                    a11y: A11ySpec {
                        role: Some(A11yRole::TabList),
                        ..A11ySpec::default()
                    },
                }),
            ],
        ),
    }
}

/// The secondary window's minimal screen spec (SV-GUI-04 slice 1,
/// synvelis#113) — `Panel > [status Label, transport buttons Panel,
/// Seek]`: a transport deck with a deliberately different region
/// layout from [`main_screen_spec`]'s toolbar/tabs tree, driven by
/// the same [`MainScreenState`] so both windows display the same
/// authoritative transport readout. The contract is identical:
/// `control_id` equals `target` on driven controls and every
/// `ComponentKey` is distinct within this spec.
#[must_use]
pub fn secondary_screen_spec(state: &MainScreenState) -> ViewSpec {
    let seek_range = seek_range_seconds(state.project_length_seconds);
    ViewSpec {
        root: panel(
            "screen2.root",
            Some("Synvelis — Secondary"),
            vec![
                transport_status_label(&state.transport),
                panel(
                    "transport2.buttons",
                    None,
                    vec![
                        transport_button("transport2.play", "Play", "play"),
                        transport_button("transport2.stop", "Stop", "stop"),
                    ],
                ),
                transport_seek_keyed("seek2.position", seek_range),
            ],
        ),
    }
}

/// Shared mount path for both product views — mint the `AttachEpoch`
/// inside from the composition-root, session-owned allocator (the
/// mandatory mint path: the allocator is passed, not an epoch, so a
/// caller cannot bypass it) and mount `spec`. Each call is a *new
/// host* for the same logical view — the monotonically minted epoch
/// keeps the fresh host's command-id namespace disjoint from every
/// earlier host's, so stale receipts are absorbed by the dead
/// namespace instead of mutating the live binding (RF387
/// `fresh_host_same_logical_id` contract).
fn mount_screen_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    spec: ViewSpec,
    label: &str,
) -> Result<ViewHost, UiError> {
    let mut host = ViewHost::new(instance, attach_epochs.borrow_mut().mint());
    host.mount(spec).map_err(|e| UiError::Internal {
        message: format!("{label} spec invalid: {e}"),
    })?;
    Ok(host)
}

/// Mounts the product main view onto a fresh [`ViewHost`], minting
/// the `AttachEpoch` inside from the composition-root, session-owned
/// allocator — the mandatory mint path (the allocator is passed, not
/// an epoch, so a caller cannot bypass it). Each call is a *new host*
/// for the same logical view — the monotonically minted epoch keeps
/// the fresh host's command-id namespace disjoint from every earlier
/// host's, so stale receipts (delayed `syn.control.receipt`
/// translations carrying an old `command_id`) are absorbed by the
/// dead namespace instead of mutating the live binding (RF387
/// `fresh_host_same_logical_id` contract).
///
/// Take the allocator handle from
/// [`SynvelisApp::attach_epoch_allocator`](crate::SynvelisApp::attach_epoch_allocator)
/// before the app seals, or from
/// [`NativeShell::attach_epoch_allocator`](crate::NativeShell::attach_epoch_allocator)
/// for mid-session recreation.
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation — a code
/// defect (the seek range is always finite and ordered).
pub fn mount_main_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    state: &MainScreenState,
) -> Result<ViewHost, UiError> {
    mount_screen_view(
        attach_epochs,
        main_view_instance(),
        main_screen_spec(state),
        "main screen",
    )
}

/// Mounts the product secondary view (the transport deck,
/// [`secondary_screen_spec`]) onto a fresh [`ViewHost`] — same
/// session-owned allocator mint contract as [`mount_main_view`], a
/// distinct logical view id ([`secondary_view_instance`]) and its
/// own command namespace. Used by the env-gated second native window
/// (SV-GUI-04 slice 1) and the two-window `FakeDriver` fixture.
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation — a code
/// defect.
pub fn mount_secondary_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    state: &MainScreenState,
) -> Result<ViewHost, UiError> {
    mount_screen_view(
        attach_epochs,
        secondary_view_instance(),
        secondary_screen_spec(state),
        "secondary screen",
    )
}
