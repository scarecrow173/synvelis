//! WS-A acceptance tests — `docs/test-specs/ws-a-e2e-workspace-foundation.md`.
//!
//! Deterministic: `CounterIdGenerator` ids and `FakeWindowHost`.

use ruviform_ui::dock::{DockError, DockEvent, DockTarget, WindowCloseDisposition};
use ruviform_ui::event::HostEvent;
use ruviform_ui::window::{CloseDecision, MonitorInfo};
use synvelis_project::{
    ClipId, CounterIdGenerator, LaneId, MusicalRange, MusicalTick, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, SourceFrame, TimeAnchor, TrackId,
};
use synvelis_workspace::{
    CommandContext, CommandOutcome, EditingSelection, FakeWindowHost, InputContext, InspectionMode,
    LinkGroupId, ProjectSession, SemanticTarget, ViewType, WorkspaceError, WorkspaceManager,
    WorkspaceSnapshot,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xA11CE), "ws-test");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    WorkspaceManager::new(
        FakeWindowHost::with_monitors(vec![MonitorInfo {
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

/// Fixture-only mutation, bypassing the command router (setup, not
/// the thing under test).
fn apply(ws: &mut Ws, op: ProjectOp) -> ProjectRevision {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op")
}

/// A routed project mutation — goes through `dispatch` so all views
/// are notified/rendered.
fn rename_project(ws: &mut Ws, name: &str) {
    let ctx = CommandContext {
        id: "project.rename".to_string(),
        origin_window: None,
        origin_view: None,
        input: InputContext::View,
        explicit_targets: Vec::new(),
        selection: None,
        expected_revision: None,
        arg: Some(name.to_string()),
    };
    assert!(matches!(ws.dispatch(ctx), CommandOutcome::Applied { .. }));
}

/// track + lane + source + clip on the lane.
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
            anchor: TimeAnchor::Musical(
                MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(960)).unwrap(),
            ),
            source_offset: SourceFrame::new(0).unwrap(),
        },
    );
    let clip_id = *ws.session().project().clips.keys().next().unwrap();
    (ws, track_id, lane_id, clip_id)
}

fn add_clip(ws: &mut Ws, lane_id: LaneId) -> ClipId {
    let source_id = *ws.session().project().sources.keys().next().unwrap();
    apply(
        ws,
        ProjectOp::AddClip {
            lane_id,
            source_id,
            anchor: TimeAnchor::Musical(
                MusicalRange::try_new(MusicalTick::new(960), MusicalTick::new(1920)).unwrap(),
            ),
            source_offset: SourceFrame::new(0).unwrap(),
        },
    );
    *ws.session().project().clips.keys().last().unwrap()
}

// ---------------------------------------------------------------------------
// WS-A-01 single source of truth + independent per-view state
// ---------------------------------------------------------------------------

#[test]
fn ws_a_01_shared_project_independent_view_state() {
    let (mut ws, track, _lane, clip) = seeded();
    let win_a = ws.create_window("A", 1280, 800).unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let arr = ws
        .spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win_a))
        .unwrap();
    let mix = ws
        .spawn_view(ViewType::Mixer, "mix", DockTarget::Window(win_b))
        .unwrap();
    let insp = ws
        .spawn_view(ViewType::Inspector, "insp", DockTarget::Window(win_b))
        .unwrap();

    // one routed project mutation → every view, in every window,
    // notified + rendered off the same live revision
    rename_project(&mut ws, "shared");
    for v in [arr, mix, insp] {
        assert_eq!(ws.state(v).unwrap().revision_inbox.len(), 1);
        assert_eq!(ws.registry().headless(v).unwrap().render_count, 1);
    }
    assert_eq!(ws.session().project().name, "shared");
    assert!(ws.session().project().tracks.contains_key(&track));

    // per-view state independence
    ws.state_mut(arr).unwrap().editing_selection =
        EditingSelection::single(SemanticTarget::Clip(clip));
    ws.state_mut(insp).unwrap().inspection_focus.target = Some(SemanticTarget::Track(track));
    ws.state_mut(arr).unwrap().viewport.scroll_x = 42.0;
    ws.state_mut(mix).unwrap().viewport.zoom_x = 2.0;

    assert!(ws.state(mix).unwrap().editing_selection.is_empty());
    assert!(ws.state(arr).unwrap().inspection_focus.target.is_none());
    assert_eq!(ws.state(mix).unwrap().viewport.scroll_x, 0.0);
    assert_eq!(ws.state(arr).unwrap().viewport.zoom_x, 1.0);

    // the dock location never gates state ownership
    assert_eq!(ws.dock().owner_of(arr), Some(win_a));
    assert_eq!(ws.dock().owner_of(mix), Some(win_b));
}

// ---------------------------------------------------------------------------
// WS-A-02 link groups
// ---------------------------------------------------------------------------

#[test]
fn ws_a_02_link_group_semantics() {
    let (mut ws, track, lane, clip) = seeded();
    let win_a = ws.create_window("A", 1280, 800).unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let arr = ws
        .spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win_a))
        .unwrap();
    let mix = ws
        .spawn_view(ViewType::Mixer, "mix", DockTarget::Window(win_b))
        .unwrap();
    let insp = ws
        .spawn_view(ViewType::Inspector, "insp", DockTarget::Window(win_b))
        .unwrap();
    let pinned = ws
        .spawn_view(ViewType::Inspector, "pinned", DockTarget::Window(win_b))
        .unwrap();
    let other = ws
        .spawn_view(ViewType::SignalFlow, "other", DockTarget::Window(win_b))
        .unwrap();

    let pinned_target = SemanticTarget::Lane(lane);
    ws.state_mut(pinned).unwrap().inspection_focus.mode = InspectionMode::Pinned;
    ws.state_mut(pinned).unwrap().inspection_focus.target = Some(pinned_target);

    for v in [arr, mix, insp, pinned] {
        ws.links_mut().set_group(v, Some(LinkGroupId(1)));
    }
    ws.links_mut().set_group(other, Some(LinkGroupId(2)));

    // semantic mapping: clip selection → owning track highlight
    let delivered = ws
        .select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    assert_eq!(delivered.len(), 3);
    let mix_ctx = &ws.state(mix).unwrap().linked_context;
    assert_eq!(mix_ctx.target, Some(SemanticTarget::Track(track)));
    assert_eq!(mix_ctx.source, Some(arr));
    // linked context is not an editing selection
    assert!(ws.state(mix).unwrap().editing_selection.is_empty());
    // follow inspector tracks the semantic target
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(SemanticTarget::Track(track))
    );
    // pinned inspector keeps its own target
    assert_eq!(
        ws.state(pinned).unwrap().inspection_focus.target,
        Some(pinned_target)
    );
    // a different group receives nothing
    assert!(ws.state(other).unwrap().linked_context.target.is_none());

    // lane selection also maps to its track
    let _ = ws.select(arr, EditingSelection::single(SemanticTarget::Lane(lane)));
    assert_eq!(
        ws.state(mix).unwrap().linked_context.target,
        Some(SemanticTarget::Track(track))
    );

    // unmappable selection → no delivery at all
    let ghost = SemanticTarget::Clip(ClipId::new(0xDEAD));
    let delivered = ws.select(arr, EditingSelection::single(ghost)).unwrap();
    assert_eq!(delivered.len(), 0);
    // linked context from before is not disturbed by the failed publish
    assert_eq!(
        ws.state(mix).unwrap().linked_context.target,
        Some(SemanticTarget::Track(track))
    );
}

#[test]
fn ws_a_02b_link_dedup_no_reemit() {
    let (mut ws, _track, _lane, clip) = seeded();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let arr = ws
        .spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win))
        .unwrap();
    let mix = ws
        .spawn_view(ViewType::Mixer, "mix", DockTarget::Window(win))
        .unwrap();
    let loner = ws
        .spawn_view(ViewType::SignalFlow, "loner", DockTarget::Window(win))
        .unwrap();
    ws.links_mut().set_group(arr, Some(LinkGroupId(1)));
    ws.links_mut().set_group(mix, Some(LinkGroupId(1)));

    let delivered = ws
        .select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    let event = delivered[0].1;

    // same event twice → second delivery dropped (no duplicate apply,
    // no notification loop)
    assert!(!ws.redeliver(event, mix));
    assert_eq!(
        ws.state(mix).unwrap().linked_context.last_event,
        Some(event.id)
    );
    // delivery to a view outside the event's group is refused
    assert!(!ws.redeliver(event, loner));
    // delivery never re-emits: the receiver's own publish queue is
    // unaffected and produces no downstream deliveries to group
    // peers (arr's own state was not touched by mixer's apply)
    assert!(ws.state(arr).unwrap().linked_context.target.is_none());
}

// ---------------------------------------------------------------------------
// WS-A-03 command routing
// ---------------------------------------------------------------------------

#[test]
fn ws_a_03_command_routing() {
    let (mut ws, _track, lane, clip) = seeded();
    let win_a = ws.create_window("A", 1280, 800).unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let arr = ws
        .spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win_a))
        .unwrap();
    let insp = ws
        .spawn_view(ViewType::Inspector, "insp", DockTarget::Window(win_b))
        .unwrap();
    let mix = ws
        .spawn_view(ViewType::Mixer, "mix", DockTarget::Window(win_b))
        .unwrap();

    // editing command resolves the origin view's own selection
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    let rev = ws.session().revision();
    let outcome = ws.dispatch(CommandContext::from_view("clip.remove", arr));
    assert!(matches!(outcome, CommandOutcome::Applied { revision: Some(r) } if r.0 == rev.0 + 1));
    assert!(!ws.session().project().clips.contains_key(&clip));

    // undo restores it — history flows through the controller only
    assert!(ws.session_mut().controller_mut().undo().unwrap());
    assert!(ws.session().project().clips.contains_key(&clip));

    // explicit target (context-menu invocation) wins over selection
    let clip2 = add_clip(&mut ws, lane);
    let mut ctx = CommandContext::from_view("clip.remove", arr);
    ctx.explicit_targets = vec![SemanticTarget::Clip(clip2)];
    let outcome = ws.dispatch(ctx);
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    assert!(!ws.session().project().clips.contains_key(&clip2));
    // the merely-selected clip was not the victim
    assert!(ws.session().project().clips.contains_key(&clip));

    // unresolved: the origin view's selection is empty and no explicit
    // target was given — refused, project unchanged, and the command
    // never fell back to another window's stale selection
    ws.state_mut(mix).unwrap().editing_selection =
        EditingSelection::single(SemanticTarget::Clip(clip));
    let outcome = ws.dispatch(CommandContext::from_view("clip.remove", insp));
    assert!(matches!(outcome, CommandOutcome::Unresolved { .. }));
    assert!(ws.session().project().clips.contains_key(&clip));

    // kind-mismatched explicit target → unresolved
    let mut ctx = CommandContext::from_view("clip.remove", arr);
    ctx.explicit_targets = vec![SemanticTarget::Track(TrackId::new(0xBEEF))];
    let outcome = ws.dispatch(ctx);
    assert!(matches!(outcome, CommandOutcome::Unresolved { .. }));

    // input contexts capture editing/transport/project commands
    for input in [
        InputContext::TextInput,
        InputContext::Ime,
        InputContext::PluginGui,
        InputContext::Modal,
    ] {
        let mut ctx = CommandContext::from_view("clip.remove", arr);
        ctx.input = input;
        assert!(matches!(
            ws.dispatch(ctx),
            CommandOutcome::Suppressed { .. }
        ));
        let mut ctx = CommandContext::from_view("transport.play", arr);
        ctx.input = input;
        assert!(matches!(
            ws.dispatch(ctx),
            CommandOutcome::Suppressed { .. }
        ));
    }
    assert!(!ws.session().transport().playing);

    // stale expected revision → rejected, project unchanged
    let mut ctx = CommandContext::from_view("clip.remove", arr);
    ctx.expected_revision = Some(ProjectRevision(0));
    let outcome = ws.dispatch(ctx);
    match outcome {
        CommandOutcome::Rejected { error } => {
            assert!(format!("{error}").contains("stale"), "{error}");
        }
        other => panic!("expected Rejected, got {other:?}"),
    }
    assert!(ws.session().project().clips.contains_key(&clip));

    // view command activates a tab without touching the project
    let rev = ws.session().revision();
    let outcome = ws.dispatch(CommandContext::from_view("view.activate", mix));
    assert!(matches!(
        outcome,
        CommandOutcome::Applied { revision: None }
    ));
    assert_eq!(ws.session().revision(), rev);

    // transport command moves the transport stub only
    let outcome = ws.dispatch(CommandContext::from_view("transport.play", arr));
    assert!(matches!(
        outcome,
        CommandOutcome::Applied { revision: None }
    ));
    assert!(ws.session().transport().playing);
    assert_eq!(ws.session().revision(), rev);

    // unknown command → rejected
    let outcome = ws.dispatch(CommandContext::from_view("nope.nope", arr));
    assert!(matches!(outcome, CommandOutcome::Rejected { .. }));
}

// ---------------------------------------------------------------------------
// WS-A-04 dock lifecycle
// ---------------------------------------------------------------------------

#[test]
fn ws_a_04_detach_reattach_move_rollback() {
    let (mut ws, _t, _l, clip) = seeded();
    let win_a = ws.create_window("A", 1280, 800).unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let arr = ws
        .spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win_a))
        .unwrap();
    ws.state_mut(arr).unwrap().editing_selection =
        EditingSelection::single(SemanticTarget::Clip(clip));

    // detach → new window: identity + state preserved
    let win_c = ws.detach_to_new_window(arr, "detached-arr").unwrap();
    assert_eq!(ws.dock().owner_of(arr), Some(win_c));
    assert!(ws.dock().is_registered(win_c));
    assert_eq!(
        ws.state(arr).unwrap().editing_selection.primary(),
        Some(SemanticTarget::Clip(clip))
    );
    // reattach to window B
    ws.reattach(arr, win_b).unwrap();
    assert_eq!(ws.dock().owner_of(arr), Some(win_b));

    // two-phase move rollback: destination disappears between
    // begin and commit → commit fails as TargetUnavailable, the view
    // keeps source ownership, MoveAborted is emitted
    let token = ws.begin_transfer(arr, DockTarget::Window(win_a)).unwrap();
    assert!(ws.dock().is_transferring(arr));
    ws.on_window_event(win_a, &HostEvent::Destroyed).unwrap();
    let err = ws.commit_transfer(token).unwrap_err();
    assert!(matches!(
        err,
        WorkspaceError::Dock(DockError::TargetUnavailable { .. })
    ));
    let events = ws.drain_dock_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DockEvent::MoveAborted { view } if *view == arr))
    );
    assert_eq!(ws.dock().owner_of(arr), Some(win_b));
    assert!(!ws.dock().is_transferring(arr));

    // host-side create failure → whole operation fails cleanly and
    // the view stays at its source
    ws.host.fail_create = true;
    assert!(ws.detach_to_new_window(arr, "wont-exist").is_err());
    assert_eq!(ws.dock().owner_of(arr), Some(win_b));
    ws.host.fail_create = false;

    // explicit abort path: begin against a live target, then abort —
    // the view keeps source ownership
    let token = ws.begin_transfer(arr, DockTarget::Window(win_c)).unwrap();
    ws.abort_transfer(token).unwrap();
    assert_eq!(ws.dock().owner_of(arr), Some(win_b));
    assert!(!ws.dock().is_transferring(arr));

    // editing a mid-transfer view is locked (ViewBusy)
    let token = ws.begin_transfer(arr, DockTarget::Window(win_b)).unwrap();
    let outcome = ws.dispatch(CommandContext::from_view("clip.remove", arr));
    match outcome {
        CommandOutcome::Rejected {
            error: WorkspaceError::ViewBusy(v),
        } => assert_eq!(v, arr),
        other => panic!("expected ViewBusy rejection, got {other:?}"),
    }
    ws.abort_transfer(token).unwrap();
}

#[test]
fn ws_a_04b_window_close_veto_and_detach() {
    let (mut ws, _t, _l, _c) = seeded();
    let win_a = ws.create_window("A", 1280, 800).unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let mix = ws
        .spawn_view(ViewType::Mixer, "mix", DockTarget::Window(win_b))
        .unwrap();
    let insp = ws
        .spawn_view(ViewType::Inspector, "insp", DockTarget::Window(win_b))
        .unwrap();
    let arr = ws
        .spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win_a))
        .unwrap();

    // vetoed close: window stays registered, views stay docked
    ws.set_close_veto(win_b, true);
    ws.request_close(win_b);
    ws.on_window_event(win_b, &HostEvent::CloseRequested)
        .unwrap();
    assert_eq!(
        ws.host.close_decisions.last(),
        Some(&(win_b, CloseDecision::Veto))
    );
    assert!(ws.dock().is_registered(win_b));
    assert_eq!(ws.dock().owner_of(mix), Some(win_b));

    // approved close: views land in the detached pool and stay in the
    // registries (DetachViews disposition)
    ws.set_close_veto(win_b, false);
    ws.on_window_event(win_b, &HostEvent::CloseRequested)
        .unwrap();
    assert_eq!(
        ws.host.close_decisions.last(),
        Some(&(win_b, CloseDecision::Approve))
    );
    ws.on_window_event(win_b, &HostEvent::Destroyed).unwrap();
    assert!(!ws.dock().is_registered(win_b));
    assert!(ws.dock().is_detached(mix));
    assert!(ws.dock().is_detached(insp));
    assert!(ws.registry().contains(mix));
    assert!(ws.state(mix).is_ok());
    assert_eq!(ws.dock().owner_of(arr), Some(win_a));
}

#[test]
fn ws_a_04c_layout_save_restore() {
    let (mut ws, track, _l, clip) = seeded();
    let win_a = ws.create_window_at("A", 0, 0, 1280, 800).unwrap();
    let win_b = ws.create_window_at("B", 100, 50, 1024, 768).unwrap();
    let arr = ws
        .spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win_a))
        .unwrap();
    let mix = ws
        .spawn_view(ViewType::Mixer, "mix", DockTarget::Window(win_b))
        .unwrap();
    let insp = ws.spawn_detached_view(ViewType::Inspector, "insp").unwrap();
    ws.links_mut().set_group(arr, Some(LinkGroupId(1)));
    ws.links_mut().set_group(mix, Some(LinkGroupId(1)));
    ws.state_mut(arr).unwrap().editing_selection =
        EditingSelection::single(SemanticTarget::Clip(clip));
    ws.state_mut(insp).unwrap().inspection_focus.target = Some(SemanticTarget::Track(track));

    let snap = ws.capture().unwrap();
    let json = snap.to_json().unwrap();

    // restore into a fresh workspace+host
    let mut ws2 = fresh_ws();
    let monitors = ws2.host.monitors.clone();
    let snap2 = WorkspaceSnapshot::from_json(&json).unwrap();
    ws2.restore(&snap2, &monitors).unwrap();

    // two windows recreated in snapshot order, views docked identically
    assert_eq!(ws2.windows().len(), 2);
    assert_eq!(ws2.windows()[0].title, "A");
    assert_eq!(ws2.windows()[1].title, "B");
    assert_eq!(ws2.dock().owner_of(arr), Some(ws2.windows()[0].id));
    assert_eq!(ws2.dock().owner_of(mix), Some(ws2.windows()[1].id));
    // detached view stays detached; state + links restored
    assert!(ws2.dock().is_detached(insp));
    assert_eq!(
        ws2.state(arr).unwrap().editing_selection.primary(),
        Some(SemanticTarget::Clip(clip))
    );
    assert_eq!(
        ws2.state(insp).unwrap().inspection_focus.target,
        Some(SemanticTarget::Track(track))
    );
    assert_eq!(ws2.links().group_of(arr), Some(LinkGroupId(1)));
    assert_eq!(ws2.links().group_of(mix), Some(LinkGroupId(1)));
    assert_eq!(ws2.descriptor(arr).unwrap().kind, ViewType::Arrangement);

    // malformed JSON → hard error before any state is touched
    assert!(WorkspaceSnapshot::from_json("not json {{{").is_err());
    // wrong envelope schema → rejected
    let mut bad_schema = snap2.clone();
    bad_schema.schema = 999;
    let bad_json = serde_json::to_string(&bad_schema).unwrap();
    assert!(WorkspaceSnapshot::from_json(&bad_json).is_err());
    // corrupted layout payload → safe default: views still restored
    // and parked in the detached pool rather than lost
    let mut corrupted = snap2.clone();
    corrupted.layout.schema = 999;
    let mut ws3 = fresh_ws();
    ws3.restore(&corrupted, &[]).unwrap();
    assert!(ws3.registry().contains(arr));
    assert!(ws3.dock().is_detached(arr));
    assert!(ws3.dock().is_detached(mix));
    assert_eq!(
        ws3.state(arr).unwrap().editing_selection.primary(),
        Some(SemanticTarget::Clip(clip))
    );
}

// ---------------------------------------------------------------------------
// WS-A-05 dock/window ops never touch project history or the engine stub
// ---------------------------------------------------------------------------

#[test]
fn ws_a_05_layout_ops_leave_project_and_engine_alone() {
    let (mut ws, _t, _l, clip) = seeded();
    let win_a = ws.create_window("A", 1280, 800).unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let arr = ws
        .spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win_a))
        .unwrap();

    let rev0 = ws.session().revision();
    let builds0 = ws.session().engine().graph_builds;

    // dock + window + focus churn
    let win_c = ws.detach_to_new_window(arr, "c").unwrap();
    ws.reattach(arr, win_a).unwrap();
    ws.move_view(arr, DockTarget::Window(win_b)).unwrap();
    let outcome = ws.dispatch(CommandContext::from_view("view.activate", arr));
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    ws.on_window_event(win_a, &HostEvent::Focused(true))
        .unwrap();
    ws.on_window_event(win_b, &HostEvent::Moved { x: 5, y: 6 })
        .unwrap();
    ws.on_window_event(
        win_b,
        &HostEvent::Resized {
            width: 500,
            height: 400,
        },
    )
    .unwrap();
    ws.set_close_veto(win_c, true);
    ws.on_window_event(win_c, &HostEvent::CloseRequested)
        .unwrap();

    assert_eq!(ws.session().revision(), rev0);
    assert_eq!(ws.session().engine().graph_builds, builds0);
    // undo history unpolluted: the last *project* op still undoes
    assert!(ws.session_mut().controller_mut().undo().unwrap());
    assert!(ws.session_mut().controller_mut().redo().unwrap());
    assert!(ws.session().project().clips.contains_key(&clip));

    // a project command is the only thing that advances both counters
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    let outcome = ws.dispatch(CommandContext::from_view("clip.remove", arr));
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    // undo and redo each advanced the live revision by one
    assert_eq!(ws.session().revision().0, rev0.0 + 3);
    assert_eq!(ws.session().engine().graph_builds, builds0 + 1);
}

// keep the import used
#[allow(dead_code)]
fn _close_disposition_uses(_: WindowCloseDisposition) {}
