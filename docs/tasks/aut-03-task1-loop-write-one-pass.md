---
id: "TASK-034AUT03Pq9rWx3nVm7sJk2d"
type: "task"
status: "done"
title: "AUT-03 task1: Write Range 閉鎖・境界 flush・明示 Rearm 実装 + headless テスト"
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
      - type: "file"
        path: "docs/impl/ir/aut-03-loop-write-one-pass.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/controls_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_01.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_a.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_b.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_c1.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/plans/aut-03-loop-write-one-pass.md"
  implemented-by: []
  depends-on:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-03-e2e-loop-write-one-pass.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
  defers: []
  deferred-by: []
---

# AUT-03 task1: Write Range 閉鎖・境界 flush・明示 Rearm

## 目的

design §2–§6 のとおり、`ControlSession` の recording gate を
1 Loop Pass の半開区間で閉じる契約へ拡張し、境界 flush・
coverage_end・追加 seal 契約・明示 Rearm を実装する。
test-spec F1–F7 を `tests/aut_03.rs` で pin する。

## チェックリスト

- [x] `LoopPassId` 型 + `PlaybackState.pass`(play edge = 1、
      wrap = +1、resume/seek/stop 不変)実装
- [x] `RecordingTransaction.{pass,fixed_scopes,coverage_end}` と
      fingerprint 拡張
- [x] `SealReason` 追加(`EpochChange`/`GraphStale`/`LoopChanged`/
      `NewTransportPlay`)+ write range 閉鎖の bool 報告
      (`close_write_range` 戻り値 / `TransportEffect.sealed`)
- [x] `close_write_range(project, bound)` 共通経路(first-seal-
      wins、loop clamp、write-gated gesture flush、active writer
      lane の coverage_end 記録)
- [x] 全 seal 経路の `close_write_range` 化(wrap/stop/seek/arm/
      mode/loop/epoch/project-change/play-edge)+ `&Project`
      signature 更新
- [x] `note_graph_stale` 新設(open tx を `GraphStale` で close)
- [x] `record_write` の `pos >= loop_end` Validation 拒否・
      gesture `record_points` の pass 境界 clip
- [x] `prepare_record_commit` の coverage_end → Replace range
      反映(`> last_pos` のみ採用、resolve 失敗は last+ε)
- [x] `controls_ui` / `Session` wrapper / 既存テストの呼出追従
- [x] `tests/aut_03.rs`: test-spec F1–F7(19 テスト)
- [x] `mise run fmt`/`check`/`lint`/`test`/`test-rt`/`test-msrv`
      全緑 + 既存 aut_01/pc_c1/pc_a 等無退行
- [x] `docs/impl/ir/` 実装記録

## 完了条件

- test-spec の Feature 群が `tests/aut_03.rs` で pin され全緑。
- 既存 aut_01/pc_a/pc_b/pc_c1/pc_b_rt テストが無退行。
- Wrap/Stop/Seek/Epoch/Graph/Loop/Arm/Mode/Play edge の全閉鎖
  経路で coverage_end・flush が 1 度だけ実行される。
