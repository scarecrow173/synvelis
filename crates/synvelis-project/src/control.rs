//! The Parameter Control Model (issue #40, implementing the agreed
//! model of issue #6): persisted automation/modulation entities, value
//! evaluation, control-graph compilation, explicit conflict detection,
//! and the boundary to the ruviform parameter contracts.
//!
//! Consumed upstream contracts (never reimplemented):
//!
//! - rf#189 musical time — [`BeatPosition`]/[`Ppqn`]/[`TickPosition`]/
//!   [`TempoMap`]; persisted through the [`BeatPoint`] wire DTO because
//!   ruviform types carry no serde impl.
//! - rf#186 param control — [`ParamId`]/[`ParamFlags`]/[`ParamValue`]/
//!   [`ParamInfo`]/[`ParamWriteMeans`]/`ParamWriteRejection`/
//!   [`check_param_write`]/[`ParamEvent`]/[`TimedParamEvent`].
//!
//! The model is the single source of truth: Lane, Signal Flow and
//! Inspector projections read this data — they never own a duplicate
//! control series. All mutation flows through [`ProjectOp`] and the
//! [`crate::ProjectController`], so one apply is one undo step and a
//! failed commit leaves the project untouched.
//!
//! [`ProjectOp`]: crate::ProjectOp

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ruviform_media_core::{BeatPosition, MediaTime, Ppqn, Tick, TickPosition};
use ruviform_plugin_core::{
    ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, ParamWriteMeans, TimedParamEvent,
    check_param_write,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    id::{
        AutomationLaneId, ClipId, ModulationAssignmentId, ModulationSourceId, NoteId,
        ParameterTargetId, TrackId,
    },
    project::{Project, TimeAnchor},
    range::Range,
};

// ---------------------------------------------------------------------------
// time wire types
// ---------------------------------------------------------------------------

/// Wire DTO for a canonical beat position `num/den` quarter notes —
/// the rational [`BeatPosition`] of rf#189 without any tick grid.
///
/// Automation points are stored as `BeatPoint` so a tempo change never
/// rewrites them and a PPQN change never quantizes them. Ordering and
/// equality are exact rational comparisons (`i128` cross-products).
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "BeatPointDto", into = "BeatPointDto")]
#[serde(deny_unknown_fields)]
pub struct BeatPoint {
    num: i64,
    den: u32,
}

/// Serialized form of [`BeatPoint`]; validates `den != 0` on load.
#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BeatPointDto {
    num: i64,
    den: u32,
}

impl BeatPoint {
    /// The zero position.
    pub const ZERO: Self = Self { num: 0, den: 1 };

    /// Creates a normalized rational position (`den` must be `>= 1`;
    /// the fraction is reduced).
    ///
    /// # Errors
    /// [`Error::Validation`] when `den == 0`.
    pub fn new(num: i64, den: u32) -> Result<Self, Error> {
        if den == 0 {
            return Err(Error::Validation("beat denominator is zero".into()));
        }
        match BeatPosition::new(num, den) {
            Ok(b) => Ok(Self {
                num: b.numerator(),
                den: b.denominator(),
            }),
            Err(_) => Err(Error::Validation("beat denominator is zero".into())),
        }
    }

    /// Wraps an already-normalized ruviform [`BeatPosition`].
    #[must_use]
    pub const fn from_beat(beat: BeatPosition) -> Self {
        Self {
            num: beat.numerator(),
            den: beat.denominator(),
        }
    }

    /// The canonical [`BeatPosition`] — exact, resolution-independent.
    #[must_use]
    pub fn to_beat(self) -> BeatPosition {
        match BeatPosition::new(self.num, self.den) {
            Ok(b) => b,
            // `den >= 1` is a `BeatPoint` invariant — `new` cannot fail.
            Err(_) => unreachable!("BeatPoint denominator is non-zero"),
        }
    }

    /// Signed numerator (carries the sign).
    #[must_use]
    pub const fn num(self) -> i64 {
        self.num
    }

    /// Denominator (always `>= 1`).
    #[must_use]
    pub const fn den(self) -> u32 {
        self.den
    }

    /// Exact `self + rhs`, checked through [`BeatPosition`].
    ///
    /// # Errors
    /// [`Error::Overflow`] when the reduced result exceeds `i64`/`u32`.
    pub fn checked_add(self, rhs: Self) -> Result<Self, Error> {
        self.to_beat()
            .checked_add(rhs.to_beat())
            .map(Self::from_beat)
            .map_err(|_| Error::Overflow)
    }

    /// Exact `self - rhs`, checked through [`BeatPosition`].
    ///
    /// # Errors
    /// [`Error::Overflow`] when the reduced result exceeds `i64`/`u32`.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, Error> {
        self.to_beat()
            .checked_sub(rhs.to_beat())
            .map(Self::from_beat)
            .map_err(|_| Error::Overflow)
    }

    /// Lossy `f64` projection — evaluation/display only, never the
    /// canonical comparison path.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // documented lossy projection
    pub fn to_f64(self) -> f64 {
        self.num as f64 / f64::from(self.den)
    }
}

impl PartialEq for BeatPoint {
    fn eq(&self, other: &Self) -> bool {
        i128::from(self.num) * i128::from(other.den) == i128::from(other.num) * i128::from(self.den)
    }
}
impl Eq for BeatPoint {}
impl PartialOrd for BeatPoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for BeatPoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (i128::from(self.num) * i128::from(other.den))
            .cmp(&(i128::from(other.num) * i128::from(self.den)))
    }
}
impl fmt::Display for BeatPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.to_beat().fmt(f)
    }
}
impl From<BeatPoint> for BeatPointDto {
    fn from(p: BeatPoint) -> Self {
        Self {
            num: p.num,
            den: p.den,
        }
    }
}
impl TryFrom<BeatPointDto> for BeatPoint {
    type Error = Error;
    fn try_from(dto: BeatPointDto) -> Result<Self, Self::Error> {
        Self::new(dto.num, dto.den)
    }
}

/// `[start, end)` over [`BeatPoint`] positions.
pub type BeatRange = Range<BeatPoint>;

/// Lossy `a - b` in beats for evaluation math (interpolation fractions).
#[allow(clippy::cast_precision_loss)] // documented lossy evaluation path
fn beat_diff_f64(a: BeatPoint, b: BeatPoint) -> f64 {
    (i128::from(a.num()) * i128::from(b.den()) - i128::from(b.num()) * i128::from(a.den())) as f64
        / (i128::from(a.den()) * i128::from(b.den())) as f64
}

// ---------------------------------------------------------------------------
// ownership scope + target description
// ---------------------------------------------------------------------------

/// Which entity owns a control target / source — the scope determines
/// the time coordinate its points are expressed in.
///
/// - `Track` / `Project` — project musical time (beats on the tempo map).
/// - `Clip` — clip-relative beats: point `p` evaluates at project
///   position `clip.start + p` while the clip is musically anchored.
/// - `Note` — note-relative beats; the note entity model is not part
///   of this `PoC`, so note-scoped evaluation reports
///   [`Error::ScopeUnavailable`] honestly.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", content = "id", deny_unknown_fields)]
pub enum ControlScope {
    /// Track/project-time owned parameter.
    Track(TrackId),
    /// Clip-local parameter; coordinates are clip-relative beats.
    Clip(ClipId),
    /// Note-local parameter; coordinates are note-relative beats.
    Note(NoteId),
    /// Project-global parameter.
    Project,
}

/// The quantized/discrete behavior of a target's value domain.
/// Continuous targets accept free normalized values; discrete kinds
/// quantize the final evaluated value by an explicit rule.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ValueKind {
    /// Free normalized `[0, 1]` domain.
    Continuous,
    /// `steps` equally spaced legal values (`steps >= 2`).
    Stepped {
        /// Number of legal steps (>= 2).
        steps: u32,
    },
    /// Two legal values (`0.0` / `1.0`).
    Boolean,
    /// `options` choice values (`options >= 2`).
    Choice {
        /// Number of legal options (>= 2).
        options: u32,
    },
}

impl ValueKind {
    /// Validates the kind's own invariants.
    ///
    /// # Errors
    /// [`Error::Validation`] when a discrete kind has fewer than 2 steps.
    pub fn validate(&self) -> Result<(), Error> {
        match *self {
            Self::Stepped { steps } if steps < 2 => {
                Err(Error::Validation("stepped kind needs >= 2 steps".into()))
            }
            Self::Choice { options } if options < 2 => {
                Err(Error::Validation("choice kind needs >= 2 options".into()))
            }
            _ => Ok(()),
        }
    }
}

/// Quantizes a normalized value by the kind's explicit rule.
#[must_use]
#[allow(clippy::cast_precision_loss)] // documented lossy projection
pub fn quantize_value(kind: ValueKind, value: f64) -> f64 {
    let v = value.clamp(0.0, 1.0);
    match kind {
        ValueKind::Continuous => v,
        ValueKind::Stepped { steps } => (v * f64::from(steps - 1)).round() / f64::from(steps - 1),
        ValueKind::Boolean => {
            if v >= 0.5 {
                1.0
            } else {
                0.0
            }
        }
        ValueKind::Choice { options } => {
            (v * f64::from(options - 1)).round() / f64::from(options - 1)
        }
    }
}

/// Whether continuous modulation may target this kind: only
/// `Continuous` targets accept a free normalized contribution. Discrete
/// kinds reject modulation at assignment time — the final-output
/// quantization rule does not make a continuous increment meaningful.
#[must_use]
pub const fn modulation_allowed(kind: ValueKind) -> bool {
    matches!(kind, ValueKind::Continuous)
}

/// How the normalized domain maps onto a target's plain value domain.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mapping", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ParamMapping {
    /// `plain = min + v * (max - min)`.
    Linear,
    /// `plain = min * (max / min)^v` — requires `min > 0`. Non-linear:
    /// an equal normalized modulation depth produces a *position-
    /// dependent* plain increment, never a fixed plain delta.
    Exponential,
}

impl ParamMapping {
    /// Maps a normalized value into `[min, max]` under this mapping.
    /// `Exponential` with `min <= 0` falls back to nothing — callers
    /// must have validated `min > 0` at capability construction.
    #[must_use]
    pub fn denormalize(&self, value: f64, min: f64, max: f64) -> f64 {
        match self {
            Self::Linear => min + value * (max - min),
            Self::Exponential => min * (max / min).powf(value),
        }
    }
}

/// Fixture/plugin capability contract carried by a target: the
/// [`ParamFlags`]/range data needed to run [`check_param_write`]
/// honestly. `flags_bits` restores [`ParamFlags`] strictly — an
/// unknown bit is a validation error, not silently dropped.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetCapability {
    /// Plugin-local parameter id (rf#186 `ParamId`).
    pub param_id: u32,
    /// Raw `ParamFlags` bitmask (restored via `ParamFlags::from_bits`).
    pub flags_bits: u32,
    /// Lower bound of the plain value range.
    pub min_value: f64,
    /// Upper bound of the plain value range.
    pub max_value: f64,
    /// Default value in the plain domain.
    pub default_value: f64,
}

impl TargetCapability {
    /// Restores the declared [`ParamFlags`].
    ///
    /// # Errors
    /// [`Error::Validation`] when `flags_bits` contains an unknown bit.
    pub fn flags(&self) -> Result<ParamFlags, Error> {
        ParamFlags::from_bits(self.flags_bits)
            .ok_or_else(|| Error::Validation("unknown param flag bits".into()))
    }

    /// The [`ParamInfo`] needed for [`check_param_write`].
    ///
    /// # Errors
    /// [`Error::Validation`] when flags are unknown or the range is
    /// non-finite / inverted.
    pub fn param_info(&self, name: &'static str) -> Result<ParamInfo, Error> {
        if !(self.min_value.is_finite()
            && self.max_value.is_finite()
            && self.default_value.is_finite()
            && self.min_value <= self.max_value)
        {
            return Err(Error::Validation("capability range is invalid".into()));
        }
        Ok(ParamInfo {
            id: ParamId(self.param_id),
            name,
            unit: "",
            min_value: self.min_value,
            max_value: self.max_value,
            default_value: self.default_value,
            flags: self.flags()?,
        })
    }
}

/// A controllable parameter — the identity automation and modulation
/// bind to. `base` is the normalized manual/default value
/// (`[0, 1]`, finite) used when no automation or manual gesture
/// overrides it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterTarget {
    /// Stable ID.
    pub id: ParameterTargetId,
    /// Owning scope (defines the time coordinate of its automation).
    pub owner: ControlScope,
    /// Display name.
    pub name: String,
    /// Discrete/continuous value domain.
    pub kind: ValueKind,
    /// Normalized→plain mapping (non-linear aware).
    pub mapping: ParamMapping,
    /// Optional plugin capability contract (fixture `ParamInfo`).
    /// `None` = a pure internal target (no `ParamId`, no write contract).
    #[serde(default)]
    pub capability: Option<TargetCapability>,
    /// Normalized base value in `[0, 1]`.
    pub base: f64,
}

impl ParameterTarget {
    /// Whether [`ParamFlags::READONLY`] is declared on the capability.
    #[must_use]
    pub fn is_readonly(&self) -> bool {
        self.capability
            .is_some_and(|c| c.flags().is_ok_and(|f| f.contains(ParamFlags::READONLY)))
    }
}

// ---------------------------------------------------------------------------
// automation
// ---------------------------------------------------------------------------

/// One automation point: normalized value at a scope-relative beat.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationPoint {
    /// Position in the target's scope coordinate.
    pub pos: BeatPoint,
    /// Normalized value in `[0, 1]` (finite).
    pub value: f64,
}

/// How the value moves between two consecutive points.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "interp", rename_all = "kebab-case")]
pub enum Interpolation {
    /// Hold the previous point's value until the next point.
    Step,
    /// Linear ramp between the two points.
    Linear,
}

/// A bounded group of points over `[range.start, range.end)`.
/// Regions on the same target must never overlap — competing series
/// on the same evaluation time are a detected conflict, not a silent
/// priority decision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationRegion {
    /// Half-open extent this region's points live in.
    pub range: BeatRange,
    /// Segment interpolation used from each point to the next.
    pub interpolation: Interpolation,
    /// Points, strictly ascending in `pos`, all inside `range`.
    pub points: Vec<AutomationPoint>,
}

impl AutomationRegion {
    /// Validates the region's own invariants.
    ///
    /// # Errors
    /// [`Error::Validation`] on empty points, unsorted/duplicate
    /// positions, out-of-range points or non-finite/out-of-domain
    /// values.
    pub fn validate(&self) -> Result<(), Error> {
        if self.points.is_empty() {
            return Err(Error::Validation("region has no points".into()));
        }
        if self.range.is_empty() {
            return Err(Error::Validation("region range is empty".into()));
        }
        let mut prev: Option<BeatPoint> = None;
        for p in &self.points {
            if !p.value.is_finite() || !(0.0..=1.0).contains(&p.value) {
                return Err(Error::Validation("automation value outside [0,1]".into()));
            }
            if !self.range.contains(p.pos) {
                return Err(Error::Validation(format!(
                    "point {} outside region range",
                    p.pos
                )));
            }
            if prev.is_some_and(|q| p.pos <= q) {
                return Err(Error::Validation(
                    "region points not strictly ascending".into(),
                ));
            }
            prev = Some(p.pos);
        }
        Ok(())
    }
}

/// A persistent automation series bound to one [`ParameterTarget`].
/// The lane owns region data only — evaluation merges all lanes of a
/// target into one logical series; nothing outside `Project::control`
/// holds a copy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationLane {
    /// Stable ID.
    pub id: AutomationLaneId,
    /// The parameter this lane feeds.
    pub target: ParameterTargetId,
    /// Point groups (ranges never overlap on the same target).
    pub regions: Vec<AutomationRegion>,
}

// ---------------------------------------------------------------------------
// modulation
// ---------------------------------------------------------------------------

/// LFO waveform.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum LfoShape {
    /// `sin(2π·phase)` in `[-1, 1]`.
    Sine,
    /// Triangle in `[-1, 1]`.
    Triangle,
    /// Square in `{-1, 1}`.
    Square,
}

/// What a [`ModulationSource`] emits: a normalized source signal in
/// `[-1, 1]` (bipolar sources) or `[0, 1]` (unipolar).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", deny_unknown_fields)]
pub enum SourceKind {
    /// Periodic oscillator. `rate` is the cycle length in beats (> 0).
    Lfo {
        /// Cycle length in beats (> 0).
        rate: BeatPoint,
        /// Waveform.
        shape: LfoShape,
        /// `true` = `[-1, 1]` output; `false` = `[0, 1]`.
        bipolar: bool,
    },
    /// Constant macro knob value in `[-1, 1]`.
    Macro {
        /// Constant output.
        value: f64,
    },
    /// Sequenced values: `floor(pos / step) % len` indexes `values`.
    StepSeq {
        /// Step outputs in `[-1, 1]` (non-empty).
        values: Vec<f64>,
        /// Step length in beats (> 0).
        step: BeatPoint,
    },
}

impl SourceKind {
    /// Validates the source definition.
    ///
    /// # Errors
    /// [`Error::Validation`] on non-positive rates/steps, empty step
    /// sequences or out-of-domain values.
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Lfo { rate, .. } => {
                if rate.to_f64() <= 0.0 {
                    return Err(Error::Validation("lfo rate must be > 0".into()));
                }
            }
            Self::Macro { value } => {
                if !value.is_finite() || !(-1.0..=1.0).contains(value) {
                    return Err(Error::Validation("macro value outside [-1,1]".into()));
                }
            }
            Self::StepSeq { values, step } => {
                if step.to_f64() <= 0.0 {
                    return Err(Error::Validation("step length must be > 0".into()));
                }
                if values.is_empty() {
                    return Err(Error::Validation("step sequence is empty".into()));
                }
                if values
                    .iter()
                    .any(|v| !v.is_finite() || !(-1.0..=1.0).contains(v))
                {
                    return Err(Error::Validation(
                        "step sequence value outside [-1,1]".into(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// The source's signal at `pos` — deterministic `f64` math on the
    /// lossy projection; the canonical position stays `BeatPosition`.
    #[must_use]
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    // documented lossy evaluation path; step-seq index truncation is intentional
    pub fn value(&self, pos: BeatPoint) -> f64 {
        match self {
            Self::Lfo {
                rate,
                shape,
                bipolar,
            } => {
                let phase = pos.to_f64() / rate.to_f64();
                let frac = phase - phase.floor();
                let raw = match shape {
                    LfoShape::Sine => (std::f64::consts::TAU * frac).sin(),
                    LfoShape::Triangle => 4.0 * (frac - (frac + 0.5).floor()).abs() - 1.0,
                    LfoShape::Square => {
                        if frac < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                };
                if *bipolar { raw } else { raw.midpoint(1.0) }
            }
            Self::Macro { value } => *value,
            Self::StepSeq { values, step } => {
                let idx = (pos.to_f64() / step.to_f64()).floor() as i64;
                let len = i64::try_from(values.len()).unwrap_or(i64::MAX);
                values[usize::try_from(idx.rem_euclid(len)).unwrap_or(0)]
            }
        }
    }
}

/// A modulation signal provider owned by a scope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulationSource {
    /// Stable ID.
    pub id: ModulationSourceId,
    /// Owning scope.
    pub owner: ControlScope,
    /// Signal definition.
    pub kind: SourceKind,
}

/// Whether a bipolar source signal is applied symmetrically or lifted
/// into `[0, 1]` before scaling.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "polarity", rename_all = "kebab-case")]
pub enum Polarity {
    /// Signal applied as `[-1, 1]`.
    Bipolar,
    /// Signal lifted to `[0, 1]` via `(x + 1) / 2`.
    Unipolar,
}

/// Deterministic shaping applied to the raw source signal before
/// polarity lifting and depth scaling.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "transform", rename_all = "kebab-case")]
pub enum Transform {
    /// `x`
    Identity,
    /// `-x`
    Invert,
    /// `|x|`
    Abs,
    /// `x * x` — always `>= 0`.
    Square,
}

impl Transform {
    /// Applies the transform to a `[-1, 1]` signal.
    #[must_use]
    pub fn apply(&self, x: f64) -> f64 {
        match self {
            Self::Identity => x,
            Self::Invert => -x,
            Self::Abs => x.abs(),
            Self::Square => x * x,
        }
    }
}

/// Binds a [`ModulationSource`] to a [`ParameterTarget`] with a depth
/// (normalized increment scale, finite; sign allowed) and an optional
/// second target driving the depth itself (hierarchical modulation —
/// the edge the control graph orders).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulationAssignment {
    /// Stable ID.
    pub id: ModulationAssignmentId,
    /// Signal provider.
    pub source: ModulationSourceId,
    /// Modulated parameter.
    pub target: ParameterTargetId,
    /// Normalized increment scale applied to the transformed signal.
    pub depth: f64,
    /// Optional target whose evaluated normalized value scales
    /// `depth` — `Some(t)` means the contribution is
    /// `depth * eval(t).host_normalized * transformed`.
    #[serde(default)]
    pub depth_source: Option<ParameterTargetId>,
    /// Bipolar/unipolar application of the transformed signal.
    pub polarity: Polarity,
    /// Signal shaping.
    pub transform: Transform,
    /// Disabled assignments contribute nothing and create no edges.
    pub enabled: bool,
}

/// The complete persisted control model — part of the [`Project`]
/// aggregate, the single source of truth for every Lane / Signal Flow /
/// Inspector projection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlModel {
    /// Parameter targets by ID.
    pub targets: BTreeMap<ParameterTargetId, ParameterTarget>,
    /// Automation lanes by ID.
    pub lanes: BTreeMap<AutomationLaneId, AutomationLane>,
    /// Modulation sources by ID.
    pub sources: BTreeMap<ModulationSourceId, ModulationSource>,
    /// Modulation assignments by ID.
    pub assignments: BTreeMap<ModulationAssignmentId, ModulationAssignment>,
}

// ---------------------------------------------------------------------------
// conflict detection — explicit, never a hidden priority
// ---------------------------------------------------------------------------

/// Two automation series competing for the same target at the same
/// evaluation time. Reported — never resolved by a hidden order.
#[derive(Clone, Debug, PartialEq)]
pub struct AutomationConflict {
    /// The target with competing series.
    pub target: ParameterTargetId,
    /// First lane in the conflict.
    pub lane_a: AutomationLaneId,
    /// Second lane (or the same lane for intra-lane overlap).
    pub lane_b: AutomationLaneId,
    /// The shared evaluation interval.
    pub overlap: BeatRange,
}

/// Detects every pair of regions on the same target whose ranges
/// overlap. A well-formed project has none — ops enforce the invariant
/// at write time; this reports what a tampered snapshot contains.
#[must_use]
pub fn detect_conflicts(project: &Project) -> Vec<AutomationConflict> {
    let mut by_target: BTreeMap<ParameterTargetId, Vec<(AutomationLaneId, BeatRange)>> =
        BTreeMap::new();
    for lane in project.control.lanes.values() {
        for region in &lane.regions {
            by_target
                .entry(lane.target)
                .or_default()
                .push((lane.id, region.range));
        }
    }
    let mut conflicts = Vec::new();
    for (target, entries) in by_target {
        for i in 0..entries.len() {
            for j in i..entries.len() {
                let (lane_a, range_a) = entries[i];
                let (lane_b, range_b) = entries[j];
                if lane_a == lane_b && i == j {
                    continue;
                }
                if let Some(overlap) = range_a.intersection(&range_b) {
                    conflicts.push(AutomationConflict {
                        target,
                        lane_a,
                        lane_b,
                        overlap,
                    });
                }
            }
        }
    }
    conflicts
}

/// Caller-chosen resolution for a conflicting write — explicit, never
/// implied by insertion order.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConflictPolicy {
    /// Reject the write when any existing region overlaps —
    /// [`Error::AutomationConflict`] reports the competing series.
    Reject,
    /// Explicitly overwrite: existing points inside the new region's
    /// range are removed (regions split at the boundary) before the
    /// new region is inserted.
    Replace,
}

/// Removes every point inside `cut` from `regions`, splitting ranges
/// at the boundary and dropping pieces left without points.
pub fn cut_range_points(regions: &mut Vec<AutomationRegion>, cut: &BeatRange) {
    let mut next: Vec<AutomationRegion> = Vec::with_capacity(regions.len() + 1);
    for region in regions.drain(..) {
        if !region.range.overlaps(cut) {
            next.push(region);
            continue;
        }
        let left_pts: Vec<AutomationPoint> = region
            .points
            .iter()
            .copied()
            .filter(|p| p.pos < cut.start())
            .collect();
        let right_pts: Vec<AutomationPoint> = region
            .points
            .iter()
            .copied()
            .filter(|p| p.pos >= cut.end())
            .collect();
        if let (Ok(left_range), false) = (
            Range::try_new(region.range.start(), cut.start()),
            left_pts.is_empty(),
        ) {
            next.push(AutomationRegion {
                range: left_range,
                interpolation: region.interpolation,
                points: left_pts,
            });
        }
        if let (Ok(right_range), false) = (
            Range::try_new(cut.end(), region.range.end()),
            right_pts.is_empty(),
        ) {
            next.push(AutomationRegion {
                range: right_range,
                interpolation: region.interpolation,
                points: right_pts,
            });
        }
    }
    *regions = next;
}

// ---------------------------------------------------------------------------
// evaluation
// ---------------------------------------------------------------------------

/// Where the evaluation's reference value came from — automation wins
/// only where points exist; a manual gesture wins during touch; base
/// is the persistent default.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReferenceSource {
    /// `ParameterTarget::base`.
    Base,
    /// The merged automation series.
    Automation,
    /// An injected manual gesture value (pre-modulation).
    Manual,
}

/// One assignment's contribution to the evaluated value — every stage
/// of the math is exposed for the Inspector breakdown.
#[derive(Clone, Debug, PartialEq)]
pub struct Contribution {
    /// The assignment that produced this contribution.
    pub assignment: ModulationAssignmentId,
    /// The source evaluated.
    pub source: ModulationSourceId,
    /// Raw source signal at the evaluation position.
    pub raw: f64,
    /// Signal after transform + polarity lifting.
    pub transformed: f64,
    /// Effective depth (assignment depth × depth-source value).
    pub depth: f64,
    /// The normalized increment added to the reference.
    pub value: f64,
}

/// The full value breakdown for one target at one position — the data
/// Lane, Signal Flow and Inspector all render from.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlEvaluation {
    /// Scope-resolved position evaluated (clip-local positions are
    /// already resolved to clip-relative beats).
    pub position: BeatPosition,
    /// `ParameterTarget::base`.
    pub base: f64,
    /// Automation value at `position`, if the series reaches it.
    pub automation: Option<f64>,
    /// `manual` → automation → base, in that order.
    pub reference: f64,
    /// Which source supplied `reference`.
    pub reference_source: ReferenceSource,
    /// Per-assignment contributions, in `assignment` ID order — the
    /// fixed evaluation order making the sum order-independent.
    pub contributions: Vec<Contribution>,
    /// `reference + Σ contributions` before clamping.
    pub raw_total: f64,
    /// Value after `[0, 1]` clamp and kind quantization.
    pub host_normalized: f64,
    /// `host_normalized` mapped to the plain domain via the target's
    /// `mapping` (capability range, or `[0, 1]` without a capability).
    pub host_plain: f64,
    /// Whether `raw_total` was clamped into `[0, 1]`.
    pub saturated: bool,
    /// Whether a discrete `ValueKind` quantized the clamped value.
    pub quantized: bool,
}

/// Resolves a scope's evaluation coordinate from a project beat
/// position. Clip scope maps project → clip-relative beats through the
/// clip's current anchor (so `MoveClip` shifts evaluation without
/// rewriting points); `Note` scope is honestly unavailable.
///
/// # Errors
/// [`Error::ScopeUnavailable`] for note scope, media-anchored clips,
/// positions outside the clip's musical range, or missing clips.
pub fn resolve_scope_pos(
    project: &Project,
    scope: ControlScope,
    pos: BeatPoint,
) -> Result<BeatPoint, Error> {
    match scope {
        ControlScope::Track(_) | ControlScope::Project => Ok(pos),
        ControlScope::Note(id) => Err(Error::ScopeUnavailable {
            detail: format!("note scope {id} has no entity model yet"),
        }),
        ControlScope::Clip(clip_id) => {
            let clip = project
                .clips
                .get(&clip_id)
                .ok_or_else(|| Error::NotFound(format!("clip {clip_id}")))?;
            let TimeAnchor::Musical(range) = clip.anchor else {
                return Err(Error::ScopeUnavailable {
                    detail: format!("clip {clip_id} is not musically anchored"),
                });
            };
            let ppqn = Ppqn::new(u16::try_from(project.ppqn.0).map_err(|_| Error::Overflow)?)
                .map_err(|e| Error::Validation(format!("ppqn: {e:?}")))?;
            let start = TickPosition::new(Tick::new(range.start().value()), ppqn).to_beat();
            let end = TickPosition::new(Tick::new(range.end().value()), ppqn).to_beat();
            let pos_b = pos.to_beat();
            if pos_b < start || pos_b >= end {
                return Err(Error::ScopeUnavailable {
                    detail: format!("position {pos} outside clip {clip_id} range"),
                });
            }
            pos_b
                .checked_sub(start)
                .map(BeatPoint::from_beat)
                .map_err(|_| Error::Overflow)
        }
    }
}

/// Merged `(pos, value, segment interpolation)` of every region on
/// every lane of `target`, sorted by position — the one logical series
/// all projections read.
fn merged_series(
    project: &Project,
    target: ParameterTargetId,
) -> Vec<(BeatPoint, f64, Interpolation)> {
    let mut pts: Vec<(BeatPoint, f64, Interpolation)> = project
        .control
        .lanes
        .values()
        .filter(|lane| lane.target == target)
        .flat_map(|lane| {
            lane.regions.iter().flat_map(|region| {
                region
                    .points
                    .iter()
                    .map(|p| (p.pos, p.value, region.interpolation))
            })
        })
        .collect();
    pts.sort_by_key(|p| p.0);
    pts
}

/// The automation value of `target` at `pos` (chase semantics: the
/// latest point at or before `pos` holds; before the first point the
/// series does not reach and `None` falls back to `base`).
///
/// `pos` must already be scope-resolved — callers evaluate a clip-local
/// series with clip-relative beats.
#[must_use]
#[allow(clippy::cast_precision_loss)] // documented lossy evaluation path
pub fn lane_value(project: &Project, target: ParameterTargetId, pos: BeatPoint) -> Option<f64> {
    let series = merged_series(project, target);
    if series.is_empty() || pos < series[0].0 {
        return None;
    }
    let idx = series.partition_point(|p| p.0 <= pos) - 1;
    let (p0, v0, interp) = series[idx];
    let Some(&(p1, v1, _)) = series.get(idx + 1) else {
        return Some(v0);
    };
    match interp {
        Interpolation::Step => Some(v0),
        Interpolation::Linear => {
            let frac = beat_diff_f64(pos, p0) / beat_diff_f64(p1, p0);
            Some(v0 + (v1 - v0) * frac.clamp(0.0, 1.0))
        }
    }
}

/// Evaluates `target` at project position `pos` (or a manual
/// pre-modulation override), returning the full breakdown.
///
/// `manual` is the gesture/reference-layer value — recorded as-is by
/// write modes; the modulated `host_normalized` is **never** a value a
/// caller may write back (no double modulation).
///
/// # Errors
/// [`Error::NotFound`] for missing entities, [`Error::ScopeUnavailable`]
/// when the scope coordinate cannot be resolved,
/// [`Error::ControlCycle`] when depth-source references recurse.
pub fn evaluate_control(
    project: &Project,
    target: ParameterTargetId,
    pos: BeatPoint,
    manual: Option<f64>,
) -> Result<ControlEvaluation, Error> {
    let mut visiting = BTreeSet::new();
    eval_inner(project, target, pos, manual, &mut visiting)
}

fn eval_inner(
    project: &Project,
    target_id: ParameterTargetId,
    pos: BeatPoint,
    manual: Option<f64>,
    visiting: &mut BTreeSet<ParameterTargetId>,
) -> Result<ControlEvaluation, Error> {
    if !visiting.insert(target_id) {
        return Err(Error::ControlCycle {
            path: format!("depth-source recursion at {target_id}"),
        });
    }
    let result = (|| {
        let target = project
            .control
            .targets
            .get(&target_id)
            .ok_or_else(|| Error::NotFound(format!("param {target_id}")))?;
        let local = resolve_scope_pos(project, target.owner, pos)?;
        let automation = lane_value(project, target_id, local);
        let (reference, reference_source) = match (manual, automation) {
            (Some(m), _) => (m, ReferenceSource::Manual),
            (None, Some(a)) => (a, ReferenceSource::Automation),
            (None, None) => (target.base, ReferenceSource::Base),
        };
        let mut contributions = Vec::new();
        for assignment in project.control.assignments.values() {
            if assignment.target != target_id || !assignment.enabled {
                continue;
            }
            let source = project
                .control
                .sources
                .get(&assignment.source)
                .ok_or_else(|| Error::NotFound(format!("mod source {}", assignment.source)))?;
            let raw = source.kind.value(local);
            let mut transformed = assignment.transform.apply(raw);
            if assignment.polarity == Polarity::Unipolar {
                transformed = transformed.midpoint(1.0);
            }
            let depth_eff = match assignment.depth_source {
                Some(ds) => {
                    // Depth modulation reads the driver's resolved value;
                    // manual gestures do not propagate into drivers.
                    let driver = eval_inner(project, ds, pos, None, visiting)?;
                    assignment.depth * driver.host_normalized
                }
                None => assignment.depth,
            };
            contributions.push(Contribution {
                assignment: assignment.id,
                source: assignment.source,
                raw,
                transformed,
                depth: depth_eff,
                value: depth_eff * transformed,
            });
        }
        let raw_total = reference + contributions.iter().map(|c| c.value).sum::<f64>();
        let clamped = raw_total.clamp(0.0, 1.0);
        let host_normalized = quantize_value(target.kind, clamped);
        let (min, max) = target
            .capability
            .map_or((0.0, 1.0), |c| (c.min_value, c.max_value));
        Ok(ControlEvaluation {
            position: local.to_beat(),
            base: target.base,
            automation,
            reference,
            reference_source,
            contributions,
            raw_total,
            host_normalized,
            host_plain: target.mapping.denormalize(host_normalized, min, max),
            saturated: !(0.0..=1.0).contains(&raw_total),
            quantized: !matches!(target.kind, ValueKind::Continuous),
        })
    })();
    visiting.remove(&target_id);
    result
}

// ---------------------------------------------------------------------------
// control graph — non-RT compile, bounded RT-order evaluation
// ---------------------------------------------------------------------------

/// The compiled control graph: a topologically ordered evaluation plan
/// over targets. Preparation/validation happens here, off the RT path;
/// RT-side evaluation walks `order` — bounded, fixed-order, no cycles.
#[derive(Clone, Debug, PartialEq)]
pub struct EvalPlan {
    /// Targets in dependency order (depth drivers before dependents).
    pub order: Vec<ParameterTargetId>,
}

/// Compiles the project's control graph — a DAG over
/// [`ParameterTargetId`] nodes with an edge `depth_source → target`
/// per enabled assignment. Zero-delay cycles (mutual depth-source
/// references) are rejected as [`Error::ControlCycle`]; disabled
/// assignments contribute no edges.
pub struct ControlGraph;

impl ControlGraph {
    /// Compiles the current project into an [`EvalPlan`]. Never mutates
    /// the project; a rejected compile leaves the previously installed
    /// plan untouched (immutability by value semantics).
    ///
    /// # Errors
    /// [`Error::ControlCycle`] when enabled assignments form a
    /// zero-delay dependency cycle.
    pub fn compile(project: &Project) -> Result<EvalPlan, Error> {
        let mut edges: BTreeMap<ParameterTargetId, Vec<ParameterTargetId>> = BTreeMap::new();
        for target_id in project.control.targets.keys() {
            edges.entry(*target_id).or_default();
        }
        for assignment in project.control.assignments.values() {
            if !assignment.enabled {
                continue;
            }
            if let Some(ds) = assignment.depth_source {
                edges.entry(ds).or_default().push(assignment.target);
            }
        }
        // Kahn-style DFS topo with an explicit path for cycle reports.
        let mut order = Vec::with_capacity(edges.len());
        let mut state: BTreeMap<ParameterTargetId, u8> = BTreeMap::new(); // 0=none 1=visiting 2=done
        let mut stack: Vec<ParameterTargetId> = Vec::new();
        for start in edges.keys().copied().collect::<Vec<_>>() {
            if state.get(&start).copied().unwrap_or(0) == 2 {
                continue;
            }
            dfs_topo(start, &edges, &mut state, &mut stack, &mut order)?;
        }
        order.reverse();
        Ok(EvalPlan { order })
    }
}

fn dfs_topo(
    node: ParameterTargetId,
    edges: &BTreeMap<ParameterTargetId, Vec<ParameterTargetId>>,
    state: &mut BTreeMap<ParameterTargetId, u8>,
    stack: &mut Vec<ParameterTargetId>,
    order: &mut Vec<ParameterTargetId>,
) -> Result<(), Error> {
    match state.get(&node).copied().unwrap_or(0) {
        2 => return Ok(()),
        1 => {
            let path = stack
                .iter()
                .chain(std::iter::once(&node))
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Error::ControlCycle { path });
        }
        _ => {}
    }
    state.insert(node, 1);
    stack.push(node);
    for next in edges.get(&node).into_iter().flatten().copied() {
        dfs_topo(next, edges, state, stack, order)?;
    }
    stack.pop();
    state.insert(node, 2);
    order.push(node);
    Ok(())
}

// ---------------------------------------------------------------------------
// dispatch — the rf#186 write contract boundary
// ---------------------------------------------------------------------------

/// Checks whether `target` accepts a write via `means`. A target
/// without a capability contract is a pure internal parameter and
/// accepts anything (the caller owns meaning); one with a capability
/// runs the upstream [`check_param_write`] means×flags matrix
/// unchanged.
///
/// # Errors
/// [`Error::WriteRejected`] carrying the typed
/// [`ParamWriteRejection`], or [`Error::Validation`] for a malformed
/// capability record.
pub fn dispatch_check(target: &ParameterTarget, means: ParamWriteMeans) -> Result<(), Error> {
    let Some(cap) = target.capability else {
        return Ok(());
    };
    let info = cap.param_info("")?;
    check_param_write(&info, means).map_err(|rejection| Error::WriteRejected {
        means: format!("{means:?}"),
        rejection: rejection.to_string(),
    })
}

/// Lowers a normalized evaluated value into a [`TimedParamEvent`]
/// (rf#186): the write contract is checked first, then the value is
/// packaged as a [`ParamEvent`] at `position`.
///
/// # Errors
/// [`Error::WriteRejected`] per `check_param_write`;
/// [`Error::Validation`] when the target carries no `param_id` or the
/// value is non-finite (rf#186 `ParamValue` NaN policy).
pub fn lower_event(
    target: &ParameterTarget,
    value: f64,
    position: MediaTime,
    means: ParamWriteMeans,
) -> Result<TimedParamEvent, Error> {
    let cap = target
        .capability
        .ok_or_else(|| Error::Validation("target has no plugin param_id".into()))?;
    dispatch_check(target, means)?;
    let param_value =
        ParamValue::new(value).map_err(|e| Error::Validation(format!("param value: {e:?}")))?;
    Ok(TimedParamEvent {
        position,
        event: ParamEvent {
            param_id: ParamId(cap.param_id),
            value: param_value,
        },
    })
}
