//! UI-C5-02/03 acceptance fixtures — Synvelis #102 slice 2
//! (`docs/test-specs/pc-c5-02-03-gesture-stale-inspector.md`).
//!
//! Covers: Control Layer edge gesture Begin/Preview/Commit/Cancel with
//! stale drop (window epoch + project revision CAS), inspect ≠ edit,
//! Inspector Follow/Explore/Pinned with follow-source scoping and
//! explore history, multi-selection preservation, and two-window /
//! dock / focus changes never retargeting a running gesture.
//!
//! Deterministic: `CounterIdGenerator` ids and `FakeWindowHost`.

use ruviform_ui::dock::{DockTarget, ViewInstanceId};
use ruviform_ui::window::MonitorInfo;
use synvelis_project::{
    AutomationLaneId, AutomationTimeScope, ClipExtent, ClipId, ClipPlacement, ClipPlayback,
    ControlEntityRef, ControlOwner, CounterIdGenerator, LaneId, ModulationAssignmentId,
    ModulationSourceId, MusicalDuration, MusicalTick, ParamMapping, ParameterRole,
    ParameterTargetId, PlaybackMapping, Polarity, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, SourceFrame, SourceKind, SourceRange, TimeAnchor, TrackId, Transform,
    ValueKind, is_issuable_raw,
};
use synvelis_workspace::{
    CancelReason, ControlEdgeOutcome, ControlEdgePreview, EditOperation, EditTimeDomain,
    EditTimePoint, EditingSelection, FakeWindowHost, FollowSource, Gesture, GestureFixation,
    GesturePreview, InspectionMode, LinkGroupId, ProjectSession, SemanticTarget, ViewType,
    WorkspaceError, WorkspaceManager, WorkspaceSnapshot, control_entity_tag,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xC503), "uic5-test");
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

fn window(ws: &mut Ws, title: &str) -> ruviform_ui::window::WindowId {
    ws.create_window(title, 1280, 800).unwrap()
}

fn view(
    ws: &mut Ws,
    kind: ViewType,
    title: &str,
    win: ruviform_ui::window::WindowId,
) -> ViewInstanceId {
    ws.spawn_view(kind, title, DockTarget::Window(win)).unwrap()
}

/// A project-seeded control pair: one issuable Continuous parameter
/// target and one modulation source — the endpoints a Control Layer
/// edge gesture connects.
fn control_pair(ws: &mut Ws) -> (ParameterTargetId, ModulationSourceId) {
    apply(
        ws,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let target = *ws
        .session()
        .project()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .expect("param target");
    apply(
        ws,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Project,
            kind: SourceKind::Macro { value: 0.25 },
        },
    );
    let source = *ws
        .session()
        .project()
        .control
        .sources
        .keys()
        .next_back()
        .expect("modulation source");
    (target, source)
}

/// track + lane + source + clip — the semantic targets inspection
/// and link fixtures navigate.
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
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_000_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    let clip_id = *ws.session().project().clips.keys().next().unwrap();
    (ws, track_id, lane_id, clip_id)
}

fn second_track(ws: &mut Ws) -> TrackId {
    apply(
        ws,
        ProjectOp::AddTrack {
            name: "t2".to_string(),
        },
    );
    *ws.session().project().tracks.keys().next_back().unwrap()
}

// ---------------------------------------------------------------------------
// UI-C5-02 — Control Layer edge gesture lifecycle
// ---------------------------------------------------------------------------

/// Begin fixes typed endpoints + revision + window epoch; Preview is
/// non-mutating; Commit applies exactly one assignment (one undo
/// boundary); a second commit finds no gesture.
#[test]
fn uic5_02_edge_begin_preview_commit_undo() {
    let mut ws = fresh_ws();
    let (target, source) = control_pair(&mut ws);
    let win = window(&mut ws, "A");
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win);

    let rev0 = ws.session().revision();
    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(source),
        ControlEntityRef::Parameter(target),
    )
    .unwrap();
    let g = ws.state(sf).unwrap().control_edge_gesture.as_ref().unwrap();
    assert_eq!(g.fixation.source, source);
    assert_eq!(g.fixation.target, target);
    assert_eq!(g.fixation.revision, rev0);
    assert_eq!(g.fixation.window, Some(win));

    // Preview mutates only the in-memory preview — revision, inbox
    // and project are untouched.
    ws.control_edge_preview(
        sf,
        ControlEdgePreview {
            depth: 0.5,
            polarity: Polarity::Unipolar,
        },
    )
    .unwrap();
    assert_eq!(ws.session().revision(), rev0);
    assert_eq!(ws.state(sf).unwrap().revision_inbox.len(), 0);
    assert_eq!(ws.session().project().control.assignments.len(), 0);

    // Commit applies one AddModulationAssignment and distributes.
    let out = ws.control_edge_commit(sf).unwrap();
    let ControlEdgeOutcome::Applied { revision } = out else {
        panic!("expected Applied, got {out:?}");
    };
    assert!(revision > rev0);
    assert_eq!(ws.session().revision(), revision);
    assert_eq!(ws.session().project().control.assignments.len(), 1);
    let assignment = ws
        .session()
        .project()
        .control
        .assignments
        .values()
        .next()
        .unwrap();
    assert_eq!(assignment.source, source);
    assert_eq!(assignment.target, target);
    assert_eq!(assignment.depth, 0.5);
    assert_eq!(assignment.polarity, Polarity::Unipolar);
    assert!(assignment.enabled);
    assert_eq!(ws.state(sf).unwrap().revision_inbox, vec![revision]);
    assert!(ws.state(sf).unwrap().control_edge_gesture.is_none());

    // Exactly one undo boundary: one undo removes the assignment,
    // the next reaches the fixture's own last op.
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(ws.session().project().control.assignments.len(), 0);
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(ws.session().project().control.sources.len(), 0);

    // The gesture is gone — a second commit is NoGesture.
    assert!(matches!(
        ws.control_edge_commit(sf),
        Err(WorkspaceError::NoGesture(_))
    ));
}

/// Begin rejects non connect endpoint kinds; a second begin while
/// one runs is [`GestureActive`].
#[test]
fn uic5_02_edge_begin_validates_endpoints_and_exclusivity() {
    let mut ws = fresh_ws();
    let (target, source) = control_pair(&mut ws);
    let win = window(&mut ws, "A");
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win);

    // Parameter -> Parameter is not a connect gesture.
    assert!(matches!(
        ws.control_edge_begin(
            sf,
            ControlEntityRef::Parameter(target),
            ControlEntityRef::Parameter(target),
        ),
        Err(WorkspaceError::InvalidGesture(_))
    ));
    // AutomationLane is not a modulation source.
    assert!(matches!(
        ws.control_edge_begin(
            sf,
            ControlEntityRef::AutomationLane(synvelis_project::AutomationLaneId::new(1)),
            ControlEntityRef::Parameter(target),
        ),
        Err(WorkspaceError::InvalidGesture(_))
    ));

    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(source),
        ControlEntityRef::Parameter(target),
    )
    .unwrap();
    assert!(matches!(
        ws.control_edge_begin(
            sf,
            ControlEntityRef::ModulationSource(source),
            ControlEntityRef::Parameter(target),
        ),
        Err(WorkspaceError::GestureActive(_))
    ));
}

/// Preview and Cancel never touch the project: revision, undo
/// history and assignment set are all unchanged.
#[test]
fn uic5_02_preview_and_cancel_leave_project_untouched() {
    let mut ws = fresh_ws();
    let (target, source) = control_pair(&mut ws);
    let win = window(&mut ws, "A");
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win);
    let rev0 = ws.session().revision();

    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(source),
        ControlEntityRef::Parameter(target),
    )
    .unwrap();
    ws.control_edge_preview(
        sf,
        ControlEdgePreview {
            depth: -0.75,
            polarity: Polarity::Bipolar,
        },
    )
    .unwrap();
    assert_eq!(ws.session().revision(), rev0);

    let out = ws.control_edge_cancel(sf, CancelReason::Escape).unwrap();
    assert!(matches!(out, ControlEdgeOutcome::Cancelled { .. }));
    assert_eq!(ws.session().revision(), rev0);
    assert_eq!(ws.session().project().control.assignments.len(), 0);
    assert!(ws.state(sf).unwrap().control_edge_gesture.is_none());
    assert_eq!(ws.state(sf).unwrap().revision_inbox.len(), 0);

    // The next undo reaches the fixture's own last op — the gesture
    // contributed no boundary.
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(ws.session().project().control.sources.len(), 0);
}

/// A dock move between Begin and Commit changes the window epoch —
/// the commit drops as Stale instead of landing on the moved view.
/// A mid-transfer commit is likewise stale; abort restores commit.
#[test]
fn uic5_02_commit_drops_on_window_epoch_change() {
    let mut ws = fresh_ws();
    let (target, source) = control_pair(&mut ws);
    let win_a = window(&mut ws, "A");
    let win_b = window(&mut ws, "B");
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win_a);

    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(source),
        ControlEntityRef::Parameter(target),
    )
    .unwrap();
    ws.move_view(sf, DockTarget::Window(win_b)).unwrap();
    assert_eq!(ws.dock().owner_of(sf), Some(win_b));

    let out = ws.control_edge_commit(sf).unwrap();
    assert_eq!(out, ControlEdgeOutcome::Stale);
    assert_eq!(ws.session().project().control.assignments.len(), 0);
    assert!(ws.state(sf).unwrap().control_edge_gesture.is_none());

    // Mid-transfer: begin again, start a move, commit while the
    // transfer token is outstanding → stale; abort keeps the view.
    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(source),
        ControlEntityRef::Parameter(target),
    )
    .unwrap();
    let token = ws.begin_transfer(sf, DockTarget::Window(win_a)).unwrap();
    let out = ws.control_edge_commit(sf).unwrap();
    assert_eq!(out, ControlEdgeOutcome::Stale);
    ws.abort_transfer(token).unwrap();
    assert_eq!(ws.dock().owner_of(sf), Some(win_b));
    assert_eq!(ws.session().project().control.assignments.len(), 0);
}

/// Any project change between Begin and Commit advances the live
/// revision past the fixated one — the commit drops as Stale rather
/// than applying against a revision it never saw.
#[test]
fn uic5_02_commit_drops_on_stale_revision() {
    let mut ws = fresh_ws();
    let (target, source) = control_pair(&mut ws);
    let win = window(&mut ws, "A");
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win);

    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(source),
        ControlEntityRef::Parameter(target),
    )
    .unwrap();
    // A foreign project change (undo of a fixture op would also do;
    // a direct fixture apply is the simplest revision advance).
    apply(
        &mut ws,
        ProjectOp::SetProjectName {
            name: "moved".into(),
        },
    );

    let out = ws.control_edge_commit(sf).unwrap();
    assert_eq!(out, ControlEdgeOutcome::Stale);
    assert_eq!(ws.session().project().control.assignments.len(), 0);
    assert!(ws.state(sf).unwrap().control_edge_gesture.is_none());
}

/// Focus moves, window activation changes and peer selections during
/// a gesture never retarget it: commit still applies the fixated
/// endpoints.
#[test]
fn uic5_02_focus_selection_change_never_retargets() {
    let (mut ws, track, _lane, clip) = seeded();
    let (target, source) = control_pair(&mut ws);
    let win_a = window(&mut ws, "A");
    let win_b = window(&mut ws, "B");
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win_a);
    let arr = view(&mut ws, ViewType::Arrangement, "arr", win_b);

    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(source),
        ControlEntityRef::Parameter(target),
    )
    .unwrap();

    // Focus moves to the other window; the peer view changes its
    // per-domain selections — neither may retarget the gesture.
    ws.focus_mut().native_window = Some(win_b);
    ws.focus_mut().active_view = Some(arr);
    ws.state_mut(arr)
        .unwrap()
        .object_selections
        .set(
            synvelis_workspace::SelectionDomain::Tracks,
            EditingSelection::single(SemanticTarget::Track(track)),
        )
        .unwrap();
    ws.state_mut(arr)
        .unwrap()
        .object_selections
        .set(
            synvelis_workspace::SelectionDomain::Clips,
            EditingSelection::single(SemanticTarget::Clip(clip)),
        )
        .unwrap();

    let out = ws.control_edge_commit(sf).unwrap();
    assert!(matches!(out, ControlEdgeOutcome::Applied { .. }));
    let assignment = ws
        .session()
        .project()
        .control
        .assignments
        .values()
        .next()
        .unwrap();
    assert_eq!(assignment.source, source);
    assert_eq!(assignment.target, target);
    // Gesture view's own selection stayed empty — the gesture never
    // became an editing selection.
    assert_eq!(ws.state(sf).unwrap().object_selections.selections.len(), 0);
}

/// An inspect navigation changes only `inspection_focus` — never the
/// editing selection, project revision, revision inbox or undo
/// history. Inspect click ≠ Edit (UI-C5-02).
#[test]
fn uic5_02_inspect_is_not_edit() {
    let (mut ws, track, _lane, clip) = seeded();
    let win = window(&mut ws, "A");
    let insp = view(&mut ws, ViewType::Inspector, "insp", win);
    let arr = view(&mut ws, ViewType::Arrangement, "arr", win);

    // Give the inspector a multi-target editing selection first —
    // inspection must preserve it exactly.
    let t2 = second_track(&mut ws);
    ws.state_mut(insp)
        .unwrap()
        .object_selections
        .set(
            synvelis_workspace::SelectionDomain::Tracks,
            EditingSelection {
                targets: vec![SemanticTarget::Track(track), SemanticTarget::Track(t2)],
            },
        )
        .unwrap();

    let rev0 = ws.session().revision();
    let inbox0 = ws.state(insp).unwrap().revision_inbox.len();
    ws.inspect(insp, SemanticTarget::Clip(clip)).unwrap();
    ws.inspect(insp, SemanticTarget::Track(track)).unwrap();

    let st = ws.state(insp).unwrap();
    assert_eq!(st.inspection_focus.mode, InspectionMode::Explore);
    assert_eq!(
        st.inspection_focus.target,
        Some(SemanticTarget::Track(track))
    );
    // Editing selection: both targets, order preserved.
    assert_eq!(
        st.object_selections
            .selection(synvelis_workspace::SelectionDomain::Tracks)
            .unwrap()
            .targets,
        vec![SemanticTarget::Track(track), SemanticTarget::Track(t2)]
    );
    // No project mutation, no revision broadcast, no undo boundary:
    // the next undo still reaches the fixture's own last op (t2's
    // AddTrack), proving inspect contributed zero history.
    assert_eq!(ws.session().revision(), rev0);
    assert_eq!(ws.state(insp).unwrap().revision_inbox.len(), inbox0);
    assert!(ws.session_mut().undo().unwrap());
    assert!(!ws.session().project().tracks.contains_key(&t2));
    assert!(ws.session_mut().undo().unwrap());
    assert!(!ws.session().project().clips.contains_key(&clip));

    // The arrangement view was untouched too.
    assert!(ws.state(arr).unwrap().inspection_focus.target.is_none());
    assert_eq!(ws.state(arr).unwrap().object_selections.selections.len(), 0);
}

/// A running Audio Routing gesture blocks a Control Layer edge
/// begin on the same view ([`GestureActive`]), while a different view's
/// edge gesture proceeds independently — the two gesture families
/// cannot be confused or cross-commit.
#[test]
fn uic5_02_edge_and_routing_gestures_are_distinct() {
    let (mut ws, _track, _lane, clip_id) = seeded();
    let (target, source) = control_pair(&mut ws);
    let win_a = window(&mut ws, "A");
    let win_b = window(&mut ws, "B");
    let arr = view(&mut ws, ViewType::Arrangement, "arr", win_a);
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win_b);

    // Inject an Audio Routing gesture fixation directly — the same
    // shape pointer_down produces.
    let rev = ws.session().revision();
    ws.state_mut(arr).unwrap().gesture = Some(Gesture {
        fixation: GestureFixation {
            operation: EditOperation::MoveClips,
            targets: vec![SemanticTarget::Clip(clip_id)],
            revision: rev,
            domain: EditTimeDomain::Musical,
            origin: EditTimePoint::Musical(MusicalTick::new(0)),
        },
        preview: GesturePreview {
            position: EditTimePoint::Musical(MusicalTick::new(0)),
            delta: 0,
            snap: None,
        },
    });

    // Same view: edge begin is refused while a routing gesture runs.
    let arr_sf = view(&mut ws, ViewType::SignalFlow, "sf2", win_a);
    let routing = ws.state(arr).unwrap().gesture.clone();
    ws.state_mut(arr_sf).unwrap().gesture = routing;
    assert!(matches!(
        ws.control_edge_begin(
            arr_sf,
            ControlEntityRef::ModulationSource(source),
            ControlEntityRef::Parameter(target),
        ),
        Err(WorkspaceError::GestureActive(_))
    ));

    // Different view: the edge gesture proceeds and commits on its
    // own endpoints; the routing gesture is untouched.
    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(source),
        ControlEntityRef::Parameter(target),
    )
    .unwrap();
    let out = ws.control_edge_commit(sf).unwrap();
    assert!(matches!(out, ControlEdgeOutcome::Applied { .. }));
    assert!(ws.state(arr).unwrap().gesture.is_some());
    assert_eq!(ws.session().project().control.assignments.len(), 1);
}

// ---------------------------------------------------------------------------
// UI-C5-03 — Inspector modes, follow source, explore history
// ---------------------------------------------------------------------------

/// Follow adopts delivered targets only when the follow source
/// matches; Explore and Pinned never retarget; the editing selection
/// is never touched by any of it.
#[test]
fn uic5_03_follow_source_scoping_and_modes() {
    let (mut ws, track, lane, clip) = seeded();
    let t2 = second_track(&mut ws);
    let win = window(&mut ws, "A");
    let arr = view(&mut ws, ViewType::Arrangement, "arr", win);
    let mix = view(&mut ws, ViewType::Mixer, "mix", win);
    let insp = view(&mut ws, ViewType::Inspector, "insp", win);
    let pinned = view(&mut ws, ViewType::Inspector, "pinned", win);

    for v in [arr, mix, insp, pinned] {
        ws.links_mut().set_group(v, Some(LinkGroupId(1)));
    }
    ws.state_mut(pinned).unwrap().inspection_focus.mode = InspectionMode::Pinned;
    ws.state_mut(pinned).unwrap().inspection_focus.target = Some(SemanticTarget::Lane(lane));

    // Baseline Follow (follow_source None) adopts the mapped target.
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(SemanticTarget::Track(track))
    );

    // Narrow Follow to a different publisher: arr's events no longer
    // retarget insp, though linked_context still records them.
    ws.set_follow_source(insp, Some(FollowSource::View(mix)))
        .unwrap();
    ws.select(arr, EditingSelection::single(SemanticTarget::Track(t2)))
        .unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(SemanticTarget::Track(track)),
        "follow_source View(mix) must ignore arr's events"
    );
    assert_eq!(
        ws.state(insp).unwrap().linked_context.target,
        Some(SemanticTarget::Track(t2)),
        "linked_context still records the delivery"
    );

    // Events from the configured source do retarget — to a distinct
    // target so adoption is observable.
    ws.select(mix, EditingSelection::single(SemanticTarget::Track(t2)))
        .unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(SemanticTarget::Track(t2))
    );

    // Group-scoped follow source matches any publisher in the group.
    ws.set_follow_source(insp, Some(FollowSource::Group(LinkGroupId(1))))
        .unwrap();
    ws.select(arr, EditingSelection::single(SemanticTarget::Track(track)))
        .unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(SemanticTarget::Track(track))
    );

    // Explore: link deliveries update linked_context but never the
    // inspected target.
    ws.inspect(insp, SemanticTarget::Clip(clip)).unwrap();
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(SemanticTarget::Clip(clip)),
        "Explore must not retarget on link events"
    );

    // Pinned keeps its target regardless.
    ws.set_inspection_mode(insp, InspectionMode::Pinned)
        .unwrap();
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(SemanticTarget::Clip(clip))
    );

    // No inspector mode ever wrote an editing selection.
    for v in [insp, pinned] {
        assert_eq!(ws.state(v).unwrap().object_selections.selections.len(), 0);
    }
    // pinned inspector kept its own lane target throughout.
    assert_eq!(
        ws.state(pinned).unwrap().inspection_focus.target,
        Some(SemanticTarget::Lane(lane))
    );
}

/// Explore back/forward walks only the Inspector's own history;
/// leaving Explore clears it; the editing selection is untouched.
#[test]
fn uic5_03_explore_history_back_forward() {
    let (mut ws, track, _lane, clip) = seeded();
    let t2 = second_track(&mut ws);
    let t3 = second_track(&mut ws);
    let win = window(&mut ws, "A");
    let insp = view(&mut ws, ViewType::Inspector, "insp", win);

    let targets = [
        SemanticTarget::Track(track),
        SemanticTarget::Track(t2),
        SemanticTarget::Track(t3),
        SemanticTarget::Clip(clip),
    ];
    for t in targets {
        ws.inspect(insp, t).unwrap();
    }
    let st = ws.state(insp).unwrap();
    assert_eq!(st.inspection_focus.target, Some(targets[3]));
    assert_eq!(
        st.inspection_focus.explore_back,
        vec![targets[0], targets[1], targets[2]]
    );
    assert_eq!(st.inspection_focus.explore_fwd.len(), 0);

    // Back walks the stack; forward mirrors it.
    ws.explore_back(insp).unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(targets[2])
    );
    ws.explore_back(insp).unwrap();
    ws.explore_forward(insp).unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(targets[2])
    );
    ws.explore_forward(insp).unwrap();
    ws.explore_forward(insp).unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(targets[3])
    );

    // Back/forward are no-ops outside Explore.
    ws.set_inspection_mode(insp, InspectionMode::Follow)
        .unwrap();
    ws.explore_back(insp).unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(targets[3])
    );

    // Leaving Explore cleared the history — re-entering starts empty.
    ws.set_inspection_mode(insp, InspectionMode::Explore)
        .unwrap();
    ws.explore_back(insp).unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(targets[3]),
        "history must not survive a mode exit"
    );
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.explore_back.len(),
        0
    );
}

/// A Pinned target that disappears from the project stays pinned —
/// the focus keeps reporting the (now unresolved) target instead of
/// silently substituting another one.
#[test]
fn uic5_03_pinned_target_stays_unresolved_after_delete() {
    let (mut ws, _track, _lane, clip) = seeded();
    let win = window(&mut ws, "A");
    let insp = view(&mut ws, ViewType::Inspector, "insp", win);

    ws.inspect(insp, SemanticTarget::Clip(clip)).unwrap();
    ws.set_inspection_mode(insp, InspectionMode::Pinned)
        .unwrap();
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(SemanticTarget::Clip(clip))
    );

    apply(&mut ws, ProjectOp::RemoveClip { clip_id: clip });
    assert!(!ws.session().project().clips.contains_key(&clip));

    // Still pinned to the deleted target — no substitution.
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.target,
        Some(SemanticTarget::Clip(clip))
    );
    assert_eq!(
        ws.state(insp).unwrap().inspection_focus.mode,
        InspectionMode::Pinned
    );
}

/// Two-window + dock + focus churn during an edge gesture never
/// retargets it — commit still lands on the fixated endpoints even
/// after the owning view moved windows *before* Begin's epoch check
/// would matter (i.e. move happens, then a *fresh* begin+commit on
/// the new epoch works; the old gesture was already dropped).
#[test]
fn uic5_03_two_window_dock_focus_no_retarget() {
    let (mut ws, track, _lane, _clip) = seeded();
    let (target, source) = control_pair(&mut ws);
    let win_a = window(&mut ws, "A");
    let win_b = window(&mut ws, "B");
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win_a);
    let insp_a = view(&mut ws, ViewType::Inspector, "insp-a", win_a);
    let insp_b = view(&mut ws, ViewType::Inspector, "insp-b", win_b);

    // Gesture begins in window A.
    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(source),
        ControlEntityRef::Parameter(target),
    )
    .unwrap();

    // Focus churn + inspector navigation on BOTH windows mid-gesture.
    ws.focus_mut().native_window = Some(win_b);
    ws.focus_mut().active_view = Some(insp_b);
    ws.inspect(insp_a, SemanticTarget::Track(track)).unwrap();
    ws.inspect(insp_b, SemanticTarget::Track(track)).unwrap();

    // Dock transfer of the gesture view mid-gesture → commit stale.
    ws.move_view(sf, DockTarget::Window(win_b)).unwrap();
    let out = ws.control_edge_commit(sf).unwrap();
    assert_eq!(out, ControlEdgeOutcome::Stale);
    assert_eq!(ws.session().project().control.assignments.len(), 0);

    // Inspector state on both windows stayed independent — each
    // kept its own Explore target and history.
    assert_eq!(
        ws.state(insp_a).unwrap().inspection_focus.target,
        Some(SemanticTarget::Track(track))
    );
    assert_eq!(
        ws.state(insp_b).unwrap().inspection_focus.target,
        Some(SemanticTarget::Track(track))
    );
    assert_eq!(ws.dock().owner_of(insp_a), Some(win_a));
    assert_eq!(ws.dock().owner_of(insp_b), Some(win_b));
}

// ---------------------------------------------------------------------------
// UI-C5 Control Relation inspection focus (synvelis#102 integration slice)
// ---------------------------------------------------------------------------

/// Automation lane + assignment over the `control_pair` endpoints —
/// the relation neighborhood `control_relation_inspect` anchors on.
fn control_lane_assignment(
    ws: &mut Ws,
    target: ParameterTargetId,
    source: ModulationSourceId,
) -> (AutomationLaneId, ModulationAssignmentId) {
    apply(ws, ProjectOp::AddAutomationLane { target });
    let lane = *ws
        .session()
        .project()
        .control
        .lanes
        .keys()
        .next_back()
        .expect("automation lane");
    apply(
        ws,
        ProjectOp::AddModulationAssignment {
            source,
            target,
            depth: 0.5,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    let assignment = *ws
        .session()
        .project()
        .control
        .assignments
        .keys()
        .next_back()
        .expect("modulation assignment");
    (lane, assignment)
}

/// Inspect enters Explore and pushes the previous anchor; back /
/// forward walk the history; leaving Explore clears it. Editing
/// selection, `inspection_focus`, project revision and undo depth
/// are never touched — inspect ≠ edit (UI-C5-01/02/03).
#[test]
fn uic5_control_relation_inspect_explore_back_and_modes() {
    let (mut ws, _track, _lane, _clip) = seeded();
    let (target, source) = control_pair(&mut ws);
    let (_lane, assignment) = control_lane_assignment(&mut ws, target, source);
    let win = window(&mut ws, "A");
    let view = view(&mut ws, ViewType::SignalFlow, "sf", win);
    let param = ControlEntityRef::Parameter(target);
    let assign = ControlEntityRef::Assignment(assignment);

    let rev_before = ws.session().revision();
    let undo_before = ws.session().controller().history_states().count();
    let sel_before = ws.state(view).unwrap().object_selections.clone();
    let insp_before = ws.state(view).unwrap().inspection_focus.clone();

    // First inspect: Explore with no back-stack.
    ws.control_relation_inspect(view, param).unwrap();
    {
        let focus = &ws.control_relation_state(view).unwrap().inspection;
        assert_eq!(focus.mode, InspectionMode::Explore);
        assert_eq!(
            focus.target.as_deref(),
            Some(control_entity_tag(param).as_str())
        );
        assert_eq!(focus.explore_back, [] as [ControlEntityRef; 0]);
        assert_eq!(focus.explore_fwd, [] as [ControlEntityRef; 0]);
    }

    // Descend to the assignment — the parameter anchor is pushed.
    ws.control_relation_inspect(view, assign).unwrap();
    {
        let focus = &ws.control_relation_state(view).unwrap().inspection;
        assert_eq!(focus.explore_back, vec![param]);
        assert_eq!(focus.explore_fwd, [] as [ControlEntityRef; 0]);
    }

    // Back restores the parameter; forward re-descends.
    ws.control_relation_explore_back(view).unwrap();
    assert_eq!(
        ws.control_relation_state(view)
            .unwrap()
            .inspection
            .target
            .as_deref(),
        Some(control_entity_tag(param).as_str())
    );
    ws.control_relation_explore_forward(view).unwrap();
    assert_eq!(
        ws.control_relation_state(view)
            .unwrap()
            .inspection
            .target
            .as_deref(),
        Some(control_entity_tag(assign).as_str())
    );

    // Re-inspecting the current anchor is a no-op on the stacks.
    ws.control_relation_inspect(view, assign).unwrap();
    assert_eq!(
        ws.control_relation_state(view)
            .unwrap()
            .inspection
            .explore_back,
        vec![param]
    );

    // Leaving Explore clears history; Pinned keeps the anchor.
    ws.control_relation_set_inspection_mode(view, InspectionMode::Pinned)
        .unwrap();
    {
        let focus = &ws.control_relation_state(view).unwrap().inspection;
        assert_eq!(focus.mode, InspectionMode::Pinned);
        assert_eq!(focus.explore_back, [] as [ControlEntityRef; 0]);
        assert_eq!(focus.explore_fwd, [] as [ControlEntityRef; 0]);
        assert_eq!(
            focus.target.as_deref(),
            Some(control_entity_tag(assign).as_str())
        );
    }

    // Follow-mode source filter round-trips.
    ws.control_relation_set_inspection_mode(view, InspectionMode::Follow)
        .unwrap();
    ws.control_relation_set_follow_source(view, Some(FollowSource::View(view)))
        .unwrap();
    assert_eq!(
        ws.control_relation_state(view)
            .unwrap()
            .inspection
            .follow_source,
        Some(FollowSource::View(view))
    );
    ws.control_relation_set_follow_source(view, None).unwrap();
    assert_eq!(
        ws.control_relation_state(view)
            .unwrap()
            .inspection
            .follow_source,
        None
    );

    // Nothing above touched the editing surfaces or project truth.
    let after = ws.state(view).unwrap();
    assert_eq!(after.object_selections, sel_before);
    assert_eq!(after.inspection_focus, insp_before);
    assert_eq!(ws.session().revision(), rev_before);
    assert_eq!(
        ws.session().controller().history_states().count(),
        undo_before
    );
}

/// Snapshot round-trip: `target`/`mode`/`follow_source` persist; the
/// Explore stacks are session-only and come back empty. A malformed
/// `control_relation` payload degrades to `None` like the other
/// per-view surfaces.
#[test]
fn uic5_control_relation_snapshot_roundtrip_and_degrade() {
    let (mut ws, _track, _lane, _clip) = seeded();
    let (target, source) = control_pair(&mut ws);
    let (_lane, assignment) = control_lane_assignment(&mut ws, target, source);
    let win = window(&mut ws, "A");
    let view = view(&mut ws, ViewType::SignalFlow, "sf", win);
    let param = ControlEntityRef::Parameter(target);
    let assign = ControlEntityRef::Assignment(assignment);

    ws.control_relation_inspect(view, param).unwrap();
    ws.control_relation_inspect(view, assign).unwrap();
    ws.control_relation_set_follow_source(view, Some(FollowSource::Group(LinkGroupId(7))))
        .unwrap();

    let snap = ws.capture().unwrap();
    // The persisted contract is the JSON boundary — an in-memory
    // capture/restore keeps session state by design.
    let persisted = WorkspaceSnapshot::from_json(&snap.to_json().unwrap()).unwrap();
    let mut ws2 = fresh_ws();
    ws2.restore(&persisted, &[]).unwrap();

    let focus = &ws2
        .state(view)
        .unwrap()
        .control_relation
        .as_ref()
        .expect("restored control_relation state")
        .inspection;
    assert_eq!(focus.mode, InspectionMode::Explore);
    assert_eq!(
        focus.target.as_deref(),
        Some(control_entity_tag(assign).as_str())
    );
    assert_eq!(
        focus.follow_source,
        Some(FollowSource::Group(LinkGroupId(7)))
    );
    // Explore history is session-only — never persisted.
    assert_eq!(focus.explore_back, [] as [ControlEntityRef; 0]);
    assert_eq!(focus.explore_fwd, [] as [ControlEntityRef; 0]);

    // Malformed payload degrades to None; the rest of the snapshot
    // survives.
    let snap = ws.capture().unwrap();
    let mut value = serde_json::to_value(&snap).unwrap();
    value["views"][0]["state"]["control_relation"] = serde_json::json!({"bogus": true});
    let parsed = WorkspaceSnapshot::from_json(&value.to_string()).unwrap();
    let record = parsed
        .views
        .iter()
        .find(|v| v.id == view.get())
        .expect("view record");
    assert!(record.state.control_relation.is_none());
    assert_eq!(record.state.viewport.zoom_x, 1.0);
}
