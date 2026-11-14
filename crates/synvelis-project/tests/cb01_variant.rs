//! CB-01 Creative Branch headless fixtures (synvelis#278).
//!
//! Covers the model/controller contract at the Fake Domain Adapter
//! level: scope typing + schema roundtrip, fork/edit/adopt/discard
//! CAS semantics, stale-rejection fail-closed behavior, live-space ID
//! reservation, lineage, typed rejections, and the project-commit /
//! runtime-publish receipt split (CB-MODEL-01..08, CB-MODEL-RCP-01
//! fixture portion). Real OS / plugin / RT / render lanes are `NotRun`
//! — recorded in `evidence/sv278/`.

use std::collections::BTreeSet;

use synvelis_project::apply::{ApplyApproval, PluginAvailability};
use synvelis_project::control::BeatPoint;
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::id::{CounterIdGenerator, ProjectId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::placement::{
    ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, TimeAnchor,
};
use synvelis_project::project::Project;
use synvelis_project::range::{MusicalRange, SourceRange};
use synvelis_project::routing::{PluginRef, ProcessorBinding};
use synvelis_project::snapshot::{ContentDigest, SnapshotCancel, SnapshotResolver};
use synvelis_project::store::{InMemoryStore, ProjectStore, SCHEMA_VERSION};
use synvelis_project::time::{MusicalDuration, MusicalTick, SourceFrame};
use synvelis_project::variant::{
    AdoptIntent, AdoptOutcome, AdoptSources, ArrangementDomainMask, BranchContentRef,
    BranchContentStore, BranchEditIntent, BranchRevision, FakeDomainAdapter, FakeRuntimePublish,
    InMemoryBranchContentStore, NoopRuntimePublish, ProcessingRangeRef, RuntimePublish,
    RuntimePublishState, StripDomainMask, StripField, TakeUsageTarget, VariantError, VariantScope,
    adopt_branch, decode_candidate, discard_branch, edit_branch, fork_variant_set, scope_digest,
};
use synvelis_project::{
    BranchId, ChainId, ClipId, LaneId, NoteId, ProcessorId, StripId, TakeGroupId, TrackId,
    VariantSetId,
};

// ── fixtures ───────────────────────────────────────────────────────

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "cb-fixture"),
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

fn clip_placement(anchor_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
    }
}

fn clip_playback() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::ZERO,
            SourceFrame::new(1_000_000).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

/// Track → lane → asset+source → one audio clip → one clip note.
fn audio_fixture() -> (ProjectController, TrackId, LaneId, ClipId, NoteId) {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::RegisterAsset {
            digest: "asset-a".into(),
        },
    );
    let asset = *c.live().assets.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "digest-1".into(),
        },
    );
    let source = *c.live().sources.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: clip_placement(0),
            playback: clip_playback(),
        },
    );
    let clip = *c.live().clips.keys().next_back().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddNote {
            clip,
            key: 60,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    let note = *c.live().notes.keys().next_back().unwrap();
    (c, track, lane, clip, note)
}

struct Fixture {
    controller: ProjectController,
    store: InMemoryBranchContentStore,
    adapter: FakeDomainAdapter,
    resolver: SnapshotResolver,
}

impl Fixture {
    fn new(controller: ProjectController) -> Self {
        Self {
            controller,
            store: InMemoryBranchContentStore::default(),
            adapter: FakeDomainAdapter::new(),
            resolver: SnapshotResolver::new(),
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

    fn edit(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        expected: BranchRevision,
        op: ProjectOp,
    ) -> synvelis_project::variant::EditReceipt {
        edit_branch(
            &mut self.controller,
            &self.adapter,
            &mut self.store,
            &BranchEditIntent {
                variant_set: set,
                branch,
                expected_branch_revision: expected,
                op,
            },
        )
        .unwrap()
    }

    fn intent(&self, set: VariantSetId, branch: BranchId, scope: &VariantScope) -> AdoptIntent {
        let set_state = &self.controller.live().variant_sets[&set];
        AdoptIntent {
            variant_set: set,
            branch,
            expected_project_revision: self.controller.live_revision(),
            expected_branch_revision: set_state.branches[&branch].content.revision(),
            expected_scope_digest: scope_digest(self.controller.live(), scope).unwrap(),
            approval: ApplyApproval {
                ticket: "approve".into(),
            },
        }
    }

    fn adopt(
        &mut self,
        set: VariantSetId,
        branch: BranchId,
        scope: &VariantScope,
        runtime: &dyn RuntimePublish,
    ) -> synvelis_project::variant::AdoptReceipt {
        let intent = self.intent(set, branch, scope);
        adopt_branch(
            &mut self.controller,
            &mut self.store,
            &AdoptSources {
                assets: None,
                plugins: None,
                runtime,
            },
            &intent,
        )
        .unwrap()
    }

    fn adopt_raw(
        &mut self,
        intent: &AdoptIntent,
        sources: &AdoptSources<'_>,
    ) -> Result<synvelis_project::variant::AdoptReceipt, VariantError> {
        adopt_branch(&mut self.controller, &mut self.store, sources, intent)
    }
}

fn audio_scope(clip: ClipId) -> VariantScope {
    VariantScope::AudioClip { clip }
}

fn arrangement_scope(track: TrackId) -> VariantScope {
    VariantScope::Arrangement {
        tracks: vec![track],
        time: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(100_000)).unwrap(),
        fields: ArrangementDomainMask {
            clips: true,
            notes: true,
        },
    }
}

// ── CB-MODEL-01: typed scopes + schema roundtrip ───────────────────

#[test]
fn cb_model_01_scope_typed_serde_roundtrip() {
    let scopes = vec![
        VariantScope::AudioClip {
            clip: ClipId::new(11),
        },
        VariantScope::TakeUsage {
            target: TakeUsageTarget {
                take_group: TakeGroupId::new(12),
            },
        },
        VariantScope::MidiClip {
            clip: ClipId::new(13),
        },
        VariantScope::InstrumentSlot {
            processor: ProcessorId::new(14),
        },
        VariantScope::ProcessingRange {
            range: ProcessingRangeRef {
                chain: ChainId::new(15),
                start: ProcessorId::new(16),
                end: ProcessorId::new(17),
            },
        },
        VariantScope::StripFields {
            strip: StripId::new(18),
            fields: StripDomainMask(BTreeSet::from([StripField::Fader, StripField::Mute])),
        },
        arrangement_scope(TrackId::new(19)),
    ];
    for scope in &scopes {
        let json = serde_json::to_string(scope).unwrap();
        let back: VariantScope = serde_json::from_str(&json).unwrap();
        assert_eq!(*scope, back);
        // The wire form is self-describing — a swapped kind tag fails.
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value["kind"] = serde_json::json!("bogus");
        assert!(serde_json::from_value::<VariantScope>(value).is_err());
    }
}

#[test]
fn cb_model_01_variant_set_save_load_roundtrip() {
    let (c, _track, _lane, clip, _note) = audio_fixture();
    let mut fx = Fixture::new(c);
    let receipt = fx.fork(audio_scope(clip));
    let scope = fx.controller.live().variant_sets[&receipt.variant_set]
        .scope
        .clone();

    // Persist through the canonical store path and reload.
    let mut store = InMemoryStore::new();
    fx.controller.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();

    let set = &c2.live().variant_sets[&receipt.variant_set];
    assert_eq!(set.scope, scope);
    assert_eq!(set.adopted, receipt.adopted);
    assert_eq!(set.branches.len(), 2);
    assert!(matches!(
        set.branches[&receipt.candidate].content,
        BranchContentRef::Captured { .. }
    ));
    // Stable IDs / assets / HEAD survive untouched.
    assert_eq!(c2.live().assets.len(), 1);
    assert_eq!(c2.live().clips.len(), 1);
    assert!(c2.saved().is_some());
}

#[test]
fn cb_model_01_v13_snapshot_migrates_to_v14() {
    let (c, _track, _lane, clip, _note) = audio_fixture();
    let mut fx = Fixture::new(c);
    fx.fork(audio_scope(clip));

    // Emulate a genuine v13 body: rewrite the envelope's schema version
    // and drop the v14-only field.
    let mut store = InMemoryStore::new();
    fx.controller.save(&mut store).unwrap();
    let head = store.head().unwrap().unwrap();
    let bytes = store.read_snapshot(&head).unwrap();
    let mut env: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    env["schema_version"] = serde_json::json!(13);
    env["project"]
        .as_object_mut()
        .unwrap()
        .remove("variant_sets");
    let v13 = serde_json::to_vec(&env).unwrap();
    let mut store13 = InMemoryStore::new();
    store13
        .save(
            &fx.controller.live().project_id.to_tagged(),
            fx.controller.live_revision(),
            &v13,
        )
        .unwrap();

    let mut c2 = controller();
    c2.load(&store13).unwrap();
    assert!(c2.live().variant_sets.is_empty());
    assert_eq!(c2.live().clips.len(), 1);
    assert_eq!(c2.live().assets.len(), 1);
    // Migrated load counts as one implicit revision step.
    assert_eq!(c2.live_revision().0, fx.controller.live_revision().0 + 1);
    assert_eq!(SCHEMA_VERSION, 14);
}

// ── CB-MODEL-02: non-adopted edits never touch live ─────────────────

#[test]
fn cb_model_02_candidate_edit_leaves_live_untouched() {
    let (c, _t, _l, clip, _n) = audio_fixture();
    let mut fx = Fixture::new(c);
    let live_before = fx.controller.live().clone();
    let receipt = fx.fork(audio_scope(clip));

    fx.edit(
        receipt.variant_set,
        receipt.candidate,
        BranchRevision::new(1),
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );

    // Live domain state is bit-identical outside variant bookkeeping.
    let live = fx.controller.live();
    assert_eq!(live.clips, live_before.clips);
    assert!(!live.clips[&clip].muted);
    let set = &live.variant_sets[&receipt.variant_set];
    assert_eq!(
        set.branches[&receipt.candidate].content.revision(),
        BranchRevision::new(2)
    );
    assert!(matches!(
        set.branches[&receipt.adopted].content,
        BranchContentRef::Live {
            revision: BranchRevision(1)
        }
    ));
}

// ── CB-MODEL-03: adopted edits are one CAS, undo/redo/save stable ───

#[test]
fn cb_model_03_adopted_edit_single_cas_and_roundtrip() {
    let (c, _t, _l, clip, _n) = audio_fixture();
    let mut fx = Fixture::new(c);
    let receipt = fx.fork(audio_scope(clip));
    let rev_before = fx.controller.live_revision();

    let edit = fx.edit(
        receipt.variant_set,
        receipt.adopted,
        BranchRevision::new(1),
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );
    assert!(fx.controller.live().clips[&clip].muted);
    assert_eq!(edit.project_revision.0, rev_before.0 + 1);
    // Scope fingerprint moved → branch content revision bumped in the
    // same CAS.
    let set = &fx.controller.live().variant_sets[&receipt.variant_set];
    assert_eq!(
        set.branches[&receipt.adopted].content.revision(),
        BranchRevision::new(2)
    );

    // Undo restores clip AND branch bookkeeping together.
    assert!(fx.controller.undo().unwrap());
    assert!(!fx.controller.live().clips[&clip].muted);
    let set = &fx.controller.live().variant_sets[&receipt.variant_set];
    assert_eq!(
        set.branches[&receipt.adopted].content.revision(),
        BranchRevision::new(1)
    );
    assert!(fx.controller.redo().unwrap());
    assert!(fx.controller.live().clips[&clip].muted);

    // Save/reload keeps the same state — no divergence.
    let mut store = InMemoryStore::new();
    fx.controller.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert!(c2.live().clips[&clip].muted);
    let set = &c2.live().variant_sets[&receipt.variant_set];
    assert_eq!(
        set.branches[&receipt.adopted].content.revision(),
        BranchRevision::new(2)
    );
}

// ── CB-MODEL-04: stale adopt fails closed ───────────────────────────

#[test]
fn cb_model_04_stale_adopt_rejected_without_partial_apply() {
    let (c, _t, _l, clip, _n) = audio_fixture();
    let mut fx = Fixture::new(c);
    let receipt = fx.fork(audio_scope(clip));
    fx.edit(
        receipt.variant_set,
        receipt.candidate,
        BranchRevision::new(1),
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );
    let live_before = fx.controller.live().clone();
    let rev_before = fx.controller.live_revision();

    let runtime = NoopRuntimePublish;
    let sources = AdoptSources {
        assets: None,
        plugins: None,
        runtime: &runtime,
    };
    let base = fx.intent(receipt.variant_set, receipt.candidate, &audio_scope(clip));

    // Stale project revision.
    let mut intent = base.clone();
    intent.expected_project_revision = ProjectRevision(rev_before.0 - 1);
    let err = fx.adopt_raw(&intent, &sources).unwrap_err();
    assert!(matches!(err, VariantError::StaleProjectRevision { .. }));

    // Stale branch revision.
    let mut intent = base.clone();
    intent.expected_branch_revision = BranchRevision::new(1);
    let err = fx.adopt_raw(&intent, &sources).unwrap_err();
    assert!(matches!(err, VariantError::StaleBranchRevision { .. }));

    // Stale scope digest.
    let mut intent = base.clone();
    intent.expected_scope_digest = ContentDigest::sha256(b"other");
    let err = fx.adopt_raw(&intent, &sources).unwrap_err();
    assert!(matches!(err, VariantError::StaleScope { .. }));

    // Unapproved.
    let mut intent = base;
    intent.approval.ticket = String::new();
    let err = fx.adopt_raw(&intent, &sources).unwrap_err();
    assert!(matches!(err, VariantError::Unapproved));

    // Nothing moved: live identical, revision identical — no partial
    // application in project, undo history, HEAD or candidates.
    assert_eq!(*fx.controller.live(), live_before);
    assert_eq!(fx.controller.live_revision(), rev_before);
}

// ── CB-MODEL-05: multi-object adopt = 1 commit + 1 undo ────────────

#[test]
fn cb_model_05_multi_object_adopt_single_commit_single_undo() {
    let (c, _t, _l, clip, orig_note) = audio_fixture();
    let mut fx = Fixture::new(c);
    let receipt = fx.fork(audio_scope(clip));
    // Two scoped changes in the candidate: mute the clip AND add a
    // clip-local note — the AudioClip scope owns both.
    fx.edit(
        receipt.variant_set,
        receipt.candidate,
        BranchRevision::new(1),
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );
    fx.edit(
        receipt.variant_set,
        receipt.candidate,
        BranchRevision::new(2),
        ProjectOp::AddNote {
            clip,
            key: 64,
            start: BeatPoint::new(0, 2).unwrap(),
            len: BeatPoint::new(1, 2).unwrap(),
        },
    );

    let rev_before = fx.controller.live_revision();
    let notes_before = fx.controller.live().notes.len();
    let adopt = fx.adopt(
        receipt.variant_set,
        receipt.candidate,
        &audio_scope(clip),
        &NoopRuntimePublish,
    );
    assert_eq!(adopt.outcome, AdoptOutcome::Applied);
    assert!(adopt.objects >= 2, "objects={}", adopt.objects);
    assert_eq!(adopt.project.undo_steps, 1);
    assert_eq!(adopt.after_revision.0, rev_before.0 + 1);
    assert!(fx.controller.live().clips[&clip].muted);
    assert_eq!(fx.controller.live().notes.len(), notes_before + 1);
    let minted_note = *fx
        .controller
        .live()
        .notes
        .keys()
        .find(|id| **id != orig_note)
        .unwrap();

    // One undo reverts the whole adopt — clip AND note together.
    assert!(fx.controller.undo().unwrap());
    assert!(!fx.controller.live().clips[&clip].muted);
    assert_eq!(fx.controller.live().notes.len(), notes_before);
    assert!(!fx.controller.live().notes.contains_key(&minted_note));

    // Redo re-applies with identical IDs — no reissue.
    assert!(fx.controller.redo().unwrap());
    assert!(fx.controller.live().clips[&clip].muted);
    assert!(fx.controller.live().notes.contains_key(&minted_note));
}

// ── CB-MODEL-06: branch-local IDs + lineage ─────────────────────────

#[test]
fn cb_model_06_branch_local_ids_no_collision_and_lineage() {
    let (c, track, lane, clip, _n) = audio_fixture();
    let mut fx = Fixture::new(c);
    let scope = arrangement_scope(track);
    let live_ids_before: Vec<String> = fx
        .controller
        .live()
        .clips
        .keys()
        .map(|c| c.to_tagged())
        .collect();
    let receipt = fx.fork(scope.clone());

    // Branch-local clip creation inside the arrangement scope.
    fx.edit(
        receipt.variant_set,
        receipt.candidate,
        BranchRevision::new(1),
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: fx.controller.live().clips[&clip].source_id,
            placement: clip_placement(8 * 960),
            playback: clip_playback(),
        },
    );

    let set = &fx.controller.live().variant_sets[&receipt.variant_set];
    let branch = &set.branches[&receipt.candidate];
    // Lineage recorded the minted clip id.
    assert_eq!(branch.lineage.local_ids.len(), 1);
    let minted_id = branch.lineage.local_ids.iter().next().unwrap().0.clone();
    assert!(minted_id.starts_with("clip-"));
    // It cannot collide with any live id.
    assert!(!live_ids_before.contains(&minted_id));

    // Adopt materializes the new clip under the SAME id — no remap.
    fx.adopt(
        receipt.variant_set,
        receipt.candidate,
        &scope,
        &NoopRuntimePublish,
    );
    assert!(
        fx.controller
            .live()
            .clips
            .contains_key(&ClipId::from_tagged(&minted_id).unwrap())
    );
}

#[test]
fn cb_model_06_candidate_payload_digest_is_verified() {
    let (c, _t, _l, clip, _n) = audio_fixture();
    let mut fx = Fixture::new(c);
    let receipt = fx.fork(audio_scope(clip));
    fx.edit(
        receipt.variant_set,
        receipt.candidate,
        BranchRevision::new(1),
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );
    let set = &fx.controller.live().variant_sets[&receipt.variant_set];
    let BranchContentRef::Captured { .. } = set.branches[&receipt.candidate].content else {
        panic!("candidate must be captured");
    };
    // Bytes stored under a *different* digest than declared → the
    // decode oracle rejects (digest mismatch), never silently loads.
    let mut foreign = fx.controller.live().clone();
    foreign.variant_sets.clear();
    let bytes = serde_json::to_vec(&foreign).unwrap();
    let wrong_digest = ContentDigest::sha256(b"not-the-payload");
    fx.store.put(&wrong_digest, &bytes).unwrap();
    let forged = BranchContentRef::Captured {
        revision: BranchRevision::new(2),
        payload_digest: wrong_digest,
        scope_digest: ContentDigest::sha256(b"x"),
        schema_version: SCHEMA_VERSION,
    };
    assert!(decode_candidate(&fx.store, &forged).is_err());
    // And the genuine payload still decodes.
    let ok = decode_candidate(&fx.store, &set.branches[&receipt.candidate].content).unwrap();
    assert_eq!(ok.clips.len(), 1);
}

// ── CB-MODEL-07: typed rejections, no silent replacement ───────────

#[test]
fn cb_model_07_overlapping_scope_rejected() {
    let (c, track, _l, clip, _n) = audio_fixture();
    let mut fx = Fixture::new(c);
    fx.fork(audio_scope(clip));
    let err = fork_variant_set(
        &mut fx.controller,
        &fx.adapter,
        &mut fx.store,
        &fx.resolver,
        arrangement_scope(track),
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        VariantError::ScopeConflict { .. } | VariantError::Project(_)
    ));
}

#[test]
fn cb_model_07_invalid_scope_and_unsupported_op_rejected() {
    let (c, _track, _lane, clip, _note) = audio_fixture();
    let mut fx = Fixture::new(c);

    // Empty track selection.
    let err = fork_variant_set(
        &mut fx.controller,
        &fx.adapter,
        &mut fx.store,
        &fx.resolver,
        VariantScope::Arrangement {
            tracks: vec![],
            time: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(1)).unwrap(),
            fields: ArrangementDomainMask {
                clips: true,
                notes: false,
            },
        },
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(matches!(err, VariantError::Project(_)));

    // Empty domain mask.
    let err = fork_variant_set(
        &mut fx.controller,
        &fx.adapter,
        &mut fx.store,
        &fx.resolver,
        VariantScope::StripFields {
            strip: StripId::new(1),
            fields: StripDomainMask::default(),
        },
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(matches!(err, VariantError::Project(_)));

    // Unknown root reference.
    let err = fork_variant_set(
        &mut fx.controller,
        &fx.adapter,
        &mut fx.store,
        &fx.resolver,
        VariantScope::AudioClip {
            clip: ClipId::new(999),
        },
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(matches!(err, VariantError::Project(_)));

    // Unsupported edit op on a candidate fails typed, not silently.
    let receipt = fx.fork(audio_scope(clip));
    let err = edit_branch(
        &mut fx.controller,
        &fx.adapter,
        &mut fx.store,
        &BranchEditIntent {
            variant_set: receipt.variant_set,
            branch: receipt.candidate,
            expected_branch_revision: BranchRevision::new(1),
            op: ProjectOp::AddTrack {
                name: "out-of-scope".into(),
            },
        },
    )
    .unwrap_err();
    assert!(matches!(err, VariantError::Unsupported { .. }));
}

#[test]
fn cb_model_07_shared_midi_source_scope_rejected() {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::AddMidiSource {
            division: synvelis_project::MidiDivision::ppqn(480).unwrap(),
        },
    );
    let source = *c.live().midi_sources.keys().next().unwrap();
    for anchor in [0, 4 * 960] {
        apply_op(
            &mut c,
            ProjectOp::AddClip {
                lane_id: lane,
                source_id: source,
                placement: clip_placement(anchor),
                playback: ClipPlayback {
                    source_region: SourceRange::try_new(
                        SourceFrame::ZERO,
                        SourceFrame::new(4 * 960).unwrap(),
                    )
                    .unwrap(),
                    mapping: PlaybackMapping::Natural,
                },
            },
        );
    }
    let clips: Vec<ClipId> = c.live().clips.keys().copied().collect();
    assert_eq!(clips.len(), 2);

    let mut fx = Fixture::new(c);
    // Both clips share one MidiSource — ownership is ambiguous.
    let err = fork_variant_set(
        &mut fx.controller,
        &fx.adapter,
        &mut fx.store,
        &fx.resolver,
        VariantScope::MidiClip { clip: clips[0] },
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(matches!(err, VariantError::Project(_)));
}

// ── CB-MODEL-08: project commit ≠ runtime publish ───────────────────

#[test]
fn cb_model_08_runtime_failure_keeps_project_committed() {
    let (c, _t, _l, clip, _n) = audio_fixture();
    let mut fx = Fixture::new(c);
    let receipt = fx.fork(audio_scope(clip));
    fx.edit(
        receipt.variant_set,
        receipt.candidate,
        BranchRevision::new(1),
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );

    let failing = FakeRuntimePublish::new(RuntimePublishState::Failed {
        reason: "engine offline".into(),
    });
    let adopt = fx.adopt(
        receipt.variant_set,
        receipt.candidate,
        &audio_scope(clip),
        &failing,
    );
    // Project committed; runtime reported failed — separate receipts.
    assert!(adopt.project.committed);
    assert_eq!(adopt.project.undo_steps, 1);
    assert!(matches!(
        adopt.runtime.state,
        RuntimePublishState::Failed { .. }
    ));
    assert!(fx.controller.live().clips[&clip].muted);
    // Undo still works — the commit was real.
    assert!(fx.controller.undo().unwrap());
    assert!(!fx.controller.live().clips[&clip].muted);
}

#[test]
fn cb_model_08_pending_publish_is_not_reported_as_current() {
    let (c, _t, _l, clip, _n) = audio_fixture();
    let mut fx = Fixture::new(c);
    let receipt = fx.fork(audio_scope(clip));
    fx.edit(
        receipt.variant_set,
        receipt.candidate,
        BranchRevision::new(1),
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );
    let pending = FakeRuntimePublish::new(RuntimePublishState::Pending {
        reason: "graph rebuild queued".into(),
    });
    let adopt = fx.adopt(
        receipt.variant_set,
        receipt.candidate,
        &audio_scope(clip),
        &pending,
    );
    assert!(adopt.project.committed);
    assert!(matches!(
        adopt.runtime.state,
        RuntimePublishState::Pending { .. }
    ));
}

// ── CB-MODEL-RCP-01 (fixture portion): reuse identity + safe adopt ──

#[test]
fn cb_model_rcp_01_scope_digest_is_render_reuse_identity() {
    let (c, _t, _l, clip, _n) = audio_fixture();
    let mut fx = Fixture::new(c);
    let live_digest = scope_digest(fx.controller.live(), &audio_scope(clip)).unwrap();
    let receipt = fx.fork(audio_scope(clip));
    let set = &fx.controller.live().variant_sets[&receipt.variant_set];
    let BranchContentRef::Captured { scope_digest, .. } = set.branches[&receipt.candidate].content
    else {
        panic!("candidate must be captured");
    };
    // Fork candidate == live scope content → identical digest: the
    // cross-branch/adopt render-reuse key.
    assert_eq!(scope_digest, live_digest);
}

struct NoPlugins;

impl PluginAvailability for NoPlugins {
    fn plugin_available(&self, _plugin: &PluginRef) -> bool {
        false
    }
}

/// Strip with a two-processor chain inside one `ProcessingRange` scope.
fn processor_fixture() -> (
    ProjectController,
    synvelis_project::StripId,
    ProcessingRangeRef,
) {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    let plugin = |d: &str| {
        ProcessorBinding::Resolved(PluginRef {
            descriptor: d.into(),
            version: "1.0".into(),
            capability_revision: 1,
        })
    };
    for (index, name) in [(0usize, "p1"), (1, "p2")] {
        apply_op(
            &mut c,
            ProjectOp::InsertProcessor {
                strip_id: strip,
                index,
                name: name.into(),
                binding: plugin(if index == 0 { "plug-a" } else { "plug-b" }),
                sidechain_input: false,
            },
        );
    }
    let chain = c.live().routing.strips[&strip].chain;
    let procs = c.live().routing.chains[&chain].processors.clone();
    assert_eq!(procs.len(), 2);
    let range = ProcessingRangeRef {
        chain,
        start: procs[0],
        end: procs[1],
    };
    (c, strip, range)
}

#[test]
fn cb_model_rcp_01_missing_plugin_safe_adopt_and_reject() {
    let (c, _strip, range) = processor_fixture();
    let scope = VariantScope::ProcessingRange { range };
    let mut fx = Fixture::new(c);
    let receipt = fx.fork(scope.clone());

    // Safe adopt: the host lacks the plugins, but the candidate
    // introduces no *new* resolved binding — structural state, plugin
    // identity, semantic ports and control targets all validate, so
    // the project commit proceeds (runtime reports plugin
    // availability separately, never blocking project truth).
    let intent = fx.intent(receipt.variant_set, receipt.candidate, &scope);
    let sources = AdoptSources {
        assets: None,
        plugins: Some(&NoPlugins),
        runtime: &NoopRuntimePublish,
    };
    let adopt = fx.adopt_raw(&intent, &sources).unwrap();
    assert!(adopt.project.committed);

    // Missing-plugin reject: a candidate that *adds* a processor bound
    // to an unavailable plugin fails typed — no partial application.
    let (c2, strip2, range2) = processor_fixture();
    let scope2 = VariantScope::ProcessingRange { range: range2 };
    let mut fx2 = Fixture::new(c2);
    let receipt2 = fx2.fork(scope2.clone());
    fx2.edit(
        receipt2.variant_set,
        receipt2.candidate,
        BranchRevision::new(1),
        ProjectOp::InsertProcessor {
            strip_id: strip2,
            index: 1,
            name: "p3".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: "plug-missing".into(),
                version: "9.9".into(),
                capability_revision: 1,
            }),
            sidechain_input: false,
        },
    );
    let live_before = fx2.controller.live().clone();
    let intent2 = fx2.intent(receipt2.variant_set, receipt2.candidate, &scope2);
    let err = fx2
        .adopt_raw(
            &intent2,
            &AdoptSources {
                assets: None,
                plugins: Some(&NoPlugins),
                runtime: &NoopRuntimePublish,
            },
        )
        .unwrap_err();
    assert!(
        matches!(err, VariantError::MissingPlugin { .. }),
        "expected MissingPlugin, got {err:?}"
    );
    assert_eq!(*fx2.controller.live(), live_before);
}

// ── discard ────────────────────────────────────────────────────────

#[test]
fn discard_branch_removes_candidate_only() {
    let (c, _t, _l, clip, _n) = audio_fixture();
    let mut fx = Fixture::new(c);
    let receipt = fx.fork(audio_scope(clip));
    fx.edit(
        receipt.variant_set,
        receipt.candidate,
        BranchRevision::new(1),
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );
    discard_branch(&mut fx.controller, receipt.variant_set, receipt.candidate).unwrap();
    let set = &fx.controller.live().variant_sets[&receipt.variant_set];
    assert_eq!(set.branches.len(), 1);
    assert_eq!(set.adopted, receipt.adopted);
    assert!(!fx.controller.live().clips[&clip].muted);
    // Discarding the adopted branch is rejected.
    assert!(discard_branch(&mut fx.controller, receipt.variant_set, receipt.adopted,).is_err());
}
