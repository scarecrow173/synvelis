---
id: "TSPEC-sv517iJ1kL2mN3oP4qR5sT6"
type: "test-spec"
status: "approved"
title: "Issue #517 test spec — render/branch pipeline 本番配線の検証保証"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/issue-517-render-branch-wiring.md"
    - "docs/designs/issue-517-render-branch-wiring.md"
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
  derives-from:
    - "docs/specs/issue-517-render-branch-wiring.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-517-render-branch-wiring.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #517 test spec

## Feature: 永続 BranchContentStore

### Rule R1-1: restart persistence
Example: `put` した payload を新規 `DirectoryBranchContentStore`
instance（process restart 相当）が digest のみで `get` できる。
File: `crates/synvelis-project/tests/sv517_render_wiring.rs`
`directory_store_persists_payloads_across_instances`.

### Rule R1-2: digest verification + fail-closed corruption
Example: 宣言 digest と異なる bytes の `put` は拒否。publish 済み
file を改竄すると冪等 re-put が `VariantError::Store` で fail
closed（digest-named file を信用しない）。
File: 同上 `directory_store_fails_closed_on_corrupted_payload`.

## Feature: JournaledRuntimePublish

### Rule R2-1: typed Pending, never Published
Example: publish は常に `RuntimePublishState::Pending` を返し、
journal record `<dir>/<request-digest>.json` が durable に残る。
重複 request は同一 record（冪等）。
File: 同上 `journaled_publish_reports_typed_pending_and_survives_restart`.

### Rule R2-2: typed Failed on journal failure
Example: journal dir が作成不能な場合 `RuntimePublishState::Failed`
を返す — panic も捏造の Pending/Published もしない。
File: 同上 `journaled_publish_reports_failed_when_journal_unwritable`.

## Feature: synvelis.render.* gateway 登録

### Rule R3-1: capability 登録 parity
Example: 9 つの `synvelis.render.*` id が `ProjectMutation`
domain で registry 登録され、read-only action は Read effect・
job command は Write effect・publish/remote は
ExternalAction permission を宣言する。
File: `crates/synvelis-gateway/tests/sv517_render_actions.rs`
`all_nine_render_capabilities_are_registered`.

### Rule R3-2: dispatch parity + non-mutation
Example: `synvelis.render.request` の prepare→apply→commit が
`RenderDispatch` port へ intent を配送し、project revision は
不変（`receipt.revision == base_revision`）、provenance に typed
dispatch state が記録される。
File: 同上 `render_request_commits_and_dispatches_without_mutating_project`.

### Rule R3-3: typed failure, never fabricated success
Example: `Pending` は commit されるが provenance は `pending`。
`Unavailable`（backend 不在 port）と port 未注入は determined
non-commit rejection。
File: 同上 `render_pending_and_unavailable_are_typed_never_success`,
`render_without_dispatch_port_fails_closed`.

### Rule R3-4: grant floor + idempotency parity
Example: capability を grant が名指さない prepare は
"capability outside grant" で拒否。`synvelis.render.remote` は
capability 名指しで prepare 可能（Target::None ゆえ scoped
effect なし — branch ops と同一機構）。同一 idempotency key の
apply 再送は `DuplicateRequest` で determined replay。
File: 同上 `render_respects_the_capability_floor`,
`render_apply_replays_idempotently_like_branch_ops`.

## Feature: VCS-RESTORE-10 fork-copy

### Rule R4-1: payload fork-copy
Example: `Captured` digest を参照する snapshot の
`restore_as_copy` が source store から payload を読み、digest
再検証のうえ dest `branch-content/<hex>` へ publish。
`receipt.branch_content.copied == referenced`、復元 project の
`DirectoryBranchContentStore` + `decode_candidate` が payload を
解決できる。
File: `crates/synvelis-project/tests/sv517_render_wiring.rs`
`restore_as_copy_fork_copies_captured_branch_payloads`.

### Rule R4-2: honest unavailable verdict
Example: `branch_content` port 不在の restore は成功するが
`receipt.branch_content.unavailable` に全 referenced digest が
列挙され、`copied == 0` — `CandidateUnavailable` の正直 verdict。
dest に `branch-content/` は作られない。
File: 同上 `restore_as_copy_reports_unavailable_candidates_without_source`.

## NotRun（別 gate）

- 実 CLI/MCP/GUI surface E2E・実 render pipeline 実走・実 RT 計測
  — sv#267/sv#288/sv#296/sv#114 の責務。
- cb10_variant_actions・vcs 系・rcp 系の既存 suite は回帰確認のみ
  （`mise run test` 各 crate で実施）。
