---
id: "TSPEC-034YCB10gwActions"
type: "test-spec"
status: "draft"
title: "CB-10 Creative Branch gateway typed contract — headless slice"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/287"
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
  related:
    - "evidence/sv287/verification.md"
    - "docs/impl/ir/cb-10-creative-branch-gateway-typed-contract-slice.md"
    - "docs/impl/exp/cb-10-creative-branch-gateway-typed-contract-slice.md"
    - "crates/synvelis-gateway/tests/cb10_variant_actions.rs"
    - "docs/specs/creative-branch.md"
    - "docs/designs/creative-branch-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/creative-branch-design.md"
  derived-by: []
  verifies:
    - "docs/specs/creative-branch.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# CB-10 Creative Branch gateway typed contract — headless slice

## Purpose

Issue #287（CB-10）の headless typed contract slice が、
`synvelis.branch.{fork,edit,adopt,discard,list,describe,compare}`
の 7 capability を既存 Capability/Command Gateway の
prepare → proposal → approval → apply → `CommitGate` pipeline に
正しく載せ、Gateway/`ProjectController` の権威境界・明示
targeting・認可分離・CAS・journal・provenance・fail-closed を
保持することを保証する。この spec は headless 契約のみを
対象とし、CLI binary E2E・MCP stdio E2E・workflow engine・
GUI・RuntimePublish・plugin/PCM・cross-gate 受入は対象外
（issue #287 残件）。headless Pass をそれらへ換算しない。

## Feature

Creative Branch typed gateway contract（headless）

## Rules

- Rule: branch mutation capability は `synvelis.operation.prepare`
  経由でのみ起動し、直接 mutation executor を持たない。
  query capability（`list`/`describe`/`compare`）は
  `CapabilityOperation` executor を持ち pinned snapshot のみ読む。
- Rule: 候補生成（`edit`）と live materialization（`adopt`）は
  別 capability であり、grant の capability floor が各々を
  個別に要求する。agent grant が `synvelis.branch.fork` のみを
  名指しすれば adopt は prepare できない。
- Rule: `adopt` は approval を推測しない。consumed approval
  receipt が無い apply（`DelegatedApply`）は commit で
  `unapproved` の determined reject となり、live state は
  不変。`ApproveToApply` + trusted approve の receipt id が
  domain の `ApplyApproval.ticket` に注入される。
- Rule: prepare は live project を変更しない。variant intent は
  scratch `ProjectController` + read-through `ScratchStore` で
  domain engine を実走し、retained candidate・derived effects・
  preview は実 post-state 由来。variant ports 未配線では
  prepare/commit とも fail-closed。
- Rule: revision CAS は多層で効く。prepare の
  `expected_branch_revision`/`expected_scope_digest` は
  simulation で検査、apply の base revision fence（SEC-11）と
  commit fence 内の `expected_project_revision` 再検査が
  stale prepared state の commit を止める。stale は常に
  determined `Conflicted`/`Rejected` であり silent success
  しない。
- Rule: `branch.edit` の nested op は named operation として
  既存 ops 表で lowering され、`ProjectOp` を wire serialize
  しない。variant capability・`synvelis.operation.batch` を
  nested member に取る edit は拒否。variant op 自体は batch
  member にもなれない。
- Rule: commit された variant mutation は domain receipt JSON を
  provenance として journal に残す。`ProjectCommitEvidence` と
  `RuntimePublishEvidence` は別 block であり、runtime publish
  の pending/failed/not-requested を project truth や runtime
  成功として表現しない。replay は provenance を receipt に
  復元し、通常 project/manual/undo/redo commit は
  `provenance: None`。
- Rule: query は pinned snapshot のみを読み mutate しない。
  `compare` は記録済み metadata を信用せず、captured payload を
  content store から decode して scope digest を再導出する。
  store 未配線・payload 不在は `verified: false` +
  verify_error で fail-closed に報告する。
- Rule: `describe`/`compare` は `VariantSet` resource target を
  必須とし、set id を args の独立権威として重複させない。

## Examples

| test | guarantees |
| --- | --- |
| `fork_prepares_without_mutating_then_commits_with_provenance` | prepare は simulation のみで live 不変、`PreparedMutation::Variant` 保持、commit で set+branch pair 生成・receipt provenance journal・同一 proposal の replay が settled receipt を返す |
| `fork_fails_closed_without_variant_ports` | ports 未配線の prepare は internal/misconfiguration error で拒否 |
| `fork_requires_the_branch_capability_in_the_grant` | `synvelis.clip.mute` のみの grant は fork を capability floor で拒否、`synvelis.branch.fork` 付与で prepare 成功（候補生成と adopt の権限分離） |
| `edit_candidate_branch_commits_payload_not_live` | candidate への edit は immutable payload に着地し live clip を変更しない、branch content revision 前進、compare が digest 不一致を正直に報告 |
| `edit_rejects_nested_variant_and_batch_ops` | nested op に variant capability や batch を取る edit は "ordinary project operations only" で拒否 |
| `edit_with_stale_branch_revision_fails_closed` | simulation が `stale-branch-revision` で prepare を拒否 |
| `stale_base_variant_proposal_cannot_apply_after_live_moves` | 同一 base の 2 prepare のうち先の commit 後、後者の apply は SEC-11 fence で `RevisionConflict` |
| `adopt_without_approval_fails_closed` | `DelegatedApply`（receipt 無し）の adopt は commit で `unapproved` determined reject、adopted branch 不変 |
| `adopt_with_consumed_receipt_commits` | `ApproveToApply` + human approve の receipt が ticket として注入され adopt commit 成功、payload が live scope に materialize、provenance に runtime evidence block |
| `discard_removes_the_branch` | discard commit で branch 除去・adopted branch 温存 |
| `queries_list_describe_and_compare_on_the_pin` | list/describe/compare が pinned state を read-only で報告、scope_kind filter、identical scope の digest equal、captured content の store 再検証 `verified: true` |
| `describe_and_compare_require_the_set_target` | target 欠落は `TargetRequired` で拒否 |
| `variant_ops_cannot_be_batch_members` | variant op を member に取る batch prepare は拒否 |
| `commit_rechecks_the_expected_revision_for_variants` | prepare 後に live が進んだ variant commit は fence 内 `RevisionConflict` |
