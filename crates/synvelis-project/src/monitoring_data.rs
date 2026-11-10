//! Monitor-domain persisted entities (POD only; behavior lives in
//! `synvelis-recording`). Authored Arm/Monitor declarations and punch
//! marks are part of the `Project` aggregate's serialized state
//! (synvelis#117, schema v12), keeping a single store, single writer,
//! and full undo coverage.
//!
//! Session *observations* — `input_output_ready`, hardware-direct
//! state, latency reports, live monitor sessions — are deliberately
//! absent here: they are runtime/external observations, not authored
//! declarations, and stay session-scoped in `synvelis-app`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    id::{LaneId, TrackId},
    time::MusicalTick,
};

/// Record Arm state of a monitor declaration (serde `"off"`/`"on"`).
///
/// Same vocabulary as `synvelis_recording::ArmState`, duplicated as POD
/// because the dependency direction is `project ← recording` — the app
/// boundary maps between the two.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArmState {
    /// Not armed.
    #[default]
    Off,
    /// Armed.
    On,
}

/// Monitor mode of a monitor declaration
/// (serde `"off"`/`"auto"`/`"on"`).
///
/// Same vocabulary as `synvelis_recording::MonitorState`; see
/// [`ArmState`] for why the project crate carries its own copy.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MonitorState {
    /// Monitor path off.
    #[default]
    Off,
    /// Monitor follows Arm + transport/gate.
    Auto,
    /// Monitor input whenever it is ready.
    On,
}

/// Output-channel → input-channel map of a monitor declaration.
///
/// `channels[j]` names the input channel fed by output channel `j`;
/// the map must be non-empty (enforced by `Project::validate_references`
/// on load and by `ProjectOp::SetTrackMonitor` on apply). Default is the
/// stereo identity `[0, 1]`, matching the session-only predecessor's
/// `ChannelMap::identity(2)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitorChannelMap {
    /// Per-output-channel input channel indices.
    pub channels: Vec<u32>,
}

impl MonitorChannelMap {
    /// The stereo identity map `[0, 1]`.
    #[must_use]
    pub fn identity() -> Self {
        Self {
            channels: vec![0, 1],
        }
    }

    /// Number of declared channels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.channels.len()
    }

    /// `true` when no channels are declared (invalid — rejected by the
    /// reference oracle).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// The channel index slice.
    #[must_use]
    pub fn channels(&self) -> &[u32] {
        &self.channels
    }
}

impl Default for MonitorChannelMap {
    fn default() -> Self {
        Self::identity()
    }
}

/// Track-scoped authored monitor declaration.
///
/// The map key in [`MonitoringData::tracks`] is the [`TrackId`]; the
/// declaration's *existence* is itself authored state ("this track is
/// monitor-managed"). All fields default to the off/auto-resolve
/// posture so a minimally-specified declaration is legal.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackMonitorDecl {
    /// Explicit capture lane. `None` resolves to the track's first
    /// lane when a monitor session starts (existing session semantics).
    pub lane: Option<LaneId>,
    /// Channel map for the monitor binding.
    pub channel_map: MonitorChannelMap,
    /// Record Arm state.
    pub arm: ArmState,
    /// Monitor mode.
    pub monitor: MonitorState,
}

/// Project-wide punch in/out marks (project ticks, half-open
/// `[in, out)` once both are set). Persisted as authored session
/// intent; whether they form a `GateSpec::Punch` is decided by
/// `synvelis-recording` at session start — the project stores the
/// marks verbatim.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PunchMarks {
    /// Punch-in tick (negative values legal).
    pub punch_in: Option<MusicalTick>,
    /// Punch-out tick.
    pub punch_out: Option<MusicalTick>,
}

/// The `Project::monitoring` value: authored monitor declarations plus
/// punch marks. Empty by default (schema v12 introduces the field;
/// `#[serde(default)]` on the project field keeps v11 bodies loadable).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitoringData {
    /// `TrackId` → declaration. `BTreeMap` keeps serialization and
    /// iteration order deterministic.
    pub tracks: BTreeMap<TrackId, TrackMonitorDecl>,
    /// Punch marks shared by the project's monitor sessions.
    pub punch: PunchMarks,
}
