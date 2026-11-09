//! Arrangement read model (SV-GUI-03 slice 2, Issue #112) — the
//! read-only table backing the Arrangement tab.
//!
//! [`arrangement_rows`] walks `Project.tracks → lanes → clips` in
//! persisted order and resolves each clip's seconds span through the
//! same [`crate::viz_ui::timeline_items`] machinery the timeline
//! surface uses (`TimeAnchor`/`ClipExtent`/`TickTimeMapping`/
//! `resolve_span`), so the table's start/end always agree with the
//! timeline's. It is read-side only: no domain ops, no mutation.
//!
//! **RT contract:** non-RT only.

use std::collections::HashMap;

use crate::session::ProjectSession;
use crate::viz_ui::{TimelineAxis, timeline_items};

/// One Arrangement table row — a clip resolved into its track/lane
/// context with display-ready fields. `start_seconds`/`end_seconds`
/// are `None` when the clip's placement does not resolve on the
/// seconds axis (the row still renders — a missing span is shown
/// honestly, never dropped or fabricated).
#[derive(Clone, Debug, PartialEq)]
pub struct ArrangementRow {
    /// Owning track's display name.
    pub track: String,
    /// Lane tagged id (`lane-<32hex>` — lanes carry no name).
    pub lane: String,
    /// Clip tagged id.
    pub clip: String,
    /// Source tagged id the clip reads.
    pub source: String,
    /// Clip start in seconds, when resolvable.
    pub start_seconds: Option<f64>,
    /// Clip end in seconds, when resolvable.
    pub end_seconds: Option<f64>,
    /// Muted flag — muted clips render but contribute silence.
    pub muted: bool,
}

/// Builds the Arrangement table rows for `session`'s live project.
///
/// Rows follow the persisted structure order — `Track::lane_ids` then
/// `Lane::clip_ids` — not timeline position; a clip whose span fails
/// to resolve keeps its row with `None` seconds. `media_rate` is the
/// seconds axis' frame rate (the same constant the seek range and
/// project length use).
#[must_use]
pub fn arrangement_rows(session: &ProjectSession, media_rate: u32) -> Vec<ArrangementRow> {
    let project = session.project();
    // clip tagged id → resolved seconds span (timeline_items skips
    // unresolvable clips, so a missing entry means None seconds).
    let spans: HashMap<String, (f64, f64)> =
        timeline_items(session, TimelineAxis::Seconds, media_rate)
            .0
            .iter()
            .map(|item| (item.payload.clip.clone(), (item.start, item.end)))
            .collect();
    let mut rows = Vec::with_capacity(project.clips.len());
    for track in project.tracks.values() {
        for lane_id in &track.lane_ids {
            let Some(lane) = project.lanes.get(lane_id) else {
                continue;
            };
            for clip_id in &lane.clip_ids {
                let Some(clip) = project.clips.get(clip_id) else {
                    continue;
                };
                let clip_tag = clip.id.to_tagged();
                let (start_seconds, end_seconds) = spans
                    .get(&clip_tag)
                    .map_or((None, None), |(s, e)| (Some(*s), Some(*e)));
                rows.push(ArrangementRow {
                    track: track.name.clone(),
                    lane: lane.id.to_tagged(),
                    clip: clip_tag,
                    source: clip.source_id.to_tagged(),
                    start_seconds,
                    end_seconds,
                    muted: clip.muted,
                });
            }
        }
    }
    rows
}
