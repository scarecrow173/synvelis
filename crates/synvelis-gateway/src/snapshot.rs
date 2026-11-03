//! Pinned-snapshot seam and project-state projection (GW-03).
//!
//! The context engine reads project state through [`SnapshotSource`] —
//! a host-supplied seam that pins the *existing* `synvelis-project`
//! aggregate at a committed revision. The gateway never mutates it and
//! never keeps a second canonical store: a [`PinnedSnapshot`] is an
//! immutable read view (`Arc<Project>`), and the controller remains
//! the sole writer and revision owner.
//!
//! [`MemorySnapshotSource`] is the deterministic headless
//! implementation — snapshots are seeded at explicit revisions so a
//! fixture can prove that a pagination chain never mixes revisions
//! (DATA-03) and that an evicted pin never silently re-bases onto
//! `Latest`.
//!
//! [`ProjectOracle`] adapts a pinned `Project` to the GW-02
//! [`ResourceOracle`] seam — scope facts derived from the same
//! aggregate the projection reads, so a headless fixture's authority
//! and its data agree on the revision.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use serde_json::{Map, Value, json};
use synvelis_project::{
    Asset, Clip, Comp, ControlScope, Lane, MidiSource, Note, ParameterTarget, PlaybackMapping,
    Project, ProjectId, ProjectRange, ProjectRevision, Source, SourceState, TakeGroup, Track,
    ValueKind,
};

use crate::context::{Completeness, FieldGroup, FieldSelection, ProjectedItem};
use crate::grant::{ResourceAttributes, ResourceFacts, ResourceOracle};
use crate::request::{ResourceKind, ResourceRef, TimeDomain, TypedRange};

/// An immutable pinned read view of one project at one revision.
///
/// `state` is shared — a pin never copies the aggregate again after
/// the [`SnapshotSource`] produced it.
#[derive(Clone, Debug)]
pub struct PinnedSnapshot {
    /// The pinned-snapshot reference the engine minted for this pin.
    pub reference: crate::id::SnapshotRef,
    /// The project this pin serves.
    pub project: ProjectId,
    /// The pinned revision.
    pub revision: ProjectRevision,
    /// The immutable project view.
    pub state: Arc<Project>,
}

/// Host seam supplying committed project state at a revision.
///
/// The gateway does not own project state — the host pins what its
/// single writer committed. `pin` returns `None` when the revision
/// cannot be served; the engine reports that as an expired snapshot
/// and never falls back to a different revision.
pub trait SnapshotSource: Send + Sync {
    /// The latest committed revision for `project` — `None` when the
    /// project is unknown to the source.
    fn latest_revision(&self, project: ProjectId) -> Option<ProjectRevision>;
    /// Pins the committed state at `revision` — `None` when the
    /// revision is not servable (never a substitute revision).
    fn pin(&self, project: ProjectId, revision: ProjectRevision) -> Option<Arc<Project>>;
}

/// Deterministic headless [`SnapshotSource`] — committed states are
/// seeded explicitly at their revisions.
#[derive(Debug, Default)]
pub struct MemorySnapshotSource {
    latest: RwLock<BTreeMap<ProjectId, ProjectRevision>>,
    states: RwLock<BTreeMap<(ProjectId, ProjectRevision), Arc<Project>>>,
}

impl MemorySnapshotSource {
    /// An empty source.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Seeds `state` as the committed revision `revision` of `project`
    /// and makes it the source's latest.
    ///
    /// # Panics
    /// Panics if the store's lock is poisoned.
    pub fn commit(&self, project: ProjectId, revision: ProjectRevision, state: Project) {
        self.states
            .write()
            .expect("snapshot source poisoned")
            .insert((project, revision), Arc::new(state));
        self.latest
            .write()
            .expect("snapshot source poisoned")
            .insert(project, revision);
    }

    /// Removes a previously seeded revision — simulates retention
    /// expiry so callers can verify no silent `Latest` fallback.
    ///
    /// # Panics
    /// Panics if the store's lock is poisoned.
    pub fn evict(&self, project: ProjectId, revision: ProjectRevision) {
        self.states
            .write()
            .expect("snapshot source poisoned")
            .remove(&(project, revision));
    }
}

impl SnapshotSource for MemorySnapshotSource {
    fn latest_revision(&self, project: ProjectId) -> Option<ProjectRevision> {
        self.latest
            .read()
            .expect("snapshot source poisoned")
            .get(&project)
            .copied()
    }

    fn pin(&self, project: ProjectId, revision: ProjectRevision) -> Option<Arc<Project>> {
        self.states
            .read()
            .expect("snapshot source poisoned")
            .get(&(project, revision))
            .cloned()
    }
}

/// A [`ResourceOracle`] that derives facts from a fixed `Project` view.
///
/// `created_revision` is always `None` — the aggregate does not record
/// per-entity creation revisions, so a dynamic scope's `include_future`
/// evaluates fail-closed rather than guessing (AHG-008).
#[derive(Clone, Debug)]
pub struct ProjectOracle {
    state: Arc<Project>,
}

impl ProjectOracle {
    /// An oracle over `state`.
    #[must_use]
    pub fn new(state: Arc<Project>) -> Self {
        Self { state }
    }

    /// An oracle over an owned project.
    #[must_use]
    pub fn of(state: Project) -> Self {
        Self::new(Arc::new(state))
    }
}

impl ResourceOracle for ProjectOracle {
    fn facts(&self, project: ProjectId, resource: &ResourceRef) -> ResourceFacts {
        if project != self.state.project_id {
            return ResourceFacts::Unresolved;
        }
        match attributes(&self.state, resource) {
            Some(attrs) => ResourceFacts::Known(attrs),
            None => ResourceFacts::Unresolved,
        }
    }
}

/// Enumerates every resource of `kind` present in `project`, in a
/// stable deterministic order (the `BTreeMap` id order).
///
/// `Source` enumerates the shared `SourceId` space — media and MIDI
/// sources merged. `Note` enumerates clip-local notes and MIDI source
/// notes merged. The project itself is not enumerable through a kind.
#[must_use]
pub fn enumerate_kind(project: &Project, kind: ResourceKind) -> Vec<ResourceRef> {
    match kind {
        ResourceKind::Project => vec![ResourceRef::Project(project.project_id)],
        ResourceKind::Track => project
            .tracks
            .keys()
            .map(|id| ResourceRef::Track(*id))
            .collect(),
        ResourceKind::Lane => project
            .lanes
            .keys()
            .map(|id| ResourceRef::Lane(*id))
            .collect(),
        ResourceKind::Clip => project
            .clips
            .keys()
            .map(|id| ResourceRef::Clip(*id))
            .collect(),
        ResourceKind::Source => project
            .sources
            .keys()
            .chain(project.midi_sources.keys())
            .map(|id| ResourceRef::Source(*id))
            .collect(),
        ResourceKind::Asset => project
            .assets
            .keys()
            .map(|id| ResourceRef::Asset(*id))
            .collect(),
        ResourceKind::Note => project
            .notes
            .keys()
            .chain(project.midi_sources.values().flat_map(|s| s.notes.keys()))
            .map(|id| ResourceRef::Note(*id))
            .collect(),
        ResourceKind::TakeGroup => project
            .comp
            .take_groups
            .keys()
            .map(|id| ResourceRef::TakeGroup(*id))
            .collect(),
        ResourceKind::Comp => project
            .comp
            .comps
            .keys()
            .map(|id| ResourceRef::Comp(*id))
            .collect(),
        ResourceKind::ParameterTarget => project
            .control
            .targets
            .keys()
            .map(|id| ResourceRef::ParameterTarget(*id))
            .collect(),
        ResourceKind::AutomationLane => project
            .control
            .lanes
            .keys()
            .map(|id| ResourceRef::AutomationLane(*id))
            .collect(),
    }
}

/// The kinds [`enumerate_kind`] walks for a whole-project selector,
/// in their canonical emission order.
pub const ENUMERATED_KINDS: [ResourceKind; 10] = [
    ResourceKind::Track,
    ResourceKind::Lane,
    ResourceKind::Clip,
    ResourceKind::Source,
    ResourceKind::Asset,
    ResourceKind::Note,
    ResourceKind::TakeGroup,
    ResourceKind::Comp,
    ResourceKind::ParameterTarget,
    ResourceKind::AutomationLane,
];

/// The scope fact set for `resource` inside `project` — `None` when
/// the resource does not resolve in this aggregate.
///
/// `related` names sharing relationships the model makes explicit —
/// a clip relates to its `SourceId` (shared media/MIDI semantics).
/// `created_revision` is `None` everywhere (see [`ProjectOracle`]).
#[must_use]
#[allow(clippy::too_many_lines)] // one arm per resource kind — splitting adds indirection
pub fn attributes(project: &Project, resource: &ResourceRef) -> Option<ResourceAttributes> {
    let attrs = |kind, owner, range, related| ResourceAttributes {
        kind,
        owner,
        range,
        created_revision: None,
        related,
    };
    Some(match resource {
        ResourceRef::Project(id) if *id == project.project_id => {
            attrs(ResourceKind::Project, None, None, vec![])
        }
        ResourceRef::Track(id) => {
            let track = project.tracks.get(id)?;
            attrs(
                ResourceKind::Track,
                Some(ResourceRef::Project(project.project_id)),
                None,
                track
                    .lane_ids
                    .iter()
                    .map(|l| ResourceRef::Lane(*l))
                    .collect(),
            )
        }
        ResourceRef::Lane(id) => {
            let lane = project.lanes.get(id)?;
            attrs(
                ResourceKind::Lane,
                Some(ResourceRef::Track(lane.track_id)),
                None,
                lane.clip_ids
                    .iter()
                    .map(|c| ResourceRef::Clip(*c))
                    .collect(),
            )
        }
        ResourceRef::Clip(id) => {
            let clip = project.clips.get(id)?;
            attrs(
                ResourceKind::Clip,
                Some(ResourceRef::Lane(clip.lane_id)),
                clip_window(clip),
                vec![ResourceRef::Source(clip.source_id)],
            )
        }
        ResourceRef::Source(id) if project.sources.contains_key(id) => {
            let source = &project.sources[id];
            attrs(
                ResourceKind::Source,
                Some(ResourceRef::Project(project.project_id)),
                None,
                vec![ResourceRef::Asset(source.asset_id)],
            )
        }
        ResourceRef::Source(id) if project.midi_sources.contains_key(id) => attrs(
            ResourceKind::Source,
            Some(ResourceRef::Project(project.project_id)),
            None,
            vec![],
        ),
        ResourceRef::Asset(id) if project.assets.contains_key(id) => attrs(
            ResourceKind::Asset,
            Some(ResourceRef::Project(project.project_id)),
            None,
            vec![],
        ),
        ResourceRef::Note(id) => {
            if let Some(note) = project.notes.get(id) {
                attrs(
                    ResourceKind::Note,
                    Some(ResourceRef::Clip(note.clip)),
                    None,
                    vec![],
                )
            } else {
                let (source_id, note) = project
                    .midi_sources
                    .iter()
                    .find_map(|(sid, s)| s.notes.get(id).map(|n| (*sid, n)))?;
                attrs(
                    ResourceKind::Note,
                    Some(ResourceRef::Source(source_id)),
                    Some(TypedRange {
                        domain: TimeDomain::SourceNative,
                        start: note.start.value(),
                        len: note.len.value(),
                    }),
                    vec![],
                )
            }
        }
        ResourceRef::TakeGroup(id) => {
            let group = project.comp.take_groups.get(id)?;
            attrs(
                ResourceKind::TakeGroup,
                Some(ResourceRef::Lane(group.owner_lane_id)),
                project_range_window(&group.target_range),
                vec![],
            )
        }
        ResourceRef::Comp(id) => {
            let comp = project.comp.comps.get(id)?;
            attrs(
                ResourceKind::Comp,
                Some(ResourceRef::TakeGroup(comp.take_group_id)),
                None,
                vec![],
            )
        }
        ResourceRef::ParameterTarget(id) => {
            let target = project.control.targets.get(id)?;
            attrs(
                ResourceKind::ParameterTarget,
                Some(control_scope_ref(&target.owner, project.project_id)),
                None,
                vec![],
            )
        }
        ResourceRef::AutomationLane(id) => {
            let lane = project.control.lanes.get(id)?;
            attrs(
                ResourceKind::AutomationLane,
                Some(ResourceRef::ParameterTarget(lane.target)),
                None,
                vec![],
            )
        }
        // Foreign/absent ids: `Project` guarded on equality above,
        // `Source`/`Asset` on membership.
        ResourceRef::Project(_) | ResourceRef::Source(_) | ResourceRef::Asset(_) => return None,
    })
}

/// A [`ControlScope`] as the scope-fact owner reference.
fn control_scope_ref(scope: &ControlScope, project: ProjectId) -> ResourceRef {
    match scope {
        ControlScope::Track(id) => ResourceRef::Track(*id),
        ControlScope::Clip(id) => ResourceRef::Clip(*id),
        ControlScope::Note(id) => ResourceRef::Note(*id),
        ControlScope::Project => ResourceRef::Project(project),
    }
}

fn clip_window(clip: &Clip) -> Option<TypedRange> {
    use synvelis_project::{ClipExtent, TimeAnchor};
    // Only a musical placement collapses into a `TypedRange`
    // honestly. A `Media` anchor and `Media` extent each carry their
    // own `(numerator, denominator)` time base, and `TypedRange`
    // carries none — combining their raw `value()`s would compare
    // different units, like `project_range_window`'s media arm.
    match (clip.placement.anchor, clip.placement.extent) {
        (TimeAnchor::Musical(anchor), ClipExtent::Musical(extent)) => Some(TypedRange {
            domain: TimeDomain::ProjectMusical,
            start: anchor.value(),
            len: extent.value(),
        }),
        _ => None,
    }
}

/// A [`ProjectRange`] as a typed window where the domain allows it.
fn project_range_window(range: &ProjectRange) -> Option<TypedRange> {
    match range {
        ProjectRange::Musical(r) => Some(TypedRange {
            domain: TimeDomain::ProjectMusical,
            start: r.start().value(),
            len: r.end().ticks_since(r.start()).ok()?,
        }),
        // A media span's endpoints can carry different time bases —
        // there is no single honest typed range.
        ProjectRange::Media(_) => None,
    }
}

/// The domain window `resource` occupies for range filtering —
/// `None` when it has no extent expressible in a single domain.
///
/// A [`MidiSource`] is filtered by its note payload instead (a
/// source-native range intersects notes, not the source itself), so
/// it reports `None` here.
#[must_use]
pub fn item_window(project: &Project, resource: &ResourceRef) -> Option<TypedRange> {
    match resource {
        ResourceRef::Clip(id) => project.clips.get(id).and_then(clip_window),
        ResourceRef::Note(id) => project
            .midi_sources
            .iter()
            .find_map(|(_, s)| s.notes.get(id))
            .map(|note| TypedRange {
                domain: TimeDomain::SourceNative,
                start: note.start.value(),
                len: note.len.value(),
            }),
        ResourceRef::TakeGroup(id) => project
            .comp
            .take_groups
            .get(id)
            .and_then(|g| project_range_window(&g.target_range)),
        _ => None,
    }
}

/// Whether a MIDI source carries at least one note intersecting a
/// source-native window.
#[must_use]
pub fn midi_source_intersects(source: &MidiSource, start: i64, len: i64) -> bool {
    let Some(end) = start.checked_add(len) else {
        return false;
    };
    source.notes.values().any(|note| {
        note.end()
            .is_ok_and(|note_end| note.start.value() < end && note_end.value() > start)
    })
}

/// Projects one resource as a [`ProjectedItem`] against the pinned
/// `project`, emitting the requested [`FieldGroup`]s that resolve.
///
/// - `metadata` — identity + structure for every kind.
/// - `content` — MIDI source notes and clip-local note timing.
/// - `media` — bounded media descriptors on media sources only.
/// - `derived` — always reports unavailable: the aggregate carries no
///   derived data and the projection never fabricates it.
/// - `selection` — handled at projection level, never per item.
///
/// A resource that does not resolve returns a [`Completeness::Unknown`]
/// item — the same verdict whether it is absent or never existed.
#[must_use]
pub fn project_item(
    project: &Project,
    resource: &ResourceRef,
    fields: &FieldSelection,
    range: Option<TypedRange>,
) -> ProjectedItem {
    let mut item = ProjectedItem {
        resource: *resource,
        completeness: Completeness::Complete,
        fields: Map::new(),
        unavailable: Vec::new(),
    };
    for group in fields.groups() {
        let name = group.name();
        let emitted = match group {
            FieldGroup::Metadata => metadata_group(project, resource),
            FieldGroup::Content => content_group(project, resource, range),
            FieldGroup::Media => media_group(project, resource),
            FieldGroup::Derived => None,
            // Selection is a projection-level group — never item data.
            FieldGroup::Selection => continue,
        };
        match emitted {
            Some(payload) => {
                item.fields.insert(name.to_owned(), payload);
            }
            None => item.unavailable.push(name.to_owned()),
        }
    }
    if resolve_entity(project, resource).is_none() {
        item.completeness = Completeness::Unknown;
        item.fields.clear();
        item.unavailable.clear();
    } else if !item.unavailable.is_empty() {
        item.completeness = Completeness::Unavailable;
    }
    item
}

/// Whether `resource` resolves to any entity in `project`.
fn resolve_entity(project: &Project, resource: &ResourceRef) -> Option<()> {
    attributes(project, resource).map(|_| ())
}

/// The `metadata` group — identity and structure, per kind.
#[allow(clippy::too_many_lines)]
fn metadata_group(project: &Project, resource: &ResourceRef) -> Option<Value> {
    Some(match resource {
        ResourceRef::Project(id) if *id == project.project_id => json!({
            "name": project.name,
            "ppqn": project.ppqn.0,
            "counts": {
                "tracks": project.tracks.len(),
                "lanes": project.lanes.len(),
                "clips": project.clips.len(),
                "notes": project.notes.len(),
                "assets": project.assets.len(),
                "sources": project.sources.len(),
                "midi_sources": project.midi_sources.len(),
                "take_groups": project.comp.take_groups.len(),
                "comps": project.comp.comps.len(),
                "parameter_targets": project.control.targets.len(),
                "automation_lanes": project.control.lanes.len(),
            },
        }),
        ResourceRef::Track(id) => {
            let Track { name, lane_ids, .. } = project.tracks.get(id)?;
            json!({
                "name": name,
                "lanes": lane_ids.iter().map(|l| l.to_tagged()).collect::<Vec<_>>(),
            })
        }
        ResourceRef::Lane(id) => {
            let Lane {
                track_id, clip_ids, ..
            } = project.lanes.get(id)?;
            json!({
                "track": track_id.to_tagged(),
                "clips": clip_ids.iter().map(|c| c.to_tagged()).collect::<Vec<_>>(),
            })
        }
        ResourceRef::Clip(id) => {
            let clip = project.clips.get(id)?;
            clip_metadata(clip)
        }
        ResourceRef::Source(id) => {
            if let Some(Source {
                asset_id,
                content_digest,
                state,
                ..
            }) = project.sources.get(id)
            {
                json!({
                    "source_class": "media",
                    "asset": asset_id.to_tagged(),
                    "content_digest": content_digest,
                    "state": source_state_name(*state),
                })
            } else {
                let source = project.midi_sources.get(id)?;
                json!({
                    "source_class": "midi",
                    "division": {
                        "kind": "ppqn",
                        "ticks_per_quarter": source.division.ticks_per_quarter(),
                    },
                    "note_count": source.notes.len(),
                })
            }
        }
        ResourceRef::Asset(id) => {
            let Asset { digest, .. } = project.assets.get(id)?;
            json!({"digest": digest})
        }
        ResourceRef::Note(id) => {
            if let Some(Note { clip, key, .. }) = project.notes.get(id) {
                json!({
                    "owner": {"kind": "clip", "id": clip.to_tagged()},
                    "key": key,
                })
            } else {
                let (source_id, note) = project
                    .midi_sources
                    .iter()
                    .find_map(|(sid, s)| s.notes.get(id).map(|n| (*sid, n)))?;
                json!({
                    "owner": {"kind": "source", "id": source_id.to_tagged()},
                    "key": note.key,
                })
            }
        }
        ResourceRef::TakeGroup(id) => {
            let TakeGroup {
                owner_lane_id,
                target_range,
                take_ids,
                active_comp_id,
                source_rate,
                ..
            } = project.comp.take_groups.get(id)?;
            json!({
                "lane": owner_lane_id.to_tagged(),
                "target_range": project_range_json(target_range),
                "take_count": take_ids.len(),
                "active_comp": active_comp_id.map(synvelis_project::CompId::to_tagged),
                "source_rate": source_rate,
            })
        }
        ResourceRef::Comp(id) => {
            let Comp {
                take_group_id,
                region_ids,
                transition_ids,
                ..
            } = project.comp.comps.get(id)?;
            json!({
                "take_group": take_group_id.to_tagged(),
                "region_count": region_ids.len(),
                "transition_count": transition_ids.len(),
            })
        }
        ResourceRef::ParameterTarget(id) => {
            let ParameterTarget {
                owner,
                name,
                kind,
                base,
                ..
            } = project.control.targets.get(id)?;
            json!({
                "name": name,
                "owner": control_scope_json(owner),
                "kind": value_kind_name(*kind),
                "base": base,
            })
        }
        ResourceRef::AutomationLane(id) => {
            let lane = project.control.lanes.get(id)?;
            json!({
                "target": lane.target.to_tagged(),
                "region_count": lane.regions.len(),
            })
        }
        // `ResourceRef::Project` with a foreign id — the project arm
        // above guards on `*id == project.project_id`.
        ResourceRef::Project(_) => return None,
    })
}

fn clip_metadata(clip: &Clip) -> Value {
    use synvelis_project::{ClipExtent, TimeAnchor};
    let anchor = match clip.placement.anchor {
        TimeAnchor::Musical(tick) => json!({
            "domain": "project-musical",
            "start": tick.value(),
        }),
        TimeAnchor::Absolute(media) => json!({
            "domain": "media",
            "start": media.value(),
            "time_base": {
                "numerator": media.numerator(),
                "denominator": media.denominator(),
            },
        }),
    };
    let extent = match clip.placement.extent {
        ClipExtent::Musical(dur) => json!({
            "domain": "project-musical",
            "len": dur.value(),
        }),
        ClipExtent::Absolute(dur) => json!({
            "domain": "media",
            "len": dur.value(),
            "time_base": {
                "numerator": dur.numerator(),
                "denominator": dur.denominator(),
            },
        }),
    };
    let region = &clip.playback.source_region;
    let mapping = match &clip.playback.mapping {
        PlaybackMapping::Natural => json!({"kind": "natural"}),
        PlaybackMapping::Loop => json!({"kind": "loop"}),
        PlaybackMapping::SimpleStretch(ratio) => json!({
            "kind": "simple-stretch",
            "numerator": ratio.numerator(),
            "denominator": ratio.denominator(),
        }),
    };
    json!({
        "lane": clip.lane_id.to_tagged(),
        // The shared source identity — emitted verbatim; reading the
        // clip never implies reading the source's own data (SEC-05).
        "source": clip.source_id.to_tagged(),
        "muted": clip.muted,
        "placement": {"anchor": anchor, "extent": extent},
        "playback": {
            "source_region": {
                "domain": "source-native",
                "start": region.start().value(),
                "len": region.end().frames_since(region.start()).ok(),
            },
            "mapping": mapping,
        },
    })
}

/// The `content` group — structured payloads: MIDI source notes
/// (native ticks verbatim, filtered by a source-native range) and
/// clip-local note timing (clip-relative beats).
fn content_group(
    project: &Project,
    resource: &ResourceRef,
    range: Option<TypedRange>,
) -> Option<Value> {
    match resource {
        ResourceRef::Source(id) => {
            let source = project.midi_sources.get(id)?;
            let native = range.filter(|r| r.domain == TimeDomain::SourceNative);
            let notes: Vec<Value> = source
                .notes
                .values()
                .filter(|note| {
                    native.is_none_or(|r| {
                        r.start.checked_add(r.len).is_some_and(|end| {
                            note.end().is_ok_and(|note_end| {
                                note.start.value() < end && note_end.value() > r.start
                            })
                        })
                    })
                })
                .map(|note| {
                    json!({
                        "id": note.id.to_tagged(),
                        "key": note.key,
                        "velocity": note.velocity,
                        "start": note.start.value(),
                        "len": note.len.value(),
                    })
                })
                .collect();
            Some(json!({
                // Source-native time is preserved verbatim — the
                // projection never requantizes to project PPQN.
                "domain": "source-native",
                "division": {
                    "kind": "ppqn",
                    "ticks_per_quarter": source.division.ticks_per_quarter(),
                },
                "notes": notes,
            }))
        }
        ResourceRef::Clip(id) => {
            let clip = project.clips.get(id)?;
            let notes: Vec<Value> = project
                .notes
                .values()
                .filter(|n| n.clip == clip.id)
                .map(|n| {
                    json!({
                        "id": n.id.to_tagged(),
                        "key": n.key,
                        "start": {"num": n.start.num(), "den": n.start.den()},
                        "len": {"num": n.len.num(), "den": n.len.den()},
                    })
                })
                .collect();
            Some(json!({"domain": "clip-beats", "notes": notes}))
        }
        ResourceRef::Note(id) => {
            if let Some(note) = project.notes.get(id) {
                Some(json!({
                    "domain": "clip-beats",
                    "key": note.key,
                    "start": {"num": note.start.num(), "den": note.start.den()},
                    "len": {"num": note.len.num(), "den": note.len.den()},
                }))
            } else {
                let (source_id, source) = project
                    .midi_sources
                    .iter()
                    .find(|(_, s)| s.notes.contains_key(id))?;
                let note = &source.notes[id];
                Some(json!({
                    "domain": "source-native",
                    "division": {
                        "kind": "ppqn",
                        "ticks_per_quarter": source.division.ticks_per_quarter(),
                    },
                    "source": source_id.to_tagged(),
                    "key": note.key,
                    "velocity": note.velocity,
                    "start": note.start.value(),
                    "len": note.len.value(),
                }))
            }
        }
        _ => None,
    }
}

/// The `media` group — the bounded descriptor of a media source:
/// asset identity, content digest/version, and verification state.
/// Media *bytes* are never part of the projection — a bounded transfer
/// is a separate contract (the issue's non-target).
fn media_group(project: &Project, resource: &ResourceRef) -> Option<Value> {
    match resource {
        ResourceRef::Source(id) => {
            let Source {
                asset_id,
                content_digest,
                state,
                ..
            } = project.sources.get(id)?;
            Some(json!({
                "asset": asset_id.to_tagged(),
                "content_digest": content_digest,
                "state": source_state_name(*state),
                "transfer": "descriptor-only",
            }))
        }
        _ => None,
    }
}

fn project_range_json(range: &ProjectRange) -> Value {
    match range {
        ProjectRange::Musical(r) => json!({
            "domain": "project-musical",
            "start": r.start().value(),
            "len": r.end().ticks_since(r.start()).ok(),
        }),
        ProjectRange::Media(span) => json!({
            "domain": "media",
            "start": {
                "value": span.start().value(),
                "numerator": span.start().numerator(),
                "denominator": span.start().denominator(),
            },
            "end": {
                "value": span.end().value(),
                "numerator": span.end().numerator(),
                "denominator": span.end().denominator(),
            },
        }),
    }
}

fn control_scope_json(scope: &ControlScope) -> Value {
    match scope {
        ControlScope::Track(id) => json!({"scope": "track", "id": id.to_tagged()}),
        ControlScope::Clip(id) => json!({"scope": "clip", "id": id.to_tagged()}),
        ControlScope::Note(id) => json!({"scope": "note", "id": id.to_tagged()}),
        ControlScope::Project => json!({"scope": "project"}),
    }
}

pub(crate) const fn source_state_name(state: SourceState) -> &'static str {
    match state {
        SourceState::Imported => "imported",
        SourceState::Verified => "verified",
        SourceState::Committed => "committed",
        SourceState::Saved => "saved",
    }
}

const fn value_kind_name(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Continuous => "continuous",
        ValueKind::Stepped { .. } => "stepped",
        ValueKind::Boolean => "boolean",
        ValueKind::Choice { .. } => "choice",
    }
}
