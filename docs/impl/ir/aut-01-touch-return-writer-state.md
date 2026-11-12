---
id: "IMPL-AUT01TouchReturnWriterState"
type: "impl"
status: "completed"
title: "AUT-01 Touch Return 150ms Smoothstep・Read/Touch/Latch/Write 境界 State 実装記録"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/131"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_01.rs"
      - type: "file"
        path: "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
      - type: "file"
        path: "docs/designs/aut-01-touch-return-writer-state-design.md"
      - type: "file"
        path: "docs/plans/aut-01-touch-return-writer-state.md"
      - type: "file"
        path: "docs/tasks/aut-01-task1-touch-return-writer-state.md"
      - type: "file"
        path: "docs/test-specs/aut-01-e2e-touch-return-writer-state.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/controls_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/session.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_a.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_b.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_c1.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/aut-01-task1-touch-return-writer-state.md"
  implemented-by: []
  depends-on:
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/designs/aut-01-touch-return-writer-state-design.md"
    - "docs/plans/aut-01-touch-return-writer-state.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/ctl-01-control-owner-time-scope.md"
    - "docs/impl/ir/pc-c1-recording-transaction-prepare-apply-finalize.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-01-e2e-touch-return-writer-state.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUT-01 実装記録: Touch Return 150ms Smoothstep・境界 State

## Summary

Issue #131 の中核を、Plugin 実機・GUI 完成前の**純粋状態遷移/時計制御
proof of concept** として実装した。Automation Mode(Off/Read/Touch/
Latch/Write)のリリース後境界と、録音 transaction(PC-C1/#98)・
CTL-01(#121)の ControlOwner/AutomationTimeScope 分離との整合を
headless で確定させた。Touch Return 150ms Smoothstep・
Read/Touch/Latch/Write 境界 State・セッション時計・GestureFixation・
録音 transaction 整合を `synvelis-project`/`synvelis-workspace` に
実装し、`tests/aut_01.rs` 15 件を含む全 gate を green にした。
Plugin binding・GUI 接続は Integration 対象であり本スコープ外。

## Context

- Issue: https://github.com/scarecrow173/synvelis/issues/131
- Spec: `docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md`
- Design: `docs/designs/aut-01-touch-return-writer-state-design.md`
- Plan: `docs/plans/aut-01-touch-return-writer-state.md`
- Task: `docs/tasks/aut-01-task1-touch-return-writer-state.md`
- Test spec: `docs/test-specs/aut-01-e2e-touch-return-writer-state.md`
- 前提: PC-C1 録音 transaction(#98)・CTL-01 ControlOwner/
  AutomationTimeScope 分離(#121)。

## Implementation

### Touch Return — runtime interpolation, 非破壊
(`synvelis-project/src/control.rs`, `synvelis-workspace/src/control.rs`)

- `ReferenceSource::Return` を追加。session が注入する
  pre-modulation reference 領域の runtime blend マーカーであり、
  `evaluate_control` 自身は設定しない(永続カーブの評価結果と
  runtime blend を型で区別)。
- `ControlSession` が `touch_returns: BTreeMap<ParameterTargetId,
  TouchReturn>` を保持。Touch gesture の `gesture_end` で manual
  override を外し、`v = v_release + (v_automation(t) − v_release)·w`、
  `w = 3x² − 2x³`、`x = clamp(elapsed/return_duration, 0, 1)` の
  Smoothstep 補間を `evaluate` で合成する。
- `v_automation(t)` は評価位置 `t` ごとに現在の automation series を
  再評価するため、Return 区間中のカーブ編集・Step/Linear の両
  interpolation に追従する。
- Return は **runtime のみ**: Automation Point 追加・削除・書換・
  ProjectRevision 変化は行わない(`aut_01` の非破壊テストで検証)。
- `DEFAULT_TOUCH_RETURN_MS = 150` を暫定既定とし、
  `set_return_duration` で 0 以上の任意 duration を受理。0ms は合法だが
  `WriterState::Returning { instant: true }` として click/zipper 境界を
  可視化する。

### セッション時計(beat playhead と独立)

- `clock_advance(now: MediaTime)`: `frame_base` への exact convert
  (nonrepresentable base は `Error::Validation`)、単調性強制
  (reverse move は拒否)を行う単調時計。`clock_now` を保持し、
  stop/pause 中も Return は wall-clock 相当の時計で完走する。
- `clock_advance` 時に完了済み `touch_returns` を sweep し、
  `WriterState` は `Returning → Idle` へ落ちる。

### Read/Touch/Latch/Write 境界 State + 明示閉鎖ログ

- `WriterState { Idle, Gesture, LatchHeld{value}, WriteHeld{value},
  Returning{release_value, progress, instant} }` を mode 行列の
  inspectable 側面として追加。`writer_state(target)` で参照可能。
- `WriterCloseReason` (`TransportStop`/`TransportSeek`/`LoopWrap`/
  `NewTransportPlay`/`FocusLost`/`PluginCrash`/`TargetDeleted`/
  `ModeChanged`/`Cancelled`)と `WriterClosure` の bounded ログ
  (`WRITER_CLOSE_CAP = 256` の `VecDeque`)を追加。
- `transport_stop`/`transport_seek`/loop wrap/非Playing→Playing の新規
  play edge で全 runtime state(gesture override・latch/write held・
  in-flight return)を `close_runtime_state` で明示閉鎖し closure を
  記録 — Latch/Write は次 playback で**暗黙に再 armed しない**。
- `transport_pause`/`transport_resume` は同一 playback の継続なので
  閉鎖しない(新規 play edge のみ `NewTransportPlay` 閉鎖)。
- `note_focus_lost` / `note_plugin_crash(project, processor)` を追加。
  `note_project_changed(project)` は既存の foreign-change seal に
  加え、プロジェクトから削除された runtime target を
  `TargetDeleted` で閉鎖する。
- `set_mode` 時に `LatchHeld`/`WriteHeld`/`Gesture` を `ModeChanged`
  として閉鎖する。in-flight の Return は mode-independent(リリースの
  完走のみ)なので mode 変更後も走り続ける。

### GestureFixation

- `ControlGestureFixation { target, revision, arm, epoch, frame_base }`
  を `gesture_begin(project, target, at, revision)` で捕捉し
  `ControlGesture` に保持。Inspector follow・focus・window 変更が
  in-flight gesture を retarget しないことを `gesture_fixation` で
  検証可能にした。

### 録音 transaction との整合(PC-C1 維持)

- `record_start(project, revision)`: transaction 開始時に fixed
  target 集合・lane mapping・revision を捕捉。`Write` mode の fixed
  target は `evaluate_control` で start position の Reference を
  seed し `manual_overrides` に保持する(control-rate で連続書込み)。
  domain 外 start position(clip scope 範囲外等)の evaluate 失敗は
  unheld のまま許容し、transaction 生成自体は失敗させない
  (pc_b_02 regression で確認・修正)。
- `note_project_changed` の自己 seal 回避ルールは維持 — transaction
  自身の commit は foreign change ではない。
- caller 整合: `gesture_begin` / `record_start` /
  `note_project_changed` の新シグネチャに合わせ `session.rs`・
  `controls_ui.rs`・既存テスト(pc_a/pc_b/pc_c1)を更新。

## Related Experiments

### Adopted

なし — `metadata.experiments.adopted` は空。Shadow CI の観測記録
`docs/impl/exp/issue-131-aut-01-shadow-ci.md`(#157 蓄積)は採用/
却下の実験ではなく、selective 判定の偽陰性対照として Validation
節に記録した証跡。

### Rejected

なし — `metadata.experiments.rejected` は空。

## Validation

実行環境: Windows、toolchain pin `rust 1.99.0`、MSRV `rust 1.89.0`。
すべて `mise run <task>` 経由。HEAD = `origin/main` `b47c90f` 上。

| Gate | 結果 |
| --- | --- |
| `mise run fmt` / `mise run fmt-check` | PASS (exit 0) |
| `mise run check` | PASS (exit 0) |
| `mise run lint` | PASS (exit 0、`cargo clippy --workspace --all-targets -- -D warnings`) |
| `mise run test` | PASS (exit 0、`cargo test --workspace --locked`、aut_01 15 テスト含む全 binary ok) |
| `mise run test-rt` | PASS (exit 0、rt_ contract filter) |
| `mise run test-msrv` | PASS (exit 0、`cargo +1.89.0 test --workspace --locked`、aut_01 15 テストも 1.89 で pass) |

AUT-01 acceptance (`crates/synvelis-workspace/tests/aut_01.rs`、15 件):
Read override のみ期間・Touch gesture write + Smoothstep 中点
(w(0.5)=0.5、release 1.0→auto 0.0 で reference 0.5)・150ms 完走・
0ms instant flag・300ms 可変 duration・Step/Linear curve follow・
re-touch blended seed・Latch held + ModeChanged closure・Write seed +
stop・Stop/Seek/LoopWrap/FocusLost/NewPlay/Cancelled/PluginCrash/
TargetDeleted 全境界の Idle + closure ログ・fixation 内容・非破壊性
(revision/regions 不変)。

Shadow CI 証跡(selective 判定の偽陰性対照)は
`docs/impl/exp/issue-131-aut-01-shadow-ci.md` に記録(#157 蓄積)。

## Risks

- Plugin binding・実 VST3 callback 適用(#101 Integration)と GUI
  接続(#102/#114)は未実装 — 本記録は headless proof of concept
  のみで、実 host/GUI 経路での動作は未検証。
- `DEFAULT_TOUCH_RETURN_MS = 150` は暫定既定値 — 製品仕様の確定は
  Integration/UX レビュー側の承認事項(後述 Follow-ups)。
- `WriterClosure` ログは `WRITER_CLOSE_CAP = 256` の bounded —
  全 closure の永続記録ではなく、上限を超えた分は保持されない。
- Return の進行は `clock_advance` による session 時計の駆動に
  依存する — 駆動されない host では `Returning` が完走しない
  (beat playhead から独立した設計どおりの契約)。

## Follow-ups

- Plugin binding・実 VST3 callback 適用(#101 Integration)、GUI 接続
  (#102/#114)は Integration スコープ。
- `DEFAULT_TOUCH_RETURN_MS = 150` は暫定値。製品仕様の確定は
  Integration/UX レビュー側の承認事項。
- AUT-02/03/04・AUT-F01–F05 は本スコープ外。
