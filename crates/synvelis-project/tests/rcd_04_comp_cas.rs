//! RCD-04 comp fixture — Group Comp CAS / all-or-nothing /
//! Undo-Redo / Save-Reload / serialized-diff stable IDs / raw
//! take-source immutability (synvelis#118 crate fixture slice).
//!
//! Headless model `PoC`: only `ProjectController` + `CompOp` drive
//! mutations; `project_comp` is the pure read. Takes/segments are
//! seeded as POD directly (recording-side ops live in
//! `synvelis-recording`). No real device/GUI/OS claims.

use synvelis_project::*;

const RATE: u32 = 48_000;
/// 120 BPM @ PPQN 960 → 25 frames per tick at 48 kHz.
const FRAMES_PER_TICK: i64 = 25;

// ---------- helpers (md_a.rs conventions) ----------

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

/// A project seeded like `md_a.rs`: one track, two lanes, one source,
/// three takes — `take_a0`/`take_a1` on `lane_a`, `take_b` on
/// `lane_b`, all reading the same `Source`.
struct Seeded {
    lane_a: LaneId,
    lane_b: LaneId,
    take_a0: TakeId,
    take_a1: TakeId,
    take_b: TakeId,
    source_id: SourceId,
}

fn seeded() -> (Project, Seeded) {
    let mut p = Project::new(ProjectId::new(0), "rcd-04-comp");
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

/// Builds group → comp → 3 regions → active → transition.
/// Returns (`group`, `comp`, `region_ids`, `transition_id`).
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

fn comp_json(p: &Project) -> serde_json::Value {
    serde_json::to_value(&p.comp).unwrap()
}

// ---------------------------------------------------------------------
// CAS / all-or-nothing
// ---------------------------------------------------------------------

#[test]
fn comp_group_cas_rejects_stale_revision_without_partial_apply() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (_group, _comp, region_ids, _transition) = build_comp(&mut c, &s);
    let live = c.live_revision();
    let comp_before = comp_json(c.live());
    let watermark_before = c.live().id_watermark;

    // Stale expected revision: rejected, nothing advances.
    let err = c
        .apply(Command {
            expected_revision: ProjectRevision(live.0 - 1),
            op: CompOp::RemoveCompRegion {
                region_id: region_ids[2],
            },
        })
        .unwrap_err();
    assert!(matches!(
        err,
        Error::StaleRevision { .. } | Error::Validation(_)
    ));
    assert_eq!(c.live_revision(), live);
    assert_eq!(comp_json(c.live()), comp_before);
    assert_eq!(c.live().id_watermark, watermark_before);
}

#[test]
fn comp_invalid_ops_are_all_or_nothing() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (_group, comp, region_ids, _t) = build_comp(&mut c, &s);
    let comp_before = comp_json(c.live());

    // Overlapping region: rejected, comp registry untouched.
    rejected(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(500, 1_500), // overlaps existing regions
            candidate: CompCandidateRef::Silence,
            mapping: PlaybackMapping::Natural,
        },
    );
    assert_eq!(comp_json(c.live()), comp_before);

    // Transition between non-adjacent regions: rejected.
    rejected(
        &mut c,
        CompOp::AddCompTransition {
            left_region_id: region_ids[0],
            right_region_id: region_ids[2], // not touching
            overlap_range: mr(0, 100),
            fade_out: FadeShape::Linear,
            fade_in: FadeShape::Linear,
        },
    );
    assert_eq!(comp_json(c.live()), comp_before);

    // Candidate referencing a take on another lane's group context:
    // take_b belongs to lane_b, not this take group → rejected.
    rejected(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(2_000, 2_100),
            candidate: CompCandidateRef::Take {
                take_id: s.take_b,
                source_offset: SourceFrame::ZERO,
            },
            mapping: PlaybackMapping::Natural,
        },
    );
    assert_eq!(comp_json(c.live()), comp_before);

    // Nonexistent take id → rejected.
    rejected(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(2_000, 2_100),
            candidate: CompCandidateRef::Take {
                take_id: TakeId::new(999),
                source_offset: SourceFrame::ZERO,
            },
            mapping: PlaybackMapping::Natural,
        },
    );
    assert_eq!(comp_json(c.live()), comp_before);
}

// ---------------------------------------------------------------------
// Undo/redo + save/reload + serialized diff — stable IDs
// ---------------------------------------------------------------------

#[test]
fn comp_ids_survive_undo_redo_save_reload_and_diff() {
    let (p, s) = seeded();
    let mut c = controller(p);
    let (group, comp, region_ids, transition) = build_comp(&mut c, &s);

    let live_json = serde_json::to_string(c.live()).unwrap();

    // Undo the transition → comp state rolls back exactly.
    assert!(c.undo().unwrap());
    assert!(c.live().comp.transitions.is_empty());
    // Redo → identical IDs come back (stable, not re-minted).
    assert!(c.redo().unwrap());
    assert!(c.live().comp.transitions.contains_key(&transition));
    assert_eq!(
        c.live().comp.comps[&comp].region_ids,
        region_ids,
        "redo must restore identical region IDs"
    );
    assert!(c.live().comp.take_groups.contains_key(&group));

    // Save → load into a fresh controller: all stable IDs and the
    // canonical serialization are identical (the VCS-diff surface).
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller(Project::new(ProjectId::new(0), "rcd-04-comp"));
    c2.load(&store).unwrap();
    assert_eq!(
        serde_json::to_string(c2.live()).unwrap(),
        live_json,
        "save/reload must reproduce identical project state"
    );
    let p2 = c2.live();
    assert!(p2.comp.take_groups.contains_key(&group));
    assert!(p2.comp.comps.contains_key(&comp));
    for rid in &region_ids {
        assert!(p2.comp.regions.contains_key(rid));
    }
    assert!(p2.comp.transitions.contains_key(&transition));
    for tid in [s.take_a0, s.take_a1, s.take_b] {
        assert!(p2.recording.takes.contains_key(&tid));
    }
    assert!(p2.sources.contains_key(&s.source_id));

    // Serialized-diff equivalence: an unrelated op changes only its
    // own field — comp/take/source IDs are byte-identical in the
    // canonical JSON.
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    );
    let v1: serde_json::Value = serde_json::from_str(&live_json).unwrap();
    let v2: serde_json::Value = serde_json::to_value(c.live()).unwrap();
    assert_eq!(v1["comp"], v2["comp"], "comp section must diff-clean");
    assert_eq!(
        v1["recording"]["takes"], v2["recording"]["takes"],
        "takes must diff-clean"
    );
    assert_eq!(v1["sources"], v2["sources"], "sources must diff-clean");
    assert_ne!(v1["name"], v2["name"], "the rename must be visible");
}

/// RCD-04 residual (synvelis#118): an active-comp switch is ONE
/// `ProjectController` CAS step with ONE undo — `SetActiveComp`
/// flips `take_group.active_comp_id` and nothing else, so a single
/// `undo()` restores the exact pre-switch state and `redo()`
/// re-applies it without re-minting IDs. Raw takes/sources/assets
/// stay byte-identical across the whole switch sequence.
#[test]
fn comp_switch_is_single_cas_undo_and_leaves_raw_immutable() {
    let (p, s) = seeded();
    let takes_before = p.recording.takes.clone();
    let sources_before = p.sources.clone();
    let assets_before = p.assets.clone();
    let mut c = controller(p);
    let (group, comp1, _region_ids, _transition) = build_comp(&mut c, &s);

    // Second comp on the same group — the switch target. One adopted
    // region makes the switch substantive rather than a pointer flip
    // between an empty shell.
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp2 = *c
        .live()
        .comp
        .comps
        .keys()
        .find(|id| **id != comp1)
        .expect("second comp created");
    apply(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp2,
            project_range: mr(0, 960),
            candidate: CompCandidateRef::Silence,
            mapping: PlaybackMapping::Natural,
        },
    );

    let rev_before = c.live_revision();
    let comp_json_before = comp_json(c.live());
    assert_eq!(
        c.live().comp.take_groups[&group].active_comp_id,
        Some(comp1)
    );

    // comp1 → comp2 in ONE apply: exactly one revision step.
    apply(
        &mut c,
        CompOp::SetActiveComp {
            take_group_id: group,
            comp_id: Some(comp2),
        },
    );
    assert_eq!(c.live_revision().0, rev_before.0 + 1);
    assert_eq!(
        c.live().comp.take_groups[&group].active_comp_id,
        Some(comp2)
    );
    // The switch touches only the active pointer — comp contents,
    // regions, and transitions are untouched.
    let comp_json_switched = comp_json(c.live());
    assert_eq!(
        comp_json_switched["comps"], comp_json_before["comps"],
        "comp registry must be identical across an activation switch"
    );
    assert_eq!(comp_json_switched["regions"], comp_json_before["regions"],);
    assert_eq!(
        comp_json_switched["transitions"],
        comp_json_before["transitions"],
    );
    // take_groups too — every field except the active pointer must be
    // identical (take_ids, target_range, source_rate, owner_lane_id).
    let mask_groups = |v: &serde_json::Value| {
        let mut v = v.clone();
        for g in v["take_groups"].as_object_mut().unwrap().values_mut() {
            g["active_comp_id"] = serde_json::Value::Null;
        }
        v["take_groups"].clone()
    };
    assert_eq!(
        mask_groups(&comp_json_switched),
        mask_groups(&comp_json_before),
        "only active_comp_id may differ inside take_groups"
    );

    // ONE undo restores the exact pre-switch project — no cascade.
    // (Undo issues a new live revision by design; the pin is that a
    // single undo fully restores the pre-switch content.)
    let rev_switched = c.live_revision();
    assert!(c.undo().unwrap());
    assert_eq!(c.live_revision().0, rev_switched.0 + 1);
    assert_eq!(comp_json(c.live()), comp_json_before);
    assert_eq!(
        c.live().comp.take_groups[&group].active_comp_id,
        Some(comp1)
    );

    // Redo re-applies the identical switch — the whole comp state is
    // restored byte-for-byte, IDs included (not re-minted).
    let rev_undone = c.live_revision();
    assert!(c.redo().unwrap());
    assert_eq!(c.live_revision().0, rev_undone.0 + 1);
    assert_eq!(comp_json(c.live()), comp_json_switched);
    assert_eq!(
        c.live().comp.take_groups[&group].active_comp_id,
        Some(comp2)
    );
    assert!(c.live().comp.comps.contains_key(&comp2));

    // Raw takes/sources/assets are immutable across the sequence —
    // comp switching never rewrites recorded material.
    let p = c.live();
    assert_eq!(p.recording.takes, takes_before);
    assert_eq!(p.sources, sources_before);
    assert_eq!(p.assets, assets_before);
    for t in p.recording.takes.values() {
        assert_eq!(t.state, TakeState::Verified);
    }
}

// ---------------------------------------------------------------------
// Immutability + projection determinism
// ---------------------------------------------------------------------

#[test]
fn comp_ops_never_mutate_takes_sources_or_assets() {
    let (p, s) = seeded();
    let takes_before = p.recording.takes.clone();
    let sources_before = p.sources.clone();
    let assets_before = p.assets.clone();
    let mut c = controller(p);
    build_comp(&mut c, &s);
    // Undo everything back to the seeded state and compare PODs.
    while c.undo().unwrap() {}
    let p = c.live();
    assert_eq!(p.recording.takes, takes_before);
    assert_eq!(p.sources, sources_before);
    assert_eq!(p.assets, assets_before);
    // Take states stay Verified — comp adoption never rewrites them.
    for t in p.recording.takes.values() {
        assert_eq!(t.state, TakeState::Verified);
    }
}

#[test]
fn project_comp_is_deterministic_and_preserves_provenance() {
    let (project, seed) = seeded();
    let mut ctl = controller(project);
    let (_group, comp_id, region_ids, _transition) = build_comp(&mut ctl, &seed);

    let first = project_comp(ctl.live(), comp_id).unwrap();
    let second = project_comp(ctl.live(), comp_id).unwrap();
    assert_eq!(first, second, "projection must be reproducible");

    assert_eq!(first.segments.len(), 3);
    assert_eq!(
        first.segments[0].provenance,
        CompProvenance::Take(seed.take_a0)
    );
    assert_eq!(
        first.segments[1].provenance,
        CompProvenance::ExistingSource(seed.source_id)
    );
    assert_eq!(first.segments[2].provenance, CompProvenance::Silence);
    assert_eq!(first.segments[0].region_id, region_ids[0]);

    // Transition projection: boundary + overlap + extended bindings.
    assert_eq!(first.transitions.len(), 1);
    let tr = &first.transitions[0];
    assert_eq!(tr.fade_out, FadeShape::EqualPower);
    assert_eq!(tr.fade_in, FadeShape::Linear);
    assert!(tr.left.is_some() && tr.right.is_some());

    // Sanity on frame math used by the PCM fixture downstream:
    // 960 project ticks @25 frames/tick = 24_000 media frames.
    let b0 = first.segments[0].binding.as_ref().unwrap();
    assert_eq!(b0.window.start().value(), 0);
    assert_eq!(b0.window.end().value(), 960 * FRAMES_PER_TICK);
}
