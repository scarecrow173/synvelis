---
id: "EXP-sv517lM4nO5pQ6rS7tU8vW9"
type: "experiment"
status: "completed"
title: "issue-517-render-branch-wiring — 実験記録"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/impl/ir/issue-517-render-branch-wiring.md"
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
    - "docs/impl/ir/issue-517-render-branch-wiring.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/exp/issue-517-render-branch-wiring-exp.jsonl"
  defers: []
  deferred-by: []
---

# issue-517-render-branch-wiring — experiment log

詳細イベントは `issue-517-render-branch-wiring-exp.jsonl` を参照。

## E1: DirectoryBranchContentStore の publish 競合

`PublishMode::CreateNew` で exists-check と publish の間に別
writer が同 digest を publish した場合、`CreateNew` は失敗する。
readback で digest 一致を確認して受理する経路を実装 — content
addressing により「同名・別 bytes」は honest writer では起きない。

## E2: JournaledRuntimePublish の状態選択

`Published` を返す設計を検討したが、`synvelis-project` 内に
activation を確認する runtime が存在しないため契約違反。
RCP-BR-04（project commit ≠ runtime activation、last-good 保持）
に従い typed `Pending` のみを返す設計に確定。engine/session 側の
rebuild は sv#514 の SessionRoutingEngine 領域。

## E3: render op の認可 floor 実測

当初 test で per-op `required_permissions`（RunJob/ExternalAction）
の grant 強制を期待したが、実測では `synvelis.operation.prepare`/
`apply` wire capability の permission のみが強制され、inner op の
`required_permissions`/`data_access` は descriptor metadata
（emitted classes は resource effects 由来 — Target::None の
render op では scoped effect なし）。これは branch ops を含む全
op に共通の既存契約であり、render も同一機構 = parity 成立。
test は capability floor（実効）+ descriptor metadata 宣言の
parity を pin する形に修正。

## E4: idempotent apply replay の実測

同一 idempotency key + 同一 payload の apply 再送は
`GatewayError::DuplicateRequest`（determined replay、二重 effect
なし）。`IdempotencyConflict` は同一 key + 異 payload の場合。
branch ops と同一機構であることを test で pin。

## E5: restore fork-copy の receipt 設計

`copied` を staging 時点でなく publish 後の dest 実在で再計算 —
receipt は published reality を反映する（部分失敗時の虚偽報告を
防止）。unavailable は restore 失敗ではなく per-reference verdict。
