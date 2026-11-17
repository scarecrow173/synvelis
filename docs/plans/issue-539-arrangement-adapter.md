---
id: "PLAN-539aArrngmntAdptr05"
type: "plan"
status: "approved"
title: "Issue #539 plan — ArrangementAdapter 登録 + pin fixture positive flip slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/539"
    - "docs/specs/issue-539-arrangement-adapter.md"
    - "docs/designs/issue-539-arrangement-adapter.md"
    - "docs/adr/issue-539-arrangement-adapter.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/variant.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs"
      - type: "file"
        path: "docs/impl/ir/cb-11-cross-gate.md"
      - type: "file"
        path: "docs/impl/exp/cb-11-cross-gate.md"
      - type: "file"
        path: "docs/test-specs/cb-11-cross-gate.md"
      - type: "file"
        path: "evidence/sv288/coverage.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-539-arrangement-adapter.md"
    - "docs/designs/issue-539-arrangement-adapter.md"
  implemented-by:
    - "docs/tasks/issue-539-arrangement-adapter.md"
  depends-on:
    - "docs/designs/issue-539-arrangement-adapter.md"
    - "docs/adr/issue-539-arrangement-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "evidence/sv539/verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-539-arrangement-adapter.md"
  derived-by:
    - "docs/tasks/issue-539-arrangement-adapter.md"
    - "docs/impl/ir/issue-539-arrangement-adapter.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #539 plan — ArrangementAdapter 登録 + pin fixture positive flip slice

## Scope

単一 slice。`VariantPorts::standard` への adapter 登録と
pin fixture の positive flip、gap 記録の解消、検証証拠の記録。

## File map

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-gateway/src/variant.rs` | `ArrangementAdapter` import 追加・adapters map 登録・陳腐化 doc comment 4 箇所更新 |
| `crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs` | モジュール doc 更新・`move_op` helper 追加・旧 fail-closed test を positive test へ置換 |
| `docs/impl/ir/cb-11-cross-gate.md` | gap 解消注記 |
| `docs/impl/exp/cb-11-cross-gate.md` | E8 に解消追記 |
| `docs/test-specs/cb-11-cross-gate.md` | Example/Non-goals の gap 記述更新 |
| `evidence/sv288/coverage.md` | gateway 経路行 Pass 化・fixture 表・gap 節解消記録 |
| `evidence/sv539/` | 本 slice の検証証拠（新規） |
| `docs/{discovery,specs,designs,adr,plans,tasks,test-specs,impl/ir,impl/exp}/issue-539-arrangement-adapter.md` | doc chain（本 slice） |

## Dependency graph

registration → test flip → gap 記録解消 → 検証 → commit/PR。

## Risk register

| リスク | 対応 |
| --- | --- |
| `adapter_for` fail-closed 経路が到達不能化 | 機構は維持。負 fixture は新規 kind 追加まで構成不能 — 既存 fail-closed 境界（Unapproved/CAS/scope violation）の非回帰で代替 |
| 両 host への自動伝播で予期しない経路変化 | `standard()` 登録のみで wire 経路は既存汎用 — dispatch 追加以外の挙動変更なし |
| scope 外不変の assert 漏れ | project 軸 reference test（`cb11_e2e_03_*`）と同等の receipt/isolation assert を gateway test に移植 |

## Verification matrix

| 検証 | コマンド |
| --- | --- |
| format | `mise run fmt-check` |
| gateway check/lint/test | `mise run check|lint|test synvelis-gateway` |
| headless deps | `mise run check-headless-deps` |
| adapter 契約非回帰 | `mise run check|test synvelis-project` |
| GateDeps 伝播非回帰 | `mise run check|test synvelis-workspace` |
| hooks | `mise exec -- prek run --all-files` |
