//! Keymap dispatch — SV-GUI-03-07 (Issue #112).
//!
//! Headless key→command layer between the OS/GPUI key event source
//! and [`WorkspaceManager::dispatch`], implementing the observable
//! contract of `docs/designs/sv-gui-03-06-keyboard-gesture-design.md`:
//!
//! - §3.2 default binding table in *logical* modifier form — `Mod`
//!   resolves to Ctrl on Windows/Linux and Cmd on macOS (§3.3);
//! - §4.1 `InputContext` × lane firing matrix, applied as a single
//!   keymap-layer gate (KG-D-4 — the contract fixes observable
//!   behaviour, not which layer owns it);
//! - §4.2 pipeline: text/IME consumption → `Esc` reserved key →
//!   binding match → repeat gate → lane gate → gesture-preview gate
//!   → `CommandContext` → `dispatch`, with `CommandOutcome` returned
//!   verbatim (§4.5);
//! - §5.3 auto-repeat policy — only `view.scroll-*`, `edit.undo`,
//!   `edit.redo` accept repeat re-fires;
//! - §5.4/KG-C-5 remap conflict rejection — lane-internal uniqueness
//!   and OS-reserved chords are refused with a reason, never
//!   silently overwritten.
//!
//! Deferred (other slices/issues): real OS/GPUI key events and
//! physical key-name resolution for non-US layouts, chord/multi-stroke
//! sequences, remap persistence and the settings UI, and the menu
//! model itself (#114 GUI-L2 / #176 EXT-L3 / later slices). The
//! [`menu_coverage`] table and [`Keymap::accelerator_label`] are the
//! headless contract surface for §6 menu coverage — they describe
//! coverage, they do not implement menus.

use ruviform_ui::window::WindowHandle;

use crate::command::{CommandContext, CommandKind, CommandOutcome, spec_for};
use crate::focus::InputContext;
use crate::gesture::CancelReason;
use crate::workspace::WorkspaceManager;

/// OS family — selects which physical modifier `Mod` resolves to
/// (§3.3). Windows and Linux share Ctrl; macOS uses Cmd.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OsFamily {
    /// Windows — `Mod` is Ctrl.
    Windows,
    /// Linux — `Mod` is Ctrl.
    Linux,
    /// macOS — `Mod` is Cmd.
    MacOs,
}

impl OsFamily {
    /// Whether the primary (`Mod`) modifier is the Command/Super key
    /// on this OS.
    #[must_use]
    fn mod_is_command(self) -> bool {
        matches!(self, Self::MacOs)
    }
}

/// Physical modifier bitmask as reported by the OS/GPUI adapter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PhysicalModifiers(u8);

impl PhysicalModifiers {
    /// No modifiers.
    pub const NONE: Self = Self(0);
    /// Physical Ctrl.
    pub const CONTROL: Self = Self(0b0001);
    /// Physical Alt/Option.
    pub const ALT: Self = Self(0b0010);
    /// Physical Shift.
    pub const SHIFT: Self = Self(0b0100);
    /// Physical Command on macOS / Super (Win-key) elsewhere.
    pub const COMMAND: Self = Self(0b1000);

    /// Whether every flag in `other` is set.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for PhysicalModifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Contract key names (§3.2 notation).
///
/// Printable keys are [`Key::Char`]; named variants cover the
/// non-printable set the default map and the OS-reservation table
/// need. Letters in [`Key::Char`] match ASCII-case-insensitively —
/// Shift is carried separately as a modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// `Space`.
    Space,
    /// `Enter`/`Return`.
    Enter,
    /// `Esc`.
    Escape,
    /// `Delete` (forward delete).
    Delete,
    /// `Backspace`.
    Backspace,
    /// `Tab`.
    Tab,
    /// Arrow keys.
    ArrowLeft,
    /// Arrow keys.
    ArrowRight,
    /// Arrow keys.
    ArrowUp,
    /// Arrow keys.
    ArrowDown,
    /// Function keys — `Function(4)` is F4.
    Function(u8),
    /// A printable character key.
    Char(char),
}

impl Key {
    /// ASCII-lowercases `Char` keys so letter matching ignores the
    /// Shift-produced case (Shift itself stays a separate modifier).
    fn normalized(self) -> Self {
        match self {
            Self::Char(c) => Self::Char(c.to_ascii_lowercase()),
            other => other,
        }
    }
}

/// Logical modifiers — the OS-independent form bindings are stored
/// and remaps are validated in (§3.3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LogicalModifiers {
    /// `Mod` — Ctrl on Windows/Linux, Cmd on macOS.
    pub primary: bool,
    /// Shift.
    pub shift: bool,
    /// Alt/Option.
    pub alt: bool,
}

/// A key chord in logical (OS-independent) form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyChord {
    /// The key.
    pub key: Key,
    /// Logical modifiers.
    pub modifiers: LogicalModifiers,
}

impl KeyChord {
    /// Normalizes a physical stroke for `os`. Returns `None` when a
    /// modifier outside the logical vocabulary is held — e.g. the
    /// non-primary Ctrl/Cmd pair or the Win/Super key — because no
    /// binding can represent such a chord.
    fn normalize(os: OsFamily, stroke: &KeyStroke) -> Option<Self> {
        let m = stroke.modifiers;
        let primary_is_command = os.mod_is_command();
        let primary = m.contains(if primary_is_command {
            PhysicalModifiers::COMMAND
        } else {
            PhysicalModifiers::CONTROL
        });
        let other = m.contains(if primary_is_command {
            PhysicalModifiers::CONTROL
        } else {
            PhysicalModifiers::COMMAND
        });
        if other {
            return None;
        }
        Some(Self {
            key: stroke.key.normalized(),
            modifiers: LogicalModifiers {
                primary,
                shift: m.contains(PhysicalModifiers::SHIFT),
                alt: m.contains(PhysicalModifiers::ALT),
            },
        })
    }
}

/// A raw key stroke from the OS/GPUI adapter: physical modifiers plus
/// the OS auto-repeat flag.
#[derive(Debug, Clone, Copy)]
pub struct KeyStroke {
    /// The key.
    pub key: Key,
    /// Physical modifier state.
    pub modifiers: PhysicalModifiers,
    /// `true` when this event is an OS auto-repeat re-fire (§5.3).
    pub repeat: bool,
}

/// One binding: logical chord → contract command id (+arg).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// The logical chord.
    pub chord: KeyChord,
    /// Contract command id — always a [`spec_for`] vocabulary entry.
    pub command: String,
    /// Command argument (`tool.activate` carries the tool name).
    pub arg: Option<String>,
}

/// The keymap dispatch verdict — distinguishable outcomes, never a
/// fake success (§4.5).
#[derive(Debug)]
pub enum KeyOutcome {
    /// §4.2 step 1 / KG-C-6(1): the stroke belongs to the text/IME
    /// lane — `TextInput`/`Ime` contexts deliver unmodified keys to
    /// the field before keymap matching ever sees them.
    TextLane,
    /// KG-C-6(4): no binding matches the normalized chord — a no-op
    /// that leaves room for view-local processing.
    Unbound,
    /// §5.3: an OS auto-repeat re-fire of an edge-triggered command.
    RepeatIgnored,
    /// §5.1: `Esc` cancelled the innermost transient gesture preview.
    TransientCancelled,
    /// §5.1: `Esc` found no transient — reserved-key no-op. Dock drags
    /// and modal/menu dismissal are owned outside this layer.
    NoTransient,
    /// §4.1 / §5.1: the firing matrix or the gesture-preview gate
    /// suppressed the matched binding — nothing was dispatched.
    Suppressed {
        /// Why firing was suppressed.
        reason: &'static str,
    },
    /// A contract command was issued to `dispatch`; the outcome is
    /// observed verbatim — `Suppressed`/`Unresolved`/`Rejected` are
    /// never swallowed or converted (§4.5).
    Dispatched(CommandOutcome),
}

/// Why a remap assignment was rejected (KG-C-5) — the reason is part
/// of the contract; implicit overwrites never happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindRejection {
    /// The chord is already bound — lane-internal uniqueness
    /// (KG-C-1) is enforced table-wide because bindings are global:
    /// context gating derives from the command kind, so a duplicate
    /// chord could never resolve deterministically.
    Conflict {
        /// The command already holding the chord.
        command: String,
    },
    /// The chord collides with an OS/WM reservation on the given OS
    /// (§3.3, KG-C-4).
    OsReserved,
    /// The command id is not in the `spec_for` vocabulary.
    UnknownCommand(String),
}

/// The keymap: the §3.2 default binding table plus the dispatch
/// pipeline. Remaps may be added through [`Keymap::try_bind`];
/// persistence and the settings UI are a later slice.
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: Vec<Binding>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::standard()
    }
}

impl Keymap {
    /// The §3.2 default keymap (KG-KEY-01…15).
    #[must_use]
    pub fn standard() -> Self {
        Self {
            bindings: default_bindings(),
        }
    }

    /// Every binding in the table (defaults plus accepted remaps).
    #[must_use]
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// The binding for a physical stroke under `os`, or `None` when
    /// the chord is unbound or unrepresentable in logical form.
    #[must_use]
    pub fn lookup(&self, os: OsFamily, stroke: &KeyStroke) -> Option<&Binding> {
        let chord = KeyChord::normalize(os, stroke)?;
        self.bindings.iter().find(|b| b.chord == chord)
    }

    /// §6.2/CM-2: accelerator display for `command` in OS-conventional
    /// notation (`⌘`/`Ctrl` family), or `None` when the command has
    /// no default binding (e.g. `project.export`, KG-KEY-14).
    #[must_use]
    pub fn accelerator_label(&self, os: OsFamily, command: &str) -> Option<String> {
        let binding = self.bindings.iter().find(|b| b.command == command)?;
        Some(accelerator_label(os, &binding.chord))
    }

    /// KG-C-5 remap validation: rejects assignments that would break
    /// lane-internal uniqueness, collide with an OS reservation on
    /// `os`, or name an unregistered command — with a reason, never
    /// an implicit overwrite. Accepted assignments append to the
    /// table (replacing nothing; callers unbind explicitly in a
    /// later slice if needed).
    ///
    /// # Errors
    /// [`BindRejection`] describing why the assignment was refused.
    pub fn try_bind(
        &mut self,
        os: OsFamily,
        chord: KeyChord,
        command: &str,
        arg: Option<String>,
    ) -> Result<(), BindRejection> {
        if spec_for(command).is_none() {
            return Err(BindRejection::UnknownCommand(command.to_string()));
        }
        let chord = KeyChord {
            key: chord.key.normalized(),
            modifiers: chord.modifiers,
        };
        if os_reserved(os, &chord) {
            return Err(BindRejection::OsReserved);
        }
        if let Some(existing) = self.bindings.iter().find(|b| b.chord == chord) {
            return Err(BindRejection::Conflict {
                command: existing.command.clone(),
            });
        }
        self.bindings.push(Binding {
            chord,
            command: command.to_string(),
            arg,
        });
        Ok(())
    }

    /// Runs one key stroke through the §4.2 pipeline against `ws`.
    ///
    /// Pipeline order: text/IME consumption (§4.2 step 1) → `Esc`
    /// reserved key (§5.1) → binding match → auto-repeat gate (§5.3)
    /// → §4.1 lane×context gate → §5.1 gesture-preview gate →
    /// `CommandContext` → [`WorkspaceManager::dispatch`].
    pub fn dispatch_key<H: WindowHandle>(
        &self,
        ws: &mut WorkspaceManager<H>,
        os: OsFamily,
        stroke: &KeyStroke,
    ) -> KeyOutcome {
        let input = ws.focus().input;
        // §4.2 step 1: in TextInput/Ime only Mod-combos reach keymap
        // matching; every other key belongs to the text/IME lane.
        let primary_flag = if os.mod_is_command() {
            PhysicalModifiers::COMMAND
        } else {
            PhysicalModifiers::CONTROL
        };
        let has_mod = stroke.modifiers.contains(primary_flag);
        if matches!(input, InputContext::TextInput | InputContext::Ime) && !has_mod {
            return KeyOutcome::TextLane;
        }
        // §5.1: a completely bare Esc is the reserved
        // transient-cancel key — any modifier makes it an ordinary
        // (unbound) chord.
        if stroke.key == Key::Escape && stroke.modifiers == PhysicalModifiers::NONE {
            return Self::dispatch_escape(ws);
        }
        let Some(binding) = self.lookup(os, stroke) else {
            return KeyOutcome::Unbound;
        };
        // §5.3: auto-repeat is accepted only for the scrolling and
        // undo/redo command ids; everything else is edge-triggered.
        if stroke.repeat && !accepts_repeat(&binding.command) {
            return KeyOutcome::RepeatIgnored;
        }
        let Some(spec) = spec_for(&binding.command) else {
            // try_bind validates ids, so this is unreachable for a
            // well-formed table — treat as unbound rather than panic.
            return KeyOutcome::Unbound;
        };
        if !lane_fires(spec.kind, input) {
            return KeyOutcome::Suppressed {
                reason: suppression_reason(spec.kind),
            };
        }
        // §5.1: an active gesture preview suppresses editing bindings
        // on its own view — edits must flow through commit/cancel.
        if spec.kind == CommandKind::Editing
            && let Some(view) = ws.focus().active_view
            && gesture_active(ws, view)
        {
            return KeyOutcome::Suppressed {
                reason: "gesture preview active",
            };
        }
        let ctx = CommandContext {
            id: binding.command.clone(),
            origin_window: ws.focus().native_window,
            origin_view: ws.focus().active_view,
            input,
            explicit_targets: Vec::new(),
            selection: None,
            expected_revision: None,
            arg: binding.arg.clone(),
        };
        KeyOutcome::Dispatched(ws.dispatch(ctx))
    }

    /// §5.1 `Esc`: cancels the innermost transient — the gesture
    /// preview on the focused view. Gesture types are mutually
    /// exclusive per view (`pointer_down` rejects a second gesture),
    /// so the check order is only a deterministic tiebreak. Dock
    /// drags and modal/menu dismissal live outside this layer and
    /// fall through to the no-transient no-op.
    fn dispatch_escape<H: WindowHandle>(ws: &mut WorkspaceManager<H>) -> KeyOutcome {
        let Some(view) = ws.focus().active_view else {
            return KeyOutcome::NoTransient;
        };
        let transient = ws.state(view).ok().map(|s| {
            (
                s.gesture.is_some(),
                s.control_edge_gesture.is_some(),
                s.routing_gesture.is_some(),
            )
        });
        let Some((gesture, control_edge, routing)) = transient else {
            return KeyOutcome::NoTransient;
        };
        let cancelled = if gesture {
            ws.cancel_gesture(view, CancelReason::Escape).is_ok()
        } else if control_edge {
            ws.control_edge_cancel(view, CancelReason::Escape).is_ok()
        } else if routing {
            ws.routing_gesture_cancel(view, CancelReason::Escape)
                .is_ok()
        } else {
            false
        };
        if cancelled {
            KeyOutcome::TransientCancelled
        } else {
            KeyOutcome::NoTransient
        }
    }
}

/// §4.1 firing matrix: whether a command of `kind` may fire under
/// `input`. This is the single authoritative copy of the matrix —
/// `dispatch` keeps its own Project/Transport/Editing suppression
/// for non-keymap callers, and the two agree by construction.
fn lane_fires(kind: CommandKind, input: InputContext) -> bool {
    match kind {
        CommandKind::Undo => matches!(
            input,
            InputContext::View | InputContext::TextInput | InputContext::Ime
        ),
        CommandKind::View => input == InputContext::View,
        CommandKind::Application => matches!(
            input,
            InputContext::View | InputContext::TextInput | InputContext::Modal
        ),
        CommandKind::Project | CommandKind::Transport | CommandKind::Editing => {
            input == InputContext::View
        }
    }
}

/// Human-readable reason for a §4.1 lane-gate suppression.
fn suppression_reason(kind: CommandKind) -> &'static str {
    match kind {
        CommandKind::Undo => "input context captures undo",
        CommandKind::Application => "application lane suppressed in this input context",
        CommandKind::View => "view lane commands require View input context",
        CommandKind::Project | CommandKind::Transport | CommandKind::Editing => {
            "input context captures daw commands"
        }
    }
}

/// §5.3: the command ids that accept OS auto-repeat re-fires.
fn accepts_repeat(command: &str) -> bool {
    matches!(
        command,
        "view.scroll-left"
            | "view.scroll-right"
            | "view.scroll-up"
            | "view.scroll-down"
            | "edit.undo"
            | "edit.redo"
    )
}

/// Whether any gesture preview is active on `view` (§5.1).
fn gesture_active<H: WindowHandle>(
    ws: &WorkspaceManager<H>,
    view: ruviform_ui::dock::ViewInstanceId,
) -> bool {
    ws.state(view).is_ok_and(|s| {
        s.gesture.is_some() || s.control_edge_gesture.is_some() || s.routing_gesture.is_some()
    })
}

/// The §3.2 default binding table. `Delete`/`Backspace` and
/// `Mod+=`/`Mod+Shift+=` are deliberate same-command aliases (KG-KEY
/// contract); every other chord is unique — verified by TS-EX-01.
fn default_bindings() -> Vec<Binding> {
    fn bind(key: Key, modifiers: LogicalModifiers, command: &str, arg: Option<&str>) -> Binding {
        Binding {
            chord: KeyChord { key, modifiers },
            command: command.to_string(),
            arg: arg.map(str::to_string),
        }
    }
    let plain = LogicalModifiers::default();
    let mod_ = LogicalModifiers {
        primary: true,
        shift: false,
        alt: false,
    };
    let mod_shift = LogicalModifiers {
        primary: true,
        shift: true,
        alt: false,
    };
    vec![
        // KG-KEY-01…03 — transport
        bind(Key::Space, plain, "transport.toggle", None),
        bind(Key::Enter, plain, "transport.stop-return", None),
        bind(Key::Char('r'), plain, "transport.record", None),
        // KG-KEY-04…05 — undo/redo
        bind(Key::Char('z'), mod_, "edit.undo", None),
        bind(Key::Char('z'), mod_shift, "edit.redo", None),
        // KG-KEY-06 — delete (two aliases for the same command)
        bind(Key::Delete, plain, "edit.delete", None),
        bind(Key::Backspace, plain, "edit.delete", None),
        // KG-KEY-07 — clipboard
        bind(Key::Char('c'), mod_, "edit.copy", None),
        bind(Key::Char('v'), mod_, "edit.paste", None),
        // KG-KEY-09 — tool switching
        bind(Key::Char('v'), plain, "tool.activate", Some("smart")),
        bind(Key::Char('b'), plain, "tool.activate", Some("draw")),
        bind(Key::Char('s'), plain, "tool.activate", Some("split")),
        bind(Key::Char('t'), plain, "tool.activate", Some("trim")),
        bind(Key::Char('e'), plain, "tool.activate", Some("erase")),
        // KG-KEY-10…11 — zoom (Mod+Shift+= is the same binding's alias)
        bind(Key::Char('='), mod_, "view.zoom-in", None),
        bind(Key::Char('='), mod_shift, "view.zoom-in", None),
        bind(Key::Char('-'), mod_, "view.zoom-out", None),
        // KG-KEY-12 — scroll
        bind(Key::ArrowLeft, plain, "view.scroll-left", None),
        bind(Key::ArrowRight, plain, "view.scroll-right", None),
        bind(Key::ArrowUp, plain, "view.scroll-up", None),
        bind(Key::ArrowDown, plain, "view.scroll-down", None),
        // KG-KEY-13…15 — save / quit; export has no default bind
        bind(Key::Char('s'), mod_, "project.save", None),
        bind(Key::Char('q'), mod_, "app.quit", None),
    ]
}

/// §3.3 OS/WM reservations, evaluated on the *physical* expansion of
/// a logical chord for `os`. The reservation set is the contract's
/// own list — physical key names beyond this vocabulary (non-US
/// layouts, chords) are a later slice.
fn os_reserved(os: OsFamily, chord: &KeyChord) -> bool {
    let m = chord.modifiers;
    // Expand `Mod` to the OS's physical primary modifier; the other
    // primary-family modifier is unrepresentable and already excluded
    // by normalization.
    let control = m.primary && !os.mod_is_command();
    let command = m.primary && os.mod_is_command();
    let alt = m.alt;
    let shift = m.shift;
    let key = chord.key;
    let char_is = |c: char| matches!(key, Key::Char(k) if k.eq_ignore_ascii_case(&c));
    match os {
        OsFamily::Windows | OsFamily::Linux => {
            // Alt+F4 / Alt+Tab / Ctrl+Alt+Del / Ctrl+Esc.
            // (Super/Win-key combos cannot be expressed in logical
            // form — normalization already rejects them.)
            if alt && !control && !shift && !command {
                return matches!(key, Key::Function(4) | Key::Tab);
            }
            if control && alt && !shift && !command && key == Key::Delete {
                return true;
            }
            control && !alt && !shift && !command && key == Key::Escape
        }
        OsFamily::MacOs => {
            // Cmd+Space (Spotlight) / Cmd+Tab / Cmd+H / Cmd+M.
            command
                && !control
                && !alt
                && !shift
                && (matches!(key, Key::Space | Key::Tab) || char_is('h') || char_is('m'))
        }
    }
}

/// OS-conventional accelerator label for a logical chord (§6.2/CM-2).
fn accelerator_label(os: OsFamily, chord: &KeyChord) -> String {
    let m = chord.modifiers;
    let mut label = String::new();
    match os {
        OsFamily::MacOs => {
            if m.primary {
                label.push('⌘');
            }
            if m.alt {
                label.push('⌥');
            }
            if m.shift {
                label.push('⇧');
            }
        }
        OsFamily::Windows | OsFamily::Linux => {
            let mut parts = Vec::new();
            if m.primary {
                parts.push("Ctrl");
            }
            if m.alt {
                parts.push("Alt");
            }
            if m.shift {
                parts.push("Shift");
            }
            label.push_str(&parts.join("+"));
            if !parts.is_empty() {
                label.push('+');
            }
        }
    }
    label.push_str(&key_label(chord.key));
    label
}

/// Display name for a key inside an accelerator label.
fn key_label(key: Key) -> String {
    match key {
        Key::Char(c) if c.is_ascii_alphabetic() => c.to_ascii_uppercase().to_string(),
        Key::Char(c) => c.to_string(),
        Key::Space => "Space".to_string(),
        Key::Enter => "Enter".to_string(),
        Key::Escape => "Esc".to_string(),
        Key::Delete => "Delete".to_string(),
        Key::Backspace => "Backspace".to_string(),
        Key::Tab => "Tab".to_string(),
        Key::ArrowLeft => "Left".to_string(),
        Key::ArrowRight => "Right".to_string(),
        Key::ArrowUp => "Up".to_string(),
        Key::ArrowDown => "Down".to_string(),
        Key::Function(n) => format!("F{n}"),
    }
}

/// Menu surface kinds from §6.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSurface {
    /// Application menu bar (MB).
    MenuBar,
    /// Arrangement context menu (AC).
    ArrangementMenu,
    /// View/pane context menu (VC).
    ViewMenu,
}

/// §6.2 coverage inventory entry for one contract command id —
/// which menu surfaces the command is expected to appear on, and
/// why it has none or optional placement.
#[derive(Debug, Clone, Copy)]
pub struct MenuCoverage {
    /// Menu surfaces (§6.2 MB/AC/VC); empty = no menu surface.
    pub surfaces: &'static [MenuSurface],
    /// Placement note or the reason there is no surface.
    pub note: Option<&'static str>,
}

const MB: &[MenuSurface] = &[MenuSurface::MenuBar];
const AC: &[MenuSurface] = &[MenuSurface::ArrangementMenu];
const MB_AC_VC: &[MenuSurface] = &[
    MenuSurface::MenuBar,
    MenuSurface::ArrangementMenu,
    MenuSurface::ViewMenu,
];
const NONE: &[MenuSurface] = &[];

/// §6.2 coverage inventory — every contract catalog id maps to its
/// menu surfaces (or an explicit no-menu reason). This is the
/// headless contract surface for TS-EX-10: the menu model itself
/// lives in `synvelis-app` and is a later slice, so coverage here is
/// data + verification, not a menu implementation.
#[must_use]
pub fn menu_coverage(command: &str) -> Option<MenuCoverage> {
    use MenuSurface as S;
    let coverage = match command {
        "app.quit" => MenuCoverage {
            surfaces: MB,
            note: Some("Application > Quit"),
        },
        "project.save" => MenuCoverage {
            surfaces: MB,
            note: Some("File > Save"),
        },
        "project.export" => MenuCoverage {
            surfaces: MB,
            note: Some("File > Export — no default key (KG-KEY-14)"),
        },
        "project.rename" => MenuCoverage {
            surfaces: MB,
            note: Some("no default key"),
        },
        "edit.undo" => MenuCoverage {
            surfaces: MB,
            note: Some("Edit > Undo"),
        },
        "edit.redo" => MenuCoverage {
            surfaces: MB,
            note: Some("Edit > Redo"),
        },
        "edit.copy" | "edit.paste" | "edit.delete" => MenuCoverage {
            surfaces: MB_AC_VC,
            note: Some("Edit menu + arrangement/view context menus"),
        },
        "clip.remove" | "clip.mute" => MenuCoverage {
            surfaces: AC,
            note: Some("arrangement context menu only"),
        },
        "clips.delete-range" => MenuCoverage {
            surfaces: &[S::MenuBar, S::ArrangementMenu],
            note: Some("Edit > Delete Range + context menu"),
        },
        "transport.toggle" | "transport.stop-return" | "transport.record" => MenuCoverage {
            surfaces: MB,
            note: Some("Transport menu"),
        },
        // Internal resolution targets — menu items point at the
        // contract ids (toggle/stop-return), never at these.
        "transport.play" | "transport.stop" => MenuCoverage {
            surfaces: NONE,
            note: Some("internal resolution target — no direct menu item"),
        },
        "tool.activate" => MenuCoverage {
            surfaces: &[S::MenuBar, S::ViewMenu],
            note: Some("View > Tool submenu + tool picker"),
        },
        "view.zoom-in" | "view.zoom-out" => MenuCoverage {
            surfaces: MB,
            note: Some("View > Zoom"),
        },
        "view.scroll-left" | "view.scroll-right" | "view.scroll-up" | "view.scroll-down" => {
            MenuCoverage {
                surfaces: MB,
                note: Some("View > Scroll — omissible per §6.2"),
            }
        }
        "view.activate" => MenuCoverage {
            surfaces: MB,
            note: Some("Window > Tabs/View list"),
        },
        _ => return None,
    };
    Some(coverage)
}
