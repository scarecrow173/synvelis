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

/// Re-projects one stored musical tick through exact rational beats.
///
/// `what` names the owning value for rejection messages. Under
/// [`PpqnConversionPolicy::Exact`] a value the target grid cannot
/// represent exactly is an explicit [`Error::Validation`] rejection;
/// under `Round` the declared ruviform rounding is applied.
fn reproject_tick(
    tick: i64,
    old: Ppqn,
    new: Ppqn,
    policy: PpqnConversionPolicy,
    what: &str,
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
                    "{what}: tick {tick} is not exactly representable at ppqn {new}"
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

/// Re-projects a [`ProjectRange`] when it lives in the musical domain.
/// Media ranges keep their declared time base untouched.
fn reproject_project_range(
    range: &mut ProjectRange,
    old: Ppqn,
    new: Ppqn,
    policy: PpqnConversionPolicy,
) -> Result<(), Error> {
    let ProjectRange::Musical(musical) = range else {
        return Ok(());
    };
    let start = reproject_tick(
        musical.start().value(),
        old,
        new,
        policy,
        "comp range start",
    )?;
    let end = reproject_tick(musical.end().value(), old, new, policy, "comp range end")?;
    *range = ProjectRange::Musical(MusicalRange::try_new(
        MusicalTick::new(start),
        MusicalTick::new(end),
    )?);
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

    for event in &mut project.tempo_map.tempo_events {
        event.tick = reproject_tick(event.tick, old, new, policy, "tempo event")?;
    }
    for event in &mut project.tempo_map.signature_events {
        event.tick = reproject_tick(event.tick, old, new, policy, "signature event")?;
    }

    for clip in project.clips.values_mut() {
        if let TimeAnchor::Musical(tick) = &mut clip.placement.anchor {
            *tick = MusicalTick::new(reproject_tick(
                tick.value(),
                old,
                new,
                policy,
                "clip anchor",
            )?);
        }
        if let ClipExtent::Musical(duration) = &mut clip.placement.extent {
            let value = reproject_tick(duration.value(), old, new, policy, "clip extent")?;
            *duration = MusicalDuration::new(value)?;
        }
    }

    for group in project.comp.take_groups.values_mut() {
        reproject_project_range(&mut group.target_range, old, new, policy)?;
    }
    for region in project.comp.regions.values_mut() {
        reproject_project_range(&mut region.project_range, old, new, policy)?;
    }
    for transition in project.comp.transitions.values_mut() {
        reproject_project_range(&mut transition.overlap_range, old, new, policy)?;
    }

    project.ppqn = ProjectPpqn(new_ppqn);
    project.tempo_map.ppqn = new_ppqn;
    project.tempo_map.revision += 1;
    Ok(())
}
