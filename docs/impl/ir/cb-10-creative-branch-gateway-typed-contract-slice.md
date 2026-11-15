---
id: "IMPL-034XPMJDlse0HdFeZh9vJ3"
type: "impl"
status: "completed"
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

- `crates/synvelis-gateway/src/request.rs` — `ResourceKind::{VariantSet,
  Branch}` + `ResourceRef` variants + `"variant-set"`/`"branch"` wire
  parsing (committed separately as `b726162`).
- `crates/synvelis-gateway/src/variant.rs` (new) — the CB-10 module:
  `VariantIntent` (fork/edit/adopt/discard), `VariantPorts` (per-kind
  `DomainAdapter`s, shared `BranchContentStore`, `SnapshotResolver`,
  `RuntimePublish`, optional asset/plugin probes), `lower_*` arg
  lowering, `simulate` (scratch `ProjectController` + read-through
  `ScratchStore`), `execute` (commit-time dispatch), declared-facts
  scope roots, mutation/query descriptors + schemas, and
  `VariantQueryEngine` (`list`/`describe`/`compare` over pinned
  snapshots; compare decodes candidate payloads and re-derives scope
  digests).
- `crates/synvelis-gateway/src/prepare.rs` — `PreparedMutation::{Project,
  Variant}`; `PreparedRecord` retains the mutation enum with a
  `project_op()` compatibility accessor.
- `crates/synvelis-gateway/src/prepare_engine.rs` — variant op table +
  `with_variant_ports`; `lower_variant` schema-validates args, binds
  the declared target, lowers `branch.edit`'s nested *named* operation
  through the ordinary ops table (variant/batch members rejected), and
  produces `PreparedMutation::Variant`; prepare runs `variant::simulate`
  so candidate/effects/preview are the real post-state.
- `crates/synvelis-gateway/src/commit.rs` — `GateDeps.variant`; fenced
  dispatch inside the existing single-writer fence with an explicit
  expected-project-revision CAS check (domain ops derive their own CAS
  internally); adopt substitutes the consumed approval receipt id for
  the simulation ticket (absent → `Unapproved`); domain
  `StaleProjectRevision`/`StaleRevision` → `Conflicted`, other domain
  failures → determined `Rejected`; `CommitRecord`/`CommitReceipt`
  carry optional `provenance` (serde-default, replay-safe).
- `crates/synvelis-gateway/src/{lib,proposal,job,snapshot}.rs` — module
  export and exhaustive `ResourceRef`/`PreparedMutation` match arms.
- `crates/synvelis-project/src/variant/engine.rs` — `Clone`/`Debug`
  derives on `BranchEditOpIntent` (retained-record requirement).
- Host wiring: `crates/synvelis-cli/src/host.rs` and
  `crates/mcp-poc/src/main.rs` construct `VariantPorts::standard()`
  (shared `InMemoryBranchContentStore`, `NoopRuntimePublish`) for both
  prepare and commit; every other `GateDeps` site passes `variant:
  None` (fail-closed).
- Tests: `crates/synvelis-gateway/tests/cb10_variant_actions.rs` (14
  cases) + variant-aware `tests/rig/mod.rs` (`VariantRig`,
  `variant_rig()`, `gate_deps_variant`).

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
- **Prepare-time simulation honesty**: variant preparation clones live
  state into a scratch `ProjectController` and runs the real domain
  engine (`ScratchStore` reads through to the shared store but keeps
  writes local), so retained candidate/effects/preview cannot drift
  from what commit will do. For `adopt`, the scratch controller's
  commit counter differs from live, so simulation pins
  `expected_project_revision` to the scratch tip — project-revision CAS
  remains a commit-time check inside the fence; branch-revision and
  scope-digest CAS are still verified honestly at prepare.
- **Adopt approval**: the wire carries no approval token; lowering
  stores `SIMULATION_TICKET` as a placeholder and the commit gate
  substitutes the consumed approval receipt's id. `DelegatedApply`
  (no receipt) therefore fails `Unapproved` at commit — approval is
  never inferred.
- **Commit-time revision fence**: domain variant ops derive CAS from
  the live controller internally, which would bypass the apply's
  `expected_revision` assertion; the variant arm checks
  `controller.live_revision()` against the apply's expected revision
  inside the fence before dispatch.
- **Provenance**: the domain receipt JSON is journaled as commit
  provenance — evidence only, distinct from project-commit success;
  `RuntimePublish` pending/failed/not-requested is never reported as
  runtime truth.

## Related Experiments

### Adopted

- `docs/impl/exp/cb-10-creative-branch-gateway-typed-contract-slice.md`
  — scratch-simulation vs declared-effects experiment; simulation won
  because declared approximations cannot feed effect-aware
  authorization honestly.

### Rejected

- Promoting variant ops into `ProjectOp` — rejected: the `Operation`
  trait signature cannot carry adapter/store/resolver/publish ports
  (see Decisions).

## Validation

- `cargo check --workspace --all-targets` — clean.
- `cargo fmt --all -- --check` (`mise run fmt-check`) — clean.
- `mise run lint synvelis-gateway|synvelis-project|synvelis-cli|mcp-poc|synvelis-workspace`
  (clippy `-D warnings`) — clean.
- `cargo test -p synvelis-gateway` — all suites green, including the
  new `cb10_variant_actions` (14 tests: descriptor/schema lowering,
  prepare simulation without live mutation, capability-floor
  separation, nested-op/batch rejection, stale branch/project CAS,
  apply-stage revision fence, adopt approval consume + `Unapproved`
  fail-closed, discard, list/describe/compare verification,
  provenance + idempotent replay, missing-ports fail-closed).
- `cargo test` for `synvelis-project`, `synvelis-workspace`,
  `synvelis-cli`, `mcp-poc` — see `evidence/sv287/verification.md`.
- Canonical record: `evidence/sv287/verification.md`.

## Risks

- `VariantPorts::standard()` wires `NoopRuntimePublish` and an
  in-memory content store — production persistence/real runtime
  publish are follow-up slices (issue #287 remainder).
- Arrangement scope has no `DomainAdapter` and fails closed by
  design; scopes requiring it cannot fork until an adapter lands.
- Query `compare` verification depends on the shared content store
  being wired; unwired stores report `verified: false` honestly.

## Follow-ups

- Real CLI binary E2E, MCP stdio E2E, workflow-engine execution, GUI
  scenarios, RuntimePublish wiring, plugin/PCM verification (issue #287
  remainder).
