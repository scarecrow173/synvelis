//! Scoped context query model and projection types (GW-03, GTC-004).
//!
//! A [`ContextQuery`] is the typed form of a validated context-read
//! request: an explicit [`ResourceSelector`], a [`FieldSelection`]
//! classified by [`DataAccessClass`], an optional range window, an
//! optional page request, and the request's own consistency pin.
//! [`ContextProjection`] is what the engine emits — a snapshot-revision-
//! fixed page of [`ProjectedItem`]s whose completeness reports
//! `complete` / `unknown` / `unavailable` honestly.
//!
//! # The two boundaries
//!
//! Preflight read permission and external disclosure permission are
//! **separate** evaluations (GTC-004 / GT-004):
//!
//! - *Preflight* (`AuthorizationStage::ReadContext`) proves the caller
//!   may run the query at all — task/grant binding, capability and
//!   permission coverage, and the descriptor's `data_access` floor.
//! - *Disclosure* ([`ContextAuthorizer::authorize_disclosure`]) is a
//!   second boundary evaluated on the data actually emitted: every
//!   emitted class must be covered by the grant, and an external caller
//!   (agent principal or external adapter) must additionally hold
//!   [`DataAccessClass::ExternalDisclosure`]. Internal validation reads
//!   never pass through the disclosure boundary — a resource the
//!   engine resolves internally is not thereby disclosed.
//!
//! # What the projection preserves
//!
//! - Source-native time: MIDI notes carry `MidiTick` values and the
//!   source's `MidiDivision` verbatim — never requantized.
//! - Content hash/version: `Source::content_digest` / `SourceState` /
//!   `Asset` digest are emitted as stored.
//! - Explicit sharing: a clip's `source` ref names its source id; the
//!   source's own data requires its own scope coverage — a clip read
//!   never implies a source read (SEC-05).
//! - Workspace selection is an *output* reached only through an
//!   explicitly named [`SelectionViewRef`] plus the
//!   [`DataAccessClass::WorkspaceSelectionRead`] class — UI selection
//!   is never an implicit input and never alters request or grant
//!   targets (SEC-07/AHG-006).
//!
//! The engine behind this model lives in [`crate::context_engine`];
//! the snapshot seam it reads through lives in [`crate::snapshot`].

use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;
use synvelis_project::{ProjectId, ProjectRevision};

use crate::capability::DataAccessClass;
use crate::id::{CursorRef, SelectionViewRef, SnapshotRef};
use crate::outcome::{ErrorEnvelope, GatewayError};
use crate::request::{
    OperationRequest, OperationTarget, ResourceKind, ResourceRef, TimeDomain, TypedRange,
};

/// A disclosed field group — each maps to exactly one
/// [`DataAccessClass`], which is what the disclosure boundary checks.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FieldGroup {
    /// Identity and structure — ids, kinds, owners, names, placements,
    /// source refs, divisions, digests, and per-collection counts.
    Metadata,
    /// Structured content payloads — MIDI notes and clip-local notes.
    Content,
    /// Media payload descriptors — bounded references (asset, digest,
    /// state, native window); never the bytes themselves.
    Media,
    /// Derived data (analysis features, computed fingerprints).
    Derived,
    /// Workspace selection state through a named view.
    Selection,
}

impl FieldGroup {
    /// The wire name used in `fields` arrays and emitted group keys.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Content => "content",
            Self::Media => "media",
            Self::Derived => "derived",
            Self::Selection => "selection",
        }
    }

    /// The data-access class this group's emission is classified under.
    #[must_use]
    pub const fn data_class(&self) -> DataAccessClass {
        match self {
            Self::Metadata => DataAccessClass::MetadataRead,
            Self::Content => DataAccessClass::StructuredContentRead,
            Self::Media => DataAccessClass::MediaBytesRead,
            Self::Derived => DataAccessClass::DerivedDataRead,
            Self::Selection => DataAccessClass::WorkspaceSelectionRead,
        }
    }

    /// Parses a wire `fields` entry.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "metadata" => Some(Self::Metadata),
            "content" => Some(Self::Content),
            "media" => Some(Self::Media),
            "derived" => Some(Self::Derived),
            "selection" => Some(Self::Selection),
            _ => None,
        }
    }

    /// Every group the contract defines — for schema declarations.
    #[must_use]
    pub fn all_names() -> Vec<String> {
        [
            Self::Metadata,
            Self::Content,
            Self::Media,
            Self::Derived,
            Self::Selection,
        ]
        .iter()
        .map(|g| g.name().to_owned())
        .collect()
    }
}

/// The requested field groups. `Metadata` is always present — every
/// projected item carries at least its identity, so `MetadataRead` is
/// the emission floor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldSelection(pub(crate) BTreeSet<FieldGroup>);

impl FieldSelection {
    /// The groups that will be emitted (metadata is always included).
    #[must_use]
    pub fn groups(&self) -> &BTreeSet<FieldGroup> {
        &self.0
    }

    /// The data-access classes this selection emits — the disclosure
    /// boundary requires each to be grant-covered.
    #[must_use]
    pub fn classes(&self) -> BTreeSet<DataAccessClass> {
        self.0.iter().map(FieldGroup::data_class).collect()
    }

    /// Whether `group` was requested.
    #[must_use]
    pub fn wants(&self, group: FieldGroup) -> bool {
        self.0.contains(&group)
    }
}

/// An explicit typed resource selector (GTC-004).
///
/// Enumeration selectors ([`Kind`][Self::Kind] / [`All`][Self::All])
/// yield only scope-admitted members — coverage is a *filter* for
/// enumeration and a *verdict* for an explicit id list (existence-blind:
/// an unlisted id denies like an absent one).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceSelector {
    /// The project aggregate itself.
    Project,
    /// An explicit list of tagged resource ids (deduplicated, order
    /// preserved).
    Resources(Vec<ResourceRef>),
    /// Every resource of one kind in the pinned snapshot.
    Kind(ResourceKind),
    /// Every projectable resource in the pinned snapshot.
    All,
}

/// Page request shape — first page or a cursor continuation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageRequest {
    /// First page of a new pagination chain.
    First {
        /// Requested page size; `None` selects the engine default.
        limit: Option<u32>,
    },
    /// Continuation of an existing chain. Query parameters are pinned
    /// by the cursor — a continuation names only the token.
    Continuation {
        /// The opaque cursor token.
        cursor: CursorRef,
    },
}

/// The typed context query — parsed from a bound [`OperationRequest`]'s
/// validated args.
///
/// The wire's `target` is [`OperationTarget::None`]: the request's
/// `project` field is the explicit project identity, and the selector
/// inside `args` names the resources — both are explicit and typed
/// (GT-001), and neither consults UI state.
#[derive(Clone, Debug)]
pub struct ContextQuery {
    /// Which resources to project.
    pub selector: ResourceSelector,
    /// Which field groups to emit.
    pub fields: FieldSelection,
    /// Optional window filter — items with a matching-domain extent
    /// must intersect it; MIDI note content is filtered to it.
    pub range: Option<TypedRange>,
    /// The named selection view — required iff `fields` contains
    /// [`FieldGroup::Selection`].
    pub selection_view: Option<SelectionViewRef>,
    /// First page or cursor continuation.
    pub page: PageRequest,
}

fn violation(path: &str, reason: impl Into<String>) -> ErrorEnvelope {
    GatewayError::SchemaViolation {
        path: path.to_owned(),
        reason: reason.into(),
    }
    .into()
}

#[allow(clippy::result_large_err)] // typed envelope is the contract
impl ContextQuery {
    /// Parses the validated args of a bound context-query request.
    ///
    /// Semantic checks beyond the schema: exactly one selector form,
    /// `selection` requires `selection_view` (and vice versa), `limit`
    /// and `cursor` are exclusive, and a continuation carries no query
    /// parameters (they are pinned inside the cursor record).
    ///
    /// # Errors
    /// [`GatewayError::SchemaViolation`] or [`GatewayError::InvalidId`]
    /// on malformed input.
    pub fn parse(req: &OperationRequest) -> Result<Self, ErrorEnvelope> {
        debug_assert!(matches!(req.target(), OperationTarget::None));
        let args = req.args();
        let Some(selector_value) = args.get("selector") else {
            // A continuation carries only `page` — its selector is in
            // the cursor record, not the wire args.
            return Self::parse_continuation(req, args);
        };
        let selector = Self::parse_selector(selector_value)?;

        let mut groups = BTreeSet::from([FieldGroup::Metadata]);
        if let Some(fields) = args.get("fields") {
            let arr = fields
                .as_array()
                .ok_or_else(|| violation("args.fields", "expected an array"))?;
            for (i, v) in arr.iter().enumerate() {
                let name = v
                    .as_str()
                    .ok_or_else(|| violation(&format!("args.fields[{i}]"), "expected a string"))?;
                let Some(group) = FieldGroup::parse(name) else {
                    return Err(violation(
                        &format!("args.fields[{i}]"),
                        format!("unknown field group: {name}"),
                    ));
                };
                groups.insert(group);
            }
        }
        let fields = FieldSelection(groups);

        let selection_view = match args.get("selection_view") {
            Some(v) => Some(
                SelectionViewRef::new(
                    v.as_str()
                        .ok_or_else(|| violation("args.selection_view", "expected a string"))?,
                )
                .map_err(ErrorEnvelope::from)?,
            ),
            None => None,
        };
        match (fields.wants(FieldGroup::Selection), &selection_view) {
            (true, None) => {
                return Err(violation(
                    "args.selection_view",
                    "selection field requires a named selection view",
                ));
            }
            (false, Some(_)) => {
                return Err(violation(
                    "args.selection_view",
                    "selection view requires the selection field",
                ));
            }
            _ => {}
        }

        let range = match args.get("range") {
            Some(v) => Some(Self::parse_range(v)?),
            None => None,
        };

        let page = match args.get("page") {
            Some(v) => Self::parse_page(v)?,
            None => PageRequest::First { limit: None },
        };
        if matches!(page, PageRequest::Continuation { .. }) {
            return Err(violation(
                "args",
                "a cursor continuation carries no query parameters",
            ));
        }

        Ok(Self {
            selector,
            fields,
            range,
            selection_view,
            page,
        })
    }

    fn parse_continuation(_req: &OperationRequest, args: &Value) -> Result<Self, ErrorEnvelope> {
        if args.get("fields").is_some()
            || args.get("range").is_some()
            || args.get("selection_view").is_some()
        {
            return Err(violation(
                "args",
                "a cursor continuation carries no query parameters",
            ));
        }
        let Some(page) = args.get("page") else {
            return Err(violation("args.selector", "missing required field"));
        };
        let page = Self::parse_page(page)?;
        if !matches!(page, PageRequest::Continuation { .. }) {
            return Err(violation(
                "args.selector",
                "first-page queries must name a selector",
            ));
        }
        Ok(Self {
            selector: ResourceSelector::Project, // replaced by the cursor record
            fields: FieldSelection(BTreeSet::from([FieldGroup::Metadata])),
            range: None,
            selection_view: None,
            page,
        })
    }

    fn parse_selector(value: &Value) -> Result<ResourceSelector, ErrorEnvelope> {
        let obj = value
            .as_object()
            .ok_or_else(|| violation("args.selector", "expected an object"))?;
        let present: Vec<&str> = ["resources", "kind", "all", "project"]
            .iter()
            .copied()
            .filter(|k| obj.get(*k).is_some_and(|v| !v.is_null()))
            .collect();
        if present.len() != 1 {
            return Err(violation(
                "args.selector",
                "exactly one of resources, kind, all, project is required",
            ));
        }
        match present[0] {
            "project" => match obj["project"].as_bool() {
                Some(true) => Ok(ResourceSelector::Project),
                _ => Err(violation("args.selector.project", "expected true")),
            },
            "all" => match obj["all"].as_bool() {
                Some(true) => Ok(ResourceSelector::All),
                _ => Err(violation("args.selector.all", "expected true")),
            },
            "kind" => {
                let name = obj["kind"]
                    .as_str()
                    .ok_or_else(|| violation("args.selector.kind", "expected a string"))?;
                let kind = ResourceKind::parse(name).map_err(ErrorEnvelope::from)?;
                if kind == ResourceKind::Project {
                    return Err(violation(
                        "args.selector.kind",
                        "use the project selector form for the project",
                    ));
                }
                Ok(ResourceSelector::Kind(kind))
            }
            "resources" => {
                let arr = obj["resources"]
                    .as_array()
                    .ok_or_else(|| violation("args.selector.resources", "expected an array"))?;
                if arr.is_empty() {
                    return Err(violation(
                        "args.selector.resources",
                        "resource list must not be empty",
                    ));
                }
                let mut seen = BTreeSet::new();
                let mut refs = Vec::with_capacity(arr.len());
                for (i, v) in arr.iter().enumerate() {
                    let s = v.as_str().ok_or_else(|| {
                        violation(
                            &format!("args.selector.resources[{i}]"),
                            "expected a string",
                        )
                    })?;
                    let r = ResourceRef::parse_tagged(s).map_err(ErrorEnvelope::from)?;
                    if seen.insert(r) {
                        refs.push(r);
                    }
                }
                Ok(ResourceSelector::Resources(refs))
            }
            _ => unreachable!(),
        }
    }

    fn parse_range(value: &Value) -> Result<TypedRange, ErrorEnvelope> {
        let obj = value
            .as_object()
            .ok_or_else(|| violation("args.range", "expected an object"))?;
        let domain = match obj
            .get("domain")
            .and_then(Value::as_str)
            .ok_or_else(|| violation("args.range.domain", "missing domain tag"))?
        {
            "project-musical" => TimeDomain::ProjectMusical,
            "media" => TimeDomain::Media,
            "source-native" => TimeDomain::SourceNative,
            other => {
                return Err(violation(
                    "args.range.domain",
                    format!("unknown time domain: {other}"),
                ));
            }
        };
        let start = obj
            .get("start")
            .and_then(Value::as_i64)
            .ok_or_else(|| violation("args.range.start", "expected an integer"))?;
        let len = obj
            .get("len")
            .and_then(Value::as_i64)
            .ok_or_else(|| violation("args.range.len", "expected an integer"))?;
        if len < 1 {
            return Err(violation("args.range.len", "expected a positive length"));
        }
        Ok(TypedRange { domain, start, len })
    }

    fn parse_page(value: &Value) -> Result<PageRequest, ErrorEnvelope> {
        let obj = value
            .as_object()
            .ok_or_else(|| violation("args.page", "expected an object"))?;
        let limit = obj.get("limit");
        let cursor = obj.get("cursor");
        match (limit, cursor) {
            (Some(_), Some(_)) => Err(violation(
                "args.page",
                "limit and cursor are exclusive — a continuation reuses the chain's page size",
            )),
            (Some(v), None) => {
                let n = v
                    .as_i64()
                    .ok_or_else(|| violation("args.page.limit", "expected an integer"))?;
                let limit = u32::try_from(n)
                    .ok()
                    .filter(|n| *n >= 1)
                    .ok_or_else(|| violation("args.page.limit", "expected a positive integer"))?;
                Ok(PageRequest::First { limit: Some(limit) })
            }
            (None, Some(v)) => Ok(PageRequest::Continuation {
                cursor: CursorRef::new(
                    v.as_str()
                        .ok_or_else(|| violation("args.page.cursor", "expected a string"))?,
                )
                .map_err(ErrorEnvelope::from)?,
            }),
            (None, None) => Ok(PageRequest::First { limit: None }),
        }
    }
}

/// How much of one item's requested data was servable (GTC-004 —
/// the projection reports `complete` / `unknown` / `unavailable`
/// honestly instead of silently omitting).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Completeness {
    /// Every requested group resolved.
    Complete,
    /// The resource could not be resolved in the pinned snapshot —
    /// for an explicit selector, "absent" and "never existed" share
    /// this verdict (existence-blind).
    Unknown,
    /// The resource resolved but at least one requested group could
    /// not be served (e.g. derived data that does not exist, or media
    /// bytes outside the projection's bounded-transfer contract).
    Unavailable,
}

/// One projected resource — identity plus the requested field groups
/// that resolved against the pinned snapshot.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectedItem {
    /// The projected resource's typed reference.
    pub resource: ResourceRef,
    /// How much of the requested data was servable.
    pub completeness: Completeness,
    /// Emitted field groups keyed by group name — only groups that
    /// resolved are present.
    pub fields: serde_json::Map<String, Value>,
    /// Requested groups that could not be served on this resource
    /// (serialized names).
    pub unavailable: Vec<String>,
}

/// The disclosed selection view (page-0 only).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SelectionProjection {
    /// The named view this projection answers for.
    pub view: SelectionViewRef,
    /// Scope-admitted selected resources — members outside the grant
    /// scope are dropped without existence detail.
    pub resources: Vec<ResourceRef>,
    /// Whether every selected resource was admitted and resolvable.
    pub completeness: Completeness,
}

/// Page metadata of one emission.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PageInfo {
    /// 0-based index of this page in its chain.
    pub index: u32,
    /// The page size applied.
    pub limit: u32,
    /// Continuation token for the next page — `null` when the chain
    /// is complete.
    pub next_cursor: Option<CursorRef>,
}

/// One page of a scoped context projection.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ContextProjection {
    /// The projected project.
    pub project: ProjectId,
    /// The pinned snapshot revision — every page of a chain reports
    /// the same revision; project changes never mix into a chain
    /// (DATA-03).
    pub revision: ProjectRevision,
    /// The pinned snapshot this page was read from — reusable as
    /// `Consistency::FixedSnapshot` for follow-up queries.
    pub snapshot: SnapshotRef,
    /// The data-access classes this emission carried — recorded for
    /// audit, not a grant.
    pub data_access: BTreeSet<DataAccessClass>,
    /// Worst-case completeness across the emitted items and selection.
    pub completeness: Completeness,
    /// This page's items.
    pub items: Vec<ProjectedItem>,
    /// The disclosed selection view — present on page 0 when requested.
    pub selection: Option<SelectionProjection>,
    /// Page metadata.
    pub page: PageInfo,
}

/// Bounded budgets for the context engine (GTC-004 / DATA-08).
///
/// Every dimension that a request can grow is capped: items per page,
/// pages per cursor chain, total items per chain, explicit selector
/// size, range window, and retained chains/snapshots.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ContextBudget {
    /// Default page size when the request names no `limit`.
    pub default_page_items: u32,
    /// Maximum `limit` a request may ask for.
    pub max_page_items: u32,
    /// Maximum pages one cursor chain may serve.
    pub max_pages: u32,
    /// Maximum admitted items across one chain.
    pub max_total_items: u32,
    /// Maximum explicit ids in one `resources` selector.
    pub max_selector_ids: u32,
    /// Maximum `range.len` in domain units (the time-window budget).
    pub max_range_len: i64,
    /// Cursor time-to-live in [`PolicyInstant`][crate::clock::PolicyInstant]
    /// units — after it the chain is dead (`CursorExpired`).
    pub cursor_ttl: u64,
    /// Maximum pinned snapshots retained.
    pub max_pinned_snapshots: usize,
    /// Maximum live cursor chains retained.
    pub max_cursors: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            default_page_items: 32,
            max_page_items: 256,
            max_pages: 64,
            max_total_items: 4096,
            max_selector_ids: 1024,
            max_range_len: i64::MAX / 2,
            cursor_ttl: 60_000,
            max_pinned_snapshots: 64,
            max_cursors: 256,
        }
    }
}

/// Workspace-selection facts supplied by the host seam.
///
/// `Unknown` and "not visible" are deliberately not distinguished —
/// the same verdict covers both (existence-blind), like
/// [`crate::grant::ResourceFacts`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionFacts {
    /// The view is known; these resources are currently selected.
    Selected(Vec<ResourceRef>),
    /// The view cannot be resolved (absent or invisible).
    Unknown,
    /// The view resolves but its state cannot be served.
    Unavailable,
}

/// Host seam supplying workspace-selection state (GTC-004).
///
/// The gateway owns no UI state — a named [`SelectionViewRef`] is
/// resolved by the host's view of the workspace, and only when the
/// query asks for it explicitly.
pub trait SelectionSource: Send + Sync {
    /// Resolves the named view's current selection for `project`.
    fn selection(&self, project: ProjectId, view: &SelectionViewRef) -> SelectionFacts;
}

/// A [`SelectionSource`] that reports every view unavailable — the
/// honest answer when the host exposes no selection state.
pub struct NoSelectionSource;

impl SelectionSource for NoSelectionSource {
    fn selection(&self, _project: ProjectId, _view: &SelectionViewRef) -> SelectionFacts {
        SelectionFacts::Unavailable
    }
}

/// The scope admission of a candidate set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScopeAdmission {
    /// Every candidate is admitted (manual authority — the trusted
    /// operator sees the whole projection).
    All,
    /// The scope-covered subset (agent grant scope).
    Subset(Vec<ResourceRef>),
}

/// The disclosure-authorization seam (GW-03's second boundary).
///
/// Implemented by [`crate::policy::GrantPolicy`]; kept as a trait so
/// the context engine composes whatever evaluator the host wires.
/// Both methods perform a *fresh* policy read on every call —
/// revocation or expiry between pages is observed at the page that
/// follows it, never masked by an earlier permit (DATA-04/09).
pub trait ContextAuthorizer: Send + Sync {
    /// Classifies `candidates` by grant scope under the request's
    /// authority — the filter that makes enumeration disclose only
    /// admitted members.
    ///
    /// `facts` is the pinned-snapshot fact source the engine resolved
    /// for this page — coverage is judged on the attributes the
    /// projection actually emits, never a newer live view (DATA-03).
    ///
    /// # Errors
    /// A deny reason when the request has no applicable authority at
    /// all (the engine surfaces it as a stage denial).
    fn admitted_scope(
        &self,
        req: &OperationRequest,
        descriptor: &crate::capability::CapabilityDescriptor,
        candidates: &[ResourceRef],
        facts: &dyn crate::grant::ResourceOracle,
    ) -> Result<ScopeAdmission, String>;

    /// Evaluates the disclosure boundary for `classes` on this
    /// request — a fresh policy-chain read plus the emitted classes
    /// against the grant's `data_access`. `external` adds
    /// [`DataAccessClass::ExternalDisclosure`] to the emitted set —
    /// data leaving toward an external caller requires it.
    ///
    /// Contract: the engine may call this while holding its state
    /// lock (cursor consume + authorize + rotate are one atomic
    /// transaction), so an implementation must never call back into
    /// the engine — doing so deadlocks.
    fn authorize_disclosure(
        &self,
        req: &OperationRequest,
        descriptor: &crate::capability::CapabilityDescriptor,
        classes: &BTreeSet<DataAccessClass>,
        external: bool,
    ) -> crate::authorization::AuthorizationDecision;
}
