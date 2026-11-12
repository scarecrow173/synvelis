//! Configurable project PPQN — the `SetProjectPpqn` re-projection
//! (issue #125 / TIM-01).
//!
//! Project PPQN is only the *projection context* for stored
//! musical-tick values: every persisted position is lifted through
//! exact rational beats ([`TickPosition::to_beat`]) and re-projected
//! onto the new grid under an explicit [`PpqnConversionPolicy`] —
//! `project.ppqn` is never simply overwritten. Absolute media anchors,
//! source-native MIDI ticks/divisions/raw bytes, recording
//! session/source frames, and automation `BeatPoint`s are outside the
//! musical tick domain and are never touched.
//!
//! The selectable range is `1..=32767` (the SMF 15-bit PPQ bound);
//! ruviform's [`Ppqn`] itself accepts `1..=u16::MAX`.

use std::fmt;

use ruviform_media_core::{Ppqn, Rounding, Tick, TickPosition};

use crate::{
    error::Error,
    placement::{ClipExtent, TimeAnchor},
    project::{Project, ProjectRange},
    range::MusicalRange,
    tempo::ProjectPpqn,
    time::{MusicalDuration, MusicalTick},
};

/// Smallest selectable project PPQN.
pub const PPQN_MIN: u32 = 1;
/// Largest selectable project PPQN — the SMF 15-bit PPQ bound.
pub const PPQN_MAX: u32 = 32_767;

/// Explicit conversion policy for
/// [`ProjectOp::SetProjectPpqn`](crate::ops::ProjectOp::SetProjectPpqn).
///
/// The policy is required command input — a PPQN change can never
/// silently quantize a stored musical position.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PpqnConversionPolicy {
    /// Reject the command when any stored musical value is not
    /// exactly representable on the new PPQN grid.
    Exact,
    /// Re-project under the declared ruviform [`Rounding`] — the
    /// quantization is explicit, bounded, and recorded in the
    /// command, never silent.
    Round(Rounding),
}

/// `true` when `value` is inside the selectable `1..=32767` range.
#[must_use]
pub fn ppqn_in_range(value: u32) -> bool {
    (PPQN_MIN..=PPQN_MAX).contains(&value)
}

/// Validates `value` against the selectable range and returns the
/// ruviform resolution.
fn ruviform_ppqn(value: u32) -> Result<Ppqn, Error> {
    if !ppqn_in_range(value) {
        return Err(Error::Validation(format!(
            "ppqn {value} outside selectable range {PPQN_MIN}..={PPQN_MAX}"
        )));
    }
    let raw = u16::try_from(value).map_err(|_| Error::Overflow)?;
    Ppqn::new(raw).map_err(|e| Error::Validation(format!("ppqn: {e:?}")))
}

/// Identifies one persisted musical-tick value the PPQN
/// re-projection touches — the shared vocabulary of the mutating
/// walk ([`reproject_ppqn_with`]) and the [`preview_ppqn`] dry-run
/// so both enumerate exactly the same sites.
#[derive(Clone, Debug)]
pub(crate) struct PpqnSite {
    /// What owns the value — `tempo_event`, `signature_event`,
    /// `clip_anchor`, `clip_extent`, `take_group_range`,
    /// `comp_region_range`, or `comp_transition_range`.
    pub kind: &'static str,
    /// Instance discriminator — the entity's tagged id, or the
    /// tempo/signature event's index in its list.
    pub instance: String,
    /// `"start"`/`"end"` for ranged values; `None` for single ticks.
    pub part: Option<&'static str>,
}

impl PpqnSite {
    /// A single-tick site (no range part).
    fn single(kind: &'static str, instance: impl Into<String>) -> Self {
        Self {
            kind,
            instance: instance.into(),
            part: None,
        }
    }

    /// The `part` endpoint of a ranged site sharing `self`'s
    /// identity.
    fn endpoint(&self, part: &'static str) -> Self {
        Self {
            part: Some(part),
            ..self.clone()
        }
    }
}

impl fmt::Display for PpqnSite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            "tempo_event" => "tempo event",
            "signature_event" => "signature event",
            "clip_anchor" => "clip anchor",
            "clip_extent" => "clip extent",
            "take_group_range" => "take group range",
            "comp_region_range" => "comp region range",
            "comp_transition_range" => "comp transition range",
            other => other,
        };
        write!(f, "{kind} {}", self.instance)?;
        if let Some(part) = self.part {
            write!(f, " {part}")?;
        }
        Ok(())
    }
}

/// Per-site conversion callback — receives the site identity and
/// the stored tick on the old grid and returns the tick to store on
/// the new grid, or an error to abort the walk.
type TickConversion<'a> = dyn FnMut(&PpqnSite, i64) -> Result<i64, Error> + 'a;

/// Re-projects one stored musical tick through exact rational beats.
///
/// `site` names the owning value for rejection messages. Under
/// [`PpqnConversionPolicy::Exact`] a value the target grid cannot
/// represent exactly is an explicit [`Error::Validation`] rejection;
/// under `Round` the declared ruviform rounding is applied.
fn reproject_tick(
    site: &PpqnSite,
    tick: i64,
    old: Ppqn,
    new: Ppqn,
    policy: PpqnConversionPolicy,
) -> Result<i64, Error> {
    let beat = TickPosition::new(Tick::new(tick), old).to_beat();
    let projected = match policy {
        PpqnConversionPolicy::Exact => {
            // Floor and Ceil agree iff the beat lands exactly on the
            // new grid; a one-tick gap means inexact representation.
            let floor =
                TickPosition::from_beat(beat, new, Rounding::Floor).map_err(|_| Error::Overflow)?;
            let ceil =
                TickPosition::from_beat(beat, new, Rounding::Ceil).map_err(|_| Error::Overflow)?;
            if floor.tick() != ceil.tick() {
                return Err(Error::Validation(format!(
                    "{site}: tick {tick} is not exactly representable at ppqn {new}"
                )));
            }
            floor
        }
        PpqnConversionPolicy::Round(rounding) => {
            TickPosition::from_beat(beat, new, rounding).map_err(|_| Error::Overflow)?
        }
    };
    Ok(projected.tick().value())
}

/// Re-projects a [`ProjectRange`] when it lives in the musical
/// domain through `convert`; `site` names the owning entity.
/// Media ranges keep their declared time base untouched.
fn reproject_project_range(
    range: &mut ProjectRange,
    site: &PpqnSite,
    convert: &mut TickConversion<'_>,
) -> Result<(), Error> {
    let ProjectRange::Musical(musical) = range else {
        return Ok(());
    };
    let start = convert(&site.endpoint("start"), musical.start().value())?;
    let end = convert(&site.endpoint("end"), musical.end().value())?;
    *range = ProjectRange::Musical(MusicalRange::try_new(
        MusicalTick::new(start),
        MusicalTick::new(end),
    )?);
    Ok(())
}

/// Walks every persisted musical-tick value in `project` through
/// `convert` — the single site enumeration shared by
/// [`reproject_ppqn`] and [`preview_ppqn`] so the mutating command
/// and the dry-run report can never disagree about what converts.
///
/// Only the musical tick domain is visited: tempo/signature events,
/// musical clip anchors/extents, and musical comp ranges. Absolute
/// media anchors, source-native MIDI data, recording frames, and
/// automation `BeatPoint`s are outside this walk by construction.
fn reproject_ppqn_with(
    project: &mut Project,
    convert: &mut TickConversion<'_>,
) -> Result<(), Error> {
    for (i, event) in project.tempo_map.tempo_events.iter_mut().enumerate() {
        event.tick = convert(&PpqnSite::single("tempo_event", i.to_string()), event.tick)?;
    }
    for (i, event) in project.tempo_map.signature_events.iter_mut().enumerate() {
        event.tick = convert(
            &PpqnSite::single("signature_event", i.to_string()),
            event.tick,
        )?;
    }

    for clip in project.clips.values_mut() {
        let instance = clip.id.to_tagged();
        if let TimeAnchor::Musical(tick) = &mut clip.placement.anchor {
            *tick = MusicalTick::new(convert(
                &PpqnSite::single("clip_anchor", instance.clone()),
                tick.value(),
            )?);
        }
        if let ClipExtent::Musical(duration) = &mut clip.placement.extent {
            let value = convert(
                &PpqnSite::single("clip_extent", instance.clone()),
                duration.value(),
            )?;
            *duration = MusicalDuration::new(value)?;
        }
    }

    for (id, group) in &mut project.comp.take_groups {
        reproject_project_range(
            &mut group.target_range,
            &PpqnSite::single("take_group_range", id.to_tagged()),
            convert,
        )?;
    }
    for (id, region) in &mut project.comp.regions {
        reproject_project_range(
            &mut region.project_range,
            &PpqnSite::single("comp_region_range", id.to_tagged()),
            convert,
        )?;
    }
    for (id, transition) in &mut project.comp.transitions {
        reproject_project_range(
            &mut transition.overlap_range,
            &PpqnSite::single("comp_transition_range", id.to_tagged()),
            convert,
        )?;
    }
    Ok(())
}

/// Re-projects every persisted musical-tick value in `project` from
/// `old_ppqn` onto `new_ppqn` under `policy`, then updates both PPQN
/// fields and bumps the tempo-map revision (its persisted event ticks
/// changed grids, so `TempoMapRef`s must not keep pointing at it).
///
/// `old_ppqn == new_ppqn` is an identity no-op: no field changes and
/// no revision churn.
///
/// # Errors
/// [`Error::Validation`] on an out-of-range PPQN, an inexact value
/// under [`PpqnConversionPolicy::Exact`], or a re-projected range
/// that inverts; [`Error::Overflow`]/[`Error::Inverted`] propagated
/// from the checked conversions.
pub(crate) fn reproject_ppqn(
    project: &mut Project,
    old_ppqn: u32,
    new_ppqn: u32,
    policy: PpqnConversionPolicy,
) -> Result<(), Error> {
    if old_ppqn == new_ppqn {
        return Ok(());
    }
    let old = ruviform_ppqn(old_ppqn)?;
    let new = ruviform_ppqn(new_ppqn)?;

    reproject_ppqn_with(project, &mut |site, tick| {
        reproject_tick(site, tick, old, new, policy)
    })?;

    project.ppqn = ProjectPpqn(new_ppqn);
    project.tempo_map.ppqn = new_ppqn;
    project.tempo_map.revision += 1;
    Ok(())
}

/// One persisted musical-tick value a PPQN change would
/// re-project, as flagged by [`PpqnChangePreview`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PpqnPreviewSite {
    /// What owns the value — `tempo_event`, `signature_event`,
    /// `clip_anchor`, `clip_extent`, `take_group_range`,
    /// `comp_region_range`, or `comp_transition_range`.
    pub kind: &'static str,
    /// Instance discriminator — the entity's tagged id, or the
    /// tempo/signature event's index in its list.
    pub instance: String,
    /// `"start"`/`"end"` for ranged values; `None` for single ticks.
    pub part: Option<&'static str>,
    /// The stored tick on the current PPQN grid.
    pub tick: i64,
    /// The tick the value becomes under the requested policy —
    /// `None` when the value is inexact under
    /// [`PpqnConversionPolicy::Exact`] or overflows the conversion.
    pub projected: Option<i64>,
}

/// Non-destructive dry-run report for a project PPQN change —
/// what [`ProjectOp::SetProjectPpqn`](crate::ops::ProjectOp::SetProjectPpqn)
/// would touch, produced by [`preview_ppqn`].
#[derive(Clone, Debug)]
pub struct PpqnChangePreview {
    /// The live PPQN the preview was computed against.
    pub old_ppqn: u32,
    /// The requested target PPQN.
    pub new_ppqn: u32,
    /// The requested conversion policy.
    pub policy: PpqnConversionPolicy,
    /// How many persisted musical-tick values the change
    /// re-projects — tempo/signature events, musical clip
    /// anchors/extents, and musical comp ranges.
    pub site_count: usize,
    /// Sites not exactly representable on the new grid —
    /// [`PpqnConversionPolicy::Exact`] rejects them,
    /// [`PpqnConversionPolicy::Round`] quantizes them explicitly.
    pub inexact: Vec<PpqnPreviewSite>,
    /// Sites whose re-projection overflows the checked `i64`
    /// conversion — any overflow fails every policy.
    pub overflow_sites: Vec<PpqnPreviewSite>,
    /// `true` when `SetProjectPpqn` under
    /// [`PpqnConversionPolicy::Exact`] would succeed — no inexact
    /// sites and no overflow.
    pub exact_ok: bool,
    /// `true` when a `Round` policy has no overflow to reject —
    /// the declared rounding is applied explicitly at apply time.
    /// [`Self::applies`] is the authoritative answer for the
    /// requested policy.
    pub round_ok: bool,
    /// `true` when the requested `policy` dry-run on a scratch
    /// clone succeeded — the same code path the command's
    /// validation runs.
    pub applies: bool,
    /// The rejection the requested-policy dry run produced, if any.
    pub rejection: Option<String>,
}

/// Non-destructive dry-run of a PPQN change to `new_ppqn` under
/// `policy` — the "変更前 Preview" half of the TIM-01 contract
/// (issue #125).
///
/// `project` is never mutated. An analytical pass walks the same
/// site enumeration [`reproject_ppqn`] uses — with an identity
/// conversion, so nothing changes — and classifies every persisted
/// musical-tick value as exact, inexact, or overflowing on the new
/// grid. A second pass dry-runs the real [`reproject_ppqn`] on a
/// scratch clone under the requested `policy` for the authoritative
/// `applies`/`rejection`, so the report can never drift from what
/// the command would do.
///
/// # Errors
/// [`Error::Validation`] when `new_ppqn` (or the live
/// `project.ppqn`) is outside the selectable `1..=32767` range —
/// the same resolution [`reproject_ppqn`] performs.
pub fn preview_ppqn(
    project: &Project,
    new_ppqn: u32,
    policy: PpqnConversionPolicy,
) -> Result<PpqnChangePreview, Error> {
    let old_ppqn = project.ppqn.0;
    let old = ruviform_ppqn(old_ppqn)?;
    let new = ruviform_ppqn(new_ppqn)?;

    let mut site_count = 0_usize;
    let mut inexact = Vec::new();
    let mut overflow_sites = Vec::new();
    {
        let mut scratch = project.clone();
        reproject_ppqn_with(&mut scratch, &mut |site, tick| {
            site_count += 1;
            let beat = TickPosition::new(Tick::new(tick), old).to_beat();
            let floor = TickPosition::from_beat(beat, new, Rounding::Floor);
            let ceil = TickPosition::from_beat(beat, new, Rounding::Ceil);
            let flagged = |projected: Option<i64>| PpqnPreviewSite {
                kind: site.kind,
                instance: site.instance.clone(),
                part: site.part,
                tick,
                projected,
            };
            match (floor, ceil) {
                (Ok(f), Ok(c)) if f.tick() == c.tick() => {}
                (Ok(_), Ok(_)) => {
                    let projected = match policy {
                        PpqnConversionPolicy::Exact => None,
                        PpqnConversionPolicy::Round(rounding) => {
                            TickPosition::from_beat(beat, new, rounding)
                                .ok()
                                .map(|p| p.tick().value())
                        }
                    };
                    inexact.push(flagged(projected));
                }
                _ => overflow_sites.push(flagged(None)),
            }
            Ok(tick)
        })?;
    }

    let mut verify = project.clone();
    let (applies, rejection) = match reproject_ppqn(&mut verify, old_ppqn, new_ppqn, policy) {
        Ok(()) => (true, None),
        Err(error) => (false, Some(error.to_string())),
    };

    let exact_ok = inexact.is_empty() && overflow_sites.is_empty();
    let round_ok = overflow_sites.is_empty();
    Ok(PpqnChangePreview {
        old_ppqn,
        new_ppqn,
        policy,
        site_count,
        inexact,
        overflow_sites,
        exact_ok,
        round_ok,
        applies,
        rejection,
    })
}
