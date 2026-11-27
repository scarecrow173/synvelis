//! Project-owned musical transport compilation for production plans.
//!
//! This module is the single Synvelis consumer boundary that turns the
//! persisted project tempo snapshot into the complete plan-owned source
//! ruviform expects: tempo map, tempo revision, and project PPQN. It does
//! not synthesize missing state and never substitutes the conventional
//! 960 PPQN for absent or invalid project data.

use std::fmt;
use std::sync::Arc;

use ruviform_media_core::{Ppqn, TempoMap, TempoMapRevision, TransportPlanBuilder};
use synvelis_project::tempo::TempoMapData;
use synvelis_project::{Project, ppqn_in_range};

/// Why a project snapshot cannot supply musical transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MusicalTransportError {
    /// `Project::ppqn` is outside the selectable project PPQN range.
    ProjectPpqnOutOfRange {
        /// The invalid project PPQN value.
        ppqn: u32,
    },
    /// The top-level project PPQN and the tempo snapshot PPQN disagree.
    PpqnMismatch {
        /// Top-level project PPQN.
        project_ppqn: u32,
        /// PPQN persisted in the tempo snapshot.
        tempo_map_ppqn: u32,
    },
    /// The persisted tempo events failed `TempoMapBuilder` validation.
    TempoMapCompile {
        /// Rendered project-layer compile error.
        error: String,
    },
}

impl fmt::Display for MusicalTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectPpqnOutOfRange { ppqn } => {
                write!(f, "project ppqn {ppqn} is outside the selectable range")
            }
            Self::PpqnMismatch {
                project_ppqn,
                tempo_map_ppqn,
            } => write!(
                f,
                "project ppqn {project_ppqn} differs from tempo map ppqn {tempo_map_ppqn}"
            ),
            Self::TempoMapCompile { error } => write!(f, "tempo map compile failed: {error}"),
        }
    }
}

impl std::error::Error for MusicalTransportError {}

/// A complete, validated musical transport source for production plans.
///
/// The persisted snapshot is retained as the exact change key. Revision
/// numbers alone are not an identity because undo/redo can restore equal
/// revision values for different snapshot contents.
#[derive(Clone, Debug)]
pub struct PlanMusicalTransport {
    data: TempoMapData,
    project_ppqn: u32,
    map: Arc<TempoMap>,
    ppqn: Ppqn,
}

impl PlanMusicalTransport {
    /// Tempo revision supplied to every plan built from this source.
    #[must_use]
    pub fn tempo_revision(&self) -> TempoMapRevision {
        TempoMapRevision::new(self.data.revision)
    }

    /// Project PPQN supplied to every plan built from this source.
    #[must_use]
    pub const fn ppqn(&self) -> Ppqn {
        self.ppqn
    }

    /// The unvalidated project PPQN fact this source was compiled from.
    #[must_use]
    pub const fn project_ppqn(&self) -> u32 {
        self.project_ppqn
    }

    /// Whether two sources represent the same persisted project basis.
    #[must_use]
    pub fn same_project_basis(&self, other: &Self) -> bool {
        self.project_ppqn == other.project_ppqn && self.data == other.data
    }

    /// Applies this complete source to one plan builder.
    pub(crate) fn apply<P>(&self, builder: TransportPlanBuilder<P>) -> TransportPlanBuilder<P> {
        builder.with_musical_transport(Arc::clone(&self.map), self.tempo_revision(), self.ppqn)
    }

    /// Whether a compiled plan carries this exact source — revision,
    /// project PPQN, and the same compiled tempo-map allocation rather
    /// than merely equal-looking scalar fields.
    #[must_use]
    pub fn plan_carries<P>(&self, plan: &ruviform_media_core::TransportPlan<P>) -> bool {
        plan.tempo_revision == self.tempo_revision()
            && plan.musical_ppqn == Some(self.ppqn)
            && plan
                .tempo_map
                .as_ref()
                .is_some_and(|map| Arc::ptr_eq(map, &self.map))
    }
}

/// Compiles the current project snapshot into a complete plan source.
///
/// A missing or invalid half is a typed error. Callers must leave the
/// plan's musical fields empty for that state rather than inventing a
/// tempo map or fixed PPQN.
///
/// # Errors
/// [`MusicalTransportError`] when the project PPQN is out of range, the
/// two persisted PPQN fields disagree, or the tempo snapshot does not
/// compile.
pub fn compile_project_musical_transport(
    project: &Project,
) -> Result<PlanMusicalTransport, MusicalTransportError> {
    let project_ppqn = project.ppqn.0;
    if !ppqn_in_range(project_ppqn) {
        return Err(MusicalTransportError::ProjectPpqnOutOfRange { ppqn: project_ppqn });
    }
    let tempo_map_ppqn = project.tempo_map.ppqn;
    if tempo_map_ppqn != project_ppqn {
        return Err(MusicalTransportError::PpqnMismatch {
            project_ppqn,
            tempo_map_ppqn,
        });
    }
    let raw = u16::try_from(project_ppqn)
        .map_err(|_| MusicalTransportError::ProjectPpqnOutOfRange { ppqn: project_ppqn })?;
    let ppqn = Ppqn::new(raw)
        .map_err(|_| MusicalTransportError::ProjectPpqnOutOfRange { ppqn: project_ppqn })?;
    let map =
        project
            .tempo_map
            .compile()
            .map_err(|error| MusicalTransportError::TempoMapCompile {
                error: error.to_string(),
            })?;
    Ok(PlanMusicalTransport {
        data: project.tempo_map.clone(),
        project_ppqn,
        map: Arc::new(map),
        ppqn,
    })
}
