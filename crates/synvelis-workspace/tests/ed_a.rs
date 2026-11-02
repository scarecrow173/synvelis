//! ED-A acceptance tests — `docs/test-specs/ed-a-e2e-editing-model.md`.
//!
//! The agreed editing-model contracts (issue #11/#38), verified
//! headlessly: deterministic `CounterIdGenerator` ids,
//! `FakeWindowHost`, and semantic `HitProbe`s — no pixels, no OS
//! input devices.

use ruviform_ui::dock::DockTarget;
use ruviform_ui::window::MonitorInfo;
use synvelis_project::{
    ClipId, CounterIdGenerator, LaneId, MediaSpan, MediaTimeValue, MusicalRange, MusicalTick,
    Project, ProjectController, ProjectId, ProjectOp, ProjectRevision, SourceFrame, SourceId,
    TimeAnchor, TrackId,
};
use synvelis_workspace::{
    CancelReason, CommandContext, CommandOutcome, EditOperation, EditingSelection, FakeWindowHost,
    GestureOutcome, HandleKind, HitCandidate, HitProbe, HitZone, InputContext, LaneScope,
    LinkGroupId, PointerOutcome, ProjectSession, SelectionDomain, SemanticTarget, SnapCandidate,
    SnapEnvironment, SnapSource, SnapUnit, TimeSelection, Tool, ViewType, WorkspaceError,
    WorkspaceManager,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xEDA), "ed-test");
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

/// Fixture-only mutation, bypassing the command router.
fn apply(ws: &mut Ws, op: ProjectOp) -> ProjectRevision {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op")
}

/// track + lane + source + musical clip `[0, 960)` on the lane.
fn seeded() -> (Ws, TrackId, LaneId, ClipId, SourceId) {
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
    let clip_id = add_musical_clip(&mut ws, lane_id, 0, 960);
    (ws, track_id, lane_id, clip_id, source_id)
}

fn add_musical_clip(ws: &mut Ws, lane_id: LaneId, start: i64, end: i64) -> ClipId {
    let source_id = *ws.session().project().sources.keys().next().unwrap();
    apply(
        ws,
        ProjectOp::AddClip {
            lane_id,
            source_id,
            anchor: TimeAnchor::Musical(
                MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap(),
            ),
            source_offset: SourceFrame::new(0).unwrap(),
        },
    );
    *ws.session().project().clips.keys().last().unwrap()
}

fn add_media_clip(ws: &mut Ws, lane_id: LaneId) -> ClipId {
    let source_id = *ws.session().project().sources.keys().next().unwrap();
    let at = |v| MediaTimeValue::new(v, 44_100, 1).unwrap();
    apply(
        ws,
        ProjectOp::AddClip {
            lane_id,
            source_id,
            anchor: TimeAnchor::Media(MediaSpan::try_new(at(0), at(44_100)).unwrap()),
            source_offset: SourceFrame::new(0).unwrap(),
        },
    );
    *ws.session().project().clips.keys().last().unwrap()
}

fn arrangement(ws: &mut Ws) -> ruviform_ui::dock::ViewInstanceId {
    let win = ws.create_window("arr-win", 1280, 800).unwrap();
    ws.spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win))
        .unwrap()
}

fn body_probe(pos: i64, target: SemanticTarget) -> HitProbe {
    HitProbe {
        position: MusicalTick::new(pos),
        candidates: vec![HitCandidate {
            zone: HitZone::Body,
            target: Some(target),
        }],
    }
}

fn musical_range(start: i64, end: i64) -> MusicalRange {
    MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap()
}

fn clip_anchor(ws: &Ws, clip: ClipId) -> TimeAnchor {
    ws.session().project().clips.get(&clip).unwrap().anchor
}

fn musical_bounds(ws: &Ws, clip: ClipId) -> (i64, i64) {
    match clip_anchor(ws, clip) {
        TimeAnchor::Musical(r) => (r.start().value(), r.end().value()),
        other @ TimeAnchor::Media(_) => panic!("clip {clip} is not musical: {other:?}"),
    }
}

fn ctx_from(id: &str, view: ruviform_ui::dock::ViewInstanceId) -> CommandContext {
    CommandContext::from_view(id, view)
}

fn dispatch(ws: &mut Ws, id: &str, view: ruviform_ui::dock::ViewInstanceId) -> CommandOutcome {
    ws.dispatch(ctx_from(id, view))
}

// ---------------------------------------------------------------------------
// ED-A-01 — Selection model separation
// ---------------------------------------------------------------------------

/// Per-view + per-domain selection independence, explicit primary,
/// time selection never implying object deletion, domain-gated
/// commands, explicit range delete.
#[test]
fn ed_a_01_selection_model_separation() {
    let (mut ws, track, lane, clip, _source) = seeded();
    let clip2 = add_musical_clip(&mut ws, lane, 960, 1920);
    let arr_a = arrangement(&mut ws);
    let arr_b = arrangement(&mut ws);

    // View A and view B keep independent editing state.
    ws.select(
        arr_a,
        EditingSelection {
            targets: vec![SemanticTarget::Clip(clip), SemanticTarget::Clip(clip2)],
        },
    )
    .unwrap();
    assert_eq!(
        ws.state(arr_a).unwrap().object_selections.primary(),
        Some(SemanticTarget::Clip(clip2)),
        "primary = last pushed target (gesture anchor)"
    );
    assert!(ws.state(arr_b).unwrap().object_selections.is_empty());

    // Per-domain independence: selecting in Tracks keeps the stored
    // Clips selection; switching back restores it.
    let st = ws.state_mut(arr_a).unwrap();
    st.object_selections
        .set(
            SelectionDomain::Tracks,
            EditingSelection::single(SemanticTarget::Track(track)),
        )
        .unwrap();
    assert_eq!(st.object_selections.active, SelectionDomain::Tracks);
    assert_eq!(
        st.object_selections
            .selection(SelectionDomain::Clips)
            .unwrap()
            .primary(),
        Some(SemanticTarget::Clip(clip2)),
    );

    // Mixed-domain selection is rejected, not silently split.
    let err = ws
        .state_mut(arr_a)
        .unwrap()
        .object_selections
        .assign(EditingSelection {
            targets: vec![SemanticTarget::Clip(clip), SemanticTarget::Track(track)],
        })
        .unwrap_err();
    assert!(matches!(err, WorkspaceError::MixedDomains(_)));

    // Domain-incompatible command: Tracks is active, so clip.remove
    // cannot reach the stored clip selection.
    let outcome = dispatch(&mut ws, "clip.remove", arr_a);
    assert!(
        matches!(outcome, CommandOutcome::Unresolved { .. }),
        "clips-domain command must not resolve through Tracks selection: {outcome:?}"
    );
    assert!(ws.session().project().clips.contains_key(&clip));

    // Switching back to Clips makes the same command resolve against
    // the surviving clip selection (primary = clip2).
    ws.state_mut(arr_a)
        .unwrap()
        .object_selections
        .activate(SelectionDomain::Clips);
    let outcome = dispatch(&mut ws, "clip.remove", arr_a);
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    assert!(!ws.session().project().clips.contains_key(&clip2));
    assert!(ws.session().project().clips.contains_key(&clip));
}

/// A time selection alone never triggers object deletion; the
/// explicit `clips.delete-range` command sweeps only scoped lanes'
/// overlapping musical clips.
#[test]
fn ed_a_01b_time_selection_and_range_delete() {
    let (mut ws, _track, lane, clip, _source) = seeded();
    let clip2 = add_musical_clip(&mut ws, lane, 960, 1920);
    let track_id = *ws.session().project().tracks.keys().next().unwrap();
    apply(&mut ws, ProjectOp::AddLane { track_id });
    let lane2 = *ws.session().project().lanes.keys().last().unwrap();
    let clip_other_lane = add_musical_clip(&mut ws, lane2, 0, 960);
    let media_clip = add_media_clip(&mut ws, lane);
    let arr = arrangement(&mut ws);

    // Time selection only — no object selection: clip.remove stays
    // unresolved; nothing is deleted implicitly.
    ws.set_time_selection(arr, Some(TimeSelection::Musical(musical_range(0, 960))))
        .unwrap();
    let outcome = dispatch(&mut ws, "clip.remove", arr);
    assert!(matches!(outcome, CommandOutcome::Unresolved { .. }));
    assert!(ws.session().project().clips.contains_key(&clip));

    // Object + time selection coexist; remove acts on objects only.
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip2)))
        .unwrap();
    let outcome = dispatch(&mut ws, "clip.remove", arr);
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    assert!(ws.session().project().clips.contains_key(&clip));
    assert!(!ws.session().project().clips.contains_key(&clip2));

    // Explicit range delete over lane scope [lane] removes the
    // overlapping musical clip only: other lane's clip and the
    // media-anchored clip (different time domain) survive.
    ws.set_lane_scope(arr, LaneScope::new(vec![lane])).unwrap();
    let outcome = dispatch(&mut ws, "clips.delete-range", arr);
    assert!(
        matches!(outcome, CommandOutcome::Applied { .. }),
        "delete-range outcome: {outcome:?}"
    );
    assert!(!ws.session().project().clips.contains_key(&clip));
    assert!(ws.session().project().clips.contains_key(&clip_other_lane));
    assert!(ws.session().project().clips.contains_key(&media_clip));
}

// ---------------------------------------------------------------------------
// ED-A-02 — Tool resolution + gesture fixation
// ---------------------------------------------------------------------------

/// Smart-tool zone priority, dedicated-tool explicitness, ambiguous
/// hits, and start-time fixation of operation/targets.
#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenarios
fn ed_a_02_tool_resolution_and_fixation() {
    let (mut ws, _track, lane, clip, _source) = seeded();
    let arr = arrangement(&mut ws);

    // Handle outranks body at the same position.
    let probe = HitProbe {
        position: MusicalTick::new(10),
        candidates: vec![
            HitCandidate {
                zone: HitZone::Body,
                target: Some(SemanticTarget::Clip(clip)),
            },
            HitCandidate {
                zone: HitZone::Handle(HandleKind::StartTrim),
                target: Some(SemanticTarget::Clip(clip)),
            },
        ],
    };
    assert_eq!(
        ws.pointer_down(arr, &probe).unwrap(),
        PointerOutcome::GestureStarted
    );
    assert_eq!(
        ws.state(arr)
            .unwrap()
            .gesture
            .as_ref()
            .unwrap()
            .fixation
            .operation,
        EditOperation::TrimClip {
            edge: HandleKind::StartTrim
        },
    );
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();

    // Body outranks lane-empty.
    let probe = HitProbe {
        position: MusicalTick::new(10),
        candidates: vec![
            HitCandidate {
                zone: HitZone::LaneEmpty,
                target: Some(SemanticTarget::Lane(lane)),
            },
            HitCandidate {
                zone: HitZone::Body,
                target: Some(SemanticTarget::Clip(clip)),
            },
        ],
    };
    ws.pointer_down(arr, &probe).unwrap();
    assert_eq!(
        ws.state(arr)
            .unwrap()
            .gesture
            .as_ref()
            .unwrap()
            .fixation
            .operation,
        EditOperation::MoveClips,
    );
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();

    // Two equally-ranked candidates resolve Ambiguous with suggested
    // dedicated tools — no gesture starts.
    let probe = HitProbe {
        position: MusicalTick::new(10),
        candidates: vec![
            HitCandidate {
                zone: HitZone::Handle(HandleKind::StartTrim),
                target: Some(SemanticTarget::Clip(clip)),
            },
            HitCandidate {
                zone: HitZone::Handle(HandleKind::FadeIn),
                target: Some(SemanticTarget::Clip(clip)),
            },
        ],
    };
    let PointerOutcome::Ambiguous {
        candidates,
        suggested,
    } = ws.pointer_down(arr, &probe).unwrap()
    else {
        panic!("expected Ambiguous");
    };
    assert!(candidates.contains(&EditOperation::TrimClip {
        edge: HandleKind::StartTrim
    }));
    assert!(candidates.contains(&EditOperation::FadeClip));
    assert!(suggested.contains(&Tool::Trim) && suggested.contains(&Tool::Fade));
    assert!(ws.state(arr).unwrap().gesture.is_none());

    // Dedicated tools resolve explicitly — Trim on a body hit is a
    // no-gesture (it never falls through the smart ladder).
    ws.set_tool(arr, Tool::Trim).unwrap();
    assert_eq!(
        ws.pointer_down(arr, &body_probe(10, SemanticTarget::Clip(clip)))
            .unwrap(),
        PointerOutcome::NoGesture,
    );
    ws.set_tool(arr, Tool::Select).unwrap();
    ws.pointer_down(arr, &body_probe(10, SemanticTarget::Clip(clip)))
        .unwrap();
    assert_eq!(
        ws.state(arr)
            .unwrap()
            .gesture
            .as_ref()
            .unwrap()
            .fixation
            .operation,
        EditOperation::Select,
    );
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();

    // Fixation: a move started on clip keeps its operation/targets
    // even when later pointer positions cross a handle zone and the
    // object selection changes mid-gesture.
    ws.set_tool(arr, Tool::Smart).unwrap();
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    ws.pointer_down(arr, &body_probe(10, SemanticTarget::Clip(clip)))
        .unwrap();
    // Mid-gesture selection change must not retarget.
    let clip2 = add_musical_clip(&mut ws, lane, 960, 1920);
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip2)))
        .unwrap();
    let fixation = &ws.state(arr).unwrap().gesture.as_ref().unwrap().fixation;
    assert_eq!(fixation.operation, EditOperation::MoveClips);
    assert_eq!(fixation.targets, vec![SemanticTarget::Clip(clip)]);
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();

    // Slip resolves but its commit semantics are honestly unsupported.
    ws.set_tool(arr, Tool::Slip).unwrap();
    ws.pointer_down(arr, &body_probe(10, SemanticTarget::Clip(clip)))
        .unwrap();
    let err = ws.commit_gesture(arr).unwrap_err();
    assert!(matches!(err, WorkspaceError::UnsupportedOperation("slip")));
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();
}

// ---------------------------------------------------------------------------
// ED-A-03 — Grid/snap separation + explicit time domain
// ---------------------------------------------------------------------------

/// Snap works with the grid hidden; explicit units ignore zoom, only
/// Auto follows it; the primary snaps while co-movers keep relative
/// offsets; mixed-domain moves refuse to start.
#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenarios
fn ed_a_03_snap_grid_time_domain() {
    let (mut ws, _track, lane, clip, _source) = seeded();
    let clip2 = add_musical_clip(&mut ws, lane, 960, 1920);
    let arr = arrangement(&mut ws);

    // Grid hidden, snap enabled — snapping still works.
    ws.state_mut(arr).unwrap().grid.visible = false;
    let env = SnapEnvironment::default(); // explicit 960 grid
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    ws.pointer_down(arr, &body_probe(0, SemanticTarget::Clip(clip)))
        .unwrap();
    ws.drag_update(arr, MusicalTick::new(1000), &env).unwrap();
    let preview = &ws.state(arr).unwrap().gesture.as_ref().unwrap().preview;
    assert_eq!(preview.delta, 960, "snapped to the 960 grid line");
    assert_eq!(preview.snap.unwrap().label(), "grid@960");

    // Explicit unit ignores viewport zoom; only Auto follows it.
    ws.state_mut(arr).unwrap().viewport.zoom_x = 8.0;
    ws.drag_update(arr, MusicalTick::new(1000), &env).unwrap();
    assert_eq!(
        ws.state(arr)
            .unwrap()
            .gesture
            .as_ref()
            .unwrap()
            .preview
            .delta,
        960,
        "explicit unit must not follow zoom"
    );
    ws.state_mut(arr).unwrap().snap.unit = SnapUnit::Auto;
    let env = SnapEnvironment {
        auto_step: MusicalTick::new(200), // zoom-derived step
        candidates: Vec::new(),
    };
    // leave hysteresis window first
    ws.drag_update(arr, MusicalTick::new(2000), &env).unwrap();
    ws.drag_update(arr, MusicalTick::new(1000), &env).unwrap();
    assert_eq!(
        ws.state(arr)
            .unwrap()
            .gesture
            .as_ref()
            .unwrap()
            .preview
            .delta,
        1000,
        "auto unit snaps to the zoom-derived step"
    );
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();

    // Non-grid candidates: clip bounds beat the grid when nearer;
    // the winner is reported with its source label.
    ws.state_mut(arr).unwrap().snap.unit = SnapUnit::Explicit(MusicalTick::new(960));
    let env = SnapEnvironment {
        auto_step: MusicalTick::new(960),
        candidates: vec![
            SnapCandidate {
                source: SnapSource::ClipBounds,
                position: MusicalTick::new(970),
                domain: synvelis_workspace::EditTimeDomain::Musical,
            },
            SnapCandidate {
                source: SnapSource::ClipBounds,
                position: MusicalTick::new(3000), // media-domain would be filtered
                domain: synvelis_workspace::EditTimeDomain::Media,
            },
        ],
    };
    ws.pointer_down(arr, &body_probe(0, SemanticTarget::Clip(clip)))
        .unwrap();
    ws.drag_update(arr, MusicalTick::new(1000), &env).unwrap();
    let preview = &ws.state(arr).unwrap().gesture.as_ref().unwrap().preview;
    assert_eq!(preview.snap.unwrap().label(), "clip-bounds@970");
    assert_eq!(preview.delta, 970);

    // Hysteresis keeps the winner under sub-window drift.
    ws.drag_update(arr, MusicalTick::new(1010), &env).unwrap();
    let preview = &ws.state(arr).unwrap().gesture.as_ref().unwrap().preview;
    assert_eq!(preview.snap.unwrap().label(), "clip-bounds@970");
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();

    // Multi-clip move: only the primary snaps; the other clip keeps
    // its relative offset (same delta applied).
    ws.select(
        arr,
        EditingSelection {
            targets: vec![
                SemanticTarget::Clip(clip2),
                SemanticTarget::Clip(clip), // primary (last)
            ],
        },
    )
    .unwrap();
    let env = SnapEnvironment::default();
    ws.pointer_down(arr, &body_probe(0, SemanticTarget::Clip(clip)))
        .unwrap();
    ws.drag_update(arr, MusicalTick::new(1000), &env).unwrap();
    let out = ws.commit_gesture(arr).unwrap();
    assert!(matches!(out, GestureOutcome::Applied { .. }));
    assert_eq!(musical_bounds(&ws, clip), (960, 1920));
    assert_eq!(
        musical_bounds(&ws, clip2),
        (1920, 2880),
        "co-mover keeps the 960-tick relative offset"
    );

    // Mixed musical+media targets refuse at gesture start.
    let media_clip = add_media_clip(&mut ws, lane);
    ws.select(
        arr,
        EditingSelection {
            targets: vec![SemanticTarget::Clip(clip), SemanticTarget::Clip(media_clip)],
        },
    )
    .unwrap();
    let err = ws
        .pointer_down(arr, &body_probe(960, SemanticTarget::Clip(media_clip)))
        .unwrap_err();
    assert!(matches!(err, WorkspaceError::MixedTimeDomains(_)));
}

// ---------------------------------------------------------------------------
// ED-A-04 — Preview/transaction/undo
// ---------------------------------------------------------------------------

/// Preview is session state only; commit is one atomic op = one undo;
/// validation is all-or-nothing; stale gestures reject; cancels leave
/// the project untouched; undo/redo route through the session engine
/// hook while text lanes keep their own undo.
#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenarios
fn ed_a_04_preview_atomic_undo() {
    let (mut ws, _track, lane, clip, _source) = seeded();
    let clip2 = add_musical_clip(&mut ws, lane, 960, 1920);
    let arr = arrangement(&mut ws);
    let env = SnapEnvironment::default();

    // --- preview never mutates ------------------------------------------------
    let rev0 = ws.session().revision();
    let builds0 = ws.session().engine().graph_builds;
    ws.select(
        arr,
        EditingSelection {
            targets: vec![SemanticTarget::Clip(clip), SemanticTarget::Clip(clip2)],
        },
    )
    .unwrap();
    ws.pointer_down(arr, &body_probe(0, SemanticTarget::Clip(clip2)))
        .unwrap();
    for pos in [200, 500, 1000] {
        ws.drag_update(arr, MusicalTick::new(pos), &env).unwrap();
    }
    assert_eq!(ws.session().revision(), rev0, "preview must not commit");
    assert_eq!(
        ws.session().engine().graph_builds,
        builds0,
        "preview must not rebuild the engine graph"
    );
    assert_eq!(
        musical_bounds(&ws, clip2),
        (960, 1920),
        "preview is not a project mutation"
    );

    // --- one atomic commit = one undo ------------------------------------------
    let out = ws.commit_gesture(arr).unwrap();
    let GestureOutcome::Applied { revision } = out else {
        panic!("expected Applied, got {out:?}");
    };
    assert_eq!(revision.0, rev0.0 + 1, "single atomic apply = one revision");
    assert_eq!(musical_bounds(&ws, clip), (960, 1920));
    assert_eq!(musical_bounds(&ws, clip2), (1920, 2880));

    let undo_ctx = ctx_from("edit.undo", arr);
    let outcome = ws.dispatch(undo_ctx);
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    assert_eq!(musical_bounds(&ws, clip), (0, 960));
    assert_eq!(
        musical_bounds(&ws, clip2),
        (960, 1920),
        "one undo restores the whole multi-target gesture"
    );
    assert!(
        ws.session().engine().graph_builds > builds0,
        "undo routes through the session engine hook"
    );
    let outcome = ws.dispatch(ctx_from("edit.redo", arr));
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    assert_eq!(musical_bounds(&ws, clip), (960, 1920));

    // --- all-or-nothing validation ---------------------------------------------
    let ghost = SemanticTarget::Clip(ClipId::new(9_999));
    ws.select(
        arr,
        EditingSelection {
            targets: vec![SemanticTarget::Clip(clip), ghost],
        },
    )
    .unwrap();
    let before = musical_bounds(&ws, clip);
    ws.pointer_down(arr, &body_probe(960, ghost)).unwrap();
    ws.drag_update(arr, MusicalTick::new(1200), &env).unwrap();
    let err = ws.commit_gesture(arr).unwrap_err();
    assert!(
        matches!(
            err,
            WorkspaceError::Project(_) | WorkspaceError::InvalidGesture(_)
        ),
        "missing fixated target must reject the whole commit: {err:?}"
    );
    assert_eq!(
        musical_bounds(&ws, clip),
        before,
        "one invalid target must veto the whole commit"
    );
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();

    // --- stale revision rejects, never retargets --------------------------------
    ws.select(arr, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    ws.pointer_down(arr, &body_probe(960, SemanticTarget::Clip(clip)))
        .unwrap();
    ws.drag_update(arr, MusicalTick::new(1920), &env).unwrap();
    apply(
        &mut ws,
        ProjectOp::SetProjectName {
            name: "meanwhile".to_string(),
        },
    );
    let err = ws.commit_gesture(arr).unwrap_err();
    assert!(
        matches!(
            err,
            WorkspaceError::Project(synvelis_project::Error::StaleRevision { .. })
        ),
        "stale gesture must reject, got {err:?}"
    );
    assert_eq!(musical_bounds(&ws, clip), (960, 1920));
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();

    // --- cancel paths -----------------------------------------------------------
    for reason in [
        CancelReason::Escape,
        CancelReason::PointerCancel,
        CancelReason::InputScopeLost,
    ] {
        let before_rev = ws.session().revision();
        ws.pointer_down(arr, &body_probe(960, SemanticTarget::Clip(clip)))
            .unwrap();
        ws.drag_update(arr, MusicalTick::new(1500), &env).unwrap();
        let out = ws.cancel_gesture(arr, reason).unwrap();
        assert!(matches!(out, GestureOutcome::Cancelled { .. }));
        assert!(ws.state(arr).unwrap().gesture.is_none());
        assert_eq!(
            ws.session().revision(),
            before_rev,
            "cancel must not commit"
        );
    }
    // Window destruction drops a gesture with the view state.
    ws.pointer_down(arr, &body_probe(960, SemanticTarget::Clip(clip)))
        .unwrap();
    let before_rev = ws.session().revision();
    ws.remove_view(arr).unwrap();
    assert_eq!(ws.session().revision(), before_rev);

    // --- undo lane routing ------------------------------------------------------
    let arr = arrangement(&mut ws);
    let mut ctx = ctx_from("edit.undo", arr);
    ctx.input = InputContext::TextInput;
    let rev_before = ws.session().revision();
    let builds_before = ws.session().engine().graph_builds;
    let outcome = ws.dispatch(ctx);
    assert!(matches!(
        outcome,
        CommandOutcome::Applied { revision: None }
    ));
    assert_eq!(ws.text_lane_undos(), 1, "text undo lives on its own lane");
    assert_eq!(ws.session().revision(), rev_before);
    assert_eq!(ws.session().engine().graph_builds, builds_before);
    let mut ctx = ctx_from("edit.undo", arr);
    ctx.input = InputContext::Modal;
    assert!(matches!(
        ws.dispatch(ctx),
        CommandOutcome::Suppressed { .. }
    ));
}

// ---------------------------------------------------------------------------
// ED-A-05 — Cross-view / linked context separation
// ---------------------------------------------------------------------------

/// `edit_binding` is a display binding, never a command target; link
/// events during a gesture don't retarget it; link delivery is
/// non-destructive.
#[test]
fn ed_a_05_linked_context_separation() {
    let (mut ws, track, _lane, clip, _source) = seeded();
    let arr_a = arrangement(&mut ws);
    let arr_b = arrangement(&mut ws);
    ws.links_mut().set_group(arr_a, Some(LinkGroupId(1)));
    ws.links_mut().set_group(arr_b, Some(LinkGroupId(1)));

    // open_in_editor sets a display binding only.
    ws.open_in_editor(arr_b, clip).unwrap();
    let st = ws.state(arr_b).unwrap();
    assert_eq!(st.edit_binding.unwrap().clip, clip);
    assert!(st.object_selections.is_empty());
    assert!(st.linked_context.target.is_none());

    // The binding alone does not resolve editing commands.
    let outcome = dispatch(&mut ws, "clip.remove", arr_b);
    assert!(matches!(outcome, CommandOutcome::Unresolved { .. }));
    assert!(ws.session().project().clips.contains_key(&clip));

    // A gesture on arr_a keeps its fixed target while arr_b's link
    // event arrives mid-gesture.
    ws.select(arr_a, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap();
    ws.pointer_down(arr_a, &body_probe(0, SemanticTarget::Clip(clip)))
        .unwrap();
    ws.select(arr_b, EditingSelection::single(SemanticTarget::Clip(clip)))
        .unwrap(); // publishes a link event to arr_a
    let st = ws.state(arr_a).unwrap();
    assert_eq!(
        st.gesture.as_ref().unwrap().fixation.targets,
        vec![SemanticTarget::Clip(clip)],
        "link events must not retarget a fixed gesture"
    );

    // Link delivery wrote only linked_context / inspection_focus on
    // arr_a — never the editing selection, never the project.
    assert_eq!(
        st.linked_context.target,
        Some(SemanticTarget::Track(track)),
        "clip links map to the owning track"
    );
    let st = ws.state(arr_a).unwrap();
    assert_eq!(
        st.object_selections.primary(),
        Some(SemanticTarget::Clip(clip)),
        "delivery must not rewrite the editing selection"
    );

    // The gesture still commits against its fixation.
    let env = SnapEnvironment::default();
    ws.drag_update(arr_a, MusicalTick::new(1000), &env).unwrap();
    let out = ws.commit_gesture(arr_a).unwrap();
    assert!(matches!(out, GestureOutcome::Applied { .. }));
    assert_eq!(musical_bounds(&ws, clip), (960, 1920));

    // Read-only peer views receive context, not edit power:
    // arr_b's linked context is a track highlight that left its own
    // (separate) editing state — and the project — untouched.
    let st = ws.state(arr_b).unwrap();
    assert!(st.linked_context.target.is_some());
    assert_eq!(
        st.edit_binding.unwrap().clip,
        clip,
        "display binding persists untouched by the link event"
    );
}
