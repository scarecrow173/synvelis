//! The policy-store seam and its in-memory implementation (GW-02).
//!
//! The store is the authority for tasks, proactive permits, grants, the
//! policy epoch, and apply-budget consumption. Evaluation reads go
//! through the [`PolicyStore`] trait; [`GrantPolicy`][crate::GrantPolicy]
//! never touches store internals.
//!
//! # Linearization contract (ACG-003, presented to GW-06)
//!
//! Every trait method is a single atomic store operation. A revoke
//! confirmed **before** a commit-time read is observed by that read and
//! denies; a revoke confirmed **after** it does not retroactively undo
//! an authorization the commit already relied on — the commit stands
//! (revocation is not auto-undo). GW-06's fence serializes the commit
//! read of `grant()`/`policy_epoch()`/`try_consume_apply` with the
//! project commit: whichever side of the serialization point the
//! revocation lands on decides the outcome. The in-memory store orders
//! operations under one `RwLock`, which satisfies the contract at mock
//! level; the durable store and the crash/recovery story are GW-06's.

use std::collections::BTreeMap;
use std::sync::RwLock;

use serde::Serialize;

use crate::grant::{Grant, PolicyEpoch, Validity};
use crate::id::{GrantRef, ProactivePermitRef, TaskRef};
use crate::request::Principal;
use synvelis_project::ProjectId;

/// A user-issued or explicitly-permitted proactive task activation
/// (AHG-007: activation authority is a separate contract from the
/// execution grant).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "activation", rename_all = "kebab-case")]
pub enum Activation {
    /// The user explicitly activated this task.
    UserIssued,
    /// The task was started proactively under a registered proactive
    /// permit — the permit itself is re-checked at evaluation time, so
    /// a revoked/expired permit stops authorizing.
    Proactive {
        /// The proactive permit this activation relies on.
        permit: ProactivePermitRef,
    },
}

/// An authorized task — activation authority, separate from any
/// execution grant (AHG-007). An agent request resolves its `task` ref
/// here; holding the ref proves nothing by itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizedTask {
    /// The task's stable reference.
    pub task: TaskRef,
    /// The principal the task belongs to.
    pub principal: Principal,
    /// The project the task belongs to.
    pub project: ProjectId,
    /// How the task was activated.
    pub activation: Activation,
    /// Expiry / revocation.
    pub validity: Validity,
}

/// An explicit proactive-activation permit (AHG-007).
///
/// A delegated-apply grant is **not** proactive authority — only a
/// permit of this kind authorizes a proactively-started task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProactivePermit {
    /// The permit's stable reference.
    pub id: ProactivePermitRef,
    /// The principal the permit authorizes proactive activation for.
    pub principal: Principal,
    /// The project the permit applies to.
    pub project: ProjectId,
    /// A human-readable condition tag recording what the user allowed
    /// (the mock keeps it opaque; richer condition typing is a
    /// GW-09/policy-schema concern).
    pub condition: String,
    /// Expiry / revocation.
    pub validity: Validity,
}

/// The grant record: the immutable grant plus mutable consumption
/// state, read atomically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantRecord {
    /// The issued grant.
    pub grant: Grant,
    /// Delegated applies already consumed against the grant's budget.
    pub consumed_applies: u64,
}

/// The evaluation-time seam to policy state.
///
/// Each method is one atomic operation — the linearization unit a
/// commit fence composes (see module docs).
pub trait PolicyStore: Send + Sync {
    /// The current grant record for `id`.
    fn grant(&self, id: &GrantRef) -> Option<GrantRecord>;

    /// The authorized-task record for `id`.
    fn task(&self, id: &TaskRef) -> Option<AuthorizedTask>;

    /// The proactive permit for `id`.
    fn proactive_permit(&self, id: &ProactivePermitRef) -> Option<ProactivePermit>;

    /// The current policy epoch.
    fn policy_epoch(&self) -> PolicyEpoch;

    /// Atomically revalidates `id`'s grant and consumes one
    /// delegated-apply unit against its budget. Returns `false`
    /// without consuming when the grant is absent, dead (revoked or
    /// expired at `now`), epoch-stale, or the budget is exhausted —
    /// the check and the spend are one operation, so a revoke landing
    /// between the decision read and the spend is observed here
    /// (ACG-003).
    fn try_consume_apply(&self, id: &GrantRef, now: crate::clock::PolicyInstant) -> bool;
}

#[derive(Default)]
struct StoreState {
    tasks: BTreeMap<TaskRef, AuthorizedTask>,
    proactive_permits: BTreeMap<ProactivePermitRef, ProactivePermit>,
    grants: BTreeMap<GrantRef, GrantRecord>,
    epoch: PolicyEpoch,
}

/// In-memory policy store — the mock implementation Issue #54 allows
/// ahead of GW-06's durable fence.
///
/// All operations serialize on one `RwLock`, so reads and the
/// check-and-consume of `try_consume_apply` are linearizable. Ids are
/// single-issuance: issuing under an existing id is a no-op — the
/// existing record keeps its revocation and consumption state, so a
/// confirmed revocation can never be silently resurrected and a
/// consumed budget never refilled by reissue.
#[derive(Default)]
pub struct InMemoryPolicyStore {
    state: RwLock<StoreState>,
}

impl InMemoryPolicyStore {
    /// An empty store at epoch zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Issues an authorized task. Single-issuance: an existing id is
    /// kept (its revocation state cannot be reset by reissue).
    ///
    /// # Panics
    ///
    /// Panics if the store's lock is poisoned.
    pub fn issue_task(&self, task: AuthorizedTask) {
        self.state
            .write()
            .expect("policy store poisoned")
            .tasks
            .entry(task.task.clone())
            .or_insert(task);
    }

    /// Confirms revocation of a task (recorded, not deleted).
    ///
    /// # Panics
    ///
    /// Panics if the store's lock is poisoned.
    pub fn revoke_task(&self, id: &TaskRef) {
        if let Some(task) = self
            .state
            .write()
            .expect("policy store poisoned")
            .tasks
            .get_mut(id)
        {
            task.validity.revoked = true;
        }
    }

    /// Issues a proactive permit. Single-issuance: an existing id is
    /// kept.
    ///
    /// # Panics
    ///
    /// Panics if the store's lock is poisoned.
    pub fn issue_proactive_permit(&self, permit: ProactivePermit) {
        self.state
            .write()
            .expect("policy store poisoned")
            .proactive_permits
            .entry(permit.id.clone())
            .or_insert(permit);
    }

    /// Confirms revocation of a proactive permit.
    ///
    /// # Panics
    ///
    /// Panics if the store's lock is poisoned.
    pub fn revoke_proactive_permit(&self, id: &ProactivePermitRef) {
        if let Some(permit) = self
            .state
            .write()
            .expect("policy store poisoned")
            .proactive_permits
            .get_mut(id)
        {
            permit.validity.revoked = true;
        }
    }

    /// Issues a grant with a fresh consumption counter.
    /// Single-issuance: an existing id is kept — its revocation and
    /// consumption state cannot be reset by reissue.
    ///
    /// # Panics
    ///
    /// Panics if the store's lock is poisoned.
    pub fn issue_grant(&self, grant: Grant) {
        self.state
            .write()
            .expect("policy store poisoned")
            .grants
            .entry(grant.id.clone())
            .or_insert(GrantRecord {
                grant,
                consumed_applies: 0,
            });
    }

    /// Confirms revocation of a grant (recorded, not deleted).
    ///
    /// A subsequent evaluation observing this record denies — the
    /// linearization contract with an in-flight commit is the fence's
    /// (see module docs).
    ///
    /// # Panics
    ///
    /// Panics if the store's lock is poisoned.
    pub fn revoke_grant(&self, id: &GrantRef) {
        if let Some(record) = self
            .state
            .write()
            .expect("policy store poisoned")
            .grants
            .get_mut(id)
        {
            record.grant.validity.revoked = true;
        }
    }

    /// Advances the policy epoch — a policy-version update. Grants with
    /// an `epoch_ceiling` below the new epoch are stale.
    ///
    /// # Panics
    ///
    /// Panics if the store's lock is poisoned.
    pub fn update_policy(&self) -> PolicyEpoch {
        let state = &mut self.state.write().expect("policy store poisoned");
        state.epoch = PolicyEpoch(state.epoch.0.saturating_add(1));
        state.epoch
    }
}

impl PolicyStore for InMemoryPolicyStore {
    fn grant(&self, id: &GrantRef) -> Option<GrantRecord> {
        self.state
            .read()
            .expect("policy store poisoned")
            .grants
            .get(id)
            .cloned()
    }

    fn task(&self, id: &TaskRef) -> Option<AuthorizedTask> {
        self.state
            .read()
            .expect("policy store poisoned")
            .tasks
            .get(id)
            .cloned()
    }

    fn proactive_permit(&self, id: &ProactivePermitRef) -> Option<ProactivePermit> {
        self.state
            .read()
            .expect("policy store poisoned")
            .proactive_permits
            .get(id)
            .cloned()
    }

    fn policy_epoch(&self) -> PolicyEpoch {
        self.state.read().expect("policy store poisoned").epoch
    }

    fn try_consume_apply(&self, id: &GrantRef, now: crate::clock::PolicyInstant) -> bool {
        let mut state = self.state.write().expect("policy store poisoned");
        let epoch = state.epoch;
        let Some(record) = state.grants.get_mut(id) else {
            return false;
        };
        if !record.grant.validity.live(now)
            || record
                .grant
                .epoch_ceiling
                .is_some_and(|ceiling| epoch > ceiling)
            || record
                .grant
                .limits
                .apply_budget
                .is_some_and(|budget| record.consumed_applies >= budget)
        {
            return false;
        }
        record.consumed_applies = record.consumed_applies.saturating_add(1);
        true
    }
}
