---
id: "DESIGN-034AUT01Kp3mWv9zR2sQfL6h"
type: "design"
status: "approved"
title: "AUT-01 Touch Return・Writer State 境界設計 — session clock + 明示閉鎖"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/131"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
  implemented-by: []
  depends-on:
    - "docs/designs/pc-a-parameter-control-design.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-01-e2e-touch-return-writer-state.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/100"
  defers: []
  deferred-by: []
---

# AUT-01 設計 — Touch Return・Writer State 境界

## 前提と不変条件

- `ControlSession`(synvelis-workspace)は session state のみを所有し、
  `Project::control` が系列の唯一の正本。本件も一切の project
  mutation を持たない。
- PC-C1(#98)の Recording Transaction 契約(`fixed_targets`/
  `fixed_lanes`/`fixed_revision`・seal・非破壊 prepare・CAS apply・
  成功時のみ消費)は変更しない。AUT-01 はその上流の gesture/writer
  状態機械の境界を閉じる。
- CTL-01(#121)で分離された `ControlOwner`/`AutomationTimeScope` の
  契約に従う。Write 点列は `record_points` が owner/time_scope で
  scope 解決する既存経路をそのまま使う。
- `elapsed` の時計は `playback.position`(beat 座標)とは独立した
  session media clock。Return duration は実時間(ms)の概念であり、
  tempo map を通さない。

## 型

```rust
/// gesture begin(pointer down 相当)で固定する実行文脈。
pub struct ControlGestureFixation {
    pub target: ParameterTargetId,
    pub revision: ProjectRevision,
    pub arm: WriteArm,
    pub epoch: ClockEpochId,
    pub frame_base: TimeBase,
}

/// 進行中の Touch Return。runtime のみ — series/revision 非所有。
struct TouchReturn { release_value: f64, released_at: MediaTime }

/// 境界イベントで明示閉鎖された runtime state の記録。
pub enum WriterCloseReason {
    TransportStop, TransportSeek, LoopWrap, NewTransportPlay,
    FocusLost, PluginCrash, TargetDeleted,
    ModeChanged, Cancelled,
}
pub struct WriterClosure { pub target, pub reason: WriterCloseReason,
                           pub state: WriterState }

/// per-target runtime writer state(introspection)。
pub enum WriterState {
    Idle,
    Gesture,
    LatchHeld { value: f64 },
    WriteHeld { value: f64 },
    Returning { release_value: f64, progress: f64, instant: bool },
}
```

`ControlGesture` は `fixation: ControlGestureFixation` を追加保持する。
`InspectorBreakdown` に `writer_state: WriterState` を追加する。

`synvelis-project` 側の `ReferenceSource` に `Return` を 1 variant 追加
する(session が注入した touch-return 補間値であることを breakdown に
正直に表示するため。`evaluate_control` の `manual: Option<f64>`
シグネチャは変更しない — 補間値は既存の pre-modulation 注入経路を
通り、session 側が `reference_source` を `Return` に訂正する)。

## Session clock

- `clock_now: MediaTime`(frame_base 単位)を session が保持する。
  初期値 `MediaTime::new(0, frame_base)`。
- `clock_advance(now: MediaTime)` が唯一の前進経路。`now` は
  `convert_exact(frame_base)` できること(不能は `Validation`)、
  かつ **単調非減少**(`now < clock_now` は `Validation`)であること。
  `transport_advance`(beat 座標)とは独立 — host が control rate で
  駆動する deterministic 入力として、Stop 中の Return も完了できる。
- advance 時に完了した return を sweep する
  (`elapsed >= return_ticks` を除去)。

## Touch Return 評価

```rust
let x = if return_ticks == 0 { 1.0 }
        else { (elapsed_ticks as f64 / return_ticks as f64).clamp(0., 1.) };
let w = 3.0*x*x - 2.0*x*x*x;
let v = (1.0 - w) * release_value + w * v_auto;   // v_auto = automation ?? base
```

- `v_auto` は `evaluate_control(project, target, pos, None).reference`
  — automation 未到達時の base fallback を含めて既存規則と一致。
- `evaluate` は manual override/latch が無い間に active return があれば
  上記 blended 値を `manual` として `evaluate_control` に渡し、
  `reference_source = ReferenceSource::Return` に訂正する。
- `return_ticks` は `set_return_duration(MediaDuration)` で可変設定
  (negative → `Validation`、frame_base へ `convert_exact`)。
  既定 = 暫定 150ms(frame_base へ整数換算)。
- `return_ticks == 0` は instant return — 即完了だが `instant: true`
  として `WriterState::Returning` で観測可能(click/zipper 表示)。

## 状態遷移(per target)

| event | 効果 |
| --- | --- |
| `gesture_begin` | fixation 固定・override seed(現 reference 値)・active return 除去 |
| `gesture_end`(Touch) | write gates 成立なら区間点列を txn へ → `TouchReturn` 開始 |
| `gesture_end`(Latch) | gates 成立なら区間点列 + `latch_held` に最終値保持 |
| `gesture_end`(Write) | gates 成立なら区間点列 + override は write-held として残る |
| `gesture_end`(Read) | override 破棄・即復帰(補間なし — Return は Touch のみ) |
| `transport_play` | **新規 play edge**(非 Playing→Playing)で全 runtime state 閉鎖 |
| `transport_stop` | 全 runtime state 閉鎖 + txn seal(Stopped) |
| `transport_seek` | 全 runtime state 閉鎖 + txn seal(Seek) |
| `transport_advance` の wrap | 全 runtime state 閉鎖 + txn seal(LoopWrap) |
| `note_focus_lost` | 全 runtime state 閉鎖(FocusLost) |
| `note_plugin_crash(project, processor)` | 当該 processor 所有 target の state 閉鎖 |
| `note_project_changed(project)` | txn seal(ProjectChange)+ 消えた target の state 閉鎖(TargetDeleted) |
| `set_mode` | latch/write-held 除去を `ModeChanged` で記録(return は mode 非依存で継続) |
| `gesture_cancel` | preview 破棄 + `Cancelled` 閉鎖記録 |

「閉鎖」= gesture preview 破棄 + manual override・latch held・
touch return の除去 + `WriterClosure` 記録(bounded log)。
transport 断続イベントでは txn が seal 済みなので discard/end の区別は
実質同値 — preview(uncommitted)の破棄で統一する。

## Write の記録開始点固定

`record_start(project, revision)`:

- signature に `&Project` を追加し、Arm+write-mode で固定された各
  target のうち `mode == Write` のものへ、開始位置で評価した
  reference(`evaluate_control(project, t, position, None).reference`)
  を `manual_overrides` に seed する。evaluate 失敗(例: scope 解決
  不能)は `record_start` の `Err` として伝搬 — silent fallback しない。
- `transport_advance` 中の既存 held-write 収集
  (`manual_overrides` ∩ `mode == Write`)がそのまま control-rate write
  を行う。非 armed/非 write-mode target は `write_gates` が塞ぐので
  不変のまま。
- Touch/Latch の fixed target には seed しない(gesture が起点)。

## 不変条件のまとめ

- Return は reference 領域の runtime 値 — 保存点列・revision・undo・
  recording segments を変更しない。`WriterClosure` log と
  `writer_state` だけが観測面。
- `record_start` は常に明示 — stop/play/seal 後に暗黙再開しない。
- Latch は stop/mode change で閉じ、write-held override は stop で除去。
- `evaluate` の manual 注入は gesture > latch/write-held > return の
  優先順 — 同時に 1 種のみ意味を持つ。

## RT / 依存方向

- 全て synvelis-workspace(non-RT crate)内。補間は pure f64。
  `eval_rt`/dispatch/session→project 依存方向は変更しない。
