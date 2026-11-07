//! `ViewBinding` wire-shape tests (SV-GPUI-01).
//!
//! Headless verification of the typed view-side adapter: intent
//! production (op/params/`DispatchContext`), result correlation,
//! generation adoption from the attach reply, notify channel demux,
//! and `view_id` filtering. No driver involved — the outbox/apply
//! seam is the whole contract.

use serde_json::{Value, json};

use ruviform_ui::ipc::{Event, Intent};

use synvelis_app::{BindingNotice, ViewBinding};
use synvelis_workspace::controls_ui::ControlValue;

fn binding() -> ViewBinding {
    ViewBinding::new("console", "w1")
}

/// Applies the `result` answering `binding`'s last minted intent.
fn answer_last(binding: &mut ViewBinding, data: Value) -> Vec<BindingNotice> {
    let id = binding
        .take_outbox()
        .last()
        .expect("an intent was minted")
        .id
        .clone();
    binding.apply(&Event::result_ok(id, data))
}

fn attach(binding: &mut ViewBinding, generation: u64) {
    binding.attach();
    let notices = answer_last(
        binding,
        json!({ "view_id": "console", "generation": generation, "attached": true }),
    );
    assert!(
        notices
            .iter()
            .any(|n| matches!(n, BindingNotice::Attached { .. })),
        "attach result yields an Attached notice: {notices:?}"
    );
}

#[test]
fn attach_intent_wire_shape_and_generation_adoption() {
    let mut b = binding();
    b.attach();
    let intents = b.take_outbox();
    assert_eq!(intents.len(), 1);
    let Intent { id, op, params, .. } = &intents[0];
    assert_eq!(op, "syn.view.attach");
    assert_eq!(params["view_id"], "console");
    assert_eq!(params["window_id"], "w1");
    assert_eq!(b.generation(), 0, "no epoch before the reply");

    let notices = b.apply(&Event::result_ok(
        id.clone(),
        json!({ "view_id": "console", "generation": 7u64, "viz_generation": 9u64,
                "attached": true }),
    ));
    let mut saw_result = false;
    let mut saw_attached = false;
    for n in &notices {
        match n {
            BindingNotice::CallResult { op, ok, .. } => {
                saw_result = true;
                assert_eq!(op.as_deref(), Some("syn.view.attach"));
                assert!(ok);
            }
            BindingNotice::Attached { generation } => {
                saw_attached = true;
                assert_eq!(*generation, 7);
            }
            _ => {}
        }
    }
    assert!(saw_result && saw_attached);
    assert_eq!(b.generation(), 7, "attach epoch adopted");
    assert!(b.attached());
}

#[test]
fn control_dispatch_carries_typed_intent_and_ctx() {
    let mut b = binding();
    attach(&mut b, 3);
    let command_id = b.control_action("transport", "transport-1", "play");
    assert!(command_id.starts_with("console-c"), "{command_id}");
    let intents = b.take_outbox();
    let dispatch = intents.last().unwrap();
    assert_eq!(dispatch.op, "syn.control.dispatch");
    assert_eq!(dispatch.params["intent"]["kind"], "action");
    assert_eq!(dispatch.params["intent"]["target"], "transport");
    assert_eq!(dispatch.params["intent"]["controlId"], "transport-1");
    assert_eq!(dispatch.params["intent"]["action"], "play");
    assert_eq!(dispatch.params["intent"]["originView"], "console");
    let ctx = &dispatch.params["ctx"];
    assert_eq!(ctx["commandId"], command_id);
    assert_eq!(ctx["viewId"], "console");
    assert_eq!(ctx["windowId"], "w1");
    assert_eq!(ctx["generation"], 3, "attach epoch applies to ctx");
}

#[test]
fn gesture_lifecycle_mints_shared_gesture_id() {
    let mut b = binding();
    attach(&mut b, 1);
    let value = ControlValue {
        plain: 4.0,
        normalized: 0.5,
    };
    let gesture = b.gesture_begin("transport/position", "seekbar", value);
    assert_eq!(gesture, "console-g1");
    b.gesture_preview(&gesture, "transport/position", "seekbar", value);
    let commit_cmd = b.gesture_commit(&gesture, "transport/position", "seekbar", value);
    assert!(commit_cmd.starts_with("console-c"));

    let kinds: Vec<String> = b
        .take_outbox()
        .iter()
        .map(|i| i.params["intent"]["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds, ["begin", "preview", "commit"]);
    let intents = b.gesture_begin("transport/position", "seekbar", value);
    assert_eq!(intents, "console-g2");
    let cancel_cmd = b.gesture_cancel(
        &gesture,
        "transport/position",
        "seekbar",
        synvelis_workspace::controls_ui::GestureCancelReason::Explicit,
    );
    assert!(cancel_cmd.starts_with("console-c"));
    let last = b.take_outbox();
    let cancel = last.last().unwrap();
    assert_eq!(cancel.params["intent"]["kind"], "cancel");
    assert_eq!(cancel.params["intent"]["gestureId"], "console-g1");
    assert_eq!(cancel.params["intent"]["reason"], "explicit");
}

#[test]
fn receipt_and_projection_demux_with_view_filter() {
    let mut b = binding();
    let receipt = json!({
        "commandId": "console-c1", "status": "applied", "revision": 5u64,
    });
    let notices = b.apply(&Event::Notify {
        seq: 1,
        data: json!({
            "channel": "syn.control.receipt",
            "data": { "view_id": "console", "target": "transport",
                      "receipt": receipt },
        }),
    });
    assert_eq!(notices.len(), 1);
    match &notices[0] {
        BindingNotice::Receipt { target, receipt } => {
            assert_eq!(target, "transport");
            assert_eq!(receipt.command_id, "console-c1");
            assert_eq!(receipt.revision, Some(5));
        }
        other => panic!("expected Receipt, got {other:?}"),
    }

    // A receipt for another view is ignored, never delivered.
    let notices = b.apply(&Event::Notify {
        seq: 2,
        data: json!({
            "channel": "syn.control.receipt",
            "data": { "view_id": "other", "target": "transport",
                      "receipt": { "commandId": "x", "status": "applied" } },
        }),
    });
    assert!(matches!(notices[0], BindingNotice::Ignored));

    // A projection row parses into the typed mirror.
    let notices = b.apply(&Event::Notify {
        seq: 3,
        data: json!({
            "channel": "syn.control.projection",
            "data": {
                "view_id": "console", "target": "transport",
                "row": {
                    "kind": "transport", "phase": "applied",
                    "generation": 1u64, "attached": true, "subscribed": true,
                    "timed_out": false, "observed_seq": 2u64,
                    "observed": { "revision": 5u64,
                                  "value": { "status": "Playing" } },
                },
            },
        }),
    });
    match &notices[0] {
        BindingNotice::Projection { target, row } => {
            assert_eq!(target, "transport");
            assert_eq!(row.kind, "transport");
            assert_eq!(row.phase, "applied");
            assert!(row.attached && row.subscribed);
            assert_eq!(row.observed.as_ref().unwrap()["value"]["status"], "Playing");
        }
        other => panic!("expected Projection, got {other:?}"),
    }
}

#[test]
fn app_state_view_state_lifecycle_and_error_demux() {
    let mut b = binding();

    // Channel-less push → app-state broadcast.
    let notices = b.apply(&Event::Notify {
        seq: 1,
        data: json!({ "app": "synvelis", "project": { "name": "demo" } }),
    });
    assert!(matches!(notices[0], BindingNotice::AppState { .. }));

    // syn.view.state for this view.
    let notices = b.apply(&Event::Notify {
        seq: 2,
        data: json!({
            "channel": "syn.view.state",
            "data": { "view_id": "console", "attached": true },
        }),
    });
    match &notices[0] {
        BindingNotice::ViewState { attached, closed } => {
            assert!(*attached && !closed);
        }
        other => panic!("expected ViewState, got {other:?}"),
    }

    // lifecycle + uncorrelated error pass through typed.
    let notices = b.apply(&Event::lifecycle(ruviform_ui::ipc::LifecycleState::Closing));
    assert!(matches!(notices[0], BindingNotice::Lifecycle { .. }));
    let notices = b.apply(&Event::error(ruviform_ui::ipc::ErrorBody::new(
        "malformed_message",
        "bad",
    )));
    assert!(matches!(notices[0], BindingNotice::Error { .. }));
}

#[test]
fn detach_and_close_transitions() {
    let mut b = binding();
    attach(&mut b, 1);
    b.detach("unmount");
    answer_last(&mut b, json!({ "view_id": "console", "attached": false }));
    assert!(!b.attached(), "detach clears attached");

    b.close();
    let notices = answer_last(&mut b, json!({ "view_id": "console", "closed": true }));
    assert!(notices.iter().any(|n| matches!(n, BindingNotice::Closed)));
    assert!(b.closed() && !b.attached());
}

// ── RF387 `rvf.ui.command.v1` wire translation ────────────────────────
//
// The mounted-view wire (`{command_id, expect_receipt, intent:
// <snake_case>}`) is translated type-to-type into the camelCase
// `syn.control.dispatch` shape, and `syn.control.receipt`/
// `syn.control.projection` notifies back into the `Result`/`Notify`
// payloads `ViewHost` consumes. Strict — no silent defaults.

use synvelis_app::binding::{
    COMMAND_INTENT_OP, decode_view_command, notify_to_view_event, view_command_dispatch,
};

fn rvf_intent(kind: &str) -> Value {
    json!({
        "kind": kind,
        "target": "transport/position",
        "control_id": "seekbar",
        "sequence": 3,
        "gesture_id": "g1",
        "origin_view": "console",
        "value": { "plain": 4.0, "normalized": 0.4 },
    })
}

fn rvf_params(intent: &Value) -> Value {
    json!({
        "command_id": "cmd-1",
        "expect_receipt": true,
        "intent": intent,
    })
}

#[test]
fn decode_view_command_snake_to_camel() {
    let cmd = decode_view_command(&rvf_params(&rvf_intent("commit"))).expect("decodes");
    assert_eq!(cmd.command_id, "cmd-1");
    assert!(cmd.expect_receipt);
    assert_eq!(cmd.origin_view.as_deref(), Some("console"));
    // The SV-typed intent re-serializes camelCase — `atomic` absent in
    // the peer wire means a definite `false`, never `None`.
    let wire = serde_json::to_value(&cmd.intent).unwrap();
    assert_eq!(wire["kind"], "commit");
    assert_eq!(wire["controlId"], "seekbar");
    assert_eq!(wire["gestureId"], "g1");
    assert_eq!(wire["originView"], "console");
    assert_eq!(wire["atomic"], false);
    assert!(wire.get("control_id").is_none());
    assert_eq!(wire["value"]["plain"], 4.0);
}

#[test]
fn decode_view_command_atomic_and_action_distinctions() {
    // `atomic: true` stays `Some(true)` — 未指定/false/true are distinct.
    let mut begin = rvf_intent("begin");
    begin["atomic"] = json!(true);
    let cmd = decode_view_command(&rvf_params(&begin)).unwrap();
    let wire = serde_json::to_value(&cmd.intent).unwrap();
    assert_eq!(wire["atomic"], true);

    // `Action` has no `atomic` on the peer — `None`, so it serializes
    // absent rather than fabricated `false`.
    let action = decode_view_command(&rvf_params(&json!({
        "kind": "action",
        "target": "transport",
        "control_id": "transport",
        "sequence": 1,
        "action": "play",
    })))
    .unwrap();
    let wire = serde_json::to_value(&action.intent).unwrap();
    assert_eq!(wire["kind"], "action");
    assert!(wire.get("atomic").is_none(), "action atomic must be absent");

    // Preview keeps its coalescible flag.
    let preview = decode_view_command(&rvf_params(&rvf_intent("preview"))).unwrap();
    assert_eq!(
        serde_json::to_value(&preview.intent).unwrap()["coalescible"],
        true
    );
}

#[test]
fn decode_view_command_rejects_malformed() {
    // Unknown kind.
    let mut i = rvf_intent("bogus");
    i["extra"] = json!(1);
    assert!(decode_view_command(&rvf_params(&i)).is_err());
    // Unknown field on a known kind — deny, not default.
    let mut i = rvf_intent("commit");
    i["controlId"] = json!("dup");
    assert!(decode_view_command(&rvf_params(&i)).is_err());
    // Missing required field.
    let mut i = rvf_intent("begin");
    i.as_object_mut().unwrap().remove("gesture_id");
    assert!(decode_view_command(&rvf_params(&i)).is_err());
    // Missing envelope keys / wrong types.
    assert!(decode_view_command(&json!({ "command_id": "c" })).is_err());
    assert!(decode_view_command(&rvf_params(&json!({"kind": 3}))).is_err());
}

#[test]
fn view_command_dispatch_mints_dispatch_intent() {
    let cmd = decode_view_command(&rvf_params(&rvf_intent("commit"))).unwrap();
    let intent = view_command_dispatch(&cmd, "console", "w-9", 7, 42);
    assert_eq!(intent.id, "cmd-1", "command_id reused for correlation");
    assert_eq!(intent.op, "syn.control.dispatch");
    assert_eq!(intent.params["ctx"]["commandId"], "cmd-1");
    assert_eq!(intent.params["ctx"]["viewId"], "console");
    assert_eq!(intent.params["ctx"]["windowId"], "w-9");
    assert_eq!(intent.params["ctx"]["generation"], 7);
    assert_eq!(intent.params["ctx"]["issuedAt"], 42);
    // The dispatched intent survives the backend's typed decode.
    let decoded: synvelis_workspace::controls_ui::CommandIntent =
        serde_json::from_value(intent.params["intent"].clone()).expect("typed decode");
    assert!(matches!(
        decoded,
        synvelis_workspace::controls_ui::CommandIntent::Commit { .. }
    ));
}

#[test]
fn notify_to_view_event_receipt_statuses() {
    let mk = |status: &str| {
        json!({
            "channel": "syn.control.receipt",
            "data": {
                "view_id": "console",
                "target": "transport/position",
                "receipt": {
                    "commandId": "cmd-1", "status": status,
                    "revision": 11, "detail": format!("d-{status}"),
                },
            },
        })
    };
    for status in ["accepted", "queued", "applied"] {
        let ev = notify_to_view_event(&Event::Notify {
            seq: 5,
            data: mk(status),
        })
        .expect("receipt translates");
        assert_eq!(ev.0, "console");
        let Event::Result {
            id,
            ok,
            data,
            error,
        } = ev.1
        else {
            panic!("receipt must become a Result")
        };
        assert_eq!(id, "cmd-1");
        assert!(ok, "{status} is not a terminal failure");
        assert_eq!(data.unwrap()["status"], status);
        assert!(error.is_none());
    }
    for status in ["rejected", "unsupported"] {
        let (view, Event::Result { ok, error, .. }) = notify_to_view_event(&Event::Notify {
            seq: 5,
            data: mk(status),
        })
        .unwrap() else {
            panic!("terminal failure must become a failed Result")
        };
        assert_eq!(view, "console");
        assert!(!ok);
        assert_eq!(error.unwrap().code, status);
    }
}

#[test]
fn notify_to_view_event_projection_row() {
    let notify = Event::Notify {
        seq: 9,
        data: json!({
            "channel": "syn.control.projection",
            "data": {
                "view_id": "console",
                "target": "transport/position",
                "row": {
                    "observed": {
                        "revision": 4,
                        "value": { "plain": 2.0, "normalized": 0.2 },
                        "provenance": "commit",
                    },
                },
            },
        }),
    };
    let (view, Event::Notify { seq, data }) = notify_to_view_event(&notify).unwrap() else {
        panic!("projection must become a Notify")
    };
    assert_eq!(view, "console");
    assert_eq!(seq, 9, "seq carried");
    assert_eq!(data["control_id"], "transport/position");
    assert_eq!(data["revision"], 4);
    assert_eq!(data["value"]["normalized"], 0.2);
    assert_eq!(data["provenance"], "commit");

    // A row with no `observed` yet is not a state push.
    let mut empty = notify.clone();
    if let Event::Notify { data, .. } = &mut empty {
        data["data"]["row"]["observed"] = Value::Null;
    }
    assert!(notify_to_view_event(&empty).is_none());
    // Unrelated channels pass through untouched.
    assert!(
        notify_to_view_event(&Event::Notify {
            seq: 1,
            data: json!({ "channel": "syn.viz.frame", "data": {} }),
        })
        .is_none()
    );
    let _ = COMMAND_INTENT_OP;
}
