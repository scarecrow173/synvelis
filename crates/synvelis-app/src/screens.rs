//! The product's minimal neutral screen spec (SV-GPUI-01 view-mount
//! slice, Issue #175 successor).
//!
//! [`main_screen_spec`] is the smallest *product-shaped* `ViewSpec`
//! mounted through the RF387 seam: `Panel > Tabs > Seek` driving the
//! real `transport/position` command target (`Begin → Preview →
//! Commit/Cancel` gesture dispatch; projection display is not yet
//! reflected by the RF renderer, see Ruviform #414) plus a `Fader` on a second tab
//! marked `unsupported` — the honest "known but undriven" state for a
//! mixer gain that no parameter target exists for yet.
//!
//! Contract notes baked into the spec:
//!
//! - `control_id` equals the binding `target` on driven controls.
//!   `syn.control.projection` rows translate to the view wire as
//!   `control_id: <target>` and `ViewHost::apply_projection`
//!   correlates by `control_id`, so a control whose `control_id`
//!   differs from its `target` would silently drop every projection
//!   (`crate::binding::notify_to_view_event`).
//! - The Mixer fader is `unsupported: true` — mutating inputs mint
//!   `Unavailable` notifications, never commands, so a target that
//!   does not exist in `syn.control.targets` is never dispatched
//!   (RF387 availability semantics; RF410 shared-`control_id` under-
//!   determination is avoided by giving every control a distinct id).
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
    A11yRole, A11ySpec, AttachEpochAllocator, ComponentKey, ComponentSpec, ComponentVisualState,
    ControlSpec, Orientation, PanelSpec, TabSpec, TabsSpec, ViewHost, ViewSpec,
};
use synvelis_workspace::controls_ui::TRANSPORT_POSITION_TARGET;

/// The single logical view this slice mounts — `ViewInstanceId(1)`
/// renders as `origin_view`/`view_id` `"view-1"` on the wire.
const MAIN_VIEW_INSTANCE_RAW: u64 = 1;

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

/// Targets the main view subscribes after attach — every driven
/// control's binding target. One entry per unique target (the
/// subscription is `(view, target)`, not per control).
#[must_use]
pub fn main_view_targets() -> Vec<String> {
    vec![TRANSPORT_POSITION_TARGET.to_string()]
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
/// — the one continuous target the boot project always exposes.
fn transport_seek() -> ComponentSpec {
    ComponentSpec::Seek(ControlSpec {
        key: key("seek.position"),
        control_id: Some(TRANSPORT_POSITION_TARGET.to_string()),
        target: Some(TRANSPORT_POSITION_TARGET.to_string()),
        descriptor: ValueDescriptor {
            default_value: Some(0.0),
            step: Some(1.0),
            ..ValueDescriptor::ranged(0.0, 240.0)
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
            value_max: Some(240.0),
            value_text: None,
        },
    })
}

/// The `Fader` the slice names — a mixer gain. No `mixer.*` parameter
/// target exists in the boot project (`syn.control.targets` lists only
/// `transport`/`transport/position`), so it is mounted
/// `unsupported: true`: rendered but uninputtable — the honest shape
/// of a control whose domain binding has not landed yet.
fn mixer_fader() -> ComponentSpec {
    ComponentSpec::Fader(ControlSpec {
        key: key("fader.gain"),
        control_id: Some("mixer.gain".to_string()),
        target: Some("mixer.gain".to_string()),
        descriptor: ValueDescriptor {
            default_value: Some(0.5),
            step: Some(0.05),
            ..ValueDescriptor::ranged(0.0, 1.0)
        },
        orientation: Orientation::Vertical,
        detach_policy: DetachPolicy::Cancel,
        unsupported: true,
        visual: ComponentVisualState::default(),
        a11y: a11y_slider("Gain", Orientation::Vertical),
    })
}

/// The product's minimal screen spec — `Panel > Tabs`:
///
/// - **Transport** tab: the `Seek` bound to `transport/position` —
///   `Begin → Preview → Commit/Cancel` gesture dispatch through the
///   real `syn.control.*` surface. Projection delivery reaches the
///   host binding; on-screen display follows once Ruviform #414 lands.
/// - **Mixer** tab: the `Fader` mounted `unsupported` (see
///   [`mixer_fader`]).
#[must_use]
pub fn main_screen_spec() -> ViewSpec {
    ViewSpec {
        root: panel(
            "screen.root",
            Some("Synvelis"),
            vec![ComponentSpec::Tabs(TabsSpec {
                key: key("screen.tabs"),
                active: 0,
                tabs: vec![
                    TabSpec {
                        key: key("tab.transport"),
                        title: "Transport".to_string(),
                        child: Box::new(panel("tab.transport.panel", None, vec![transport_seek()])),
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
            })],
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
/// defect (the spec is a compile-time constant).
pub fn mount_main_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
) -> Result<ViewHost, UiError> {
    let mut host = ViewHost::new(main_view_instance(), attach_epochs.borrow_mut().mint());
    host.mount(main_screen_spec())
        .map_err(|e| UiError::Internal {
            message: format!("main screen spec invalid: {e}"),
        })?;
    Ok(host)
}
