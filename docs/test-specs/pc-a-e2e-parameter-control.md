---
id: "TSPEC-034PCA11Wr6fZa2gIjWkLm"
type: "test-spec"
status: "approved"
title: "PC-A 検証仕様 — Parameter Control 合意8領域 headless 受入"
created: "2026-11-03"
updated: "2026-11-03"
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
  derives-from:
    - "docs/designs/pc-a-parameter-control-design.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-A 検証仕様 — Parameter Control 合意8領域 headless 受入

## Purpose

Issue #6 合意の Parameter Control 8 領域が `synvelis-workspace` +
`synvelis-project` 上で不変条件として守られることを固定する。検証
本体: `crates/synvelis-workspace/tests/pc_a.rs`。Fixture:
`CounterIdGenerator` + `FakeWindowHost` + fixture `ParamInfo`
capability で完全決定論。

## Feature: PC-A-01 単一正本 + 3 投影

- `Project::control` に target/lane/source/assignment が保持され、
  `lane_view`・`signal_flow_view`・`inspector_breakdown` が同一の
  region/point/assignment 内容を返す (複製系列なし)。
- `WriteAutomationRegion` で追加した point が 3 投影全てに即座に
  反映される。controller 外での書込み経路は存在しない。

## Feature: PC-A-02 Scope 分離

- `Scope::Track` の track volume automation と `Scope::Clip` の clip
  envelope が独立に評価される (clip envelope は track target に
  着弾しない)。
- Clip の `MoveClip` で clip-scope envelope の評価位置が追従する
  (point は書換えられない)。
- clip-local point は clip 相対 beat で保持され、clip anchor 経由で
  project 時間へ解決される。clip 範囲外では `LaneOutside`。

## Feature: PC-A-03 競合検出

- 同一 target・同一評価時刻に重複する region を `Reject` policy で
  書くと `Error::AutomationConflict` (競合 lane・range を報告)、
  project 不変。
- `Replace` policy は明示上書きとして成功する。
- 重複のない複数 region は同一論理系列として評価される
  (先の region の値が次の region の手前まで保持される chase)。
- `validate_references` が lane 内重複 region を `Validation` で
  拒否する。

## Feature: PC-A-04 Value Breakdown + 観測

- `ControlEvaluation` が base/automation/reference + source/
  reference_source/各 contribution (assignment・source・raw・
  transformed・depth・value)/raw_total/clamp 状態/host normalized・
  plain を返す。
- clamp 発生時 `clamped` (または saturated) が立つ。
- `feed_plugin_report`/`feed_probe` で `MediaTimestamp`+provenance
  を保持し、playback 文脈で `AtPlayhead`/`Live`、feed 無しで
  `Unavailable`、capture で `Captured`、観測が評価 revision/位置と
  ずれると `Stale` を区別する。

## Feature: PC-A-05 Mode 状態機械 + 記録

- `set_mode` だけでは revision が進まない (project 不変)。
- Off: gesture_end が `SetParameterBase` で base 更新 (undo 対象)、
  automation 停止。
- Read: gesture_end で override 破棄・lane 書込みなし (復帰)。
- Touch: arm+record+playing 成立時のみ gesture 区間が recording
  segment に着く。条件不足では書込みなし。
- Latch: release 後も `transport_advance` で最終値を継続書込み、
  transport stop/mode 変更で終了し次回再生に持ち越さない。
- Write: 成立対象のみ継続書込み、未 arm/未 record は拒否。
- 記録 segment は manual 値 (pre-modulation) のみ — LFO 動作中でも
  host_resolved が lane に書き戻されない。
- 1 record session の commit が 1 apply = 1 undo (1 undo で全区間
  復旧)。
- `transport_advance` が loop_range を越えると transaction が
  `LoopWrap` seal され、以後の record_write が拒否される (silent
  上書きなし)。
- 録音中に inspector follow (他 view の selection/inspection) が
  変わっても `RecordingTransaction::fixed_targets` は不変。

## Feature: PC-A-06 Control Graph

- depth_source による相互参照 cycle (A→B, B→A) で
  `ControlGraph::compile` が `Error::ControlCycle` を返す。
- compile は project を変えず、失敗時に既存 plan を壊さない。
- 複数 enabled assignment の加算が順序独立 (assignment 追加順と
  eval 結果が無関係、id 昇順固定)。

## Feature: PC-A-07 Capability

- READONLY target への `SetParameterBase`/`WriteAutomationRegion`/
  `lower` が拒否される。
- 非 AUTOMATABLE で TimedEvent が `NotAutomatable`、BlockBoundary
  への明示 degrade が `Degraded` として返る。`Immediate`/`PerNote`
  は `UnsupportedMeans`。
- Boolean/Choice/Stepped は最終値が量子化規則で quantize される;
  `modulation_allowed` が非 Continuous を拒否。
- `ParamMapping::Exponential` で同一 normalized depth が位置依存の
  plain 増分を持つ (depth≠plain 増分)。

## Feature: PC-A-08 時間整合

- `AddTempoEvent` で tempo map revision が進んでも Beat 保持 point
  の評価が不変。
- `MusicalTick`↔`BeatPosition` 変換が `Ppqn`/`TickPosition` 経由で
  正確 (1/7 beat が grid に quantize されず保持)。
- clip 移動で clip-scope 評価が追従。loop wrap で `position` が
  loop.start に戻り評価が一貫。
- 同一 grid で playback 逐次評価と offline render 評価が一致。
- `map_stamp_to_frame` で native stamp が一度だけ frame へ写像
  され、UnmappedDomain/BaseMismatch/BeforeAnchor/EpochMismatch が
  defined rejection として返る。

## Guarantees

- Mode/Arm/Gesture/Recording/Observation は常に session-local。
- Recording commit は必ず 1 回の atomic apply。
- 失敗時は project 正本が不変で、理由が呼び出し側へ返る。
- 競合・capability・cycle の拒否は全て defined rejection (silent
  fallback/優先順位解決なし)。

## Non-goals

- 実 GUI・実 plugin adapter・Bake・Take multi-pass・Feedback
  graph・MIDI Control Mapping・Note entity・sample-accurate 配送・
  smoothing。

## Risk

- `SCHEMA_VERSION` 2 で schema 1 snapshot と非互換 — PoC 間の
  意図的変更として impl record に記録。
- `ruviform-plugin-core` 新規 dep — 同一 rev `94c5bf8` 維持を
  `cargo tree` で確認。
