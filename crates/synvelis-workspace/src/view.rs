//! View types, descriptors, registry and the headless view factory.

use std::collections::BTreeMap;

use ruviform_ui::dock::ViewInstanceId;
use serde::{Deserialize, Serialize};
use synvelis_project::ProjectRevision;

use crate::error::WorkspaceError;

/// The DAW surface kinds the workspace can host.
///
/// Rendering is out of scope for this crate — each kind is produced by
/// the headless [`ViewFactory`] as a stub that records render calls so
/// state delivery can be observed in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewType {
    /// Timeline arrangement surface.
    Arrangement,
    /// Mixer / channel strip surface.
    Mixer,
    /// Context inspector surface.
    Inspector,
    /// Signal-flow routing surface.
    SignalFlow,
    /// Plugin dock surface — vocabulary only (SV177): the native
    /// plugin host/editor implementation is owned by SV176.
    PluginDock,
    /// VCS History / A-B Visual Diff surface (VCS-04, synvelis#95) —
    /// read-only projection; never mutates project or repository.
    VcsHistory,
}

/// Persistent identity of a view instance: id + kind + display title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewDescriptor {
    /// Logical view id (owned by at most one dock location or the
    /// detached pool — `ruviform-ui` invariant).
    pub id: ViewInstanceId,
    /// The surface kind.
    pub kind: ViewType,
    /// Display title (tab caption / window chrome).
    pub title: String,
}

/// Headless render stub. A real adapter would paint pixels; this one
/// only counts renders and records the project revision it last saw,
/// which is enough to prove update distribution reaches every view.
#[derive(Debug, Clone, Default)]
pub struct HeadlessView {
    /// How many times `render` ran.
    pub render_count: u64,
    /// Project revision of the most recent render, if any.
    pub last_rendered_revision: Option<ProjectRevision>,
}

impl HeadlessView {
    /// Records one render pass against `revision`.
    pub fn render(&mut self, revision: ProjectRevision) {
        self.render_count += 1;
        self.last_rendered_revision = Some(revision);
    }
}

/// Produces headless view stubs per kind. Stateless.
#[derive(Debug, Default)]
pub struct ViewFactory;

impl ViewFactory {
    /// Creates a fresh headless view of `kind`.
    #[must_use]
    pub fn create(&self, kind: ViewType) -> HeadlessView {
        let _ = kind;
        HeadlessView::default()
    }
}

/// One registered view: descriptor plus its headless stub.
#[derive(Debug)]
pub struct ViewEntry {
    /// Identity.
    pub descriptor: ViewDescriptor,
    /// Headless rendering stub.
    pub headless: HeadlessView,
}

/// Registry of every live view in the workspace — independent of where
/// (or whether) the view is docked.
#[derive(Debug, Default)]
pub struct ViewRegistry {
    entries: BTreeMap<ViewInstanceId, ViewEntry>,
    factory: ViewFactory,
}

impl ViewRegistry {
    /// Registers a new view of `kind` under `id`.
    pub fn register(&mut self, id: ViewInstanceId, kind: ViewType, title: String) {
        let descriptor = ViewDescriptor { id, kind, title };
        let headless = self.factory.create(kind);
        self.entries.insert(
            id,
            ViewEntry {
                descriptor,
                headless,
            },
        );
    }

    /// Removes the view's registry entry.
    ///
    /// # Errors
    /// [`WorkspaceError::UnknownView`] when the id is not registered.
    pub fn unregister(&mut self, id: ViewInstanceId) -> Result<(), WorkspaceError> {
        self.entries
            .remove(&id)
            .map(|_| ())
            .ok_or(WorkspaceError::UnknownView(id))
    }

    /// The descriptor for `id`.
    #[must_use]
    pub fn descriptor(&self, id: ViewInstanceId) -> Option<&ViewDescriptor> {
        self.entries.get(&id).map(|e| &e.descriptor)
    }

    /// The headless stub for `id`.
    #[must_use]
    pub fn headless(&self, id: ViewInstanceId) -> Option<&HeadlessView> {
        self.entries.get(&id).map(|e| &e.headless)
    }

    /// Mutable access to the headless stub (render delivery).
    #[must_use]
    pub fn headless_mut(&mut self, id: ViewInstanceId) -> Option<&mut HeadlessView> {
        self.entries.get_mut(&id).map(|e| &mut e.headless)
    }

    /// Whether `id` is registered.
    #[must_use]
    pub fn contains(&self, id: ViewInstanceId) -> bool {
        self.entries.contains_key(&id)
    }

    /// All registered view ids, in id order.
    #[must_use]
    pub fn ids(&self) -> Vec<ViewInstanceId> {
        self.entries.keys().copied().collect()
    }
}
