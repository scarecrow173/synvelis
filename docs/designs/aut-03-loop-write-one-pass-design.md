---
id: "DESIGN-034AUT03B1wG9hW1zS0Yf4"
type: "design"
status: "approved"
title: "AUT-03 Loop 録音 1 Pass — Write Range 半開区間閉鎖・境界 flush・明示 Rearm 設計"
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
  implemented-by:
    - "docs/plans/aut-03-loop-write-one-pass.md"
  depends-on:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
    - "docs/designs/aut-01-touch-return-writer-state-design.md"
    - "docs/designs/aut-02-region-replace-boundary-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
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
    - "https://github.com/scarecrow173/synvelis/issues/20"
  defers: []
  deferred-by: []
---

# AUT-03 Loop 録音 1 Pass 設計

## 1. 目的

`ControlSession` の recording gate が **1 Loop Pass の半開
Project Beat 区間**を記録し、境界で Write Range を閉じて
Pending/Prepared Commit へ遷移する契約を実装する。次 Pass の
記録は Explicit Rearm(`record_start`)のみ。詳細契約は
`docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md`、
判断理由は `docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md`。

## 2. 変更する型・状態

全て `crates/synvelis-workspace/src/control.rs`(session 内のみ、
永続化 schema には触れない)。

### 2.1 `LoopPassId`

```rust
pub struct LoopPassId(u64);
```

`PartialEq`/`Eq`/`Ord`/`Copy`/`Display` 導出(他 session ID と同規約)。
`PlaybackState.pass: LoopPassId` を新設し:

- `transport_play`: 非 `Playing` → `Playing` edge で `pass = 1`
  (`resume` は同一 run で不変、既存 `run_id` reset と同じ
  判定 `old_state != Playing` を共有)。
- wrap(`to >= end` で `to = start` に戻る経路、stop 時を除く):
  `pass = pass.checked_add(1)`(u64 overflow は現実的に到達不能、
  checked API で明示)。
- `transport_pause`/`resume`/`seek`/`stop`: `pass` は変えない。
  `seek`/`stop` 時も pass は「その run で何周目か」を表し続け、
  新規 play edge でのみ 1 に戻る。

### 2.2 `RecordingTransaction` 追加フィールド

```rust
pub pass: LoopPassId,                      // record_start で fixation
pub fixed_scopes: BTreeMap<TargetId, AutomationTimeScope>,
pub coverage_end: BTreeMap<AutomationLaneId, BeatPoint>,
```

- `fixed_scopes`: `record_start` 時点で `project.control.target(t)`
  の `AutomationTimeScope` を target 全 lane で固定(fingerprint/
  audit)。`recording_target_lanes` 生成の `&scope` 引数はこの
  fixed 値を使う(現行の call-site で project から再取得する
  経路を置換)。
- `coverage_end`: D-1 の lane ごとの半開区間終端(Project Beat)。
  `sealed.is_none()` の間のみ書き込み、first-seal-wins。
- `SessionTxPayload` fingerprint に `pass`/`fixed_scopes`/
  `coverage_end` を加える(冪等 apply の同一性検査)。

### 2.3 `SealReason` 追加

```rust
EpochChange, GraphStale, LoopChanged, NewTransportPlay,
```

`Display` は snake_case 文字列に写像(既存 variant と同規約)。

### 2.4 `SealOutcome` 追加フィールド

```rust
pub write_range_closed: bool,   // coverage_end/flush を実行したか
```

観測可能性のためだけの報告フィールド(契約には使わない)。

## 3. Write Range close の共通経路

全 seal 経路は既存の「`sealed.is_none()` ガード + reason 設定」
の先頭で `close_write_range(project, bound)` を呼ぶ:

```rust
fn close_write_range(&mut self, project: &Project, bound: BeatPoint)
```

- `bound` は close 経路ごとに固定:
  - LoopWrap: `loop_end`(半開区間の排他的終端)。
  - Stop/Seek/Epoch/Graph/Loop/Arm/Mode/ProjectChange: 現在
    `playback.position`(その位置の書込みを含む区間)。
- `tx.sealed.is_some()` なら即 return(first-seal-wins)。
- loop 設定中は `bound = min(bound, loop_end)` — loop 外側の
  延長は Pass 1 に帰属しない。
- **flush**(D-2): `self.write_gates()` を満たす場合のみ、
  `in_flight_gestures` の `Write`/`Latch` で `record` が arm
  された target について、配送済み `points` を
  `p.pos <= bound_resolved && p.pos < pass_end` で clip して
  `tx.record_points(...)` へ流す。clip 規則の比較は **project
  座標で行ってから scope resolve** せず、delivery 時に既に
  scope resolve 済みの pos を使う — `pos < pass_end` の比較は
  `resolve_scope_pos` で loop end を scope 座標へ変換してから
  比較する(resolve 失敗 lane は clip しない = flush しない)。
- **coverage_end 記録**: `Write`/`Latch`/`Touch` のいずれかで
  その lane の writer(`AutomationWriterState::Write`/`LatchHeld`/
  `WriteHeld`/`TouchHeld` の write-gated 系)が境界瞬間に存在
  する lane について `coverage_end[lane] = bound`。
- gesture runtime state は従来どおり `gesture_end(Cancelled)`
  で閉じる — flush 後に残った preview 値は捨てる。

### 3.1 seal 経路ごとの変更

| 経路 | reason | bound | 変更 |
| --- | --- | --- | --- |
| loop wrap | `LoopWrap` | loop `end` | 既存 seal を close_write_range 化 + `pass += 1` |
| `transport_stop` | `Stopped` | position | 同上(signature に `&Project`) |
| `transport_seek` | `Seek` | position(to) | 同上 |
| `install_clock_anchor` epoch 変更 | `EpochChange` | position | 既存 epoch 差分検出を seal 化 |
| `note_graph_stale`(新設) | `GraphStale` | position | host 明示通知で close |
| `set_loop` 録音中 loop 変更 | `LoopChanged` | position | 録音中の loop 範囲変更を seal 化 |
| `set_write_arm(false)` | `ArmOff` | position | close_write_range 化 |
| `set_mode(Read/Touch...)` | `ModeChange` | position | 同上 |
| `note_project_changed` | `ProjectChange` | position | 同上 |
| 非 `Playing` → `Playing` edge(録音 open 中) | `NewTransportPlay` | position | `transport_play` で既存 paused/stopped からの edge を seal 化 |
| `FocusLost`/`PluginCrash`/`Cancelled` | — | — | **Write Range は閉じない**(runtime closure のみ) |

`close_write_range` は `Project` の `resolve_scope_pos` を参照
するため、全 seal 経路の signature が `&Project` 依存になる
(`control.rs` 内の `ControlSession` method に限る。`Session`/
`controls_ui` の呼出側は既に `&Project` 相当を持つ)。

### 3.2 `record_write` / `record_points` の Pass 境界ガード(D-4)

- `record_write`: tx が open で loop 設定中、`at >= loop_end` は
  `Reject::Validation` で拒否。`at < loop_start` は許容。
- gesture の `record_points`(in-flight 中の逐次配送): `pos >=
  loop_end` の point は clip(skip)し、`pos <= bound` までは通常
  どおり受け付ける。clip は silent ではなく debug log 相当の
  観測点を残す(戻り値で clip 数を返すのは API 面を増やすため
  見送り、test は結果の point 集合で検証)。

### 3.3 `prepare_record_commit` の coverage 反映

```text
for each segment:
    end_bound = tx.coverage_end.get(lane)
        .filter(|b| *b > last_pos)
        .map(|b| resolve_scope_pos(scope, b))
        .flatten()            // resolve 失敗は coverage 不採用
        .unwrap_or(last_pos + epsilon)
    WriteAutomationRegion{Replace}[first_pos .. end_bound)
```

`coverage_end > last_pos` の場合のみ採用(≤ なら point 列の方が
広い = 既存動作)。`resolve_scope_pos` 失敗時は last+ε fallback。
`BeatPoint` → scope 座標変換は既存 `recording_target_lanes` と
同じ `resolve_scope_pos` 経路を使う。

## 4. `record_start` の fixation 変更

```rust
pub pass: LoopPassId = playback.pass;
fixed_scopes = targets.map(|t| (t, project.control.target(t).scope));
```

- `record_start` 呼出時点の `playback.pass`/`ProjectRevision`/
  `ClockEpoch`/`AutomationTimeScope` を transaction へ固定する
  既存契約に `pass`/`scope` を追加。
- `targets` 固定は既存 `fixed_targets` のまま。Mode/Follow/
  Inspector/Window 変更による retarget は起きない — 録音中は
  `self.automation_mode` を変えても `fixed_targets` が優先される
  (ModeChange は Write Range close イベントでもある)。

## 5. 変更しないもの(設計判断)

- `sealed`/`Prepared`/`Applied`/`Conflict`/`Rejected` 状態機械、
  CAS expected_revision、exactly-once apply、Rebase/Reject、
  1 commit = 1 undo、`StoredAutomation` の pre-modulation 値
  契約は PC-C1 のまま不変。
- AUT-02 の boundary anchor / Replace 適用は `ops.rs` 側の
  責務 — `prepare` が coverage_end で作った Replace range は
  そのまま anchor 適用を受ける。
- `eval_rt`/dispatch/RT publish、PointBudget、`PendingCount`、
  `PluginCrash`/`FocusLost`/`Cancelled` の runtime closure。
- `record_start` より前の pos を許容する既存 punch-in 規則。
- Multi-pass Take / Punch 履歴 / GUI 配線 / 実 Plugin 連携。

## 6. 例外・境界条件

- **wrap と seal の競合**: `transport_advance` で `to >= end` の
  wrap は同一呼出内で pass+1 → close_write_range → seal
  `LoopWrap` の順で処理。`pos == end` ちょうどでも wrap 扱い
  (`>=`)。`stop` との同時呼出は session API の逐次呼出のため
  発生しない。
- **loop 中の stop/seek**: `bound = min(position, loop_end)`。
  position が loop 内ならそのまま、loop 外へ seek したら
  `loop_end` で clamp されるため coverage は loop 内に収まる。
- **録音中の `set_loop`**: loop 変更は座標基準失効として
  `LoopChanged` seal。loop クリア(録音中に None)も同様。
- **loop 無しでの録音**: `loop_range` が `None` のままの録音は
  coverage の clamp 対象なし、`pos >= loop_end` ガードも無効 —
  従来動作(pass は wrap しない限り 1 のまま)。
- **first-seal-wins**: 既に sealed の tx では
  `close_write_range` は no-op — coverage_end/flush を二度
  実行しない。
- **`NewTransportPlay` edge**: `transport_play` が `Stopped`
  からだけでなく `Paused` から呼ばれた場合(現行 API は resume
  とは別に直接 play も受け付ける)も、非 `Playing` → `Playing`
  edge として同じ seal 規則を適用。
- **epoch 変更の検出**: `install_clock_anchor` で渡される anchor
  の `clock_epoch` と tx が固定した `clock_epoch` が異なる場合に
  `EpochChange` seal。同 epoch の anchor 更新は seal しない
  (従来どおり anchor 更新のみ)。
- **`note_graph_stale`**: 引数なし(現在の open tx を `GraphStale`
  で close)。open tx が無ければ no-op。

## 7. 呼出側(`controls_ui`)の対応

- `transport_stop`/`transport_seek`/`set_loop`/
  `install_clock_anchor`/`note_graph_stale` を呼ぶ箇所で
  `&Project` を渡す — いずれも UI 層は既に project を持つ。
- `note_graph_stale` の呼出: `controls_ui` の LastGood graph
  stale 検知点(存在する経路)に接続。headless test は session
  API を直接呼ぶ。

## 8. テスト計画(`tests/aut_03.rs`)

- **Loop Wrap**: loop 中央で record_start → wrap → pass=2・
  coverage_end=`end`・sealed=`LoopWrap`・point は `pos < end`
  のみ。2 周目の transport_advance では何も記録されない。
- **半開区間**: `end` ちょうどの位置で wrap した場合、その pos
  の point は次 Pass 所属として Pass 1 segment から除外。
- **in-block wrap**: `transport_advance(to > end)` で wrap が
  block 内に収まる場合も同じ閉鎖が起きる。
- **mid-loop start**: loop 中央 position で record_start しても
  区間 `[start_pos, end)` が記録され、先頭 `< start_pos` の点は
  入らない。
- **tempo change 中の wrap**: `install_clock_anchor` は既存の
  epoch 変更検出を残したまま、wrap 境界が Project Beat 座標で
  正しく閉じる。
- **same-frame events**: wrap 後の同 frame で `record_start`
  (次 Pass) → 新 tx の `pass=2`・`expected_revision` が apply
  前値・`coverage_end` は未記録(新しい録音のため)。
- **seal 追加理由**: `EpochChange`/`GraphStale`/`LoopChanged`/
  `NewTransportPlay` を個別に pin。
- **失敗時保全**: Stale CAS + wrap closed tx で point 保持、
  Conflict → `RequiresUserRebase` 経路。
- **1 undo**: commit 後 `undo` 1 回で pass-1 区間が元に戻る。
- **flush**: write-gated gesture の配送済み point が wrap 時に
  transaction へ入り、`pos >= end` は入らない。
- **coverage > last**: held Write が `end` まで書き続けた場合、
  Replace range が `end` まで伸びる(headless では
  `coverage_end` を直接検証し、`prepare` の range 決定も pin)。
- **非 write-gated gesture は破棄**: Read/Unarmed gesture は
  flush されず、runtime closure のみ(AUT-01 契約の維持)。
