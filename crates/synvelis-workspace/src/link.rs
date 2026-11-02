//! Link groups — semantic selection propagation between views
//! (WS-A scope 3).
//!
//! A link group carries *semantic* targets, not raw selections:
//! publishing a clip selection maps it through the project (clip →
//! owning lane → owning track) so a mixer member highlights the
//! track the clip belongs to. Mapping failure suppresses delivery
//! entirely — a member never receives an event it cannot interpret.
//!
//! Loop prevention is structural:
//! - receivers never re-emit — `deliver` mutates view state only;
//! - each `(receiver, event_id)` pair is applied at most once.
//!
//! Groups are independent: an event only reaches members of the
//! publisher's group, and each view belongs to at most one group.

use std::collections::{BTreeSet, HashMap, HashSet};

use ruviform_ui::dock::ViewInstanceId;
use serde::{Deserialize, Serialize};
use synvelis_project::Project;

use crate::state::{EditingSelection, InspectionMode, SemanticTarget, ViewStateRegistry};

/// A link-group identity. `0` is unused; groups are minted from `1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LinkGroupId(pub u64);

/// One propagated link event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkEvent {
    /// Unique event id (per workspace, monotonically increasing).
    pub id: u64,
    /// Publishing view.
    pub source: ViewInstanceId,
    /// The group the event is scoped to.
    pub group: LinkGroupId,
    /// The *semantic* target after mapping (for example the track
    /// owning a selected clip).
    pub target: SemanticTarget,
}

/// Maps an editing selection to its semantic link target through the
/// project model. `None` = unmappable → no event is produced.
///
/// The WS-A topic is *track selection*: a clip resolves to its owning
/// lane's track, a lane to its track, a track to itself, a source to
/// nothing.
#[must_use]
pub fn map_to_track(selection: &EditingSelection, project: &Project) -> Option<SemanticTarget> {
    match selection.primary()? {
        SemanticTarget::Clip(clip_id) => project
            .clips
            .get(&clip_id)
            .and_then(|clip| project.lanes.get(&clip.lane_id))
            .map(|lane| SemanticTarget::Track(lane.track_id)),
        SemanticTarget::Lane(lane_id) => project
            .lanes
            .get(&lane_id)
            .map(|lane| SemanticTarget::Track(lane.track_id)),
        SemanticTarget::Track(track_id) => project
            .tracks
            .contains_key(&track_id)
            .then_some(SemanticTarget::Track(track_id)),
        SemanticTarget::Source(_) => None,
    }
}

/// Link-group membership and event dispatch.
#[derive(Debug, Default)]
pub struct LinkGroupManager {
    view_group: HashMap<ViewInstanceId, LinkGroupId>,
    members: HashMap<LinkGroupId, BTreeSet<ViewInstanceId>>,
    next_event: u64,
    /// `(receiver, event_id)` pairs already applied — dedup evidence.
    applied: HashSet<(ViewInstanceId, u64)>,
}

impl LinkGroupManager {
    /// Assigns `view` to `group` (`None` removes membership). A view
    /// belongs to at most one group.
    pub fn set_group(&mut self, view: ViewInstanceId, group: Option<LinkGroupId>) {
        if let Some(old) = self.view_group.remove(&view)
            && let Some(set) = self.members.get_mut(&old)
        {
            set.remove(&view);
        }
        if let Some(g) = group {
            self.view_group.insert(view, g);
            self.members.entry(g).or_default().insert(view);
        }
    }

    /// The group `view` belongs to, if any.
    #[must_use]
    pub fn group_of(&self, view: ViewInstanceId) -> Option<LinkGroupId> {
        self.view_group.get(&view).copied()
    }

    /// Member views of `group`, in id order.
    #[must_use]
    pub fn members(&self, group: LinkGroupId) -> Vec<ViewInstanceId> {
        self.members
            .get(&group)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// All current `(view, group)` assignments — for persistence.
    #[must_use]
    pub fn assignments(&self) -> Vec<(ViewInstanceId, LinkGroupId)> {
        let mut out: Vec<_> = self.view_group.iter().map(|(v, g)| (*v, *g)).collect();
        out.sort();
        out
    }

    /// Publishes `source`'s selection (after semantic mapping through
    /// `project`) to the members of its group. Returns the event and
    /// the receiver list; `None` when the source is ungrouped or the
    /// selection does not map.
    pub fn publish(
        &mut self,
        source: ViewInstanceId,
        selection: &EditingSelection,
        project: &Project,
    ) -> Option<(LinkEvent, Vec<ViewInstanceId>)> {
        let group = self.group_of(source)?;
        let target = map_to_track(selection, project)?;
        self.next_event += 1;
        let event = LinkEvent {
            id: self.next_event,
            source,
            group,
            target,
        };
        let receivers: Vec<_> = self
            .members(group)
            .into_iter()
            .filter(|v| *v != source)
            .collect();
        Some((event, receivers))
    }

    /// Delivers `event` to `receiver`'s state. Returns `true` when the
    /// event was applied, `false` when it was dropped (duplicate or
    /// group violation — an event never crosses groups).
    ///
    /// Applied effects: `linked_context` records the semantic target
    /// and provenance; a Follow-mode inspector focus adopts the
    /// target. The editing selection is never touched, and no new
    /// event is emitted from the receiver.
    pub fn deliver(
        &mut self,
        event: LinkEvent,
        receiver: ViewInstanceId,
        states: &mut ViewStateRegistry,
    ) -> bool {
        if receiver == event.source || self.group_of(receiver) != Some(event.group) {
            return false;
        }
        if !self.applied.insert((receiver, event.id)) {
            return false;
        }
        let state = states.ensure(receiver);
        state.linked_context.target = Some(event.target);
        state.linked_context.source = Some(event.source);
        state.linked_context.last_event = Some(event.id);
        if state.inspection_focus.mode == InspectionMode::Follow {
            state.inspection_focus.target = Some(event.target);
        }
        true
    }
}
