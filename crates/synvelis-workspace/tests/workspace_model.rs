//! Workspace Model (Issue #2) headless acceptance — playback-active
//! dock/window churn.
//!
//! `docs/test-specs/workspace-model.md` WSM-T-08: while transport is
//! rolling, dock moves, window create/close-veto, view detach/reattach
//! and layout churn must not touch transport state, the project
//! revision, the engine graph build counter, or the undo history.
//! `ws_a_05` covers the same invariants with transport stopped; this
//! test pins the "while playing" qualifier that Issue #2 calls out.
//! Deterministic: `CounterIdGenerator` ids and `FakeWindowHost`.

use ruviform_ui::dock::DockTarget;
use ruviform_ui::event::HostEvent;
use synvelis_project::{
    ClipExtent, ClipId, ClipPlacement, ClipPlayback, CounterIdGenerator, MusicalDuration,
    MusicalTick, PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp, SourceFrame,
    SourceRange, TimeAnchor,
};
use synvelis_workspace::{
    CommandContext, CommandOutcome, EditingSelection, FakeWindowHost, ProjectSession,
    SemanticTarget, ViewType, WorkspaceManager,
};

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xA11CE), "wsm-test");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    WorkspaceManager::new(FakeWindowHost::default(), ProjectSession::new(controller))
}

fn apply(ws: &mut Ws, op: ProjectOp) {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op");
}

fn playback() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::ZERO,
            SourceFrame::new(1_000_000).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

/// One track + lane + clip so a real editing selection exists.
fn seeded() -> (Ws, ClipId) {
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
            playback: playback(),
        },
    );
    let clip_id = *ws.session().project().clips.keys().next().unwrap();
    (ws, clip_id)
}

/// WSM-T-08 — dock/window churn while `transport.playing` must leave
/// transport, project revision, engine graph builds and undo history
/// untouched; the view's own state survives the moves.
#[test]
fn playback_dock_window_churn_preserves_transport_revision_and_history() {
    let (mut ws, clip) = seeded();
    let win_a = ws.create_window("A", 1280, 800).unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let arr = ws
        .spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win_a))
        .unwrap();
    let mixer = ws
        .spawn_view(ViewType::Mixer, "mix", DockTarget::Window(win_b))
        .unwrap();

    // Per-view state worth preserving across the churn.
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    ws.inspect(mixer, SemanticTarget::Clip(clip)).unwrap();

    // Start playback through the routed command (transport lane).
    let outcome = ws.dispatch(CommandContext::from_view("transport.play", arr));
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    assert!(ws.session().transport().playing);

    let rev0 = ws.session().revision();
    let builds0 = ws.session().engine().graph_builds();

    // Dock/window churn while playing: detach to a fresh window,
    // reattach, move between windows, window events, close veto.
    let win_c = ws.detach_to_new_window(arr, "c").unwrap();
    ws.reattach(arr, win_a).unwrap();
    ws.move_view(arr, DockTarget::Window(win_b)).unwrap();
    ws.move_view(mixer, DockTarget::Window(win_a)).unwrap();
    let outcome = ws.dispatch(CommandContext::from_view("view.activate", arr));
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    ws.on_window_event(win_a, &HostEvent::Focused(true))
        .unwrap();
    ws.on_window_event(win_b, &HostEvent::Moved { x: 5, y: 6 })
        .unwrap();
    ws.set_close_veto(win_c, true);
    ws.on_window_event(win_c, &HostEvent::CloseRequested)
        .unwrap();

    // Transport keeps rolling; project/engine/undo untouched.
    assert!(ws.session().transport().playing, "playback survives churn");
    assert_eq!(ws.session().revision(), rev0);
    assert_eq!(ws.session().engine().graph_builds(), builds0);
    assert!(ws.session_mut().undo().unwrap());
    assert!(ws.session_mut().redo().unwrap());
    assert!(ws.session().project().clips.contains_key(&clip));

    // View-local state survived the moves.
    let arr_state = ws.state(arr).expect("arr state");
    assert_eq!(
        arr_state.object_selections.primary(),
        Some(SemanticTarget::Clip(clip)),
        "editing selection preserved across dock moves"
    );
    let mix_state = ws.state(mixer).expect("mix state");
    assert_eq!(
        mix_state.inspection_focus.target,
        Some(SemanticTarget::Clip(clip)),
        "inspection focus preserved across dock moves"
    );

    // A routed editing command still works afterwards — playback does
    // not block the command lane. The undo+redo roundtrip above and
    // this remove are the only project mutations since rev0, so both
    // counters advance by exactly 3 (same accounting as ws_a_05).
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    let outcome = ws.dispatch(CommandContext::from_view("clip.remove", arr));
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    assert!(ws.session().transport().playing);
    assert_eq!(ws.session().revision().0, rev0.0 + 3);
    assert_eq!(ws.session().engine().graph_builds(), builds0 + 3);
}
