---
id: "IMPL-034XPMJDlse0HdFeZh9vJ3"
type: "impl"
status: "in-progress"
title: "CB-10 creative branch gateway typed contract slice"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# CB-10 creative branch gateway typed contract slice

## Summary

Headless typed contract slice for issue `#287` (SV-CB-10): register the
`synvelis.branch.{fork,edit,adopt,discard,list,describe,compare}`
capabilities on the command gateway, add `variant-set`/`branch` resource
kinds, and route branch mutations through the existing
prepare → proposal → apply → `CommitGate` authorized commit path so
fork/edit/adopt/discard inherit CAS, approval, idempotency, journal, and
fail-closed settlement unchanged.

## Context

- Issue: `scarecrow173/synvelis#287` — Creative Branch Action/Workflow/CLI/AI
  Gateway integration, permissions, provenance. This slice covers only the
  headless typed gateway contract; CLI binary E2E, MCP stdio E2E,
  workflow-engine runs, GUI scenarios, RuntimePublish, and plugin/PCM work
  are explicitly follow-ups. The issue stays open (`Refs`, never `Closes`).
- Design authority: `docs/designs/creative-branch-design.md` §9.2 (CB-10 row)
  and the CB-01 variant engine shipped under
  `crates/synvelis-project/src/variant/` (`fork_variant_set`,
  `edit_branch`/`edit_branch_op`, `adopt_branch`/`adopt_branch_adapted`,
  `discard_branch`, `BranchContentStore`, `DomainAdapter`,
  `SnapshotResolver::capture_live`).
- Gateway authority: `Gateway` binds/authorizes; `PrepareEngine` lowers
  named mutation ops to `PreparedRecord`; `ProposalEngine` owns
  preview/approve/apply; `CommitGate` is the only fenced mutation path
  (revalidation → `ProjectController` CAS → journal → settle).

## Implementation

### Changes

<!-- Describe the important code or document changes. -->

### Decisions During Implementation

- **Mutation transport (design fork)**: Option (b) — a `GateDeps`
  authorized variant path inside the existing commit fence — was chosen
  over promoting variant operations into `ProjectOp`. `ProjectOp::apply`
  receives only `(&mut Project, &mut dyn IdGenerator)`; fork/edit/adopt
  need `DomainAdapter`, `BranchContentStore`, `SnapshotResolver`,
  `AdoptSources`, and `RuntimePublish`, none of which the `Operation`
  trait can carry. `VariantOp` stays `pub(crate)` inside
  `synvelis-project`; the gateway therefore carries a typed
  `VariantIntent` on `PreparedRecord` and `CommitGate` dispatches to the
  public `variant::*` engine functions inside the same fence, reusing
  ledger/idempotency/journal/settle verbatim.
- Capability ids follow the dispatch (`synvelis.branch.*`), not the
  issue's provisional `variant.*` names.
- `edit`/`adopt`/`discard`/`describe`/`compare` target the `VariantSet`
  resource with the `branch` id carried in args — the set is the
  aggregate root (`Project.variant_sets` → `set.branches`), matching how
  lane ops target the lane while naming track-scoped parents in args.
- `fork` uses `TargetContract::Create { parent: Project, creates:
  VariantSet }`; `list` targets `Project`; queries are `Query` domain,
  mutations `ProjectMutation` domain.

## Related Experiments

### Adopted

<!-- Record adopted experiment logs and why they were adopted. -->

### Rejected

<!-- Record rejected experiment logs and why they were rejected. -->

## Validation

<!-- Record the checks that were run. -->

## Risks

<!-- Record remaining risks. -->

## Follow-ups

- Real CLI binary E2E, MCP stdio E2E, workflow-engine execution, GUI
  scenarios, RuntimePublish wiring, plugin/PCM verification (issue #287
  remainder).
