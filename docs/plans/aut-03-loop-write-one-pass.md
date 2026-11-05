---
id: "PLAN-034AUT03Cm4rT8vXy1qKp9sL"
type: "plan"
status: "approved"
title: "AUT-03 Loop 録音 1 Pass・Write Range 閉鎖・明示 Rearm 実装計画"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/133"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_03.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/controls_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
  implemented-by:
    - "docs/tasks/aut-03-task1-loop-write-one-pass.md"
  depends-on:
    - "docs/plans/pc-c1-recording-transaction.md"
    - "docs/plans/aut-02-region-replace-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-03-e2e-loop-write-one-pass.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
  defers: []
  deferred-by: []
---

# AUT-03 Loop 録音 1 Pass 実装計画

## スコープ分割

単一 task で実装する。変更面は `synvelis-workspace` の session
層(`control.rs`)中心で、呼出側(`controls_ui.rs`)の signature
追従と 1 テストファイルに閉じる。

| 区分 | 内容 |
| --- | --- |
| In | `LoopPassId`・pass 計数、`fixed_scopes`/`coverage_end` fixation、`close_write_range` 共通経路、境界 flush、追加 `SealReason`、`note_graph_stale`、prepare の coverage 反映、`record_write`/`record_points` の pass 境界ガード、signature 更新、`tests/aut_03.rs` |
| Out | Multi-pass Take(AUT-F01 #6)、実 Plugin binding、GUI 配線、PC-C6 oracle、PPQN 変更、RT/eval/dispatch 変更、永続化 schema 変更 |

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-workspace/src/control.rs` | `LoopPassId`、`PlaybackState.pass`、`RecordingTransaction.{pass,fixed_scopes,coverage_end}`、`SealReason` 4 variant 追加、`SealOutcome.write_range_closed`、`close_write_range`、wrap/stop/seek/arm/mode/loop/epoch/graph/project-change/play-edge の seal 化、`note_graph_stale` 新設、`record_write`/`record_points` pass 境界ガード、`prepare_record_commit` coverage 反映、fingerprint 拡張 |
| `crates/synvelis-workspace/src/controls_ui.rs` | `&Project` 引数の引き回し(stop/seek/set_loop/install_clock_anchor/note_graph_stale 呼出) |
| `crates/synvelis-workspace/src/lib.rs` | `LoopPassId`・新 `SealReason` variant・`note_graph_stale` の export 整理(必要なら) |
| `crates/synvelis-workspace/tests/aut_03.rs` | Acceptance の headless pin(test-spec F1–F7) |
| 既存 `tests/aut_01.rs`/`pc_c1.rs` 等 | signature 追従のみ(動作変更なし) |
| `docs/impl/ir/` | 実装記録(実装後に追加) |

## 依存グラフ

- 新規外部依存なし。`synvelis-workspace` 内完結 +
  `synvelis-project` の既存公開 API(`resolve_scope_pos`・
  `AutomationTimeScope`・`Project::control`)のみ使用。
- `control.rs` の変更を先に行い、`controls_ui` / 既存テストの
  呼出側追従で型エラーを解消、`tests/aut_03.rs` で acceptance
  を pin する順。

## リスク登録

| リスク | 対応 |
| --- | --- |
| wrap 閉鎖で既存 AUT-01 テストの gesture closure 期待を破壊 | `close_write_range` は `sealed.is_none()` のときのみ実行、runtime closure(gesture_end・WriterState)は従来どおり — AUT-01 テスト全量で無退行確認 |
| `&Project` 引数追加で既存 caller が大量破壊 | seal 経路を持つ method に限定し、`Session`/`controls_ui` の薄い wrapper で吸収。test helper で `&project` を渡す形に統一 |
| coverage_end の scope resolve 失敗で Replace range が狭まる | `> last_pos` のときのみ採用・失敗は last+ε fallback — 現行動作より狭くならないことを保証 |
| flush が失敗して閉鎖自体が止まる | flush は best-effort(append 失敗 lane は残りを捨てて継続)、seal/coverage 記録は flush 結果に依存しない |
| fingerprint 拡張で exactly-once apply の一致判定が変わる | 新フィールドは record_start 時に固定される不変値のため同一 tx の再 apply で変わらない。pc_c1 既存テストで確認 |
| pass 計数が seek/resume で意図せず進む | pass は play edge と wrap のみで変化。seek/stop/pause/resume では不変を test で pin |

## ロールバック戦略

session 層のみの変更で永続化 schema に触れない — revert で
完全に旧動作へ戻る。`SealReason` 新 variant・新フィールドは
後方互換性を要求される永続データに出ない(session 内型)。

## 検証マトリクス

| 検証 | 入口 |
| --- | --- |
| fmt | `mise run fmt` → `fmt-check` |
| 型・lint | `mise run check` / `mise run lint` |
| 単体・統合 | `mise run test`(`tests/aut_03.rs` + aut_01/pc_c1/pc_a 等無退行) |
| RT | `mise run test-rt`(変更面は RT 非接触だが既存 lane で確認) |
| MSRV | `mise run test-msrv` |
| 最終前 | `git fetch origin` → `git merge origin/main` → lockfile 再生成 → 上記再実行 |

## タスク分解

- `TASK-034AUT03`: 本計画の実装 + `tests/aut_03.rs`(test-spec
  F1–F7 に対応)。
