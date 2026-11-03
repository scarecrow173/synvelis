//! GW-10 independent expectation oracle (test/PoC evidence only).
//!
//! The oracle computes *expected* gateway decisions from facts the test
//! declares — issued tasks, grants, revocations, consumed approvals,
//! project revision — using a small rule table derived from the
//! gateway contract (Issues #45/#46). It shares no code with
//! [`crate::policy::GrantPolicy`]: where the implementation evaluates
//! typed policy objects, the oracle re-expresses the externally visible
//! rule ("a revoked grant denies", "approval precedes apply") so a
//! passing comparison is cross-validation, not self-agreement.
//!
//! The oracle never learns from gateway outputs — tests feed it only
//! the facts they planted, then compare the oracle's [`Expected`] with
//! the observed [`crate::GatewayOutcome`].

use std::collections::{BTreeMap, BTreeSet};

/// The decision class a scenario expects — compared at the level
/// the contract distinguishes, not by reason-string equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expected {
    /// The operation is authorized to proceed.
    Permit,
    /// The operation is refused (any denial reason class).
    Deny,
    /// The apply pauses awaiting a human approval.
    ApprovalRequired,
    /// The operation aborts on a precondition (revision/digest drift).
    Conflict,
    /// The outcome cannot be decided (durable record absent).
    Unknown,
}

/// The authorization stage a request sits at when it is judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OracleStage {
    /// Read/query stage — disclosure classes dominate.
    Context,
    /// Semantic-preparation stage — scope and permissions dominate.
    Prepare,
    /// Apply-gate stage — the approval policy dominates.
    Apply,
    /// Commit-gate stage — revalidation against the *current* store.
    Commit,
    /// Job admission stage — the job permissions dominate.
    Job,
}

/// A task fact the scenario planted.
#[derive(Clone, Debug)]
pub struct OracleTask {
    /// Principal string the task was issued to.
    pub principal: String,
    /// Project string the task binds to.
    pub project: String,
    /// Whether the task was later revoked.
    pub revoked: bool,
}

/// A grant fact the scenario planted.
#[derive(Clone, Debug)]
pub struct OracleGrant {
    /// Principal string the grant was issued to.
    pub principal: String,
    /// Project string the grant binds to.
    pub project: String,
    /// Task string the grant binds to (empty = unbound).
    pub task: String,
    /// Capability ids the grant covers.
    pub capabilities: BTreeSet<String>,
    /// Permission names (`"MutateProject"`, `"ReadContext"`, ...).
    pub permissions: BTreeSet<String>,
    /// Data-access class names the grant discloses.
    pub data_access: BTreeSet<String>,
    /// Resource id strings in scope (empty set = no resources).
    pub scope: BTreeSet<String>,
    /// Approval policy name (`"DelegatedApply"` ...).
    pub policy: String,
    /// Remaining delegated-apply budget (`None` = unlimited).
    pub budget: Option<u64>,
    /// Whether the grant was later revoked.
    pub revoked: bool,
    /// Whether the grant's validity window has expired.
    pub expired: bool,
}

/// An approval receipt fact the scenario planted.
#[derive(Clone, Debug)]
pub struct OracleApproval {
    /// Principal the receipt was issued for.
    pub principal: String,
    /// Project the receipt binds to.
    pub project: String,
    /// Prepared-changeset digest the receipt binds to.
    pub digest: String,
    /// Whether the receipt was already consumed.
    pub consumed: bool,
}

/// What a request presents — only the facts the driver sent on the
/// wire, never implementation internals.
#[derive(Clone, Debug, Default)]
pub struct ReqView {
    /// The caller's principal string.
    pub principal: String,
    /// Presented task ref string.
    pub task: Option<String>,
    /// Presented grant ref string.
    pub grant: Option<String>,
    /// Presented capability id string.
    pub capability: String,
    /// The permission the stage requires.
    pub permission: Option<&'static str>,
    /// The disclosure class the read needs.
    pub data_class: Option<&'static str>,
    /// Target resource id strings the request touches.
    pub targets: Vec<String>,
    /// The revision the request pinned.
    pub expected_revision: Option<u64>,
    /// A presented approval receipt id.
    pub approval: Option<String>,
    /// The digest the request claims to apply.
    pub digest: Option<String>,
}

/// The event that confirmed first in an ordered pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstConfirmed {
    /// A revocation landed first — a later same-grant commit denies.
    Revoke,
    /// A commit landed first — it stands; revoke does not auto-undo.
    Commit,
}

/// The ordered-pair verdict the contract prescribes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderedVerdict {
    /// The commit is refused because revocation preceded it.
    CommitDenied,
    /// The commit stands; revocation still lands afterward.
    CommitStands,
}

/// The planted-facts oracle. All lookups are by the *string* forms the
/// wire carries — the oracle never dereferences typed ids.
#[derive(Clone, Default)]
pub struct SpecOracle {
    tasks: BTreeMap<String, OracleTask>,
    grants: BTreeMap<String, OracleGrant>,
    approvals: BTreeMap<String, OracleApproval>,
    revisions: BTreeMap<String, u64>,
}

impl SpecOracle {
    /// A fresh, empty oracle — every lookup denies (closed world).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Plants a task fact.
    pub fn with_task(&mut self, task: &str, fact: OracleTask) -> &mut Self {
        self.tasks.insert(task.to_owned(), fact);
        self
    }

    /// Plants a grant fact.
    pub fn with_grant(&mut self, grant: &str, fact: OracleGrant) -> &mut Self {
        self.grants.insert(grant.to_owned(), fact);
        self
    }

    /// Plants an approval-receipt fact.
    pub fn with_approval(&mut self, approval: &str, fact: OracleApproval) -> &mut Self {
        self.approvals.insert(approval.to_owned(), fact);
        self
    }

    /// Plants a project revision fact.
    pub fn with_revision(&mut self, project: &str, revision: u64) -> &mut Self {
        self.revisions.insert(project.to_owned(), revision);
        self
    }

    /// Marks a planted grant revoked (ordering scenarios mutate the
    /// oracle mid-flight).
    pub fn revoke_grant(&mut self, grant: &str) {
        if let Some(g) = self.grants.get_mut(grant) {
            g.revoked = true;
        }
    }

    /// Advances a planted revision (concurrent-edit scenarios).
    pub fn bump_revision(&mut self, project: &str) {
        if let Some(r) = self.revisions.get_mut(project) {
            *r += 1;
        }
    }

    /// The contract rule table: which decision must a request meeting
    /// `facts` produce at `stage`?
    ///
    /// Rules are evaluated in contract order — the first broken
    /// precondition decides, because the contract only guarantees the
    /// verdict *class*, never which reason surfaces first.
    #[must_use]
    pub fn expect(&self, stage: OracleStage, project: &str, req: &ReqView) -> Expected {
        let Some(task_id) = req.task.as_deref() else {
            return Expected::Deny;
        };
        let Some(task) = self.tasks.get(task_id) else {
            return Expected::Deny;
        };
        if task.revoked || task.principal != req.principal || task.project != project {
            return Expected::Deny;
        }
        let Some(grant_id) = req.grant.as_deref() else {
            return Expected::Deny;
        };
        let Some(grant) = self.grants.get(grant_id) else {
            return Expected::Deny;
        };
        if grant.revoked
            || grant.expired
            || grant.principal != req.principal
            || grant.project != project
            || (!grant.task.is_empty() && grant.task != task_id)
        {
            return Expected::Deny;
        }
        if !grant.capabilities.contains(&req.capability) {
            return Expected::Deny;
        }
        if let Some(perm) = req.permission
            && !grant.permissions.contains(perm)
        {
            return Expected::Deny;
        }
        if let Some(class) = req.data_class
            && !grant.data_access.contains(class)
        {
            return Expected::Deny;
        }
        if stage != OracleStage::Context
            && !grant.scope.is_empty()
            && !req.targets.iter().all(|t| grant.scope.contains(t))
        {
            return Expected::Deny;
        }
        if stage == OracleStage::Apply || stage == OracleStage::Commit {
            match grant.policy.as_str() {
                "Suggest" => return Expected::Deny,
                "ApproveToApply" => {
                    let Some(receipt) = req.approval.as_deref() else {
                        return Expected::ApprovalRequired;
                    };
                    let bound = self.approvals.get(receipt).is_some_and(|a| {
                        !a.consumed
                            && a.principal == req.principal
                            && a.project == project
                            && req.digest.as_deref() == Some(a.digest.as_str())
                    });
                    if !bound {
                        return Expected::Deny;
                    }
                }
                "DelegatedApply" if grant.budget == Some(0) => {
                    return Expected::Deny;
                }
                _ => {}
            }
        }
        if stage == OracleStage::Commit
            && let (Some(expected), Some(actual)) =
                (req.expected_revision, self.revisions.get(project))
            && expected != *actual
        {
            return Expected::Conflict;
        }
        Expected::Permit
    }

    /// The linearization verdict: when a revoke and a commit on the
    /// same grant race, the one confirmed first wins — the contract
    /// names revoke-first a denial and commit-first a standing mutation
    /// (no auto-undo). The *loser* is never silently swallowed.
    #[must_use]
    pub fn ordered_verdict(first: FirstConfirmed) -> OrderedVerdict {
        match first {
            FirstConfirmed::Revoke => OrderedVerdict::CommitDenied,
            FirstConfirmed::Commit => OrderedVerdict::CommitStands,
        }
    }
}
