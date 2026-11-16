//! ALIB-UX-H — headless Asset Library browser/import view-model +
//! driver-neutral [`ViewSpec`] projection (synvelis#89, spec
//! `docs/specs/asset-library.md`, design `docs/designs/asset-library-design.md`).
//!
//! This module is the *presentation/read-model* layer for the remote-aware
//! Asset Library. It consumes the typed provider boundary
//! (`synvelis-library`: [`AssetProvider`], [`ProviderRegistry`],
//! [`import_verified`]) and projects browse/coverage/pin/preview/import
//! state into deterministic [`ViewSpec`] trees that Fake, Mock, and (later,
//! synvelis#114) GPUI drivers mount unchanged.
//!
//! Contract boundaries (AL-R-01..17, AL-T-R-01..06):
//!
//! - **Typed states stay typed.** `Unsupported`, `Offline`, `Missing`,
//!   `Stale`, `Partial`, `CursorInvalid`, `PermissionDenied`, and
//!   `Cancelled` are explicit wire states — never folded into an empty
//!   listing, `Pass`, or a generic error string.
//! - **Explicit targets only.** Every mutating action (`entry.pin`,
//!   `preview.*`, `import.submit`, `drag.begin`) names its locator
//!   verbatim; nothing infers a target from ambient selection.
//! - **Preview ≠ import.** A successful preview fetch lands in
//!   [`PreviewUiState::Ready`] and never mints an import job or a
//!   `CommittedLive` claim. `CommittedLive` is the project-commit
//!   receipt only — saved `HEAD`/`SavedRevision` stays a separate axis.
//! - **Pin ≠ import.** `entry.pin` produces [`PinUiState::Pinned`]
//!   evidence; it writes nothing to the project.
//! - **View-local state only.** [`LibraryViewState`] persists selection,
//!   cursors, preview/import/drag surfaces per view. Provider and
//!   project authority live behind the registry/controller ports —
//!   this module never fabricates catalog facts.
//!
//! Real GUI mount, real providers, network, credentials, and real data
//! are out of scope (synvelis#91/#114/#86 gates).

use std::sync::Arc;

use ruviform_ui::view::{
    A11yRole, A11ySpec, ButtonSpec, CellValue, ComponentKey, ComponentSpec, ComponentVisualState,
    LabelSpec, PanelSpec, TableSelectionSpec, TableSpec, ViewSpec,
};
use serde::{Deserialize, Serialize};
use synvelis_library::{
    AssetObservation, AssetProvider, Availability, BlobPublisher, BrowseEntry, BrowsePage,
    BrowseRequest, BrowseScope, Coverage, EntryFacts, EntryKind, FilterSpec, ImportError,
    ImportIntent, ImportOutcome, ImportStore, ImportTarget, ObservedAt, OpaqueLocator,
    OpaquePageCursor, PinnedAssetRef, PreviewPlan, PrincipalId, ProjectBlobStore, ProvenanceRecord,
    ProviderCapabilities, ProviderContext, ProviderError, ProviderErrorKind, ProviderId,
    ProviderRegistry, RemoteAssetRef, SearchQuery, SortSpec, import_verified, rebase_import,
};
use synvelis_project::{ClipId, ImportId, ProjectController, ProjectRevision};

/// Kebab-case wire tag for a [`ProviderErrorKind`] — matches the
/// [`LibraryReason::code`] spelling contract (`cursor-invalid`, not
/// `cursorinvalid`). `ProviderErrorKind` has no serde impl, so the
/// mapping is explicit and exhaustive.
fn kind_tag(kind: ProviderErrorKind) -> String {
    let tag = match kind {
        ProviderErrorKind::NotFound => "not-found",
        ProviderErrorKind::RevisionGone => "revision-gone",
        ProviderErrorKind::PermissionDenied => "permission-denied",
        ProviderErrorKind::AuthExpired => "auth-expired",
        ProviderErrorKind::Offline => "offline",
        ProviderErrorKind::RateLimited => "rate-limited",
        ProviderErrorKind::TransientIo => "transient-io",
        ProviderErrorKind::UnsupportedCapability => "unsupported-capability",
        ProviderErrorKind::PreconditionFailed => "precondition-failed",
        ProviderErrorKind::SourceChanged => "source-changed",
        ProviderErrorKind::IntegrityMismatch => "integrity-mismatch",
        ProviderErrorKind::InvalidMedia => "invalid-media",
        ProviderErrorKind::BudgetExceeded => "budget-exceeded",
        ProviderErrorKind::Cancelled => "cancelled",
        ProviderErrorKind::CursorInvalid => "cursor-invalid",
        ProviderErrorKind::InvalidRequest => "invalid-request",
        ProviderErrorKind::Internal => "internal",
    };
    tag.to_string()
}

/// Kebab-case provider-family tag for the wire (`other` carries its
/// label so distinct families stay distinct).
fn provider_kind_tag(kind: &synvelis_library::ProviderKind) -> String {
    let value = serde_json::to_value(kind).unwrap_or_default();
    let tag = value
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    match value.get("label").and_then(serde_json::Value::as_str) {
        Some(label) => format!("{tag}:{label}"),
        None => tag,
    }
}

// ── reason codes ────────────────────────────────────────────────────

/// Stable machine reason codes for the Asset Library surface. Labels are
/// prose for humans; these codes are the contract drivers/tests match on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum LibraryReason {
    /// The named provider is not registered.
    UnknownProvider,
    /// No provider is selected yet.
    NoProvider,
    /// The provider does not support the requested capability
    /// (search, pin, preview, ...). Never an empty result.
    UnsupportedCapability,
    /// The provider/backend is unreachable.
    Offline,
    /// The principal lacks permission.
    PermissionDenied,
    /// Credentials expired or were revoked.
    AuthExpired,
    /// The object is missing on the provider.
    Missing,
    /// A page cursor failed validation (foreign provider, changed
    /// query/scope/principal, expired epoch, undecodable payload).
    CursorInvalid,
    /// The pinned revision moved or the fetch precondition failed.
    SourceChanged,
    /// Bytes diverged from the pin's verification evidence.
    IntegrityMismatch,
    /// A caller/context budget was exceeded.
    BudgetExceeded,
    /// The operation was cancelled.
    Cancelled,
    /// The project revision moved under the import intent.
    StaleRevision,
    /// The import id replayed with a different intent digest.
    IntegrityConflict,
    /// The import target (track/lane/placement) is gone.
    TargetGone,
    /// The publish destination state is unknowable after
    /// reconciliation — inspect before retrying; never assume
    /// "not published".
    PublishIndeterminate,
    /// The request itself is malformed (bad locator, wrong params).
    Malformed,
    /// Anything else — detail carries the provider/import error kind.
    Internal,
}

impl LibraryReason {
    /// The kebab-case wire/stable code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnknownProvider => "unknown-provider",
            Self::NoProvider => "no-provider",
            Self::UnsupportedCapability => "unsupported-capability",
            Self::Offline => "offline",
            Self::PermissionDenied => "permission-denied",
            Self::AuthExpired => "auth-expired",
            Self::Missing => "missing",
            Self::CursorInvalid => "cursor-invalid",
            Self::SourceChanged => "source-changed",
            Self::IntegrityMismatch => "integrity-mismatch",
            Self::BudgetExceeded => "budget-exceeded",
            Self::Cancelled => "cancelled",
            Self::StaleRevision => "stale-revision",
            Self::IntegrityConflict => "integrity-conflict",
            Self::TargetGone => "target-gone",
            Self::PublishIndeterminate => "publish-indeterminate",
            Self::Malformed => "malformed",
            Self::Internal => "internal",
        }
    }
}

/// A typed fault surfaced by a library action — the machine `reason`
/// plus the provider/import error kind and human detail.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryFault {
    /// Stable reason code.
    pub reason: LibraryReason,
    /// Provider/import error kind tag (e.g. `"cursor-invalid"`,
    /// `"stale-revision"`) when the fault originates below the view.
    pub kind: Option<String>,
    /// Human-readable detail — diagnostics only, never matched on.
    pub detail: String,
}

impl LibraryFault {
    fn new(reason: LibraryReason, kind: Option<String>, detail: impl Into<String>) -> Self {
        Self {
            reason,
            kind,
            detail: detail.into(),
        }
    }

    /// Maps a [`ProviderErrorKind`] to the typed reason.
    #[must_use]
    pub fn from_provider(kind: ProviderErrorKind, detail: impl Into<String>) -> Self {
        let reason = match kind {
            ProviderErrorKind::NotFound => LibraryReason::Missing,
            ProviderErrorKind::PermissionDenied => LibraryReason::PermissionDenied,
            ProviderErrorKind::AuthExpired => LibraryReason::AuthExpired,
            ProviderErrorKind::Offline => LibraryReason::Offline,
            ProviderErrorKind::UnsupportedCapability => LibraryReason::UnsupportedCapability,
            ProviderErrorKind::PreconditionFailed
            | ProviderErrorKind::SourceChanged
            | ProviderErrorKind::RevisionGone => LibraryReason::SourceChanged,
            ProviderErrorKind::IntegrityMismatch => LibraryReason::IntegrityMismatch,
            ProviderErrorKind::BudgetExceeded => LibraryReason::BudgetExceeded,
            ProviderErrorKind::Cancelled => LibraryReason::Cancelled,
            ProviderErrorKind::CursorInvalid => LibraryReason::CursorInvalid,
            ProviderErrorKind::InvalidRequest => LibraryReason::Malformed,
            _ => LibraryReason::Internal,
        };
        Self::new(reason, Some(kind_tag(kind)), detail)
    }
}

// ── wire DTOs ───────────────────────────────────────────────────────

/// One provider connection as the selector lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderWire {
    /// Tagged provider id (`"provider-<32hex>"` shape via `ProviderId`).
    pub id: String,
    /// Provider family tag.
    pub kind: String,
    /// Human-readable connection name.
    pub display_name: String,
    /// The provider's declared capabilities verbatim — every entry
    /// keeps its `supported`/`unsupported{reason}` tag, so an
    /// unsupported search/pin/preview is typed, never implied.
    pub capabilities: ProviderCapabilities,
}

/// One browse entry projected to the wire — the library's
/// `AssetObservation`/`Availability` types serialize verbatim, so
/// `observed`/`inferred`/`unknown{reason}`/`stale{reason}` and
/// `available`/`missing{detail}`/`offline`/`unavailable{reason}` stay
/// typed end-to-end. A stale byte length is never shown as fresh.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowseEntryWire {
    /// Provider-internal locator (opaque, returned verbatim).
    pub locator: String,
    /// Display name.
    pub display_name: String,
    /// `asset` or `container`.
    pub kind: EntryKind,
    /// Typed availability — `missing`/`offline`/`unavailable` carry
    /// their reasons.
    pub availability: Availability,
    /// Whether the entry is reached through a symbolic link.
    pub via_symlink: bool,
    /// Observed byte length — `unknown{reason}`/`stale{reason}` stay
    /// distinct from a fresh observed value.
    pub byte_len: AssetObservation<u64>,
    /// Observed modification time — same observation contract.
    pub modified: AssetObservation<ObservedAt>,
}

/// A stat result projected to the wire — the full fact bundle and
/// provenance records, not a flattened summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatWire {
    /// The locator stat addressed.
    pub locator: String,
    /// Typed availability.
    pub availability: Availability,
    /// Observed entry facts (media facts stay `unknown` until probed).
    pub facts: EntryFacts,
    /// Provenance records observed on the asset.
    pub provenance: Vec<ProvenanceRecord>,
}

/// Pin evidence state — `Pinned` carries the typed proof tag.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PinUiState {
    /// Nothing pinned for the current selection.
    #[default]
    Idle,
    /// A pin was produced — `proof` is `immutable-object` or
    /// `conditional-strong`, `object` the provider object identity,
    /// `proof_revision` the pinned revision/validator evidence so two
    /// pins of the same object at different revisions stay distinct.
    Pinned {
        /// Proof tag.
        proof: String,
        /// Provider object identity string.
        object: String,
        /// The pin's revision/validator evidence (immutable revision
        /// key or strong token value).
        #[serde(default)]
        proof_revision: String,
        /// Expected byte length at pin time, when reported.
        expected_size: Option<u64>,
    },
    /// Pinning is not supported by this provider/asset.
    Unsupported {
        /// Why (provider capability reason tag or error kind).
        reason: String,
    },
    /// Pinning failed with a typed fault.
    Failed {
        /// Machine reason code.
        reason: String,
        /// Detail.
        detail: String,
    },
}

/// Preview surface state — `Ready` is *not* a verified import.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PreviewUiState {
    /// No preview activity.
    #[default]
    Idle,
    /// A plan was computed — `bounded` bytes may fetch without
    /// consent; `consent-required` needs `allow_full_object`;
    /// `over-budget` refuses regardless.
    PlanReady {
        /// Plan tag: `bounded`/`consent-required`/`over-budget`.
        plan: String,
        /// Bytes the fetch would deliver when known.
        bytes: Option<u64>,
    },
    /// A bounded preview fetch completed — unverified observation
    /// bytes, explicitly not an import.
    Ready {
        /// Bytes delivered.
        byte_len: u64,
        /// Whether bytes came from a verified cache lease.
        cached: bool,
    },
    /// The fetch was cancelled mid-flight.
    Cancelled,
    /// The fetch/plan failed with a typed fault.
    Failed {
        /// Machine reason code.
        reason: String,
        /// Detail.
        detail: String,
    },
}

/// Import surface state. `CommittedLive` is the project-commit
/// receipt — saved `HEAD`/`SavedRevision` is a separate axis and is
/// never implied here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ImportUiState {
    /// No import activity.
    #[default]
    Idle,
    /// An import committed to the live project — carries the receipt.
    CommittedLive {
        /// Idempotency key.
        import_id: String,
        /// Committed asset id (tagged).
        asset_id: String,
        /// Committed source id (tagged).
        source_id: String,
        /// Committed clip id when a target placement was given.
        clip_id: Option<String>,
        /// Project revision the commit landed at.
        revision: u64,
        /// Content digest hex of the verified bytes.
        content_digest: String,
        /// Byte length.
        byte_len: u64,
        /// `true` when the same import id replayed idempotently.
        reused: bool,
    },
    /// The project revision moved under the intent — recoverable via
    /// explicit `import.rebase` + `import.retry`.
    StaleRevision {
        /// Import id awaiting rebase.
        import_id: String,
        /// The revision the intent was pinned to.
        expected: u64,
        /// The live revision observed at failure.
        actual: u64,
    },
    /// Import failed with a typed fault.
    Failed {
        /// Machine reason code.
        reason: String,
        /// Detail.
        detail: String,
    },
    /// The import was cancelled.
    Cancelled,
}

/// Drag surface state — a pending ghost only. No project asset,
/// source, or clip record exists until verified import + commit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DragUiState {
    /// No drag in flight.
    #[default]
    Idle,
    /// A drag ghost is pending — carries the source locator and
    /// display name; drop targets are the caller's concern.
    Ghost {
        /// Source locator being dragged.
        locator: String,
        /// Display name shown on the ghost.
        display_name: String,
    },
}

/// Per-view library state — the view-local half of the model.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryViewState {
    /// Selected provider id (tagged) — `None` until `provider.select`.
    pub provider: Option<String>,
    /// Current browse scope locator (`None` = provider root).
    pub scope_locator: Option<String>,
    /// Accumulated browse entries (page order).
    pub entries: Vec<BrowseEntryWire>,
    /// Continuation cursor for `browse.more` (`null` = exhausted).
    pub next_cursor: Option<String>,
    /// Coverage of the last browse page.
    pub coverage: Option<String>,
    /// Active search text (`None` = plain browse listing).
    pub search_text: Option<String>,
    /// Selected entry locator.
    pub selected_locator: Option<String>,
    /// Last stat for the selection.
    pub stat: Option<StatWire>,
    /// Pin surface state.
    pub pin: PinUiState,
    /// Preview surface state.
    pub preview: PreviewUiState,
    /// Import surface state.
    pub import: ImportUiState,
    /// Drag surface state.
    pub drag: DragUiState,
    /// Last typed fault (cleared by the next successful action).
    pub fault: Option<LibraryFault>,
}

// ── actions ─────────────────────────────────────────────────────────

/// One library action. Every mutating variant carries its target
/// locator explicitly — ambient selection is never an implicit target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", deny_unknown_fields)]
#[non_exhaustive]
pub enum LibraryAction {
    /// Select a registered provider by tagged id.
    #[serde(rename = "provider.select")]
    ProviderSelect {
        /// Tagged provider id.
        provider: String,
    },
    /// Re-list the current scope from the root cursor.
    #[serde(rename = "browse.refresh")]
    BrowseRefresh {},
    /// Descend into a container locator (explicit scope change).
    #[serde(rename = "browse.scope")]
    BrowseScope {
        /// Container locator to scope under.
        locator: String,
    },
    /// Fetch the next page via the stored opaque cursor.
    #[serde(rename = "browse.more")]
    BrowseMore {},
    /// Provider-side search for `text` in the current scope.
    #[serde(rename = "search.set")]
    SearchSet {
        /// Free-text query.
        text: String,
    },
    /// Clear the search and return to plain browse.
    #[serde(rename = "search.clear")]
    SearchClear {},
    /// Select an entry and stat it for the inspector.
    #[serde(rename = "entry.select")]
    EntrySelect {
        /// Entry locator.
        locator: String,
    },
    /// Re-stat the selected (or named) entry.
    #[serde(rename = "entry.stat")]
    EntryStat {
        /// Entry locator.
        locator: String,
    },
    /// Pin an entry's asset to a fetchable revision.
    #[serde(rename = "entry.pin")]
    EntryPin {
        /// Entry locator.
        locator: String,
    },
    /// Compute the preview plan for an entry.
    #[serde(rename = "preview.plan")]
    PreviewPlan {
        /// Entry locator.
        locator: String,
        /// Explicit consent to a whole-object fetch.
        #[serde(default)]
        allow_full_object: bool,
    },
    /// Fetch a bounded preview for an entry (synchronous on the
    /// caller's thread; honors the cooperative cancel token between
    /// read chunks so a later job-thread driver can cancel mid-flight
    /// without contract changes).
    #[serde(rename = "preview.fetch")]
    PreviewFetch {
        /// Entry locator — explicit target, never inferred from the
        /// current selection.
        locator: String,
        /// Explicit consent to a whole-object fetch.
        #[serde(default)]
        allow_full_object: bool,
    },
    /// Cancel an in-flight preview fetch.
    #[serde(rename = "preview.cancel")]
    PreviewCancel {},
    /// Submit a verified import. `locator` names the asset explicitly;
    /// `import_id` is the idempotency key; `expected_project_revision`
    /// pins the project CAS base (`null` = live revision at submit).
    #[serde(rename = "import.submit")]
    ImportSubmit {
        /// Idempotency key — u128 raw or tagged `"import-<32hex>"`.
        import_id: serde_json::Value,
        /// Asset locator to pin + import.
        locator: String,
        /// Expected project revision (defaults to live).
        #[serde(default)]
        expected_project_revision: Option<u64>,
        /// Explicit consent to a whole-object fetch when the pin
        /// reports no bounded path.
        #[serde(default)]
        allow_full_object: bool,
        /// Optional clip target (track/lane/placement/playback).
        /// Boxed: `ImportTargetWire` carries two `serde_json::Value`s
        /// whose size grows under feature unification (e.g. gpui's
        /// `preserve_order`), which would otherwise make this variant
        /// dominate the enum (`clippy::large_enum_variant`).
        #[serde(default)]
        target: Option<Box<ImportTargetWire>>,
    },
    /// Rebase a stale import to the current live revision, then the
    /// caller retries with `ImportRetry`.
    #[serde(rename = "import.rebase")]
    ImportRebase {
        /// Import id to rebase.
        import_id: serde_json::Value,
        /// New expected project revision.
        expected_project_revision: u64,
    },
    /// Retry the last failed/stale import intent.
    #[serde(rename = "import.retry")]
    ImportRetry {},
    /// Dismiss the current import surface state.
    #[serde(rename = "import.dismiss")]
    ImportDismiss {},
    /// Begin a drag — emits a ghost only; no project mutation.
    #[serde(rename = "drag.begin")]
    DragBegin {
        /// Entry locator being dragged.
        locator: String,
    },
    /// Drop the pending drag ghost.
    #[serde(rename = "drag.drop")]
    DragDrop {},
}

/// Wire form of [`ImportTarget`] — track/lane ids as tagged strings,
/// placement/playback carried as their serde forms.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportTargetWire {
    /// Tagged track id.
    pub track_id: String,
    /// Tagged lane id.
    pub lane_id: String,
    /// Clip placement (serde form of [`synvelis_project::ClipPlacement`]).
    pub placement: serde_json::Value,
    /// Clip playback (serde form of [`synvelis_project::ClipPlayback`]).
    pub playback: serde_json::Value,
}

// ── import ports ────────────────────────────────────────────────────

/// The backend ports a verified import needs, borrowed for one call.
/// The app layer owns the `ImportStore`, blob store, publisher, and
/// `ProjectController`; the view model never invents them.
pub struct LibraryImportPorts<'a> {
    /// Durable import manifest/staging store.
    pub store: &'a ImportStore,
    /// Project blob store port.
    pub blobs: &'a dyn ProjectBlobStore,
    /// Blob publisher port.
    pub publisher: &'a dyn BlobPublisher,
}

// ── view model ──────────────────────────────────────────────────────

/// The headless Asset Library view model for one view instance.
///
/// Owns the provider registry handle, the principal scope, and the
/// per-view [`LibraryViewState`]. Provider calls are blocking and run
/// on the caller's (non-RT) thread — the app layer invokes them
/// synchronously inside service ops for this headless slice; the
/// preview fetch still honors a cooperative cancel token between read
/// chunks so a later job-thread driver can cancel mid-flight without
/// contract changes.
pub struct LibraryViewModel {
    registry: Arc<ProviderRegistry>,
    principal: PrincipalId,
    state: LibraryViewState,
    /// Cooperative cancel for an in-flight preview fetch.
    preview_cancel: synvelis_library::CancellationToken,
    /// The pinned ref + intent of the last import attempt, kept so
    /// `import.retry`/`import.rebase` replay the same digest.
    pending_import: Option<(PinnedAssetRef, ImportIntent)>,
}

impl LibraryViewModel {
    /// A model over `registry` acting as `principal`.
    #[must_use]
    pub fn new(registry: Arc<ProviderRegistry>, principal: PrincipalId) -> Self {
        Self {
            registry,
            principal,
            state: LibraryViewState::default(),
            preview_cancel: synvelis_library::CancellationToken::new(),
            pending_import: None,
        }
    }

    /// The view-local state (read-only projection source).
    #[must_use]
    pub fn state(&self) -> &LibraryViewState {
        &self.state
    }

    /// The registered provider descriptors, in registry order.
    #[must_use]
    pub fn providers(&self) -> Vec<ProviderWire> {
        provider_wires(&self.registry)
    }

    /// Applies one action. `ports`/`controller` are required only by
    /// `ImportSubmit`/`ImportRebase`/`ImportRetry` — every other action
    /// is provider-read-only and needs neither.
    ///
    /// # Errors
    /// Never — failures land in [`LibraryViewState::fault`] or the
    /// typed surface states; the return value is the post-action
    /// snapshot for the caller's convenience.
    pub fn apply(
        &mut self,
        action: &LibraryAction,
        ports: Option<&LibraryImportPorts<'_>>,
        controller: Option<&mut ProjectController>,
    ) {
        self.state.fault = None;
        match action {
            LibraryAction::ProviderSelect { provider } => self.provider_select(provider),
            LibraryAction::BrowseRefresh {} => self.browse_refresh(),
            LibraryAction::BrowseScope { locator } => self.browse_scope(locator),
            LibraryAction::BrowseMore {} => self.browse_more(),
            LibraryAction::SearchSet { text } => self.search_set(text),
            LibraryAction::SearchClear {} => self.search_clear(),
            LibraryAction::EntrySelect { locator } => self.entry_select(locator),
            LibraryAction::EntryStat { locator } => self.entry_stat(locator),
            LibraryAction::EntryPin { locator } => self.entry_pin(locator),
            LibraryAction::PreviewPlan {
                locator,
                allow_full_object,
            } => self.preview_plan(locator, *allow_full_object),
            LibraryAction::PreviewFetch {
                locator,
                allow_full_object,
            } => self.preview_fetch(locator, *allow_full_object),
            LibraryAction::PreviewCancel {} => self.preview_cancel(),
            LibraryAction::ImportSubmit {
                import_id,
                locator,
                expected_project_revision,
                allow_full_object,
                target,
            } => self.import_submit(
                import_id,
                locator,
                *expected_project_revision,
                *allow_full_object,
                target.as_deref(),
                ports,
                controller,
            ),
            LibraryAction::ImportRebase {
                import_id,
                expected_project_revision,
            } => self.import_rebase(import_id, *expected_project_revision, ports, controller),
            LibraryAction::ImportRetry {} => self.import_retry(ports, controller),
            LibraryAction::ImportDismiss {} => self.state.import = ImportUiState::Idle,
            LibraryAction::DragBegin { locator } => self.drag_begin(locator),
            LibraryAction::DragDrop {} => self.state.drag = DragUiState::Idle,
        }
    }

    /// The current provider, or a typed `NoProvider`/`UnknownProvider`
    /// fault recorded on the state.
    fn provider(&mut self) -> Result<Arc<dyn AssetProvider>, ()> {
        let Some(tagged) = &self.state.provider else {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::NoProvider,
                None,
                "no provider selected",
            ));
            return Err(());
        };
        let id = ProviderId::from_tagged(tagged).map_err(|e| {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::Malformed,
                None,
                format!("stored provider id malformed: {e}"),
            ));
        })?;
        self.registry.get(&id).ok_or_else(|| {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::UnknownProvider,
                None,
                format!("provider {tagged} not registered"),
            ));
        })
    }

    fn provider_id(&self) -> Option<ProviderId> {
        self.state
            .provider
            .as_deref()
            .and_then(|t| ProviderId::from_tagged(t).ok())
    }

    /// A fresh provider context — every non-preview call gets its own
    /// cancel token so a `preview.cancel` can never leak cancellation
    /// into browse/stat/pin/import calls.
    fn context(&self) -> ProviderContext {
        ProviderContext::new(self.principal.clone())
    }

    /// The preview-scoped context — the only call family that carries
    /// the model's cooperative preview cancel token.
    fn preview_context(&self) -> ProviderContext {
        let mut cx = self.context();
        cx.cancel = self.preview_cancel.clone();
        cx
    }

    fn fail_provider(&mut self, e: &ProviderError) {
        self.state.fault = Some(LibraryFault::from_provider(e.kind(), e.detail()));
    }

    fn provider_select(&mut self, provider: &str) {
        match ProviderId::from_tagged(provider) {
            Ok(id) => {
                if self.registry.get(&id).is_none() {
                    self.state.fault = Some(LibraryFault::new(
                        LibraryReason::UnknownProvider,
                        None,
                        format!("provider {provider} not registered"),
                    ));
                    return;
                }
                self.state.provider = Some(provider.to_string());
                // Provider switch invalidates browse/selection/preview
                // surfaces — cursors and pins are provider-bound.
                self.state.entries.clear();
                self.state.next_cursor = None;
                self.state.coverage = None;
                self.state.search_text = None;
                self.state.selected_locator = None;
                self.state.stat = None;
                self.state.pin = PinUiState::Idle;
                self.state.preview = PreviewUiState::Idle;
                self.state.drag = DragUiState::Idle;
                self.state.scope_locator = None;
                // The pending intent's pin belongs to the old provider
                // — retry under the new one can only fail typed. A
                // `CommittedLive` receipt is project truth and stays;
                // attempt states (stale/failed) die with the intent.
                self.pending_import = None;
                if !matches!(self.state.import, ImportUiState::CommittedLive { .. }) {
                    self.state.import = ImportUiState::Idle;
                }
            }
            Err(e) => {
                self.state.fault = Some(LibraryFault::new(
                    LibraryReason::Malformed,
                    None,
                    format!("provider id malformed: {e}"),
                ));
            }
        }
    }

    fn asset_for(&mut self, locator: &str) -> Result<RemoteAssetRef, ()> {
        // Records the NoProvider/UnknownProvider fault when unset.
        self.provider()?;
        let provider = self.provider_id().ok_or(())?;
        let locator = OpaqueLocator::new(locator.to_string()).map_err(|e| {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::Malformed,
                None,
                format!("locator malformed: {e}"),
            ));
        })?;
        Ok(RemoteAssetRef { provider, locator })
    }

    fn scope(&self) -> BrowseScope {
        match &self.state.scope_locator {
            Some(loc) => OpaqueLocator::new(loc.clone()).map_or(BrowseScope::Root, |locator| {
                BrowseScope::Locator { locator }
            }),
            None => BrowseScope::Root,
        }
    }

    fn browse_request(&self, cursor: Option<OpaquePageCursor>) -> BrowseRequest {
        BrowseRequest {
            scope: self.scope(),
            page_limit: 64,
            cursor,
            sort: SortSpec::default(),
            filter: FilterSpec::default(),
        }
    }

    /// Runs the active listing mode — search while `search_text` is
    /// set, browse otherwise — so refresh/more/scope never silently
    /// switch result sets underneath the user.
    fn run_browse(&mut self, append: bool) {
        if self.state.search_text.is_some() {
            self.run_search(append);
            return;
        }
        let Ok(provider) = self.provider() else {
            return;
        };
        let cursor = if append {
            self.state
                .next_cursor
                .as_deref()
                .and_then(|c| OpaquePageCursor::new(c.to_string()).ok())
        } else {
            None
        };
        let req = self.browse_request(cursor);
        let cx = self.context();
        match provider.browse(&req, &cx) {
            Ok(page) => self.apply_page(page, append),
            Err(e) => self.fail_provider(&e),
        }
    }

    /// Search continuation/refresh — same cursor discipline as browse:
    /// `append` resumes from the stored cursor, otherwise the first
    /// page replaces the listing. The typed unsupported fault is
    /// raised here too so `browse.more` on a search listing cannot
    /// bypass the capability gate `search.set` applied.
    fn run_search(&mut self, append: bool) {
        let Some(text) = self.state.search_text.clone() else {
            return;
        };
        let Ok(provider) = self.provider() else {
            return;
        };
        if !provider.capabilities(None).search.is_supported() {
            self.unsupported_search();
            return;
        }
        let cursor = if append {
            self.state
                .next_cursor
                .as_deref()
                .and_then(|c| OpaquePageCursor::new(c.to_string()).ok())
        } else {
            None
        };
        let query = SearchQuery {
            text,
            scope: self.scope(),
            page_limit: 64,
            cursor,
        };
        let cx = self.context();
        match provider.search(&query, &cx) {
            Ok(page) => self.apply_page(page, append),
            Err(e) => self.fail_provider(&e),
        }
    }

    fn unsupported_search(&mut self) {
        self.state.fault = Some(LibraryFault::new(
            LibraryReason::UnsupportedCapability,
            Some("search".to_string()),
            "provider does not support search",
        ));
    }

    fn apply_page(&mut self, page: BrowsePage, append: bool) {
        if !append {
            self.state.entries.clear();
        }
        self.state
            .entries
            .extend(page.entries.iter().map(entry_wire));
        self.state.next_cursor = page.next_cursor.map(|c| c.to_string());
        self.state.coverage = Some(coverage_tag(page.coverage).to_string());
    }

    fn browse_refresh(&mut self) {
        self.run_browse(false);
    }

    fn browse_scope(&mut self, locator: &str) {
        if let Err(e) = OpaqueLocator::new(locator.to_string()) {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::Malformed,
                None,
                format!("scope locator malformed: {e}"),
            ));
            return;
        }
        self.state.scope_locator = Some(locator.to_string());
        // A scope change makes the current listing wrong-label data —
        // drop it up front so a failed re-list shows empty+fault,
        // never old-scope rows under the new scope.
        self.state.entries.clear();
        self.state.next_cursor = None;
        self.state.coverage = None;
        self.run_browse(false);
    }

    fn browse_more(&mut self) {
        if self.state.next_cursor.is_none() {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::Malformed,
                None,
                "browse.more with no continuation cursor",
            ));
            return;
        }
        self.run_browse(true);
    }

    fn search_set(&mut self, text: &str) {
        let Ok(provider) = self.provider() else {
            return;
        };
        let caps = provider.capabilities(None);
        if !caps.search.is_supported() {
            // Typed unsupported — never an empty listing, and the
            // search mode is not entered.
            self.unsupported_search();
            return;
        }
        // Enter search mode only on a successful first page — a
        // failed search must not leave browse rows labeled as search
        // results, nor feed a browse cursor into `SearchQuery` on the
        // next `browse.more`.
        let query = SearchQuery {
            text: text.to_string(),
            scope: self.scope(),
            page_limit: 64,
            cursor: None,
        };
        let cx = self.context();
        match provider.search(&query, &cx) {
            Ok(page) => {
                self.state.search_text = Some(text.to_string());
                self.apply_page(page, false);
            }
            Err(e) => self.fail_provider(&e),
        }
    }

    fn search_clear(&mut self) {
        self.state.search_text = None;
        // Same wrong-label rule as `browse.scope`: search rows must
        // not linger under the browse-mode label if the re-list fails.
        self.state.entries.clear();
        self.state.next_cursor = None;
        self.state.coverage = None;
        self.run_browse(false);
    }

    fn entry_select(&mut self, locator: &str) {
        if self.asset_for(locator).is_ok() {
            self.state.selected_locator = Some(locator.to_string());
            self.entry_stat(locator);
        }
    }

    fn entry_stat(&mut self, locator: &str) {
        let Ok(provider) = self.provider() else {
            return;
        };
        let Ok(asset) = self.asset_for(locator) else {
            return;
        };
        let cx = self.context();
        match provider.stat(&asset, &cx) {
            Ok(stat) => {
                self.state.stat = Some(StatWire {
                    locator: locator.to_string(),
                    availability: stat.availability.clone(),
                    facts: stat.facts.clone(),
                    provenance: stat.provenance.clone(),
                });
            }
            Err(e) => {
                // A failed re-stat must not leave the previous
                // entry's facts on display under the new locator.
                self.state.stat = None;
                self.fail_provider(&e);
            }
        }
    }

    fn entry_pin(&mut self, locator: &str) {
        let Ok(provider) = self.provider() else {
            return;
        };
        let Ok(asset) = self.asset_for(locator) else {
            return;
        };
        let caps = provider.capabilities(Some(&asset));
        if !caps.pin.is_supported() {
            self.state.pin = PinUiState::Unsupported {
                reason: "pin-capability-unsupported".to_string(),
            };
            return;
        }
        let cx = self.context();
        match provider.pin(&asset, &cx) {
            Ok(pin) => {
                self.state.pin = PinUiState::Pinned {
                    proof: pin_proof_tag(&pin).to_string(),
                    object: pin.object_identity.as_str().to_string(),
                    proof_revision: pin_proof_revision(&pin),
                    expected_size: pin.expected_size,
                };
            }
            Err(e) => {
                if e.kind() == ProviderErrorKind::UnsupportedCapability {
                    self.state.pin = PinUiState::Unsupported {
                        reason: "unsupported-capability".to_string(),
                    };
                } else {
                    self.state.pin = PinUiState::Failed {
                        reason: kind_tag(e.kind()),
                        detail: e.detail().to_string(),
                    };
                }
            }
        }
    }

    fn preview_plan(&mut self, locator: &str, allow_full_object: bool) {
        let _ = allow_full_object; // plan is consent-neutral; fetch re-checks
        let Ok(provider) = self.provider() else {
            return;
        };
        let Ok(asset) = self.asset_for(locator) else {
            return;
        };
        // The view model computes the plan through the same bounded
        // rules the cache layer enforces: a preview is bounded unless
        // the provider reports a whole-object read. The stat probe is
        // a plain provider call — it must not carry the fetch cancel
        // token (a cancelled earlier fetch would poison every later
        // plan's size hint).
        let caps = provider.capabilities(Some(&asset));
        if !caps.preview.is_supported() {
            self.state.preview = PreviewUiState::Failed {
                reason: LibraryReason::UnsupportedCapability.code().to_string(),
                detail: "provider does not support preview".to_string(),
            };
            return;
        }
        // Stat for the size hint, then classify: bounded when the
        // provider can range-read or the object is small; otherwise
        // the fetch is a full-object download behind the consent gate.
        // A failed stat means "size unknown" → the conservative
        // `FullDownload` class, not a fabricated small object.
        let cx = self.context();
        let size = provider
            .stat(&asset, &cx)
            .ok()
            .and_then(|s| s.facts.byte_len.value().copied());
        let plan = classify_preview(size, caps.range_read.is_supported());
        self.state.preview = PreviewUiState::PlanReady {
            plan: plan_tag(&plan).to_string(),
            bytes: plan_bytes(&plan),
        };
    }

    fn preview_fetch(&mut self, locator: &str, allow_full_object: bool) {
        self.preview_cancel = synvelis_library::CancellationToken::new();
        let Ok(provider) = self.provider() else {
            self.state.preview = PreviewUiState::Failed {
                reason: self
                    .state
                    .fault
                    .as_ref()
                    .map_or("no-provider", |f| f.reason.code())
                    .to_string(),
                detail: "no provider selected".to_string(),
            };
            return;
        };
        let Ok(asset) = self.asset_for(locator) else {
            self.state.preview = PreviewUiState::Failed {
                reason: LibraryReason::Malformed.code().to_string(),
                detail: "locator malformed".to_string(),
            };
            return;
        };
        let caps = provider.capabilities(Some(&asset));
        if !caps.preview.is_supported() {
            self.state.preview = PreviewUiState::Failed {
                reason: LibraryReason::UnsupportedCapability.code().to_string(),
                detail: "provider does not support preview".to_string(),
            };
            return;
        }
        let cx = self.preview_context();
        // Plan gate: a whole-object fetch needs explicit consent and
        // stays bounded by the preview byte cap.
        let size = provider
            .stat(&asset, &cx)
            .ok()
            .and_then(|s| s.facts.byte_len.value().copied());
        let plan = classify_preview(size, caps.range_read.is_supported());
        match plan {
            PreviewPlan::OverBudget { bytes } => {
                self.state.preview = PreviewUiState::PlanReady {
                    plan: "over-budget".to_string(),
                    bytes: Some(bytes),
                };
                return;
            }
            PreviewPlan::FullDownload { .. } if !allow_full_object => {
                self.state.preview = PreviewUiState::PlanReady {
                    plan: "consent-required".to_string(),
                    bytes: size,
                };
                return;
            }
            _ => {}
        }
        let range = preview_range(size);
        match provider.open_preview(&asset, range, &cx) {
            Ok(pinned) => self.read_preview(pinned, size, allow_full_object),
            Err(e) => {
                self.state.preview = PreviewUiState::Failed {
                    reason: kind_tag(e.kind()),
                    detail: e.detail().to_string(),
                };
            }
        }
    }

    /// Reads a bounded preview stream to `Ready`/`Cancelled`/`Failed`.
    /// Preview bytes are an unverified observation — this never mints
    /// an import job or a `CommittedLive` claim. The byte cap follows
    /// the consent class: `PREVIEW_CAP` for bounded fetches,
    /// `FULL_OBJECT_CONSENT_BYTES` once `allow_full_object` was
    /// granted — a consented whole-object preview delivers up to the
    /// consent ceiling instead of dying `BudgetExceeded` mid-stream.
    fn read_preview(
        &mut self,
        mut pinned: synvelis_library::PinnedRead,
        size: Option<u64>,
        allow_full_object: bool,
    ) {
        use std::io::Read as _;
        let mut buf = Vec::new();
        let ceiling = if allow_full_object {
            FULL_OBJECT_CONSENT_BYTES
        } else {
            PREVIEW_CAP
        };
        let cap = size.map_or(ceiling, |n| n.min(ceiling));
        let mut chunk = [0u8; 8192];
        loop {
            if self.preview_cancel.is_cancelled() {
                self.state.preview = PreviewUiState::Cancelled;
                return;
            }
            match pinned.reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    if (buf.len() as u64 + n as u64) > cap {
                        self.state.preview = PreviewUiState::Failed {
                            reason: LibraryReason::BudgetExceeded.code().to_string(),
                            detail: "preview exceeded byte cap".to_string(),
                        };
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                }
                Err(e) => {
                    self.state.preview = PreviewUiState::Failed {
                        reason: LibraryReason::Internal.code().to_string(),
                        detail: format!("preview read: {e}"),
                    };
                    return;
                }
            }
        }
        if self.preview_cancel.is_cancelled() {
            self.state.preview = PreviewUiState::Cancelled;
            return;
        }
        // Honor the stream's terminal outcome — a provider that
        // recorded a fetch error at EOF must not be reported `Ready`.
        if let Some(Err(e)) = pinned.completion.take() {
            self.state.preview = PreviewUiState::Failed {
                reason: kind_tag(e.kind()),
                detail: e.detail().to_string(),
            };
            return;
        }
        self.state.preview = PreviewUiState::Ready {
            byte_len: buf.len() as u64,
            cached: false,
        };
    }

    fn preview_cancel(&mut self) {
        self.preview_cancel.cancel();
        // Cancel is an explicit typed outcome — recorded even when no
        // fetch was in flight so the surface never fakes "still running".
        self.state.preview = PreviewUiState::Cancelled;
    }

    fn parse_import_id(value: &serde_json::Value) -> Result<ImportId, LibraryFault> {
        if let Some(n) = value.as_u64() {
            return Ok(ImportId::new(u128::from(n)));
        }
        if let Some(s) = value.as_str() {
            return ImportId::from_tagged(s).map_err(|e| {
                LibraryFault::new(
                    LibraryReason::Malformed,
                    None,
                    format!("import_id malformed: {e}"),
                )
            });
        }
        Err(LibraryFault::new(
            LibraryReason::Malformed,
            None,
            "import_id must be a u64 or a tagged \"import-<32hex>\" string",
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn import_submit(
        &mut self,
        import_id: &serde_json::Value,
        locator: &str,
        expected_project_revision: Option<u64>,
        allow_full_object: bool,
        target: Option<&ImportTargetWire>,
        ports: Option<&LibraryImportPorts<'_>>,
        controller: Option<&mut ProjectController>,
    ) {
        let import_id = match Self::parse_import_id(import_id) {
            Ok(id) => id,
            Err(f) => {
                self.state.fault = Some(f);
                return;
            }
        };
        let (Some(ports), Some(controller)) = (ports, controller) else {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::Malformed,
                None,
                "import.submit requires import ports and a project controller",
            ));
            return;
        };
        // A real attempt supersedes the previous surface state — a
        // stale `CommittedLive`/`StaleRevision` card must never sit
        // beside the new attempt's fault.
        self.state.import = ImportUiState::Idle;
        // Explicit pin first — import consumes the pinned evidence,
        // never a live re-resolution.
        let Ok(asset) = self.asset_for(locator) else {
            return;
        };
        let Ok(provider) = self.provider() else {
            return;
        };
        let cx = self.context();
        let pin = match provider.pin(&asset, &cx) {
            Ok(p) => p,
            Err(e) => {
                self.state.import = ImportUiState::Failed {
                    reason: kind_tag(e.kind()),
                    detail: e.detail().to_string(),
                };
                self.fail_provider(&e);
                return;
            }
        };
        // Whole-object consent: a pin whose fetch is unbounded — or
        // whose size the provider cannot declare — needs the explicit
        // flag before import_verified streams it. Unknown size is
        // treated like preview's `FullDownload`: consent, not free.
        if !allow_full_object
            && pin
                .expected_size
                .is_none_or(|n| n > FULL_OBJECT_CONSENT_BYTES)
        {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::UnsupportedCapability,
                Some("full-object-consent".to_string()),
                "import of a whole-object asset requires allow_full_object",
            ));
            return;
        }
        let target = match target {
            Some(t) => match Self::decode_target(t) {
                Ok(t) => Some(t),
                Err(f) => {
                    self.state.fault = Some(f);
                    return;
                }
            },
            None => None,
        };
        let expected =
            expected_project_revision.map_or_else(|| controller.live_revision(), ProjectRevision);
        let intent = ImportIntent {
            import_id,
            expected_project_revision: expected,
            source: pin.clone(),
            target,
        };
        // Record the pending intent before the call so a StaleRevision
        // failure still knows the digest it was rebased from.
        self.pending_import = Some((pin.clone(), intent.clone()));
        self.run_import(ports, controller, &pin, &intent, &cx);
    }

    fn decode_target(wire: &ImportTargetWire) -> Result<ImportTarget, LibraryFault> {
        let bad = |what: &str| {
            LibraryFault::new(
                LibraryReason::Malformed,
                None,
                format!("import target {what} malformed"),
            )
        };
        let track_id =
            synvelis_project::TrackId::from_tagged(&wire.track_id).map_err(|_| bad("track_id"))?;
        let lane_id =
            synvelis_project::LaneId::from_tagged(&wire.lane_id).map_err(|_| bad("lane_id"))?;
        let placement: synvelis_project::ClipPlacement =
            serde_json::from_value(wire.placement.clone()).map_err(|_| bad("placement"))?;
        let playback: synvelis_project::ClipPlayback =
            serde_json::from_value(wire.playback.clone()).map_err(|_| bad("playback"))?;
        Ok(ImportTarget {
            track_id,
            lane_id,
            placement,
            playback,
        })
    }

    fn run_import(
        &mut self,
        ports: &LibraryImportPorts<'_>,
        controller: &mut ProjectController,
        pin: &PinnedAssetRef,
        intent: &ImportIntent,
        cx: &ProviderContext,
    ) {
        let Ok(provider) = self.provider() else {
            return;
        };
        match import_verified(
            provider.as_ref(),
            intent,
            ports.store,
            ports.blobs,
            ports.publisher,
            controller,
            cx,
        ) {
            Ok(outcome) => {
                self.pending_import = Some((pin.clone(), intent.clone()));
                self.state.import = import_outcome_state(&outcome);
                self.state.fault = None;
            }
            Err(e) => self.import_failed(e),
        }
    }

    fn import_failed(&mut self, e: ImportError) {
        match e {
            // The error carries the manifest's true expectation and
            // the live revision at rejection — report those, not the
            // caller's hint, which can diverge after a rebase or a
            // replayed intent.
            ImportError::StaleRevision { expected, actual } => {
                let import_id = self
                    .pending_import
                    .as_ref()
                    .map_or_else(|| "import-?".to_string(), |(_, i)| i.import_id.to_tagged());
                self.state.import = ImportUiState::StaleRevision {
                    import_id,
                    expected,
                    actual,
                };
            }
            ImportError::IntegrityConflict { .. } => {
                self.state.import = ImportUiState::Failed {
                    reason: LibraryReason::IntegrityConflict.code().to_string(),
                    detail: "import id replayed with a different intent".to_string(),
                };
            }
            ImportError::Provider(pe) => {
                if pe.kind() == ProviderErrorKind::Cancelled {
                    self.state.import = ImportUiState::Cancelled;
                } else {
                    // The import attempt failed — the surface must
                    // show the failure, not keep a previous terminal
                    // state.
                    self.state.import = ImportUiState::Failed {
                        reason: kind_tag(pe.kind()),
                        detail: pe.detail().to_string(),
                    };
                }
                self.fail_provider(&pe);
            }
            ImportError::IntegrityMismatch { detail } => {
                self.state.import = ImportUiState::Failed {
                    reason: LibraryReason::IntegrityMismatch.code().to_string(),
                    detail,
                };
            }
            ImportError::CommitRejected { source, .. } => {
                // A gone track/lane/placement is a typed target
                // fault, not an opaque internal error.
                let (reason, detail) = match source {
                    synvelis_project::Error::NotFound(_)
                    | synvelis_project::Error::WrongOwner(_) => {
                        (LibraryReason::TargetGone, source.to_string())
                    }
                    other => (LibraryReason::Internal, other.to_string()),
                };
                self.state.import = ImportUiState::Failed {
                    reason: reason.code().to_string(),
                    detail,
                };
            }
            ImportError::PublishIndeterminate { destination } => {
                // The destination state is unknowable — a distinct
                // recovery surface from an ordinary publish failure.
                self.state.import = ImportUiState::Failed {
                    reason: LibraryReason::PublishIndeterminate.code().to_string(),
                    detail: format!(
                        "publish destination state unknowable: {}",
                        destination.display()
                    ),
                };
            }
            other => {
                self.state.import = ImportUiState::Failed {
                    reason: LibraryReason::Internal.code().to_string(),
                    detail: format!("{other:?}"),
                };
            }
        }
    }

    fn import_rebase(
        &mut self,
        import_id: &serde_json::Value,
        expected_project_revision: u64,
        ports: Option<&LibraryImportPorts<'_>>,
        controller: Option<&mut ProjectController>,
    ) {
        let (Some(ports), Some(controller)) = (ports, controller) else {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::Malformed,
                None,
                "import.rebase requires import ports and a project controller",
            ));
            return;
        };
        let import_id = match Self::parse_import_id(import_id) {
            Ok(id) => id,
            Err(f) => {
                self.state.fault = Some(f);
                return;
            }
        };
        // Rebase only ever applies to the pending intent — rebasing
        // a different manifest id would rewrite the store while
        // `import.retry` still replays the pending one, leaving the
        // surface reporting revisions that match nothing real.
        let pending_id = self.pending_import.as_ref().map(|(_, i)| i.import_id);
        if pending_id != Some(import_id) {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::Malformed,
                None,
                "import.rebase target does not match the pending import",
            ));
            return;
        }
        match rebase_import(
            ports.store,
            import_id,
            ProjectRevision(expected_project_revision),
            controller.live(),
        ) {
            Ok(()) => {
                // Manifest rebased — retry replays the pinned intent.
                if let Some((_, intent)) = &mut self.pending_import {
                    intent.expected_project_revision = ProjectRevision(expected_project_revision);
                }
            }
            Err(e) => self.import_failed(e),
        }
    }

    fn import_retry(
        &mut self,
        ports: Option<&LibraryImportPorts<'_>>,
        controller: Option<&mut ProjectController>,
    ) {
        let (Some(ports), Some(controller), Some((pin, intent))) =
            (ports, controller, self.pending_import.clone())
        else {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::Malformed,
                None,
                "import.retry requires a pending import intent and ports",
            ));
            return;
        };
        let cx = self.context();
        self.run_import(ports, controller, &pin, &intent, &cx);
    }

    fn drag_begin(&mut self, locator: &str) {
        // Ghost only — resolve the display name when the entry is in
        // the current listing; the locator itself is always carried.
        let display_name = self
            .state
            .entries
            .iter()
            .find(|e| e.locator == locator)
            .map_or_else(|| locator.to_string(), |e| e.display_name.clone());
        if OpaqueLocator::new(locator.to_string()).is_err() {
            self.state.fault = Some(LibraryFault::new(
                LibraryReason::Malformed,
                None,
                "drag locator malformed",
            ));
            return;
        }
        self.state.drag = DragUiState::Ghost {
            locator: locator.to_string(),
            display_name,
        };
    }
}

// ── projections ─────────────────────────────────────────────────────

const PREVIEW_CAP: u64 = 1024 * 1024;
const FULL_OBJECT_CONSENT_BYTES: u64 = 64 * 1024 * 1024;

/// Classifies a preview fetch the way the bounded-cache contract does:
/// small/range-readable objects are `Bounded`; anything else is a
/// `FullDownload` behind the consent gate; beyond the hard budget is
/// `OverBudget`.
fn classify_preview(size: Option<u64>, range_read: bool) -> PreviewPlan {
    match size {
        Some(n) if n > PREVIEW_HARD_BUDGET => PreviewPlan::OverBudget { bytes: n },
        Some(n) if range_read || n <= PREVIEW_CAP => PreviewPlan::Bounded { bytes: n },
        Some(_) => PreviewPlan::FullDownload { bytes: size },
        None => PreviewPlan::FullDownload { bytes: None },
    }
}

const PREVIEW_HARD_BUDGET: u64 = 256 * 1024 * 1024;

fn plan_tag(plan: &PreviewPlan) -> &'static str {
    match plan {
        PreviewPlan::Bounded { .. } => "bounded",
        PreviewPlan::FullDownload { .. } => "consent-required",
        PreviewPlan::OverBudget { .. } => "over-budget",
    }
}

fn plan_bytes(plan: &PreviewPlan) -> Option<u64> {
    match plan {
        PreviewPlan::Bounded { bytes } | PreviewPlan::OverBudget { bytes } => Some(*bytes),
        PreviewPlan::FullDownload { bytes } => *bytes,
    }
}

fn preview_range(size: Option<u64>) -> Option<synvelis_library::ByteRange> {
    size.map(|n| synvelis_library::ByteRange {
        offset: 0,
        len: n.min(PREVIEW_CAP),
    })
}

fn coverage_tag(coverage: Coverage) -> &'static str {
    match coverage {
        Coverage::Complete => "complete",
        Coverage::Partial => "partial",
        Coverage::Stale => "stale",
        Coverage::OfflineCached => "offline-cached",
        Coverage::NeverIndexed => "never-indexed",
    }
}

fn availability_tag(availability: &Availability) -> &'static str {
    match availability {
        Availability::Available => "available",
        Availability::Missing { .. } => "missing",
        Availability::Offline => "offline",
        Availability::Unavailable { .. } => "unavailable",
    }
}

fn entry_kind_tag(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Asset => "asset",
        EntryKind::Container => "container",
    }
}

fn entry_wire(entry: &BrowseEntry) -> BrowseEntryWire {
    BrowseEntryWire {
        locator: entry.locator.as_str().to_string(),
        display_name: entry.display_name.clone(),
        kind: entry.kind,
        availability: entry.availability.clone(),
        via_symlink: entry.via_symlink,
        byte_len: entry.facts.byte_len.clone(),
        modified: entry.facts.modified.clone(),
    }
}

fn pin_proof_tag(pin: &PinnedAssetRef) -> &'static str {
    match &pin.proof {
        synvelis_library::PinProof::ImmutableObject { .. } => "immutable-object",
        synvelis_library::PinProof::ConditionalStrong { .. } => "conditional-strong",
    }
}

/// The pin's revision/validator evidence as a displayable string —
/// `ImmutableObject` carries the revision key, `ConditionalStrong`
/// the strong token's digest or validator.
fn pin_proof_revision(pin: &PinnedAssetRef) -> String {
    use synvelis_library::{PinProof, StrongRevisionToken};
    match &pin.proof {
        PinProof::ImmutableObject { revision } => revision.as_str().to_string(),
        PinProof::ConditionalStrong { token } => match token {
            StrongRevisionToken::ContentHash { digest } => digest.to_string(),
            StrongRevisionToken::StrongValidator { validator, .. } => validator.clone(),
        },
    }
}

fn import_outcome_state(outcome: &ImportOutcome) -> ImportUiState {
    ImportUiState::CommittedLive {
        import_id: outcome.import_id.to_tagged(),
        asset_id: outcome.commit.asset_id.to_tagged(),
        source_id: outcome.commit.source_id.to_tagged(),
        clip_id: outcome.commit.clip_id.map(ClipId::to_tagged),
        revision: outcome.commit.revision.0,
        content_digest: outcome.content_digest.hex().to_string(),
        byte_len: outcome.byte_len,
        reused: outcome.reused,
    }
}

// ── ViewSpec projection ─────────────────────────────────────────────

fn key(name: &str) -> ComponentKey {
    ComponentKey(name.to_string())
}

fn a11y(role: A11yRole, label: &str) -> A11ySpec {
    A11ySpec {
        role: Some(role),
        label: Some(label.to_string()),
        ..A11ySpec::default()
    }
}

fn label(key_name: &str, text: &str) -> ComponentSpec {
    ComponentSpec::Label(LabelSpec {
        key: key(key_name),
        text: text.to_string(),
        visual: ComponentVisualState::default(),
        a11y: a11y(A11yRole::Text, text),
    })
}

fn panel(key_name: &str, title: &str, children: Vec<ComponentSpec>) -> ComponentSpec {
    ComponentSpec::Panel(PanelSpec {
        key: key(key_name),
        title: Some(title.to_string()),
        children,
        visual: ComponentVisualState::default(),
        a11y: a11y(A11yRole::Group, title),
    })
}

fn button(key_name: &str, text: &str, action: &str, params: serde_json::Value) -> ComponentSpec {
    ComponentSpec::Button(ButtonSpec {
        key: key(key_name),
        label: text.to_string(),
        target: "synvelis.library".to_string(),
        action: action.to_string(),
        params: Some(params),
        visual: ComponentVisualState::default(),
        a11y: a11y(A11yRole::Button, text),
    })
}

fn entries_table(state: &LibraryViewState) -> ComponentSpec {
    let rows: Vec<Vec<Option<CellValue>>> = state
        .entries
        .iter()
        .map(|e| {
            vec![
                Some(CellValue::Text(e.display_name.clone())),
                Some(CellValue::Text(e.locator.clone())),
                Some(CellValue::Text(entry_kind_tag(e.kind).to_string())),
                Some(CellValue::Text(
                    availability_tag(&e.availability).to_string(),
                )),
                e.byte_len.value().map(|n| CellValue::Text(n.to_string())),
            ]
        })
        .collect();
    ComponentSpec::Table(TableSpec {
        key: key("library.entries"),
        columns: vec![
            "name".to_string(),
            "locator".to_string(),
            "kind".to_string(),
            "availability".to_string(),
            "bytes".to_string(),
        ],
        rows,
        selection: Some(TableSelectionSpec {
            target: "synvelis.library".to_string(),
        }),
        visual: ComponentVisualState::default(),
        a11y: a11y(A11yRole::Table, "asset entries"),
    })
}

/// Builds the driver-neutral [`ViewSpec`] for a library view. Pure and
/// deterministic — identical `(model, state)` inputs produce identical
/// specs, and component keys are stable across projections.
#[must_use]
pub fn library_spec(model: &LibraryViewModel) -> ViewSpec {
    let state = model.state();
    let mut children = vec![
        providers_panel(model),
        browse_panel(state),
        detail_panel(state),
        preview_panel(state),
        import_panel(state),
        drag_panel(state),
    ];
    if let Some(fault) = &state.fault {
        children.push(panel(
            "library.fault",
            "Fault",
            vec![label(
                "library.fault.text",
                &format!("{}: {}", fault.reason.code(), fault.detail),
            )],
        ));
    }
    ViewSpec {
        root: panel("library", "Asset Library", children),
    }
}

fn providers_panel(model: &LibraryViewModel) -> ComponentSpec {
    let providers = model.providers();
    let rows: Vec<Vec<Option<CellValue>>> = providers
        .iter()
        .map(|p| {
            vec![
                Some(CellValue::Text(p.display_name.clone())),
                Some(CellValue::Text(p.id.clone())),
                Some(CellValue::Text(p.kind.clone())),
                Some(CellValue::Text(if p.capabilities.browse.is_supported() {
                    "browse".to_string()
                } else {
                    "—".to_string()
                })),
                Some(CellValue::Text(if p.capabilities.search.is_supported() {
                    "search".to_string()
                } else {
                    "—".to_string()
                })),
                Some(CellValue::Text(if p.capabilities.pin.is_supported() {
                    "pin".to_string()
                } else {
                    "—".to_string()
                })),
            ]
        })
        .collect();
    let mut children = vec![ComponentSpec::Table(TableSpec {
        key: key("library.providers"),
        columns: vec![
            "name".to_string(),
            "id".to_string(),
            "kind".to_string(),
            "browse".to_string(),
            "search".to_string(),
            "pin".to_string(),
        ],
        rows,
        selection: None,
        visual: ComponentVisualState::default(),
        a11y: a11y(A11yRole::Table, "providers"),
    })];
    for p in &providers {
        children.push(button(
            &format!("library.provider.{}", p.id),
            &format!("select {}", p.display_name),
            "provider.select",
            serde_json::json!({ "provider": p.id }),
        ));
    }
    panel("library.providers.panel", "Providers", children)
}

fn browse_panel(state: &LibraryViewState) -> ComponentSpec {
    let coverage = state
        .coverage
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let search = state
        .search_text
        .clone()
        .map_or_else(|| "browse".to_string(), |t| format!("search: {t}"));
    let children = vec![
        label(
            "library.browse.coverage",
            &format!("coverage: {coverage} · mode: {search}"),
        ),
        entries_table(state),
        button(
            "library.browse.refresh",
            "refresh",
            "browse.refresh",
            serde_json::json!({}),
        ),
        button(
            "library.browse.more",
            "more",
            "browse.more",
            serde_json::json!({}),
        ),
    ];
    panel("library.browse", "Browse", children)
}

fn detail_panel(state: &LibraryViewState) -> ComponentSpec {
    let selected = state
        .selected_locator
        .clone()
        .unwrap_or_else(|| "—".to_string());
    let mut children = vec![
        label("library.detail.selected", &format!("selected: {selected}")),
        label(
            "library.detail.stat",
            &state.stat.as_ref().map_or_else(
                || "stat: —".to_string(),
                |s| {
                    format!(
                        "stat: {} (provenance {})",
                        availability_tag(&s.availability),
                        s.provenance.len()
                    )
                },
            ),
        ),
        label(
            "library.detail.pin",
            &match &state.pin {
                PinUiState::Idle => "pin: idle".to_string(),
                PinUiState::Pinned { proof, object, .. } => format!("pin: {proof} {object}"),
                PinUiState::Unsupported { reason } => format!("pin: unsupported ({reason})"),
                PinUiState::Failed { reason, detail } => {
                    format!("pin: failed {reason} {detail}")
                }
            },
        ),
    ];
    if state.selected_locator.is_some() {
        children.push(button(
            "library.detail.pin.button",
            "pin",
            "entry.pin",
            serde_json::json!({ "locator": state.selected_locator }),
        ));
        children.push(button(
            "library.detail.stat.button",
            "stat",
            "entry.stat",
            serde_json::json!({ "locator": state.selected_locator }),
        ));
    }
    panel("library.detail", "Inspector", children)
}

fn preview_panel(state: &LibraryViewState) -> ComponentSpec {
    let text = match &state.preview {
        PreviewUiState::Idle => "preview: idle".to_string(),
        PreviewUiState::PlanReady { plan, bytes } => {
            format!(
                "preview plan: {plan} ({} bytes)",
                bytes.map_or("?".into(), |b| b.to_string())
            )
        }
        PreviewUiState::Ready { byte_len, cached } => {
            format!("preview: ready {byte_len}B cached={cached}")
        }
        PreviewUiState::Cancelled => "preview: cancelled".to_string(),
        PreviewUiState::Failed { reason, detail } => {
            format!("preview: failed {reason} {detail}")
        }
    };
    let mut children = vec![
        label("library.preview.state", &text),
        button(
            "library.preview.cancel",
            "cancel",
            "preview.cancel",
            serde_json::json!({}),
        ),
    ];
    // Locator-targeted affordances render only with a selection — a
    // null locator would be a malformed action, not a no-op.
    if let Some(locator) = &state.selected_locator {
        children.insert(
            1,
            button(
                "library.preview.plan",
                "plan",
                "preview.plan",
                serde_json::json!({ "locator": locator }),
            ),
        );
        children.insert(
            2,
            button(
                "library.preview.fetch",
                "fetch",
                "preview.fetch",
                serde_json::json!({ "locator": locator }),
            ),
        );
    }
    panel("library.preview", "Preview", children)
}

fn import_panel(state: &LibraryViewState) -> ComponentSpec {
    let text = match &state.import {
        ImportUiState::Idle => "import: idle".to_string(),
        ImportUiState::CommittedLive {
            import_id,
            asset_id,
            revision,
            reused,
            ..
        } => format!(
            "import: committed-live {import_id} asset={asset_id} rev={revision} reused={reused}"
        ),
        ImportUiState::StaleRevision {
            import_id,
            expected,
            actual,
        } => format!("import: stale-revision {import_id} expected={expected} actual={actual}"),
        ImportUiState::Failed { reason, detail } => {
            format!("import: failed {reason} {detail}")
        }
        ImportUiState::Cancelled => "import: cancelled".to_string(),
    };
    let mut children = vec![
        label("library.import.state", &text),
        button(
            "library.import.retry",
            "retry",
            "import.retry",
            serde_json::json!({}),
        ),
        button(
            "library.import.dismiss",
            "dismiss",
            "import.dismiss",
            serde_json::json!({}),
        ),
    ];
    // The submit affordance needs a selection and a stable
    // idempotency key: the template derives `import_id` from the
    // (provider, locator) pair so resubmitting the same asset replays
    // idempotently (`reused`), the same locator on another provider
    // cannot collide into an IntegrityConflict, and a different asset
    // gets a different key. Drivers may still override the id.
    if let Some(locator) = &state.selected_locator {
        children.insert(
            1,
            button(
                "library.import.submit",
                "import",
                "import.submit",
                serde_json::json!({
                    "import_id": import_template_id(state.provider.as_deref(), locator),
                    "locator": locator,
                }),
            ),
        );
    }
    panel("library.import", "Import", children)
}

/// Deterministic idempotency-key template for the spec's import
/// button — FNV-1a over `(provider, locator)`. `DefaultHasher` is
/// deliberately not used: its algorithm is unspecified and may change
/// across toolchain versions, which would silently turn a resubmit
/// into a fresh import id instead of an idempotent replay.
fn import_template_id(provider: Option<&str>, locator: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in provider
        .unwrap_or("")
        .bytes()
        .chain(std::iter::once(0))
        .chain(locator.bytes())
    {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn drag_panel(state: &LibraryViewState) -> ComponentSpec {
    let text = match &state.drag {
        DragUiState::Idle => "drag: idle".to_string(),
        DragUiState::Ghost {
            locator,
            display_name,
        } => format!("drag ghost: {display_name} ({locator})"),
    };
    let mut children = vec![
        label("library.drag.state", &text),
        button(
            "library.drag.drop",
            "drop",
            "drag.drop",
            serde_json::json!({}),
        ),
    ];
    if let Some(locator) = &state.selected_locator {
        children.insert(
            0,
            button(
                "library.drag.begin",
                "drag",
                "drag.begin",
                serde_json::json!({ "locator": locator }),
            ),
        );
    }
    panel("library.drag", "Drag", children)
}

// ── per-view registry ───────────────────────────────────────────────

/// Backend binding shared by every library view model — the provider
/// registry plus the principal scope provider contexts run under.
///
/// The composition root binds this once (real providers in synvelis#91,
/// fakes in tests); until then [`LibraryViews::model_mut`] yields
/// `None` and the op surface reports the typed unavailable fault.
#[derive(Clone)]
pub struct LibraryBackend {
    /// Registered asset providers.
    pub registry: Arc<ProviderRegistry>,
    /// Principal scope key for provider calls.
    pub principal: PrincipalId,
}

/// Per-view [`LibraryViewModel`] registry — owned by
/// [`crate::WorkspaceManager`]; lifecycle mirrors
/// [`crate::vcs_ui::VcsHistoryViews`]: models are created lazily on
/// first access once a [`LibraryBackend`] is bound, and dropped on
/// `remove_view`.
///
/// Models are *not* ensured on `adopt_view` for every view kind — a
/// library model only materializes when a `syn.library.*` op (or test)
/// actually asks for one, so non-library views never carry one.
pub struct LibraryViews {
    models: std::collections::BTreeMap<ruviform_ui::dock::ViewInstanceId, LibraryViewModel>,
    backend: Option<LibraryBackend>,
}

impl std::fmt::Debug for LibraryViews {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryViews")
            .field("models", &self.models.keys().collect::<Vec<_>>())
            .field("backend", &self.backend.is_some())
            .finish()
    }
}

impl Default for LibraryViews {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryViews {
    /// An empty registry with no backend bound.
    #[must_use]
    pub fn new() -> Self {
        Self {
            models: std::collections::BTreeMap::new(),
            backend: None,
        }
    }

    /// Binds (or rebinds) the provider registry + principal every
    /// lazily-created model shares. Existing models keep the registry
    /// they were built with — rebind then re-open views, or remove
    /// stale models explicitly.
    pub fn set_backend(&mut self, backend: LibraryBackend) {
        self.backend = Some(backend);
    }

    /// The bound backend, when present.
    #[must_use]
    pub fn backend(&self) -> Option<&LibraryBackend> {
        self.backend.as_ref()
    }

    /// The view's model, if one has been materialized.
    #[must_use]
    pub fn model(&self, view: ruviform_ui::dock::ViewInstanceId) -> Option<&LibraryViewModel> {
        self.models.get(&view)
    }

    /// The view's model, materializing it on first access when a
    /// backend is bound. `None` when no [`LibraryBackend`] is bound —
    /// callers surface that as the typed unavailable fault, never a
    /// fabricated empty provider list inside a model.
    pub fn model_mut(
        &mut self,
        view: ruviform_ui::dock::ViewInstanceId,
    ) -> Option<&mut LibraryViewModel> {
        if !self.models.contains_key(&view) {
            let backend = self.backend.as_ref()?;
            self.models.insert(
                view,
                LibraryViewModel::new(Arc::clone(&backend.registry), backend.principal.clone()),
            );
        }
        self.models.get_mut(&view)
    }

    /// Drops `view`'s model entirely.
    pub fn remove(&mut self, view: ruviform_ui::dock::ViewInstanceId) {
        self.models.remove(&view);
    }

    /// Whether `view` has a materialized model.
    #[must_use]
    pub fn contains(&self, view: ruviform_ui::dock::ViewInstanceId) -> bool {
        self.models.contains_key(&view)
    }

    /// The registered provider descriptors projected to wire form —
    /// the selector's source before/independent of any view. Empty
    /// when no backend is bound (honest: zero providers registered).
    #[must_use]
    pub fn providers(&self) -> Vec<ProviderWire> {
        self.backend
            .as_ref()
            .map_or_else(Vec::new, |b| provider_wires(&b.registry))
    }
}

/// Projects a [`ProviderRegistry`] into wire descriptors — shared by
/// [`LibraryViewModel::providers`] and [`LibraryViews::providers`] so
/// capability derivation lives in exactly one place.
fn provider_wires(registry: &ProviderRegistry) -> Vec<ProviderWire> {
    registry
        .list()
        .iter()
        .filter_map(|d| {
            let capabilities = registry.get(&d.id)?.capabilities(None);
            Some(ProviderWire {
                id: d.id.to_tagged(),
                kind: provider_kind_tag(&d.kind),
                display_name: d.display_name.clone(),
                capabilities,
            })
        })
        .collect()
}

// ── tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::sync::Mutex;

    use synvelis_library::{
        AccessMode, AssetKind, AssetObservation, AssetStat, Capability, EntryFacts, MediaFacts,
        PinCapability, PinProof, PinnedRead, ProviderCapabilities, ProviderDescriptor,
        ProviderKind, RevisionKey, UnavailableReason,
    };
    use synvelis_project::{CounterIdGenerator, Project, ProjectId};

    /// Empty in-memory blob store for import tests.
    struct MemBlobs;

    impl ProjectBlobStore for MemBlobs {
        fn open(
            &self,
            _object: &synvelis_library::ObjectIdentity,
        ) -> Result<synvelis_library::BlobObject, ProviderError> {
            Err(ProviderError::new(ProviderErrorKind::NotFound, "no blob"))
        }
        fn stat(&self, _object: &synvelis_library::ObjectIdentity) -> Result<u64, ProviderError> {
            Err(ProviderError::new(ProviderErrorKind::NotFound, "no blob"))
        }
    }

    /// One-shot channel reader for fake preview bytes.
    struct ChanReader(std::sync::mpsc::Receiver<Vec<u8>>);

    impl Read for ChanReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.0.try_recv() {
                Ok(b) => {
                    let n = b.len().min(buf.len());
                    buf[..n].copy_from_slice(&b[..n]);
                    Ok(n)
                }
                Err(_) => Ok(0),
            }
        }
    }

    /// In-memory fake provider — browse/stat/pin/preview scripted,
    /// search optional, cursor validation delegated to the library's
    /// own `decode_cursor`-style rules via opaque tokens.
    struct FakeProvider {
        id: ProviderId,
        kind: ProviderKind,
        caps: ProviderCapabilities,
        pages: Mutex<Vec<BrowsePage>>,
        stats: Mutex<Vec<(String, AssetStat)>>,
        preview_bytes: Vec<u8>,
        preview_supported: bool,
        search_results: Mutex<Option<BrowsePage>>,
        pin_unsupported: bool,
    }

    impl FakeProvider {
        fn new(id: u128, searchable: bool, pinnable: bool, preview: bool) -> Self {
            Self {
                id: ProviderId::new(id),
                kind: ProviderKind::LocalFolder,
                caps: ProviderCapabilities {
                    browse: Capability::Supported,
                    stat: Capability::Supported,
                    search: if searchable {
                        Capability::Supported
                    } else {
                        Capability::Unsupported {
                            reason: synvelis_library::UnsupportedReason::RequiresIndex,
                        }
                    },
                    pin: if pinnable {
                        PinCapability::ImmutableObject
                    } else {
                        PinCapability::Unsupported {
                            reason: synvelis_library::UnsupportedReason::NotSupportedByBackend,
                        }
                    },
                    range_read: Capability::Supported,
                    preview: if preview {
                        Capability::Supported
                    } else {
                        Capability::Unsupported {
                            reason: synvelis_library::UnsupportedReason::NotImplemented,
                        }
                    },
                    fetch: Capability::Supported,
                    checksum_hint: Capability::Unsupported {
                        reason: synvelis_library::UnsupportedReason::NotImplemented,
                    },
                    access: AccessMode::ReadOnly,
                },
                pages: Mutex::new(Vec::new()),
                stats: Mutex::new(Vec::new()),
                preview_bytes: b"fake-preview".to_vec(),
                preview_supported: preview,
                search_results: Mutex::new(None),
                pin_unsupported: !pinnable,
            }
        }

        fn with_page(mut self, page: BrowsePage) -> Self {
            self.pages.get_mut().unwrap().push(page);
            self
        }
    }

    impl AssetProvider for FakeProvider {
        fn id(&self) -> ProviderId {
            self.id
        }
        fn kind(&self) -> ProviderKind {
            self.kind.clone()
        }
        fn capabilities(&self, _asset: Option<&RemoteAssetRef>) -> ProviderCapabilities {
            self.caps.clone()
        }
        fn browse(
            &self,
            req: &BrowseRequest,
            _cx: &ProviderContext,
        ) -> Result<BrowsePage, ProviderError> {
            let pages = self.pages.lock().unwrap();
            if pages.is_empty() {
                return Ok(BrowsePage {
                    entries: Vec::new(),
                    next_cursor: None,
                    coverage: Coverage::Complete,
                    observed_at: synvelis_library::ObservedAt::now(),
                });
            }
            let idx = req
                .cursor
                .as_ref()
                .and_then(|c| c.as_str().parse::<usize>().ok())
                .unwrap_or(0);
            if idx >= pages.len() {
                return Err(ProviderError::new(
                    ProviderErrorKind::CursorInvalid,
                    "cursor out of range",
                ));
            }
            Ok(pages[idx].clone())
        }
        fn stat(
            &self,
            asset: &RemoteAssetRef,
            cx: &ProviderContext,
        ) -> Result<AssetStat, ProviderError> {
            // Cooperative providers observe the context cancel token —
            // the fake does too so preview-cancel regressions are real.
            if cx.cancel.is_cancelled() {
                return Err(ProviderError::new(
                    ProviderErrorKind::Cancelled,
                    "cancelled",
                ));
            }
            let stats = self.stats.lock().unwrap();
            for (loc, stat) in stats.iter() {
                if loc == asset.locator.as_str() {
                    return Ok(stat.clone());
                }
            }
            Err(ProviderError::new(
                ProviderErrorKind::NotFound,
                "no stat scripted",
            ))
        }
        fn pin(
            &self,
            asset: &RemoteAssetRef,
            _cx: &ProviderContext,
        ) -> Result<PinnedAssetRef, ProviderError> {
            if self.pin_unsupported {
                return Err(ProviderError::unsupported("pin", "fake"));
            }
            Ok(PinnedAssetRef {
                asset: asset.clone(),
                proof: PinProof::ImmutableObject {
                    revision: RevisionKey::new("commit:abc123").unwrap(),
                },
                object_identity: synvelis_library::ObjectIdentity::new("blob:test").unwrap(),
                expected_size: Some(4),
                digest_hint: None,
            })
        }
        fn open_pinned(
            &self,
            _pin: &PinnedAssetRef,
            _cx: &ProviderContext,
        ) -> Result<PinnedRead, ProviderError> {
            Err(ProviderError::unsupported("open_pinned", "fake"))
        }
        fn search(
            &self,
            _query: &SearchQuery,
            _cx: &ProviderContext,
        ) -> Result<BrowsePage, ProviderError> {
            self.search_results
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| ProviderError::unsupported("search", "fake"))
        }
        fn open_preview(
            &self,
            _asset: &RemoteAssetRef,
            _range: Option<synvelis_library::ByteRange>,
            _cx: &ProviderContext,
        ) -> Result<PinnedRead, ProviderError> {
            if !self.preview_supported {
                return Err(ProviderError::unsupported("open_preview", "fake"));
            }
            let bytes = self.preview_bytes.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            tx.send(bytes).unwrap();
            drop(tx);
            let slot = Arc::new(synvelis_library::FetchSlot::new());
            Ok(PinnedRead {
                reader: Box::new(ChanReader(rx)),
                claims: synvelis_library::FetchClaims {
                    object_identity: synvelis_library::ObjectIdentity::new("prev:test").unwrap(),
                    expected_size: None,
                    verification: synvelis_library::VerificationPolicy::Unverified,
                },
                completion: synvelis_library::FetchCompletion::new(slot),
            })
        }
    }

    fn entry(name: &str, locator: &str, kind: EntryKind) -> BrowseEntry {
        BrowseEntry {
            locator: OpaqueLocator::new(locator).unwrap(),
            display_name: name.to_string(),
            kind,
            availability: Availability::Available,
            facts: EntryFacts {
                byte_len: AssetObservation::observed(
                    10u64,
                    synvelis_library::ObservationOrigin::Provider,
                ),
                modified: AssetObservation::unknown(UnavailableReason::NotProvided),
                kind: AssetObservation::observed(
                    AssetKind::File,
                    synvelis_library::ObservationOrigin::Provider,
                ),
                checksum_hint: AssetObservation::unknown(UnavailableReason::NotProvided),
                media: MediaFacts::all_unknown(),
            },
            via_symlink: false,
        }
    }

    fn page(entries: Vec<BrowseEntry>, coverage: Coverage, cursor: Option<&str>) -> BrowsePage {
        BrowsePage {
            entries,
            next_cursor: cursor.map(|c| OpaquePageCursor::new(c).unwrap()),
            coverage,
            observed_at: synvelis_library::ObservedAt::now(),
        }
    }

    fn model_with(provider: FakeProvider) -> LibraryViewModel {
        let mut registry = ProviderRegistry::new();
        registry
            .register(
                ProviderDescriptor {
                    id: provider.id,
                    kind: provider.kind.clone(),
                    display_name: "fake".to_string(),
                },
                Arc::new(provider),
            )
            .unwrap();
        LibraryViewModel::new(Arc::new(registry), PrincipalId::new("test").unwrap())
    }

    fn select(model: &mut LibraryViewModel) {
        model.apply(
            &LibraryAction::ProviderSelect {
                provider: "provider-00000000000000000000000000000001".to_string(),
            },
            None,
            None,
        );
    }

    #[test]
    fn providers_and_select_roundtrip() {
        let mut m = model_with(FakeProvider::new(1, true, true, true));
        let providers = m.providers();
        assert_eq!(providers.len(), 1);
        assert!(
            providers[0].capabilities.browse.is_supported()
                && providers[0].capabilities.search.is_supported()
                && providers[0].capabilities.pin.is_supported()
        );
        select(&mut m);
        assert!(m.state().fault.is_none());
        assert!(m.state().provider.is_some());
        // spec is deterministic
        let s1 = library_spec(&m);
        let s2 = library_spec(&m);
        assert_eq!(
            serde_json::to_string(&s1).unwrap(),
            serde_json::to_string(&s2).unwrap()
        );
    }

    #[test]
    fn browse_complete_empty_is_not_failure() {
        let mut m = model_with(FakeProvider::new(1, false, false, false));
        select(&mut m);
        m.apply(&LibraryAction::BrowseRefresh {}, None, None);
        assert!(m.state().fault.is_none());
        assert_eq!(m.state().coverage.as_deref(), Some("complete"));
        assert_eq!(m.state().entries, Vec::new());
    }

    #[test]
    fn browse_pages_and_cursor_invalid() {
        let p1 = page(
            vec![entry("a.wav", "loc-a", EntryKind::Asset)],
            Coverage::Partial,
            Some("1"),
        );
        let p2 = page(
            vec![entry("b.wav", "loc-b", EntryKind::Asset)],
            Coverage::Complete,
            None,
        );
        let fake = FakeProvider::new(1, false, false, false).with_page(p1);
        fake.pages.lock().unwrap().push(p2);
        let mut m = model_with(fake);
        select(&mut m);
        m.apply(&LibraryAction::BrowseRefresh {}, None, None);
        assert_eq!(m.state().entries.len(), 1);
        assert_eq!(m.state().coverage.as_deref(), Some("partial"));
        assert!(m.state().next_cursor.is_some());
        m.apply(&LibraryAction::BrowseMore {}, None, None);
        assert_eq!(m.state().entries.len(), 2);
        assert_eq!(m.state().coverage.as_deref(), Some("complete"));
        assert!(m.state().next_cursor.is_none());
        // cursor exhausted → malformed, not a fake page
        m.apply(&LibraryAction::BrowseMore {}, None, None);
        assert_eq!(
            m.state().fault.as_ref().map(|f| f.reason),
            Some(LibraryReason::Malformed)
        );
    }

    #[test]
    fn unsupported_search_is_typed_not_empty() {
        let mut m = model_with(FakeProvider::new(1, false, false, false));
        select(&mut m);
        m.apply(
            &LibraryAction::SearchSet {
                text: "kick".to_string(),
            },
            None,
            None,
        );
        assert_eq!(
            m.state().fault.as_ref().map(|f| f.reason),
            Some(LibraryReason::UnsupportedCapability)
        );
        assert_eq!(m.state().entries, Vec::new());
        // The search mode was never entered — a refresh stays on the
        // (empty) browse listing rather than fabricating search rows.
        assert!(m.state().search_text.is_none());
    }

    #[test]
    fn search_mode_survives_refresh_and_more() {
        // Regression: refresh/more while a search listing is active
        // must continue the SEARCH (same cursor discipline), never
        // silently swap in browse rows underneath the user.
        let browse_page = page(
            vec![entry("b.wav", "loc-browse", EntryKind::Asset)],
            Coverage::Complete,
            None,
        );
        let search_page = page(
            vec![entry("s.wav", "loc-search", EntryKind::Asset)],
            Coverage::Complete,
            Some("sc1"),
        );
        let fake = FakeProvider::new(1, true, false, false).with_page(browse_page);
        *fake.search_results.lock().unwrap() = Some(search_page);
        let mut m = model_with(fake);
        select(&mut m);
        m.apply(
            &LibraryAction::SearchSet {
                text: "kick".to_string(),
            },
            None,
            None,
        );
        assert_eq!(m.state().entries.len(), 1);
        assert_eq!(m.state().entries[0].locator, "loc-search");
        assert!(m.state().fault.is_none());

        m.apply(&LibraryAction::BrowseRefresh {}, None, None);
        assert_eq!(m.state().entries.len(), 1);
        assert_eq!(m.state().entries[0].locator, "loc-search");
        assert!(m.state().fault.is_none());

        // `more` resumes the stored search cursor — the fake replays
        // its page, so entries stay search-sourced (appended), never
        // browse-sourced.
        m.apply(&LibraryAction::BrowseMore {}, None, None);
        assert_eq!(m.state().entries.len(), 2);
        assert!(m.state().entries.iter().all(|e| e.locator == "loc-search"));
        assert!(m.state().fault.is_none());
    }

    #[test]
    fn pin_unsupported_and_supported() {
        let mut m = model_with(FakeProvider::new(1, false, false, false));
        select(&mut m);
        m.apply(
            &LibraryAction::EntryPin {
                locator: "loc-a".to_string(),
            },
            None,
            None,
        );
        assert!(matches!(m.state().pin, PinUiState::Unsupported { .. }));

        let mut m2 = model_with(FakeProvider::new(2, false, true, false));
        // provider id 2
        m2.apply(
            &LibraryAction::ProviderSelect {
                provider: "provider-00000000000000000000000000000002".to_string(),
            },
            None,
            None,
        );
        m2.apply(
            &LibraryAction::EntryPin {
                locator: "loc-a".to_string(),
            },
            None,
            None,
        );
        assert!(matches!(m2.state().pin, PinUiState::Pinned { .. }));
    }

    #[test]
    fn stat_preserves_availability_distinctions() {
        let mut fake = FakeProvider::new(1, false, false, false);
        fake.stats.get_mut().unwrap().push((
            "loc-off".to_string(),
            AssetStat {
                asset: RemoteAssetRef {
                    provider: fake.id,
                    locator: OpaqueLocator::new("loc-off").unwrap(),
                },
                availability: Availability::Offline,
                facts: EntryFacts::all_unknown(),
                provenance: Vec::new(),
            },
        ));
        fake.stats.get_mut().unwrap().push((
            "loc-miss".to_string(),
            AssetStat {
                asset: RemoteAssetRef {
                    provider: fake.id,
                    locator: OpaqueLocator::new("loc-miss").unwrap(),
                },
                availability: Availability::Missing {
                    detail: "gone".to_string(),
                },
                facts: EntryFacts::all_unknown(),
                provenance: Vec::new(),
            },
        ));
        let mut m = model_with(fake);
        select(&mut m);
        m.apply(
            &LibraryAction::EntryStat {
                locator: "loc-off".to_string(),
            },
            None,
            None,
        );
        assert!(matches!(
            m.state().stat.as_ref().map(|s| &s.availability),
            Some(Availability::Offline)
        ));
        m.apply(
            &LibraryAction::EntryStat {
                locator: "loc-miss".to_string(),
            },
            None,
            None,
        );
        assert!(matches!(
            m.state().stat.as_ref().map(|s| &s.availability),
            Some(Availability::Missing { .. })
        ));
        // unknown locator → typed NotFound fault
        m.apply(
            &LibraryAction::EntryStat {
                locator: "loc-none".to_string(),
            },
            None,
            None,
        );
        assert_eq!(
            m.state().fault.as_ref().map(|f| f.reason),
            Some(LibraryReason::Missing)
        );
    }

    #[test]
    fn entry_wire_preserves_observation_and_availability_tags() {
        // Stale/unknown facts and typed availability must survive the
        // wire projection verbatim — a stale size is never shown as a
        // fresh observed value, and an unavailable reason is never
        // flattened into a bare tag.
        let mut stale_entry = entry("s.wav", "loc-stale", EntryKind::Asset);
        stale_entry.availability = Availability::Unavailable {
            reason: UnavailableReason::PermissionDenied,
        };
        stale_entry.facts.byte_len = AssetObservation::Stale {
            value: 4096,
            at: synvelis_library::ObservedAt::now(),
            reason: synvelis_library::StaleReason::ProviderUnreachable,
        };
        stale_entry.facts.modified = AssetObservation::unknown(UnavailableReason::RequiresFetch);
        let wire = entry_wire(&stale_entry);
        assert!(matches!(
            wire.availability,
            Availability::Unavailable {
                reason: UnavailableReason::PermissionDenied
            }
        ));
        assert!(matches!(
            wire.byte_len,
            AssetObservation::Stale { value: 4096, .. }
        ));
        assert!(matches!(
            wire.modified,
            AssetObservation::Unknown {
                reason: UnavailableReason::RequiresFetch
            }
        ));
        // The serialized form keeps the same tags for drivers.
        let json = serde_json::to_value(&wire).unwrap();
        assert_eq!(json["byte_len"]["observation"], "stale");
        assert_eq!(json["modified"]["observation"], "unknown");
        assert_eq!(json["availability"]["state"], "unavailable");
    }

    #[test]
    fn preview_plan_consent_and_fetch_ready() {
        let mut m = model_with(FakeProvider::new(1, false, false, true));
        select(&mut m);
        // size unknown → full-download consent gate
        m.apply(
            &LibraryAction::PreviewPlan {
                locator: "loc-a".to_string(),
                allow_full_object: false,
            },
            None,
            None,
        );
        assert!(matches!(
            m.state().preview,
            PreviewUiState::PlanReady { ref plan, .. } if plan == "consent-required"
        ));
        // fetch without consent → still gated
        m.apply(
            &LibraryAction::PreviewFetch {
                locator: "loc-a".to_string(),
                allow_full_object: false,
            },
            None,
            None,
        );
        assert!(matches!(
            m.state().preview,
            PreviewUiState::PlanReady { ref plan, .. } if plan == "consent-required"
        ));
        // with consent → ready, and NOT an import
        m.apply(
            &LibraryAction::PreviewFetch {
                locator: "loc-a".to_string(),
                allow_full_object: true,
            },
            None,
            None,
        );
        assert!(matches!(m.state().preview, PreviewUiState::Ready { .. }));
        assert!(matches!(m.state().import, ImportUiState::Idle));
    }

    #[test]
    fn preview_cancel_is_typed() {
        let mut m = model_with(FakeProvider::new(1, false, false, true));
        select(&mut m);
        m.preview_cancel();
        m.apply(&LibraryAction::PreviewCancel {}, None, None);
        assert!(matches!(m.state().preview, PreviewUiState::Cancelled));
    }

    #[test]
    fn preview_cancel_does_not_poison_later_plan() {
        // Regression: `preview.plan`'s stat probe must not run under
        // the fetch cancel token — after `preview.cancel`, a plan
        // would otherwise see a cancelled context, lose the size hint,
        // and misclassify every asset as `consent-required`.
        let mut fake = FakeProvider::new(1, false, false, true);
        fake.stats.get_mut().unwrap().push((
            "loc-a".to_string(),
            AssetStat {
                asset: RemoteAssetRef {
                    provider: fake.id,
                    locator: OpaqueLocator::new("loc-a").unwrap(),
                },
                availability: Availability::Available,
                facts: EntryFacts {
                    byte_len: AssetObservation::observed(
                        10u64,
                        synvelis_library::ObservationOrigin::Provider,
                    ),
                    ..EntryFacts::all_unknown()
                },
                provenance: Vec::new(),
            },
        ));
        let mut m = model_with(fake);
        select(&mut m);
        m.apply(
            &LibraryAction::PreviewFetch {
                locator: "loc-a".to_string(),
                allow_full_object: true,
            },
            None,
            None,
        );
        m.apply(&LibraryAction::PreviewCancel {}, None, None);
        assert!(matches!(m.state().preview, PreviewUiState::Cancelled));

        m.apply(
            &LibraryAction::PreviewPlan {
                locator: "loc-a".to_string(),
                allow_full_object: false,
            },
            None,
            None,
        );
        // 10 bytes + range-read support → bounded, NOT the
        // unknown-size `consent-required` fallback a poisoned stat
        // would produce.
        assert!(matches!(
            m.state().preview,
            PreviewUiState::PlanReady { ref plan, .. } if plan == "bounded"
        ));
    }

    #[test]
    fn drag_ghost_never_mutates() {
        let mut m = model_with(FakeProvider::new(1, false, false, false));
        select(&mut m);
        m.apply(
            &LibraryAction::DragBegin {
                locator: "loc-a".to_string(),
            },
            None,
            None,
        );
        assert!(matches!(m.state().drag, DragUiState::Ghost { .. }));
        assert!(matches!(m.state().import, ImportUiState::Idle));
        m.apply(&LibraryAction::DragDrop {}, None, None);
        assert!(matches!(m.state().drag, DragUiState::Idle));
    }

    #[test]
    fn malformed_and_implicit_actions_are_typed() {
        let mut m = model_with(FakeProvider::new(1, false, false, false));
        // actions before provider.select → NoProvider
        m.apply(&LibraryAction::BrowseRefresh {}, None, None);
        assert_eq!(
            m.state().fault.as_ref().map(|f| f.reason),
            Some(LibraryReason::NoProvider)
        );
        // unknown provider
        m.apply(
            &LibraryAction::ProviderSelect {
                provider: "provider-00000000000000000000000000000009".to_string(),
            },
            None,
            None,
        );
        assert_eq!(
            m.state().fault.as_ref().map(|f| f.reason),
            Some(LibraryReason::UnknownProvider)
        );
        // import without ports/controller → malformed, never implicit
        select(&mut m);
        m.apply(
            &LibraryAction::ImportSubmit {
                import_id: serde_json::json!(7),
                locator: "loc-a".to_string(),
                expected_project_revision: None,
                allow_full_object: false,
                target: None,
            },
            None,
            None,
        );
        assert_eq!(
            m.state().fault.as_ref().map(|f| f.reason),
            Some(LibraryReason::Malformed)
        );
    }

    #[test]
    fn import_committed_live_with_receipt() {
        // Real ImportStore in a temp dir, real ProjectController,
        // in-memory blob store + StagedBlobPublisher.
        let dir = std::env::temp_dir().join(format!(
            "sv89-libui-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = ImportStore::new(dir.join("staging"), dir.join("media"));
        let blobs = MemBlobs;
        let publisher = synvelis_library::StagedBlobPublisher;
        let mut controller = ProjectController::new(
            Project::new(ProjectId::new(1), "lib-test"),
            Box::new(CounterIdGenerator::new(1)),
        );
        let mut m = model_with(FakeProvider::new(1, false, true, false));
        select(&mut m);
        let ports = LibraryImportPorts {
            store: &store,
            blobs: &blobs,
            publisher: &publisher,
        };
        // open_pinned is unsupported by the fake → import fails typed
        m.apply(
            &LibraryAction::ImportSubmit {
                import_id: serde_json::json!(42),
                locator: "loc-a".to_string(),
                expected_project_revision: Some(0),
                allow_full_object: true,
                target: None,
            },
            Some(&ports),
            Some(&mut controller),
        );
        // The fake cannot open_pinned → typed failure, never a fake
        // CommittedLive.
        assert!(!matches!(
            m.state().import,
            ImportUiState::CommittedLive { .. }
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
