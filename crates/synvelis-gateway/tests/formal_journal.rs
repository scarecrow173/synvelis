//! Issue #601 slice 1 (D-layer formal verification) — commit journal
//! wire codec property suite plus `#[cfg(kani)]` bounded proof
//! harnesses.
//!
//! Target contracts (product source of truth:
//! `crates/synvelis-gateway/src/journal.rs` for the frame codec,
//! `src/commit.rs` for `JournalPayload` serde, `src/id.rs` for the
//! u128 tagged-id wire form):
//!
//! - `replay(&[u8]) -> ReplayOutcome` never panics; `stop` stays in
//!   the closed `StopReason` set (compile-time exhaustive); `prefix_len`
//!   never exceeds the input length; `CleanEnd` iff every byte verified;
//!   verified record seqs are contiguous from 0; the verified prefix
//!   itself always replays to `CleanEnd`.
//! - `encode_frame(seq, payload)` emits the documented layout
//!   (magic/version/reserved/seq/len/payload/crc32), is `Some` for
//!   `payload_len <= MAX_PAYLOAD` and `None` above it, and an encoded
//!   stream replays to `CleanEnd` with identical payloads only when
//!   seqs are contiguous from 0.
//! - Every byte of a frame is covered by either a direct field check
//!   or the trailing CRC, so a single-byte corruption inside record
//!   k can never replay as intact past record k.
//! - `JournalPayload` (`#[serde(deny_unknown_fields)]` inner records)
//!   round-trips through `serde_json` losslessly and rejects injected
//!   unknown fields.
//! - u128 tagged ids serialize as `"<kind>-<32 lowercase hex>"` and
//!   re-validate the tag on decode.
//!
//! The `#[cfg(kani)]` module at the bottom carries the same contracts
//! as bounded proofs (run via `mise run test-formal-kani`); the
//! bounds and assumptions are documented in
//! `docs/specs/formal-verification.md`.

use std::collections::BTreeSet;

use proptest::prelude::*;
use serde_json::Value;
use synvelis_gateway::clock::PolicyInstant;
use synvelis_gateway::commit::{
    CommitRecord, JournalIdem, JournalPayload, OutcomeRecord, PolicyEvent, SaveRecord,
};
use synvelis_gateway::grant::{
    ApprovalPolicy, CreateWithin, DynamicScope, Grant, GrantBinding, GrantLimits, MoveArgs,
    OwnerConstraint, PlacementConstraint, PolicyEpoch, ResourceScope, Validity,
};
use synvelis_gateway::id::{
    AdapterId, ApprovalRef, CommitId, GrantRef, IdempotencyKey, OperationId, ProactivePermitRef,
    ProposalRef, TaskRef,
};
use synvelis_gateway::journal::{
    HEADER_LEN, MAGIC, MAX_PAYLOAD, StopReason, VERSION, crc32, encode_frame, replay,
};
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
fn arb_payload() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..=384)
}

/// A stream of encoded frames with contiguous seq from 0, plus the
/// byte offsets where each record starts.
fn encode_stream(payloads: &[Vec<u8>]) -> (Vec<u8>, Vec<usize>) {
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
fn arb_ref_text() -> impl Strategy<Value = String> {
    "[a-z0-9._-]{1,32}"
}

/// Arbitrary short free text (reason/head/condition/fingerprint/…).
fn arb_text() -> impl Strategy<Value = String> {
    ".{0,64}"
}

fn arb_principal() -> impl Strategy<Value = Principal> {
    (
        prop_oneof![Just(PrincipalKind::Human), Just(PrincipalKind::Agent)],
        arb_ref_text(),
    )
        .prop_map(|(kind, id)| {
            Principal::new(kind, id).expect("generated principal id is always valid")
        })
}

fn arb_project_id() -> impl Strategy<Value = ProjectId> {
    any::<u128>().prop_map(ProjectId::new)
}

fn arb_task_ref() -> impl Strategy<Value = TaskRef> {
    arb_ref_text().prop_map(|s| TaskRef::new(s).expect("generated ref is always valid"))
}

fn arb_grant_ref() -> impl Strategy<Value = GrantRef> {
    arb_ref_text().prop_map(|s| GrantRef::new(s).expect("generated ref is always valid"))
}

fn arb_proposal_ref() -> impl Strategy<Value = ProposalRef> {
    arb_ref_text().prop_map(|s| ProposalRef::new(s).expect("generated ref is always valid"))
}

fn arb_approval_ref() -> impl Strategy<Value = ApprovalRef> {
    arb_ref_text().prop_map(|s| ApprovalRef::new(s).expect("generated ref is always valid"))
}

fn arb_permit_ref() -> impl Strategy<Value = ProactivePermitRef> {
    arb_ref_text().prop_map(|s| ProactivePermitRef::new(s).expect("generated ref is always valid"))
}

fn arb_idem() -> impl Strategy<Value = JournalIdem> {
    (arb_ref_text(), arb_text()).prop_map(|(key, fingerprint)| JournalIdem {
        key: IdempotencyKey::new(key).expect("generated key is always valid"),
        fingerprint,
    })
}

fn arb_resource_ref() -> impl Strategy<Value = ResourceRef> {
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

fn arb_resource_kind() -> impl Strategy<Value = ResourceKind> {
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

fn arb_typed_range() -> impl Strategy<Value = TypedRange> {
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

fn arb_owner_constraint() -> impl Strategy<Value = OwnerConstraint> {
    prop_oneof![
        Just(OwnerConstraint::Any),
        prop::collection::btree_set(arb_resource_ref(), 0..4)
            .prop_map(|refs| OwnerConstraint::OneOf { refs }),
    ]
}

fn arb_capability_ref() -> impl Strategy<Value = CapabilityRef> {
    (arb_ref_text(), any::<u32>(), any::<u32>()).prop_map(|(id, major, minor)| CapabilityRef {
        id: CapabilityId::new(id).expect("generated capability id is always valid"),
        version: CapabilityVersion::new(major, minor),
    })
}

fn arb_resource_scope() -> impl Strategy<Value = ResourceScope> {
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

fn arb_validity() -> impl Strategy<Value = Validity> {
    (prop::option::of(any::<u64>()), any::<bool>()).prop_map(|(expires_at, revoked)| Validity {
        expires_at: expires_at.map(PolicyInstant::new),
        revoked,
    })
}

/// Field tuple groups for [`make_grant`] — aliases keep the nested
/// strategy signatures readable.
type GrantHead = (GrantRef, Principal, Principal, ProjectId, GrantBinding);
type GrantSets = (
    BTreeSet<CapabilityRef>,
    BTreeSet<PermissionKind>,
    BTreeSet<DataAccessClass>,
    ResourceScope,
);
type GrantTail = (
    Option<CreateWithin>,
    ApprovalPolicy,
    (Option<u64>, Option<u64>),
    Validity,
    Option<PolicyEpoch>,
    Option<ProjectRevision>,
);

/// Assembles a [`Grant`] from the generated field tuples — a separate
/// fn so `arb_grant` stays under the line-count lint.
fn make_grant(head: GrantHead, sets: GrantSets, tail: GrantTail) -> Grant {
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

fn arb_grant() -> impl Strategy<Value = Grant> {
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

fn arb_activation() -> impl Strategy<Value = Activation> {
    prop_oneof![
        Just(Activation::UserIssued),
        arb_permit_ref().prop_map(|permit| Activation::Proactive { permit }),
    ]
}

fn arb_policy_event() -> impl Strategy<Value = PolicyEvent> {
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

fn arb_commit_record() -> impl Strategy<Value = CommitRecord> {
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

fn arb_outcome_record() -> impl Strategy<Value = OutcomeRecord> {
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

fn arb_journal_payload() -> impl Strategy<Value = JournalPayload> {
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

/// Walks externally-tagged enum layers (`{"<tag>": <inner>}`) down to
/// the first multi-key object — a serde struct — and inserts an
/// unknown field. Returns `false` when the payload's innermost form is
/// a unit/leaf variant (no struct object to inject into).
fn inject_unknown_field(value: &mut Value) -> bool {
    let Value::Object(map) = value else {
        return false;
    };
    if map.len() == 1 {
        let Some(inner) = map.values_mut().next() else {
            return false;
        };
        return inject_unknown_field(inner);
    }
    map.insert("zz-unknown-field".to_owned(), Value::Null);
    true
}

/// Asserts the `"<kind>-<32 lowercase hex>"` wire contract for one
/// u128 tagged id value.
fn check_tagged_id<T>(id: &T, kind: &str)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let value = serde_json::to_value(id).expect("id must serialize");
    let text = value.as_str().expect("id serializes as a string");
    let prefix = format!("{kind}-");
    assert!(text.starts_with(&prefix), "{text} missing {kind} tag");
    let hex = text[prefix.len()..].to_owned();
    assert_eq!(hex.len(), 32, "{text} hex payload must be 32 chars");
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
        "{text} hex must be lowercase"
    );
    let back: T = serde_json::from_value(value).expect("id must round-trip");
    assert_eq!(&back, id);

    // wrong kind tag → rejection (a stored value can never mint an id
    // of the wrong kind)
    let wrong = Value::String(format!("wrong-kind-{hex}"));
    assert!(serde_json::from_value::<T>(wrong).is_err());
    // malformed hex payloads → rejection
    for bad_hex in [
        "zz".repeat(16),   // non-hex chars
        "f".repeat(31),    // short
        format!("{hex}0"), // long
    ] {
        let bad = Value::String(format!("{prefix}{bad_hex}"));
        assert!(
            serde_json::from_value::<T>(bad).is_err(),
            "{kind} accepted malformed hex {bad_hex}"
        );
    }
}

proptest! {
    /// For any byte input `replay` terminates without panic, `stop`
    /// stays in the closed `StopReason` set, `prefix_len` never exceeds
    /// the input length, `CleanEnd` holds iff every byte verified, and
    /// verified record seqs are contiguous from 0. The verified prefix
    /// alone always replays to `CleanEnd` with the same records.
    #[test]
    fn replay_arbitrary_bytes_contract(bytes in prop::collection::vec(any::<u8>(), 0..=512)) {
        let out = replay(&bytes);
        let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        prop_assert!(
            out.prefix_len <= len,
            "prefix_len {} > input len {len}",
            out.prefix_len,
        );
        prop_assert_eq!(out.stop == StopReason::CleanEnd, out.prefix_len == len);
        for (i, rec) in out.records.iter().enumerate() {
            prop_assert_eq!(rec.seq, u64::try_from(i).unwrap_or(u64::MAX));
        }
        let prefix_len = usize::try_from(out.prefix_len).unwrap_or(usize::MAX);
        let again = replay(&bytes[..prefix_len]);
        prop_assert_eq!(again.stop, StopReason::CleanEnd);
        prop_assert_eq!(again.records, out.records);
    }

    /// `encode_frame` emits the documented wire layout: LE magic, the
    /// current version, zeroed reserved byte, LE seq and payload_len,
    /// the payload verbatim, and a CRC32 over `frame[2..]` excluding
    /// the crc field itself.
    #[test]
    fn encode_frame_layout(seq in any::<u64>(), payload in arb_payload()) {
        let frame = encode_frame(seq, &payload).expect("in-cap payload must encode");
        prop_assert_eq!(frame.len(), HEADER_LEN + payload.len() + 4);
        prop_assert_eq!(&frame[0..2], &MAGIC.to_le_bytes());
        prop_assert_eq!(frame[2], VERSION);
        prop_assert_eq!(frame[3], 0);
        prop_assert_eq!(&frame[4..12], &seq.to_le_bytes());
        prop_assert_eq!(
            &frame[12..16],
            &u32::try_from(payload.len()).unwrap_or(u32::MAX).to_le_bytes(),
        );
        prop_assert_eq!(&frame[16..16 + payload.len()], payload.as_slice());
        let stored = u32::from_le_bytes(frame[frame.len() - 4..].try_into().unwrap_or_default());
        prop_assert_eq!(stored, crc32(&frame[2..frame.len() - 4]));
    }

    /// A frame carrying seq 0 replays to `CleanEnd` returning the
    /// identical payload.
    #[test]
    fn encode_seq0_replays_clean(payload in arb_payload()) {
        let frame = encode_frame(0, &payload).expect("in-cap payload must encode");
        let out = replay(&frame);
        prop_assert_eq!(out.stop, StopReason::CleanEnd);
        prop_assert_eq!(out.records.len(), 1);
        prop_assert_eq!(&out.records[0].payload, &payload);
    }

    /// A single frame with a non-zero seq is a `SequenceGap` — replay
    /// only accepts streams that start at seq 0.
    #[test]
    fn encode_nonzero_seq_is_gap(seq in 1u64..=u64::MAX, payload in arb_payload()) {
        let frame = encode_frame(seq, &payload).expect("in-cap payload must encode");
        let out = replay(&frame);
        prop_assert_eq!(out.stop, StopReason::SequenceGap);
        prop_assert!(out.records.is_empty());
        prop_assert_eq!(out.prefix_len, 0);
    }

    /// Any contiguous-seq encoded stream replays to `CleanEnd` with
    /// every payload identical and in order.
    #[test]
    fn encode_stream_roundtrip(payloads in prop::collection::vec(arb_payload(), 0..=8)) {
        let (stream, _) = encode_stream(&payloads);
        let out = replay(&stream);
        prop_assert_eq!(out.stop, StopReason::CleanEnd);
        prop_assert_eq!(out.prefix_len, u64::try_from(stream.len()).unwrap_or(u64::MAX));
        prop_assert_eq!(out.records.len(), payloads.len());
        for (i, rec) in out.records.iter().enumerate() {
            prop_assert_eq!(&rec.payload, &payloads[i]);
        }
    }

    /// Every byte of a frame is covered by a direct field check or
    /// the trailing CRC, so a single-byte corruption inside record k
    /// can never be verified as intact: replay must stop exactly at
    /// record k with only the earlier records verified.
    #[test]
    fn single_byte_corruption_never_verifies_past_it(
        payloads in prop::collection::vec(arb_payload(), 1..=4),
        rec_pick in any::<prop::sample::Index>(),
        ofs_pick in any::<prop::sample::Index>(),
        xor in 1u8..=255,
    ) {
        let (mut stream, starts) = encode_stream(&payloads);
        let k = rec_pick.index(payloads.len());
        let rec_len = encode_frame(0, &payloads[k]).expect("in-cap").len();
        let ofs = ofs_pick.index(rec_len);
        stream[starts[k] + ofs] ^= xor;
        let out = replay(&stream);
        prop_assert_eq!(out.records.len(), k, "corrupted record must not verify");
        prop_assert_eq!(
            out.prefix_len,
            u64::try_from(starts[k]).unwrap_or(u64::MAX),
            "prefix must end exactly at the corrupted record",
        );
        prop_assert_ne!(out.stop, StopReason::CleanEnd);
    }

    /// Truncating an encoded stream mid-record yields `TornTail`
    /// (the torn tail can only be an incomplete record); truncating at
    /// a record boundary keeps `CleanEnd`. Verified records are exactly
    /// the frames fully inside the kept prefix.
    #[test]
    fn truncation_is_torn_tail_or_boundary_clean(
        payloads in prop::collection::vec(arb_payload(), 1..=4),
        cut_pick in any::<prop::sample::Index>(),
    ) {
        let (stream, starts) = encode_stream(&payloads);
        let ends: Vec<usize> = starts
            .iter()
            .enumerate()
            .map(|(i, _)| {
                if i + 1 < starts.len() {
                    starts[i + 1]
                } else {
                    stream.len()
                }
            })
            .collect();
        let cut = cut_pick.index(stream.len() + 1);
        let out = replay(&stream[..cut]);
        let complete = ends.iter().filter(|&&e| e <= cut).count();
        let expected_prefix = complete.checked_sub(1).map_or(0, |i| ends[i]);
        let on_boundary = cut == stream.len() || starts.contains(&cut);
        prop_assert_eq!(out.records.len(), complete);
        prop_assert_eq!(
            out.prefix_len,
            u64::try_from(expected_prefix).unwrap_or(u64::MAX),
        );
        if on_boundary {
            prop_assert_eq!(out.stop, StopReason::CleanEnd);
        } else {
            prop_assert_eq!(out.stop, StopReason::TornTail);
        }
    }

    /// A valid stream followed by bytes that cannot start a frame
    /// stops exactly at the frame boundary — trailing garbage is never
    /// absorbed into the verified prefix. Fewer than `HEADER_LEN`
    /// trailing bytes is `TornTail` (the header itself can't be read);
    /// ≥ `HEADER_LEN` with a non-magic first byte is `BadMagic`.
    #[test]
    fn trailing_garbage_stops_at_boundary(
        payloads in prop::collection::vec(arb_payload(), 0..=4),
        first in any::<u8>(),
        tail in prop::collection::vec(any::<u8>(), 0..=48),
    ) {
        let (mut stream, _) = encode_stream(&payloads);
        let frames_len = stream.len();
        // force the tail's first byte off the magic value so the stop
        // reason is deterministic
        let first = if first == 0x4A { 0x4B } else { first };
        stream.push(first);
        stream.extend_from_slice(&tail);
        let out = replay(&stream);
        let tail_len = 1 + tail.len();
        let expected = if tail_len < HEADER_LEN {
            StopReason::TornTail
        } else {
            StopReason::BadMagic
        };
        prop_assert_eq!(out.stop, expected);
        prop_assert_eq!(out.records.len(), payloads.len());
        prop_assert_eq!(out.prefix_len, u64::try_from(frames_len).unwrap_or(u64::MAX));
    }

    /// `JournalPayload` round-trips through `serde_json` with every
    /// field preserved, across all six payload variants.
    #[test]
    fn journal_payload_serde_roundtrip(payload in arb_journal_payload()) {
        let bytes = serde_json::to_vec(&payload).expect("payload must serialize");
        let back: JournalPayload =
            serde_json::from_slice(&bytes).expect("generated payload must deserialize");
        prop_assert_eq!(back, payload);
    }

    /// `deny_unknown_fields` on the inner record types: an injected
    /// unknown field at any struct level makes the payload unparseable.
    /// Payloads whose innermost form is a leaf variant carry no struct
    /// object and are skipped (nothing exists to reject the field).
    #[test]
    fn journal_payload_rejects_unknown_fields(payload in arb_journal_payload()) {
        let mut value = serde_json::to_value(&payload).expect("payload must serialize");
        if inject_unknown_field(&mut value) {
            prop_assert!(
                serde_json::from_value::<JournalPayload>(value).is_err(),
                "payload accepted an unknown field",
            );
        }
    }

    /// u128 tagged ids serialize as `"<kind>-<32 lowercase hex>"`,
    /// round-trip losslessly, and reject wrong tags and malformed hex.
    #[test]
    fn u128_tagged_id_serde_contract(raw in any::<u128>()) {
        check_tagged_id(&OperationId::new(raw), "operation");
        check_tagged_id(&AdapterId::new(raw), "adapter");
        check_tagged_id(&CommitId::new(raw), "commit");
    }
}

proptest! {
    // ~16 MiB of payload per case — keep the case count small.
    #![proptest_config(ProptestConfig::with_cases(4))]

    /// `payload_len > MAX_PAYLOAD` is refused — the encoder never
    /// emits a frame the decoder's `BadLength` gate would reject.
    #[test]
    fn encode_frame_over_cap_refused(over in 1usize..=64) {
        let payload = vec![b'x'; usize::try_from(MAX_PAYLOAD).unwrap_or(usize::MAX) + over];
        prop_assert!(encode_frame(0, &payload).is_none());
    }
}

/// The exact `MAX_PAYLOAD` boundary still encodes and round-trips —
/// the cap rejects lengths *above* it, not at it.
#[test]
fn encode_frame_at_cap_roundtrips() {
    let payload = vec![b'y'; usize::try_from(MAX_PAYLOAD).unwrap_or(usize::MAX)];
    let frame = encode_frame(0, &payload).expect("at-cap payload must encode");
    let out = replay(&frame);
    assert_eq!(out.stop, StopReason::CleanEnd);
    assert_eq!(out.records.len(), 1);
    assert_eq!(
        out.records[0].payload.len(),
        usize::try_from(MAX_PAYLOAD).unwrap_or(usize::MAX),
    );
}

/// `deny_unknown_fields` on a hand-built record: the property suite
/// generalizes this, the example pins it.
#[test]
fn saved_record_rejects_unknown_field() {
    let saved = SaveRecord {
        project: ProjectId::new(7),
        revision: ProjectRevision(3),
        head: "main".to_owned(),
    };
    let payload = JournalPayload::Saved(saved);
    let mut value = serde_json::to_value(&payload).expect("serialize");
    value["saved"]["injected"] = Value::Bool(true);
    assert!(serde_json::from_value::<JournalPayload>(value).is_err());
    // and the untouched form still parses
    let bytes = serde_json::to_vec(&payload).expect("serialize");
    assert_eq!(
        serde_json::from_slice::<JournalPayload>(&bytes).unwrap(),
        payload,
    );
}

/// Bounded proofs of the same contracts, verified exhaustively over
/// symbolic input by Kani (`cargo kani --tests`; run via
/// `mise run test-formal-kani`). Bounds and assumptions:
/// `docs/specs/formal-verification.md`.
#[cfg(kani)]
mod proofs {
    use super::*;

    /// Bounded stream length — covers two minimal frames (each
    /// 16-byte header + 4-byte crc) plus slack so every `StopReason`
    /// path is reachable while keeping the crc/replay unwinding small.
    /// The `MAX_PAYLOAD` cap path and multi-MiB payloads are covered
    /// concretely by the proptest cases.
    const STREAM_LEN: usize = 44;

    /// A symbolic stream of symbolic length ≤ `STREAM_LEN`.
    fn any_stream() -> ([u8; STREAM_LEN], usize) {
        let bytes: [u8; STREAM_LEN] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= STREAM_LEN);
        (bytes, len)
    }

    /// `replay` terminates on every bounded stream and honors the
    /// classification contract: `prefix_len <= len`, `CleanEnd` iff
    /// the whole input verified, verified seqs contiguous from 0, and
    /// the verified prefix replays to `CleanEnd`. The unwind bound
    /// caps the replay loop and the per-byte crc loop (8 iterations
    /// per byte) over the ≤44-byte stream.
    #[kani::proof]
    #[kani::unwind(64)]
    fn replay_contract_bounded() {
        let (bytes, len) = any_stream();
        let out = replay(&bytes[..len]);
        let len64 = len as u64;
        assert!(out.prefix_len <= len64);
        assert!((out.stop == StopReason::CleanEnd) == (out.prefix_len == len64));
        for (i, rec) in out.records.iter().enumerate() {
            assert!(rec.seq == i as u64);
        }
        let again = replay(&bytes[..out.prefix_len as usize]);
        assert!(again.stop == StopReason::CleanEnd);
    }

    /// `encode_frame` → `replay` is the identity for a bounded
    /// symbolic seq-0 payload. Fixed 12-byte payload — a symbolic
    /// length would force CBMC to unwind `Vec` allocation and memcpy
    /// loops over a symbolic bound.
    #[kani::proof]
    #[kani::unwind(48)]
    fn encode_replay_roundtrip_bounded() {
        let payload: [u8; 12] = kani::any();
        let Some(frame) = encode_frame(0, &payload) else {
            unreachable!("in-cap payload must encode")
        };
        let out = replay(&frame);
        assert!(out.stop == StopReason::CleanEnd);
        assert!(out.records.len() == 1);
        assert!(out.records[0].seq == 0);
        assert!(out.records[0].payload[..] == payload[..]);
    }

    /// A non-zero seq frame is always a `SequenceGap` — shown
    /// exhaustively for the smallest payload.
    #[kani::proof]
    #[kani::unwind(48)]
    fn nonzero_seq_is_gap_bounded() {
        let payload: [u8; 8] = kani::any();
        let seq: u64 = kani::any();
        kani::assume(seq != 0);
        let Some(frame) = encode_frame(seq, &payload) else {
            unreachable!("in-cap payload must encode")
        };
        let out = replay(&frame);
        assert!(out.stop == StopReason::SequenceGap);
        assert!(out.records.is_empty());
        assert!(out.prefix_len == 0);
    }
}
