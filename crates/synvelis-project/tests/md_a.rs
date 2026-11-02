//! MD-A acceptance tests — TakeGroup/Comp schema, Effective Comp
//! projection, and the ClipPlacement/ClipPlayback time contract.
//!
//! Headless model `PoC`: only `ProjectController` + `Command<..>` drive
//! mutations; the projection is a pure read. Takes/segments are seeded
//! as POD directly (recording-side ops live in `synvelis-recording`,
//! which may not be a dependency of this crate's tests).
//!
//! Tempo fixture: default map is 120 BPM @ PPQN 960 → 25 frames per
//! tick at 48 kHz.

use synvelis_project::*;

const RATE: u32 = 48_000;
/// 120 BPM @ PPQN 960 → 0.5 s/quarter → 25 frames per tick @48 kHz.
const FRAMES_PER_TICK: i64 = 25;

// ---------- fixtures ----------

fn controller(project: Project) -> ProjectController {
    ProjectController::new(project, Box::new(CounterIdGenerator::new(1_000)))
}

fn apply<O: Operation>(c: &mut ProjectController, op: O) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap()
}

/// Apply, expecting rejection: the error is returned and the live
/// revision is asserted unchanged by the caller-side helper
/// `rejected`.
fn try_apply<O: Operation>(c: &mut ProjectController, op: O) -> Result<ProjectRevision, Error> {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
}

fn rejected<O: Operation>(c: &mut ProjectController, op: O) {
    let rev = c.live_revision();
    assert!(try_apply(c, op).is_err(), "op must be rejected");
    assert_eq!(
        c.live_revision(),
        rev,
        "rejected op must not advance revision"
    );
}

/// A project seeded with one track, two lanes, one source, and three
/// takes on `lane_a`/`lane_b` over a shared source.
///
/// Take coverage: `take_a0` covers source `[0, 30_000)`, `take_a1`
/// covers `[40_000, 70_000)`, `take_b` (other lane) covers `[0, 1_000)`.
struct Seeded {
    lane_a: LaneId,
    lane_b: LaneId,
    take_a0: TakeId,
    take_a1: TakeId,
    take_b: TakeId,
    source_id: SourceId,
}

fn seeded() -> (Project, Seeded) {
    let mut p = Project::new(ProjectId::new(0), "md-a");
    let s = Seeded {
        lane_a: LaneId::new(2),
        lane_b: LaneId::new(3),
        take_a0: TakeId::new(15),
        take_a1: TakeId::new(16),
        take_b: TakeId::new(17),
        source_id: SourceId::new(5),
    };
    seed_structure(&mut p, s.lane_a, s.lane_b, s.source_id);
    seed_recording(&mut p, &s);
    (p, s)
}

fn seed_structure(p: &mut Project, lane_a: LaneId, lane_b: LaneId, source_id: SourceId) {
    let track_id = TrackId::new(1);
    let asset_id = AssetId::new(4);
    p.tracks.insert(
        track_id,
        Track {
            id: track_id,
            name: "t".into(),
            lane_ids: vec![lane_a, lane_b],
        },
    );
    for (id, track) in [(lane_a, track_id), (lane_b, track_id)] {
        p.lanes.insert(
            id,
            Lane {
                id,
                track_id: track,
                clip_ids: Vec::new(),
            },
        );
    }
    p.assets.insert(
        asset_id,
        Asset {
            id: asset_id,
            digest: "aa".into(),
        },
    );
    p.sources.insert(
        source_id,
        Source {
            id: source_id,
            asset_id,
            content_digest: "aa".into(),
            state: SourceState::Committed,
        },
    );
    p.clock_epochs.insert(
        ClockEpochId::new(18),
        ClockEpoch {
            id: ClockEpochId::new(18),
            description: "mock".into(),
        },
    );
}

fn seed_recording(p: &mut Project, s: &Seeded) {
    let session_id = RecordingSessionId::new(6);
    let pass0 = RecordingPassId::new(7);
    let pass1 = RecordingPassId::new(8);
    let pass2 = RecordingPassId::new(9);
    let stream_a = CaptureStreamId::new(10);
    let stream_b = CaptureStreamId::new(11);
    let seg0 = CaptureSegmentId::new(12);
    let seg1 = CaptureSegmentId::new(13);
    let seg2 = CaptureSegmentId::new(14);
    let epoch = ClockEpochId::new(18);

    p.recording.sessions.insert(
        session_id,
        RecordingSession {
            id: session_id,
            name: "s".into(),
            state: SessionState::Finalized,
        },
    );
    for (i, id) in [pass0, pass1, pass2].into_iter().enumerate() {
        p.recording.passes.insert(
            id,
            RecordingPass {
                id,
                session_id,
                index: u32::try_from(i).unwrap(),
            },
        );
    }
    for (id, lane) in [(stream_a, s.lane_a), (stream_b, s.lane_b)] {
        p.recording.streams.insert(
            id,
            CaptureStream {
                id,
                session_id,
                lane_id: lane,
                clock_epoch_id: epoch,
            },
        );
    }
    // Segments: take_a0 → [0,30000) src, take_a1 → [40000,70000) src,
    // take_b → [0,1000) src (other lane).
    for (id, stream, pass, s_lo, s_hi) in [
        (seg0, stream_a, pass0, 0_i64, 30_000_i64),
        (seg1, stream_a, pass1, 40_000, 70_000),
        (seg2, stream_b, pass2, 0, 1_000),
    ] {
        p.recording.segments.insert(
            id,
            CaptureSegment {
                id,
                stream_id: stream,
                pass_id: pass,
                session_range: SessionRange::try_new(
                    SessionFrame::new(s_lo).unwrap(),
                    SessionFrame::new(s_hi).unwrap(),
                )
                .unwrap(),
                source_range: SourceRange::try_new(
                    SourceFrame::new(s_lo).unwrap(),
                    SourceFrame::new(s_hi).unwrap(),
                )
                .unwrap(),
                source_id: s.source_id,
            },
        );
    }
    for (id, pass, stream, lane, segs) in [
        (s.take_a0, pass0, stream_a, s.lane_a, vec![seg0]),
        (s.take_a1, pass1, stream_a, s.lane_a, vec![seg1]),
        (s.take_b, pass2, stream_b, s.lane_b, vec![seg2]),
    ] {
        p.recording.takes.insert(
            id,
            Take {
                id,
                session_id,
                pass_id: pass,
                stream_id: stream,
                lane_id: lane,
                source_id: s.source_id,
                segment_ids: segs,
                state: TakeState::Verified,
            },
        );
    }
}

fn mr(start: i64, end: i64) -> ProjectRange {
    ProjectRange::Musical(
        MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap(),
    )
}

fn at(pos: i64) -> ProjectTime {
    ProjectTime::Musical(MusicalTick::new(pos))
}

fn tempo_map(p: &Project) -> TickTimeMapping {
    TickTimeMapping::new(&p.tempo_map).unwrap()
}

/// Builds the canonical comp used across tests:
/// group over `lane_a`'s two takes, one comp, regions
/// `[0,960) Take(take_a0@0)`, `[960,1920) ExistingSource@2000`,
/// `[1920,2000) Silence`, active, transition `[900,1020)`.
/// Returns (group, comp, [r0, r1, r2], transition).
fn build_comp(
    c: &mut ProjectController,
    s: &Seeded,
) -> (TakeGroupId, CompId, Vec<CompRegionId>, CompTransitionId) {
    apply(
        c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_a0, s.take_a1],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    for (range, candidate) in [
        (
            mr(0, 960),
            CompCandidateRef::Take {
                take_id: s.take_a0,
                source_offset: SourceFrame::ZERO,
            },
        ),
        (
            mr(960, 1_920),
            CompCandidateRef::ExistingSource {
                source_id: s.source_id,
                handle: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(96_000).unwrap())
                    .unwrap(),
                play_from: SourceFrame::new(2_000).unwrap(),
            },
        ),
        (mr(1_920, 2_000), CompCandidateRef::Silence),
    ] {
        apply(
            c,
            CompOp::AddCompRegion {
                comp_id: comp,
                project_range: range,
                candidate,
                mapping: PlaybackMapping::Natural,
            },
        );
    }
    apply(
        c,
        CompOp::SetActiveComp {
            take_group_id: group,
            comp_id: Some(comp),
        },
    );
    let region_ids = c.live().comp.comps[&comp].region_ids.clone();
    apply(
        c,
        CompOp::AddCompTransition {
            left_region_id: region_ids[0],
            right_region_id: region_ids[1],
            overlap_range: mr(900, 1_020),
            fade_out: FadeShape::EqualPower,
            fade_in: FadeShape::Linear,
        },
    );
    let transition = *c.live().comp.transitions.keys().next().unwrap();
    (group, comp, region_ids, transition)
}

// ---------- MD-A-01: construction → reproducible projection, source immutability ----------

#[test]
fn md_a_01_comp_projection_reproducible_and_non_destructive() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (group, comp, region_ids, _transition) = build_comp(&mut c, &s);

    let proj = project_comp(c.live(), comp).unwrap();
    assert_eq!(proj.comp_id, comp);
    // Regions in order, non-overlapping, provenance per candidate.
    assert_eq!(proj.segments.len(), 3);
    assert_eq!(proj.segments[0].region_id, region_ids[0]);
    assert_eq!(proj.segments[0].provenance, CompProvenance::Take(s.take_a0));
    assert_eq!(
        proj.segments[1].provenance,
        CompProvenance::ExistingSource(s.source_id)
    );
    assert_eq!(proj.segments[2].provenance, CompProvenance::Silence);
    // Bindings: take_a0 reads [0, 960*25); existing source reads
    // [2000, 2000 + 960*25); silence has none.
    let b0 = proj.segments[0].binding.as_ref().unwrap();
    assert_eq!(b0.window.start().value(), 0);
    assert_eq!(b0.window.end().value(), 960 * FRAMES_PER_TICK);
    assert_eq!(b0.source_id, s.source_id);
    let b1 = proj.segments[1].binding.as_ref().unwrap();
    assert_eq!(b1.window.start().value(), 2_000);
    assert_eq!(b1.window.end().value(), 2_000 + 960 * FRAMES_PER_TICK);
    assert!(proj.segments[2].binding.is_none());
    // One transition with the expected boundary.
    assert_eq!(proj.transitions.len(), 1);
    assert_eq!(proj.transitions[0].boundary, at(960));

    // Reproducible: two projections are identical.
    let again = project_comp(c.live(), comp).unwrap();
    assert_eq!(proj, again);

    // Non-destructive: sources/assets/takes unchanged by projection.
    let before = (
        c.live().sources.clone(),
        c.live().assets.clone(),
        c.live().recording.takes.clone(),
    );
    let _ = project_comp(c.live(), comp).unwrap();
    assert_eq!(
        before,
        (
            c.live().sources.clone(),
            c.live().assets.clone(),
            c.live().recording.takes.clone(),
        )
    );
    // The group still points at its active comp.
    assert_eq!(c.live().comp.take_groups[&group].active_comp_id, Some(comp));

    // Save → reload → identical projection.
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut reloaded = controller(seeded().0);
    reloaded.load(&store).unwrap();
    let reloaded_proj = project_comp(reloaded.live(), comp).unwrap();
    assert_eq!(proj, reloaded_proj);
}

// ---------- MD-A-02: validation rejects, atomically ----------

#[test]
fn md_a_02_overlapping_region_rejected() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (_, comp, _, _) = build_comp(&mut c, &s);
    rejected(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(500, 700),
            candidate: CompCandidateRef::Silence,
            mapping: PlaybackMapping::Natural,
        },
    );
    // Same-domain containment is enforced too.
    rejected(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(1_900, 2_500),
            candidate: CompCandidateRef::Silence,
            mapping: PlaybackMapping::Natural,
        },
    );
}

#[test]
fn md_a_02_transition_rules_enforced() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (_, _comp, region_ids, transition) = build_comp(&mut c, &s);
    let (r0, r1, _r2) = (region_ids[0], region_ids[1], region_ids[2]);

    // Overlap not containing the boundary (960) → reject.
    rejected(
        &mut c,
        CompOp::AddCompTransition {
            left_region_id: r0,
            right_region_id: r1,
            overlap_range: mr(700, 800),
            fade_out: FadeShape::Linear,
            fade_in: FadeShape::Linear,
        },
    );
    // Overlap escaping [left.start, right.end] → reject.
    rejected(
        &mut c,
        CompOp::AddCompTransition {
            left_region_id: r0,
            right_region_id: r1,
            overlap_range: mr(950, 2_500),
            fade_out: FadeShape::Linear,
            fade_in: FadeShape::Linear,
        },
    );
    // Overlap strictly containing the boundary but colliding with the
    // existing transition [900,1020) → reject.
    rejected(
        &mut c,
        CompOp::AddCompTransition {
            left_region_id: r0,
            right_region_id: r1,
            overlap_range: mr(910, 1_030),
            fade_out: FadeShape::Linear,
            fade_in: FadeShape::Linear,
        },
    );
    // Regions from different comps → reject.
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp2 = *c.live().comp.comps.keys().next_back().unwrap();
    apply(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp2,
            project_range: mr(0, 960),
            candidate: CompCandidateRef::Silence,
            mapping: PlaybackMapping::Natural,
        },
    );
    let other = c.live().comp.comps[&comp2].region_ids[0];
    rejected(
        &mut c,
        CompOp::AddCompTransition {
            left_region_id: r0,
            right_region_id: other,
            overlap_range: mr(900, 1_000),
            fade_out: FadeShape::Linear,
            fade_in: FadeShape::Linear,
        },
    );
    // Resizing a region with an attached transition → reject.
    rejected(
        &mut c,
        CompOp::SetCompRegionRange {
            region_id: r0,
            project_range: mr(0, 500),
        },
    );
    // Removing a transition frees the region for resize.
    apply(
        &mut c,
        CompOp::RemoveCompTransition {
            transition_id: transition,
        },
    );
    apply(
        &mut c,
        CompOp::SetCompRegionRange {
            region_id: r0,
            project_range: mr(0, 500),
        },
    );
}

#[test]
fn md_a_02_non_touching_adjacent_regions_reject_transition() {
    let (p, s) = seeded();
    let mut c = controller(p);
    apply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_a0],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    for (lo, hi, off) in [(0_i64, 960_i64, 0_i64), (1_200, 2_000, 1_000)] {
        apply(
            &mut c,
            CompOp::AddCompRegion {
                comp_id: comp,
                project_range: mr(lo, hi),
                candidate: CompCandidateRef::Take {
                    take_id: s.take_a0,
                    source_offset: SourceFrame::new(off).unwrap(),
                },
                mapping: PlaybackMapping::Natural,
            },
        );
    }
    let ids = c.live().comp.comps[&comp].region_ids.clone();
    // Adjacent in order but gapped — no shared boundary → reject.
    rejected(
        &mut c,
        CompOp::AddCompTransition {
            left_region_id: ids[0],
            right_region_id: ids[1],
            overlap_range: mr(900, 1_300),
            fade_out: FadeShape::Linear,
            fade_in: FadeShape::Linear,
        },
    );
}

#[test]
fn md_a_02_take_handle_insufficiency_rejected() {
    let (p, s) = seeded();
    let mut c = controller(p);
    apply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_a0],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    // [0,100) ticks → 2_500 frames; offset 29_000 needs [29000,31500)
    // inside coverage [0,30000) → fails.
    rejected(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(0, 100),
            candidate: CompCandidateRef::Take {
                take_id: s.take_a0,
                source_offset: SourceFrame::new(29_000).unwrap(),
            },
            mapping: PlaybackMapping::Natural,
        },
    );
    // Same window at offset 27_500 fits exactly → accepted.
    apply(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(0, 100),
            candidate: CompCandidateRef::Take {
                take_id: s.take_a0,
                source_offset: SourceFrame::new(27_500).unwrap(),
            },
            mapping: PlaybackMapping::Natural,
        },
    );
}

#[test]
fn md_a_02_ownership_enforced() {
    let (p, s) = seeded();
    let mut c = controller(p);
    // take_b lives on lane_b — wrong owner → reject.
    rejected(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_a0, s.take_b],
            source_rate: RATE,
        },
    );
    let (group, comp, _, _) = build_comp(&mut c, &s);
    // Second group + comp on lane_b's take.
    apply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_b,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_b],
            source_rate: RATE,
        },
    );
    let group_b = *c.live().comp.take_groups.keys().next_back().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group_b,
        },
    );
    // SetActiveComp with a comp owned by another group → reject.
    let foreign = *c.live().comp.comps.keys().next_back().unwrap();
    rejected(
        &mut c,
        CompOp::SetActiveComp {
            take_group_id: group,
            comp_id: Some(foreign),
        },
    );
    let _ = comp;
}

#[test]
fn md_a_02_zero_rate_and_empty_target_rejected() {
    let (p, s) = seeded();
    let mut c = controller(p);
    rejected(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_a0],
            source_rate: 0,
        },
    );
    rejected(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 0),
            take_ids: vec![s.take_a0],
            source_rate: RATE,
        },
    );
    // A group needs at least one take.
    rejected(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![],
            source_rate: RATE,
        },
    );
}

// ---------- MD-A-03: anchor re-derivation across tempo-map edits ----------

fn clip_placement(anchor: i64, len: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor)),
        extent: ClipExtent::Musical(MusicalDuration::new(len).unwrap()),
    }
}

fn wide_playback() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::ZERO,
            SourceFrame::new(1_000_000).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn seeded_clips() -> (ProjectController, Seeded) {
    let (p, s) = seeded();
    (controller(p), s)
}

#[test]
fn md_a_03_musical_natural_rederives_after_tempo_edit() {
    let (mut c, s) = seeded_clips();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: s.lane_a,
            source_id: s.source_id,
            placement: clip_placement(960, 1_920),
            playback: wide_playback(),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();
    let map1 = tempo_map(c.live());
    let r1 = c.live().clips[&clip]
        .placement
        .resolve(&c.live().clips[&clip].playback, Some(&map1), RATE)
        .unwrap();
    // 120 BPM: anchor tick 960 → 24_000; end tick 2_880 → 72_000.
    assert_eq!(r1.media_span.start().value(), 24_000);
    assert_eq!(r1.media_span.end().value(), 72_000);
    assert_eq!(r1.source_start.value(), 0);

    // Halve tempo from tick 480 on: 60 BPM → 50 frames/tick there.
    apply(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 480,
            bpm_milli: 60_000,
        },
    );
    let map2 = tempo_map(c.live());
    let r2 = c.live().clips[&clip]
        .placement
        .resolve(&c.live().clips[&clip].playback, Some(&map2), RATE)
        .unwrap();
    // anchor tick 960 → 480*25 + 480*50 = 36_000; end tick 2_880 →
    // 480*25 + 2_400*50 = 132_000.
    assert_eq!(r2.media_span.start().value(), 36_000);
    assert_eq!(r2.media_span.end().value(), 132_000);
    // Source Native Frame is invariant — the saved anchor/extent never
    // rewrote the read window.
    assert_eq!(r2.source_start, r1.source_start);
    assert_eq!(
        c.live().clips[&clip].playback.source_region,
        SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(1_000_000).unwrap()).unwrap()
    );
}

#[test]
fn md_a_03_absolute_natural_invariant_across_tempo_edit() {
    let (mut c, s) = seeded_clips();
    let placement = ClipPlacement {
        anchor: TimeAnchor::Absolute(MediaTimeValue::new(24_000, 1, RATE).unwrap()),
        extent: ClipExtent::Absolute(MediaDuration::new(48_000, 1, RATE).unwrap()),
    };
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: s.lane_a,
            source_id: s.source_id,
            placement,
            playback: wide_playback(),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();
    let map1 = tempo_map(c.live());
    let r1 = placement
        .resolve(&c.live().clips[&clip].playback, Some(&map1), RATE)
        .unwrap();
    apply(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 480,
            bpm_milli: 60_000,
        },
    );
    let map2 = tempo_map(c.live());
    let r2 = c.live().clips[&clip]
        .placement
        .resolve(&c.live().clips[&clip].playback, Some(&map2), RATE)
        .unwrap();
    // Absolute anchor+extent never move in media time.
    assert_eq!(r2.media_span, r1.media_span);
    assert_eq!(r2.source_start.value(), 0);
    // But the musical reading re-derives through the new map.
    let m1 = placement.resolve_musical_range(&map1, RATE).unwrap();
    let m2 = c.live().clips[&clip]
        .placement
        .resolve_musical_range(&map2, RATE)
        .unwrap();
    assert_ne!(m1.end(), m2.end(), "musical view must re-derive");
}

#[test]
fn md_a_03_mixed_anchor_extent_rederives() {
    let (mut c, s) = seeded_clips();
    // Musical anchor + absolute extent: anchor tick is authority; the
    // media end follows the re-derived anchor position.
    let placement = ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(960)),
        extent: ClipExtent::Absolute(MediaDuration::new(24_000, 1, RATE).unwrap()),
    };
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: s.lane_a,
            source_id: s.source_id,
            placement,
            playback: wide_playback(),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();
    let r1 = c.live().clips[&clip]
        .placement
        .resolve(
            &c.live().clips[&clip].playback,
            Some(&tempo_map(c.live())),
            RATE,
        )
        .unwrap();
    assert_eq!(
        (r1.media_span.start().value(), r1.media_span.end().value()),
        (24_000, 48_000)
    );
    apply(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 480,
            bpm_milli: 60_000,
        },
    );
    let r2 = c.live().clips[&clip]
        .placement
        .resolve(
            &c.live().clips[&clip].playback,
            Some(&tempo_map(c.live())),
            RATE,
        )
        .unwrap();
    // Anchor tick 960 → 36_000 under the new map; the 24_000-frame
    // absolute extent rides on top of the re-derived anchor.
    assert_eq!(
        (r2.media_span.start().value(), r2.media_span.end().value()),
        (36_000, 60_000)
    );
    assert_eq!(r2.source_start, r1.source_start);
}

#[test]
fn md_a_03_stretch_consumes_at_ratio() {
    let (mut c, s) = seeded_clips();
    let playback = ClipPlayback {
        source_region: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(200_000).unwrap())
            .unwrap(),
        mapping: PlaybackMapping::SimpleStretch(StretchRatio::new(2, 1).unwrap()),
    };
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: s.lane_a,
            source_id: s.source_id,
            placement: clip_placement(0, 960),
            playback,
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();
    let r1 = c.live().clips[&clip]
        .placement
        .resolve(
            &c.live().clips[&clip].playback,
            Some(&tempo_map(c.live())),
            RATE,
        )
        .unwrap();
    // 960 ticks → 24_000 media frames → 48_000 source frames at 2:1.
    assert_eq!(
        r1.media_span.end().value() - r1.media_span.start().value(),
        24_000
    );
    assert_eq!(r1.source_len, 48_000);
    apply(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 480,
            bpm_milli: 60_000,
        },
    );
    let r2 = c.live().clips[&clip]
        .placement
        .resolve(
            &c.live().clips[&clip].playback,
            Some(&tempo_map(c.live())),
            RATE,
        )
        .unwrap();
    // Media span re-derived; consumed keeps the 2:1 ratio and the
    // window start never moved.
    let media_len = r2.media_span.end().value() - r2.media_span.start().value();
    assert_eq!(r2.source_len, u64::try_from(media_len).unwrap() * 2);
    assert_eq!(r2.source_start.value(), 0);
}

#[test]
fn md_a_03_move_and_extent_ops_keep_contract() {
    let (mut c, s) = seeded_clips();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: s.lane_a,
            source_id: s.source_id,
            placement: clip_placement(0, 960),
            playback: wide_playback(),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();
    // MoveClip changes only the anchor — extent/playback untouched.
    apply(
        &mut c,
        ProjectOp::MoveClip {
            clip_id: clip,
            anchor: TimeAnchor::Musical(MusicalTick::new(480)),
        },
    );
    let cl = &c.live().clips[&clip];
    assert_eq!(
        cl.placement.anchor,
        TimeAnchor::Musical(MusicalTick::new(480))
    );
    assert_eq!(cl.playback, wide_playback());
    // Empty extent rejected.
    let rev = c.live_revision();
    assert!(
        try_apply(
            &mut c,
            ProjectOp::SetClipExtent {
                clip_id: clip,
                extent: ClipExtent::Musical(MusicalDuration::new(0).unwrap()),
            },
        )
        .is_err()
    );
    assert_eq!(c.live_revision(), rev);
    // Musical extent: op-time validation checks only a non-empty
    // window — fit is resolved at consume time under a real rate.
    apply(
        &mut c,
        ProjectOp::SetClipPlayback {
            clip_id: clip,
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(500).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    // At consume time the too-small window surfaces: 24_000 media
    // frames demanded from a 500-frame window → resolve fails.
    assert!(
        c.live().clips[&clip]
            .placement
            .resolve(
                &c.live().clips[&clip].playback,
                Some(&tempo_map(c.live())),
                RATE
            )
            .is_err()
    );
    // Absolute extent: the declared-base window check runs at op
    // time — extent 960 declared-base frames vs window 500 → reject.
    let rev = c.live_revision();
    assert!(
        try_apply(
            &mut c,
            ProjectOp::SetClipExtent {
                clip_id: clip,
                extent: ClipExtent::Absolute(MediaDuration::new(960, 1, RATE).unwrap()),
            },
        )
        .is_err()
    );
    assert_eq!(c.live_revision(), rev);
}

// ---------- MD-A-04: half-open boundary ownership, bounded double-play ----------

#[test]
fn md_a_04_boundary_positions_belong_to_right_region() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (_, comp, region_ids, transition) = build_comp(&mut c, &s);
    let proj = project_comp(c.live(), comp).unwrap();

    // Tick 959 → left region; 960 (the boundary) → right region only.
    assert_eq!(proj.segment_at(&at(959)).unwrap().region_id, region_ids[0]);
    assert_eq!(proj.segment_at(&at(960)).unwrap().region_id, region_ids[1]);
    // Inside a gap beyond all regions → None.
    assert!(proj.segment_at(&at(2_000)).is_none());
    // Overlap zone: inside → exactly the one transition; outside → none.
    assert_eq!(
        proj.transition_at(&at(900)).unwrap().transition_id,
        transition
    );
    assert_eq!(
        proj.transition_at(&at(1_019)).unwrap().transition_id,
        transition
    );
    assert!(proj.transition_at(&at(1_020)).is_none());
    assert!(proj.transition_at(&at(899)).is_none());

    // Bounded double-play: the left read extends only to overlap.end,
    // the right read starts only at overlap.start — no overhang
    // escapes the zone.
    let tr = &proj.transitions[0];
    let left = tr.left.as_ref().unwrap();
    let right = tr.right.as_ref().unwrap();
    // Base windows: left [0, 24000), right [2000, 26000).
    // Post-handle (960→1020 = 60 ticks → 1500 frames): left → 25_500.
    assert_eq!(left.window.start().value(), 0);
    assert_eq!(left.window.end().value(), 24_000 + 60 * FRAMES_PER_TICK);
    // Pre-handle (900→960 = 60 ticks → 1500 frames): right → 500.
    assert_eq!(right.window.start().value(), 2_000 - 60 * FRAMES_PER_TICK);
    assert_eq!(right.window.end().value(), 26_000);
    // Both stay inside their candidate handles (validated at apply).
    let p = c.live();
    assert!(
        p.comp.transitions[&transition]
            .overlap_range
            .contains_time(&at(960))
    );
}

// ---------- MD-A-05: undo/redo + persistence ----------

#[test]
fn md_a_05_undo_redo_restores_ids_and_state() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (group, comp, region_ids, transition) = build_comp(&mut c, &s);
    let built = c.live().comp.clone();

    // Seven ops built the comp — undo all of them.
    for _ in 0..7 {
        assert!(c.undo().unwrap());
    }
    assert_eq!(c.live().comp, CompRegistry::default());
    assert!(c.live().comp.take_groups.is_empty());

    // Redo restores the same IDs (deterministic generator reseed).
    for _ in 0..7 {
        assert!(c.redo().unwrap());
    }
    assert_eq!(c.live().comp, built);
    assert!(c.live().comp.take_groups.contains_key(&group));
    assert!(c.live().comp.comps.contains_key(&comp));
    for r in &region_ids {
        assert!(c.live().comp.regions.contains_key(r));
    }
    assert!(c.live().comp.transitions.contains_key(&transition));
}

#[test]
fn md_a_05_schema_version_and_projection_stable_across_reload() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (_, comp, _, _) = build_comp(&mut c, &s);

    let mut store = InMemoryStore::new();
    let saved = c.save(&mut store).unwrap();
    let bytes = store.read_snapshot(&saved.head).unwrap();
    let env: SnapshotEnvelope = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(env.schema_version, SCHEMA_VERSION);
    assert_eq!(env.schema_version, 2);

    let mut reloaded = controller(seeded().0);
    reloaded.load(&store).unwrap();
    assert_eq!(
        project_comp(c.live(), comp).unwrap(),
        project_comp(reloaded.live(), comp).unwrap()
    );
    // And the reloaded registry itself is identical.
    assert_eq!(c.live().comp, reloaded.live().comp);
}

// ---------- MD-A-06: editor target type contracts ----------

#[test]
fn md_a_06_editor_targets_are_type_level_distinct() {
    let (_, s) = seeded();
    let comp_id = CompId::new(42);

    // Raw capture (version 0) is not addressable as an editable source.
    assert!(SourceVersionRef::new(s.source_id, 0).is_err());
    let derived = SourceVersionRef::new(s.source_id, 1).unwrap();

    // Editing targets: comp arrangement vs editable derived source.
    let edit_comp = EditorEditingTarget::Comp(comp_id);
    let edit_src = EditorEditingTarget::EditableSource(derived);
    assert_ne!(edit_comp, edit_src);
    assert!(matches!(edit_comp, EditorEditingTarget::Comp(_)));
    assert!(matches!(edit_src, EditorEditingTarget::EditableSource(_)));

    // Display targets: the three agreed views.
    let views = [
        EditorDisplayTarget::EffectiveComp(comp_id),
        EditorDisplayTarget::TakeSource(s.take_a0),
        EditorDisplayTarget::Compare(comp_id),
    ];
    assert!(matches!(views[0], EditorDisplayTarget::EffectiveComp(_)));
    assert!(matches!(views[1], EditorDisplayTarget::TakeSource(_)));
    assert!(matches!(views[2], EditorDisplayTarget::Compare(_)));

    // Persisted form rejects the raw capture too (serde try_from).
    let raw_json = serde_json::json!({
        "source_id": s.source_id.to_tagged(),
        "version": 0_u64,
    });
    assert!(serde_json::from_value::<SourceVersionRef>(raw_json).is_err());
    let ok_json = serde_json::json!({
        "source_id": s.source_id.to_tagged(),
        "version": 1_u64,
    });
    assert_eq!(
        serde_json::from_value::<SourceVersionRef>(ok_json).unwrap(),
        derived
    );
}

// ---------- review regressions: confirmed P1/P2 findings ----------

/// P1: a tempo-map edit that would leave a comp region's source
/// window outside its take handle is rejected at the controller
/// boundary. Before the post-apply `validate_references` check, the op
/// committed a snapshot the load oracle itself would reject.
#[test]
fn regression_tempo_edit_cannot_brick_comp_handles() {
    let (p, s) = seeded();
    let mut c = controller(p);
    apply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_a0],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    // [0,960) ticks @120 BPM consumes 24_000 frames from offset 5_000
    // → [5_000, 29_000) fits inside take_a0's [0, 30_000) coverage.
    apply(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(0, 960),
            candidate: CompCandidateRef::Take {
                take_id: s.take_a0,
                source_offset: SourceFrame::new(5_000).unwrap(),
            },
            mapping: PlaybackMapping::Natural,
        },
    );
    // Halving tempo at tick 480 makes the same region consume 36_000
    // frames — the window no longer fits the handle → rejected with
    // the pre-edit state intact (revision unchanged, registry valid).
    let rev = c.live_revision();
    assert!(
        try_apply(
            &mut c,
            ProjectOp::AddTempoEvent {
                tick: 480,
                bpm_milli: 60_000,
            }
        )
        .is_err()
    );
    assert_eq!(c.live_revision(), rev);
    synvelis_project::comp::validate_registry(c.live()).unwrap();
}

/// P1: zero time-base/ratio components are rejected by constructors
/// *and* by the serde wire form — a crafted snapshot cannot smuggle a
/// `den = 0` value that later divides by zero.
#[test]
fn regression_zero_denominator_rejected() {
    assert!(MediaTimeValue::new(0, 0, 1).is_err());
    assert!(MediaTimeValue::new(0, 1, 0).is_err());
    assert!(MediaDuration::new(1, 1, 0).is_err());
    assert!(MediaDuration::new(1, 0, 1).is_err());
    assert!(StretchRatio::new(0, 1).is_err());
    assert!(StretchRatio::new(1, 0).is_err());

    let bad_time = serde_json::json!({
        "value": 1_i64, "numerator": 1_u32, "denominator": 0_u32,
    });
    assert!(serde_json::from_value::<MediaTimeValue>(bad_time).is_err());
    let bad_dur = serde_json::json!({
        "value": 1_i64, "numerator": 0_u32, "denominator": 1_u32,
    });
    assert!(serde_json::from_value::<MediaDuration>(bad_dur).is_err());
    let bad_ratio = serde_json::json!({
        "numerator": 1_u32, "denominator": 0_u32,
    });
    assert!(serde_json::from_value::<StretchRatio>(bad_ratio).is_err());
}

/// P2: a `Loop` region projects the containing handle interval as its
/// repeat-unit window — not an unbounded `[play_from, +len)` read.
#[test]
fn regression_loop_window_is_the_repeat_unit() {
    let (p, s) = seeded();
    let mut c = controller(p);
    apply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_a0],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(0, 100),
            candidate: CompCandidateRef::Take {
                take_id: s.take_a0,
                source_offset: SourceFrame::new(15_000).unwrap(),
            },
            mapping: PlaybackMapping::Loop,
        },
    );
    let eff = project_comp(c.live(), comp).unwrap();
    let window = eff.segments[0].binding.as_ref().unwrap().window;
    assert_eq!(
        window,
        SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(30_000).unwrap()).unwrap(),
        "loop repeat unit is the containing handle interval, not play_from + extent"
    );
}

/// P2: `Loop` still requires `play_from` inside a readable handle
/// interval — the overhang waiver never waived containment.
#[test]
fn regression_loop_play_from_outside_handle_rejected() {
    let (p, s) = seeded();
    let mut c = controller(p);
    apply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_a0],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    // take_a0 covers [0, 30_000) — offset 35_000 is outside.
    rejected(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(0, 100),
            candidate: CompCandidateRef::Take {
                take_id: s.take_a0,
                source_offset: SourceFrame::new(35_000).unwrap(),
            },
            mapping: PlaybackMapping::Loop,
        },
    );
}

fn med_range(v0: i64, n0: u32, d0: u32, v1: i64, n1: u32, d1: u32) -> ProjectRange {
    ProjectRange::Media(
        MediaSpan::try_new(
            MediaTimeValue::new(v0, n0, d0).unwrap(),
            MediaTimeValue::new(v1, n1, d1).unwrap(),
        )
        .unwrap(),
    )
}

/// P2: `region_ids` sort order uses absolute media time (`lt`), not
/// the raw `value` — two bases can name the same axis.
#[test]
fn regression_media_region_sort_uses_time_not_raw_value() {
    let (p, s) = seeded();
    let mut c = controller(p);
    apply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: med_range(0, 1, 1, 10, 1, 1),
            take_ids: vec![s.take_a0],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    let candidate = || CompCandidateRef::ExistingSource {
        source_id: s.source_id,
        handle: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(9_000_000).unwrap())
            .unwrap(),
        play_from: SourceFrame::ZERO,
    };
    // Insert the later region first: [3,4) at base 1/1 = time 3..4.
    apply(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: med_range(3, 1, 1, 4, 1, 1),
            candidate: candidate(),
            mapping: PlaybackMapping::Natural,
        },
    );
    // Then [4,5) at base 1/2 = time 2.0..2.5 — raw values are larger
    // (4 > 3) but the absolute position is earlier.
    apply(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: med_range(4, 1, 2, 5, 1, 2),
            candidate: candidate(),
            mapping: PlaybackMapping::Natural,
        },
    );
    let order = &c.live().comp.comps[&comp].region_ids;
    let starts: Vec<ProjectTime> = order
        .iter()
        .map(|id| c.live().comp.regions[id].project_range.start_time())
        .collect();
    assert!(
        matches!(starts[0], ProjectTime::Media(v) if v.value() == 4 && v.denominator() == 2),
        "the 1/2-base region (time 2.0) must sort first: {starts:?}"
    );
}

/// P2: the registry oracle also catches corruption ops can't produce —
/// key↔id mismatches and entities missing from their comp's listings
/// (e.g. a hand-edited snapshot).
#[test]
fn regression_registry_oracle_catches_corrupt_state() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (group_id, comp, region_ids, transition) = build_comp(&mut c, &s);
    let base = c.live().clone();

    // Key does not match the stored entity's id.
    let mut bad_key = base.clone();
    let group = bad_key.comp.take_groups[&group_id].clone();
    bad_key.comp.take_groups.remove(&group_id);
    bad_key
        .comp
        .take_groups
        .insert(TakeGroupId::new(9_999), group);
    assert!(synvelis_project::comp::validate_registry(&bad_key).is_err());

    // Region in the map but not listed in comp.region_ids.
    let mut unlisted = base.clone();
    let dropped = unlisted
        .comp
        .comps
        .get_mut(&comp)
        .unwrap()
        .region_ids
        .pop()
        .unwrap();
    assert!(region_ids.contains(&dropped));
    assert!(synvelis_project::comp::validate_registry(&unlisted).is_err());

    // Transition in the map but not listed in comp.transition_ids.
    let mut unlisted_t = base.clone();
    unlisted_t
        .comp
        .comps
        .get_mut(&comp)
        .unwrap()
        .transition_ids
        .retain(|id| *id != transition);
    assert!(synvelis_project::comp::validate_registry(&unlisted_t).is_err());
}

/// P2: window-end arithmetic is checked — a handle near `i64::MAX`
/// yields `Error`, not an overflow panic or a wrapped window.
#[test]
fn regression_window_end_overflow_is_error_not_panic() {
    let (p, s) = seeded();
    let mut c = controller(p);
    apply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: s.lane_a,
            target_range: mr(0, 2_000),
            take_ids: vec![s.take_a0],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    rejected(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(0, 100),
            candidate: CompCandidateRef::ExistingSource {
                source_id: s.source_id,
                handle: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(i64::MAX).unwrap(),
                )
                .unwrap(),
                play_from: SourceFrame::new(i64::MAX - 10).unwrap(),
            },
            mapping: PlaybackMapping::Natural,
        },
    );
}
