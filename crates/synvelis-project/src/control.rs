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
use serde::{Deserialize, Serialize, de::Error as _};

use crate::{
    error::Error,
    id::{
        AutomationLaneId, ClipId, ModulationAssignmentId, ModulationSourceId, NoteId,
        ParameterTargetId, ProcessorId, SendId, StripId, TrackId,
    },
    placement::{ClipExtent, TimeAnchor},
    project::Project,
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

/// The entity that owns a control target / source (issue #121).
///
/// Ownership is *only* about identity/lifecycle — the time coordinate
/// of a target's automation is declared separately by
/// [`AutomationTimeScope`], replacing the legacy `ControlScope` which
/// conflated both responsibilities.
///
/// There is deliberately no `Bus` variant: a bus's audio parameters are
/// bound to the [`StripId`] its `StripOwner::Bus(BusId)` names — the
/// strip is the single DSP authority.
///
/// **Wire contract:** serialized as `{"owner": "<Variant>", "id":
/// "<tagged-id>"}` (no `id` for `Project`). Deserialization also
/// accepts the legacy v6 `{"scope": ...}` tag so the v6→v7 migration
/// step receives a typed `Project` — this is the reader-side migration
/// boundary, not a runtime compatibility layer; serialization only
/// ever writes the new form.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "owner", content = "id", deny_unknown_fields)]
pub enum ControlOwner {
    /// Project-global parameter.
    Project,
    /// Production track's parameter (container owner).
    Track(TrackId),
    /// Channel strip's parameter — fader/pan/mute/solo and bus DSP
    /// bindings (the strip may be `StripOwner::Track` or
    /// `StripOwner::Bus` owned).
    Strip(StripId),
    /// Send-owned parameter — the send's level target.
    Send(SendId),
    /// Processor-owned parameter (plugin param binding).
    Processor(ProcessorId),
    /// Clip-owned parameter.
    Clip(ClipId),
    /// Note-owned parameter.
    Note(NoteId),
}

impl<'de> Deserialize<'de> for ControlOwner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Repr {
            /// Variant name — the migration boundary accepts the legacy
            /// v6 `scope` tag as well as the current `owner` tag.
            #[serde(alias = "scope")]
            owner: String,
            /// Tagged entity id (`track-…`, `strip-…`, …).
            #[serde(default)]
            id: Option<String>,
        }
        fn tagged<T, E>(
            id: Option<String>,
            parse: impl Fn(&str) -> Result<T, Error>,
        ) -> Result<T, E>
        where
            E: serde::de::Error,
        {
            let raw = id.ok_or_else(|| E::custom("control owner missing id"))?;
            parse(&raw).map_err(E::custom)
        }
        let repr = Repr::deserialize(deserializer)?;
        let unknown = || D::Error::custom(format!("unknown control owner {:?}", repr.owner));
        Ok(match repr.owner.as_str() {
            "Project" => Self::Project,
            "Track" => Self::Track(tagged::<_, D::Error>(repr.id, TrackId::from_tagged)?),
            "Strip" => Self::Strip(tagged::<_, D::Error>(repr.id, StripId::from_tagged)?),
            "Send" => Self::Send(tagged::<_, D::Error>(repr.id, SendId::from_tagged)?),
            "Processor" => {
                Self::Processor(tagged::<_, D::Error>(repr.id, ProcessorId::from_tagged)?)
            }
            "Clip" => Self::Clip(tagged::<_, D::Error>(repr.id, ClipId::from_tagged)?),
            "Note" => Self::Note(tagged::<_, D::Error>(repr.id, NoteId::from_tagged)?),
            _ => return Err(unknown()),
        })
    }
}

/// The time coordinate a target's automation points are expressed in
/// (issue #121) — declared independently of [`ControlOwner`].
///
/// - `ProjectMusical` — project musical beats on the tempo map
///   (points evaluate at the project position as-is).
/// - `ProjectAbsolute` — absolute project time. The type is introduced
///   here so persisted bindings can name it; evaluation reports
///   [`Error::ScopeUnavailable`] honestly until the absolute-time point
///   model lands (no silent beats reinterpretation).
/// - `ClipLocal` — clip-relative beats: point `p` evaluates at project
///   position `clip.start + p` while the clip is musically anchored.
///   Only valid with [`ControlOwner::Clip`].
/// - `NoteLocal` — note-relative beats: point `p` evaluates at project
///   position `clip.start + note.start + p` inside the note's
///   `[start, start+len)` domain. Only valid with
///   [`ControlOwner::Note`]. Outside the domain evaluation reports
///   [`Error::ScopeUnavailable`].
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutomationTimeScope {
    /// Project musical beats (default and only value a v6 body derives).
    #[default]
    ProjectMusical,
    /// Absolute project time — declared, resolution unavailable.
    ProjectAbsolute,
    /// Clip-relative beats (`ControlOwner::Clip` only).
    ClipLocal,
    /// Note-relative beats (`ControlOwner::Note` only).
    NoteLocal,
}

/// The semantic role a [`ParameterTarget`] plays for its owner
/// (issue #121 `ParameterBinding::role`).
///
/// `Fader`/`Pan`/`Mute`/`Solo`/`SendLevel` are *singleton* roles: at
/// most one target per owner (`validate_references` and the add-op
/// reject duplicates). `Generic` (user/plugin params and everything a
/// v6 body migrates to) and `ProcessorParam` (a processor carries many
/// plugin params) may coexist in any number.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParameterRole {
    /// Untyped parameter — migrated v6 targets and free-form params.
    #[default]
    Generic,
    /// The strip's fader (one per strip).
    Fader,
    /// The strip's pan (one per strip).
    Pan,
    /// The strip's mute (one per strip).
    Mute,
    /// The strip's solo (one per strip).
    Solo,
    /// The send's level (one per send).
    SendLevel,
    /// A processor's plugin parameter (many per processor).
    ProcessorParam,
}

impl ParameterRole {
    /// Whether at most one target of this role may exist per owner.
    #[must_use]
    pub const fn is_singleton(self) -> bool {
        matches!(
            self,
            Self::Fader | Self::Pan | Self::Mute | Self::Solo | Self::SendLevel
        )
    }
}

/// Whether `owner` may carry `time_scope`: local scopes bind to their
/// matching owner kind; project-time scopes bind to every other owner.
const fn time_scope_allowed(owner: ControlOwner, time_scope: AutomationTimeScope) -> bool {
    match time_scope {
        AutomationTimeScope::ClipLocal => matches!(owner, ControlOwner::Clip(_)),
        AutomationTimeScope::NoteLocal => matches!(owner, ControlOwner::Note(_)),
        AutomationTimeScope::ProjectMusical | AutomationTimeScope::ProjectAbsolute => {
            !matches!(owner, ControlOwner::Clip(_) | ControlOwner::Note(_))
        }
    }
}

/// Validates that `owner` references an existing entity — the shared
/// existence oracle for targets and modulation sources, used by both
/// write-time op checks and [`Project::validate_references`].
///
/// # Errors
/// [`Error::NotFound`] when the referenced entity does not exist.
pub fn validate_control_owner(project: &Project, owner: ControlOwner) -> Result<(), Error> {
    match owner {
        ControlOwner::Track(id) if !project.tracks.contains_key(&id) => {
            Err(Error::NotFound(format!("track {id}")))
        }
        ControlOwner::Strip(id) if !project.routing.strips.contains_key(&id) => {
            Err(Error::NotFound(format!("strip {id}")))
        }
        ControlOwner::Send(id) if !project.routing.sends.contains_key(&id) => {
            Err(Error::NotFound(format!("send {id}")))
        }
        ControlOwner::Processor(id) if !project.routing.processors.contains_key(&id) => {
            Err(Error::NotFound(format!("processor {id}")))
        }
        ControlOwner::Clip(id) if !project.clips.contains_key(&id) => {
            Err(Error::NotFound(format!("clip {id}")))
        }
        ControlOwner::Note(id) if !project.notes.contains_key(&id) => {
            Err(Error::NotFound(format!("note {id}")))
        }
        // `Project` is always valid; every other variant resolved by
        // the guards above.
        _ => Ok(()),
    }
}

/// Validates a target's full binding: owner existence, the
/// owner×time-scope matrix, and singleton `(owner, role)` uniqueness.
/// `exclude` names the target being validated so re-validation of an
/// existing binding does not collide with itself.
///
/// # Errors
/// [`Error::NotFound`] for a missing owner entity,
/// [`Error::Validation`] for an incoherent combination or a duplicate
/// singleton role.
pub fn validate_control_binding(
    project: &Project,
    owner: ControlOwner,
    role: ParameterRole,
    time_scope: AutomationTimeScope,
    exclude: Option<ParameterTargetId>,
) -> Result<(), Error> {
    validate_control_owner(project, owner)?;
    if !time_scope_allowed(owner, time_scope) {
        return Err(Error::Validation(format!(
            "owner {owner:?} cannot carry time scope {time_scope:?}"
        )));
    }
    if role.is_singleton()
        && project
            .control
            .targets
            .values()
            .any(|t| Some(t.id) != exclude && t.owner == owner && t.role == role)
    {
        return Err(Error::Validation(format!(
            "owner {owner:?} already owns a {role:?} target"
        )));
    }
    Ok(())
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
///
/// `owner` + `role` + `time_scope` realize the issue-#121
/// `ParameterBinding` concept as flat persisted fields.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterTarget {
    /// Stable ID.
    pub id: ParameterTargetId,
    /// Owning entity (identity/lifecycle only).
    pub owner: ControlOwner,
    /// Semantic role — singleton roles are unique per owner.
    /// `#[serde(default)]`: absent in v6 bodies, derived by migration.
    #[serde(default)]
    pub role: ParameterRole,
    /// Time coordinate of this target's automation points.
    /// `#[serde(default)]`: absent in v6 bodies, derived by migration.
    #[serde(default)]
    pub time_scope: AutomationTimeScope,
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

/// A modulation signal provider owned by an entity. The owner carries
/// no `time_scope`: the source signal is evaluated at the *assigned
/// target's* resolved coordinate (the established evaluation
/// contract), so ownership is identity/lifecycle only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulationSource {
    /// Stable ID.
    pub id: ModulationSourceId,
    /// Owning entity.
    pub owner: ControlOwner,
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
// cut boundary preservation (issue #132, AUT-02)
// ---------------------------------------------------------------------------

/// Smallest representable sentinel-region extent: `1/u32::MAX` beats —
/// the same half-open extension `prepare_record_commit` uses.
const SENTINEL_EPS_NUM: i64 = 1;
const SENTINEL_EPS_DEN: u32 = u32::MAX;

/// A synthesized boundary point carrying the pre-edit merged-series
/// value at a cut boundary — the right-side resume anchor that keeps
/// the curve outside `[cut.start, cut.end)` identical (issue #132).
///
/// `interp` is the interpolation of the pre-edit chase source — the
/// segment the anchor must continue from `pos` to the next point — so
/// the anchor can only live inside a region whose interpolation
/// matches, or inside a dedicated sentinel region created for it.
#[derive(Copy, Clone, Debug)]
pub(crate) struct BoundaryAnchor {
    /// Boundary position — always `cut.end()` (the right resume side).
    pub pos: BeatPoint,
    /// `v_old(pos)` evaluated on the pre-edit merged series.
    pub value: f64,
    /// Segment interpolation the anchor must carry forward.
    pub interp: Interpolation,
    /// The pre-edit chase source `(pos, value, interp)` the stitch
    /// compares against — when the post-edit chase source already
    /// reproduces it, no anchor is inserted (no silent duplication).
    pub source: (BeatPoint, f64, Interpolation),
}

/// Plans the boundary anchors for a `cut` over the pre-edit merged
/// series `series` (all lanes of one target, sorted by `pos`).
///
/// Returns `(left, right)`:
///
/// - `left` — a point `(cut.start, v_old(cut.start))` the caller may
///   prepend to the incoming region. `None` when the pre-edit series
///   has no point at or before `cut.start` (the region start is owned
///   by the caller's first point — contract exception E-1).
/// - `right` — a [`BoundaryAnchor`] at `cut.end`. `None` when an
///   explicit point exists exactly at `cut.end` (the kept point owns
///   the boundary) or when the pre-edit series has no point before
///   `cut.end` (a point cannot express an unreached series — contract
///   exception E-2).
pub(crate) fn boundary_anchors(
    series: &[(BeatPoint, f64, Interpolation)],
    cut: &BeatRange,
) -> (Option<AutomationPoint>, Option<BoundaryAnchor>) {
    let left = prepared_lane_value(series, cut.start()).map(|value| AutomationPoint {
        pos: cut.start(),
        value: value.clamp(0.0, 1.0),
    });
    let mut right = None;
    if !series.iter().any(|p| p.0 == cut.end()) {
        let idx = series.partition_point(|p| p.0 < cut.end());
        if idx > 0 {
            let source = series[idx - 1];
            if let Some(value) = prepared_lane_value(series, cut.end()) {
                right = Some(BoundaryAnchor {
                    pos: cut.end(),
                    value: value.clamp(0.0, 1.0),
                    interp: source.2,
                    source,
                });
            }
        }
    }
    (left, right)
}

/// Applies `anchor` to the post-cut control model: when the post-edit
/// chase source at `anchor.pos` no longer reproduces the pre-edit one,
/// inserts the anchor as a boundary point.
///
/// Placement: the unique region containing `anchor.pos` — prepended
/// when its interpolation matches the anchor's, otherwise split so the
/// anchor gets a dedicated sentinel region `[pos, p1)` — or, when no
/// region contains `pos`, a sentinel region `[pos, min(next_start,
/// pos + 1/u32::MAX))` on `fallback_lane`. A synthesized point is never
/// inserted where an explicit point already exists, so coincident
/// points (and any silent last-wins ordering) cannot occur.
///
/// `lane.regions` stays sorted by `range.start` — the same persisted
/// order `cut_range_points` produces.
pub(crate) fn stitch_boundary_anchor(
    project: &mut Project,
    target: ParameterTargetId,
    fallback_lane: AutomationLaneId,
    anchor: &BoundaryAnchor,
) -> Result<(), Error> {
    let series = merged_series(project, target);
    let idx = series.partition_point(|p| p.0 < anchor.pos);
    let last = idx.checked_sub(1).map(|i| series[i]);
    if last.is_some_and(|(pos, value, interp)| {
        pos == anchor.source.0
            && value.to_bits() == anchor.source.1.to_bits()
            && interp == anchor.source.2
    }) {
        return Ok(());
    }
    let point = AutomationPoint {
        pos: anchor.pos,
        value: anchor.value,
    };
    // The unique region containing `pos` (at most one — ranges never
    // overlap on a target).
    let host = project
        .control
        .lanes
        .iter()
        .filter(|(_, lane)| lane.target == target)
        .find_map(|(lane_id, lane)| {
            lane.regions
                .iter()
                .position(|r| r.range.contains(anchor.pos))
                .map(|ri| (*lane_id, ri))
        });
    if let Some((lane_id, ri)) = host {
        return stitch_into_host(project, target, lane_id, ri, anchor, point);
    }
    let end = sentinel_end(project, target, anchor.pos)?;
    let sentinel = AutomationRegion {
        range: Range::try_new(anchor.pos, end)
            .map_err(|_| Error::Validation("sentinel range is empty".into()))?,
        interpolation: anchor.interp,
        points: vec![point],
    };
    let lane = project
        .control
        .lanes
        .get_mut(&fallback_lane)
        .ok_or_else(|| Error::NotFound(format!("automation lane {fallback_lane}")))?;
    let at = lane
        .regions
        .partition_point(|r| r.range.start() < anchor.pos);
    lane.regions.insert(at, sentinel);
    Ok(())
}

/// Places the anchor inside `regions[ri]` of `lane_id` — prepended in
/// ascending order when the host interpolation matches, otherwise by
/// splitting the host so the anchor lives in a dedicated sentinel
/// region `[pos, end_bound)` carrying `anchor.interp`.
fn stitch_into_host(
    project: &mut Project,
    target: ParameterTargetId,
    lane_id: AutomationLaneId,
    ri: usize,
    anchor: &BoundaryAnchor,
    point: AutomationPoint,
) -> Result<(), Error> {
    let (host_interp, host_start, host_end, insert_at, first_right_pos) = {
        let region = &project
            .control
            .lanes
            .get(&lane_id)
            .ok_or_else(|| Error::NotFound(format!("automation lane {lane_id}")))?
            .regions[ri];
        let insert_at = region.points.partition_point(|p| p.pos < anchor.pos);
        (
            region.interpolation,
            region.range.start(),
            region.range.end(),
            insert_at,
            region.points.get(insert_at).map(|p| p.pos),
        )
    };
    if host_interp == anchor.interp {
        let region = &mut project
            .control
            .lanes
            .get_mut(&lane_id)
            .ok_or_else(|| Error::NotFound(format!("automation lane {lane_id}")))?
            .regions[ri];
        region.points.insert(insert_at, point);
        return Ok(());
    }
    // Interpolation mismatch: the anchor needs its own segment kind —
    // split the host at `pos` so the anchor lives in a sentinel
    // region with `anchor.interp`.
    let end_bound = match first_right_pos {
        Some(p) => std::cmp::min(p, sentinel_eps_end(anchor.pos)?),
        None => sentinel_end(project, target, anchor.pos)?,
    };
    let lane = project
        .control
        .lanes
        .get_mut(&lane_id)
        .ok_or_else(|| Error::NotFound(format!("automation lane {lane_id}")))?;
    let region = &mut lane.regions[ri];
    let left_pts: Vec<AutomationPoint> = region.points[..insert_at].to_vec();
    let right_pts: Vec<AutomationPoint> = region.points[insert_at..].to_vec();
    let sentinel = AutomationRegion {
        range: Range::try_new(anchor.pos, end_bound)
            .map_err(|_| Error::Validation("sentinel range is empty".into()))?,
        interpolation: anchor.interp,
        points: vec![point],
    };
    let mut rebuilt: Vec<AutomationRegion> = Vec::with_capacity(3);
    if let (Ok(left_range), false) = (Range::try_new(host_start, anchor.pos), left_pts.is_empty()) {
        rebuilt.push(AutomationRegion {
            range: left_range,
            interpolation: host_interp,
            points: left_pts,
        });
    }
    rebuilt.push(sentinel);
    if let (Ok(right_range), false) = (Range::try_new(end_bound, host_end), right_pts.is_empty()) {
        rebuilt.push(AutomationRegion {
            range: right_range,
            interpolation: host_interp,
            points: right_pts,
        });
    }
    lane.regions.splice(ri..=ri, rebuilt);
    Ok(())
}

/// `pos + 1/u32::MAX` beats — the default sentinel-region extent.
fn sentinel_eps_end(pos: BeatPoint) -> Result<BeatPoint, Error> {
    pos.checked_add(BeatPoint::new(SENTINEL_EPS_NUM, SENTINEL_EPS_DEN)?)
}

/// Sentinel-region end when the anchor is not followed by a known
/// point: the earliest region start after `pos`, or `pos + ε`.
fn sentinel_end(
    project: &Project,
    target: ParameterTargetId,
    pos: BeatPoint,
) -> Result<BeatPoint, Error> {
    let next_start = project
        .control
        .lanes
        .values()
        .filter(|lane| lane.target == target)
        .flat_map(|lane| lane.regions.iter())
        .map(|r| r.range.start())
        .filter(|s| *s > pos)
        .min();
    Ok(match next_start {
        Some(s) => std::cmp::min(s, sentinel_eps_end(pos)?),
        None => sentinel_eps_end(pos)?,
    })
}

// ---------------------------------------------------------------------------
// evaluation
// ---------------------------------------------------------------------------

/// Where the evaluation's reference value came from — automation wins
/// only where points exist; a manual gesture wins during touch; a
/// touch return blends between release and automation; base is the
/// persistent default.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReferenceSource {
    /// `ParameterTarget::base`.
    Base,
    /// The merged automation series.
    Automation,
    /// An injected manual gesture value (pre-modulation).
    Manual,
    /// An injected touch-return runtime blend (issue #131, pre-modulation
    /// reference domain — never a recorded point). Set by the owning
    /// session, never by [`evaluate_control`] itself.
    Return,
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

/// Resolves a target's evaluation coordinate from a project beat
/// position under its declared [`AutomationTimeScope`]. `ClipLocal`
/// maps project → clip-relative beats through the owner's current
/// anchor (so `MoveClip` shifts evaluation without rewriting points);
/// `NoteLocal` resolves project → clip-relative → note-relative beats
/// through the owning clip's current anchor.
///
/// # Errors
/// [`Error::ScopeUnavailable`] for media-anchored clips, positions
/// outside the clip's or note's domain, and for `ProjectAbsolute`
/// (absolute-time resolution is not yet supported);
/// [`Error::NotFound`] for missing clips/notes; [`Error::Validation`]
/// when the owner/time-scope combination is incoherent — an invariant
/// `validate_control_binding` rejects before evaluation.
pub fn resolve_scope_pos(
    project: &Project,
    owner: ControlOwner,
    time_scope: AutomationTimeScope,
    pos: BeatPoint,
) -> Result<BeatPoint, Error> {
    match time_scope {
        AutomationTimeScope::ProjectMusical => match owner {
            ControlOwner::Clip(_) | ControlOwner::Note(_) => Err(Error::Validation(
                "local-scope owner cannot carry a project time scope".into(),
            )),
            _ => Ok(pos),
        },
        AutomationTimeScope::ProjectAbsolute => Err(Error::ScopeUnavailable {
            detail: "absolute time scope is not yet supported".into(),
        }),
        AutomationTimeScope::ClipLocal => match owner {
            ControlOwner::Clip(clip_id) => resolve_clip_pos(project, clip_id, pos),
            _ => Err(Error::Validation(
                "clip-local scope requires a clip owner".into(),
            )),
        },
        AutomationTimeScope::NoteLocal => match owner {
            ControlOwner::Note(id) => {
                let note = project
                    .notes
                    .get(&id)
                    .ok_or_else(|| Error::NotFound(format!("note {id}")))?;
                let clip_pos = resolve_clip_pos(project, note.clip, pos)?;
                let note_end = note.start.checked_add(note.len)?;
                if clip_pos < note.start || clip_pos >= note_end {
                    return Err(Error::ScopeUnavailable {
                        detail: format!("position {pos} outside note {id} range"),
                    });
                }
                clip_pos
                    .checked_sub(note.start)
                    .map_err(|_| Error::Overflow)
            }
            _ => Err(Error::Validation(
                "note-local scope requires a note owner".into(),
            )),
        },
    }
}

/// Project beat position → clip-relative beats through the clip's
/// current musical anchor.
fn resolve_clip_pos(
    project: &Project,
    clip_id: ClipId,
    pos: BeatPoint,
) -> Result<BeatPoint, Error> {
    let clip = project
        .clips
        .get(&clip_id)
        .ok_or_else(|| Error::NotFound(format!("clip {clip_id}")))?;
    let (TimeAnchor::Musical(start_tick), ClipExtent::Musical(dur)) =
        (clip.placement.anchor, clip.placement.extent)
    else {
        return Err(Error::ScopeUnavailable {
            detail: format!("clip {clip_id} is not musically anchored"),
        });
    };
    let end_tick = start_tick.checked_add(dur.value())?;
    let ppqn = Ppqn::new(u16::try_from(project.ppqn.0).map_err(|_| Error::Overflow)?)
        .map_err(|e| Error::Validation(format!("ppqn: {e:?}")))?;
    let start = TickPosition::new(Tick::new(start_tick.value()), ppqn).to_beat();
    let end = TickPosition::new(Tick::new(end_tick.value()), ppqn).to_beat();
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

/// Merged `(pos, value, segment interpolation)` of every region on
/// every lane of `target`, sorted by position — the one logical series
/// all projections read.
pub(crate) fn merged_series(
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

// ---------------------------------------------------------------------------
// viewport LOD (issue #134) — bounded read-only projection, never persisted
// ---------------------------------------------------------------------------

/// Bounded viewport query for [`automation_view`]. `start`/`end` are
/// the target's **scope coordinate** projected to `f64` — the display
/// axis a viewport reports (for `ProjectMusical` targets this equals
/// project beats). Stored positions keep their exact `BeatPoint`
/// truth; the `f64` window is display-only.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveQuery {
    /// Window start (inclusive).
    pub start: f64,
    /// Window end (inclusive — the last bin and the verbatim channel
    /// cover a point at exactly `end`).
    pub end: f64,
    /// Number of envelope bins (`> 0`), typically the plot pixel width.
    pub bins: usize,
    /// Maximum verbatim stored points the view may return.
    pub max_points: usize,
    /// Maximum classified features the view may emit.
    pub max_features: usize,
}

/// One display bin of the envelope: `min`/`max` are the evaluated
/// curve's **exact** extrema inside the bin (not an approximation —
/// Linear segments are affine so endpoints and stored points carry
/// every extremum). `NaN` when the series does not reach the bin.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewBin {
    /// Bin left edge (inclusive).
    pub start: f64,
    /// Bin right edge (exclusive, except the last bin which includes
    /// `end`).
    pub end: f64,
    /// Exact curve minimum inside the bin; `NaN` when uncovered.
    pub min: f64,
    /// Exact curve maximum inside the bin; `NaN` when uncovered.
    pub max: f64,
    /// Stored points whose position falls in this bin. `0` is legal
    /// for a covered bin — the tail after the last point is held by
    /// chase, distinguishing "held" from "missing" data.
    pub count: usize,
    /// Whether the evaluated curve reaches this bin at all.
    pub covered: bool,
}

/// A classified curve feature the LOD must never silently drop.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CurveFeatureKind {
    /// Step-interpolated discontinuity — the held value jumps at this
    /// exact position.
    StepEdge,
    /// Strict local extremum — the stored value reverses direction.
    Extremum,
    /// A region's `range.start`/`range.end` — a recorded coverage edge
    /// (AUT-02 sentinel anchors are captured through these).
    RegionBoundary,
}

/// A feature at an exact stored position. `pos` keeps `BeatPoint`
/// precision (display `f64` is never used for feature positions).
#[derive(Clone, Debug, PartialEq)]
pub struct CurveFeature {
    /// The feature's exact position (scope coordinate).
    pub pos: BeatPoint,
    /// The value at the feature (evaluated for boundaries; `NaN` when
    /// the series does not reach the position).
    pub value: f64,
    /// What the feature marks.
    pub kind: CurveFeatureKind,
}

/// Fidelity classification of an [`AutomationCurveView`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CurveQuality {
    /// Every in-view stored point was returned verbatim and every
    /// feature was emitted — full fidelity.
    Exact,
    /// The verbatim point channel was reduced but **no feature was
    /// dropped** — envelope and features still carry the full curve
    /// shape.
    Reduced,
    /// At least one feature was dropped — the view may lose curve
    /// shape. Reported honestly even when the rendered result looks
    /// smooth.
    Truncated,
}

/// The bounded LOD projection over one target's merged series —
/// a volatile DTO for display, never persisted and never a second
/// source of truth.
#[derive(Clone, Debug, PartialEq)]
pub struct AutomationCurveView {
    /// Exact per-bin min/max envelope, `bins` entries.
    pub bins: Vec<ViewBin>,
    /// Verbatim stored points inside the view (the explicit-anchor
    /// channel), position-ascending, reduced to `max_points`.
    pub points: Vec<AutomationPoint>,
    /// Classified features (`StepEdge`/`Extremum`/`RegionBoundary`),
    /// position-ascending, reduced to `max_features`.
    pub features: Vec<CurveFeature>,
    /// `(start, end)` extents of regions overlapping the view, in
    /// `f64` scope coordinates — the recorded coverage the curve is
    /// derived from.
    pub coverage: Vec<(f64, f64)>,
    /// Fidelity of this view.
    pub quality: CurveQuality,
    /// Stored points inside the view window (before reduction).
    pub source_points: usize,
    /// In-view stored points not returned in `points`.
    pub truncated_points: usize,
    /// Features not emitted in `features`.
    pub truncated_features: usize,
}

/// Exact `f64` evaluation at `x` over the `(pos_f64, value, interp)`
/// series — the last point at or before `x` holds (chase semantics);
/// `None` before the first point.
fn eval_at_f64(series: &[(f64, f64, Interpolation)], x: f64) -> Option<f64> {
    let idx = series.partition_point(|p| p.0 <= x).checked_sub(1)?;
    Some(series_piece(series, idx, x))
}

/// Left-limit evaluation approaching `x` — the piece active just
/// *before* `x` (a point at exactly `x` contributes nothing). `None`
/// when no point precedes `x`.
fn eval_before_f64(series: &[(f64, f64, Interpolation)], x: f64) -> Option<f64> {
    let idx = series.partition_point(|p| p.0 < x).checked_sub(1)?;
    Some(series_piece(series, idx, x))
}

/// The piecewise value at `x` under segment `idx`'s interpolation.
fn series_piece(series: &[(f64, f64, Interpolation)], idx: usize, x: f64) -> f64 {
    let (p0, v0, interp) = series[idx];
    match (interp, series.get(idx + 1)) {
        (Interpolation::Linear, Some(&(p1, v1, _))) => {
            v0 + (v1 - v0) * ((x - p0) / (p1 - p0)).clamp(0.0, 1.0)
        }
        _ => v0,
    }
}

/// The bounded viewport LOD projection of `target`'s merged series
/// (issue #134). Read-only: the project is never mutated, no undo
/// entry is created, and the persisted point columns stay the single
/// source of truth.
///
/// The envelope is exact at bin granularity; `StepEdge`/`Extremum`/
/// `RegionBoundary` features carry exact `BeatPoint` positions;
/// `points` returns stored points verbatim (the explicit-anchor
/// channel). Nothing is dropped silently: `quality` reports
/// `Exact`/`Reduced`/`Truncated`, and bins the series does not reach
/// carry `NaN` min/max with `covered = false`.
///
/// # Errors
/// [`Error::NotFound`] for an unknown target, [`Error::Validation`]
/// for a non-finite window, `end <= start`, or `bins == 0`.
pub fn automation_view(
    project: &Project,
    target: ParameterTargetId,
    query: &CurveQuery,
) -> Result<AutomationCurveView, Error> {
    if !project.control.targets.contains_key(&target) {
        return Err(Error::NotFound(format!("param {target}")));
    }
    if !query.start.is_finite() || !query.end.is_finite() || query.end <= query.start {
        return Err(Error::Validation(format!(
            "view window [{}, {}) is not finite/increasing",
            query.start, query.end
        )));
    }
    if query.bins == 0 {
        return Err(Error::Validation("view bins must be > 0".into()));
    }
    let merged = merged_series(project, target);
    let series: Vec<(f64, f64, Interpolation)> = merged
        .iter()
        .map(|(pos, v, interp)| (pos.to_f64(), *v, *interp))
        .collect();
    // In-view stored points: `pos` in the closed window `[start, end]`
    // (the last bin includes `end`).
    let in_view: Vec<usize> = (0..merged.len())
        .filter(|&i| {
            let x = series[i].0;
            query.start <= x && x <= query.end
        })
        .collect();
    let (features, coverage, truncated_features) =
        curve_view_features(project, target, &merged, &series, &in_view, query);
    // Verbatim point channel: feature-tagged points first, then plain
    // anchors in position order, capped at `max_points`.
    let flagged_pos: BTreeSet<BeatPoint> = features
        .iter()
        .filter(|f| f.kind != CurveFeatureKind::RegionBoundary)
        .map(|f| f.pos)
        .collect();
    let mut selected: Vec<usize> = in_view
        .iter()
        .copied()
        .filter(|&i| flagged_pos.contains(&merged[i].0))
        .collect();
    selected.extend(
        in_view
            .iter()
            .copied()
            .filter(|i| !flagged_pos.contains(&merged[*i].0))
            .take(query.max_points.saturating_sub(selected.len())),
    );
    selected.truncate(query.max_points);
    selected.sort_unstable();
    let points: Vec<AutomationPoint> = selected
        .iter()
        .map(|&i| AutomationPoint {
            pos: merged[i].0,
            value: merged[i].1,
        })
        .collect();
    let truncated_points = in_view.len() - selected.len();
    let bins_out = curve_view_bins(&series, &in_view, query);
    let quality = if truncated_features > 0 {
        CurveQuality::Truncated
    } else if truncated_points > 0 {
        CurveQuality::Reduced
    } else {
        CurveQuality::Exact
    };
    Ok(AutomationCurveView {
        bins: bins_out,
        points,
        features,
        coverage,
        quality,
        source_points: in_view.len(),
        truncated_points,
        truncated_features,
    })
}

/// Classifies the view's features and computes the region coverage —
/// returns `(features, coverage, truncated_features)`: shape-critical
/// `StepEdge`/`Extremum` first, `RegionBoundary` filling the rest of
/// `max_features`, everything dropped counted toward `Truncated`.
fn curve_view_features(
    project: &Project,
    target: ParameterTargetId,
    merged: &[(BeatPoint, f64, Interpolation)],
    series: &[(f64, f64, Interpolation)],
    in_view: &[usize],
    query: &CurveQuery,
) -> (Vec<CurveFeature>, Vec<(f64, f64)>, usize) {
    let mut critical: Vec<CurveFeature> = Vec::new();
    let mut boundaries: Vec<CurveFeature> = Vec::new();
    for &i in in_view {
        let (pos, v, _) = merged[i];
        // A step discontinuity arrives through the *previous* segment's
        // interpolation; stored values are exact, so an edge is an
        // exact change — not an epsilon difference.
        #[expect(clippy::float_cmp)]
        let step_edge = i > 0 && merged[i - 1].2 == Interpolation::Step && v != merged[i - 1].1;
        if step_edge {
            critical.push(CurveFeature {
                pos,
                value: v,
                kind: CurveFeatureKind::StepEdge,
            });
        }
        // Strict direction reversal — a peak or trough the LOD must
        // keep even when the verbatim channel is reduced.
        if i > 0 && i + 1 < merged.len() {
            let d1 = v - merged[i - 1].1;
            let d2 = merged[i + 1].1 - v;
            if d1 * d2 < 0.0 {
                critical.push(CurveFeature {
                    pos,
                    value: v,
                    kind: CurveFeatureKind::Extremum,
                });
            }
        }
    }
    let mut coverage = Vec::new();
    for lane in project
        .control
        .lanes
        .values()
        .filter(|l| l.target == target)
    {
        for region in &lane.regions {
            let rs = region.range.start().to_f64();
            let re = region.range.end().to_f64();
            if rs < query.end && re > query.start {
                coverage.push((rs, re));
                for (pos, at_start) in [(region.range.start(), true), (region.range.end(), false)] {
                    let x = pos.to_f64();
                    if query.start <= x && x <= query.end {
                        let value = if at_start {
                            eval_at_f64(series, x).unwrap_or(f64::NAN)
                        } else {
                            eval_before_f64(series, x).unwrap_or(f64::NAN)
                        };
                        boundaries.push(CurveFeature {
                            pos,
                            value,
                            kind: CurveFeatureKind::RegionBoundary,
                        });
                    }
                }
            }
        }
    }
    coverage.sort_by(|a, b| a.0.total_cmp(&b.0));
    let take_critical = critical.len().min(query.max_features);
    let take_boundary = boundaries
        .len()
        .min(query.max_features.saturating_sub(take_critical));
    let truncated_features = critical.len() - take_critical + (boundaries.len() - take_boundary);
    let mut features: Vec<CurveFeature> = critical
        .into_iter()
        .take(take_critical)
        .chain(boundaries.into_iter().take(take_boundary))
        .collect();
    features.sort_by(|a, b| a.pos.cmp(&b.pos).then(a.kind.cmp(&b.kind)));
    (features, coverage, truncated_features)
}

/// The exact per-bin envelope — half-open bins except the closed
/// last one; a covered bin feeds `eval_at(start)`, each stored point
/// inside, and the right-edge limit (`eval_before`, or `eval_at(end)`
/// on the last bin). Because `Linear` pieces are affine and `Step`
/// pieces are held, these candidates carry the true min/max — the
/// envelope is exact, not a sampling approximation.
#[allow(clippy::cast_precision_loss)] // same documented lossy axis
fn curve_view_bins(
    series: &[(f64, f64, Interpolation)],
    in_view: &[usize],
    query: &CurveQuery,
) -> Vec<ViewBin> {
    let first_x = series.first().map_or(f64::INFINITY, |p| p.0);
    let width = (query.end - query.start) / query.bins as f64;
    let mut bins_out = Vec::with_capacity(query.bins);
    for i in 0..query.bins {
        let s = query.start + width * i as f64;
        let last = i + 1 == query.bins;
        let e = if last {
            query.end
        } else {
            query.start + width * (i + 1) as f64
        };
        // Half-open bins cover `x >= first` iff `e > first`; the last
        // bin is closed and also covers a point at exactly `end`.
        let covered = if last { e >= first_x } else { e > first_x };
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let mut count = 0usize;
        if covered {
            let mut feed = |v: Option<f64>| {
                if let Some(v) = v {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            };
            if s >= first_x {
                feed(eval_at_f64(series, s));
            }
            for &j in in_view {
                let x = series[j].0;
                if s <= x && (x < e || (last && x <= e)) {
                    feed(Some(series[j].1));
                    count += 1;
                }
            }
            if last {
                feed(eval_at_f64(series, e));
            } else {
                feed(eval_before_f64(series, e));
            }
        }
        bins_out.push(ViewBin {
            start: s,
            end: e,
            min: if covered { lo } else { f64::NAN },
            max: if covered { hi } else { f64::NAN },
            count,
            covered,
        });
    }
    bins_out
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
        let local = resolve_scope_pos(project, target.owner, target.time_scope, pos)?;
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
/// over targets plus prepared per-target data (merged series, resolved
/// assignments). Preparation/validation happens at compile, off the RT
/// path; RT-side evaluation walks `order` — bounded, fixed-order, no
/// cycles — and only touches prepared data and scope resolution.
#[derive(Clone, Debug, PartialEq)]
pub struct EvalPlan {
    /// Targets in dependency order (depth drivers before dependents).
    pub order: Vec<ParameterTargetId>,
    /// Prepared evaluation data parallel to `order`.
    prepared: Vec<PreparedTarget>,
}

/// Per-target evaluation data frozen at [`ControlGraph::compile`] —
/// `eval_rt` never walks `BTreeMap`s for structure.
#[derive(Clone, Debug, PartialEq)]
struct PreparedTarget {
    /// Owning entity and time coordinate — resolved live per call so
    /// `MoveClip`/`AddNote` between compiles still evaluate against
    /// current placement.
    owner: ControlOwner,
    /// Declared coordinate of the frozen series positions.
    time_scope: AutomationTimeScope,
    /// Normalized base value.
    base: f64,
    /// Value domain (quantization).
    kind: ValueKind,
    /// Merged sorted `(pos, value, interp)` series across all lanes.
    series: Box<[(BeatPoint, f64, Interpolation)]>,
    /// Enabled assignments, depth sources pre-resolved to order indices.
    assignments: Box<[PreparedAssignment]>,
}

/// An enabled assignment with its depth driver resolved to an `order`
/// index (topo guarantees the driver index precedes the dependent's).
#[derive(Clone, Debug, PartialEq)]
struct PreparedAssignment {
    /// Signal source definition (cloned — `SourceKind::value` is pure).
    source: SourceKind,
    /// Depth scale.
    depth: f64,
    /// Uni/bipolar.
    polarity: Polarity,
    /// Raw→transformed map.
    transform: Transform,
    /// Driver index in `EvalPlan::order`, if depth-modulated.
    depth_source: Option<usize>,
}

/// Caller-owned scratch for [`EvalPlan::eval_rt`] — allocated once,
/// reused across control-rate calls so the RT path never allocates.
#[derive(Clone, Debug, Default)]
pub struct EvalScratch {
    values: Vec<f64>,
    computed: Vec<bool>,
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
    /// **Snapshot contract:** the plan freezes target bases, kinds,
    /// merged automation series, and assignment data as of this call.
    /// Control mutations (`WriteAutomationRegion`, `SetParameterBase`,
    /// assignment/lane edits) are **not** visible to `eval_rt` until
    /// the caller recompiles and reinstalls — callers must recompile
    /// on each relevant project revision (scope *placement* — clip
    /// anchor/extent — is resolved live so `MoveClip` stays correct
    /// without recompile). [`EvalPlan::is_stale`] detects the
    /// divergence.
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
        let order_index: BTreeMap<ParameterTargetId, usize> =
            order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let prepared = order
            .iter()
            .map(|id| {
                let target = &project.control.targets[id];
                let assignments = project
                    .control
                    .assignments
                    .values()
                    .filter(|a| a.target == *id && a.enabled)
                    .map(|a| PreparedAssignment {
                        source: project.control.sources[&a.source].kind.clone(),
                        depth: a.depth,
                        polarity: a.polarity,
                        transform: a.transform,
                        depth_source: a.depth_source.map(|ds| order_index[&ds]),
                    })
                    .collect();
                PreparedTarget {
                    owner: target.owner,
                    time_scope: target.time_scope,
                    base: target.base,
                    kind: target.kind,
                    series: merged_series(project, *id).into_boxed_slice(),
                    assignments,
                }
            })
            .collect();
        Ok(EvalPlan { order, prepared })
    }
}

impl EvalPlan {
    /// Whether the inputs this plan froze at compile have since
    /// changed — `true` when a recompile of `project` would produce a
    /// different plan (or fail outright).
    ///
    /// The check compares exactly what [`ControlGraph::compile`]
    /// freezes — target bases, kinds and owner scopes, merged
    /// automation series, enabled assignments and their resolved
    /// depth sources — by recompiling and comparing. A control
    /// mutation (`WriteAutomationRegion`, `SetParameterBase`,
    /// assignment/lane edits, target add/remove) reports stale;
    /// *placement* changes (`MoveClip`, `SetClipExtent`, note add or
    /// move) do not — scope resolution stays live per the snapshot
    /// contract, so those plans are still correct.
    ///
    /// Recompile-compare is O(model) and allocates — a non-RT check
    /// the caller runs at its own cadence (once per control tick or
    /// after each applied command), never from `eval_rt`'s
    /// allocation-free path.
    #[must_use]
    pub fn is_stale(&self, project: &Project) -> bool {
        // A control model that no longer compiles can never equal this
        // plan — it is definitionally out of date.
        ControlGraph::compile(project).map_or(true, |plan| plan != *self)
    }

    /// A caller-owned scratch sized for this plan — allocate once off
    /// the RT path, reuse across control-rate evaluations.
    #[must_use]
    pub fn scratch(&self) -> EvalScratch {
        EvalScratch {
            values: vec![0.0; self.order.len()],
            computed: vec![false; self.order.len()],
        }
    }

    /// Allocation-free control-rate evaluation of `target`'s
    /// dependency chain at `pos`. `manual` is the pre-modulation
    /// reference for `target` only (drivers never see manual input —
    /// same rule as [`evaluate_control`]). Returns `host_normalized`.
    ///
    /// The success path performs no heap allocation: series lookup is
    /// binary search, drivers resolve through `scratch`, and scope
    /// resolution only reads project maps. Error paths may allocate
    /// (message formatting) — the RT contract covers the success path.
    ///
    /// # Errors
    /// Same surface as [`evaluate_control`], plus
    /// [`Error::NotFound`] when `target` is not in the plan and
    /// [`Error::Validation`] when `scratch` is undersized.
    pub fn eval_rt(
        &self,
        project: &Project,
        target: ParameterTargetId,
        pos: BeatPoint,
        manual: Option<f64>,
        scratch: &mut EvalScratch,
    ) -> Result<f64, Error> {
        let idx = self
            .order
            .iter()
            .position(|id| *id == target)
            .ok_or_else(|| Error::NotFound(format!("param {target}")))?;
        if scratch.values.len() < self.order.len() || scratch.computed.len() < self.order.len() {
            return Err(Error::Validation("eval_rt scratch too small".into()));
        }
        scratch.computed.fill(false);
        self.eval_node(project, idx, pos, manual, scratch)
    }

    fn eval_node(
        &self,
        project: &Project,
        idx: usize,
        pos: BeatPoint,
        manual: Option<f64>,
        scratch: &mut EvalScratch,
    ) -> Result<f64, Error> {
        if scratch.computed[idx] {
            return Ok(scratch.values[idx]);
        }
        let t = &self.prepared[idx];
        let local = resolve_scope_pos(project, t.owner, t.time_scope, pos)?;
        let automation = prepared_lane_value(&t.series, local);
        let reference = match (manual, automation) {
            (Some(m), _) => m,
            (None, Some(a)) => a,
            (None, None) => t.base,
        };
        let mut raw_total = reference;
        for a in &t.assignments {
            let raw = a.source.value(local);
            let mut transformed = a.transform.apply(raw);
            if a.polarity == Polarity::Unipolar {
                transformed = transformed.midpoint(1.0);
            }
            let depth_eff = match a.depth_source {
                Some(ds) => a.depth * self.eval_node(project, ds, pos, None, scratch)?,
                None => a.depth,
            };
            raw_total += depth_eff * transformed;
        }
        let host_normalized = quantize_value(t.kind, raw_total.clamp(0.0, 1.0));
        scratch.values[idx] = host_normalized;
        scratch.computed[idx] = true;
        Ok(host_normalized)
    }
}

/// Allocation-free automation lookup over a prepared merged series —
/// the `lane_value` semantics (chase: latest point at or before `pos`;
/// before the first point `None` falls back to base) without the
/// project `BTreeMap` scan.
fn prepared_lane_value(series: &[(BeatPoint, f64, Interpolation)], pos: BeatPoint) -> Option<f64> {
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
