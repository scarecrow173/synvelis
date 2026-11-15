//! SV-GUI-03-07 keymap dispatch tests —
//! `docs/test-specs/sv-gui-03-06-keyboard-gesture.md` TS-EX-01…11
//! (Issue #112). Headless only: `CounterIdGenerator` ids,
//! `FakeWindowHost`, semantic `HitProbe`s — no OS key events, no
//! pixels, no IME (TS-GUI-* stay deferred to #114/#176).

use std::collections::HashSet;

use ruviform_ui::dock::{DockTarget, ViewInstanceId};
use ruviform_ui::event::HostEvent;
use ruviform_ui::window::WindowId;
use synvelis_project::{
    ClipExtent, ClipId, ClipPlacement, ClipPlayback, CounterIdGenerator, LaneId, MusicalDuration,
    MusicalTick, PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, SourceFrame, SourceRange, TimeAnchor, TrackId,
};
use synvelis_workspace::{
    BindRejection, CommandOutcome, EditingSelection, FakeWindowHost, HitCandidate, HitProbe,
    HitZone, InputContext, Key, KeyChord, KeyOutcome, KeyStroke, Keymap, LogicalModifiers,
    MenuSurface, OsFamily, PhysicalModifiers, ProjectSession, SemanticTarget, Tool, ViewType,
    WorkspaceError, WorkspaceManager, command::spec_for, menu_coverage,
};

// ---------------------------------------------------------------------------
// fixture (mirrors ws_a.rs / ed_a.rs conventions)
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;
const OS: OsFamily = OsFamily::Windows;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xA11CE), "keymap-test");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    WorkspaceManager::new(
        FakeWindowHost::with_monitors(vec![ruviform_ui::window::MonitorInfo {
            name: Some("primary".to_string()),
            width: 2560,
            height: 1440,
            x: 0,
            y: 0,
            scale_factor: 1.0,
        }]),
        ProjectSession::new(controller),
    )
}

fn apply(ws: &mut Ws, op: ProjectOp) -> ProjectRevision {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op")
}

fn natural_playback() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::ZERO,
            SourceFrame::new(1_000_000).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

/// track + lane + source + one musical clip.
fn seeded() -> (Ws, TrackId, LaneId, ClipId) {
    let mut ws = fresh_ws();
    apply(
        &mut ws,
        ProjectOp::RegisterAsset {
            digest: "deadbeef".to_string(),
        },
    );
    let asset_id = *ws.session().project().assets.keys().next().unwrap();
    apply(
        &mut ws,
        ProjectOp::RegisterSource {
            asset_id,
            content_digest: "deadbeef".to_string(),
        },
    );
    let source_id = *ws.session().project().sources.keys().next().unwrap();
    apply(
        &mut ws,
        ProjectOp::AddTrack {
            name: "t1".to_string(),
        },
    );
    let track_id = *ws.session().project().tracks.keys().next().unwrap();
    apply(&mut ws, ProjectOp::AddLane { track_id });
    let lane_id = *ws.session().project().lanes.keys().next().unwrap();
    apply(
        &mut ws,
        ProjectOp::AddClip {
            lane_id,
            source_id,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                extent: ClipExtent::Musical(MusicalDuration::new(960).unwrap()),
            },
            playback: natural_playback(),
        },
    );
    let clip_id = *ws.session().project().clips.keys().next().unwrap();
    (ws, track_id, lane_id, clip_id)
}

/// Window + single Arrangement view, natively focused (View input).
fn focused_arrangement() -> (Ws, WindowId, ViewInstanceId, TrackId, LaneId, ClipId) {
    let (mut ws, track, lane, clip) = seeded();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let arr = ws
        .spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win))
        .unwrap();
    ws.on_window_event(win, &HostEvent::Focused(true)).unwrap();
    (ws, win, arr, track, lane, clip)
}

fn pt(tick: i64) -> synvelis_workspace::EditTimePoint {
    synvelis_workspace::EditTimePoint::Musical(MusicalTick::new(tick))
}

fn body_probe(pos: i64, target: SemanticTarget) -> HitProbe {
    HitProbe {
        position: pt(pos),
        candidates: vec![HitCandidate {
            zone: HitZone::Body,
            target: Some(target),
        }],
    }
}

// -- stroke builders -------------------------------------------------------

fn key(k: Key) -> KeyStroke {
    KeyStroke {
        key: k,
        modifiers: PhysicalModifiers::default(),
        repeat: false,
    }
}

fn ctrl(k: Key) -> KeyStroke {
    KeyStroke {
        key: k,
        modifiers: PhysicalModifiers::CONTROL,
        repeat: false,
    }
}

fn ctrl_shift(k: Key) -> KeyStroke {
    KeyStroke {
        key: k,
        modifiers: PhysicalModifiers::CONTROL | PhysicalModifiers::SHIFT,
        repeat: false,
    }
}

/// macOS primary modifier (Cmd).
fn cmd(k: Key) -> KeyStroke {
    KeyStroke {
        key: k,
        modifiers: PhysicalModifiers::COMMAND,
        repeat: false,
    }
}

fn repeated(mut s: KeyStroke) -> KeyStroke {
    s.repeat = true;
    s
}

fn press<H: ruviform_ui::window::WindowHandle>(
    km: &Keymap,
    ws: &mut WorkspaceManager<H>,
    os: OsFamily,
    stroke: KeyStroke,
) -> KeyOutcome {
    km.dispatch_key(ws, os, &stroke)
}

fn dispatched(outcome: KeyOutcome) -> CommandOutcome {
    match outcome {
        KeyOutcome::Dispatched(inner) => inner,
        other => panic!("expected Dispatched, got {other:?}"),
    }
}

/// Asserts every stroke is lane-suppressed under the current input
/// context.
fn assert_suppressed_all(km: &Keymap, ws: &mut Ws, ctx: &str, strokes: &[KeyStroke]) {
    for &stroke in strokes {
        assert!(
            matches!(press(km, ws, OS, stroke), KeyOutcome::Suppressed { .. }),
            "{stroke:?} must be suppressed in {ctx}"
        );
    }
}

// ---------------------------------------------------------------------------
// TS-EX-01 — lane-internal uniqueness of the default keymap
// ---------------------------------------------------------------------------

#[test]
fn ts_ex_01_default_bindings_are_unique_and_registered() {
    let km = Keymap::standard();
    let mut chords = HashSet::new();
    for binding in km.bindings() {
        assert!(
            chords.insert(binding.chord),
            "duplicate chord {:?} → {}",
            binding.chord,
            binding.command
        );
        assert!(
            spec_for(&binding.command).is_some(),
            "binding {} is not a registered command id",
            binding.command
        );
    }
    // The contract's alias pairs must both be present.
    let delete_binds: Vec<_> = km
        .bindings()
        .iter()
        .filter(|b| b.command == "edit.delete")
        .collect();
    assert_eq!(delete_binds.len(), 2);
    let zoom_binds: Vec<_> = km
        .bindings()
        .iter()
        .filter(|b| b.command == "view.zoom-in")
        .collect();
    assert_eq!(zoom_binds.len(), 2);
}

// ---------------------------------------------------------------------------
// TS-EX-02 — §7.2 operation → command coverage
// ---------------------------------------------------------------------------

#[test]
fn ts_ex_02_section_7_2_operations_have_contract_commands() {
    let km = Keymap::standard();
    let cases: &[(KeyStroke, &str)] = &[
        (key(Key::Space), "transport.toggle"),
        (key(Key::Enter), "transport.stop-return"),
        (key(Key::Char('r')), "transport.record"),
        (ctrl(Key::Char('z')), "edit.undo"),
        (ctrl_shift(Key::Char('z')), "edit.redo"),
        (key(Key::Delete), "edit.delete"),
        (key(Key::Backspace), "edit.delete"),
        (ctrl(Key::Char('c')), "edit.copy"),
        (ctrl(Key::Char('v')), "edit.paste"),
        (key(Key::Char('v')), "tool.activate"),
        (key(Key::Char('b')), "tool.activate"),
        (key(Key::Char('s')), "tool.activate"),
        (key(Key::Char('t')), "tool.activate"),
        (key(Key::Char('e')), "tool.activate"),
        (ctrl(Key::Char('=')), "view.zoom-in"),
        (ctrl(Key::Char('-')), "view.zoom-out"),
        (key(Key::ArrowLeft), "view.scroll-left"),
        (key(Key::ArrowRight), "view.scroll-right"),
        (key(Key::ArrowUp), "view.scroll-up"),
        (key(Key::ArrowDown), "view.scroll-down"),
        (ctrl(Key::Char('s')), "project.save"),
        (ctrl(Key::Char('q')), "app.quit"),
    ];
    for (stroke, command) in cases {
        let binding = km
            .lookup(OS, stroke)
            .unwrap_or_else(|| panic!("no binding for {stroke:?}"));
        assert_eq!(binding.command, *command, "stroke {stroke:?}");
    }
    // Contract commands with no default bind (KG-KEY-14 export, and
    // the ids the design leaves unbound) must not be bound.
    for unbound in [
        "project.export",
        "view.activate",
        "project.rename",
        "clip.remove",
        "clip.mute",
        "clips.delete-range",
        "transport.play",
        "transport.stop",
    ] {
        assert!(
            !km.bindings().iter().any(|b| b.command == unbound),
            "{unbound} must have no default binding"
        );
    }
    // tool.activate bindings carry the tool name as arg.
    for (k, tool) in [
        ('v', "smart"),
        ('b', "draw"),
        ('s', "split"),
        ('t', "trim"),
        ('e', "erase"),
    ] {
        let binding = km.lookup(OS, &key(Key::Char(k))).unwrap();
        assert_eq!(binding.arg.as_deref(), Some(tool));
    }
}

// ---------------------------------------------------------------------------
// TS-EX-03 — OS logical-modifier normalization (§3.3)
// ---------------------------------------------------------------------------

#[test]
fn ts_ex_03_mod_normalizes_per_os() {
    let km = Keymap::standard();
    // Same logical intent through each OS's physical primary modifier.
    for (os, stroke) in [
        (OsFamily::Windows, ctrl(Key::Char('z'))),
        (OsFamily::Linux, ctrl(Key::Char('z'))),
        (OsFamily::MacOs, cmd(Key::Char('z'))),
    ] {
        let binding = km
            .lookup(os, &stroke)
            .unwrap_or_else(|| panic!("Mod+Z unbound on {os:?}"));
        assert_eq!(binding.command, "edit.undo");
    }
    for (os, stroke, command) in [
        (OsFamily::Windows, ctrl(Key::Char('s')), "project.save"),
        (OsFamily::MacOs, cmd(Key::Char('s')), "project.save"),
        (OsFamily::Windows, ctrl(Key::Char('q')), "app.quit"),
        (OsFamily::MacOs, cmd(Key::Char('q')), "app.quit"),
        (OsFamily::Windows, ctrl(Key::Char('=')), "view.zoom-in"),
        (OsFamily::MacOs, cmd(Key::Char('=')), "view.zoom-in"),
        (OsFamily::MacOs, cmd_shift(Key::Char('z')), "edit.redo"),
        (OsFamily::Windows, ctrl_shift(Key::Char('z')), "edit.redo"),
    ] {
        let binding = km
            .lookup(os, &stroke)
            .unwrap_or_else(|| panic!("{command} unbound on {os:?}"));
        assert_eq!(binding.command, command);
    }
    // The non-primary modifier family does not match: physical Ctrl
    // on macOS and physical Cmd/Super on Windows/Linux are outside
    // the logical vocabulary.
    assert!(km.lookup(OsFamily::MacOs, &ctrl(Key::Char('z'))).is_none());
    assert!(km.lookup(OsFamily::Windows, &cmd(Key::Char('z'))).is_none());
    assert!(km.lookup(OsFamily::Linux, &cmd(Key::Char('z'))).is_none());
    // Unmodified keys resolve identically on every OS.
    for os in [OsFamily::Windows, OsFamily::Linux, OsFamily::MacOs] {
        assert_eq!(
            km.lookup(os, &key(Key::Space)).unwrap().command,
            "transport.toggle"
        );
        assert_eq!(
            km.lookup(os, &key(Key::Char('v'))).unwrap().command,
            "tool.activate"
        );
    }
}

fn cmd_shift(k: Key) -> KeyStroke {
    KeyStroke {
        key: k,
        modifiers: PhysicalModifiers::COMMAND | PhysicalModifiers::SHIFT,
        repeat: false,
    }
}

// ---------------------------------------------------------------------------
// TS-EX-04 — §4.1 lane × InputContext firing matrix
// ---------------------------------------------------------------------------

#[test]
fn ts_ex_04_input_context_firing_matrix() {
    let km = Keymap::standard();
    let (mut ws, _win, arr, _t, _l, _c) = focused_arrangement();

    // -- View: everything fires --------------------------------------
    assert!(matches!(
        press(&km, &mut ws, OS, key(Key::Space)),
        KeyOutcome::Dispatched(CommandOutcome::Applied { .. })
    ));
    assert!(ws.session().transport().playing);
    assert!(matches!(
        press(&km, &mut ws, OS, key(Key::Char('b'))),
        KeyOutcome::Dispatched(CommandOutcome::Applied { .. })
    ));
    assert_eq!(ws.state(arr).unwrap().active_tool, Tool::Draw);
    assert!(matches!(
        press(&km, &mut ws, OS, ctrl(Key::Char('z'))),
        KeyOutcome::Dispatched(CommandOutcome::Applied { .. })
    ));

    // -- TextInput: unmodified keys go to the text lane; Mod-combos
    //    still reach keymap but PTE lanes suppress -------------------
    ws.set_input_context(InputContext::TextInput);
    for stroke in [
        key(Key::Char('a')),
        key(Key::Space),
        key(Key::Delete),
        key(Key::Backspace),
        key(Key::ArrowRight),
        key(Key::Enter),
        key(Key::Char('v')),
    ] {
        assert!(
            matches!(press(&km, &mut ws, OS, stroke), KeyOutcome::TextLane),
            "{stroke:?} must go to the text lane"
        );
    }
    // Undo lane fires in TextInput → text-lane undo counter.
    let before = ws.text_lane_undos();
    assert!(matches!(
        press(&km, &mut ws, OS, ctrl(Key::Char('z'))),
        KeyOutcome::Dispatched(CommandOutcome::Applied { revision: None })
    ));
    assert_eq!(ws.text_lane_undos(), before + 1);
    // Application lane fires in TextInput (Mod+Q quits).
    assert!(matches!(
        press(&km, &mut ws, OS, ctrl(Key::Char('q'))),
        KeyOutcome::Dispatched(CommandOutcome::Applied { .. })
    ));
    assert!(ws.host.exited);
    // PTE + View lanes suppress in TextInput.
    assert_suppressed_all(
        &km,
        &mut ws,
        "TextInput",
        &[
            ctrl(Key::Char('s')), // project.save — Project
            ctrl(Key::Char('=')), // view.zoom-in — View
            ctrl(Key::Char('c')), // edit.copy — Editing
            ctrl(Key::Char('v')), // edit.paste — Editing
        ],
    );

    // -- Ime: same text-lane rule, but Application is suppressed -----
    ws.set_input_context(InputContext::Ime);
    assert!(matches!(
        press(&km, &mut ws, OS, key(Key::Char('x'))),
        KeyOutcome::TextLane
    ));
    assert!(matches!(
        press(&km, &mut ws, OS, ctrl(Key::Char('q'))),
        KeyOutcome::Suppressed { .. }
    ));
    // Undo still routes to the text lane under IME.
    let before = ws.text_lane_undos();
    assert!(matches!(
        press(&km, &mut ws, OS, ctrl(Key::Char('z'))),
        KeyOutcome::Dispatched(CommandOutcome::Applied { revision: None })
    ));
    assert_eq!(ws.text_lane_undos(), before + 1);

    // -- PluginGui: every lane suppresses -----------------------------
    ws.set_input_context(InputContext::PluginGui);
    assert_suppressed_all(
        &km,
        &mut ws,
        "PluginGui",
        &[
            key(Key::Space),
            key(Key::Char('v')),
            key(Key::Delete),
            ctrl(Key::Char('z')),
            ctrl(Key::Char('q')),
            ctrl(Key::Char('s')),
            ctrl(Key::Char('=')),
        ],
    );

    // -- Modal: Undo + View + PTE suppress; Application fires --------
    ws.set_input_context(InputContext::Modal);
    assert_suppressed_all(
        &km,
        &mut ws,
        "Modal",
        &[
            key(Key::Space),
            key(Key::Char('v')),
            key(Key::Delete),
            ctrl(Key::Char('z')),
            ctrl(Key::Char('s')),
            ctrl(Key::Char('=')),
        ],
    );
}

// ---------------------------------------------------------------------------
// TS-EX-05 — Undo lane routing per input context
// ---------------------------------------------------------------------------

#[test]
fn ts_ex_05_undo_redo_lane_routing() {
    let km = Keymap::standard();
    let (mut ws, _win, _arr, _t, _l, _c) = focused_arrangement();
    apply(
        &mut ws,
        ProjectOp::SetProjectName {
            name: "before-undo".to_string(),
        },
    );
    // View: project history undo/redo.
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, ctrl(Key::Char('z')))),
        CommandOutcome::Applied { revision: Some(_) }
    ));
    assert_eq!(ws.session().project().name, "keymap-test");
    let rev_after_undo = ws.session().revision();
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, ctrl_shift(Key::Char('z')))),
        CommandOutcome::Applied { revision: Some(_) }
    ));
    assert_eq!(ws.session().project().name, "before-undo");
    assert!(ws.session().revision().0 > rev_after_undo.0);

    // TextInput / Ime: text-lane undo, project untouched.
    for input in [InputContext::TextInput, InputContext::Ime] {
        ws.set_input_context(input);
        let rev = ws.session().revision();
        let before = ws.text_lane_undos();
        assert!(matches!(
            dispatched(press(&km, &mut ws, OS, ctrl(Key::Char('z')))),
            CommandOutcome::Applied { revision: None }
        ));
        assert_eq!(ws.text_lane_undos(), before + 1);
        assert_eq!(ws.session().revision(), rev);
        assert!(matches!(
            dispatched(press(&km, &mut ws, OS, ctrl_shift(Key::Char('z')))),
            CommandOutcome::Applied { revision: None }
        ));
        assert_eq!(ws.session().revision(), rev);
    }

    // Modal / PluginGui: suppressed at the keymap gate.
    for input in [InputContext::Modal, InputContext::PluginGui] {
        ws.set_input_context(input);
        let before = ws.text_lane_undos();
        assert!(matches!(
            press(&km, &mut ws, OS, ctrl(Key::Char('z'))),
            KeyOutcome::Suppressed { .. }
        ));
        assert!(matches!(
            press(&km, &mut ws, OS, ctrl_shift(Key::Char('z'))),
            KeyOutcome::Suppressed { .. }
        ));
        assert_eq!(ws.text_lane_undos(), before);
    }
    ws.set_input_context(InputContext::View);
}

// ---------------------------------------------------------------------------
// TS-EX-06 — K-2 gesture preview suppression + Esc transient cancel
// ---------------------------------------------------------------------------

#[test]
fn ts_ex_06_gesture_preview_suppresses_edit_and_esc_cancels() {
    let km = Keymap::standard();
    let (mut ws, _win, arr, _t, _l, clip) = focused_arrangement();

    // No transient → Esc is a no-op.
    assert!(matches!(
        press(&km, &mut ws, OS, key(Key::Escape)),
        KeyOutcome::NoTransient
    ));

    // Start a gesture preview on the clip.
    let outcome = ws.pointer_down(arr, &body_probe(10, SemanticTarget::Clip(clip)));
    assert!(matches!(
        outcome,
        Ok(synvelis_workspace::PointerOutcome::GestureStarted)
    ));
    assert!(ws.state(arr).unwrap().gesture.is_some());

    // Editing bindings are suppressed while the preview is active.
    for stroke in [key(Key::Delete), ctrl(Key::Char('c')), ctrl(Key::Char('v'))] {
        assert!(
            matches!(
                press(&km, &mut ws, OS, stroke),
                KeyOutcome::Suppressed { reason } if reason == "gesture preview active"
            ),
            "{stroke:?} must be suppressed during gesture preview"
        );
    }
    // The clip is untouched.
    assert!(ws.session().project().clips.contains_key(&clip));

    // Esc cancels the innermost transient (gesture preview).
    assert!(matches!(
        press(&km, &mut ws, OS, key(Key::Escape)),
        KeyOutcome::TransientCancelled
    ));
    assert!(ws.state(arr).unwrap().gesture.is_none());
    assert!(ws.session().project().clips.contains_key(&clip));

    // Esc again → no transient.
    assert!(matches!(
        press(&km, &mut ws, OS, key(Key::Escape)),
        KeyOutcome::NoTransient
    ));

    // With the preview gone, Delete dispatches — Clips selection →
    // clip.remove, exactly one undo unit.
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    let rev = ws.session().revision();
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, key(Key::Delete))),
        CommandOutcome::Applied {
            revision: Some(r)
        } if r.0 == rev.0 + 1
    ));
    assert!(!ws.session().project().clips.contains_key(&clip));
    // One undo restores — 1 key dispatch = 1 undo unit.
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, ctrl(Key::Char('z')))),
        CommandOutcome::Applied { revision: Some(_) }
    ));
    assert!(ws.session().project().clips.contains_key(&clip));
}

// ---------------------------------------------------------------------------
// TS-EX-07 — K-4 tool.activate scoped to origin_view
// ---------------------------------------------------------------------------

#[test]
fn ts_ex_07_tool_activate_scoped_to_origin_view() {
    let km = Keymap::standard();
    let (mut ws, win_a, arr, _t, _l, _c) = focused_arrangement();
    let win_b = ws.create_window("B", 1280, 800).unwrap();

    for (k, tool) in [
        ('v', Tool::Smart),
        ('b', Tool::Draw),
        ('s', Tool::Split),
        ('t', Tool::Trim),
        ('e', Tool::Erase),
    ] {
        assert!(matches!(
            dispatched(press(&km, &mut ws, OS, key(Key::Char(k)))),
            CommandOutcome::Applied { revision: None }
        ));
        assert_eq!(ws.state(arr).unwrap().active_tool, tool, "key {k}");
    }

    // Dock move does not reset the view's tool.
    ws.reattach(arr, win_b).unwrap();
    assert_eq!(ws.state(arr).unwrap().active_tool, Tool::Erase);
    assert_eq!(ws.dock().owner_of(arr), Some(win_b));
    let _ = win_a;

    // No origin view → honest Unresolved, no fallback to another view.
    let mut ws2 = fresh_ws();
    let empty_win = ws2.create_window("empty", 1280, 800).unwrap();
    ws2.on_window_event(empty_win, &HostEvent::Focused(true))
        .unwrap();
    assert!(ws2.focus().active_view.is_none());
    assert!(matches!(
        dispatched(press(&km, &mut ws2, OS, key(Key::Char('v')))),
        CommandOutcome::Unresolved { .. }
    ));
}

// ---------------------------------------------------------------------------
// TS-EX-08 — §5.3 auto-repeat policy
// ---------------------------------------------------------------------------

#[test]
fn ts_ex_08_auto_repeat_policy() {
    let km = Keymap::standard();
    let (mut ws, _win, arr, _t, _l, _c) = focused_arrangement();

    // Scroll + undo/redo accept repeat — and scrolling really moves
    // the view-local viewport.
    assert!(matches!(
        press(&km, &mut ws, OS, repeated(key(Key::ArrowRight))),
        KeyOutcome::Dispatched(_)
    ));
    assert!(ws.state(arr).unwrap().viewport.scroll_x > 0.0);
    assert!(matches!(
        press(&km, &mut ws, OS, repeated(key(Key::ArrowDown))),
        KeyOutcome::Dispatched(_)
    ));
    assert!(ws.state(arr).unwrap().viewport.scroll_y > 0.0);
    for stroke in [
        repeated(key(Key::ArrowLeft)),
        repeated(key(Key::ArrowUp)),
        repeated(ctrl(Key::Char('z'))),
        repeated(ctrl_shift(Key::Char('z'))),
    ] {
        assert!(
            matches!(press(&km, &mut ws, OS, stroke), KeyOutcome::Dispatched(_)),
            "{stroke:?} must accept auto-repeat"
        );
    }

    // Everything else is edge-triggered.
    for stroke in [
        repeated(key(Key::Space)),      // transport.toggle
        repeated(key(Key::Enter)),      // transport.stop-return
        repeated(key(Key::Char('r'))),  // transport.record
        repeated(key(Key::Delete)),     // edit.delete
        repeated(key(Key::Backspace)),  // edit.delete alias
        repeated(ctrl(Key::Char('c'))), // edit.copy
        repeated(ctrl(Key::Char('v'))), // edit.paste
        repeated(ctrl(Key::Char('s'))), // project.save
        repeated(ctrl(Key::Char('q'))), // app.quit
        repeated(key(Key::Char('v'))),  // tool.activate
        repeated(ctrl(Key::Char('='))), // view.zoom-in
    ] {
        assert!(
            matches!(press(&km, &mut ws, OS, stroke), KeyOutcome::RepeatIgnored),
            "{stroke:?} must reject auto-repeat"
        );
    }
    // The transport never toggled from the repeated Space.
    assert!(!ws.session().transport().playing);
}

// ---------------------------------------------------------------------------
// TS-EX-09 — KG-C-5 remap conflict rejection
// ---------------------------------------------------------------------------

/// Logical chord builder for remap tests.
fn chord(key: Key, primary: bool, shift: bool, alt: bool) -> KeyChord {
    KeyChord {
        key,
        modifiers: LogicalModifiers {
            primary,
            shift,
            alt,
        },
    }
}

#[test]
fn ts_ex_09_remap_conflict_rejection() {
    let mut km = Keymap::standard();
    // Lane-internal uniqueness: Mod+Z is already bound — re-binding
    // it to any command (even the same one) conflicts, never silently
    // overwrites.
    let mod_z = chord(Key::Char('z'), true, false, false);
    for command in ["edit.redo", "edit.undo"] {
        assert!(
            matches!(
                km.try_bind(OS, mod_z, command, None),
                Err(BindRejection::Conflict { .. })
            ),
            "re-binding Mod+Z to {command} must conflict"
        );
    }
    // OS reservations on Windows/Linux (§3.3).
    for c in [
        chord(Key::Function(4), false, false, true), // Alt+F4
        chord(Key::Tab, false, false, true),         // Alt+Tab
        chord(Key::Delete, true, false, true),       // Ctrl+Alt+Del
        chord(Key::Escape, true, false, false),      // Ctrl+Esc
    ] {
        assert!(
            matches!(
                km.try_bind(OsFamily::Windows, c, "app.quit", None),
                Err(BindRejection::OsReserved)
            ),
            "{c:?} must be OS-reserved on Windows"
        );
    }
    // OS reservations on macOS (Cmd family).
    for c in [
        chord(Key::Space, true, false, false),
        chord(Key::Tab, true, false, false),
        chord(Key::Char('h'), true, false, false),
        chord(Key::Char('m'), true, false, false),
    ] {
        assert!(
            matches!(
                km.try_bind(OsFamily::MacOs, c, "app.quit", None),
                Err(BindRejection::OsReserved)
            ),
            "{c:?} must be OS-reserved on macOS"
        );
    }
    // Unregistered command ids are refused.
    let free = chord(Key::Function(5), false, false, false);
    assert!(matches!(
        km.try_bind(OS, free, "no.such.command", None),
        Err(BindRejection::UnknownCommand(_))
    ));
    // A clean assignment succeeds and resolves.
    km.try_bind(OS, free, "project.save", None).unwrap();
    let stroke = KeyStroke {
        key: Key::Function(5),
        modifiers: PhysicalModifiers::default(),
        repeat: false,
    };
    assert_eq!(km.lookup(OS, &stroke).unwrap().command, "project.save");
}

// ---------------------------------------------------------------------------
// TS-EX-10 — §6.2 menu coverage inventory (headless contract surface)
// ---------------------------------------------------------------------------

/// The §3.1 contract catalog — every id `spec_for` must know.
const CATALOG: &[&str] = &[
    "app.quit",
    "project.save",
    "project.export",
    "project.rename",
    "edit.undo",
    "edit.redo",
    "edit.delete",
    "edit.copy",
    "edit.paste",
    "clip.remove",
    "clip.mute",
    "clips.delete-range",
    "transport.toggle",
    "transport.stop-return",
    "transport.record",
    "transport.play",
    "transport.stop",
    "tool.activate",
    "view.zoom-in",
    "view.zoom-out",
    "view.scroll-left",
    "view.scroll-right",
    "view.scroll-up",
    "view.scroll-down",
    "view.activate",
];

#[test]
fn ts_ex_10_catalog_has_menu_coverage_and_accelerators() {
    let km = Keymap::standard();
    assert_eq!(CATALOG.len(), 25);
    for id in CATALOG {
        assert!(spec_for(id).is_some(), "{id} not registered");
        let coverage = menu_coverage(id).unwrap_or_else(|| panic!("{id} missing §6.2 coverage"));
        // Every catalog id has an explicit surface list — possibly
        // empty with a reason (transport.play/stop).
        assert!(
            !coverage.surfaces.is_empty() || coverage.note.is_some(),
            "{id} coverage must name surfaces or a reason"
        );
        // Accelerator notation exists exactly for default-bound ids.
        let bound = km.bindings().iter().any(|b| b.command == *id);
        assert_eq!(
            km.accelerator_label(OS, id).is_some(),
            bound,
            "{id} accelerator/binding mismatch"
        );
    }
    // CM-2: labels follow OS convention and match the §3.2 binds.
    assert_eq!(
        km.accelerator_label(OS, "edit.undo").as_deref(),
        Some("Ctrl+Z")
    );
    assert_eq!(
        km.accelerator_label(OS, "edit.redo").as_deref(),
        Some("Ctrl+Shift+Z")
    );
    assert_eq!(
        km.accelerator_label(OS, "project.save").as_deref(),
        Some("Ctrl+S")
    );
    assert_eq!(
        km.accelerator_label(OS, "app.quit").as_deref(),
        Some("Ctrl+Q")
    );
    assert_eq!(
        km.accelerator_label(OS, "transport.toggle").as_deref(),
        Some("Space")
    );
    assert_eq!(
        km.accelerator_label(OsFamily::MacOs, "edit.undo")
            .as_deref(),
        Some("⌘Z")
    );
    assert_eq!(
        km.accelerator_label(OsFamily::MacOs, "edit.redo")
            .as_deref(),
        Some("⌘⇧Z")
    );
    assert_eq!(
        km.accelerator_label(OsFamily::MacOs, "project.save")
            .as_deref(),
        Some("⌘S")
    );
    // No default bind → no accelerator (KG-KEY-14 export stays
    // menu-only).
    assert!(km.accelerator_label(OS, "project.export").is_none());
    // §6.2 spot checks: clip commands are arrangement-context only;
    // scroll items are omissible menubar entries.
    let clip_cov = menu_coverage("clip.remove").unwrap();
    assert_eq!(clip_cov.surfaces, [MenuSurface::ArrangementMenu].as_slice());
    let scroll_cov = menu_coverage("view.scroll-left").unwrap();
    assert_eq!(scroll_cov.surfaces, [MenuSurface::MenuBar].as_slice());
    let play_cov = menu_coverage("transport.play").unwrap();
    assert!(play_cov.note.is_some());
}

// ---------------------------------------------------------------------------
// TS-EX-11 — honest outcome propagation (§4.5)
// ---------------------------------------------------------------------------

#[test]
fn ts_ex_11_outcomes_stay_distinguishable() {
    let km = Keymap::standard();
    let (mut ws, win_a, arr, track, _lane, clip) = focused_arrangement();

    // Unbound chord → Unbound (not a silent no-op success).
    assert!(matches!(
        press(&km, &mut ws, OS, key(Key::Char('x'))),
        KeyOutcome::Unbound
    ));
    // Lane suppression → Suppressed at the keymap gate.
    ws.set_input_context(InputContext::TextInput);
    assert!(matches!(
        press(&km, &mut ws, OS, ctrl(Key::Char('s'))),
        KeyOutcome::Suppressed { .. }
    ));
    ws.set_input_context(InputContext::View);

    // Unresolved: honest "cannot run" outcomes, never fake Applied.
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, ctrl(Key::Char('c')))),
        CommandOutcome::Unresolved { .. }
    )); // edit.copy — no clipboard lane
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, ctrl(Key::Char('v')))),
        CommandOutcome::Unresolved { .. }
    )); // edit.paste — no clipboard lane
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, ctrl(Key::Char('s')))),
        CommandOutcome::Unresolved { .. }
    )); // project.save — no workspace save op
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, key(Key::Char('r')))),
        CommandOutcome::Unresolved { .. }
    )); // transport.record — #12 state machine pending
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, key(Key::Enter))),
        CommandOutcome::Unresolved { .. }
    )); // transport.stop-return — no position API

    // edit.delete on a non-Clips selection domain → Unresolved.
    ws.state_mut(arr)
        .unwrap()
        .object_selections
        .set(
            synvelis_workspace::SelectionDomain::Tracks,
            EditingSelection::single(SemanticTarget::Track(track)),
        )
        .unwrap();
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, key(Key::Delete))),
        CommandOutcome::Unresolved { .. }
    ));

    // Rejected propagates verbatim: edit.delete on a mid-transfer
    // view is ViewBusy.
    ws.state_mut(arr)
        .unwrap()
        .object_selections
        .set(
            synvelis_workspace::SelectionDomain::Clips,
            EditingSelection::single(SemanticTarget::Clip(clip)),
        )
        .unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let token = ws.begin_transfer(arr, DockTarget::Window(win_b)).unwrap();
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, key(Key::Delete))),
        CommandOutcome::Rejected {
            error: WorkspaceError::ViewBusy(v)
        } if v == arr
    ));
    ws.abort_transfer(token).unwrap();
    let _ = win_a;

    // Applied is honest too: transport.toggle flips the stub.
    assert!(matches!(
        dispatched(press(&km, &mut ws, OS, key(Key::Space))),
        CommandOutcome::Applied { revision: None }
    ));
    assert!(ws.session().transport().playing);
}
