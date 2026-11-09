//! Issue #52: minimal MIDI source / note editing contract — stable
//! IDs, linked sources vs explicit detach, undo/redo, save/reload,
//! reference validation, and native-tick time fidelity.

use synvelis_project::*;

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: ProjectOp) -> Command<ProjectOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

fn apply(c: &mut ProjectController, op: ProjectOp) {
    c.apply(cmd(c, op)).unwrap();
}

fn midi_window() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(4 * 960).unwrap())
            .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn musical_clip(anchor_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap()),
    }
}

/// Track → lane → MIDI source (division 480) → two linked clips.
/// Returns `(lane, source, [c1, c2])`.
fn linked_fixture(c: &mut ProjectController) -> (LaneId, SourceId, Vec<ClipId>) {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(
        c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let source = *c.live().midi_sources.keys().next().unwrap();
    for anchor in [0, 4 * 960] {
        apply(
            c,
            ProjectOp::AddClip {
                lane_id: lane,
                source_id: source,
                placement: musical_clip(anchor),
                playback: midi_window(),
            },
        );
    }
    let clips = c.live().clips.keys().copied().collect();
    (lane, source, clips)
}

fn only_note(c: &ProjectController, source: SourceId) -> MidiNote {
    c.live().midi_sources[&source]
        .notes
        .values()
        .next()
        .unwrap()
        .clone()
}

// ---------------------------------------------------------------------
// Readable types + versioned serialization + clip/source references
// ---------------------------------------------------------------------

#[test]
fn midi_source_types_are_readable_and_serialize_with_version() {
    let mut c = controller();
    let (_lane, source, clips) = linked_fixture(&mut c);
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 60,
            velocity: 100,
            start: MidiTick::new(48).unwrap(),
            len: MidiTick::new(96).unwrap(),
        },
    );
    let note = only_note(&c, source);
    assert_eq!(note.key, 60);
    assert_eq!(note.velocity, 100);
    // Half-open [start, end) boundary.
    assert_eq!(note.end().unwrap(), MidiTick::new(144).unwrap());
    let range = note.range().unwrap();
    assert!(range.contains(MidiTick::new(48).unwrap()));
    assert!(!range.contains(MidiTick::new(144).unwrap()));

    // Both clips reference the source through the shared SourceId space.
    assert!(c.live().contains_source(&source));
    for clip in &clips {
        assert_eq!(c.live().clips[clip].source_id, source);
    }

    // Versioned serialization: the envelope carries the current schema.
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let bytes = store
        .read_snapshot(&store.head().unwrap().unwrap())
        .unwrap();
    let env: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(env["schema_version"], serde_json::json!(SCHEMA_VERSION));
    // The serialized source keeps its native division and note data.
    let s = &env["project"]["midi_sources"];
    let (sid, body) = s.as_object().unwrap().iter().next().unwrap();
    assert_eq!(body["division"]["value"], serde_json::json!(480));
    assert!(sid.starts_with("source-"));
}

// ---------------------------------------------------------------------
// SourceId+NoteId+ExpectedRevision addressing; rejection contract
// ---------------------------------------------------------------------

#[test]
#[allow(clippy::too_many_lines)] // sequential rejection matrix
fn note_edits_reject_stale_revision_bad_target_and_out_of_range() {
    let mut c = controller();
    let (_lane, source, _clips) = linked_fixture(&mut c);
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 60,
            velocity: 100,
            start: MidiTick::new(0).unwrap(),
            len: MidiTick::new(120).unwrap(),
        },
    );
    let note = only_note(&c, source);

    // Stale expected revision: untouched project and revision.
    let rev = c.live_revision();
    let stale = Command {
        expected_revision: ProjectRevision(999),
        op: ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id: note.id,
            velocity: 50,
        },
    };
    assert!(matches!(c.apply(stale), Err(Error::StaleRevision { .. })));
    assert_eq!(c.live_revision(), rev);
    assert_eq!(only_note(&c, source).velocity, 100);

    // Invalid targets: missing source, missing note, missing clip.
    let bad_source = SourceId::new(0xdead);
    for op in [
        ProjectOp::AddMidiNote {
            source_id: bad_source,
            key: 1,
            velocity: 1,
            start: MidiTick::ZERO,
            len: MidiTick::new(1).unwrap(),
        },
        ProjectOp::SetMidiNoteVelocity {
            source_id: bad_source,
            note_id: note.id,
            velocity: 1,
        },
        ProjectOp::MoveMidiNote {
            source_id: bad_source,
            note_id: note.id,
            start: MidiTick::ZERO,
        },
        ProjectOp::RemoveMidiSource {
            source_id: bad_source,
        },
        ProjectOp::RemoveMidiNote {
            source_id: bad_source,
            note_id: note.id,
        },
    ] {
        assert!(matches!(c.apply(cmd(&c, op)), Err(Error::NotFound(_))));
    }
    let bad_note = NoteId::new(0xdead);
    for op in [
        ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id: bad_note,
            velocity: 1,
        },
        ProjectOp::MoveMidiNote {
            source_id: source,
            note_id: bad_note,
            start: MidiTick::ZERO,
        },
        ProjectOp::RemoveMidiNote {
            source_id: source,
            note_id: bad_note,
        },
    ] {
        assert!(matches!(c.apply(cmd(&c, op)), Err(Error::NotFound(_))));
    }
    assert!(matches!(
        c.apply(cmd(
            &c,
            ProjectOp::DetachClipSource {
                clip_id: ClipId::new(0xdead),
            }
        )),
        Err(Error::NotFound(_))
    ));

    // Out of range: velocity 0/200, key 200, len 0, overflowing end.
    for op in [
        ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id: note.id,
            velocity: 0,
        },
        ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id: note.id,
            velocity: 200,
        },
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 200,
            velocity: 100,
            start: MidiTick::ZERO,
            len: MidiTick::new(10).unwrap(),
        },
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 1,
            velocity: 0,
            start: MidiTick::ZERO,
            len: MidiTick::new(10).unwrap(),
        },
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 1,
            velocity: 100,
            start: MidiTick::ZERO,
            len: MidiTick::new(0).unwrap(),
        },
        ProjectOp::MoveMidiNote {
            source_id: source,
            note_id: note.id,
            start: MidiTick::new(i64::MAX).unwrap(),
        },
    ] {
        assert!(c.apply(cmd(&c, op)).is_err());
    }
    // Rejections stayed atomic: the note is untouched.
    assert_eq!(only_note(&c, source).velocity, 100);
    assert_eq!(only_note(&c, source).start, MidiTick::ZERO);
    // A negative native tick cannot even be constructed.
    assert!(matches!(MidiTick::new(-1), Err(Error::Negative)));
    // Nor a zero division — `MidiDivision` is `non_exhaustive`, so the
    // validating constructor is the only way to build one outside the
    // crate (the serialized form is covered by the payload tests).
    assert!(MidiDivision::ppqn(0).is_err());
}

// ---------------------------------------------------------------------
// MIDI clip contract: Musical extent + Natural mapping only
// ---------------------------------------------------------------------

#[test]
fn midi_clip_requires_musical_extent_and_natural_mapping() {
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let source = *c.live().midi_sources.keys().next().unwrap();
    let window = || ClipPlayback {
        source_region: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(4 * 480).unwrap())
            .unwrap(),
        mapping: PlaybackMapping::Natural,
    };

    // A frame-domain Absolute extent has no defined meaning over a
    // note sequence in native ticks.
    let abs = ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(0)),
        extent: ClipExtent::Absolute(MediaDuration::new(4 * 480, 1, 480).unwrap()),
    };
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::AddClip {
                lane_id: lane,
                source_id: source,
                placement: abs,
                playback: window(),
            }
        ))
        .is_err()
    );
    // Media-rate mappings are undefined over a note sequence.
    for mapping in [
        PlaybackMapping::Loop,
        PlaybackMapping::SimpleStretch(StretchRatio::new(2, 1).unwrap()),
    ] {
        let mut playback = window();
        playback.mapping = mapping;
        assert!(
            c.apply(cmd(
                &c,
                ProjectOp::AddClip {
                    lane_id: lane,
                    source_id: source,
                    placement: musical_clip(0),
                    playback,
                }
            ))
            .is_err()
        );
    }
    assert!(c.live().clips.is_empty());

    // The same holds for mutation ops on an existing MIDI clip.
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: musical_clip(0),
            playback: window(),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::SetClipExtent {
                clip_id: clip,
                extent: ClipExtent::Absolute(MediaDuration::new(4 * 480, 1, 480).unwrap()),
            }
        ))
        .is_err()
    );
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::SetClipPlayback {
                clip_id: clip,
                playback: ClipPlayback {
                    mapping: PlaybackMapping::Loop,
                    ..window()
                },
            }
        ))
        .is_err()
    );
    // Rejections were atomic — the clip kept its valid contract.
    assert!(matches!(
        c.live().clips[&clip].placement.extent,
        ClipExtent::Musical(_)
    ));
    assert!(matches!(
        c.live().clips[&clip].playback.mapping,
        PlaybackMapping::Natural
    ));
}

// ---------------------------------------------------------------------
// Linked source: shared edits, explicit detach, no implicit COW
// ---------------------------------------------------------------------

#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenario
fn linked_clips_share_source_edits_and_detach_is_explicit() {
    let mut c = controller();
    let (_lane, source, clips) = linked_fixture(&mut c);
    let [c1, c2]: [ClipId; 2] = clips.try_into().unwrap();

    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 64,
            velocity: 90,
            start: MidiTick::new(0).unwrap(),
            len: MidiTick::new(240).unwrap(),
        },
    );
    let note = only_note(&c, source);

    // An edit addressed by (source_id, note_id) — never through a clip —
    // changes what every linked clip reads: C1 and C2 still resolve to
    // the one note set that now carries velocity 40 at tick 480.
    apply(
        &mut c,
        ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id: note.id,
            velocity: 40,
        },
    );
    apply(
        &mut c,
        ProjectOp::MoveMidiNote {
            source_id: source,
            note_id: note.id,
            start: MidiTick::new(480).unwrap(),
        },
    );
    assert_eq!(c.live().clips[&c1].source_id, source);
    assert_eq!(c.live().clips[&c2].source_id, source);
    let edited = only_note(&c, source);
    assert_eq!(edited.velocity, 40);
    assert_eq!(edited.start, MidiTick::new(480).unwrap());
    assert_eq!(edited.len, MidiTick::new(240).unwrap()); // a move keeps len

    // Detach C1: a new source is created, C1 alone is repointed.
    apply(&mut c, ProjectOp::DetachClipSource { clip_id: c1 });
    assert_eq!(c.live().midi_sources.len(), 2);
    let detached = c.live().clips[&c1].source_id;
    assert_ne!(detached, source);
    assert_eq!(c.live().clips[&c2].source_id, source);
    // The copy preserves content under fresh stable IDs.
    let copy = only_note(&c, detached);
    assert_ne!(copy.id, note.id);
    assert_eq!(copy.key, 64);
    assert_eq!(copy.velocity, 40);
    assert_eq!(copy.start, MidiTick::new(480).unwrap());
    assert_eq!(
        c.live().midi_sources[&detached].division,
        MidiDivision::ppqn(480).unwrap()
    );

    // Independence: editing the detached source leaves the linked one
    // untouched — and vice versa.
    apply(
        &mut c,
        ProjectOp::SetMidiNoteVelocity {
            source_id: detached,
            note_id: copy.id,
            velocity: 127,
        },
    );
    assert_eq!(only_note(&c, source).velocity, 40);
    apply(
        &mut c,
        ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id: note.id,
            velocity: 10,
        },
    );
    assert_eq!(only_note(&c, detached).velocity, 127);

    // Detaching a media-source clip is rejected — independence is a
    // MIDI source contract, not silent copy-on-write of media.
    apply(&mut c, ProjectOp::RegisterAsset { digest: "d".into() });
    let asset = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "d".into(),
        },
    );
    let media = *c.live().sources.keys().next().unwrap();
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: media,
            placement: ClipPlacement {
                anchor: TimeAnchor::Absolute(MediaTimeValue::new(0, 1, 48_000).unwrap()),
                extent: ClipExtent::Absolute(MediaDuration::new(48_000, 1, 48_000).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(48_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    let media_clip = c
        .live()
        .clips
        .values()
        .find(|cl| cl.source_id == media)
        .unwrap()
        .id;
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::DetachClipSource {
                clip_id: media_clip,
            }
        ))
        .is_err()
    );
    // And nothing duplicated the media source.
    assert_eq!(c.live().sources.len(), 1);
    assert_eq!(c.live().midi_sources.len(), 2);
}

#[test]
fn remove_midi_source_requires_unreferenced() {
    let mut c = controller();
    let (_lane, source, _clips) = linked_fixture(&mut c);
    // Referenced by two clips: rejected.
    assert!(
        c.apply(cmd(&c, ProjectOp::RemoveMidiSource { source_id: source }))
            .is_err()
    );
    // A media SourceId is not a MIDI source: NotFound, not a panic.
    apply(&mut c, ProjectOp::RegisterAsset { digest: "d".into() });
    let asset = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "d".into(),
        },
    );
    let media = *c.live().sources.keys().next().unwrap();
    assert!(matches!(
        c.apply(cmd(&c, ProjectOp::RemoveMidiSource { source_id: media })),
        Err(Error::NotFound(_))
    ));
    // Media-domain ops see only media sources — a MIDI id is not one.
    assert!(matches!(
        c.apply(cmd(
            &c,
            ProjectOp::SetSourceState {
                source_id: source,
                state: SourceState::Verified,
            }
        )),
        Err(Error::NotFound(_))
    ));
}

// ---------------------------------------------------------------------
// Undo / Redo
// ---------------------------------------------------------------------

#[test]
fn undo_redo_cover_note_edits_and_detach() {
    let mut c = controller();
    let (_lane, source, clips) = linked_fixture(&mut c);
    let c1 = clips[0];
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 60,
            velocity: 100,
            start: MidiTick::new(0).unwrap(),
            len: MidiTick::new(120).unwrap(),
        },
    );
    let note = only_note(&c, source);
    apply(
        &mut c,
        ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id: note.id,
            velocity: 55,
        },
    );
    apply(
        &mut c,
        ProjectOp::MoveMidiNote {
            source_id: source,
            note_id: note.id,
            start: MidiTick::new(96).unwrap(),
        },
    );
    apply(&mut c, ProjectOp::DetachClipSource { clip_id: c1 });

    // Undo walks the whole chain back to the linked pre-detach state.
    assert!(c.undo().unwrap()); // detach
    assert_eq!(c.live().midi_sources.len(), 1);
    assert_eq!(c.live().clips[&c1].source_id, source);
    assert!(c.undo().unwrap()); // move
    assert_eq!(only_note(&c, source).start, MidiTick::ZERO);
    assert!(c.undo().unwrap()); // velocity
    assert_eq!(only_note(&c, source).velocity, 100);
    assert!(c.undo().unwrap()); // add note
    assert!(c.live().midi_sources[&source].notes.is_empty());

    // Redo restores every step including the detach: the same snapshot
    // comes back, so the copied source and note IDs are identical.
    while c.redo().unwrap() {}
    assert_eq!(c.live().midi_sources.len(), 2);
    let detached = c.live().clips[&c1].source_id;
    let copy = only_note(&c, detached);
    assert_ne!(detached, source);
    assert_eq!(copy.start, MidiTick::new(96).unwrap());
    assert_eq!(copy.velocity, 55);
}

// ---------------------------------------------------------------------
// Save / Reload
// ---------------------------------------------------------------------

#[test]
fn save_reload_preserves_midi_state_and_ids() {
    let mut c = controller();
    let (_lane, source, clips) = linked_fixture(&mut c);
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 60,
            velocity: 100,
            start: MidiTick::new(36).unwrap(),
            len: MidiTick::new(120).unwrap(),
        },
    );
    apply(&mut c, ProjectOp::DetachClipSource { clip_id: clips[0] });
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();

    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c.live(), c2.live());
    assert_eq!(c.live_revision(), c2.live_revision());
    // Native tick 36 in the 480-division source round-trips exactly —
    // no requantization to the project's 960 PPQN.
    let reloaded = &c2.live().midi_sources[&source];
    assert_eq!(reloaded.division, MidiDivision::ppqn(480).unwrap());
    assert_eq!(
        reloaded.notes.values().next().unwrap().start,
        MidiTick::new(36).unwrap()
    );
    // A reloaded project keeps issuing fresh, non-colliding IDs.
    apply(
        &mut c2,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 61,
            velocity: 90,
            start: MidiTick::ZERO,
            len: MidiTick::new(10).unwrap(),
        },
    );
    let ids: Vec<_> = c2
        .live()
        .midi_sources
        .values()
        .flat_map(|s| s.notes.keys())
        .collect();
    assert_eq!(ids.len(), 3);
    assert!(c2.live().notes.is_empty());
}

// ---------------------------------------------------------------------
// Schema / reference validation on crafted snapshots
// ---------------------------------------------------------------------

fn snapshot_with_midi(store: &mut InMemoryStore) -> serde_json::Value {
    let mut c = controller();
    let (_lane, source, _clips) = linked_fixture(&mut c);
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 60,
            velocity: 100,
            start: MidiTick::new(48).unwrap(),
            len: MidiTick::new(96).unwrap(),
        },
    );
    c.save(store).unwrap();
    let head = store.head().unwrap().unwrap();
    serde_json::from_slice(&store.read_snapshot(&head).unwrap()).unwrap()
}

/// Writes `env` through the store port and loads it — the same path a
/// real reload takes, so failures are the loader's own verdict.
fn try_load(env: &serde_json::Value, rev: u64) -> Result<ProjectController, Error> {
    let mut store = InMemoryStore::new();
    store
        .save("p", ProjectRevision(rev), &serde_json::to_vec(env).unwrap())
        .unwrap();
    let mut c = controller();
    c.load(&store).map(|()| c)
}

#[test]
fn load_rejects_invalid_midi_source_payloads() {
    let mut store = InMemoryStore::new();
    let good = snapshot_with_midi(&mut store);
    assert!(try_load(&good, 30).is_ok());

    // Division of zero.
    let mut bad = good.clone();
    bad["project"]["midi_sources"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["division"]["value"] = serde_json::json!(0);
    assert!(try_load(&bad, 31).is_err());

    // Velocity 0 stored in a note.
    let mut bad = good.clone();
    let source = bad["project"]["midi_sources"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    source["notes"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["velocity"] = serde_json::json!(0);
    assert!(try_load(&bad, 32).is_err());

    // Zero-length note.
    let mut bad = good.clone();
    let source = bad["project"]["midi_sources"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    source["notes"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["len"] = serde_json::json!(0);
    assert!(try_load(&bad, 33).is_err());

    // Clip pointing at a missing MIDI source.
    let mut bad = good.clone();
    bad["project"]["midi_sources"] = serde_json::json!({});
    assert!(try_load(&bad, 34).is_err());

    // Note stored under a foreign map key.
    let mut bad = good.clone();
    let source = bad["project"]["midi_sources"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    let (nid, mut note) = source["notes"]
        .as_object()
        .unwrap()
        .iter()
        .next()
        .map(|(k, v)| (k.clone(), v.clone()))
        .unwrap();
    note["id"] = serde_json::json!("note-ffffffffffffffffffffffffffffffff");
    source["notes"].as_object_mut().unwrap().remove(&nid);
    source["notes"].as_object_mut().unwrap().insert(nid, note);
    assert!(try_load(&bad, 35).is_err());

    // Unknown field on a note — deny_unknown_fields fails loudly.
    let mut bad = good.clone();
    bad["project"]["midi_sources"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["notes"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["mystery"] = serde_json::json!(1);
    assert!(try_load(&bad, 36).is_err());

    // Unknown sibling field on the tagged division repr — a smuggled
    // key next to kind/value must fail loudly, not silently drop.
    let mut bad = good.clone();
    bad["project"]["midi_sources"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["division"]
        .as_object_mut()
        .unwrap()
        .insert("smuggled".into(), serde_json::json!(1));
    assert!(try_load(&bad, 37).is_err());
}

#[test]
fn load_rejects_note_id_collisions() {
    let mut store = InMemoryStore::new();
    let good = snapshot_with_midi(&mut store);

    // The same NoteId in two MIDI sources.
    let mut bad = good.clone();
    let sources = bad["project"]["midi_sources"].as_object_mut().unwrap();
    let mut second = sources.values().next().unwrap().clone();
    second["id"] = serde_json::json!("source-000000000000000000000000000000ff");
    sources.insert(
        "source-000000000000000000000000000000ff".to_string(),
        second,
    );
    assert!(try_load(&bad, 40).is_err());

    // The same NoteId as a clip-local note and a MIDI note.
    let mut bad = good.clone();
    let sources = bad["project"]["midi_sources"].as_object_mut().unwrap();
    let note_entry = sources.values().next().unwrap()["notes"]
        .as_object()
        .unwrap()
        .iter()
        .next()
        .unwrap();
    let (nkey, nbody) = (note_entry.0.clone(), note_entry.1.clone());
    let clip_id = bad["project"]["clips"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    bad["project"]["notes"].as_object_mut().unwrap().insert(
        nkey,
        serde_json::json!({
            "id": nbody["id"],
            "clip": clip_id,
            "key": 60,
            "start": {"num": 0, "den": 1},
            "len": {"num": 1, "den": 1}
        }),
    );
    assert!(try_load(&bad, 41).is_err());
}

#[test]
fn load_rejects_source_id_shared_across_maps() {
    // Snapshot carrying BOTH a media source and a MIDI source: rename
    // the media source to collide with the MIDI source's SourceId.
    let mut store = InMemoryStore::new();
    let mut c = controller();
    let (_lane, midi_source, _clips) = linked_fixture(&mut c);
    apply(&mut c, ProjectOp::RegisterAsset { digest: "d".into() });
    let asset = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "d".into(),
        },
    );
    let media_source = *c.live().sources.keys().next().unwrap();
    c.save(&mut store).unwrap();
    let head = store.head().unwrap().unwrap();
    let mut env: serde_json::Value =
        serde_json::from_slice(&store.read_snapshot(&head).unwrap()).unwrap();

    let midi_key = midi_source.to_tagged();
    let media_key = media_source.to_tagged();
    let sources = env["project"]["sources"].as_object_mut().unwrap();
    let mut body = sources.remove(&media_key).unwrap();
    body["id"] = serde_json::json!(midi_key);
    sources.insert(midi_key, body);
    // One SourceId now names a media Source and a MidiSource at once:
    // clip/note addressing would silently depend on the map a
    // consumer reads, so load must refuse it.
    assert!(try_load(&env, 42).is_err());
}

#[test]
fn load_rejects_clip_note_stored_under_foreign_key() {
    // A clip-local note whose map key differs from its inner `id`
    // used to bypass the cross-domain NoteId disjointness check (the
    // check reads map keys): the inner id claims a MIDI note's
    // identity while the entry hides under a different key.
    let mut store = InMemoryStore::new();
    let good = snapshot_with_midi(&mut store);
    let note_body = good["project"]["midi_sources"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()["notes"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    let clip_id = good["project"]["clips"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let mut bad = good;
    bad["project"]["notes"].as_object_mut().unwrap().insert(
        "note-000000000000000000000000000000ee".to_string(),
        serde_json::json!({
            "id": note_body["id"],
            "clip": clip_id,
            "key": 60,
            "start": {"num": 0, "den": 1},
            "len": {"num": 1, "den": 1}
        }),
    );
    assert!(try_load(&bad, 43).is_err());
}

#[test]
fn load_rejects_id_watermark_below_live_ids() {
    // A forged-low watermark reseeds the generator underneath live
    // IDs — the next op would re-issue a used ID and silently
    // overwrite that entity. The loader must reject such a snapshot.
    let mut store = InMemoryStore::new();
    let mut good = snapshot_with_midi(&mut store);
    good["project"]["id_watermark"] = serde_json::json!(0);
    assert!(try_load(&good, 44).is_err());

    // Equal to a live ID's raw value is just as forged: the watermark
    // is the *next* issuance, strictly above every live key.
    let mut store = InMemoryStore::new();
    let good = snapshot_with_midi(&mut store);
    let mut bad = good.clone();
    let source_key = bad["project"]["midi_sources"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let raw = u128::from_str_radix(source_key.strip_prefix("source-").unwrap(), 16).unwrap();
    bad["project"]["id_watermark"] = serde_json::json!(raw);
    assert!(try_load(&bad, 45).is_err());
}

// ---------------------------------------------------------------------
// Time fidelity across native divisions
// ---------------------------------------------------------------------

#[test]
fn native_divisions_preserve_exact_ticks() {
    let mut c = controller();
    for div in [480u32, 1000] {
        apply(
            &mut c,
            ProjectOp::AddMidiSource {
                division: MidiDivision::ppqn(div).unwrap(),
            },
        );
    }
    let sources: Vec<_> = c.live().midi_sources.keys().copied().collect();
    assert_eq!(sources.len(), 2);
    // A 3/40-quarter note in the 480 source is exactly tick 36; the
    // same instant in the 1000 source is tick 75 — both stored
    // losslessly in their own native grid.
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: sources[0],
            key: 60,
            velocity: 100,
            start: MidiTick::new(36).unwrap(),
            len: MidiTick::new(12).unwrap(),
        },
    );
    apply(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: sources[1],
            key: 60,
            velocity: 100,
            start: MidiTick::new(75).unwrap(),
            len: MidiTick::new(25).unwrap(),
        },
    );
    let divs: Vec<_> = c
        .live()
        .midi_sources
        .values()
        .map(|s| s.division.ticks_per_quarter())
        .collect();
    assert_eq!(divs, vec![480, 1000]);
    for (source_id, (start, len)) in sources.iter().zip([(36, 12), (75, 25)]) {
        let note = only_note(&c, *source_id);
        assert_eq!(note.start.value(), start);
        assert_eq!(note.end().unwrap().value(), start + len);
    }
}
