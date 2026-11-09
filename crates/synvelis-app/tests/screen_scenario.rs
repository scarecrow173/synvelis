//! SV-GUI-03 slice 1 (synvelis#112) — `SemanticGuiTest`/`FakeGuiTest`
//! scenario coverage for the product main screen spec.
//!
//! Runs the shared ruviform #464 contract against
//! [`main_screen_spec`]: the seek gesture through
//! [`parity_gesture_scenario`], a `transport.play` invoke, receipt and
//! projection posts through `post_event`, and read-only observations.
//! Fake evidence only — `ScenarioDriver::Fake` claims
//! `SemanticAction`/`MultiWindow`, never OS/pointer/pixel proof; the
//! app-side `ControlsUi` dispatch is proven end-to-end in
//! `view_mount.rs`.

use ruviform_ui::behavior::{CommandIntent, CommandPhase, Revision, ValueInput};
use ruviform_ui::ipc::Event;
use ruviform_ui::view::ComponentKey;
use ruviform_ui_test::FakeGuiTest;
use ruviform_ui_test::scenario::{
    GuiAction, SemanticGuiTest, TargetQuery, parity_gesture_scenario,
};
use serde_json::json;

use synvelis_app::{
    InspectorRow, InspectorScreenState, MainScreenState, main_screen_spec, main_view_instance,
};

fn key(name: &str) -> ComponentKey {
    ComponentKey(name.to_string())
}

/// The shared #464 gesture-parity scenario passes against the product
/// spec: `Begin → Preview → Commit` on `seek.position` mints the
/// `transport/position` command intents, the `Applied` receipt and the
/// authoritative position projection land through `post_event` (the
/// same `deliver_event` decode every driver uses), and the controlled
/// display settles on the seconds value — the synvelis#175 seconds
/// contract.
#[test]
fn seek_gesture_and_projection_follow_the_shared_parity_scenario() {
    let mut gui = FakeGuiTest::new();
    let window = gui.open_window();
    let view = main_view_instance();
    let state = MainScreenState {
        project_length_seconds: 120.0,
        ..MainScreenState::default()
    };
    gui.mount_view(window, view, main_screen_spec(&state))
        .expect("main spec mounts on the fake adapter");
    let target = gui
        .resolve_target(&TargetQuery::new(window, view))
        .expect("view resolves");

    let seek = key("seek.position");
    let (intents, _) = parity_gesture_scenario(
        &mut gui,
        &target,
        &seek,
        "sv112-seek",
        ValueInput::Plain(8.0),
    )
    .expect("seek scenario runs");

    // Begin/Preview/Commit minted on the transport/position surface —
    // the same intents the shell routes to `syn.control.dispatch`.
    assert_eq!(intents.len(), 3, "begin+preview+commit: {intents:?}");
    for minted in &intents {
        let intent_target = match &minted.intent {
            CommandIntent::Begin { target, .. }
            | CommandIntent::Preview { target, .. }
            | CommandIntent::Commit { target, .. } => target,
            other => panic!("gesture intent expected, got {other:?}"),
        };
        assert_eq!(intent_target, "transport/position");
    }

    // The commit's authoritative receipt + position projection post
    // back through the consumer→view direction.
    let commit_id = intents
        .iter()
        .rev()
        .find_map(|m| match &m.intent {
            CommandIntent::Commit { .. } => Some(m.command_id.clone()),
            _ => None,
        })
        .expect("commit intent minted");
    gui.post_event(
        &target,
        &Event::Result {
            id: commit_id,
            ok: true,
            data: Some(json!({ "status": "applied", "revision": 1 })),
            error: None,
        },
    )
    .expect("receipt posts");
    gui.post_event(
        &target,
        &Event::Notify {
            seq: 1,
            data: json!({
                "control_id": "transport/position",
                "revision": 1,
                "value": 8.0,
            }),
        },
    )
    .expect("projection posts");
    gui.settle().expect("settle");

    let snap = gui.observe(&target, &seek).expect("seek observes");
    assert_eq!(snap.phase, CommandPhase::Observed);
    let observed = snap.observed.expect("projection observed");
    assert_eq!(observed.revision, Revision::Num(1));
    assert_eq!(observed.value, json!(8.0));
    let display = snap.display.expect("controlled display");
    assert!(
        (display.plain - 8.0).abs() < 1e-9,
        "display settles on the seconds position: {display:?}"
    );
}

/// The Transport Play `Button` mints one discrete `Action` intent on
/// the `transport` target — the intent `ControlsUi::route_action`
/// resolves to `transport_play` on the app side (the end-to-end
/// dispatch is proven in `view_mount.rs`). The Applied receipt lands
/// on the button's command binding, and the Inspector `Table` stays
/// read-only — row selection is not a legal op on it.
#[test]
fn transport_play_invokes_and_inspector_table_is_read_only() {
    let mut gui = FakeGuiTest::new();
    let window = gui.open_window();
    let view = main_view_instance();
    let state = MainScreenState {
        project_length_seconds: 120.0,
        inspector: InspectorScreenState {
            target: Some("param-abc".to_string()),
            rows: vec![InspectorRow {
                field: "mode".to_string(),
                value: "Manual".to_string(),
            }],
        },
        ..MainScreenState::default()
    };
    gui.mount_view(window, view, main_screen_spec(&state))
        .expect("main spec mounts");
    let target = gui
        .resolve_target(&TargetQuery::new(window, view))
        .expect("view resolves");

    // Play press → one Action intent on `transport`.
    gui.perform(
        &target,
        GuiAction::Invoke {
            key: key("transport.play"),
        },
    )
    .expect("play invoke");
    let intents = gui.drain_intents();
    assert_eq!(intents.len(), 1, "one action intent: {intents:?}");
    match &intents[0].intent {
        CommandIntent::Action {
            target: intent_target,
            action,
            ..
        } => {
            assert_eq!(intent_target, "transport");
            assert_eq!(action, "play");
        }
        other => panic!("action intent expected: {other:?}"),
    }

    // The Applied receipt lands on the button's command binding.
    gui.post_event(
        &target,
        &Event::Result {
            id: intents[0].command_id.clone(),
            ok: true,
            data: Some(json!({ "status": "applied" })),
            error: None,
        },
    )
    .expect("receipt posts");
    gui.settle().expect("settle");
    let play = gui
        .observe(&target, &key("transport.play"))
        .expect("button observes");
    assert!(play.mounted);
    assert_eq!(play.phase, CommandPhase::Applied);

    // The Inspector table mounts read-only: no selection, and a row
    // select is rejected rather than silently accepted.
    let table_key = key("inspector.table");
    let table = gui.observe(&target, &table_key).expect("table observes");
    assert!(table.mounted);
    assert_eq!(table.selected, None);
    assert!(
        gui.select_row(&target, &table_key, 0).is_err(),
        "read-only table rejects row selection"
    );
}
