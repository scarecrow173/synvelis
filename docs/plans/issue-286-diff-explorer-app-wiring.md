---
id: "PLAN-286DiffExplorerAppWiring04"
type: "plan"
status: "approved"
title: "Issue #286 plan — CB-09 Difference Explorer app wiring"
created: "2026-11-18"
updated: "2026-11-18"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "docs/specs/issue-286-diff-explorer-app-wiring.md"
    - "docs/designs/issue-286-diff-explorer-app-wiring.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/cb09_variant_diff.rs"
      - type: "file"
        path: "docs/discovery/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/specs/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/designs/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/plans/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/adr/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/tasks/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/test-specs/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/impl/ir/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/impl/exp/issue-286-diff-explorer-app-wiring.md"
      - type: "file"
        path: "docs/impl/exp/issue-286-diff-explorer-app-wiring-exp.jsonl"
      - type: "dir"
        path: "evidence/sv286/"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/variant_diff.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/cb09_diff_explorer.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "docs/impl/ir/cb-09-diff-explorer.md"
      - type: "file"
        path: "docs/test-specs/cb-09-diff-explorer.md"
      - type: "file"
        path: "crates/synvelis-render/src/render_job/orchestrator.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/render_dispatch.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-286-diff-explorer-app-wiring.md"
    - "docs/designs/issue-286-diff-explorer-app-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-286-diff-explorer-app-wiring.md"
---

# Issue #286 plan — CB-09 Difference Explorer app wiring

## スコープ分割

単一 slice。headless view-model（CB-09 前段）に対し、
workspace facade → app ops → view mount → project 切替
までを一貫して配線する。

## ファイルマップ

### workspace（`synvelis-workspace`）

- `src/variant_diff.rs` — `VariantDiffEngine` facade、
  `VariantDiffIntent`、`AuxLayerEvidence` 写像、
  `apply_variant_diff_view_local`、stale
  `DomainAdapterUnmerged` verdict 修正。
- `tests/cb09_diff_explorer.rs` — engine/facade テスト
  （compare・provider provenance・CAS adopt・reject・
  authority/context）。

### app（`synvelis-app`）

- `src/ui_ops.rs` — `syn.variant-diff.*` op 実装、
  `VariantDiffUiState`、error mapping。
- `src/service.rs` — op registry/dispatch 登録。
- `src/screens.rs` — Difference Explorer mount（ViewSpec）。
- `src/app.rs` — `bundle_path` accessor、project 切替
  cleanup 配線。
- `src/lib.rs` — ViewSpec mount helper re-export。
- `tests/cb09_variant_diff.rs` — op/view lifecycle/mount
  smoke テスト。

### docs / evidence

- `docs/{discovery,specs,designs,plans,adr,tasks,
  test-specs,impl/ir,impl/exp}/issue-286-diff-explorer-app-wiring.*`
- `evidence/sv286/app-wiring-verification.md`

### upstream merge debt（本 slice 起因ではない）

- `crates/synvelis-render/src/render_job/orchestrator.rs` —
  `clippy::too_many_lines` expect（upstream #518 系）。
- `crates/synvelis-workspace/src/render_dispatch.rs` —
  `clippy::result_large_err` allow ×2（同上）。
  いずれも `Refs #518` の分離コミット。

## 依存グラフ

```
discovery → spec → design → plan → task → (workspace facade → app ops → mount)
                                              ↓
                                    test-spec → IR → EXP
```

## タスク分解（実施順）

1. workspace facade: `VariantDiffEngine` + intent dispatch +
   provider 写像 + view-local helper。
2. workspace tests: facade contract（CAS/pin/provenance/
   reject）。
3. app ops: `syn.variant-diff.*` + state + error mapping。
4. app service/screens/app 配線 + ViewSpec mount。
5. app tests + gpui-test mount smoke。
6. project 切替 cleanup + 回帰テスト。
7. docs chain + evidence + verification lanes。

## リスクと対策

| リスク | 対策 |
| --- | --- |
| app が gateway 型を import してしまう | facade に閉じ込め、app import を module path の workspace 型のみに限定 |
| preview intent が commit へ流れる | intent 種別で dispatch を分岐し commit path で malformed reject |
| unavailable evidence の zero-diff 化 | `EvidenceState` をそのまま保持し変換しない |
| project 切替で session 衝突 | 切替時に engine/session/target/observation を全クリア + 回帰テスト |
| upstream merge で adapter 登録が二重化 | `VariantPorts::standard` を authoritative とし facade 側 install を除去 |

## ロールバック

- facade/ops は追加のみで既存経路を変更しないため、
  revert は当該コミットの巻き戻しで完結する。
- upstream merge debt の clippy 抑制は `Refs #518` 分離
  コミットのため個別 revert 可能。

## 検証マトリクス

| lane | 目的 |
| --- | --- |
| `mise run check-app-native` | app native feature compile |
| `mise run lint-app-native` | app clippy `-D warnings` |
| `mise run test-app-gpui` | gpui-test lane（実 window/pixel は範囲外） |
| `cargo test -p synvelis-workspace --test cb09_diff_explorer` | workspace facade contract |
| `cargo test -p synvelis-app --features gpui-test` | app op/mount smoke |
| `mise run fmt-check` | format（worktree 環境起因の os error 206 は evidence に記録） |

## 関連

- task: `docs/tasks/issue-286-diff-explorer-app-wiring.md`
- test-spec: `docs/test-specs/issue-286-diff-explorer-app-wiring.md`
- IR: `docs/impl/ir/issue-286-diff-explorer-app-wiring.md`
- 証跡: `evidence/sv286/app-wiring-verification.md`
