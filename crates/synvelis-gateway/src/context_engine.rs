//! [`ContextProjectionEngine`] — the GW-03 context-query executor.
//!
//! The engine sits behind the `synvelis.context.query` capability as a
//! normal [`CapabilityOperation`]: [`Gateway::invoke`] runs the GW-02
//! preflight chain (`ReadContext` stage) first, then this executor runs
//! the GW-03 pipeline:
//!
//! ```text
//! parse ─▶ budget ─▶ disclosure boundary ─▶ pin snapshot ─▶ select ─▶
//! admit ─▶ page ─▶ mint cursor ─▶ emit ContextProjection
//! ```
//!
//! # Invariants
//!
//! - **Revision pinning (DATA-03).** Every page of a chain reads the
//!   same [`PinnedSnapshot`]. A first page pins per the request's
//!   consistency (`Latest` / `ReadAt` / `FixedSnapshot`); a continuation
//!   must name the chain's snapshot via `FixedSnapshot` — an evicted or
//!   unknown pin is `SnapshotExpired`, never a silent `Latest` re-base.
//! - **Expiring cursors.** A cursor records the caller, task/grant
//!   refs, emitted classes, admitted list, position, and an absolute
//!   expiry on the policy clock. Continuation re-validates all of it
//!   and *rotates* the token — a spent or replayed token is
//!   `CursorInvalid`, existence-blind (DATA-05).
//! - **Separate disclosure boundary.** Emitted classes are checked
//!   against the grant's `data_access` on every page
//!   ([`ContextAuthorizer::authorize_disclosure`]); an external caller
//!   additionally needs `ExternalDisclosure` — holding an id, a cursor,
//!   or an operation handle never discloses by itself (GT-004).
//! - **Bounded budgets.** Fetch amount, page count, chain length,
//!   selector size, and range window are all capped (`DATA-08`).
//! - **Internal reads are not disclosures.** [`Self::pin_for`] is the
//!   Synvelis-internal validation seam (GW-04 preparation): it resolves
//!   the consistency pin without evaluating the disclosure boundary.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use synvelis_project::{ProjectId, ProjectRevision};

use crate::authorization::{AuthorizationDecision, AuthorizationStage};
use crate::capability::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityVersion, DataAccessClass,
    DataAccessSet, EffectKind, EffectSet, ExecutionContract, OperationDomain, PermissionKind,
    PermissionSet, TargetContract,
};
use crate::clock::{PolicyClock, PolicyInstant};
use crate::context::{
    Completeness, ContextAuthorizer, ContextBudget, ContextProjection, ContextQuery, FieldGroup,
    PageInfo, PageRequest, ProjectedItem, ResourceSelector, ScopeAdmission, SelectionFacts,
    SelectionProjection, SelectionSource,
};
use crate::id::{AdapterId, CursorRef, GrantRef, SnapshotRef, TaskRef};
use crate::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use crate::policy::OUTSIDE_SCOPE;
use crate::registry::CapabilityOperation;
use crate::request::{
    Consistency, OperationRequest, OriginKind, Principal, PrincipalKind, ResourceRef, TimeDomain,
};
use crate::schema::{FieldSchema, SchemaRef, ValueSchema};
use crate::snapshot::{
    ENUMERATED_KINDS, PinnedSnapshot, ProjectOracle, SnapshotSource, enumerate_kind, item_window,
    midi_source_intersects, project_item,
};

/// The capability id this engine serves.
pub const CONTEXT_QUERY_ID: &str = "synvelis.context.query";
/// The query input schema name.
pub const CONTEXT_QUERY_INPUT: &str = "synvelis.context.query.input";
/// The projection output schema name.
pub const CONTEXT_QUERY_OUTPUT: &str = "synvelis.context.query.output";

/// One pinned snapshot inside the engine's bounded store.
struct PinnedEntry {
    project: ProjectId,
    revision: ProjectRevision,
    state: Arc<synvelis_project::Project>,
}

/// A live pagination chain — bound to its snapshot, caller, task/grant
/// refs, emitted field groups, and the admitted member list pinned at
/// page zero. `expires_at` is absolute (mint time + TTL): a chain dies
/// on schedule, not on activity.
struct CursorRecord {
    snapshot: SnapshotRef,
    project: ProjectId,
    principal: Principal,
    adapter: AdapterId,
    task: Option<TaskRef>,
    grant: Option<GrantRef>,
    admitted: Vec<ResourceRef>,
    fields: crate::context::FieldSelection,
    external: bool,
    range: Option<crate::request::TypedRange>,
    limit: u32,
    position: usize,
    pages_served: u32,
    expires_at: PolicyInstant,
}

/// The pin for `(project, revision)` when already retained —
/// caller holds the state lock.
fn existing_pin(
    state: &EngineState,
    project: ProjectId,
    revision: ProjectRevision,
) -> Option<PinnedSnapshot> {
    let rref = state.pin_index.get(&(project, revision))?;
    let entry = state
        .pinned
        .get(rref.as_str())
        .expect("pin index disagrees with store");
    Some(PinnedSnapshot {
        reference: rref.clone(),
        project,
        revision,
        state: entry.state.clone(),
    })
}

#[derive(Default)]
struct EngineState {
    pinned: BTreeMap<String, PinnedEntry>,
    pin_index: BTreeMap<(ProjectId, ProjectRevision), SnapshotRef>,
    pin_order: VecDeque<String>,
    cursors: BTreeMap<String, CursorRecord>,
    cursor_order: VecDeque<String>,
    next_ref: u128,
}

/// The GW-03 context-projection executor.
///
/// Construct it as `Arc`, then [`Self::install`] to register the
/// capability + schemas + handler on a [`crate::Gateway`].
pub struct ContextProjectionEngine {
    source: Arc<dyn SnapshotSource>,
    selection: Arc<dyn SelectionSource>,
    authorizer: Arc<dyn ContextAuthorizer>,
    clock: Arc<dyn PolicyClock>,
    budget: ContextBudget,
    descriptor: CapabilityDescriptor,
    state: Mutex<EngineState>,
}

#[allow(clippy::result_large_err)] // typed envelopes are the contract
impl ContextProjectionEngine {
    /// The capability descriptor the engine installs and serves.
    ///
    /// `data_access` is the emission *floor* — [`MetadataRead`], the
    /// class every response carries. Field-group classes the query
    /// requests are authorized per request at the disclosure boundary;
    /// declaring them here would force every grant to cover the whole
    /// menu before a metadata-only caller could query at all (DATA-01).
    ///
    /// # Panics
    /// Panics if the static capability id were malformed — it is a
    /// tested constant.
    #[must_use]
    pub fn descriptor() -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: CapabilityId::new(CONTEXT_QUERY_ID).expect("capability id is valid"),
            version: CapabilityVersion::new(1, 0),
            domain: OperationDomain::Query,
            input_schema: SchemaRef::new(CONTEXT_QUERY_INPUT, 1, 0),
            output_schema: SchemaRef::new(CONTEXT_QUERY_OUTPUT, 1, 0),
            target: TargetContract::None,
            effect_bound: EffectSet::of([EffectKind::Read]),
            required_permissions: PermissionSet::of([
                PermissionKind::ReadContext,
                PermissionKind::DiscloseContext,
            ]),
            data_access: DataAccessSet::of([DataAccessClass::MetadataRead]),
            execution: ExecutionContract::Sync,
            availability: Availability::Available,
        }
    }

    /// The engine over a snapshot source, selection source, disclosure
    /// authorizer, clock, and budget.
    ///
    /// `max_pinned_snapshots` and `max_cursors` clamp to a minimum of
    /// one — a zero bound cannot serve its own purpose (a pin must be
    /// retained to answer the query that minted it).
    #[must_use]
    pub fn new(
        source: Arc<dyn SnapshotSource>,
        selection: Arc<dyn SelectionSource>,
        authorizer: Arc<dyn ContextAuthorizer>,
        clock: Arc<dyn PolicyClock>,
        mut budget: ContextBudget,
    ) -> Self {
        budget.max_pinned_snapshots = budget.max_pinned_snapshots.max(1);
        budget.max_cursors = budget.max_cursors.max(1);
        Self {
            source,
            selection,
            authorizer,
            clock,
            budget,
            descriptor: Self::descriptor(),
            state: Mutex::new(EngineState::default()),
        }
    }

    /// Registers the input/output schemas and the
    /// `synvelis.context.query` capability with `self` as its executor.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — schema or capability registration conflict.
    pub fn install(self: &Arc<Self>, gateway: &mut crate::Gateway) -> Result<(), ErrorEnvelope> {
        gateway
            .schemas_mut()
            .register(
                CONTEXT_QUERY_INPUT,
                crate::schema::SchemaVersion::new(1, 0),
                Self::input_schema(),
            )
            .map_err(ErrorEnvelope::from)?;
        gateway
            .schemas_mut()
            .register(
                CONTEXT_QUERY_OUTPUT,
                crate::schema::SchemaVersion::new(1, 0),
                ValueSchema::Object {
                    fields: BTreeMap::new(),
                    deny_unknown: false,
                },
            )
            .map_err(ErrorEnvelope::from)?;
        gateway.register_capability(Self::descriptor(), Some(self.clone()))
    }

    /// The internal-read seam — resolves the request's consistency to a
    /// pinned snapshot without emitting anything.
    ///
    /// This is the GW-04 preparation boundary: a `PrepareProposal`
    /// operation references the pinned revision and snapshot reference
    /// the query reported, and the projection proves they agree. No
    /// disclosure evaluation happens here — the caller is
    /// Synvelis-internal validation code, and an internal reference is
    /// never an external read (GTC-004).
    ///
    /// # Errors
    /// [`GatewayError::SnapshotExpired`] — the revision or reference
    /// cannot be pinned (evicted, unknown, or foreign project).
    pub fn pin_for(&self, req: &OperationRequest) -> Result<PinnedSnapshot, ErrorEnvelope> {
        match req.consistency() {
            Consistency::Latest => {
                let revision = self
                    .source
                    .latest_revision(req.project())
                    .ok_or(GatewayError::SnapshotExpired)?;
                self.pin_state(req.project(), revision)
            }
            Consistency::ReadAt { revision } => self.pin_state(req.project(), *revision),
            Consistency::FixedSnapshot { snapshot } => self.resolve_pin(req.project(), snapshot),
            Consistency::ExpectedRevision { .. } | Consistency::Unconditional => {
                Err(GatewayError::Internal {
                    reason: "context query bound a non-query consistency".to_owned(),
                }
                .into())
            }
        }
    }

    /// Evicts a pinned snapshot — retention control for the host and
    /// the fixture (an evicted pin never silently re-bases to Latest).
    ///
    /// # Panics
    /// Panics if the engine's lock is poisoned.
    pub fn evict_snapshot(&self, snapshot: &SnapshotRef) -> bool {
        let mut state = self.state.lock().expect("context engine poisoned");
        if let Some(entry) = state.pinned.remove(snapshot.as_str()) {
            state.pin_index.remove(&(entry.project, entry.revision));
            state.pin_order.retain(|r| r != snapshot.as_str());
            true
        } else {
            false
        }
    }

    /// Number of pinned snapshots currently retained (test/introspection).
    ///
    /// # Panics
    /// Panics if the engine's lock is poisoned.
    #[must_use]
    pub fn pinned_count(&self) -> usize {
        self.state
            .lock()
            .expect("context engine poisoned")
            .pinned
            .len()
    }

    /// Number of live cursor chains (test/introspection).
    ///
    /// # Panics
    /// Panics if the engine's lock is poisoned.
    #[must_use]
    pub fn cursor_count(&self) -> usize {
        self.state
            .lock()
            .expect("context engine poisoned")
            .cursors
            .len()
    }

    fn input_schema() -> ValueSchema {
        let mut selector_fields = BTreeMap::new();
        selector_fields.insert(
            "resources".to_owned(),
            FieldSchema::optional(ValueSchema::Array {
                item: Box::new(ValueSchema::String { max_len: Some(64) }),
                max_len: Some(1024),
            }),
        );
        selector_fields.insert(
            "kind".to_owned(),
            FieldSchema::optional(ValueSchema::String { max_len: Some(32) }),
        );
        selector_fields.insert("all".to_owned(), FieldSchema::optional(ValueSchema::Bool));
        selector_fields.insert(
            "project".to_owned(),
            FieldSchema::optional(ValueSchema::Bool),
        );

        let mut range_fields = BTreeMap::new();
        range_fields.insert(
            "domain".to_owned(),
            FieldSchema::required(ValueSchema::Enum {
                values: ["project-musical", "media", "source-native"]
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect(),
            }),
        );
        range_fields.insert(
            "start".to_owned(),
            FieldSchema::required(ValueSchema::Integer {
                min: None,
                max: None,
            }),
        );
        range_fields.insert(
            "len".to_owned(),
            FieldSchema::required(ValueSchema::Integer {
                min: Some(1),
                max: None,
            }),
        );

        let mut page_fields = BTreeMap::new();
        page_fields.insert(
            "limit".to_owned(),
            FieldSchema::optional(ValueSchema::Integer {
                min: Some(1),
                max: None,
            }),
        );
        page_fields.insert(
            "cursor".to_owned(),
            FieldSchema::optional(ValueSchema::String { max_len: Some(256) }),
        );

        let mut fields = BTreeMap::new();
        fields.insert(
            "selector".to_owned(),
            FieldSchema::optional(ValueSchema::Object {
                fields: selector_fields,
                deny_unknown: true,
            }),
        );
        fields.insert(
            "fields".to_owned(),
            FieldSchema::optional(ValueSchema::Array {
                item: Box::new(ValueSchema::Enum {
                    values: FieldGroup::all_names(),
                }),
                max_len: Some(8),
            }),
        );
        fields.insert(
            "selection_view".to_owned(),
            FieldSchema::optional(ValueSchema::String { max_len: Some(256) }),
        );
        fields.insert(
            "range".to_owned(),
            FieldSchema::optional(ValueSchema::Object {
                fields: range_fields,
                deny_unknown: true,
            }),
        );
        fields.insert(
            "page".to_owned(),
            FieldSchema::optional(ValueSchema::Object {
                fields: page_fields,
                deny_unknown: true,
            }),
        );
        ValueSchema::Object {
            fields,
            deny_unknown: true,
        }
    }

    /// Whether the caller receives data outside the trusted manual
    /// session — agent principals and anything attested through an
    /// external adapter cross the disclosure boundary.
    fn is_external(req: &OperationRequest) -> bool {
        matches!(req.caller().principal().kind(), PrincipalKind::Agent)
            || matches!(req.caller().origin(), OriginKind::ExternalAdapter)
    }

    /// The classes an emission carries — the field classes plus
    /// [`DataAccessClass::ExternalDisclosure`] for external callers.
    fn emitted_classes(
        query_or_record_classes: &BTreeSet<DataAccessClass>,
        external: bool,
    ) -> BTreeSet<DataAccessClass> {
        let mut classes = query_or_record_classes.clone();
        if external {
            classes.insert(DataAccessClass::ExternalDisclosure);
        }
        classes
    }

    fn denied(stage: AuthorizationStage, reason: String) -> ErrorEnvelope {
        GatewayError::Denied {
            stage,
            reason: Some(reason),
        }
        .into()
    }

    /// Applies the disclosure boundary — translates the authorizer's
    /// verdict into the engine's error channel.
    fn check_disclosure(
        &self,
        req: &OperationRequest,
        classes: &BTreeSet<DataAccessClass>,
        external: bool,
    ) -> Result<(), ErrorEnvelope> {
        match self
            .authorizer
            .authorize_disclosure(req, &self.descriptor, classes, external)
        {
            AuthorizationDecision::PermitForStage => Ok(()),
            AuthorizationDecision::Deny { reason } => {
                Err(Self::denied(AuthorizationStage::ReadContext, reason))
            }
            // The stage gate never requests approval for a read stage.
            AuthorizationDecision::ApprovalRequired => Err(GatewayError::Internal {
                reason: "approval decision at a read stage".to_owned(),
            }
            .into()),
        }
    }

    /// Pins `state` at `(project, revision)` — deduplicated per pair so
    /// two queries at one revision share one [`SnapshotRef`].
    ///
    /// The host [`SnapshotSource`] call runs outside the engine lock —
    /// only the lookup and the bounded insert are critical sections —
    /// so an implementation's I/O never stalls cursor traffic.
    fn pin_state(
        &self,
        project: ProjectId,
        revision: ProjectRevision,
    ) -> Result<PinnedSnapshot, ErrorEnvelope> {
        {
            let state = self.state.lock().expect("context engine poisoned");
            if let Some(pinned) = existing_pin(&state, project, revision) {
                return Ok(pinned);
            }
        }
        let Some(project_state) = self.source.pin(project, revision) else {
            return Err(GatewayError::SnapshotExpired.into());
        };
        let mut state = self.state.lock().expect("context engine poisoned");
        // A concurrent pin of the same pair won the race — share it.
        if let Some(pinned) = existing_pin(&state, project, revision) {
            return Ok(pinned);
        }
        let reference = SnapshotRef::new(format!("snapshot-{:032x}", state.next_ref))
            .map_err(ErrorEnvelope::from)?;
        state.next_ref = state.next_ref.checked_add(1).expect("ref space exhausted");
        while state.pinned.len() >= self.budget.max_pinned_snapshots
            && let Some(oldest) = state.pin_order.pop_front()
        {
            if let Some(entry) = state.pinned.remove(&oldest) {
                state.pin_index.remove(&(entry.project, entry.revision));
            }
        }
        state.pinned.insert(
            reference.as_str().to_owned(),
            PinnedEntry {
                project,
                revision,
                state: project_state.clone(),
            },
        );
        state
            .pin_index
            .insert((project, revision), reference.clone());
        state.pin_order.push_back(reference.as_str().to_owned());
        Ok(PinnedSnapshot {
            reference,
            project,
            revision,
            state: project_state,
        })
    }

    /// Resolves an existing pin by reference — a `FixedSnapshot`
    /// consistency only ever reads what was pinned, never substitutes.
    fn resolve_pin(
        &self,
        project: ProjectId,
        snapshot: &SnapshotRef,
    ) -> Result<PinnedSnapshot, ErrorEnvelope> {
        let state = self.state.lock().expect("context engine poisoned");
        let Some(entry) = state.pinned.get(snapshot.as_str()) else {
            return Err(GatewayError::SnapshotExpired.into());
        };
        if entry.project != project {
            // A reference naming a different project is not disclosed —
            // foreign and expired share one verdict.
            return Err(GatewayError::SnapshotExpired.into());
        }
        Ok(PinnedSnapshot {
            reference: snapshot.clone(),
            project,
            revision: entry.revision,
            state: entry.state.clone(),
        })
    }

    /// Candidates named by the selector — unfiltered. The range
    /// window applies *after* scope admission (see `range_filter`):
    /// pre-filtering would let the verdict shape disclose whether an
    /// out-of-scope resource resolved and where it sat (existence
    /// probing, DATA-05).
    fn candidates(query: &ContextQuery, state: &synvelis_project::Project) -> Vec<ResourceRef> {
        match &query.selector {
            ResourceSelector::Project => vec![ResourceRef::Project(state.project_id)],
            ResourceSelector::Resources(refs) => refs.clone(),
            ResourceSelector::Kind(kind) => enumerate_kind(state, *kind),
            ResourceSelector::All => ENUMERATED_KINDS
                .iter()
                .flat_map(|kind| enumerate_kind(state, *kind))
                .collect(),
        }
    }

    /// Drops members whose domain window does not intersect `range` —
    /// applied to the *admitted* set, never to candidates.
    fn range_filter(
        state: &synvelis_project::Project,
        admitted: &mut Vec<ResourceRef>,
        range: &crate::request::TypedRange,
    ) {
        admitted.retain(|r| match r {
            // A MIDI source's own window is its note payload — a
            // source-native range intersects notes, not the source.
            ResourceRef::Source(id) => match (state.midi_sources.get(id), range.domain) {
                (Some(source), TimeDomain::SourceNative) => {
                    midi_source_intersects(source, range.start, range.len)
                }
                _ => false,
            },
            _ => item_window(state, r).is_some_and(|window| {
                window.domain == range.domain
                    && window.start.checked_add(window.len).is_some_and(|w_end| {
                        range
                            .start
                            .checked_add(range.len)
                            .is_some_and(|r_end| window.start < r_end && w_end > range.start)
                    })
            }),
        });
    }

    /// The request-shape budgets — checked before any project state is
    /// touched (DATA-08). Returns the effective page size.
    fn check_request_budgets(
        &self,
        query: &ContextQuery,
        limit: Option<u32>,
    ) -> Result<u32, ErrorEnvelope> {
        let limit = limit.unwrap_or(self.budget.default_page_items);
        if limit > self.budget.max_page_items {
            return Err(GatewayError::LimitExceeded {
                limit: "page items".to_owned(),
                max: u64::from(self.budget.max_page_items),
            }
            .into());
        }
        if let ResourceSelector::Resources(refs) = &query.selector
            && refs.len() > self.budget.max_selector_ids as usize
        {
            return Err(GatewayError::LimitExceeded {
                limit: "selector ids".to_owned(),
                max: u64::from(self.budget.max_selector_ids),
            }
            .into());
        }
        if let Some(range) = query.range
            && range.len > self.budget.max_range_len
        {
            return Err(GatewayError::LimitExceeded {
                limit: "range window".to_owned(),
                // A negative configured bound wraps — clamp the report.
                max: u64::try_from(self.budget.max_range_len).unwrap_or(0),
            }
            .into());
        }
        Ok(limit)
    }

    /// Page zero of a new chain.
    fn first_page(
        &self,
        req: &OperationRequest,
        query: &ContextQuery,
        limit: Option<u32>,
    ) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let limit = self.check_request_budgets(query, limit)?;
        let external = Self::is_external(req);
        let classes = query.fields.classes();
        self.check_disclosure(req, &classes, external)?;

        let pinned = self.pin_for(req)?;
        // Scope facts come from the same pinned revision the
        // projection reads — never a newer live view (DATA-03).
        let pin_oracle = ProjectOracle::new(pinned.state.clone());

        let candidates = Self::candidates(query, &pinned.state);
        let mut admitted = match self
            .authorizer
            .admitted_scope(req, &self.descriptor, &candidates, &pin_oracle)
            .map_err(|reason| Self::denied(AuthorizationStage::ReadContext, reason))?
        {
            ScopeAdmission::All => candidates,
            ScopeAdmission::Subset(subset) => {
                // Explicit-id selectors are a verdict: an unlisted or
                // unresolvable member denies like an absent one.
                if matches!(query.selector, ResourceSelector::Resources(_))
                    && subset.len() != candidates.len()
                {
                    return Err(Self::denied(
                        AuthorizationStage::ReadContext,
                        OUTSIDE_SCOPE.to_owned(),
                    ));
                }
                subset
            }
        };
        // The range window filters the admitted set only — applied to
        // candidates it would turn the verdict above into an
        // existence/placement probe on out-of-scope resources.
        if let Some(range) = &query.range {
            Self::range_filter(&pinned.state, &mut admitted, range);
        }
        if admitted.len() > self.budget.max_total_items as usize {
            return Err(GatewayError::LimitExceeded {
                limit: "total items".to_owned(),
                max: u64::from(self.budget.max_total_items),
            }
            .into());
        }

        let selection = self.project_selection(req, query, &pinned.state, &pin_oracle)?;

        let take = admitted.len().min(limit as usize);
        let items: Vec<ProjectedItem> = admitted[..take]
            .iter()
            .map(|r| project_item(&pinned.state, r, &query.fields, query.range))
            .collect();

        let has_more = take < admitted.len();
        let next_cursor = if has_more {
            let record = CursorRecord {
                snapshot: pinned.reference.clone(),
                project: req.project(),
                principal: req.caller().principal().clone(),
                adapter: req.caller().adapter(),
                task: req.task().cloned(),
                grant: req.grant().cloned(),
                admitted,
                fields: query.fields.clone(),
                external,
                range: query.range,
                limit,
                position: take,
                pages_served: 1,
                expires_at: PolicyInstant(
                    self.clock.now().0.saturating_add(self.budget.cursor_ttl),
                ),
            };
            let mut state = self.state.lock().expect("context engine poisoned");
            Some(Self::insert_cursor_locked(
                &mut state,
                record,
                self.budget.max_cursors,
            ))
        } else {
            None
        };

        Ok(GatewayOutcome::Completed {
            result: Self::projection_value(
                pinned,
                Self::emitted_classes(&classes, external),
                selection,
                items,
                PageInfo {
                    index: 0,
                    limit,
                    next_cursor,
                },
            ),
        })
    }

    /// Emits the selection view, when the query asked for it.
    ///
    /// Members are resolved and admitted against the *same* pin the
    /// page was projected from — never a second lookup, so a `latest`
    /// commit mid-query cannot mix a newer revision into this
    /// emission. A member that does not resolve in the pin (or fails
    /// scope) is dropped without existence detail, but the drop is
    /// reported — `unavailable`, never a false `complete`.
    fn project_selection(
        &self,
        req: &OperationRequest,
        query: &ContextQuery,
        pinned: &synvelis_project::Project,
        pin_oracle: &dyn crate::grant::ResourceOracle,
    ) -> Result<Option<SelectionProjection>, ErrorEnvelope> {
        if !query.fields.wants(FieldGroup::Selection) {
            return Ok(None);
        }
        let Some(view) = query.selection_view.clone() else {
            return Err(GatewayError::Internal {
                reason: "selection field without a named view".to_owned(),
            }
            .into());
        };
        let facts = self.selection.selection(req.project(), &view);
        let (resources, completeness) = match facts {
            SelectionFacts::Selected(members) => {
                let selected = members.len();
                let members: Vec<ResourceRef> = members
                    .into_iter()
                    .filter(|r| crate::snapshot::attributes(pinned, r).is_some())
                    .collect();
                let resolved = members.len() == selected;
                match self
                    .authorizer
                    .admitted_scope(req, &self.descriptor, &members, pin_oracle)
                    .map_err(|reason| Self::denied(AuthorizationStage::ReadContext, reason))?
                {
                    ScopeAdmission::All => {
                        let complete = if resolved {
                            Completeness::Complete
                        } else {
                            Completeness::Unavailable
                        };
                        (members, complete)
                    }
                    ScopeAdmission::Subset(subset) => {
                        let complete = if resolved && subset.len() == members.len() {
                            Completeness::Complete
                        } else {
                            Completeness::Unavailable
                        };
                        (subset, complete)
                    }
                }
            }
            // An unresolvable view reports `unknown`; a resolvable
            // view that cannot be served reports `unavailable`.
            SelectionFacts::Unknown => (Vec::new(), Completeness::Unknown),
            SelectionFacts::Unavailable => (Vec::new(), Completeness::Unavailable),
        };
        Ok(Some(SelectionProjection {
            view,
            resources,
            completeness,
        }))
    }

    /// Continuation of an existing chain — the cursor re-binds the
    /// caller, task/grant, snapshot, and emitted classes, and rotates.
    /// The whole sequence runs under one lock so consume, authorize,
    /// and rotate are one transaction.
    fn continue_page(
        &self,
        req: &OperationRequest,
        cursor: &CursorRef,
    ) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        // The continuation must name the chain's own snapshot — a
        // `latest` request would silently re-base (DATA-03).
        let Consistency::FixedSnapshot { snapshot } = req.consistency() else {
            return Err(GatewayError::CursorInvalid {
                reason: "continuation requires fixed-snapshot consistency".to_owned(),
            }
            .into());
        };
        let now = self.clock.now();
        let mut state = self.state.lock().expect("context engine poisoned");
        let Some(record) = state.cursors.get(cursor.as_str()) else {
            return Err(GatewayError::CursorInvalid {
                reason: "cursor does not identify a live pagination chain".to_owned(),
            }
            .into());
        };
        if record.project != req.project()
            || record.principal != *req.caller().principal()
            || record.adapter != req.caller().adapter()
            || record.task != req.task().cloned()
            || record.grant != req.grant().cloned()
            || record.snapshot != *snapshot
        {
            // Unknown, foreign, and mismatched cursors share one
            // verdict — the answer never confirms whether the token
            // exists under another identity (DATA-05).
            return Err(GatewayError::CursorInvalid {
                reason: "cursor does not identify a live pagination chain".to_owned(),
            }
            .into());
        }
        if now > record.expires_at {
            Self::drop_cursor_locked(&mut state, cursor.as_str());
            return Err(GatewayError::CursorExpired.into());
        }
        let pinned = state
            .pinned
            .get(record.snapshot.as_str())
            .map(|e| (e.revision, e.state.clone()));
        let Some((revision, pinned_state)) = pinned else {
            Self::drop_cursor_locked(&mut state, cursor.as_str());
            return Err(GatewayError::SnapshotExpired.into());
        };
        if record.pages_served >= self.budget.max_pages {
            return Err(GatewayError::LimitExceeded {
                limit: "pages".to_owned(),
                max: u64::from(self.budget.max_pages),
            }
            .into());
        }
        let classes = record.fields.classes();
        let external = record.external;

        // Fresh policy read — a revoke or expiry since the previous
        // page is observed now, never masked by an earlier permit
        // (DATA-04/09). The authorizer never re-enters the engine, so
        // holding the lock across this call is safe.
        self.check_disclosure(req, &classes, external)?;

        // Validation passed — consume the old token and rotate.
        let mut record = state
            .cursors
            .remove(cursor.as_str())
            .expect("cursor checked above");
        state.cursor_order.retain(|c| c != cursor.as_str());
        let take = (record.admitted.len() - record.position).min(record.limit as usize);
        let page_refs: Vec<ResourceRef> =
            record.admitted[record.position..record.position + take].to_vec();
        let fields = record.fields.clone();
        let range = record.range;
        let index = record.pages_served;
        let limit = record.limit;
        record.position += take;
        record.pages_served += 1;

        let has_more = record.position < record.admitted.len();
        let next_cursor = has_more
            .then(|| Self::insert_cursor_locked(&mut state, record, self.budget.max_cursors));
        drop(state);

        // Projection is a pure read over the pinned Arc — outside the
        // lock, so payload construction never stalls other chains.
        let items: Vec<ProjectedItem> = page_refs
            .iter()
            .map(|r| project_item(&pinned_state, r, &fields, range))
            .collect();

        let pinned = PinnedSnapshot {
            reference: snapshot.clone(),
            project: req.project(),
            revision,
            state: pinned_state,
        };
        Ok(GatewayOutcome::Completed {
            result: Self::projection_value(
                pinned,
                Self::emitted_classes(&classes, external),
                None,
                items,
                PageInfo {
                    index,
                    limit,
                    next_cursor,
                },
            ),
        })
    }

    /// Inserts a record under a freshly minted token — caller holds
    /// the state lock. Retention is bounded: the oldest chain is
    /// evicted first.
    fn insert_cursor_locked(
        state: &mut EngineState,
        record: CursorRecord,
        max_cursors: usize,
    ) -> CursorRef {
        while state.cursors.len() >= max_cursors
            && let Some(oldest) = state.cursor_order.pop_front()
        {
            state.cursors.remove(&oldest);
        }
        let cursor = CursorRef::new(format!("cursor-{:032x}", state.next_ref))
            .expect("minted cursor is valid");
        state.next_ref = state.next_ref.checked_add(1).expect("ref space exhausted");
        state.cursors.insert(cursor.as_str().to_owned(), record);
        state.cursor_order.push_back(cursor.as_str().to_owned());
        cursor
    }

    fn drop_cursor_locked(state: &mut EngineState, id: &str) {
        state.cursors.remove(id);
        state.cursor_order.retain(|c| c != id);
    }

    /// Serializes one page of the projection.
    fn projection_value(
        pinned: PinnedSnapshot,
        data_access: BTreeSet<DataAccessClass>,
        selection: Option<SelectionProjection>,
        items: Vec<ProjectedItem>,
        page: PageInfo,
    ) -> Value {
        let completeness = items
            .iter()
            .map(|i| i.completeness)
            .chain(selection.iter().map(|s| s.completeness))
            .max()
            .unwrap_or(Completeness::Complete);
        serde_json::to_value(ContextProjection {
            project: pinned.project,
            revision: pinned.revision,
            snapshot: pinned.reference,
            data_access,
            completeness,
            items,
            selection,
            page,
        })
        .expect("projection serializes")
    }
}

impl CapabilityOperation for ContextProjectionEngine {
    fn execute(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let query = ContextQuery::parse(req)?;
        match &query.page {
            PageRequest::First { limit } => self.first_page(req, &query, *limit),
            PageRequest::Continuation { cursor } => self.continue_page(req, cursor),
        }
    }
}
