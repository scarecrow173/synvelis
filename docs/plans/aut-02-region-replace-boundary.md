---
id: "PLAN-034AUT02Vn5rWt9kJx4sLq8b"
type: "plan"
status: "approved"
title: "AUT-02 Region Replace 境界曲線保存 実装計画"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/132"
  changes:
    added:
      - "crates/synvelis-project/tests/aut_02.rs"
    modified:
      - "crates/synvelis-project/src/control.rs"
      - "crates/synvelis-project/src/ops.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
    - "docs/designs/aut-02-region-replace-boundary-design.md"
  implemented-by:
    - "docs/tasks/aut-02-task1-region-replace-boundary.md"
  depends-on:
    - "docs/plans/pc-c1-recording-transaction.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/designs/aut-02-region-replace-boundary-design.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/aut-02-region-replace-boundary-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-02-e2e-region-replace-boundary.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/131"
  defers: []
  deferred-by: []
---

# AUT-02 Region Replace 境界曲線保存 実装計画

## スコープ分割

単一 task で実装する(変更面が project crate 内の 1 系統 +
1 テストファイルに閉じるため)。

| 区分 | 内容 |
| --- | --- |
| In | `boundary_anchors`/`stitch_boundary_anchor` 追加、`WriteAutomationRegion{Replace}`・`EraseAutomationRange` 適用、`merged_series` の `&ControlModel` 起点化、監査と不変条件テスト |
| Out | PPQN 変更コマンド、Loop write、GUI wiring、schema 変更 |

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-project/src/control.rs` | `BoundaryAnchor` 内部 struct・`boundary_anchors`・`stitch_boundary_anchor` 追加、`merged_series` を `&ControlModel` 起点に揃え `pub(crate)` 化 |
| `crates/synvelis-project/src/ops.rs` | Replace/Erase apply で anchor 計画→cut→左 anchor 畳み込み→右 stitch |
| `crates/synvelis-project/tests/aut_02.rs` | Acceptance の property/exact fixture/atomicity テスト |
| `docs/impl/` | 実装記録(実装後に追加) |

## 依存グラフ

- 新規外部依存なし。`synvelis-project` 内完結。
- `merged_series` は `evaluate_control`/`detect_conflicts` 等の既存
  caller と共有 — signature 変更は `&Project` → `&ControlModel`
  の引き回しのみで caller は `project.control` を渡す。

## リスク登録

| リスク | 対応 |
| --- | --- |
| host region 縮小で `range` と points の包含不変条件を破る | 縮小先を host 先頭 point pos に限定し、post-apply `validate_references` oracle で強制検査 |
| sentinel region が別 region と重複 | `end = min(s_next, c1+ε)` で上限を固定、`detect_conflicts` oracle が二重検査 |
| f64 の `[0,1]` 境界割れ | anchor 値を挿入時に clamp |
| 既存テスト退行(pc-a Replace 検査等) | 変更面を Replace/Erase apply に限定、`mise run test` 全量実行 |
| Erase の右 anchor で消去範囲に residue | anchor は `pos = c1`(範囲外)のみ — 消去契約は維持 |

## ロールバック戦略

op apply 内の変更のみ — revert で完全に旧動作へ戻る。永続化
schema 変更が無いため、保存済み sentinel point は通常 point と
してそのまま読める(過去 save との後方互換に影響しない)。

## 検証マトリクス

| 検証 | 入口 |
| --- | --- |
| fmt | `mise run fmt` → `fmt-check` |
| 型・lint | `mise run check` / `mise run lint` |
| 単体・統合 | `mise run test`(`tests/aut_02.rs` 含む) |
| RT | `mise run test-rt`(post-replace eval_rt 一致を含む既存 pc_b_rt 系) |
| MSRV | `mise run test-msrv` |

## タスク分解

- `TASK-034AUT02`: 本計画の実装 + テスト(test-spec の Feature
  群に対応)。
