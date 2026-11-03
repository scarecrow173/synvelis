//! GW-03 context-projection tests — Issue #55 acceptance criteria.
//!
//! Each test names the contract it proves (DATA-01..05, DATA-08..10,
//! GT-004 / GTC-004): disclosure classes, snapshot-revision pinning,
//! expiring cursors, scope admission, budgets, and error hygiene.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{Value, json};
use synvelis_gateway::fixture::FixtureAdapter;
use synvelis_gateway::snapshot::attributes;
use synvelis_gateway::*;
use synvelis_project::{
    Asset, Clip, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Lane, LaneId, MidiDivision,
    MidiNote, MidiSource, MidiTick, MusicalDuration, MusicalTick, NoteId, PlaybackMapping, Project,
    ProjectId, ProjectRevision, Source, SourceFrame, SourceId, SourceRange, SourceState,
    TimeAnchor, Track, TrackId,
};

// ── Fixture ids ──────────────────────────────────────────────

fn project() -> ProjectId {
    ProjectId::new(7)
}
fn clip11() -> ResourceRef {
    ResourceRef::Clip(ClipId::new(11))
}
fn clip12() -> ResourceRef {
    ResourceRef::Clip(ClipId::new(12))
}
fn clip13() -> ResourceRef {
    ResourceRef::Clip(ClipId::new(13))
}
fn source9() -> ResourceRef {
    ResourceRef::Source(SourceId::new(9))
}
fn source10() -> ResourceRef {
    ResourceRef::Source(SourceId::new(10))
}
fn note20() -> ResourceRef {
    ResourceRef::Note(NoteId::new(20))
}
fn note21() -> ResourceRef {
    ResourceRef::Note(NoteId::new(21))
}
fn project_ref() -> ResourceRef {
    ResourceRef::Project(project())
}

fn task_ref(tag: &str) -> TaskRef {
    TaskRef::new(tag).unwrap()
}
fn grant_ref(tag: &str) -> GrantRef {
    GrantRef::new(tag).unwrap()
}
fn cap_ref(id: &str, major: u32, minor: u32) -> CapabilityRef {
    CapabilityRef {
        id: CapabilityId::new(id).unwrap(),
        version: CapabilityVersion::new(major, minor),
    }
}

fn adapter() -> FixtureAdapter {
    FixtureAdapter::for_testing()
}
fn agent_principal() -> Principal {
    Principal::new(PrincipalKind::Agent, "mcp-client-1").unwrap()
}
fn other_agent() -> CallerIdentity {
    adapter().agent("mcp-client-2").unwrap()
}
fn human_principal() -> Principal {
    Principal::new(PrincipalKind::Human, "user-1").unwrap()
}
fn agent() -> CallerIdentity {
    adapter().agent("mcp-client-1").unwrap()
}
fn human() -> CallerIdentity {
    adapter().human("user-1").unwrap()
}

// ── The demo project ─────────────────────────────────────────
//
// track3 ─ lane4 ─ clip11 → media source9 (asset5)
//               └ clip12 → midi source10 (notes 20/21, PPQN 480)

fn demo_project() -> Project {
    let mut p = Project::new(project(), "demo");
    let track = TrackId::new(3);
    let lane = LaneId::new(4);
    let src9 = SourceId::new(9);
    let src10 = SourceId::new(10);
    p.tracks.insert(
        track,
        Track {
            id: track,
            name: "t3".to_owned(),
            lane_ids: vec![lane],
        },
    );
    p.lanes.insert(
        lane,
        Lane {
            id: lane,
            track_id: track,
            clip_ids: vec![ClipId::new(11), ClipId::new(12)],
        },
    );
    p.assets.insert(
        synvelis_project::AssetId::new(5),
        Asset {
            id: synvelis_project::AssetId::new(5),
            digest: "sha256:asset5".to_owned(),
        },
    );
    p.sources.insert(
        src9,
        Source {
            id: src9,
            asset_id: synvelis_project::AssetId::new(5),
            content_digest: "sha256:media9".to_owned(),
            state: SourceState::Verified,
        },
    );
    let mut notes = BTreeMap::new();
    for (id, key, start, len) in [(20u128, 60u8, 0i64, 240i64), (21, 64, 480, 240)] {
        notes.insert(
            NoteId::new(id),
            MidiNote {
                id: NoteId::new(id),
                key,
                velocity: 100,
                start: MidiTick::new(start).unwrap(),
                len: MidiTick::new(len).unwrap(),
            },
        );
    }
    p.midi_sources.insert(
        src10,
        MidiSource {
            id: src10,
            division: MidiDivision::ppqn(480).unwrap(),
            notes,
        },
    );
    for (clip, source) in [(ClipId::new(11), src9), (ClipId::new(12), src10)] {
        p.clips.insert(
            clip,
            Clip {
                id: clip,
                lane_id: lane,
                source_id: source,
                placement: ClipPlacement {
                    anchor: TimeAnchor::Musical(MusicalTick::new(120)),
                    extent: ClipExtent::Musical(MusicalDuration::new(240).unwrap()),
                },
                playback: ClipPlayback {
                    source_region: SourceRange::try_new(
                        SourceFrame::new(0).unwrap(),
                        SourceFrame::new(48_000).unwrap(),
                    )
                    .unwrap(),
                    mapping: PlaybackMapping::Natural,
                },
                muted: false,
            },
        );
    }
    p
}

/// Adds clip13 on a fresh lane — the "project changed" event for
/// revision-divergence tests.
fn demo_project_v2() -> Project {
    let mut p = demo_project();
    let lane = LaneId::new(4);
    let clip = ClipId::new(13);
    p.lanes.get_mut(&lane).unwrap().clip_ids.push(clip);
    p.clips.insert(
        clip,
        Clip {
            id: clip,
            lane_id: lane,
            source_id: SourceId::new(10),
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(600)),
                extent: ClipExtent::Musical(MusicalDuration::new(120).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::new(0).unwrap(),
                    SourceFrame::new(240).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
            muted: false,
        },
    );
    p
}

// ── Rig ──────────────────────────────────────────────────────

/// The oracle reads the source's *latest* pin — divergence between
/// oracle facts and an older pinned snapshot is exactly the
/// Unknown-item case the contract requires.
struct LatestOracle {
    source: Arc<MemorySnapshotSource>,
}

impl ResourceOracle for LatestOracle {
    fn facts(&self, project: ProjectId, resource: &ResourceRef) -> ResourceFacts {
        let Some(rev) = self.source.latest_revision(project) else {
            return ResourceFacts::Unresolved;
        };
        let Some(state) = self.source.pin(project, rev) else {
            return ResourceFacts::Unresolved;
        };
        match attributes(&state, resource) {
            Some(attrs) => ResourceFacts::Known(attrs),
            None => ResourceFacts::Unresolved,
        }
    }
}

/// A selection source backed by a static view→facts map.
struct StaticSelection {
    views: BTreeMap<String, SelectionFacts>,
}

impl SelectionSource for StaticSelection {
    fn selection(&self, _project: ProjectId, view: &SelectionViewRef) -> SelectionFacts {
        self.views
            .get(view.as_str())
            .cloned()
            .unwrap_or(SelectionFacts::Unknown)
    }
}

struct Rig {
    store: Arc<InMemoryPolicyStore>,
    clock: Arc<ManualClock>,
    source: Arc<MemorySnapshotSource>,
    engine: Arc<ContextProjectionEngine>,
    gw: Gateway,
}

fn rig() -> Rig {
    rig_with(ContextBudget::default(), BTreeMap::new())
}

fn rig_with(budget: ContextBudget, views: BTreeMap<String, SelectionFacts>) -> Rig {
    let store = Arc::new(InMemoryPolicyStore::new());
    let clock = Arc::new(ManualClock::new(PolicyInstant::new(1_000)));
    let source = Arc::new(MemorySnapshotSource::new());
    source.commit(project(), ProjectRevision(1), demo_project());
    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: source.clone(),
    });
    let authorizer: Arc<dyn ContextAuthorizer> = Arc::new(GrantPolicy::new(
        store.clone(),
        clock.clone(),
        oracle.clone(),
    ));
    let engine = Arc::new(ContextProjectionEngine::new(
        source.clone(),
        Arc::new(StaticSelection { views }),
        authorizer,
        clock.clone(),
        budget,
    ));
    let mut gw = Gateway::with_evaluator(Box::new(GrantPolicy::new(
        store.clone(),
        clock.clone(),
        oracle,
    )));
    engine.install(&mut gw).unwrap();
    Rig {
        store,
        clock,
        source,
        engine,
        gw,
    }
}

// ── Authority builders ───────────────────────────────────────

fn issue_task(store: &InMemoryPolicyStore, tag: &str) {
    store.issue_task(AuthorizedTask {
        task: task_ref(tag),
        principal: agent_principal(),
        project: project(),
        activation: Activation::UserIssued,
        validity: Validity::default(),
    });
}

/// A context grant: `classes` data-access + `resources` exact scope.
fn ctx_grant(
    tag: &str,
    task_tag: &str,
    classes: &[DataAccessClass],
    resources: &[ResourceRef],
) -> Grant {
    ctx_grant_scope(
        tag,
        task_tag,
        classes,
        ResourceScope::Exact {
            resources: resources.iter().copied().collect(),
        },
    )
}

fn ctx_grant_scope(
    tag: &str,
    task_tag: &str,
    classes: &[DataAccessClass],
    scope: ResourceScope,
) -> Grant {
    Grant {
        id: grant_ref(tag),
        issued_by: human_principal(),
        principal: agent_principal(),
        project: project(),
        binding: GrantBinding::Task {
            task: task_ref(task_tag),
        },
        capabilities: BTreeSet::from([cap_ref("synvelis.context.query", 1, 0)]),
        permissions: PermissionSet::of([
            PermissionKind::ReadContext,
            PermissionKind::DiscloseContext,
        ]),
        data_access: DataAccessSet::of(classes.iter().copied()),
        scope,
        create_within: None,
        approval_policy: ApprovalPolicy::DelegatedApply,
        limits: GrantLimits::default(),
        validity: Validity::default(),
        epoch_ceiling: None,
        issued_at_revision: Some(ProjectRevision(0)),
    }
}

/// Metadata + external disclosure — the baseline external-agent
/// classes.
fn metadata_classes() -> Vec<DataAccessClass> {
    vec![
        DataAccessClass::MetadataRead,
        DataAccessClass::ExternalDisclosure,
    ]
}

fn issue_agent(store: &InMemoryPolicyStore, tag: &str, grant: Grant) {
    issue_task(store, tag);
    store.issue_grant(grant);
}

// ── Request builders ─────────────────────────────────────────

fn query_req(
    task: &str,
    grant: &str,
    args: impl Into<Value>,
    consistency: impl Into<Value>,
) -> Value {
    json!({
        "request_id": "req-ctx",
        "capability": {"id": "synvelis.context.query", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.context.query.input", "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "args": args.into(),
        "consistency": consistency.into(),
        "task": task,
        "grant": grant,
    })
}

fn latest() -> Value {
    json!({"kind": "latest"})
}
fn read_at(rev: u64) -> Value {
    json!({"kind": "read-at", "revision": rev})
}
fn fixed(snapshot: &Value) -> Value {
    json!({"kind": "fixed-snapshot", "snapshot": snapshot})
}

fn selector(args: impl Into<Value>) -> Value {
    json!({"selector": args.into()})
}
fn resources(refs: &[ResourceRef]) -> Value {
    json!({"resources": refs.iter().map(tagged).collect::<Vec<_>>()})
}
fn tagged(r: &ResourceRef) -> String {
    serde_json::to_value(r).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn complete(outcome: GatewayOutcome<Value>) -> Value {
    match outcome {
        GatewayOutcome::Completed { result } => result,
        other => panic!("expected Completed, got {other:?}"),
    }
}

fn deny_reason(err: &ErrorEnvelope) -> String {
    match &err.code {
        GatewayError::Denied { reason, .. } => reason.clone().unwrap_or_default(),
        other => panic!("expected Denied, got {other:?}"),
    }
}

// ── DATA-01: class separation ────────────────────────────────

#[test]
fn metadata_only_grant_reads_metadata_never_content() {
    // DATA-01: a metadata-only caller gets identity/structure and
    // cannot reach structured content, media, or derived groups.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source9(), source10()]),
    );

    let result = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"all": true})), latest()),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(result["completeness"], json!("complete"));
    for item in result["items"].as_array().unwrap() {
        assert!(item["fields"]["metadata"].is_object());
        assert!(item["fields"].get("content").is_none());
        assert!(item["fields"].get("media").is_none());
    }

    for group in ["content", "media", "derived"] {
        let err = rig
            .gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"all": true}, "fields": [group]}),
                    latest(),
                ),
                agent(),
            )
            .expect_err("{group} must deny for a metadata-only grant");
        assert_eq!(deny_reason(&err), "data access outside grant");
    }
}

#[test]
fn structured_content_preserves_source_native_time() {
    // DATA-02: MIDI notes keep their source-native division and ticks
    // verbatim — never requantized to project PPQN.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &[
                DataAccessClass::MetadataRead,
                DataAccessClass::StructuredContentRead,
                DataAccessClass::ExternalDisclosure,
            ],
            &[source10()],
        ),
    );
    let result = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "source"}, "fields": ["content"]}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let item = &result["items"][0];
    assert_eq!(item["completeness"], json!("complete"));
    let content = &item["fields"]["content"];
    assert_eq!(content["domain"], json!("source-native"));
    assert_eq!(content["division"]["ticks_per_quarter"], json!(480));
    let notes = content["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0]["start"], json!(0));
    assert_eq!(notes[0]["len"], json!(240));
    assert_eq!(notes[1]["start"], json!(480));
}

#[test]
fn media_group_is_descriptor_never_bytes() {
    // DATA-02: the media group is a bounded descriptor — asset
    // identity, content digest, verification state — not a byte
    // channel, and only under MediaBytesRead.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &[
                DataAccessClass::MetadataRead,
                DataAccessClass::MediaBytesRead,
                DataAccessClass::ExternalDisclosure,
            ],
            &[source9()],
        ),
    );
    let result = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "source"}, "fields": ["media"]}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let media = &result["items"][0]["fields"]["media"];
    assert_eq!(media["asset"], json!(asset5_tagged()));
    assert_eq!(media["content_digest"], json!("sha256:media9"));
    assert_eq!(media["state"], json!("verified"));
    assert_eq!(media["transfer"], json!("descriptor-only"));
}

fn asset5_tagged() -> String {
    synvelis_project::AssetId::new(5).to_tagged()
}

#[test]
fn derived_group_reports_unavailable_honestly() {
    // DATA-08 completeness: derived data does not exist in the
    // aggregate — the projection reports `unavailable`, never an
    // empty-looking success.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &[
                DataAccessClass::MetadataRead,
                DataAccessClass::DerivedDataRead,
                DataAccessClass::ExternalDisclosure,
            ],
            &[source9()],
        ),
    );
    let result = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "source"}, "fields": ["derived"]}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let item = &result["items"][0];
    assert_eq!(item["completeness"], json!("unavailable"));
    assert_eq!(item["unavailable"], json!(["derived"]));
    assert!(item["fields"].get("derived").is_none());
    assert_eq!(result["completeness"], json!("unavailable"));
}

// ── DATA-05 / SEC-05: scope admission ────────────────────────

#[test]
fn explicit_selector_denies_uncovered_and_absent_identically() {
    // DATA-05: for an explicit id list, coverage is a verdict — an
    // uncovered id and a nonexistent id deny with the same reason.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source9()]),
    );
    for refs in [
        resources(&[source9(), source10()]), // source10 exists, outside scope
        resources(&[source9(), ResourceRef::Source(SourceId::new(999))]), // absent
    ] {
        let err = rig
            .gw
            .handle(&query_req("t1", "g1", selector(refs), latest()), agent())
            .expect_err("uncovered/absent member must deny");
        assert_eq!(deny_reason(&err), "target outside grant scope");
    }
}

#[test]
fn enumeration_emits_only_admitted_members() {
    // Kind/All selectors are filters: uncovered members are dropped,
    // never named.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source9()]),
    );
    let result = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "source"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    let kinds: Vec<&str> = result["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["resource"]["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["source"]);
    assert_eq!(
        result["items"][0]["resource"]["id"],
        json!(SourceId::new(9).to_tagged())
    );
}

#[test]
fn clip_scope_never_implies_source_scope() {
    // SEC-05 / no false sharing: a clip-scoped grant reads the clip —
    // including the source *identity* it references — but the source
    // itself denies.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11()]),
    );
    let result = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "clip"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    let metadata = &result["items"][0]["fields"]["metadata"];
    assert_eq!(metadata["source"], json!(SourceId::new(9).to_tagged()));

    let err = rig
        .gw
        .handle(
            &query_req("t1", "g1", selector(resources(&[source9()])), latest()),
            agent(),
        )
        .expect_err("the clip's source is not thereby covered");
    assert_eq!(deny_reason(&err), "target outside grant scope");
}

#[test]
fn dynamic_scope_filters_by_kind() {
    // A dynamic kind-scope enumerates its members; the project oracle
    // carries no per-entity creation revision, so `include_future`
    // admits them (AHG-008 fail-closed is the alternative).
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant_scope(
            "g1",
            "t1",
            &metadata_classes(),
            ResourceScope::Dynamic(DynamicScope {
                owners: OwnerConstraint::Any,
                kinds: BTreeSet::from([ResourceKind::Source]),
                range: None,
                max_members: None,
                include_future: true,
                move_args: None,
            }),
        ),
    );
    let result = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "source"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    // Clips are not sources — the kind bound excludes them.
    let clips = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "clip"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(clips["items"].as_array().unwrap().len(), 0);
}

// ── DATA-03: revision pinning ────────────────────────────────

#[test]
fn pagination_chain_is_revision_stable_across_project_change() {
    // DATA-03: pages of one chain read one pinned snapshot — a project
    // commit between pages never mixes a second revision in.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &metadata_classes(),
            &[clip11(), clip12(), clip13()],
        ),
    );
    let args = json!({"selector": {"kind": "clip"}, "page": {"limit": 1}});
    let page0 = complete(
        rig.gw
            .handle(&query_req("t1", "g1", args, latest()), agent())
            .unwrap(),
    );
    assert_eq!(page0["revision"], json!(1));
    assert_eq!(page0["page"]["index"], json!(0));
    assert_eq!(page0["items"].as_array().unwrap().len(), 1);
    let cursor = page0["page"]["next_cursor"].as_str().unwrap().to_owned();
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();

    // The live project changes under the chain.
    rig.source
        .commit(project(), ProjectRevision(2), demo_project_v2());

    let page1 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"page": {"cursor": cursor}}),
                    fixed(&json!(snapshot)),
                ),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(page1["revision"], json!(1), "chain stays pinned to rev 1");
    assert_eq!(page1["snapshot"], json!(snapshot));
    assert_eq!(page1["page"]["index"], json!(1));
    // The rev-1 chain's second clip — rev-2's clip13 never joins.
    assert_eq!(page1["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        page1["items"][0]["resource"]["id"],
        json!(ClipId::new(12).to_tagged())
    );
    assert!(page1["page"]["next_cursor"].is_null());

    // A fresh query sees rev 2 — the pin never contaminated it.
    let fresh = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "clip"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(fresh["revision"], json!(2));
    assert_eq!(fresh["items"].as_array().unwrap().len(), 3);
}

#[test]
fn continuation_requires_the_chain_snapshot() {
    // DATA-03: a `latest` continuation would silently re-base — the
    // contract demands fixed-snapshot consistency instead.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &metadata_classes(),
            &[clip11(), clip12(), clip13()],
        ),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "clip"}, "page": {"limit": 1}}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let cursor = page0["page"]["next_cursor"].as_str().unwrap().to_owned();
    let err = rig
        .gw
        .handle(
            &query_req("t1", "g1", json!({"page": {"cursor": cursor}}), latest()),
            agent(),
        )
        .expect_err("latest continuation must reject");
    assert!(matches!(err.code, GatewayError::CursorInvalid { .. }));
}

#[test]
fn evicted_snapshot_reports_expired_never_rebased() {
    // DATA-03/09: an expired pin is `SnapshotExpired`, never a silent
    // Latest read.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source9()]),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "source"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();
    assert!(
        rig.engine
            .evict_snapshot(&SnapshotRef::new(&snapshot).unwrap())
    );

    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                selector(json!({"kind": "source"})),
                fixed(&json!(snapshot)),
            ),
            agent(),
        )
        .expect_err("evicted pin must report expired");
    assert_eq!(err.code, GatewayError::SnapshotExpired);
}

#[test]
fn unservable_revision_reports_expired() {
    // ReadAt to a revision the source cannot serve is expired, never
    // redirected to another revision.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source9()]),
    );
    let err = rig
        .gw
        .handle(
            &query_req("t1", "g1", selector(json!({"kind": "source"})), read_at(99)),
            agent(),
        )
        .expect_err("unservable revision must report expired");
    assert_eq!(err.code, GatewayError::SnapshotExpired);
}

#[test]
fn fixed_snapshot_is_reusable_for_followup_queries() {
    // A reported snapshot ref is a real pin: a later independent query
    // can name it and reads that revision even after commits.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11(), clip12()]),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "clip"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();
    rig.source
        .commit(project(), ProjectRevision(2), demo_project_v2());
    let pinned = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    selector(json!({"kind": "clip"})),
                    fixed(&json!(snapshot)),
                ),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(pinned["revision"], json!(1));
    assert_eq!(pinned["items"].as_array().unwrap().len(), 2);
}

// ── DATA-04/05/09: cursor lifecycle ──────────────────────────

#[test]
fn cursor_expires_after_ttl() {
    // DATA-09: an expiring cursor dies on schedule — no further
    // chunks after expiry.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11(), clip12()]),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "clip"}, "page": {"limit": 1}}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let cursor = page0["page"]["next_cursor"].as_str().unwrap().to_owned();
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();
    rig.clock.advance(60_001);
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"page": {"cursor": cursor}}),
                fixed(&json!(snapshot)),
            ),
            agent(),
        )
        .expect_err("expired cursor must reject");
    assert_eq!(err.code, GatewayError::CursorExpired);
}

#[test]
fn cursor_rotates_and_rejects_replay() {
    // A spent cursor token is dead — replaying it is invalid, the
    // rotated successor is the only live token.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &metadata_classes(),
            &[clip11(), clip12(), clip13()],
        ),
    );
    rig.source
        .commit(project(), ProjectRevision(2), demo_project_v2());
    let page0 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "clip"}, "page": {"limit": 1}}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let cursor = page0["page"]["next_cursor"].as_str().unwrap().to_owned();
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();
    let cont = |cursor: &str| {
        query_req(
            "t1",
            "g1",
            json!({"page": {"cursor": cursor}}),
            fixed(&json!(snapshot)),
        )
    };
    let page1 = complete(rig.gw.handle(&cont(&cursor), agent()).unwrap());
    let next = page1["page"]["next_cursor"].as_str().unwrap();
    assert_ne!(next, cursor, "tokens rotate");

    let err = rig
        .gw
        .handle(&cont(&cursor), agent())
        .expect_err("replayed token must reject");
    assert!(matches!(err.code, GatewayError::CursorInvalid { .. }));
}

#[test]
fn foreign_caller_cursor_is_indistinguishable_from_unknown() {
    // DATA-05: unknown, foreign, and mismatched cursors share one
    // verdict — probing learns nothing about the token.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11(), clip12()]),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "clip"}, "page": {"limit": 1}}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let cursor = page0["page"]["next_cursor"].as_str().unwrap().to_owned();
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();
    let req = |cursor: &str| {
        query_req(
            "t1",
            "g1",
            json!({"page": {"cursor": cursor}}),
            fixed(&json!(snapshot)),
        )
    };
    // A fabricated token and the real token in foreign hands deny
    // with the same variant and reason.
    for (caller, token) in [
        (agent(), "cursor-deadbeef"),
        (other_agent(), cursor.as_str()),
    ] {
        let err = rig
            .gw
            .handle(&req(token), caller)
            .expect_err("foreign/unknown cursor must reject");
        match &err.code {
            GatewayError::CursorInvalid { reason } => {
                assert_eq!(reason, "cursor does not identify a live pagination chain");
            }
            GatewayError::Denied { reason, .. } => {
                // The foreign agent has no grant at all — NO_AUTH
                // denies before the cursor is even examined.
                assert_eq!(reason.as_deref(), Some("no applicable authorization"));
            }
            other => panic!("expected CursorInvalid/Denied, got {other:?}"),
        }
    }
    // The fabricated token for the *owning* caller is the same
    // verdict as a foreign one — token existence is never confirmed.
    let err = rig
        .gw
        .handle(&req("cursor-deadbeef"), agent())
        .expect_err("fabricated cursor must reject");
    assert!(matches!(err.code, GatewayError::CursorInvalid { .. }));
}

#[test]
fn revoked_grant_denies_the_next_page() {
    // DATA-04: revocation is observed at the page after it lands —
    // an earlier permit never masks it.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11(), clip12()]),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "clip"}, "page": {"limit": 1}}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let cursor = page0["page"]["next_cursor"].as_str().unwrap().to_owned();
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();
    rig.store.revoke_grant(&grant_ref("g1"));
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"page": {"cursor": cursor}}),
                fixed(&json!(snapshot)),
            ),
            agent(),
        )
        .expect_err("revoked grant must deny the next page");
    assert_eq!(deny_reason(&err), "grant revoked");
}

#[test]
fn expired_grant_denies_the_next_page() {
    // DATA-09: grant expiry is observed on the fresh policy read —
    // the cursor never extends authority.
    let mut rig = rig();
    let mut grant = ctx_grant("g1", "t1", &metadata_classes(), &[clip11(), clip12()]);
    grant.validity.expires_at = Some(PolicyInstant::new(2_000));
    issue_agent(&rig.store, "t1", grant);

    let page0 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "clip"}, "page": {"limit": 1}}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let cursor = page0["page"]["next_cursor"].as_str().unwrap().to_owned();
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();
    rig.clock.set(PolicyInstant::new(2_001));
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"page": {"cursor": cursor}}),
                fixed(&json!(snapshot)),
            ),
            agent(),
        )
        .expect_err("expired grant must deny the next page");
    assert_eq!(deny_reason(&err), "grant expired");
}

#[test]
fn task_grant_refs_alone_grant_nothing() {
    // DATA-05: a task/grant pair the store does not know denies with
    // the uniform pre-binding reason — ids are never authority.
    let mut rig = rig();
    let err = rig
        .gw
        .handle(
            &query_req(
                "task-nope",
                "grant-nope",
                selector(json!({"kind": "source"})),
                latest(),
            ),
            agent(),
        )
        .expect_err("unissued authority must deny");
    assert_eq!(deny_reason(&err), "no applicable authorization");
}

// ── DATA-08: budgets ─────────────────────────────────────────

#[test]
fn page_limit_is_bounded() {
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[project_ref()]),
    );
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"selector": {"project": true}, "page": {"limit": 257}}),
                latest(),
            ),
            agent(),
        )
        .expect_err("over-limit page must reject");
    assert_eq!(
        err.code,
        GatewayError::LimitExceeded {
            limit: "page items".to_owned(),
            max: 256
        }
    );
}

#[test]
fn range_window_is_bounded() {
    let mut rig = rig_with(
        ContextBudget {
            max_range_len: 1_000,
            ..ContextBudget::default()
        },
        BTreeMap::new(),
    );
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11()]),
    );
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({
                    "selector": {"kind": "clip"},
                    "range": {"domain": "project-musical", "start": 0, "len": 2000},
                }),
                latest(),
            ),
            agent(),
        )
        .expect_err("over-window range must reject");
    assert_eq!(
        err.code,
        GatewayError::LimitExceeded {
            limit: "range window".to_owned(),
            max: 1_000
        }
    );
}

#[test]
fn selector_id_count_is_bounded() {
    let mut rig = rig_with(
        ContextBudget {
            max_selector_ids: 2,
            ..ContextBudget::default()
        },
        BTreeMap::new(),
    );
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source9()]),
    );
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                selector(resources(&[source9(), source10(), clip11()])),
                latest(),
            ),
            agent(),
        )
        .expect_err("over-count selector must reject");
    assert_eq!(
        err.code,
        GatewayError::LimitExceeded {
            limit: "selector ids".to_owned(),
            max: 2
        }
    );
}

#[test]
fn total_items_are_bounded() {
    let mut rig = rig_with(
        ContextBudget {
            max_total_items: 1,
            ..ContextBudget::default()
        },
        BTreeMap::new(),
    );
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source9(), source10()]),
    );
    let err = rig
        .gw
        .handle(
            &query_req("t1", "g1", selector(json!({"kind": "source"})), latest()),
            agent(),
        )
        .expect_err("over-total enumeration must reject");
    assert_eq!(
        err.code,
        GatewayError::LimitExceeded {
            limit: "total items".to_owned(),
            max: 1
        }
    );
}

#[test]
fn page_count_is_bounded() {
    let mut rig = rig_with(
        ContextBudget {
            max_pages: 2,
            ..ContextBudget::default()
        },
        BTreeMap::new(),
    );
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &metadata_classes(),
            &[clip11(), clip12(), clip13()],
        ),
    );
    rig.source
        .commit(project(), ProjectRevision(2), demo_project_v2());
    let page0 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "clip"}, "page": {"limit": 1}}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();
    let mut cursor = page0["page"]["next_cursor"].as_str().unwrap().to_owned();
    // Page 1 of a 3-item chain is the second page — allowed.
    let page1 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"page": {"cursor": cursor}}),
                    fixed(&json!(snapshot)),
                ),
                agent(),
            )
            .unwrap(),
    );
    cursor = page1["page"]["next_cursor"].as_str().unwrap().to_owned();
    // Page 2 exceeds max_pages=2.
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"page": {"cursor": cursor}}),
                fixed(&json!(snapshot)),
            ),
            agent(),
        )
        .expect_err("over-count pages must reject");
    assert_eq!(
        err.code,
        GatewayError::LimitExceeded {
            limit: "pages".to_owned(),
            max: 2
        }
    );
}

// ── DATA-10 / SEC-07: selection ──────────────────────────────

#[test]
fn selection_needs_named_view_and_class() {
    // The selection group requires an explicit view name AND its own
    // data-access class — neither defaults in.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &[
                DataAccessClass::MetadataRead,
                DataAccessClass::WorkspaceSelectionRead,
                DataAccessClass::ExternalDisclosure,
            ],
            &[source9()],
        ),
    );
    // selection field without a view → typed violation.
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"selector": {"kind": "source"}, "fields": ["selection"]}),
                latest(),
            ),
            agent(),
        )
        .expect_err("viewless selection must reject");
    assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));

    // A view name without the class → disclosure deny.
    issue_agent(
        &rig.store,
        "t2",
        ctx_grant("g2", "t2", &metadata_classes(), &[source9()]),
    );
    let err = rig
        .gw
        .handle(
            &query_req(
                "t2",
                "g2",
                json!({
                    "selector": {"kind": "source"},
                    "fields": ["selection"],
                    "selection_view": "view-mixer",
                }),
                latest(),
            ),
            agent(),
        )
        .expect_err("classless selection must deny");
    assert_eq!(deny_reason(&err), "data access outside grant");
}

#[test]
fn selection_emits_only_admitted_members() {
    // The view is disclosed, but its members still pass scope
    // admission — a selection never extends the grant.
    let mut rig = rig_with(
        ContextBudget::default(),
        BTreeMap::from([(
            "view-mixer".to_owned(),
            SelectionFacts::Selected(vec![source9(), clip11()]),
        )]),
    );
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &[
                DataAccessClass::MetadataRead,
                DataAccessClass::WorkspaceSelectionRead,
                DataAccessClass::ExternalDisclosure,
            ],
            &[source9()],
        ),
    );
    let result = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({
                        "selector": {"kind": "source"},
                        "fields": ["selection"],
                        "selection_view": "view-mixer",
                    }),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let sel = &result["selection"];
    assert_eq!(sel["resources"].as_array().unwrap().len(), 1);
    assert_eq!(
        sel["resources"][0]["id"],
        json!(SourceId::new(9).to_tagged())
    );
    assert_eq!(sel["completeness"], json!("unavailable"));
}

#[test]
fn selection_unknown_view_reports_unknown() {
    // An unresolvable view reports `unknown` — never an
    // empty-looking success.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &[
                DataAccessClass::MetadataRead,
                DataAccessClass::WorkspaceSelectionRead,
                DataAccessClass::ExternalDisclosure,
            ],
            &[source9()],
        ),
    );
    let result = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({
                        "selector": {"kind": "source"},
                        "fields": ["selection"],
                        "selection_view": "view-nope",
                    }),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(result["selection"]["completeness"], json!("unknown"));
    assert_eq!(result["completeness"], json!("unknown"));
}

#[test]
fn selection_pin_absent_member_reports_unavailable() {
    // A selected member that does not resolve in the pin is dropped
    // without existence detail — but the drop is reported, not
    // silently folded into a `complete` verdict.
    let mut rig = rig_with(
        ContextBudget::default(),
        BTreeMap::from([(
            "view-mixer".to_owned(),
            SelectionFacts::Selected(vec![source9(), ResourceRef::Clip(ClipId::new(999))]),
        )]),
    );
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &[
                DataAccessClass::MetadataRead,
                DataAccessClass::WorkspaceSelectionRead,
                DataAccessClass::ExternalDisclosure,
            ],
            &[source9(), ResourceRef::Clip(ClipId::new(999))],
        ),
    );
    let result = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({
                        "selector": {"kind": "source"},
                        "fields": ["selection"],
                        "selection_view": "view-mixer",
                    }),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let sel = &result["selection"];
    assert_eq!(sel["resources"].as_array().unwrap().len(), 1);
    assert_eq!(sel["completeness"], json!("unavailable"));
}

// ── Completeness ─────────────────────────────────────────────

#[test]
fn explicit_selector_denies_pin_absent_blind() {
    // Scope facts are pinned: an explicit member absent from the
    // pinned revision is dropped at admission like an uncovered or
    // never-existed one — one blind `outside scope` verdict, so the
    // answer never confirms coverage or existence.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &metadata_classes(),
            &[note20(), note21(), ResourceRef::Note(NoteId::new(22))],
        ),
    );
    // Rev 2 creates note22 — it is covered by the grant but absent
    // from the rev-1 pin.
    let mut v2 = demo_project();
    v2.midi_sources
        .get_mut(&SourceId::new(10))
        .unwrap()
        .notes
        .insert(
            NoteId::new(22),
            MidiNote {
                id: NoteId::new(22),
                key: 67,
                velocity: 90,
                start: MidiTick::new(720).unwrap(),
                len: MidiTick::new(120).unwrap(),
            },
        );
    rig.source.commit(project(), ProjectRevision(2), v2);

    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                selector(resources(&[note20(), ResourceRef::Note(NoteId::new(22))])),
                read_at(1),
            ),
            agent(),
        )
        .expect_err("pin-absent member denies like an uncovered one");
    assert_eq!(deny_reason(&err), "target outside grant scope");

    // At latest, note22 resolves — the same selector succeeds there.
    let result = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    selector(resources(&[note20(), ResourceRef::Note(NoteId::new(22))])),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    assert_eq!(result["completeness"], json!("complete"));
}

#[test]
fn deleted_member_is_served_from_its_pin() {
    // The other direction of pinned facts: a covered member deleted
    // after the pin still projects from `read_at` — the pin, not the
    // live view, decides what can be served.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11()]),
    );
    // Rev 2 deletes clip11.
    let mut v2 = demo_project();
    v2.lanes
        .get_mut(&LaneId::new(4))
        .unwrap()
        .clip_ids
        .retain(|c| *c != ClipId::new(11));
    v2.clips.remove(&ClipId::new(11));
    rig.source.commit(project(), ProjectRevision(2), v2);

    let result = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(resources(&[clip11()])), read_at(1)),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(result["items"][0]["completeness"], json!("complete"));

    // At latest, the same covered id is absent — same blind verdict.
    let err = rig
        .gw
        .handle(
            &query_req("t1", "g1", selector(resources(&[clip11()])), latest()),
            agent(),
        )
        .expect_err("absent at latest denies like an uncovered one");
    assert_eq!(deny_reason(&err), "target outside grant scope");
}

#[test]
fn human_selector_reports_unknown_for_absent() {
    // Manual authority admits every candidate — an absent explicit
    // member projects as `unknown`, the same verdict as never-existed.
    let mut rig = rig();
    let ghost = ResourceRef::Clip(ClipId::new(999));
    let result = complete(
        rig.gw
            .handle(
                &json!({
                    "request_id": "req-h2",
                    "capability": {"id": "synvelis.context.query", "version": {"major": 1, "minor": 0}},
                    "schema": {"name": "synvelis.context.query.input", "version": {"major": 1, "minor": 0}},
                    "project": project().to_tagged(),
                    "args": {"selector": {"resources": [tagged(&clip11()), tagged(&ghost)]}},
                    "consistency": {"kind": "latest"},
                }),
                human(),
            )
            .unwrap(),
    );
    let items = result["items"].as_array().unwrap();
    assert_eq!(items[0]["completeness"], json!("complete"));
    assert_eq!(items[1]["completeness"], json!("unknown"));
    assert_eq!(result["completeness"], json!("unknown"));
}

#[test]
fn human_caller_reads_without_grant() {
    // Manual authority: the trusted operator's query needs no task or
    // grant (AHG-007) — and stays inside the internal boundary.
    let mut rig = rig();
    let result = complete(
        rig.gw
            .handle(
                &json!({
                    "request_id": "req-h1",
                    "capability": {"id": "synvelis.context.query", "version": {"major": 1, "minor": 0}},
                    "schema": {"name": "synvelis.context.query.input", "version": {"major": 1, "minor": 0}},
                    "project": project().to_tagged(),
                    "args": {"selector": {"kind": "clip"}, "fields": ["content", "media"]},
                    "consistency": {"kind": "latest"},
                }),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    // Internal emission — no ExternalDisclosure class is implied.
    let classes: Vec<&str> = result["data_access"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert!(!classes.contains(&"external-disclosure"));
}

// ── GT-004: headless consistency proof ───────────────────────

#[test]
fn query_snapshot_prepared_operation_consistency() {
    // GT-004: Query → Snapshot Revision → Prepared Operation reference
    // agree — the projection's snapshot+revision is exactly what the
    // internal pin seam resolves for a prepared operation.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11(), clip12()]),
    );
    let projection = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "clip"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(projection["revision"], json!(1));
    let snapshot = projection["snapshot"].as_str().unwrap().to_owned();

    // The prepared-operation reference (GW-04's seam): pin the same
    // snapshot the query reported — same revision, same live state.
    let req = rig
        .gw
        .bind(
            Gateway::parse_wire(&query_req(
                "t1",
                "g1",
                selector(json!({"kind": "clip"})),
                fixed(&json!(snapshot)),
            ))
            .unwrap(),
            agent(),
        )
        .unwrap();
    let pin = rig.engine.pin_for(&req).unwrap();
    assert_eq!(pin.reference.as_str(), snapshot);
    assert_eq!(pin.revision, ProjectRevision(1));
    // The pinned state is the revision-1 aggregate — identity, not a
    // re-read of latest.
    assert_eq!(pin.state.clips.len(), 2);
}

#[test]
fn foreign_project_snapshot_is_existence_blind() {
    // A snapshot ref minted for project 7 named in a project-8
    // request is expired — foreign and dead pins share the verdict.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source9()]),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "source"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();

    // A second project with its own authority.
    rig.store.issue_task(AuthorizedTask {
        task: task_ref("t9"),
        principal: agent_principal(),
        project: ProjectId::new(8),
        activation: Activation::UserIssued,
        validity: Validity::default(),
    });
    let mut g9 = ctx_grant("g9", "t9", &metadata_classes(), &[]);
    g9.project = ProjectId::new(8);
    rig.store.issue_grant(g9);
    rig.source
        .commit(ProjectId::new(8), ProjectRevision(1), demo_project());

    let mut req = query_req(
        "t9",
        "g9",
        selector(json!({"kind": "source"})),
        fixed(&json!(snapshot)),
    );
    req["project"] = json!(ProjectId::new(8).to_tagged());
    let err = rig
        .gw
        .handle(&req, agent())
        .expect_err("foreign snapshot must reject");
    assert_eq!(err.code, GatewayError::SnapshotExpired);
}

// ── Error hygiene ────────────────────────────────────────────

#[test]
fn external_envelope_strips_diagnostics() {
    // Public errors carry the safe message only — the internal
    // diagnostic (e.g. which binding detail failed) never leaves.
    let mut rig = rig();
    let err = rig
        .gw
        .handle(
            &query_req(
                "task-nope",
                "grant-nope",
                selector(json!({"kind": "source"})),
                latest(),
            ),
            agent(),
        )
        .expect_err("unauthorized must deny");
    let external = err.to_external();
    let text = serde_json::to_string(&external).unwrap();
    assert!(text.contains("no applicable authorization") || text.contains("denied"));
    assert!(
        !text.contains("diagnostic"),
        "external envelope carries no diagnostics"
    );

    // An invalid cursor exposes only the uniform reason.
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source9()]),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "source"}, "page": {"limit": 1}}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"page": {"cursor": "cursor-bogus"}}),
                fixed(&json!(snapshot)),
            ),
            agent(),
        )
        .expect_err("bogus cursor must reject");
    let external = err.to_external();
    let text = serde_json::to_string(&external).unwrap();
    assert!(!text.contains("cursor-bogus") || text.contains("pagination chain"));
}

#[test]
fn wire_args_are_schema_checked() {
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source9()]),
    );
    // Unknown field group → schema violation.
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"selector": {"kind": "source"}, "fields": ["pcm"]}),
                latest(),
            ),
            agent(),
        )
        .expect_err("unknown group must reject");
    assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));
    // Two selector forms → typed violation.
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"selector": {"kind": "source", "all": true}}),
                latest(),
            ),
            agent(),
        )
        .expect_err("ambiguous selector must reject");
    assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));
    // Continuation args → typed violation.
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"page": {"cursor": "cursor-x"}, "fields": ["content"]}),
                latest(),
            ),
            agent(),
        )
        .expect_err("continuation args must reject");
    assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));
    // limit + cursor → typed violation.
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"page": {"cursor": "cursor-x", "limit": 1}}),
                latest(),
            ),
            agent(),
        )
        .expect_err("limit+cursor must reject");
    assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));
}

// ── Range filter (post-admission) ────────────────────────────

#[test]
fn range_cannot_probe_out_of_scope_resources() {
    // The range window filters the *admitted* set — pre-filtering
    // would make "deny" vs "empty page" disclose whether an
    // out-of-scope resource exists and where it sits.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11()]),
    );
    // clip12 exists but is uncovered. Both windows must deny — the
    // range never softens the explicit-selector verdict.
    for window in [0_i64, 500] {
        let err = rig
            .gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({
                        "selector": {"resources": [tagged(&clip12())]},
                        "range": {"domain": "project-musical", "start": window, "len": 200},
                    }),
                    latest(),
                ),
                agent(),
            )
            .expect_err("uncovered member denies regardless of range");
        assert_eq!(deny_reason(&err), "target outside grant scope");
    }
    // A covered member outside the window yields an empty page —
    // covered membership was proven before the filter ran.
    let result = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({
                        "selector": {"resources": [tagged(&clip11())]},
                        "range": {"domain": "project-musical", "start": 5000, "len": 100},
                    }),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(result["items"].as_array().unwrap().len(), 0);
    // And inside the window it emits normally.
    let result = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({
                        "selector": {"resources": [tagged(&clip11())]},
                        "range": {"domain": "project-musical", "start": 0, "len": 200},
                    }),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
}

// ── Pin / cursor retention ───────────────────────────────────

#[test]
fn pin_retention_evicts_oldest() {
    // max_pinned_snapshots bounds retention — the evicted ref reports
    // `SnapshotExpired`, never a re-base.
    let mut rig = rig_with(
        ContextBudget {
            max_pinned_snapshots: 1,
            ..ContextBudget::default()
        },
        BTreeMap::new(),
    );
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &metadata_classes(),
            &[clip11(), clip12(), clip13()],
        ),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "clip"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    let ref1 = page0["snapshot"].as_str().unwrap().to_owned();
    assert_eq!(rig.engine.pinned_count(), 1);

    // A second pin (rev 2) evicts rev 1's.
    rig.source
        .commit(project(), ProjectRevision(2), demo_project_v2());
    let page1 = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "clip"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(page1["revision"], json!(2));
    assert_eq!(rig.engine.pinned_count(), 1);

    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                selector(json!({"kind": "clip"})),
                fixed(&json!(ref1)),
            ),
            agent(),
        )
        .expect_err("evicted pin must report expired");
    assert_eq!(err.code, GatewayError::SnapshotExpired);
}

#[test]
fn cursor_retention_evicts_oldest() {
    // max_cursors bounds live chains — a second chain mints the
    // first one's token dead.
    let mut rig = rig_with(
        ContextBudget {
            max_cursors: 1,
            ..ContextBudget::default()
        },
        BTreeMap::new(),
    );
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11(), clip12()]),
    );
    let mut first = |limit: u64| {
        complete(
            rig.gw
                .handle(
                    &query_req(
                        "t1",
                        "g1",
                        json!({"selector": {"kind": "clip"}, "page": {"limit": limit}}),
                        latest(),
                    ),
                    agent(),
                )
                .unwrap(),
        )
    };
    let c1 = first(1)["page"]["next_cursor"].as_str().unwrap().to_owned();
    let snapshot = {
        let page = first(1);
        let c2 = page["page"]["next_cursor"].as_str().unwrap().to_owned();
        assert_ne!(c2, c1);
        assert_eq!(rig.engine.cursor_count(), 1);
        page["snapshot"].as_str().unwrap().to_owned()
    };
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"page": {"cursor": c1}}),
                fixed(&json!(snapshot)),
            ),
            agent(),
        )
        .expect_err("evicted cursor must reject");
    assert!(matches!(err.code, GatewayError::CursorInvalid { .. }));
}

#[test]
fn continuation_after_pin_eviction_reports_expired() {
    // A live cursor whose pin was evicted reports `SnapshotExpired` —
    // the chain dies with its snapshot.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[clip11(), clip12()]),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "clip"}, "page": {"limit": 1}}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let cursor = page0["page"]["next_cursor"].as_str().unwrap().to_owned();
    let snapshot = page0["snapshot"].as_str().unwrap().to_owned();
    assert!(
        rig.engine
            .evict_snapshot(&SnapshotRef::new(&snapshot).unwrap())
    );
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"page": {"cursor": cursor}}),
                fixed(&json!(snapshot)),
            ),
            agent(),
        )
        .expect_err("dead pin must report expired");
    assert_eq!(err.code, GatewayError::SnapshotExpired);
}

#[test]
fn continuation_with_mismatched_snapshot_is_cursor_invalid() {
    // A valid cursor presented against a *different* valid pin is
    // indistinguishable from an unknown token.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant(
            "g1",
            "t1",
            &metadata_classes(),
            &[clip11(), clip12(), clip13()],
        ),
    );
    let page0 = complete(
        rig.gw
            .handle(
                &query_req(
                    "t1",
                    "g1",
                    json!({"selector": {"kind": "clip"}, "page": {"limit": 1}}),
                    latest(),
                ),
                agent(),
            )
            .unwrap(),
    );
    let cursor = page0["page"]["next_cursor"].as_str().unwrap().to_owned();
    rig.source
        .commit(project(), ProjectRevision(2), demo_project_v2());
    let other = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "clip"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    let other_snapshot = other["snapshot"].as_str().unwrap().to_owned();
    let err = rig
        .gw
        .handle(
            &query_req(
                "t1",
                "g1",
                json!({"page": {"cursor": cursor}}),
                fixed(&json!(other_snapshot)),
            ),
            agent(),
        )
        .expect_err("mismatched snapshot must reject");
    assert!(matches!(err.code, GatewayError::CursorInvalid { .. }));
}

// ── Selector literals ────────────────────────────────────────

#[test]
fn selector_literals_are_strict() {
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[project_ref()]),
    );
    for selector in [
        json!({"project": false}),
        json!({"all": false}),
        json!({"resources": []}),
    ] {
        let err = rig
            .gw
            .handle(
                &query_req("t1", "g1", json!({"selector": selector}), latest()),
                agent(),
            )
            .expect_err("degenerate selector must reject");
        assert!(matches!(err.code, GatewayError::SchemaViolation { .. }),);
    }
}

// ── SEC-05 reverse direction ─────────────────────────────────

#[test]
fn source_scope_never_implies_clip_scope() {
    // The reverse of clip→source: a source-scoped grant reads the
    // source but not the clips that reference it.
    let mut rig = rig();
    issue_agent(
        &rig.store,
        "t1",
        ctx_grant("g1", "t1", &metadata_classes(), &[source10()]),
    );
    let result = complete(
        rig.gw
            .handle(
                &query_req("t1", "g1", selector(json!({"kind": "clip"})), latest()),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(result["items"].as_array().unwrap().len(), 0);
    let err = rig
        .gw
        .handle(
            &query_req("t1", "g1", selector(resources(&[clip12()])), latest()),
            agent(),
        )
        .expect_err("clip referencing the source is not covered");
    assert_eq!(deny_reason(&err), "target outside grant scope");
}
