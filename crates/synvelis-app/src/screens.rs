//! The product's minimal neutral screen spec (SV-GPUI-01 view-mount
//! slice, Issue #175 successor; mixer gain wired by Issue #185).
//!
//! [`main_screen_spec`] is the smallest *product-shaped* `ViewSpec`
//! mounted through the RF387 seam: `Panel > [Toolbar, Tabs]` where the
//! toolbar holds Save/Reload `Button`s (the `project` actions) and the
//! Transport tab carries a `Seek` driving the real `transport/position`
//! command target (`Begin → Preview → Commit/Cancel` gesture dispatch;
//! the controlled value is the seconds position derived from the
//! authoritative projection). The Seek range follows the project
//! length ([`seek_range_seconds`]); a Mixer tab carries a `Fader`
//! bound to the bootstrap master-strip `ParameterRole::Fader` target
//! ([`mixer_gain_target`]) — its controlled display is the
//! authoritative `host_normalized` projection and commits land as
//! `SetParameterBase` through the same gesture path as every
//! parameter target.
//!
//! The spec is rebuilt on every project-length change and applied with
//! `ViewHost::update` (through `GpuiDriver::update_view`), so the mounted
//! controls keep their identity and no remount happens.
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
    A11yRole, A11ySpec, AttachEpochAllocator, ButtonSpec, ComponentKey, ComponentSpec,
    ComponentVisualState, ControlSpec, Orientation, PanelSpec, TabSpec, TabsSelectionSpec,
    TabsSpec, ViewHost, ViewSpec,
};
use synvelis_project::{MASTER_FADER_RAW, ParameterTargetId};
use synvelis_workspace::controls_ui::TRANSPORT_POSITION_TARGET;

use crate::binding::{PROJECT_TARGET, VIEW_STATE_TARGET};

/// The single logical view this slice mounts — `ViewInstanceId(1)`
/// renders as `origin_view`/`view_id` `"view-1"` on the wire.
const MAIN_VIEW_INSTANCE_RAW: u64 = 1;

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

/// Wire target of the mixer gain fader — the tagged id of the
/// bootstrap master-strip [`synvelis_project::ParameterRole::Fader`]
/// target (`param-<32hex>`), resolved by `syn.control.subscribe`/
/// `dispatch` like any parameter target (synvelis#185).
#[must_use]
pub fn mixer_gain_target() -> String {
    ParameterTargetId::new(MASTER_FADER_RAW).to_tagged()
}

/// Targets the main view subscribes after attach — every driven
/// control's binding target. One entry per unique target (the
/// subscription is `(view, target)`, not per control).
#[must_use]
pub fn main_view_targets() -> Vec<String> {
    vec![TRANSPORT_POSITION_TARGET.to_string(), mixer_gain_target()]
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
    ComponentSpec::Seek(ControlSpec {
        key: key("seek.position"),
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

/// The product's minimal screen spec — `Panel > [Toolbar, Tabs]`:
///
/// - **Toolbar**: Save and Reload `Button`s (the `project` actions).
/// - **Transport** tab: the `Seek` bound to `transport/position` —
///   `Begin → Preview → Commit/Cancel` gesture dispatch through the
///   real `syn.control.*` surface. The controlled display is the
///   seconds position of the authoritative projection (see the adapter
///   in `binding.rs`). Its range is [`seek_range_seconds`] of
///   `project_length_seconds`.
/// - **Mixer** tab: the `Fader` bound to the bootstrap master-strip
///   fader target (see [`mixer_fader`]).
#[must_use]
pub fn main_screen_spec(project_length_seconds: f64) -> ViewSpec {
    let seek_range = seek_range_seconds(project_length_seconds);
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
                            key: key("tab.transport"),
                            title: "Transport".to_string(),
                            child: Box::new(panel(
                                "tab.transport.panel",
                                None,
                                vec![transport_seek(seek_range)],
                            )),
                        },
                        TabSpec {
                            key: key("tab.mixer"),
                            title: "Mixer".to_string(),
                            child: Box::new(panel("tab.mixer.panel", None, vec![mixer_fader()])),
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
    project_length_seconds: f64,
) -> Result<ViewHost, UiError> {
    let mut host = ViewHost::new(main_view_instance(), attach_epochs.borrow_mut().mint());
    host.mount(main_screen_spec(project_length_seconds))
        .map_err(|e| UiError::Internal {
            message: format!("main screen spec invalid: {e}"),
        })?;
    Ok(host)
}
