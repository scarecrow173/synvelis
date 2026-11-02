//! ED-B acceptance tests — `docs/test-specs/ed-b-e2e-media-domain-editing.md`.
//!
//! Issue #65: media-domain move/trim, `SlipClip` → `SetClipPlayback`
//! commit semantics per `PlaybackMapping`, explicit mixed-domain
//! outcomes (no silent conversion) and media-domain marquee — all
//! verified headlessly with semantic `HitProbe`s.

use ruviform_ui::dock::DockTarget;
use ruviform_ui::window::MonitorInfo;
use synvelis_project::{
    ClipExtent, ClipId, ClipPlacement, ClipPlayback, CounterIdGenerator, LaneId, MediaDuration,
    MediaTimeValue, MusicalDuration, MusicalTick, PlaybackMapping, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, SourceFrame, SourceRange, StretchRatio, TickTimeMapping,
    TimeAnchor, TrackId,
};
use synvelis_workspace::{
    CancelReason, CommandOutcome, EditTimePoint, EditingSelection, FakeWindowHost, GestureOutcome,
    HandleKind, HitCandidate, HitProbe, HitZone, LaneScope, ProjectSession, SemanticTarget,
    SnapEnvironment, Tool, ViewType, WorkspaceError, WorkspaceManager,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

const BASE: u32 = 48_000;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xEDB), "ed-b-test");
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

/// track + lane + source, no clips yet.
fn seeded() -> (Ws, TrackId, LaneId, synvelis_project::SourceId) {
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
    (ws, track_id, lane_id, source_id)
}

fn mt(v: i64) -> MediaTimeValue {
    MediaTimeValue::new(v, 1, BASE).unwrap()
}

fn media_pos(v: i64) -> EditTimePoint {
    EditTimePoint::Media(mt(v))
}

fn musical_pos(tick: i64) -> EditTimePoint {
    EditTimePoint::Musical(MusicalTick::new(tick))
}

fn add_clip(
    ws: &mut Ws,
    lane_id: LaneId,
    placement: ClipPlacement,
    playback: ClipPlayback,
) -> ClipId {
    let source_id = *ws.session().project().sources.keys().next().unwrap();
    apply(
        ws,
        ProjectOp::AddClip {
            lane_id,
            source_id,
            placement,
            playback,
        },
    );
    *ws.session().project().clips.keys().last().unwrap()
}

/// Absolute×Absolute clip `v..v+len` in base 1/BASE, natural playback
/// over a matching source window.
fn add_media_clip(ws: &mut Ws, lane_id: LaneId, v: i64, len: i64) -> ClipId {
    add_clip(
        ws,
        lane_id,
        ClipPlacement {
            anchor: TimeAnchor::Absolute(mt(v)),
            extent: ClipExtent::Absolute(MediaDuration::new(len, 1, BASE).unwrap()),
        },
        ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::new(1_000).unwrap(),
                SourceFrame::new(1_000 + len).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    )
}

/// Mixed clip: musical anchor + absolute extent (declared rate BASE).
fn add_musical_abs_clip(ws: &mut Ws, lane_id: LaneId, start: i64, len: i64) -> ClipId {
    add_clip(
        ws,
        lane_id,
        ClipPlacement {
            anchor: TimeAnchor::Musical(MusicalTick::new(start)),
            extent: ClipExtent::Absolute(MediaDuration::new(len, 1, BASE).unwrap()),
        },
        ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::new(1_000).unwrap(),
                SourceFrame::new(1_000 + len).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    )
}

/// Mixed clip: absolute anchor + musical extent.
fn add_abs_musical_clip(ws: &mut Ws, lane_id: LaneId, v: i64, ticks: i64) -> ClipId {
    add_clip(
        ws,
        lane_id,
        ClipPlacement {
            anchor: TimeAnchor::Absolute(mt(v)),
            extent: ClipExtent::Musical(MusicalDuration::new(ticks).unwrap()),
        },
        ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::ZERO,
                SourceFrame::new(1_000_000).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    )
}

fn add_musical_clip(ws: &mut Ws, lane_id: LaneId, start: i64, end: i64) -> ClipId {
    add_clip(
        ws,
        lane_id,
        ClipPlacement {
            anchor: TimeAnchor::Musical(MusicalTick::new(start)),
            extent: ClipExtent::Musical(MusicalDuration::new(end - start).unwrap()),
        },
        ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::ZERO,
                SourceFrame::new(1_000_000).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    )
}

fn arrangement(ws: &mut Ws) -> ruviform_ui::dock::ViewInstanceId {
    let win = ws.create_window("arr-win", 1280, 800).unwrap();
    ws.spawn_view(ViewType::Arrangement, "arr", DockTarget::Window(win))
        .unwrap()
}

fn probe(position: EditTimePoint, zone: HitZone, target: SemanticTarget) -> HitProbe {
    HitProbe {
        position,
        candidates: vec![HitCandidate {
            zone,
            target: Some(target),
        }],
    }
}

fn clip_of(ws: &Ws, clip_id: ClipId) -> synvelis_project::Clip {
    ws.session().project().clips.get(&clip_id).unwrap().clone()
}

fn anchor_of(ws: &Ws, clip_id: ClipId) -> TimeAnchor {
    clip_of(ws, clip_id).placement.anchor
}

fn disable_snap(ws: &mut Ws, view: ruviform_ui::dock::ViewInstanceId) {
    ws.state_mut(view).unwrap().snap.enabled = false;
}

// ---------------------------------------------------------------------------
// ED-B-01 — media-domain move commits
// ---------------------------------------------------------------------------

/// An `Absolute`-anchored clip's body drag commits `MoveClip` with a
/// media anchor; extent, playback and mute state are untouched.
#[test]
fn ed_b_01_media_move_commits_absolute_anchor() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_media_clip(&mut ws, lane, 48_000, 48_000);
    let before = clip_of(&ws, clip);
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);

    ws.pointer_down(
        arr,
        &probe(media_pos(48_000), HitZone::Body, SemanticTarget::Clip(clip)),
    )
    .unwrap();
    ws.drag_update(arr, media_pos(72_000), &SnapEnvironment::default())
        .unwrap();
    let out = ws.commit_gesture(arr).unwrap();
    assert!(matches!(out, GestureOutcome::Applied { .. }));

    let after = clip_of(&ws, clip);
    assert_eq!(
        after.placement.anchor,
        TimeAnchor::Absolute(mt(72_000)),
        "anchor moved by the media delta in its declared base"
    );
    assert_eq!(
        after.placement.extent, before.placement.extent,
        "MoveClip preserves the extent"
    );
    assert_eq!(
        after.playback, before.playback,
        "MoveClip preserves the playback contract"
    );
    assert_eq!(after.muted, before.muted);
}

/// Multiple media clips move atomically — one `Composite` apply, one
/// undo — each rescaling the shared delta into its own declared base.
#[test]
fn ed_b_01_media_multi_move_is_one_undo() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip_a = add_media_clip(&mut ws, lane, 0, 48_000);
    let clip_b = add_media_clip(&mut ws, lane, 48_000, 48_000);
    // A clip declaring a different time base rescales the same delta.
    let clip_c = add_clip(
        &mut ws,
        lane,
        ClipPlacement {
            anchor: TimeAnchor::Absolute(MediaTimeValue::new(12_000, 1, 24_000).unwrap()),
            extent: ClipExtent::Absolute(MediaDuration::new(12_000, 1, 24_000).unwrap()),
        },
        ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::new(1_000).unwrap(),
                SourceFrame::new(13_000).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    );
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);
    let rev0 = ws.session().revision();

    ws.select(
        arr,
        EditingSelection {
            targets: vec![
                SemanticTarget::Clip(clip_a),
                SemanticTarget::Clip(clip_c),
                SemanticTarget::Clip(clip_b), // primary (last)
            ],
        },
    )
    .unwrap();
    ws.pointer_down(
        arr,
        &probe(
            media_pos(48_000),
            HitZone::Body,
            SemanticTarget::Clip(clip_b),
        ),
    )
    .unwrap();
    ws.drag_update(arr, media_pos(96_000), &SnapEnvironment::default())
        .unwrap();
    let out = ws.commit_gesture(arr).unwrap();
    let GestureOutcome::Applied { revision } = out else {
        panic!("expected Applied, got {out:?}");
    };
    assert_eq!(revision.0, rev0.0 + 1, "one atomic apply");
    assert_eq!(anchor_of(&ws, clip_a), TimeAnchor::Absolute(mt(48_000)));
    assert_eq!(anchor_of(&ws, clip_b), TimeAnchor::Absolute(mt(96_000)));
    assert_eq!(
        anchor_of(&ws, clip_c),
        TimeAnchor::Absolute(MediaTimeValue::new(36_000, 1, 24_000).unwrap()),
        "the shared 48000@1/48000 delta lands as 24000@1/24000"
    );

    let outcome = ws.dispatch(synvelis_workspace::CommandContext::from_view(
        "edit.undo",
        arr,
    ));
    assert!(matches!(outcome, CommandOutcome::Applied { .. }));
    assert_eq!(anchor_of(&ws, clip_a), TimeAnchor::Absolute(mt(0)));
    assert_eq!(
        anchor_of(&ws, clip_b),
        TimeAnchor::Absolute(mt(48_000)),
        "one undo restores the whole multi-clip move"
    );
    assert_eq!(
        anchor_of(&ws, clip_c),
        TimeAnchor::Absolute(MediaTimeValue::new(12_000, 1, 24_000).unwrap())
    );
}

// ---------------------------------------------------------------------------
// ED-B-02 — media-domain trim commits
// ---------------------------------------------------------------------------

/// `StartTrim` moves the anchor and shrinks the extent atomically
/// (one `Composite`), keeping the media end fixed; `EndTrim` only
/// extends the extent.
#[test]
fn ed_b_02_media_trim_moves_edges() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_media_clip(&mut ws, lane, 48_000, 48_000); // [48000, 96000)
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);
    ws.set_tool(arr, Tool::Trim).unwrap();

    ws.pointer_down(
        arr,
        &probe(
            media_pos(48_000),
            HitZone::Handle(HandleKind::StartTrim),
            SemanticTarget::Clip(clip),
        ),
    )
    .unwrap();
    ws.drag_update(arr, media_pos(52_800), &SnapEnvironment::default())
        .unwrap();
    assert!(matches!(
        ws.commit_gesture(arr).unwrap(),
        GestureOutcome::Applied { .. }
    ));
    let c = clip_of(&ws, clip);
    assert_eq!(c.placement.anchor, TimeAnchor::Absolute(mt(52_800)));
    assert_eq!(
        c.placement.extent,
        ClipExtent::Absolute(MediaDuration::new(43_200, 1, BASE).unwrap()),
        "media end stays fixed: 96000 - 52800 = 43200"
    );

    ws.pointer_down(
        arr,
        &probe(
            media_pos(96_000),
            HitZone::Handle(HandleKind::EndTrim),
            SemanticTarget::Clip(clip),
        ),
    )
    .unwrap();
    ws.drag_update(arr, media_pos(100_800), &SnapEnvironment::default())
        .unwrap();
    assert!(matches!(
        ws.commit_gesture(arr).unwrap(),
        GestureOutcome::Applied { .. }
    ));
    let c = clip_of(&ws, clip);
    assert_eq!(
        c.placement.anchor,
        TimeAnchor::Absolute(mt(52_800)),
        "EndTrim keeps the anchor"
    );
    assert_eq!(
        c.placement.extent,
        ClipExtent::Absolute(MediaDuration::new(48_000, 1, BASE).unwrap()),
        "43200 + 4800"
    );
}

/// `StartTrim` computes the remaining extent as one floored rational
/// — `floor(extent − applied_delta)` — not `floor(end) −
/// floor(new_anchor)`, so a clip whose anchor and extent declare
/// different bases keeps its end within one extent unit of fixed
/// (and commits `0` — rejecting — when the drag passes the end).
#[test]
fn ed_b_02_start_trim_single_floor_across_bases() {
    let (mut ws, _track, lane, _source) = seeded();
    // anchor 1/6 s, extent 3/2 s → fixed end 5/3 s.
    let clip = add_clip(
        &mut ws,
        lane,
        ClipPlacement {
            anchor: TimeAnchor::Absolute(MediaTimeValue::new(1, 1, 6).unwrap()),
            extent: ClipExtent::Absolute(MediaDuration::new(3, 1, 2).unwrap()),
        },
        ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::new(1_000).unwrap(),
                SourceFrame::new(1_004).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    );
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);
    ws.set_tool(arr, Tool::Trim).unwrap();
    let base6 = |v| EditTimePoint::Media(MediaTimeValue::new(v, 1, 6).unwrap());

    ws.pointer_down(
        arr,
        &probe(
            base6(1),
            HitZone::Handle(HandleKind::StartTrim),
            SemanticTarget::Clip(clip),
        ),
    )
    .unwrap();
    // +1 @1/6s → anchor 2/6 = 1/3 s; remaining = 4/3 s → floor(4/3 ÷ 1/2) = 2.
    ws.drag_update(arr, base6(2), &SnapEnvironment::default())
        .unwrap();
    assert!(matches!(
        ws.commit_gesture(arr).unwrap(),
        GestureOutcome::Applied { .. }
    ));
    let c = clip_of(&ws, clip);
    assert_eq!(
        c.placement.anchor,
        TimeAnchor::Absolute(MediaTimeValue::new(2, 1, 6).unwrap())
    );
    assert_eq!(
        c.placement.extent,
        ClipExtent::Absolute(MediaDuration::new(2, 1, 2).unwrap()),
        "single-floor extent: end 1/3+1 = 4/3 s stays within the fixed 5/3 s"
    );
}

/// A `StartTrim` dragged past the media end produces a negative
/// extent — a defined rejection, never a clamp to zero.
#[test]
fn ed_b_02_start_trim_past_end_rejects() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_media_clip(&mut ws, lane, 0, 4_800);
    let before = clip_of(&ws, clip);
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);
    ws.set_tool(arr, Tool::Trim).unwrap();

    ws.pointer_down(
        arr,
        &probe(
            media_pos(0),
            HitZone::Handle(HandleKind::StartTrim),
            SemanticTarget::Clip(clip),
        ),
    )
    .unwrap();
    ws.drag_update(arr, media_pos(9_600), &SnapEnvironment::default())
        .unwrap();
    let err = ws.commit_gesture(arr).unwrap_err();
    assert!(
        matches!(
            err,
            WorkspaceError::Project(
                synvelis_project::Error::Negative | synvelis_project::Error::Validation(_),
            )
        ),
        "past-end start trim must reject: {err:?}"
    );
    assert_eq!(clip_of(&ws, clip), before);
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();
}

/// `EndTrim` snaps the *end* edge — the dragged boundary lands on the
/// candidate, not the stationary start.
#[test]
fn ed_b_02_end_trim_snaps_the_end_edge() {
    let (mut ws, _track, lane, _source) = seeded();
    // end 91200; the source window must cover the post-snap extent.
    let clip = add_clip(
        &mut ws,
        lane,
        ClipPlacement {
            anchor: TimeAnchor::Absolute(mt(48_000)),
            extent: ClipExtent::Absolute(MediaDuration::new(43_200, 1, BASE).unwrap()),
        },
        ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::new(1_000).unwrap(),
                SourceFrame::new(97_000).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    );
    let arr = arrangement(&mut ws);
    ws.state_mut(arr).unwrap().snap.unit = synvelis_workspace::SnapUnit::Explicit(96_000_000); // grid far away
    ws.set_tool(arr, Tool::Trim).unwrap();
    let env = SnapEnvironment {
        auto_step: 96_000_000,
        candidates: vec![synvelis_workspace::SnapCandidate {
            source: synvelis_workspace::SnapSource::ClipBounds,
            position: 96_000,
            domain: synvelis_workspace::EditTimeDomain::Media,
        }],
    };

    ws.pointer_down(
        arr,
        &probe(
            media_pos(91_200),
            HitZone::Handle(HandleKind::EndTrim),
            SemanticTarget::Clip(clip),
        ),
    )
    .unwrap();
    // End edge lands 100 units from the candidate (threshold 240).
    ws.drag_update(arr, media_pos(95_900), &env).unwrap();
    let preview = &ws.state(arr).unwrap().gesture.as_ref().unwrap().preview;
    assert_eq!(
        preview.snap.map(|w| w.label()).as_deref(),
        Some("clip-bounds@96000")
    );
    assert!(matches!(
        ws.commit_gesture(arr).unwrap(),
        GestureOutcome::Applied { .. }
    ));
    let c = clip_of(&ws, clip);
    assert_eq!(
        c.placement.extent,
        ClipExtent::Absolute(MediaDuration::new(48_000, 1, BASE).unwrap()),
        "end lands on the candidate: 96000 - 48000"
    );
}

/// Trimming an extent to zero is a defined project rejection, not a
/// silent clamp; the clip is untouched.
#[test]
fn ed_b_02_media_trim_to_zero_rejects() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_media_clip(&mut ws, lane, 0, 48_000);
    let before = clip_of(&ws, clip);
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);
    ws.set_tool(arr, Tool::Trim).unwrap();

    ws.pointer_down(
        arr,
        &probe(
            media_pos(48_000),
            HitZone::Handle(HandleKind::EndTrim),
            SemanticTarget::Clip(clip),
        ),
    )
    .unwrap();
    ws.drag_update(arr, media_pos(0), &SnapEnvironment::default())
        .unwrap();
    let err = ws.commit_gesture(arr).unwrap_err();
    assert!(
        matches!(err, WorkspaceError::Project(_)),
        "empty extent must reject, got {err:?}"
    );
    assert_eq!(clip_of(&ws, clip), before, "rejected commit is atomic");
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();
}

// ---------------------------------------------------------------------------
// ED-B-03 — slip → SetClipPlayback per mapping
// ---------------------------------------------------------------------------

fn slip(
    ws: &mut Ws,
    view: ruviform_ui::dock::ViewInstanceId,
    clip: ClipId,
    position: EditTimePoint,
    drag_to: EditTimePoint,
) -> Result<GestureOutcome, WorkspaceError> {
    ws.set_tool(view, Tool::Slip).unwrap();
    ws.pointer_down(
        view,
        &probe(position, HitZone::Body, SemanticTarget::Clip(clip)),
    )?;
    ws.drag_update(view, drag_to, &SnapEnvironment::default())?;
    ws.commit_gesture(view)
}

/// `Natural` slip translates `source_region` 1:1 with the media delta;
/// anchor/extent/mapping are preserved.
#[test]
fn ed_b_03_slip_natural_moves_source_region() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_media_clip(&mut ws, lane, 0, 48_000);
    let before = clip_of(&ws, clip);
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);

    let out = slip(&mut ws, arr, clip, media_pos(0), media_pos(4_800)).unwrap();
    assert!(matches!(out, GestureOutcome::Applied { .. }));
    let after = clip_of(&ws, clip);
    assert_eq!(
        after.playback.source_region.start().value(),
        before.playback.source_region.start().value() + 4_800,
        "Natural: media delta maps 1:1 into source frames"
    );
    assert_eq!(
        after.playback.source_region.end().value(),
        before.playback.source_region.end().value() + 4_800
    );
    assert_eq!(after.placement, before.placement, "slip keeps placement");
    assert_eq!(after.playback.mapping, PlaybackMapping::Natural);
}

/// `Loop` slip translates the looped window itself 1:1 — the repeat
/// source span moves as a unit.
#[test]
fn ed_b_03_slip_loop_moves_window_1to1() {
    let (mut ws, _track, lane, _source) = seeded();
    // Loop needs only a non-empty window; extent may exceed it.
    let clip = add_clip(
        &mut ws,
        lane,
        ClipPlacement {
            anchor: TimeAnchor::Absolute(mt(0)),
            extent: ClipExtent::Absolute(MediaDuration::new(48_000, 1, BASE).unwrap()),
        },
        ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::new(2_000).unwrap(),
                SourceFrame::new(12_000).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::Loop,
        },
    );
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);

    let out = slip(&mut ws, arr, clip, media_pos(0), media_pos(4_800)).unwrap();
    assert!(matches!(out, GestureOutcome::Applied { .. }));
    let after = clip_of(&ws, clip);
    assert_eq!(after.playback.source_region.start().value(), 6_800);
    assert_eq!(after.playback.source_region.end().value(), 16_800);
    assert_eq!(after.playback.mapping, PlaybackMapping::Loop);
}

/// `SimpleStretch` slip scales the source shift by the ratio — a
/// 2:1 clip consumes two source frames per media frame.
#[test]
fn ed_b_03_slip_stretch_scales_by_ratio() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_clip(
        &mut ws,
        lane,
        ClipPlacement {
            anchor: TimeAnchor::Absolute(mt(0)),
            extent: ClipExtent::Absolute(MediaDuration::new(48_000, 1, BASE).unwrap()),
        },
        ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::new(1_000).unwrap(),
                SourceFrame::new(1_000 + 96_000).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::SimpleStretch(StretchRatio::new(2, 1).unwrap()),
        },
    );
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);

    let out = slip(&mut ws, arr, clip, media_pos(0), media_pos(4_800)).unwrap();
    assert!(matches!(out, GestureOutcome::Applied { .. }));
    let after = clip_of(&ws, clip);
    assert_eq!(
        after.playback.source_region.start().value(),
        1_000 + 9_600,
        "2:1 stretch consumes 2 source frames per media frame"
    );
}

/// A slip that would underflow `source_region.start` is a defined
/// `Negative` rejection — never a clamp to zero.
#[test]
fn ed_b_03_slip_negative_underflow_rejects() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_media_clip(&mut ws, lane, 0, 48_000);
    let before = clip_of(&ws, clip);
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);

    // start=1000; slipping -2000 media frames needs -2000 source frames.
    let out = slip(&mut ws, arr, clip, media_pos(48_000), media_pos(46_000));
    assert!(
        matches!(
            out,
            Err(WorkspaceError::Project(synvelis_project::Error::Negative))
        ),
        "source underflow must reject: {out:?}"
    );
    assert_eq!(clip_of(&ws, clip), before, "rejected slip is atomic");
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();
}

// ---------------------------------------------------------------------------
// ED-B-04 — mixed-domain clips: explicit outcomes
// ---------------------------------------------------------------------------

/// A mixed clip moves in its anchor's domain; trim is a defined
/// rejection (no declared conversion across the mixed extent).
#[test]
fn ed_b_04_mixed_clip_move_ok_trim_rejects() {
    let (mut ws, _track, lane, _source) = seeded();
    let mixed = add_musical_abs_clip(&mut ws, lane, 0, 48_000);
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);

    // Move: musical anchor → musical gesture → MoveClip commits.
    ws.pointer_down(
        arr,
        &probe(musical_pos(0), HitZone::Body, SemanticTarget::Clip(mixed)),
    )
    .unwrap();
    ws.drag_update(arr, musical_pos(960), &SnapEnvironment::default())
        .unwrap();
    assert!(matches!(
        ws.commit_gesture(arr).unwrap(),
        GestureOutcome::Applied { .. }
    ));
    assert_eq!(
        anchor_of(&ws, mixed),
        TimeAnchor::Musical(MusicalTick::new(960)),
        "mixed clip moves in its anchor domain"
    );

    // Trim: defined rejection — the extent lives in another domain.
    ws.set_tool(arr, Tool::Trim).unwrap();
    ws.pointer_down(
        arr,
        &probe(
            musical_pos(0),
            HitZone::Handle(HandleKind::EndTrim),
            SemanticTarget::Clip(mixed),
        ),
    )
    .unwrap();
    ws.drag_update(arr, musical_pos(960), &SnapEnvironment::default())
        .unwrap();
    let err = ws.commit_gesture(arr).unwrap_err();
    assert!(matches!(
        err,
        WorkspaceError::UnsupportedOperation("trim on a mixed anchor/extent domain clip")
    ));
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();
}

/// A musically anchored clip with an absolute extent slips via the
/// declared tempo-map conversion: tick delta → media frames at the
/// declared source rate.
#[test]
fn ed_b_04_mixed_slip_uses_tempo_map() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_musical_abs_clip(&mut ws, lane, 0, 48_000);
    let before = clip_of(&ws, clip);
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);

    let map = TickTimeMapping::new(&ws.session().project().tempo_map).unwrap();
    let expected = map
        .tick_to_media(MusicalTick::new(960), 1, BASE)
        .unwrap()
        .value()
        - map
            .tick_to_media(MusicalTick::new(0), 1, BASE)
            .unwrap()
            .value();

    let out = slip(&mut ws, arr, clip, musical_pos(0), musical_pos(960)).unwrap();
    assert!(matches!(out, GestureOutcome::Applied { .. }), "{out:?}");
    let after = clip_of(&ws, clip);
    assert_eq!(
        after.playback.source_region.start().value(),
        before.playback.source_region.start().value() + expected,
        "tick delta converts through the tempo map at the declared rate"
    );
    assert_eq!(after.placement, before.placement);
}

/// An `Absolute`×`Musical` clip still moves in its anchor domain —
/// but its trim is a defined rejection, exactly like the other mixed
/// pairing (no silent extent-domain conversion).
#[test]
fn ed_b_04_abs_musical_move_ok_trim_rejects() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_abs_musical_clip(&mut ws, lane, 0, 960);
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);

    ws.pointer_down(
        arr,
        &probe(media_pos(0), HitZone::Body, SemanticTarget::Clip(clip)),
    )
    .unwrap();
    ws.drag_update(arr, media_pos(4_800), &SnapEnvironment::default())
        .unwrap();
    assert!(matches!(
        ws.commit_gesture(arr).unwrap(),
        GestureOutcome::Applied { .. }
    ));
    assert_eq!(anchor_of(&ws, clip), TimeAnchor::Absolute(mt(4_800)));

    ws.set_tool(arr, Tool::Trim).unwrap();
    ws.pointer_down(
        arr,
        &probe(
            media_pos(4_800),
            HitZone::Handle(HandleKind::EndTrim),
            SemanticTarget::Clip(clip),
        ),
    )
    .unwrap();
    ws.drag_update(arr, media_pos(9_600), &SnapEnvironment::default())
        .unwrap();
    let err = ws.commit_gesture(arr).unwrap_err();
    assert!(matches!(
        err,
        WorkspaceError::UnsupportedOperation("trim on a mixed anchor/extent domain clip")
    ));
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();
}

/// An absolutely anchored clip with a musical extent declares no
/// source rate: slip is a defined rejection (not a tempo-map guess).
#[test]
fn ed_b_04_abs_musical_slip_rejects() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_abs_musical_clip(&mut ws, lane, 0, 960);
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);

    let out = slip(&mut ws, arr, clip, media_pos(0), media_pos(4_800));
    assert!(
        matches!(
            out,
            Err(WorkspaceError::UnsupportedOperation(
                "slip needs a declared media rate (absolute extent)"
            ))
        ),
        "no declared rate → defined rejection: {out:?}"
    );
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();
}

// ---------------------------------------------------------------------------
// ED-B-05 — media-domain marquee
// ---------------------------------------------------------------------------

/// A media probe marquee selects `Absolute`×`Absolute` clips whose
/// media span overlaps the dragged span; musical and mixed clips are
/// never selected by a media gesture.
#[test]
fn ed_b_05_media_marquee_selects_media_clips() {
    let (mut ws, _track, lane, _source) = seeded();
    let media_a = add_media_clip(&mut ws, lane, 0, 48_000);
    let media_b = add_media_clip(&mut ws, lane, 96_000, 48_000);
    let _musical = add_musical_clip(&mut ws, lane, 0, 960);
    let _mixed = add_musical_abs_clip(&mut ws, lane, 960, 48_000);
    let arr = arrangement(&mut ws);
    ws.set_lane_scope(arr, LaneScope::new(vec![lane])).unwrap();
    ws.set_tool(arr, Tool::Marquee).unwrap();

    ws.pointer_down(
        arr,
        &HitProbe {
            position: media_pos(0),
            candidates: vec![HitCandidate {
                zone: HitZone::Background,
                target: None,
            }],
        },
    )
    .unwrap();
    assert_eq!(
        ws.state(arr)
            .unwrap()
            .gesture
            .as_ref()
            .unwrap()
            .fixation
            .domain,
        synvelis_workspace::EditTimeDomain::Media,
        "a media probe starts a media-domain marquee"
    );
    ws.drag_update(arr, media_pos(120_000), &SnapEnvironment::default())
        .unwrap();
    let out = ws.commit_gesture(arr).unwrap();
    assert!(matches!(out, GestureOutcome::SelectionChanged));

    let selection = &ws
        .state(arr)
        .unwrap()
        .object_selections
        .active_selection()
        .targets;
    assert_eq!(
        selection.as_slice(),
        &[SemanticTarget::Clip(media_a), SemanticTarget::Clip(media_b)],
        "only fully-media clips in the span are selected"
    );
}

/// A musical marquee ignores media clips; a media marquee ignores
/// musical clips — each domain's selection stays in-domain.
#[test]
fn ed_b_05_domains_do_not_cross() {
    let (mut ws, _track, lane, _source) = seeded();
    let _media = add_media_clip(&mut ws, lane, 0, 48_000);
    let musical = add_musical_clip(&mut ws, lane, 0, 960);
    let arr = arrangement(&mut ws);
    ws.set_lane_scope(arr, LaneScope::new(vec![lane])).unwrap();
    ws.set_tool(arr, Tool::Marquee).unwrap();

    ws.pointer_down(
        arr,
        &HitProbe {
            position: musical_pos(0),
            candidates: vec![HitCandidate {
                zone: HitZone::Background,
                target: None,
            }],
        },
    )
    .unwrap();
    ws.drag_update(arr, musical_pos(1920), &SnapEnvironment::default())
        .unwrap();
    assert!(matches!(
        ws.commit_gesture(arr).unwrap(),
        GestureOutcome::SelectionChanged
    ));
    assert_eq!(
        ws.state(arr)
            .unwrap()
            .object_selections
            .active_selection()
            .targets,
        vec![SemanticTarget::Clip(musical)],
        "musical marquee keeps its domain"
    );
}

/// The media-overlap test is an exact rational comparison — no
/// conversion to a floored end — so a clip whose span overshoots the
/// marquee `lo` by a fraction of an anchor unit still selects.
#[test]
fn ed_b_05_media_marquee_exact_overlap_across_bases() {
    let (mut ws, _track, lane, _source) = seeded();
    // clip [0 s, 0.15 s) declared across two different bases.
    let clip = add_clip(
        &mut ws,
        lane,
        ClipPlacement {
            anchor: TimeAnchor::Absolute(MediaTimeValue::new(0, 1, 10).unwrap()),
            extent: ClipExtent::Absolute(MediaDuration::new(15, 1, 100).unwrap()),
        },
        ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::new(1_000).unwrap(),
                SourceFrame::new(1_016).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    );
    let arr = arrangement(&mut ws);
    ws.set_lane_scope(arr, LaneScope::new(vec![lane])).unwrap();
    ws.set_tool(arr, Tool::Marquee).unwrap();
    let base100 = |v| EditTimePoint::Media(MediaTimeValue::new(v, 1, 100).unwrap());

    ws.pointer_down(
        arr,
        &HitProbe {
            position: base100(12), // 0.12 s — inside the true span end 0.15
            candidates: vec![HitCandidate {
                zone: HitZone::Background,
                target: None,
            }],
        },
    )
    .unwrap();
    ws.drag_update(arr, base100(50), &SnapEnvironment::default())
        .unwrap();
    assert!(matches!(
        ws.commit_gesture(arr).unwrap(),
        GestureOutcome::SelectionChanged
    ));
    assert_eq!(
        ws.state(arr)
            .unwrap()
            .object_selections
            .active_selection()
            .targets,
        vec![SemanticTarget::Clip(clip)],
        "0.12 < 0.15 overlaps even though the end is not representable \
         in the anchor's 1/10 base"
    );
}

// ---------------------------------------------------------------------------
// ED-B-06 — probe/gesture domain mismatch rejected at start
// ---------------------------------------------------------------------------

/// A probe reporting a domain that disagrees with the target clip's
/// anchor domain is rejected at `pointer_down`; a mid-gesture domain
/// mismatch is likewise rejected by `drag_update`.
#[test]
fn ed_b_06_domain_mismatch_rejects_at_start() {
    let (mut ws, _track, lane, _source) = seeded();
    let media = add_media_clip(&mut ws, lane, 0, 48_000);
    let musical = add_musical_clip(&mut ws, lane, 0, 960);
    let arr = arrangement(&mut ws);

    let err = ws
        .pointer_down(
            arr,
            &probe(musical_pos(0), HitZone::Body, SemanticTarget::Clip(media)),
        )
        .unwrap_err();
    assert!(matches!(err, WorkspaceError::MixedTimeDomains(_)));

    let err = ws
        .pointer_down(
            arr,
            &probe(media_pos(0), HitZone::Body, SemanticTarget::Clip(musical)),
        )
        .unwrap_err();
    assert!(matches!(err, WorkspaceError::MixedTimeDomains(_)));

    // Mid-gesture domain flips are rejected too.
    ws.pointer_down(
        arr,
        &probe(musical_pos(0), HitZone::Body, SemanticTarget::Clip(musical)),
    )
    .unwrap();
    let err = ws
        .drag_update(arr, media_pos(100), &SnapEnvironment::default())
        .unwrap_err();
    assert!(matches!(err, WorkspaceError::MixedTimeDomains(_)));
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();
}

// ---------------------------------------------------------------------------
// ED-B-07 — verification
// ---------------------------------------------------------------------------

/// `mise run fmt-check`/`check`/`lint`/`test` run in CI; this test
/// pins that the media path integrates without breaking the ED-A
/// preview invariant — a failed commit leaves the project untouched.
#[test]
fn ed_b_07_failed_media_commit_is_atomic() {
    let (mut ws, _track, lane, _source) = seeded();
    let clip = add_media_clip(&mut ws, lane, 0, 48_000);
    let ghost = synvelis_project::ClipId::new(9_999);
    let before = clip_of(&ws, clip);
    let rev0 = ws.session().revision();
    let arr = arrangement(&mut ws);
    disable_snap(&mut ws, arr);

    ws.select(
        arr,
        EditingSelection {
            targets: vec![SemanticTarget::Clip(clip), SemanticTarget::Clip(ghost)],
        },
    )
    .unwrap();
    ws.pointer_down(
        arr,
        &probe(media_pos(0), HitZone::Body, SemanticTarget::Clip(ghost)),
    )
    .unwrap();
    ws.drag_update(arr, media_pos(4_800), &SnapEnvironment::default())
        .unwrap();
    let err = ws.commit_gesture(arr).unwrap_err();
    assert!(matches!(
        err,
        WorkspaceError::InvalidGesture(_) | WorkspaceError::Project(_)
    ));
    assert_eq!(clip_of(&ws, clip), before);
    assert_eq!(ws.session().revision(), rev0, "no apply happened");
    ws.cancel_gesture(arr, CancelReason::Escape).unwrap();
}
