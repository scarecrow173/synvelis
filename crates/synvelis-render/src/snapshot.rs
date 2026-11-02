//! The render input snapshot: a frozen copy of everything the export
//! reads, taken once before any render work begins. Live edits after
//! the freeze cannot reach the render — the snapshot holds a cloned
//! [`Project`], and the render instance holds no processor instance
//! shared with any live driver.

use std::collections::BTreeMap;
use std::path::PathBuf;

use synvelis_project::{Project, ProjectController, ProjectRevision, SourceId};

/// Everything identifying *which* project state an export rendered —
/// recorded on the receipt so the caller can prove the render input
/// was frozen and distinct from Live/Saved/Active-Plan revisions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotIds {
    /// Live project revision at freeze time.
    pub live_revision: ProjectRevision,
    /// Last saved revision at freeze time (`None` = never saved).
    pub saved_revision: Option<ProjectRevision>,
    /// `TempoMapData::revision` the render's tick↔media mapping used.
    pub tempo_revision: u64,
    /// `source_id → content_digest` for every registered source — the
    /// Source/Asset Content Version the render pinned.
    pub source_digests: BTreeMap<SourceId, String>,
}

/// The frozen render input. `project` is a value copy: mutations of
/// the live aggregate afterwards are unobservable by the render.
pub struct RenderSnapshot {
    /// Cloned project aggregate — the frozen clips/sources/assets/
    /// tempo map the render reads.
    pub project: Project,
    /// Revision identities captured alongside the clone.
    pub ids: SnapshotIds,
    /// Media store root (`blobs/<digest16>` lives beneath it).
    pub media_root: PathBuf,
}

impl RenderSnapshot {
    /// Freezes the controller's live project: clones the aggregate and
    /// records live/saved revisions, tempo revision and every source's
    /// content digest. Takes `&ProjectController` — the freeze can
    /// never mutate the project or push an operation onto undo/redo.
    #[must_use]
    pub fn freeze(controller: &ProjectController, media_root: impl Into<PathBuf>) -> Self {
        let project = controller.live().clone();
        Self {
            ids: SnapshotIds {
                live_revision: controller.live_revision(),
                saved_revision: controller.saved().map(|s| s.revision),
                tempo_revision: project.tempo_map.revision,
                source_digests: project
                    .sources
                    .iter()
                    .map(|(id, s)| (*id, s.content_digest.clone()))
                    .collect(),
            },
            project,
            media_root: media_root.into(),
        }
    }
}
