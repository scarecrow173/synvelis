//! CB-03 Creative Branch MIDI clip/source adapter fixtures
//! (synvelis#280).
//!
//! Covers the real `MidiClipAdapter` headlessly: fork/edit/adopt of
//! `VariantScope::MidiClip` candidates, native SMF/division/tick
//! fidelity through save/reload, canonical note-level diff lineage,
//! same-context render-pin staleness (the CB-MIDI-04/RCP-01 identity
//! half), honest capability reporting, and typed rejection of
//! unsupported/ambiguous ops. Real OS / plugin / RT / MIDI-device /
//! GUI lanes are `NotRun` — recorded in `evidence/sv280/`.

use synvelis_project::diff::{
    AmbiguityReason, ChangeKind, ComparisonSpec, ObjectKind, SemanticChange, compare_snapshots,
};
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, ContentDigest, FixedSnapshotLocator, ImmutableSnapshot,
    ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness, SnapshotIdentity,
    SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::{InMemoryStore, SCHEMA_VERSION};
use synvelis_project::variant::{
    AdoptIntent, AdoptSources, BranchContentRef, BranchEditOp, BranchEditOpIntent,
    InMemoryBranchContentStore, MidiClipAdapter, NoopRuntimePublish, ProcessingCapability,
    VariantError, VariantScope, adopt_branch_adapted, decode_candidate, edit_branch_op,
    fork_variant_set, scope_digest,
};
use synvelis_project::{
    BranchId, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, CounterIdGenerator, LaneId,
    MidiDivision, MidiNote, MidiTick, MusicalDuration, MusicalTick, NoteId, PlaybackMapping,
    Project, ProjectController, ProjectId, ProjectOp, ProjectRevision, RenderBranchTarget,
    SmfDocument, SourceFrame, SourceId, SourceRange, StretchRatio, TimeAnchor, VariantSetId,
    parse_smf, resolve_render_project,
};

// ---------- SMF byte builders (tim_02_smf.rs conventions) ----------

fn vlq(mut v: u64) -> Vec<u8> {
    let mut buf = [0u8; 8];
    let mut n = 0;
    loop {
        buf[n] = (v & 0x7F) as u8;
        v >>= 7;
        n += 1;
        if v == 0 {
            break;
        }
    }
    let mut out = Vec::with_capacity(n);
    for i in (0..n).rev() {
        let mut b = buf[i];
        if i != 0 {
            b |= 0x80;
        }
        out.push(b);
    }
    out
}

fn ev(delta: u64, body: &[u8]) -> Vec<u8> {
    let mut v = vlq(delta);
    v.extend_from_slice(body);
    v
}

fn meta(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![0xFF, kind];
    v.extend(vlq(payload.len() as u64));
    v.extend_from_slice(payload);
    v
}

fn tempo(us: u32) -> Vec<u8> {
    meta(0x51, &us.to_be_bytes()[1..])
}

fn timesig(numerator: u8, denominator_log2: u8, thirtyseconds: u8, clocks: u8) -> Vec<u8> {
    meta(0x58, &[numerator, denominator_log2, thirtyseconds, clocks])
}

fn trackname(name: &str) -> Vec<u8> {
    meta(0x03, name.as_bytes())
}

fn eot() -> Vec<u8> {
    vec![0xFF, 0x2F, 0x00]
}

fn note_on(channel: u8, key: u8, velocity: u8) -> Vec<u8> {
    vec![0x90 | channel, key, velocity]
}

fn note_off(channel: u8, key: u8, velocity: u8) -> Vec<u8> {
    vec![0x80 | channel, key, velocity]
}

fn cc(channel: u8, controller: u8, value: u8) -> Vec<u8> {
    vec![0xB0 | channel, controller, value]
}

fn pitch_bend(channel: u8, value: u16) -> Vec<u8> {
    vec![
        0xE0 | channel,
        u8::try_from(value & 0x7F).unwrap(),
        u8::try_from(value >> 7).unwrap(),
    ]
}

fn sysex_f0(data: &[u8]) -> Vec<u8> {
    let mut v = vec![0xF0];
    v.extend(vlq(data.len() as u64));
    v.extend_from_slice(data);
    v
}

fn mtrk(events: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for e in events {
        body.extend_from_slice(e);
    }
    let mut v = b"MTrk".to_vec();
    v.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
    v.extend_from_slice(&body);
    v
}

fn smf_file(format: u16, division: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
    let mut v = b"MThd".to_vec();
    v.extend_from_slice(&6u32.to_be_bytes());
    v.extend_from_slice(&format.to_be_bytes());
    v.extend_from_slice(&u16::try_from(tracks.len()).unwrap().to_be_bytes());
    v.extend_from_slice(&division.to_be_bytes());
    for t in tracks {
        v.extend_from_slice(t);
    }
    v
}

/// Format 0, division 480: odd native ticks (37/101, 1001/53) prove
/// no quantize/rounding; CC64 + pitch bend + sysex stay in the
/// provenance stream only.
fn smf_format0_odd_ticks() -> Vec<u8> {
    smf_file(
        0,
        480,
        &[mtrk(&[
            ev(0, &tempo(500_000)),
            ev(0, &timesig(4, 2, 24, 8)),
            ev(0, &trackname("Lead")),
            ev(0, &cc(0, 64, 127)),
            ev(37, &note_on(0, 60, 100)),
            ev(101, &note_off(0, 60, 64)),
            ev(963, &note_on(0, 64, 90)),
            ev(53, &note_off(0, 64, 40)),
            ev(0, &pitch_bend(0, 8_192)),
            ev(0, &sysex_f0(&[0x7E, 0x00, 0x09])),
            ev(0, &eot()),
        ])],
    )
}

/// Format 1, division 96 (deliberately different PPQN), conductor
/// tempo map + two event tracks.
fn smf_format1_div96() -> Vec<u8> {
    smf_file(
        1,
        96,
        &[
            mtrk(&[
                ev(0, &tempo(500_000)),
                ev(0, &tempo(400_000)),
                ev(0, &timesig(3, 2, 24, 8)),
                ev(0, &trackname("Conductor")),
                ev(0, &eot()),
            ]),
            mtrk(&[
                ev(0, &cc(0, 64, 127)),
                ev(17, &note_on(0, 60, 100)),
                ev(29, &note_off(0, 60, 64)),
                ev(0, &eot()),
            ]),
            mtrk(&[
                ev(0, &cc(1, 64, 64)),
                ev(11, &note_on(1, 64, 90)),
                ev(23, &note_off(1, 64, 40)),
                ev(0, &eot()),
            ]),
        ],
    )
}

// ---------- shared harness (cb02_audio_take.rs conventions) ────────

struct Fx {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    resolver: SnapshotResolver,
    adapter: MidiClipAdapter,
}

impl Fx {
    fn new(controller: ProjectController) -> Self {
        Self {
            controller,
            store: InMemoryBranchContentStore::default(),
            resolver: SnapshotResolver::new(),
            adapter: MidiClipAdapter::new(),
        }
    }

    fn fork(&mut self, scope: VariantScope) -> synvelis_project::variant::ForkReceipt {
        fork_variant_set(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &self.resolver,
            scope,
            &SnapshotCancel::new(),
        )
        .unwrap()
    }

    fn try_fork(&mut self, scope: VariantScope) -> Result<(), VariantError> {
        fork_variant_set(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &self.resolver,
            scope,
            &SnapshotCancel::new(),
        )
        .map(|_| ())
    }

    fn try_edit(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        op: ProjectOp,
    ) -> Result<(), VariantError> {
        let expected = self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .revision();
        edit_branch_op(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &BranchEditOpIntent {
                variant_set: set,
                branch,
                expected_branch_revision: expected,
                op: BranchEditOp::Project(op),
            },
        )
        .map(|_| ())
    }

    fn edit(&mut self, set: VariantSetId, branch: BranchId, op: ProjectOp) {
        self.try_edit(set, branch, op).unwrap();
    }

    fn candidate(&self, set: VariantSetId, branch: BranchId) -> Project {
        let content = self.controller.live().variant_sets[&set].branches[&branch]
            .content
            .clone();
        decode_candidate(&self.store, &content).unwrap()
    }

    fn intent(&self, set: VariantSetId, branch: BranchId, scope: &VariantScope) -> AdoptIntent {
        let set_state = &self.controller.live().variant_sets[&set];
        AdoptIntent {
            variant_set: set,
            branch,
            expected_project_revision: self.controller.live_revision(),
            expected_branch_revision: set_state.branches[&branch].content.revision(),
            expected_scope_digest: scope_digest(self.controller.live(), scope).unwrap(),
            approval: synvelis_project::apply::ApplyApproval {
                ticket: "approve".into(),
            },
        }
    }

    fn try_adopt(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        scope: &VariantScope,
    ) -> Result<synvelis_project::variant::AdoptReceipt, VariantError> {
        let intent = self.intent(set, branch, scope);
        let runtime = NoopRuntimePublish;
        adopt_branch_adapted(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &AdoptSources {
                assets: None,
                plugins: None,
                runtime: &runtime,
            },
            &intent,
        )
    }

    fn adopt(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        scope: &VariantScope,
    ) -> synvelis_project::variant::AdoptReceipt {
        self.try_adopt(set, branch, scope).unwrap()
    }

    /// Canonical render-reuse pin — the RCP-01 read-adapter identity
    /// CB-MIDI-RCP-01/CB-MIDI-04 consume.
    fn render_pin(
        &self,
        set: VariantSetId,
        branch: BranchId,
    ) -> synvelis_project::render_input::BranchRenderPin {
        resolve_render_project(
            self.controller.live(),
            Some(RenderBranchTarget {
                variant_set: set,
                branch,
                store: &self.store,
            }),
        )
        .unwrap()
        .branch
        .unwrap()
    }
}

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "cb03-fixture"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply_op(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

fn musical_clip(anchor_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap()),
    }
}

fn midi_window() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(4 * 960).unwrap())
            .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

struct MidiFixture {
    clip: ClipId,
    source: SourceId,
    /// `NoteId`s sorted by (start, id) for deterministic addressing.
    notes: Vec<NoteId>,
}

/// Track → lane → editable MIDI source (division 480) → two notes →
/// one clip reading the source exclusively.
fn midi_fixture(c: &mut ProjectController) -> MidiFixture {
    apply_op(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply_op(
        c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let source = *c.live().midi_sources.keys().next().unwrap();
    for (key, velocity, start, len) in [
        (60_u8, 100_u8, 0_i64, 120_i64),
        (64_u8, 80_u8, 240_i64, 96_i64),
    ] {
        apply_op(
            c,
            ProjectOp::AddMidiNote {
                source_id: source,
                key,
                velocity,
                start: MidiTick::new(start).unwrap(),
                len: MidiTick::new(len).unwrap(),
            },
        );
    }
    apply_op(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: musical_clip(0),
            playback: midi_window(),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();
    let mut notes: Vec<(MidiTick, NoteId)> = c.live().midi_sources[&source]
        .notes
        .values()
        .map(|n| (n.start, n.id))
        .collect();
    notes.sort();
    MidiFixture {
        clip,
        source,
        notes: notes.into_iter().map(|(_, id)| id).collect(),
    }
}

fn midi_scope(clip: ClipId) -> VariantScope {
    VariantScope::MidiClip { clip }
}

/// Serialized live MIDI state — the "raw native source" witness.
fn midi_state_json(c: &ProjectController) -> serde_json::Value {
    serde_json::json!({
        "midi_sources": c.live().midi_sources,
        "smf_documents": c.live().smf_documents,
        "clips": c.live().clips,
    })
}

fn mt(tick: i64) -> MidiTick {
    MidiTick::new(tick).unwrap()
}

// ===========================================================================
// CB-MIDI-01: fork candidates from one MIDI source; edits stay isolated
// ===========================================================================

/// The engine's `VariantSet` holds adopted+candidate branches; "3候補"
/// is exercised as three distinct scoped *contents* across the
/// fork → edit → adopt → freeze → edit lifecycle, plus a sibling
/// scope proving cross-candidate isolation. Each content keeps its
/// own scope digest; candidate edits never touch live or siblings.
#[test]
#[allow(clippy::too_many_lines)] // sequential fork/edit/adopt lifecycle fixture
fn cb_midi_01_candidate_contents_isolated_across_lifecycle() {
    let mut c = controller();
    let fx = midi_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = midi_scope(fx.clip);

    let live_before = midi_state_json(&h.controller);
    let digest_origin = scope_digest(h.controller.live(), &scope).unwrap();

    let forked = h.fork(scope.clone());
    let (set, adopted, candidate_b) = (forked.variant_set, forked.adopted, forked.candidate);

    // ── B edits: velocity + move + add (note/velocity surface) ──
    let n1 = fx.notes[0];
    let n2 = fx.notes[1];
    h.edit(
        set,
        candidate_b,
        ProjectOp::SetMidiNoteVelocity {
            source_id: fx.source,
            note_id: n1,
            velocity: 90,
        },
    );
    h.edit(
        set,
        candidate_b,
        ProjectOp::MoveMidiNote {
            source_id: fx.source,
            note_id: n2,
            start: mt(480),
        },
    );
    h.edit(
        set,
        candidate_b,
        ProjectOp::AddMidiNote {
            source_id: fx.source,
            key: 67,
            velocity: 70,
            start: mt(720),
            len: mt(48),
        },
    );

    // Live and the adopted (live-mirror) branch are untouched; the
    // raw native source is byte-identical.
    assert_eq!(midi_state_json(&h.controller), live_before);
    assert!(matches!(
        h.controller.live().variant_sets[&set].branches[&adopted].content,
        BranchContentRef::Live { .. }
    ));

    let b_prime = h.candidate(set, candidate_b);
    let b_notes = &b_prime.midi_sources[&fx.source].notes;
    assert_eq!(b_notes[&n1].velocity, 90);
    assert_eq!(b_notes[&n2].start, mt(480));
    assert_eq!(b_notes[&n2].len, mt(96)); // move preserves length
    assert_eq!(b_notes.len(), 3);
    let added = b_notes
        .values()
        .find(|n| n.key == 67)
        .expect("added note present");
    // Branch-local NoteId is fresh — never collides with live ids.
    assert!(
        !h.controller.live().midi_sources[&fx.source]
            .notes
            .contains_key(&added.id)
    );

    let digest_b = scope_digest(&b_prime, &scope).unwrap();
    assert_ne!(digest_origin, digest_b);

    // ── Adopt B: live becomes B′; A freezes as the non-adopted
    // candidate holding the origin content ──
    let receipt = h.adopt(set, candidate_b, &scope);
    assert!(receipt.objects > 0);
    let live_b = midi_state_json(&h.controller);
    let digest_b_live = scope_digest(h.controller.live(), &scope).unwrap();
    assert_eq!(digest_b_live, digest_b);

    // ── C: edit the frozen A candidate — live (B′) must not move ──
    // Adopt swapped the adopted pointer to B's branch id; A's branch
    // id is now the non-adopted candidate holding origin content.
    let set_state = &h.controller.live().variant_sets[&set];
    assert_eq!(set_state.adopted, candidate_b);
    assert!(set_state.branches.contains_key(&adopted));
    let candidate_a = adopted;

    h.edit(
        set,
        candidate_a,
        ProjectOp::MoveMidiNote {
            source_id: fx.source,
            note_id: n1,
            start: mt(96),
        },
    );
    h.edit(
        set,
        candidate_a,
        ProjectOp::AddMidiNote {
            source_id: fx.source,
            key: 72,
            velocity: 60,
            start: mt(0),
            len: mt(24),
        },
    );

    // A′ edits did not touch live: live is still exactly B′.
    assert_eq!(midi_state_json(&h.controller), live_b);
    let a_prime = h.candidate(set, candidate_a);
    let digest_a = scope_digest(&a_prime, &scope).unwrap();

    // Three distinct scoped contents forked from the same MIDI
    // source: origin (A@fork), B′ (adopted/live), A′ (edited
    // candidate). Pairwise-distinct digests — no merge, no aliasing.
    assert_ne!(digest_origin, digest_b_live);
    assert_ne!(digest_b_live, digest_a);
    assert_ne!(digest_origin, digest_a);

    // A′ holds origin lineage plus its own edits; B′ lineage (n1
    // velocity 90, n2 moved, key-67 note) is untouched inside A′.
    let a_notes = &a_prime.midi_sources[&fx.source].notes;
    assert_eq!(a_notes[&n1].velocity, 100); // A never saw B's edit
    assert_eq!(a_notes[&n1].start, mt(96)); // A's own move applied
    assert_eq!(a_notes[&n2].start, mt(240)); // B's move not leaked
    assert_eq!(a_notes.len(), 3);
}

/// A second MIDI clip scope (different clip, different source) is a
/// disjoint variant set — edits in set 1 leave set 2's adopted and
/// candidate contents bitwise untouched.
#[test]
fn cb_midi_01_sibling_midi_scope_isolated() {
    let mut c = controller();
    let fx_a = midi_fixture(&mut c);
    // Second lane + second MIDI source + second clip.
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let lane_b = *c
        .live()
        .lanes
        .keys()
        .find(|l| **l != c.live().clips[&fx_a.clip].lane_id)
        .unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(240).unwrap(),
        },
    );
    let source_b = *c
        .live()
        .midi_sources
        .keys()
        .find(|s| **s != fx_a.source)
        .unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddMidiNote {
            source_id: source_b,
            key: 50,
            velocity: 88,
            start: mt(0),
            len: mt(200),
        },
    );
    apply_op(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane_b,
            source_id: source_b,
            placement: musical_clip(0),
            playback: midi_window(),
        },
    );
    let clip_b = *c.live().clips.keys().find(|cl| **cl != fx_a.clip).unwrap();
    let scope_b = midi_scope(clip_b);
    let note_b = *c.live().midi_sources[&source_b]
        .notes
        .keys()
        .next()
        .unwrap();

    let mut h = Fx::new(c);
    let fa = h.fork(midi_scope(fx_a.clip));
    let fb = h.fork(scope_b.clone());

    let b_state_before = serde_json::json!({
        "candidate": h.candidate(fb.variant_set, fb.candidate).midi_sources[&source_b],
        "adopted_digest": scope_digest(h.controller.live(), &scope_b).unwrap(),
    });

    h.edit(
        fa.variant_set,
        fa.candidate,
        ProjectOp::SetMidiNoteVelocity {
            source_id: fx_a.source,
            note_id: fx_a.notes[0],
            velocity: 33,
        },
    );
    h.edit(
        fa.variant_set,
        fa.candidate,
        ProjectOp::MoveMidiNote {
            source_id: fx_a.source,
            note_id: fx_a.notes[1],
            start: mt(999),
        },
    );

    let b_state_after = serde_json::json!({
        "candidate": h.candidate(fb.variant_set, fb.candidate).midi_sources[&source_b],
        "adopted_digest": scope_digest(h.controller.live(), &scope_b).unwrap(),
    });
    assert_eq!(b_state_before, b_state_after);
    // And B's own note is untouched inside B's candidate.
    assert_eq!(
        h.candidate(fb.variant_set, fb.candidate).midi_sources[&source_b].notes[&note_b].velocity,
        88
    );
}

// ===========================================================================
// CB-MIDI-02: canonical note diff — stable NoteId lineage, no merge on
// collision, Unknown on ambiguity
// ===========================================================================

fn snap_manual(project: Project, schema_version: u32, tag: &str) -> ImmutableSnapshot {
    let bytes = serde_json::to_vec(&project).unwrap();
    let mut material = tag.as_bytes().to_vec();
    material.extend_from_slice(&schema_version.to_le_bytes());
    material.extend_from_slice(&bytes);
    ImmutableSnapshot {
        identity: SnapshotIdentity {
            project_id: project.project_id,
            source: SnapshotOrigin::Live,
            locator: FixedSnapshotLocator::Live {
                project_id: project.project_id,
                live_revision: ProjectRevision(0),
            },
            digest: ContentDigest::sha256(&material),
            schema_version,
            projection_version: SCHEMA_VERSION,
            capture_generation: 0,
        },
        origin: SnapshotOrigin::Live,
        projection: ReadOnlyProjectProjection::new(schema_version, project),
        validation: SnapshotValidation::Valid,
        assets: AssetAvailabilityManifest::default(),
        opaque_state: Vec::new(),
        completeness: SnapshotCompleteness::Complete,
    }
}

fn compare(before: &Project, after: &Project) -> synvelis_project::diff::ChangeSet {
    let a = snap_manual(before.clone(), SCHEMA_VERSION, "before");
    let b = snap_manual(after.clone(), SCHEMA_VERSION, "after");
    compare_snapshots(
        &a,
        &b,
        &ComparisonSpec::new(a.identity.clone(), b.identity.clone()),
        &SnapshotCancel::new(),
    )
    .unwrap()
}

fn midi_note_changes(cs: &synvelis_project::diff::ChangeSet) -> Vec<&SemanticChange> {
    cs.changes
        .iter()
        .filter(|ch| ch.object.kind == ObjectKind::MidiNote)
        .collect()
}

fn tagged(note: NoteId) -> String {
    note.to_tagged()
}

/// Move/Velocity/Add/Delete on stable `NoteId` lineage produce the
/// canonical classification: matched notes diff fields, added notes
/// are `Added`, removed notes `Removed` — a delete+add pair never
/// collapses into a `Modified`.
#[test]
fn cb_midi_02_note_diff_matches_stable_lineage() {
    let mut c = controller();
    let fx = midi_fixture(&mut c);
    let n1 = fx.notes[0];
    let n2 = fx.notes[1];
    let mut h = Fx::new(c);
    let forked = h.fork(midi_scope(fx.clip));
    let (set, cand) = (forked.variant_set, forked.candidate);

    h.edit(
        set,
        cand,
        ProjectOp::SetMidiNoteVelocity {
            source_id: fx.source,
            note_id: n1,
            velocity: 90,
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::MoveMidiNote {
            source_id: fx.source,
            note_id: n2,
            start: mt(480),
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::AddMidiNote {
            source_id: fx.source,
            key: 67,
            velocity: 70,
            start: mt(720),
            len: mt(48),
        },
    );

    let before = h.controller.live().clone();
    let after = h.candidate(set, cand);
    let cs = compare(&before, &after);
    let changes = midi_note_changes(&cs);

    // n1: same NoteId, velocity field changed → Modified.
    let n1_change = changes
        .iter()
        .find(|ch| ch.object.stable_id == tagged(n1))
        .expect("n1 diff");
    assert_eq!(n1_change.kind, ChangeKind::Modified);
    assert!(
        n1_change
            .evidence
            .fields
            .iter()
            .any(|f| f.field == "velocity")
    );

    // n2: same NoteId, start field changed → Modified (a move is NOT
    // delete+add).
    let n2_change = changes
        .iter()
        .find(|ch| ch.object.stable_id == tagged(n2))
        .expect("n2 diff");
    assert_eq!(n2_change.kind, ChangeKind::Modified);
    assert!(n2_change.evidence.fields.iter().any(|f| f.field == "start"));

    // Added note: new NoteId → Added, no pairing with anything.
    let added_id = after.midi_sources[&fx.source]
        .notes
        .values()
        .find(|n| n.key == 67)
        .unwrap()
        .id;
    let add_change = changes
        .iter()
        .find(|ch| ch.object.stable_id == tagged(added_id))
        .expect("added note diff");
    assert_eq!(add_change.kind, ChangeKind::Added);
    assert_eq!(changes.len(), 3, "exactly three note changes");
}

/// A delete+add pair (different `NoteId`s) surfaces as `Removed` +
/// `Added` — it never collapses into a `Modified` on the old id.
#[test]
fn cb_midi_02_delete_add_never_merges_into_modified() {
    let mut c = controller();
    let fx = midi_fixture(&mut c);
    let n2 = fx.notes[1];
    let before = c.live().clone();

    // Crafted after-state: n2 removed, a fresh note added. The diff
    // engine reads projections, not validation.
    let mut crafted = before.clone();
    crafted
        .midi_sources
        .get_mut(&fx.source)
        .unwrap()
        .notes
        .remove(&n2);
    let fresh = MidiNote {
        id: NoteId::new(0xDEAD_BEEF),
        key: 62,
        velocity: 64,
        start: mt(500),
        len: mt(60),
    };
    crafted
        .midi_sources
        .get_mut(&fx.source)
        .unwrap()
        .notes
        .insert(fresh.id, fresh.clone());

    let cs = compare(&before, &crafted);
    let note_changes = midi_note_changes(&cs);
    let removed = note_changes
        .iter()
        .find(|ch| ch.object.stable_id == tagged(n2))
        .expect("n2 removal diff");
    assert_eq!(removed.kind, ChangeKind::Removed);
    assert!(
        note_changes
            .iter()
            .any(|ch| ch.object.stable_id == tagged(fresh.id) && ch.kind == ChangeKind::Added)
    );
    // No change claims n2 was Modified — delete+add stays split.
    assert!(
        !note_changes
            .iter()
            .any(|ch| ch.object.stable_id == tagged(n2) && ch.kind == ChangeKind::Modified)
    );
}

/// A note whose owning source changed is an owner conflict (typed
/// ambiguity), not a silent move; a raw-id collision across kinds is
/// `CrossKindId` — neither produces a wrong merge or wrong diff.
#[test]
fn cb_midi_02_ambiguity_is_typed_not_merged() {
    let mut c = controller();
    let fx = midi_fixture(&mut c);
    // Second MIDI source outside the scope (no clip needed for diff).
    apply_op(
        &mut c,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let other_source = *c
        .live()
        .midi_sources
        .keys()
        .find(|s| **s != fx.source)
        .unwrap();
    let h = Fx::new(c);
    let before = h.controller.live().clone();
    let n1 = fx.notes[0];

    // OwnerConflict: move n1 into the other source in the after-state.
    let mut owner_conflict = before.clone();
    let note = owner_conflict
        .midi_sources
        .get_mut(&fx.source)
        .unwrap()
        .notes
        .remove(&n1)
        .unwrap();
    owner_conflict
        .midi_sources
        .get_mut(&other_source)
        .unwrap()
        .notes
        .insert(n1, note);
    let cs = compare(&before, &owner_conflict);
    assert!(
        cs.unresolved.iter().any(|issue| {
            matches!(
                issue,
                synvelis_project::diff::ComparisonIssue::AmbiguousIdentity {
                    reason: AmbiguityReason::OwnerConflict,
                    ..
                }
            )
        }),
        "owner conflict must be typed ambiguity"
    );
    assert!(
        !cs.changes
            .iter()
            .any(|ch| ch.object.kind == ObjectKind::MidiNote
                && ch.object.stable_id == tagged(n1)
                && ch.kind == ChangeKind::Modified)
    );

    // CrossKindId: a second MidiSource whose SourceId raw collides
    // with n1's NoteId raw makes n1's identity ambiguous.
    let mut colliding = before.clone();
    let mut shadow = colliding.midi_sources[&fx.source].clone();
    shadow.id = SourceId::new(n1.raw());
    shadow.notes.clear();
    colliding.midi_sources.insert(shadow.id, shadow);
    let cs2 = compare(&before, &colliding);
    assert!(
        cs2.unresolved.iter().any(|issue| {
            matches!(
                issue,
                synvelis_project::diff::ComparisonIssue::AmbiguousIdentity {
                    reason: AmbiguityReason::CrossKindId,
                    ..
                }
            )
        }),
        "raw-id collision must be typed ambiguity"
    );
}

// ===========================================================================
// CB-MIDI-03: SMF0/1 native fidelity — division, event order, ticks
// survive save/reload and candidacy with no quantize
// ===========================================================================

#[test]
fn cb_midi_03_smf_native_fidelity_survives_save_reload_and_fork() {
    let mut c = controller();
    let doc0 = parse_smf(&smf_format0_odd_ticks()).unwrap();
    let doc1 = parse_smf(&smf_format1_div96()).unwrap();
    apply_op(&mut c, ProjectOp::ImportMidiSmf { document: doc0 });
    apply_op(&mut c, ProjectOp::ImportMidiSmf { document: doc1 });
    assert_eq!(c.live().smf_documents.len(), 2);
    let (src0, src1) = {
        let mut ids: Vec<SourceId> = c.live().smf_documents.keys().copied().collect();
        ids.sort();
        (ids[0], ids[1])
    };
    // Which is which by division.
    let (f0_src, f1_src) = if c.live().midi_sources[&src0].division.ticks_per_quarter() == 480 {
        (src0, src1)
    } else {
        (src1, src0)
    };
    assert_eq!(
        c.live().midi_sources[&f1_src].division.ticks_per_quarter(),
        96
    );

    // Clip reads the SMF0 source exclusively → valid MidiClip scope.
    apply_op(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: f0_src,
            placement: musical_clip(0),
            playback: midi_window(),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();

    // Odd native ticks are the projected notes — no quantization to
    // project PPQN (960) or to the other source's division.
    let notes0: Vec<(MidiTick, MidiTick)> = c.live().midi_sources[&f0_src]
        .notes
        .values()
        .map(|n| (n.start, n.len))
        .collect();
    assert!(notes0.contains(&(mt(37), mt(101))));
    assert!(notes0.contains(&(mt(1101), mt(53))));

    let mut h = Fx::new(c);
    let scope = midi_scope(clip);
    let forked = h.fork(scope.clone());

    // Candidate preserves the full SMF provenance + native division.
    let cand = h.candidate(forked.variant_set, forked.candidate);
    assert_eq!(
        serde_json::to_value(&cand.smf_documents).unwrap(),
        serde_json::to_value(&h.controller.live().smf_documents).unwrap()
    );
    assert_eq!(
        cand.midi_sources[&f0_src].division,
        h.controller.live().midi_sources[&f0_src].division
    );

    // Save → reload: SMF documents, divisions, and projected note
    // ticks round-trip byte/value-exact.
    let mut store = InMemoryStore::new();
    h.controller.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live(), h.controller.live());
    assert_eq!(
        c2.live().midi_sources[&f0_src]
            .notes
            .values()
            .map(|n| (n.start, n.len))
            .collect::<Vec<_>>(),
        notes0
    );
    assert_eq!(
        c2.live().midi_sources[&f1_src].division.ticks_per_quarter(),
        96
    );
    // The SMF1 document's ordered event stream — tempo map, meta,
    // CC, sysex-free channel events — is preserved verbatim.
    let doc1_after: &SmfDocument = &c2.live().smf_documents[&f1_src];
    assert_eq!(doc1_after.format, synvelis_project::SmfFormat::Format1);
    assert_eq!(doc1_after.division, 96);
    assert_eq!(doc1_after.tracks.len(), 3);
}

// ===========================================================================
// CB-MIDI-04 + CB-MIDI-RCP-01: same-context pin identity, scoped-only
// invalidation, context-change staleness
// ===========================================================================

/// Two branches of one set start with identical scope content and
/// shared context. A scoped candidate edit moves only the candidate's
/// `scope_digest` — the adopted/live branch's render identity and the
/// shared-context digest are unchanged, so only the edited
/// candidate's derived render is stale. An out-of-scope live edit
/// moves `shared_context_digest` for every branch — the comparison
/// context is stale and Exact re-display is impossible (CB-MIDI-04
/// semantics at the identity layer; orchestration is CB-08 scope).
#[test]
fn cb_midi_04_rcp_01_context_pin_scoped_invalidation() {
    let mut c = controller();
    let fx = midi_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = midi_scope(fx.clip);
    let forked = h.fork(scope.clone());
    let (set, adopted, cand) = (forked.variant_set, forked.adopted, forked.candidate);

    let pin_adopted_0 = h.render_pin(set, adopted);
    let pin_cand_0 = h.render_pin(set, cand);
    assert_eq!(pin_adopted_0.scope_digest, pin_cand_0.scope_digest);
    assert_eq!(
        pin_adopted_0.shared_context_digest,
        pin_cand_0.shared_context_digest
    );

    // Scoped candidate edit → only the candidate's scope digest moves.
    h.edit(
        set,
        cand,
        ProjectOp::SetMidiNoteVelocity {
            source_id: fx.source,
            note_id: fx.notes[0],
            velocity: 90,
        },
    );
    let pin_adopted_1 = h.render_pin(set, adopted);
    let pin_cand_1 = h.render_pin(set, cand);
    assert_ne!(pin_cand_0.scope_digest, pin_cand_1.scope_digest);
    assert_eq!(
        pin_adopted_0.scope_digest, pin_adopted_1.scope_digest,
        "live-mirror branch render identity must not move on a scoped edit"
    );
    assert_eq!(
        pin_cand_0.shared_context_digest, pin_cand_1.shared_context_digest,
        "scoped edits never touch shared context"
    );
    assert_eq!(
        pin_adopted_1.shared_context_digest,
        pin_cand_1.shared_context_digest
    );

    // Out-of-scope live edit (new lane = non-scope semantic object)
    // → shared context digest moves for BOTH branches: any comparison
    // pinned to the old context is stale.
    let track = *h.controller.live().tracks.keys().next().unwrap();
    apply_op(&mut h.controller, ProjectOp::AddLane { track_id: track });
    let pin_adopted_2 = h.render_pin(set, adopted);
    let pin_cand_2 = h.render_pin(set, cand);
    assert_ne!(
        pin_adopted_1.shared_context_digest,
        pin_adopted_2.shared_context_digest
    );
    assert_ne!(
        pin_cand_1.shared_context_digest,
        pin_cand_2.shared_context_digest
    );
    // Scope content itself is unchanged by the context edit.
    assert_eq!(pin_adopted_1.scope_digest, pin_adopted_2.scope_digest);
    assert_eq!(pin_cand_1.scope_digest, pin_cand_2.scope_digest);

    // The render-resolved candidate project keeps native MIDI state:
    // division, note ticks and SMF provenance pass through the read
    // adapter unquantized.
    let resolved = resolve_render_project(
        h.controller.live(),
        Some(RenderBranchTarget {
            variant_set: set,
            branch: cand,
            store: &h.store,
        }),
    )
    .unwrap()
    .project;
    assert_eq!(
        resolved.midi_sources[&fx.source].division,
        MidiDivision::ppqn(480).unwrap()
    );
}

// ===========================================================================
// CB-MIDI-05: adopt = 1 CAS + 1 Undo; Undo/Redo keeps A and B
// ===========================================================================

#[test]
fn cb_midi_05_adopt_is_single_cas_with_undo_redo() {
    let mut c = controller();
    let fx = midi_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = midi_scope(fx.clip);
    let forked = h.fork(scope.clone());
    let (set, cand) = (forked.variant_set, forked.candidate);
    let n1 = fx.notes[0];

    h.edit(
        set,
        cand,
        ProjectOp::SetMidiNoteVelocity {
            source_id: fx.source,
            note_id: n1,
            velocity: 90,
        },
    );
    h.edit(
        set,
        cand,
        ProjectOp::AddMidiNote {
            source_id: fx.source,
            key: 67,
            velocity: 70,
            start: mt(720),
            len: mt(48),
        },
    );

    let candidate_digest = h.controller.live().variant_sets[&set].branches[&cand]
        .content
        .clone();
    let rev_before = h.controller.live_revision();
    let receipt = h.adopt(set, cand, &scope);
    // Exactly one project CAS: live revision advanced by one.
    assert_eq!(receipt.after_revision.0, rev_before.0 + 1);

    // Live now carries B′: velocity edit + added note materialized,
    // untouched clip/source identities preserved.
    let live_notes = &h.controller.live().midi_sources[&fx.source].notes;
    assert_eq!(live_notes[&n1].velocity, 90);
    assert_eq!(live_notes.len(), 3);
    assert_eq!(h.controller.live().clips[&fx.clip].source_id, fx.source);

    // Undo restores origin live state; the variant set keeps both
    // branches (adopted↔candidate swap back).
    assert!(h.controller.undo().unwrap());
    let undone_notes = &h.controller.live().midi_sources[&fx.source].notes;
    assert_eq!(undone_notes[&n1].velocity, 100);
    assert_eq!(undone_notes.len(), 2);
    let set_state = &h.controller.live().variant_sets[&set];
    assert_eq!(set_state.branches.len(), 2);
    // The candidate branch still points at the same B′ payload.
    assert_eq!(
        set_state.branches[&cand].content, candidate_digest,
        "undo must not drop or rewrite candidate content"
    );

    // Redo re-applies the adopt: live is B′ again, branches intact.
    assert!(h.controller.redo().unwrap());
    assert_eq!(
        h.controller.live().midi_sources[&fx.source].notes[&n1].velocity,
        90
    );
    assert_eq!(h.controller.live().variant_sets[&set].branches.len(), 2);
}

// ===========================================================================
// CB-MIDI-06: honest capabilities + typed rejection, nothing deleted
// or silently normalized
// ===========================================================================

#[test]
fn cb_midi_06_capabilities_match_enforcement() {
    let adapter = MidiClipAdapter::new();
    let caps = adapter.capabilities();
    assert_eq!(caps.note_add, ProcessingCapability::Supported);
    assert_eq!(caps.note_move, ProcessingCapability::Supported);
    assert_eq!(caps.note_velocity, ProcessingCapability::Supported);
    for unsupported in [
        caps.note_remove,
        caps.note_key,
        caps.note_len,
        caps.cc,
        caps.pitch_bend,
        caps.smf_backed_notes,
        caps.detach_in_candidate,
        caps.loop_playback,
        caps.stretch,
        caps.midi2_unknown_events,
    ] {
        assert_eq!(unsupported, ProcessingCapability::Unsupported);
    }
}

#[test]
fn cb_midi_06_unsupported_ops_reject_typed_and_preserve_state() {
    let mut c = controller();
    let fx = midi_fixture(&mut c);
    let mut h = Fx::new(c);
    let scope = midi_scope(fx.clip);
    let forked = h.fork(scope.clone());
    let (set, cand) = (forked.variant_set, forked.candidate);
    let content_before = h.controller.live().variant_sets[&set].branches[&cand]
        .content
        .clone();
    let live_before = midi_state_json(&h.controller);

    let n1 = fx.notes[0];
    // Every unsupported surface rejects typed — no panic, no silent
    // apply, no state drift.
    for op in [
        ProjectOp::RemoveMidiNote {
            source_id: fx.source,
            note_id: n1,
        },
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(96).unwrap(),
        },
        ProjectOp::RemoveMidiSource {
            source_id: fx.source,
        },
        ProjectOp::DetachClipSource { clip_id: fx.clip },
        ProjectOp::SetClipPlayback {
            clip_id: fx.clip,
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(4 * 960).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Loop,
            },
        },
        ProjectOp::SetClipPlayback {
            clip_id: fx.clip,
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(4 * 960).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::SimpleStretch(StretchRatio::new(2, 1).unwrap()),
            },
        },
        // Out-of-scope MIDI source: the second source exists in live
        // but is not owned by this clip's scope.
        ProjectOp::SetMidiNoteVelocity {
            source_id: {
                // fabricate an out-of-scope source id that does not
                // exist — confinement must reject before apply
                SourceId::new(0xFFFF_FFFF)
            },
            note_id: n1,
            velocity: 1,
        },
    ] {
        let err = h.try_edit(set, cand, op).unwrap_err();
        assert!(
            matches!(
                err,
                VariantError::Unsupported { .. }
                    | VariantError::ScopeViolation { .. }
                    | VariantError::Project(_)
            ),
            "expected typed rejection, got {err:?}"
        );
    }

    // Nothing moved: candidate payload ref, live MIDI state, clip.
    assert_eq!(
        h.controller.live().variant_sets[&set].branches[&cand].content,
        content_before
    );
    assert_eq!(midi_state_json(&h.controller), live_before);
}

/// SMF-provenance sources are immutable inside candidates too: note
/// ops hit the op's own `reject_smf_backed` contract (typed
/// `VariantError::Project`), and the only editable path — detach —
/// is live-side, before fork.
#[test]
fn cb_midi_06_smf_backed_source_rejects_candidate_note_ops() {
    let mut c = controller();
    let doc = parse_smf(&smf_format0_odd_ticks()).unwrap();
    apply_op(&mut c, ProjectOp::ImportMidiSmf { document: doc });
    let source = *c.live().smf_documents.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: musical_clip(0),
            playback: midi_window(),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();
    let note = *c.live().midi_sources[&source].notes.keys().next().unwrap();

    let mut h = Fx::new(c);
    let forked = h.fork(midi_scope(clip));
    let (set, cand) = (forked.variant_set, forked.candidate);

    for op in [
        ProjectOp::AddMidiNote {
            source_id: source,
            key: 60,
            velocity: 90,
            start: mt(0),
            len: mt(10),
        },
        ProjectOp::SetMidiNoteVelocity {
            source_id: source,
            note_id: note,
            velocity: 90,
        },
        ProjectOp::MoveMidiNote {
            source_id: source,
            note_id: note,
            start: mt(5),
        },
        ProjectOp::RemoveMidiNote {
            source_id: source,
            note_id: note,
        },
    ] {
        let err = h.try_edit(set, cand, op).unwrap_err();
        assert!(
            matches!(err, VariantError::Project(_)),
            "SMF-backed note op must fail via the op contract, got {err:?}"
        );
    }
    // Detach inside the candidate is Unsupported — editable copies
    // are made on live before fork.
    let err = h
        .try_edit(set, cand, ProjectOp::DetachClipSource { clip_id: clip })
        .unwrap_err();
    assert!(matches!(err, VariantError::Unsupported { .. }));

    // Live SMF document and projected notes are untouched.
    assert_eq!(h.controller.live().smf_documents.len(), 1);
    assert_eq!(h.controller.live().midi_sources[&source].notes.len(), 2);
}

/// Scope validation is the mirror of `AudioClipAdapter`: a clip that
/// does not read a MIDI source, a missing clip, and the CB-01 shared-
/// source ambiguity all reject typed at fork.
#[test]
fn cb_midi_scope_validation_mirror_and_shared_source_reject() {
    let mut c = controller();
    // Audio clip (media source) under a MidiClip scope → InvalidScope.
    apply_op(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::RegisterAsset { digest: "a".into() });
    let asset = *c.live().assets.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "d".into(),
        },
    );
    let media_source = *c.live().sources.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: media_source,
            placement: musical_clip(0),
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(960).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    let media_clip = *c.live().clips.keys().next().unwrap();
    let mut h = Fx::new(c);
    let err = h.try_fork(midi_scope(media_clip)).unwrap_err();
    assert!(matches!(
        err,
        VariantError::InvalidScope { .. } | VariantError::Project(_)
    ));

    // Missing clip → typed scope error.
    let err = h
        .try_fork(midi_scope(ClipId::new(0x1234_5678)))
        .unwrap_err();
    assert!(matches!(
        err,
        VariantError::InvalidScope { .. } | VariantError::Project(_)
    ));

    // Two clips sharing one MIDI source → ambiguous ownership reject
    // (CB-01 contract, now enforced through this adapter's
    // validate_scope → scope_objects path).
    let mut c2 = controller();
    apply_op(&mut c2, ProjectOp::AddTrack { name: "t".into() });
    let track2 = *c2.live().tracks.keys().next().unwrap();
    apply_op(&mut c2, ProjectOp::AddLane { track_id: track2 });
    apply_op(&mut c2, ProjectOp::AddLane { track_id: track2 });
    let mut lanes: Vec<LaneId> = c2.live().lanes.keys().copied().collect();
    lanes.sort();
    apply_op(
        &mut c2,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let shared = *c2.live().midi_sources.keys().next().unwrap();
    for lane in lanes {
        apply_op(
            &mut c2,
            ProjectOp::AddClip {
                lane_id: lane,
                source_id: shared,
                placement: musical_clip(0),
                playback: midi_window(),
            },
        );
    }
    let mut h2 = Fx::new(c2);
    let any_clip = *h2.controller.live().clips.keys().next().unwrap();
    let err = h2.try_fork(midi_scope(any_clip)).unwrap_err();
    assert!(matches!(err, VariantError::Project(_)));
}
