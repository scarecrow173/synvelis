//! SF-05 slice 2 (synvelis#108): SF-UX-03 routing gestures,
//! SF-UX-04 Signal Flow inspection/link semantics, SF-UX-07 per-view
//! persistence.
//!
//! - SF-UX-03 — `routing_gesture_*`: Begin fixes view/source/revision/
//!   capability/window; hover is a dry-run assessment only; drop
//!   freezes the target; confirm commits one `ProjectOp::Routing`
//!   (one undo boundary); stale revision/window/capability drops
//!   without retargeting; channel-layout mismatch requires explicit
//!   `ExplicitChannelMap` consent; cancel leaves the project
//!   untouched.
//! - SF-UX-04 — `signal_flow_*` inspection focus follows the shared
//!   Follow/Explore/Pinned semantics independently of the editing
//!   selection and the arrangement inspection focus; link delivery
//!   retargets only Follow mode with a matching source filter.
//! - SF-UX-07 — `ViewState::signal_flow` round-trips through
//!   `WorkspaceSnapshot` per `ViewInstanceId`; schema v2 migrates;
//!   malformed signal-flow payloads degrade to `None`.

use ruviform_ui::dock::DockTarget;
use synvelis_project::{
    CounterIdGenerator, ExplicitChannelMap, MusicalTick, PortDirection, PortRole, Project,
    ProjectController, ProjectId, ProjectOp, ProjectRevision, SemanticPortId, StripId, TrackId,
};
use synvelis_workspace::{
    CancelReason, EditTimePoint, EditingSelection, FakeWindowHost, FollowSource, HitProbe,
    InspectionMode, LinkGroupId, ProjectSession, RoutingGestureOutcome, RoutingPortAssessment,
    SemanticTarget, SignalFlowEntityRef, SignalFlowInspectionFocus, ViewType, WorkspaceError,
    WorkspaceManager, WorkspaceSnapshot, visual_move_node,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0x108), "sf05-s2-test");
    WorkspaceManager::new(
        FakeWindowHost::default(),
        ProjectSession::new(ProjectController::new(
            project,
            Box::new(CounterIdGenerator::new(1)),
        )),
    )
}

fn apply(ws: &mut Ws, op: ProjectOp) -> ProjectRevision {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op")
}

fn add_track(ws: &mut Ws, name: &str) -> TrackId {
    apply(ws, ProjectOp::AddTrack { name: name.into() });
    *ws.session().project().tracks.keys().next_back().expect("track")
}

fn add_track_strip(ws: &mut Ws, track: TrackId) -> StripId {
    apply(ws, ProjectOp::AddTrackStrip { track_id: track });
    ws.session()
        .project()
        .routing
        .strip_of_track(track)
        .unwrap()
        .id
}

/// A strip's main port in `direction`.
fn strip_main_port(ws: &Ws, strip: StripId, direction: PortDirection) -> SemanticPortId {
    ws.session().project().routing.strips[&strip]
        .port_ids
        .iter()
        .find(|id| {
            let p = &ws.session().project().routing.ports[id];
            p.direction == direction && p.role == PortRole::Main
        })
        .copied()
        .expect("strip main port")
}

fn connections(ws: &Ws) -> usize {
    ws.session().project().routing.connections.len()
}

/// Two routed tracks (bass, kick) — each strip auto-routes to master.
fn routed_ws() -> (Ws, TrackId, TrackId, StripId, StripId) {
    let mut ws = fresh_ws();
    let bass = add_track(&mut ws, "bass");
    let kick = add_track(&mut ws, "kick");
    let bass_strip = add_track_strip(&mut ws, bass);
    let kick_strip = add_track_strip(&mut ws, kick);
    (ws, bass, kick, bass_strip, kick_strip)
}

fn spawn(ws: &mut Ws, kind: ViewType, title: &str, window: ruviform_ui::window::WindowId) -> ruviform_ui::dock::ViewInstanceId {
    ws.spawn_view(kind, title, DockTarget::Window(window))
        .unwrap()
}

// ---------------------------------------------------------------------------
// SF-UX-03: port-drag routing gesture
// ---------------------------------------------------------------------------

#[test]
fn routing_gesture_commit_is_one_atomic_undo() {
    let (mut ws, _bass, _kick, bass_strip, kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn(&mut ws, ViewType::SignalFlow, "sf", win);

    let source = strip_main_port(&ws, bass_strip, PortDirection::Output);
    let target = strip_main_port(&ws, kick_strip, PortDirection::Input);
    let before = connections(&ws);
    let revision = ws.session().revision();

    let fixation = ws.routing_gesture_begin(view, source).unwrap();
    assert_eq!(fixation.view, view);
    assert_eq!(fixation.source, source);
    assert_eq!(fixation.revision, revision);

    // Preview is assessment-only: compatible stereo→stereo main route.
    let assessment = ws.routing_gesture_hover(view, Some(target)).unwrap();
    assert_eq!(assessment, Some(RoutingPortAssessment::Compatible));
    assert_eq!(ws.routing_gesture_preview(view).unwrap().candidate, Some(target));
    // Nothing reached the project.
    assert_eq!(connections(&ws), before);
    assert_eq!(ws.session().revision(), revision);

    assert_eq!(ws.routing_gesture_drop(view).unwrap(), Some(target));
    // After drop the target is frozen — hover can no longer move it.
    assert!(matches!(
        ws.routing_gesture_hover(view, Some(source)),
        Err(WorkspaceError::InvalidGesture(_))
    ));

    let outcome = ws.routing_gesture_confirm(view, None).unwrap();
    let RoutingGestureOutcome::Applied { revision: new_rev } = outcome else {
        panic!("expected Applied, got {outcome:?}")
    };
    assert_eq!(new_rev.0, revision.0 + 1);
    assert_eq!(connections(&ws), before + 1);
    assert!(ws.routing_gesture(view).is_none());

    // Exactly one undo boundary reverts the whole commit. Live
    // revision is monotonic — undo bumps it rather than rewinding.
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(connections(&ws), before);
    assert_eq!(ws.session().revision().0, new_rev.0 + 1);
}

#[test]
fn routing_gesture_preview_and_cancel_never_touch_the_project() {
    let (mut ws, _bass, _kick, bass_strip, kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn(&mut ws, ViewType::SignalFlow, "sf", win);

    let source = strip_main_port(&ws, bass_strip, PortDirection::Output);
    let target = strip_main_port(&ws, kick_strip, PortDirection::Input);
    let before = connections(&ws);
    let revision = ws.session().revision();

    ws.routing_gesture_begin(view, source).unwrap();
    ws.routing_gesture_hover(view, Some(target)).unwrap();
    ws.routing_gesture_drop(view).unwrap();
    let outcome = ws
        .routing_gesture_cancel(view, CancelReason::Escape)
        .unwrap();
    assert!(matches!(
        outcome,
        RoutingGestureOutcome::Cancelled {
            reason: CancelReason::Escape
        }
    ));
    assert_eq!(connections(&ws), before);
    assert_eq!(ws.session().revision(), revision);
    assert!(ws.routing_gesture(view).is_none());
    // The gesture is gone — further gesture calls report NoGesture.
    assert!(matches!(
        ws.routing_gesture_cancel(view, CancelReason::Escape),
        Err(WorkspaceError::NoGesture(_))
    ));
}

#[test]
fn routing_gesture_drop_on_empty_canvas_cancels() {
    let (mut ws, _bass, _kick, bass_strip, _kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn(&mut ws, ViewType::SignalFlow, "sf", win);

    let source = strip_main_port(&ws, bass_strip, PortDirection::Output);
    ws.routing_gesture_begin(view, source).unwrap();
    // No hover candidate → drop ends the gesture with no target.
    assert_eq!(ws.routing_gesture_drop(view).unwrap(), None);
    assert!(ws.routing_gesture(view).is_none());
    assert!(matches!(
        ws.routing_gesture_confirm(view, None),
        Err(WorkspaceError::NoGesture(_))
    ));
}

#[test]
fn routing_gesture_stale_revision_drops_without_retarget() {
    let (mut ws, _bass, _kick, bass_strip, kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn(&mut ws, ViewType::SignalFlow, "sf", win);

    let source = strip_main_port(&ws, bass_strip, PortDirection::Output);
    let target = strip_main_port(&ws, kick_strip, PortDirection::Input);
    ws.routing_gesture_begin(view, source).unwrap();
    ws.routing_gesture_hover(view, Some(target)).unwrap();
    ws.routing_gesture_drop(view).unwrap();

    // A concurrent window's project mutation advances the revision —
    // the gesture must drop stale, never retarget to current state.
    let other = add_track(&mut ws, "drums");
    assert!(ws.session().project().tracks.contains_key(&other));
    let before = connections(&ws);

    let outcome = ws.routing_gesture_confirm(view, None).unwrap();
    assert_eq!(outcome, RoutingGestureOutcome::Stale);
    assert_eq!(connections(&ws), before);
    assert!(ws.routing_gesture(view).is_none());
}

#[test]
fn routing_gesture_window_transfer_drops_stale() {
    let (mut ws, _bass, _kick, bass_strip, kick_strip) = routed_ws();
    let win_a = ws.create_window("A", 1280, 800).unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let view = spawn(&mut ws, ViewType::SignalFlow, "sf", win_a);

    let source = strip_main_port(&ws, bass_strip, PortDirection::Output);
    let target = strip_main_port(&ws, kick_strip, PortDirection::Input);
    ws.routing_gesture_begin(view, source).unwrap();
    ws.routing_gesture_hover(view, Some(target)).unwrap();
    ws.routing_gesture_drop(view).unwrap();

    // The view moves windows mid-gesture — the fixated window epoch
    // no longer matches, so confirm drops stale.
    ws.move_view(view, DockTarget::Window(win_b)).unwrap();
    let before = connections(&ws);
    let outcome = ws.routing_gesture_confirm(view, None).unwrap();
    assert_eq!(outcome, RoutingGestureOutcome::Stale);
    assert_eq!(connections(&ws), before);
    assert!(ws.routing_gesture(view).is_none());
}

#[test]
fn routing_gesture_rejects_direction_mismatch_as_prohibited() {
    let (mut ws, _bass, _kick, bass_strip, kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn(&mut ws, ViewType::SignalFlow, "sf", win);

    // Input → Input is never a legal route: the dry-run surfaces the
    // typed rejection instead of a heuristic guess.
    let source = strip_main_port(&ws, bass_strip, PortDirection::Input);
    let target = strip_main_port(&ws, kick_strip, PortDirection::Input);
    ws.routing_gesture_begin(view, source).unwrap();
    let assessment = ws.routing_gesture_hover(view, Some(target)).unwrap();
    assert!(matches!(
        assessment,
        Some(RoutingPortAssessment::Prohibited { .. })
    ));
    // Committing the prohibited pair is rejected by the project
    // oracle path — the gesture stays alive for cancel, project
    // untouched.
    ws.routing_gesture_drop(view).unwrap();
    let before = connections(&ws);
    assert!(ws.routing_gesture_confirm(view, None).is_err());
    assert_eq!(connections(&ws), before);
    assert!(ws.routing_gesture(view).is_some());
    ws.routing_gesture_cancel(view, CancelReason::Escape)
        .unwrap();
}

#[test]
fn routing_gesture_channel_mismatch_requires_explicit_consent() {
    // Fixture surgery: rewrite one strip input port to mono so the
    // stereo→mono pair trips the channel-layout policy. Public ops
    // cannot mint mismatched layouts, so the project aggregate is
    // patched directly before the session is built.
    let mut ws = fresh_ws();
    let bass = add_track(&mut ws, "bass");
    let kick = add_track(&mut ws, "kick");
    let bass_strip = add_track_strip(&mut ws, bass);
    let kick_strip = add_track_strip(&mut ws, kick);
    let source = strip_main_port(&ws, bass_strip, PortDirection::Output);
    let target = strip_main_port(&ws, kick_strip, PortDirection::Input);

    let mut project = ws.session().project().clone();
    project.routing.ports.get_mut(&target).unwrap().channels = 1;
    let session = ProjectSession::new(ProjectController::new(
        project,
        Box::new(CounterIdGenerator::new(
            ws.session().project().id_watermark,
        )),
    ));
    ws = WorkspaceManager::new(FakeWindowHost::default(), session);
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn(&mut ws, ViewType::SignalFlow, "sf", win);

    ws.routing_gesture_begin(view, source).unwrap();
    let assessment = ws.routing_gesture_hover(view, Some(target)).unwrap();
    assert_eq!(
        assessment,
        Some(RoutingPortAssessment::NeedsMapping {
            source_channels: 2,
            destination_channels: 1,
        })
    );
    ws.routing_gesture_drop(view).unwrap();

    // No consent → the project rejects; the gesture stays alive.
    let before = connections(&ws);
    assert!(ws.routing_gesture_confirm(view, None).is_err());
    assert_eq!(connections(&ws), before);
    assert!(ws.routing_gesture(view).is_some());

    // Explicit consent commits the mapped route — one undo boundary.
    let outcome = ws
        .routing_gesture_confirm(
            view,
            Some(ExplicitChannelMap {
                entries: vec![0],
            }),
        )
        .unwrap();
    assert!(matches!(outcome, RoutingGestureOutcome::Applied { .. }));
    assert_eq!(connections(&ws), before + 1);
    let conn = ws
        .session()
        .project()
        .routing
        .connections
        .values()
        .find(|c| c.source == source && c.destination == target)
        .expect("committed connection");
    assert_eq!(conn.channel_map, Some(ExplicitChannelMap { entries: vec![0] }));
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(connections(&ws), before);
}

#[test]
fn routing_gesture_mutual_exclusion_with_other_gesture_families() {
    let (mut ws, _bass, _kick, bass_strip, _kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn(&mut ws, ViewType::SignalFlow, "sf", win);

    let source = strip_main_port(&ws, bass_strip, PortDirection::Output);
    ws.routing_gesture_begin(view, source).unwrap();
    // A second routing gesture and a timeline pointer-down are both
    // refused while one runs.
    assert!(matches!(
        ws.routing_gesture_begin(view, source),
        Err(WorkspaceError::GestureActive(_))
    ));
    assert!(matches!(
        ws.pointer_down(
            view,
            &HitProbe {
                position: EditTimePoint::Musical(MusicalTick::new(0)),
                candidates: vec![],
            },
        ),
        Err(WorkspaceError::GestureActive(_))
    ));
    ws.routing_gesture_cancel(view, CancelReason::Escape)
        .unwrap();
}

// ---------------------------------------------------------------------------
// SF-UX-04: inspection focus + link semantics
// ---------------------------------------------------------------------------

#[test]
fn signal_flow_inspect_is_independent_of_selection_and_arrangement_focus() {
    let (mut ws, track, _kick, _bass_strip, _kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let arr = spawn(&mut ws, ViewType::Arrangement, "arr", win);
    let sf = spawn(&mut ws, ViewType::SignalFlow, "sf", win);

    ws.signal_flow_inspect(sf, SignalFlowEntityRef::Track(track))
        .unwrap();
    let state = ws.state(sf).unwrap();
    let sf_state = state.signal_flow.as_ref().expect("signal flow state");
    assert_eq!(
        sf_state.inspection.target,
        Some(SignalFlowEntityRef::Track(track))
    );
    assert_eq!(sf_state.inspection.mode, InspectionMode::Explore);
    // The arrangement inspection focus and the editing selection are
    // untouched — three independent focus channels.
    assert!(state.inspection_focus.target.is_none());
    assert!(state.object_selections.is_empty());
    assert!(ws.state(arr).unwrap().signal_flow.is_none());
}

#[test]
fn signal_flow_link_delivery_retargets_only_follow_mode() {
    let (mut ws, track, kick, _bass_strip, _kick_strip) = routed_ws();
    let win_a = ws.create_window("A", 1280, 800).unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let arr = spawn(&mut ws, ViewType::Arrangement, "arr", win_a);
    let follow = spawn(&mut ws, ViewType::SignalFlow, "follow", win_b);
    let explore = spawn(&mut ws, ViewType::SignalFlow, "explore", win_b);
    let pinned = spawn(&mut ws, ViewType::SignalFlow, "pinned", win_b);

    // Give each Signal Flow view its state. Explore and Pinned get a
    // *different* inspected target so a wrongful retarget is
    // observable; Follow starts empty and must adopt the event.
    ws.signal_flow_set_inspection_mode(follow, InspectionMode::Follow)
        .unwrap();
    ws.signal_flow_inspect(explore, SignalFlowEntityRef::Track(kick))
        .unwrap();
    ws.signal_flow_inspect(pinned, SignalFlowEntityRef::Track(kick))
        .unwrap();
    ws.signal_flow_set_inspection_mode(pinned, InspectionMode::Pinned)
        .unwrap();
    let pinned_target = ws
        .state(pinned)
        .unwrap()
        .signal_flow
        .as_ref()
        .unwrap()
        .inspection
        .target;
    assert_eq!(pinned_target, Some(SignalFlowEntityRef::Track(kick)));

    for v in [arr, follow, explore, pinned] {
        ws.links_mut().set_group(v, Some(LinkGroupId(1)));
    }

    // A clip-free track selection publishes the track itself — but
    // `select` needs a real selection; use the track target directly.
    let delivered = ws
        .select(arr, EditingSelection::single(SemanticTarget::Track(track)))
        .unwrap();
    assert_eq!(delivered.len(), 3);

    // Follow mode adopted the semantic target as a routing entity.
    let follow_state = ws.state(follow).unwrap();
    assert_eq!(
        follow_state
            .signal_flow
            .as_ref()
            .unwrap()
            .inspection
            .target,
        Some(SignalFlowEntityRef::Track(track))
    );
    // ...without touching the editing selection, and the arrangement
    // inspection focus tracked independently (its own Follow default).
    assert!(follow_state.object_selections.is_empty());
    assert_eq!(
        follow_state.inspection_focus.target,
        Some(SemanticTarget::Track(track))
    );
    // Explore and Pinned never retarget on link events — both keep
    // their own Track(kick) target while the event carried Track(bass).
    assert_eq!(
        ws.state(explore)
            .unwrap()
            .signal_flow
            .as_ref()
            .unwrap()
            .inspection
            .target,
        Some(SignalFlowEntityRef::Track(kick))
    );
    assert_eq!(
        ws.state(pinned)
            .unwrap()
            .signal_flow
            .as_ref()
            .unwrap()
            .inspection
            .target,
        pinned_target
    );
}

#[test]
fn signal_flow_link_follow_source_filter_and_dedup() {
    let (mut ws, track, _kick, _bass_strip, _kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let arr = spawn(&mut ws, ViewType::Arrangement, "arr", win);
    let other = spawn(&mut ws, ViewType::Mixer, "other", win);
    let sf = spawn(&mut ws, ViewType::SignalFlow, "sf", win);

    ws.signal_flow_set_inspection_mode(sf, InspectionMode::Follow)
        .unwrap();
    // Narrow Follow to events published by `other` — arr's events
    // must not retarget.
    ws.signal_flow_set_follow_source(sf, Some(FollowSource::View(other)))
        .unwrap();
    for v in [arr, other, sf] {
        ws.links_mut().set_group(v, Some(LinkGroupId(1)));
    }

    let delivered = ws
        .select(arr, EditingSelection::single(SemanticTarget::Track(track)))
        .unwrap();
    assert_eq!(delivered.len(), 2);
    assert!(
        ws.state(sf)
            .unwrap()
            .signal_flow
            .as_ref()
            .unwrap()
            .inspection
            .target
            .is_none(),
        "filtered-out publisher must not retarget"
    );
    // linked_context still records the event — filtering only gates
    // inspection retargeting.
    assert_eq!(
        ws.state(sf).unwrap().linked_context.target,
        Some(SemanticTarget::Track(track))
    );

    // Widen to the whole group → the next event retargets.
    ws.signal_flow_set_follow_source(sf, Some(FollowSource::Group(LinkGroupId(1))))
        .unwrap();
    let delivered = ws
        .select(arr, EditingSelection::single(SemanticTarget::Track(track)))
        .unwrap();
    let event = delivered[0].1;
    assert_eq!(
        ws.state(sf)
            .unwrap()
            .signal_flow
            .as_ref()
            .unwrap()
            .inspection
            .target,
        Some(SignalFlowEntityRef::Track(track))
    );

    // Dedup is structural: redelivering the same event is refused and
    // cannot double-apply.
    assert!(!ws.redeliver(event, sf));
    assert_eq!(
        ws.state(sf).unwrap().linked_context.last_event,
        Some(event.id)
    );
}

#[test]
fn signal_flow_explore_history_and_unmappable_targets() {
    let (mut ws, bass, kick, _bass_strip, _kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let sf = spawn(&mut ws, ViewType::SignalFlow, "sf", win);

    ws.signal_flow_inspect(sf, SignalFlowEntityRef::Track(bass))
        .unwrap();
    ws.signal_flow_inspect(sf, SignalFlowEntityRef::Track(kick))
        .unwrap();
    let state = ws.state(sf).unwrap();
    let focus = &state.signal_flow.as_ref().unwrap().inspection;
    assert_eq!(focus.target, Some(SignalFlowEntityRef::Track(kick)));
    assert_eq!(focus.explore_back, vec![SignalFlowEntityRef::Track(bass)]);
    assert!(focus.explore_fwd.is_empty());

    ws.signal_flow_explore_back(sf).unwrap();
    let focus = &ws.state(sf).unwrap().signal_flow.as_ref().unwrap().inspection;
    assert_eq!(focus.target, Some(SignalFlowEntityRef::Track(bass)));
    assert_eq!(focus.explore_fwd, vec![SignalFlowEntityRef::Track(kick)]);

    ws.signal_flow_explore_forward(sf).unwrap();
    let focus = &ws.state(sf).unwrap().signal_flow.as_ref().unwrap().inspection;
    assert_eq!(focus.target, Some(SignalFlowEntityRef::Track(kick)));

    // Leaving Explore clears the session history.
    ws.signal_flow_set_inspection_mode(sf, InspectionMode::Pinned)
        .unwrap();
    let focus = &ws.state(sf).unwrap().signal_flow.as_ref().unwrap().inspection;
    assert!(focus.explore_back.is_empty() && focus.explore_fwd.is_empty());
    assert_eq!(focus.mode, InspectionMode::Pinned);
}

// ---------------------------------------------------------------------------
// SF-UX-07: per-view persistence
// ---------------------------------------------------------------------------

#[test]
fn capture_restore_roundtrips_signal_flow_state_per_view() {
    let (mut ws, track, _kick, bass_strip, _kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let sf = spawn(&mut ws, ViewType::SignalFlow, "sf", win);
    let plain = spawn(&mut ws, ViewType::Mixer, "plain", win);

    // Visual + inspection state on the Signal Flow view only.
    {
        let state = ws.state_mut(sf).unwrap();
        let sf_state = state.signal_flow.get_or_insert_with(Default::default);
        visual_move_node(&mut sf_state.visual, SignalFlowEntityRef::Track(track), [10.0, 20.0]);
    }
    ws.signal_flow_inspect(sf, SignalFlowEntityRef::Track(track))
        .unwrap();
    ws.signal_flow_set_inspection_mode(sf, InspectionMode::Pinned)
        .unwrap();

    let snap = ws.capture().unwrap();
    let json = snap.to_json().unwrap();
    let parsed = WorkspaceSnapshot::from_json(&json).unwrap();
    assert_eq!(parsed.schema, synvelis_workspace::WORKSPACE_SCHEMA);

    // Restore into a fresh workspace — per-view state survives,
    // including for views parked detached.
    let mut ws2 = fresh_ws();
    ws2.restore(&parsed, &[]).unwrap();
    let restored = ws2.state(sf).unwrap();
    let sf_state = restored.signal_flow.as_ref().expect("restored sf state");
    assert_eq!(
        sf_state.visual.positions.get(&SignalFlowEntityRef::Track(track)),
        Some(&[10.0, 20.0])
    );
    assert_eq!(
        sf_state.inspection.target,
        Some(SignalFlowEntityRef::Track(track))
    );
    assert_eq!(sf_state.inspection.mode, InspectionMode::Pinned);
    // Explore history is session-only — never persisted.
    assert!(sf_state.inspection.explore_back.is_empty());
    // The unrelated view carries no Signal Flow state.
    assert!(ws2.state(plain).unwrap().signal_flow.is_none());
    let _ = bass_strip;
}

#[test]
fn snapshot_v2_without_signal_flow_migrates() {
    let (mut ws, _track, _kick, _bass_strip, _kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let _sf = spawn(&mut ws, ViewType::SignalFlow, "sf", win);
    let snap = ws.capture().unwrap();
    let mut value = serde_json::to_value(&snap).unwrap();
    value["schema"] = serde_json::json!(2);
    // Strip every view's signal_flow — the v2 shape.
    for view in value["views"].as_array_mut().unwrap() {
        view["state"].as_object_mut().unwrap().remove("signal_flow");
    }
    let parsed = WorkspaceSnapshot::from_json(&value.to_string()).unwrap();
    assert_eq!(parsed.schema, synvelis_workspace::WORKSPACE_SCHEMA);
    assert!(
        parsed
            .views
            .iter()
            .all(|v| v.state.signal_flow.is_none()),
        "v2 views default to no Signal Flow state"
    );

    // And the migrated snapshot restores cleanly.
    let mut ws2 = fresh_ws();
    ws2.restore(&parsed, &[]).unwrap();
}

#[test]
fn snapshot_malformed_signal_flow_degrades_to_none() {
    let (mut ws, _track, _kick, _bass_strip, _kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let sf = spawn(&mut ws, ViewType::SignalFlow, "sf", win);
    ws.signal_flow_inspect(sf, SignalFlowEntityRef::Track(TrackId::new(1)))
        .unwrap();

    let snap = ws.capture().unwrap();
    let mut value = serde_json::to_value(&snap).unwrap();
    // Corrupt only the signal_flow payload of the first view — the
    // rest of the snapshot must survive.
    value["views"][0]["state"]["signal_flow"] = serde_json::json!({"bogus": true});
    let parsed = WorkspaceSnapshot::from_json(&value.to_string()).unwrap();
    let record = parsed
        .views
        .iter()
        .find(|v| v.id == sf.get())
        .expect("view record");
    assert!(record.state.signal_flow.is_none());
    // Untouched fields of the same ViewState still restored.
    assert_eq!(record.state.viewport.zoom_x, 1.0);
}

#[test]
fn snapshot_rejects_unreadable_schemas() {
    let (mut ws, _track, _kick, _bass_strip, _kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let _sf = spawn(&mut ws, ViewType::SignalFlow, "sf", win);
    let snap = ws.capture().unwrap();
    let json = snap.to_json().unwrap();

    for schema in [0u32, 1, 4, 99] {
        let mut value = serde_json::to_value(&snap).unwrap();
        value["schema"] = serde_json::json!(schema);
        assert!(
            WorkspaceSnapshot::from_json(&value.to_string()).is_err(),
            "schema {schema} must be rejected"
        );
    }
    // Sanity: the untouched snapshot parses.
    assert!(WorkspaceSnapshot::from_json(&json).is_ok());
}

#[test]
fn signal_flow_state_stays_view_local_across_views() {
    let (mut ws, track, _kick, _bass_strip, _kick_strip) = routed_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let sf_a = spawn(&mut ws, ViewType::SignalFlow, "sf-a", win);
    let sf_b = spawn(&mut ws, ViewType::SignalFlow, "sf-b", win);

    ws.signal_flow_inspect(sf_a, SignalFlowEntityRef::Track(track))
        .unwrap();
    assert!(ws.state(sf_b).unwrap().signal_flow.is_none());
    // B gets its own default state on first use — A's target does
    // not leak.
    ws.signal_flow_set_inspection_mode(sf_b, InspectionMode::Pinned)
        .unwrap();
    assert_eq!(
        ws.state(sf_b)
            .unwrap()
            .signal_flow
            .as_ref()
            .unwrap()
            .inspection
            .mode,
        InspectionMode::Pinned
    );
    assert_eq!(
        ws.state(sf_a)
            .unwrap()
            .signal_flow
            .as_ref()
            .unwrap()
            .inspection
            .mode,
        InspectionMode::Explore
    );
}

#[test]
fn inspection_focus_type_roundtrips_through_snapshot() {
    // The persisted SignalFlowInspectionFocus keeps target+mode+
    // follow_source; explore stacks stay session-only.
    let focus = SignalFlowInspectionFocus {
        target: Some(SignalFlowEntityRef::Track(TrackId::new(7))),
        mode: InspectionMode::Follow,
        follow_source: Some(FollowSource::Group(LinkGroupId(3))),
        explore_back: vec![SignalFlowEntityRef::Track(TrackId::new(8))],
        explore_fwd: Vec::new(),
    };
    let json = serde_json::to_string(&focus).unwrap();
    let back: SignalFlowInspectionFocus = serde_json::from_str(&json).unwrap();
    assert_eq!(back.target, focus.target);
    assert_eq!(back.mode, focus.mode);
    assert_eq!(back.follow_source, focus.follow_source);
    assert!(back.explore_back.is_empty());
}
