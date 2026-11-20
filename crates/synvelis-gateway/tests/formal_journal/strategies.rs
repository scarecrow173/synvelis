//! Shared proptest strategies for the `formal_journal` suite.
//!
//! Extracted from the slice-1 target root (Issue #601): the `arb_*`
//! generators and the `encode_stream` helper are shared between the
//! journal codec properties (root), the `WireRequest::parse` suite
//! (`wire`), and the `GrantPolicy` suite (`policy`). Behavior is
//! unchanged — visibility only widened to `pub(crate)`.

use std::collections::BTreeSet;

use proptest::prelude::*;
use synvelis_gateway::clock::PolicyInstant;
use synvelis_gateway::commit::{
    CommitRecord, JournalIdem, JournalPayload, OutcomeRecord, PolicyEvent, SaveRecord,
};
use synvelis_gateway::grant::{
    ApprovalPolicy, CreateWithin, DynamicScope, Grant, GrantBinding, GrantLimits, MoveArgs,
    OwnerConstraint, PlacementConstraint, PolicyEpoch, ResourceScope, Validity,
};
use synvelis_gateway::id::{
    AdapterId, ApprovalRef, GrantRef, IdempotencyKey, OperationId, ProactivePermitRef, ProposalRef,
    TaskRef,
};
use synvelis_gateway::journal::encode_frame;
use synvelis_gateway::request::{
    Principal, PrincipalKind, ResourceKind, ResourceRef, TimeDomain, TypedRange,
};
use synvelis_gateway::store::{Activation, AuthorizedTask, ProactivePermit};
use synvelis_gateway::{
    CapabilityId, CapabilityRef, CapabilityVersion, DataAccessClass, DataAccessSet, PermissionKind,
    PermissionSet,
};
use synvelis_project::{
    AssetId, AutomationLaneId, BranchId, ClipId, CompId, LaneId, NoteId, ParameterTargetId,
    ProcessorId, ProjectId, ProjectRevision, SendId, SourceId, StripId, TakeGroupId, TrackId,
    VariantSetId,
};

// ---- raw frame strategies ----

/// Arbitrary journal payload bodies (JSON bytes on the wire, but the
/// codec is byte-agnostic). Bounded to keep the always-on lane fast;
/// the `MAX_PAYLOAD` cap path gets its own dedicated cases.
pub(crate) fn arb_payload() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..=384)
}

/// A stream of encoded frames with contiguous seq from 0, plus the
/// byte offsets where each record starts.
pub(crate) fn encode_stream(payloads: &[Vec<u8>]) -> (Vec<u8>, Vec<usize>) {
    let mut stream = Vec::new();
    let mut starts = Vec::new();
    for (i, p) in payloads.iter().enumerate() {
        starts.push(stream.len());
        let seq = u64::try_from(i).unwrap_or(u64::MAX);
        stream.extend_from_slice(&encode_frame(seq, p).expect("in-cap payload must encode"));
    }
    (stream, starts)
}

// ---- JournalPayload strategies ----

/// Non-empty bounded id text — always valid for `Principal` /
/// `define_string_ref!` / `CapabilityId` constructors.
pub(crate) fn arb_ref_text() -> impl Strategy<Value = String> {
    "[a-z0-9._-]{1,32}"
}

/// Arbitrary short free text (reason/head/condition/fingerprint/…).
pub(crate) fn arb_text() -> impl Strategy<Value = String> {
    ".{0,64}"
}

pub(crate) fn arb_principal() -> impl Strategy<Value = Principal> {
    (
        prop_oneof![Just(PrincipalKind::Human), Just(PrincipalKind::Agent)],
        arb_ref_text(),
    )
        .prop_map(|(kind, id)| {
            Principal::new(kind, id).expect("generated principal id is always valid")
        })
}

pub(crate) fn arb_project_id() -> impl Strategy<Value = ProjectId> {
    any::<u128>().prop_map(ProjectId::new)
}

pub(crate) fn arb_task_ref() -> impl Strategy<Value = TaskRef> {
    arb_ref_text().prop_map(|s| TaskRef::new(s).expect("generated ref is always valid"))
}

pub(crate) fn arb_grant_ref() -> impl Strategy<Value = GrantRef> {
    arb_ref_text().prop_map(|s| GrantRef::new(s).expect("generated ref is always valid"))
}

pub(crate) fn arb_proposal_ref() -> impl Strategy<Value = ProposalRef> {
    arb_ref_text().prop_map(|s| ProposalRef::new(s).expect("generated ref is always valid"))
}

pub(crate) fn arb_approval_ref() -> impl Strategy<Value = ApprovalRef> {
    arb_ref_text().prop_map(|s| ApprovalRef::new(s).expect("generated ref is always valid"))
}

pub(crate) fn arb_permit_ref() -> impl Strategy<Value = ProactivePermitRef> {
    arb_ref_text().prop_map(|s| ProactivePermitRef::new(s).expect("generated ref is always valid"))
}

pub(crate) fn arb_idem() -> impl Strategy<Value = JournalIdem> {
    (arb_ref_text(), arb_text()).prop_map(|(key, fingerprint)| JournalIdem {
        key: IdempotencyKey::new(key).expect("generated key is always valid"),
        fingerprint,
    })
}

pub(crate) fn arb_resource_ref() -> impl Strategy<Value = ResourceRef> {
    (0u8..16, any::<u128>()).prop_map(|(kind, raw)| match kind {
        0 => ResourceRef::Project(ProjectId::new(raw)),
        1 => ResourceRef::Track(TrackId::new(raw)),
        2 => ResourceRef::Lane(LaneId::new(raw)),
        3 => ResourceRef::Clip(ClipId::new(raw)),
        4 => ResourceRef::Source(SourceId::new(raw)),
        5 => ResourceRef::Asset(AssetId::new(raw)),
        6 => ResourceRef::Note(NoteId::new(raw)),
        7 => ResourceRef::TakeGroup(TakeGroupId::new(raw)),
        8 => ResourceRef::Comp(CompId::new(raw)),
        9 => ResourceRef::ParameterTarget(ParameterTargetId::new(raw)),
        10 => ResourceRef::AutomationLane(AutomationLaneId::new(raw)),
        11 => ResourceRef::Strip(StripId::new(raw)),
        12 => ResourceRef::Send(SendId::new(raw)),
        13 => ResourceRef::Processor(ProcessorId::new(raw)),
        14 => ResourceRef::VariantSet(VariantSetId::new(raw)),
        _ => ResourceRef::Branch(BranchId::new(raw)),
    })
}

pub(crate) fn arb_resource_kind() -> impl Strategy<Value = ResourceKind> {
    (0u8..16).prop_map(|kind| match kind {
        0 => ResourceKind::Project,
        1 => ResourceKind::Track,
        2 => ResourceKind::Lane,
        3 => ResourceKind::Clip,
        4 => ResourceKind::Source,
        5 => ResourceKind::Asset,
        6 => ResourceKind::Note,
        7 => ResourceKind::TakeGroup,
        8 => ResourceKind::Comp,
        9 => ResourceKind::ParameterTarget,
        10 => ResourceKind::AutomationLane,
        11 => ResourceKind::Strip,
        12 => ResourceKind::Send,
        13 => ResourceKind::Processor,
        14 => ResourceKind::VariantSet,
        _ => ResourceKind::Branch,
    })
}

pub(crate) fn arb_typed_range() -> impl Strategy<Value = TypedRange> {
    (
        prop_oneof![
            Just(TimeDomain::ProjectMusical),
            Just(TimeDomain::Media),
            Just(TimeDomain::SourceNative),
        ],
        any::<i64>(),
        any::<i64>(),
    )
        .prop_map(|(domain, start, len)| TypedRange { domain, start, len })
}

pub(crate) fn arb_owner_constraint() -> impl Strategy<Value = OwnerConstraint> {
    prop_oneof![
        Just(OwnerConstraint::Any),
        prop::collection::btree_set(arb_resource_ref(), 0..4)
            .prop_map(|refs| OwnerConstraint::OneOf { refs }),
    ]
}

pub(crate) fn arb_capability_ref() -> impl Strategy<Value = CapabilityRef> {
    (arb_ref_text(), any::<u32>(), any::<u32>()).prop_map(|(id, major, minor)| CapabilityRef {
        id: CapabilityId::new(id).expect("generated capability id is always valid"),
        version: CapabilityVersion::new(major, minor),
    })
}

pub(crate) fn arb_resource_scope() -> impl Strategy<Value = ResourceScope> {
    prop_oneof![
        prop::collection::btree_set(arb_resource_ref(), 0..4)
            .prop_map(|resources| ResourceScope::Exact { resources }),
        (
            prop::collection::btree_set(arb_resource_ref(), 0..4),
            any::<u64>(),
        )
            .prop_map(|(resources, rev)| ResourceScope::Snapshot {
                resources,
                captured_revision: ProjectRevision(rev),
            }),
        (
            arb_owner_constraint(),
            prop::collection::btree_set(arb_resource_kind(), 0..4),
            prop::option::of(arb_typed_range()),
            prop::option::of(any::<u32>()),
            any::<bool>(),
            prop::option::of((prop::option::of(arb_text()), prop::option::of(arb_text()),)),
        )
            .prop_map(
                |(owners, kinds, range, max_members, include_future, move_args)| {
                    ResourceScope::Dynamic(DynamicScope {
                        owners,
                        kinds,
                        range,
                        max_members,
                        include_future,
                        move_args: move_args.map(|(parent_arg, range_arg)| MoveArgs {
                            parent_arg,
                            range_arg,
                        }),
                    })
                },
            ),
    ]
}

pub(crate) fn arb_validity() -> impl Strategy<Value = Validity> {
    (prop::option::of(any::<u64>()), any::<bool>()).prop_map(|(expires_at, revoked)| Validity {
        expires_at: expires_at.map(PolicyInstant::new),
        revoked,
    })
}

/// Field tuple groups for [`make_grant`] — aliases keep the nested
/// strategy signatures readable.
pub(crate) type GrantHead = (GrantRef, Principal, Principal, ProjectId, GrantBinding);
pub(crate) type GrantSets = (
    BTreeSet<CapabilityRef>,
    BTreeSet<PermissionKind>,
    BTreeSet<DataAccessClass>,
    ResourceScope,
);
pub(crate) type GrantTail = (
    Option<CreateWithin>,
    ApprovalPolicy,
    (Option<u64>, Option<u64>),
    Validity,
    Option<PolicyEpoch>,
    Option<ProjectRevision>,
);

/// Assembles a [`Grant`] from the generated field tuples — a separate
/// fn so `arb_grant` stays under the line-count lint.
pub(crate) fn make_grant(head: GrantHead, sets: GrantSets, tail: GrantTail) -> Grant {
    let (id, issued_by, principal, project, binding) = head;
    let (capabilities, permissions, data_access, scope) = sets;
    let (
        create_within,
        approval_policy,
        (apply_budget, job_budget),
        validity,
        epoch_ceiling,
        issued_at_revision,
    ) = tail;
    Grant {
        id,
        issued_by,
        principal,
        project,
        binding,
        capabilities,
        permissions: PermissionSet::of(permissions),
        data_access: DataAccessSet::of(data_access),
        scope,
        create_within,
        approval_policy,
        limits: GrantLimits {
            apply_budget,
            job_budget,
        },
        validity,
        epoch_ceiling,
        issued_at_revision,
    }
}

pub(crate) fn arb_grant() -> impl Strategy<Value = Grant> {
    let head = (
        arb_grant_ref(),
        arb_principal(),
        arb_principal(),
        arb_project_id(),
        prop_oneof![
            arb_task_ref().prop_map(|task| GrantBinding::Task { task }),
            any::<u128>().prop_map(|raw| GrantBinding::Session {
                adapter: AdapterId::new(raw),
            }),
            Just(GrantBinding::PersistentProject),
        ],
    );
    let sets = (
        prop::collection::btree_set(arb_capability_ref(), 0..4),
        prop::collection::btree_set(
            prop_oneof![
                Just(PermissionKind::ReadContext),
                Just(PermissionKind::DiscloseContext),
                Just(PermissionKind::MutateProject),
                Just(PermissionKind::ControlTransport),
                Just(PermissionKind::RunJob),
                Just(PermissionKind::ExternalAction),
            ],
            0..6,
        ),
        prop::collection::btree_set(
            prop_oneof![
                Just(DataAccessClass::MetadataRead),
                Just(DataAccessClass::MediaBytesRead),
                Just(DataAccessClass::StructuredContentRead),
                Just(DataAccessClass::DerivedDataRead),
                Just(DataAccessClass::WorkspaceSelectionRead),
                Just(DataAccessClass::ExternalDisclosure),
                Just(DataAccessClass::ProviderTransfer),
            ],
            0..7,
        ),
        arb_resource_scope(),
    );
    let tail = (
        prop::option::of(
            (
                arb_owner_constraint(),
                prop::collection::btree_set(arb_resource_kind(), 0..4),
                prop::option::of((arb_ref_text(), arb_typed_range())),
            )
                .prop_map(|(parents, kinds, placement)| CreateWithin {
                    parents,
                    kinds,
                    placement: placement.map(|(arg, bound)| PlacementConstraint { arg, bound }),
                }),
        ),
        prop_oneof![
            Just(ApprovalPolicy::Suggest),
            Just(ApprovalPolicy::ApproveToApply),
            Just(ApprovalPolicy::DelegatedApply),
        ],
        (
            prop::option::of(any::<u64>()),
            prop::option::of(any::<u64>()),
        ),
        arb_validity(),
        prop::option::of(any::<u64>()).prop_map(|o| o.map(PolicyEpoch)),
        prop::option::of(any::<u64>()).prop_map(|o| o.map(ProjectRevision)),
    );
    (head, sets, tail).prop_map(|(head, sets, tail)| make_grant(head, sets, tail))
}

pub(crate) fn arb_activation() -> impl Strategy<Value = Activation> {
    prop_oneof![
        Just(Activation::UserIssued),
        arb_permit_ref().prop_map(|permit| Activation::Proactive { permit }),
    ]
}

pub(crate) fn arb_policy_event() -> impl Strategy<Value = PolicyEvent> {
    prop_oneof![
        (
            arb_task_ref(),
            arb_principal(),
            arb_project_id(),
            arb_activation(),
            arb_validity(),
        )
            .prop_map(|(task, principal, project, activation, validity)| {
                PolicyEvent::IssueTask(Box::new(AuthorizedTask {
                    task,
                    principal,
                    project,
                    activation,
                    validity,
                }))
            }),
        (
            arb_permit_ref(),
            arb_principal(),
            arb_project_id(),
            arb_text(),
            arb_validity(),
        )
            .prop_map(|(id, principal, project, condition, validity)| {
                PolicyEvent::IssuePermit(Box::new(ProactivePermit {
                    id,
                    principal,
                    project,
                    condition,
                    validity,
                }))
            }),
        arb_grant().prop_map(|g| PolicyEvent::IssueGrant(Box::new(g))),
        arb_task_ref().prop_map(PolicyEvent::RevokeTask),
        arb_permit_ref().prop_map(PolicyEvent::RevokePermit),
        arb_grant_ref().prop_map(PolicyEvent::RevokeGrant),
        Just(PolicyEvent::EpochAdvanced),
    ]
}

pub(crate) fn arb_commit_record() -> impl Strategy<Value = CommitRecord> {
    (
        (
            prop::option::of(any::<u128>().prop_map(OperationId::new)),
            prop::option::of(arb_proposal_ref()),
            arb_principal(),
            arb_project_id(),
            any::<u64>(),
            any::<u64>(),
            prop::option::of(arb_idem()),
        ),
        (
            prop::option::of(arb_grant_ref()),
            any::<bool>(),
            prop::option::of(arb_approval_ref()),
            prop::option::of(arb_text()),
            prop::option::of(any::<u64>()),
            prop::collection::vec(any::<u8>(), 0..=256),
            prop::option::of(arb_text()),
        ),
    )
        .prop_map(
            |(
                (operation, proposal, principal, project, base_revision, revision, idempotency),
                (grant, budget_spend, approval, digest, policy_lsn, state, provenance),
            )| {
                CommitRecord {
                    operation,
                    proposal,
                    principal,
                    project,
                    base_revision: ProjectRevision(base_revision),
                    revision: ProjectRevision(revision),
                    idempotency,
                    grant,
                    budget_spend,
                    approval,
                    digest,
                    policy_lsn,
                    state,
                    provenance,
                }
            },
        )
}

pub(crate) fn arb_outcome_record() -> impl Strategy<Value = OutcomeRecord> {
    (
        any::<u128>().prop_map(OperationId::new),
        prop::option::of(arb_proposal_ref()),
        arb_principal(),
        arb_project_id(),
        arb_text(),
        prop::option::of(any::<u64>()),
        prop::option::of(any::<u64>()),
        prop::option::of(arb_grant_ref()),
        any::<bool>(),
        prop::option::of(arb_idem()),
    )
        .prop_map(
            |(
                operation,
                proposal,
                principal,
                project,
                reason,
                expected,
                actual,
                grant,
                budget_spend,
                idempotency,
            )| {
                OutcomeRecord {
                    operation,
                    proposal,
                    principal,
                    project,
                    reason,
                    expected: expected.map(ProjectRevision),
                    actual: actual.map(ProjectRevision),
                    grant,
                    budget_spend,
                    idempotency,
                }
            },
        )
}

pub(crate) fn arb_journal_payload() -> impl Strategy<Value = JournalPayload> {
    prop_oneof![
        arb_commit_record().prop_map(|r| JournalPayload::Committed(Box::new(r))),
        arb_outcome_record().prop_map(|r| JournalPayload::Rejected(Box::new(r))),
        arb_outcome_record().prop_map(|r| JournalPayload::Conflicted(Box::new(r))),
        arb_outcome_record().prop_map(|r| JournalPayload::Indeterminate(Box::new(r))),
        arb_policy_event().prop_map(JournalPayload::Policy),
        (arb_project_id(), any::<u64>(), arb_text()).prop_map(|(project, rev, head)| {
            JournalPayload::Saved(SaveRecord {
                project,
                revision: ProjectRevision(rev),
                head,
            })
        }),
    ]
}
