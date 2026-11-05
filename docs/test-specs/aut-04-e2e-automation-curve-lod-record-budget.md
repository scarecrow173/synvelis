---
id: "TSPEC-034AUT04VgQF9P59tpgXou"
type: "test-spec"
status: "approved"
title: "AUT-04 E2E 検証仕様 — Automation Curve 非破壊保持・Viewport LOD・有界記録 Event 予算"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/134"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_04.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_04_rt.rs"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/test-specs/pc-c1-e2e-recording-transaction.md"
    - "docs/test-specs/aut-02-e2e-region-replace-boundary.md"
    - "docs/test-specs/aut-03-e2e-loop-write-one-pass.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
    - "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/ruviform/issues/257"
    - "https://github.com/scarecrow173/ruviform/issues/314"
    - "https://github.com/scarecrow173/synvelis/issues/20"
  defers: []
  deferred-by: []
---

# AUT-04 E2E 検証仕様

## 対象・基準

Issue #134 Acceptance が正本。headless で pin できる部分を
`crates/synvelis-workspace/tests/aut_04.rs`・`aut_04_rt.rs`
(+ 既存 aut_01/02/03・pc_c1 系テストの無退行)として実施する。
実 GUI/実 Browser/実 Plugin/実 OS 依存は NotRun/Unsupported
として記録する。

## Feature F1: 記録点列の非破壊保持・provenance

- **Rule**: `record_event_rt`→`drain_record_events`→commit の
  点列は Source Beat timestamp・Normalized Value を verbatim に
  保持し、Target/TimeScope/Clock provenance は tx fixation に
  従う。暗黙の Quantize/Decimation/Resampling をしない(同 pos
  last-wins dedup のみ明示正規化)。
- **Examples**:
  - 10^2 点 queue→drain→commit で lane region の pos/value が
    投入値と bit-exact(並びは pos sort 正規化のみ)
  - scope 解決済み pos・`tx.fixed_scopes`/`tx.epoch`/`tx.pass`
    が `record_start` 時点の値に固定される
  - commit→undo 1 回で point 列が消失、save/reload で復元
    (project のみ永続)

## Feature F2: Viewport LOD の正確性 — envelope・feature・gap

- **Rule**: `automation_view` の bin `min`/`max` は評価曲線の厳密
  値で近似ではない。StepEdge/Extremum/RegionBoundary の `pos` は
  `BeatPoint` 正確。series 未到達 bin は `covered=false`・
  `min/max=NaN`。chase により最終点以降は `covered=true`・
  `count=0`(held と missing を区別)。
- **Examples**:
  - Linear 2 点 series の bin min/max が eval の厳密値と一致
  - Step series の jump 位置に StepEdge feature(正確 `BeatPoint`)
  - 上がり→下がり転換点に Extremum、region range 端に
    RegionBoundary(点を持たない端でも発行)
  - 第 1 点より前の bin が NaN gap、最終点以降が held(covered,
    count=0)
  - 10^4 点で bins=64: 全 bin envelope が直接 eval と一致
    (uniform subsample 検証で厳密性を pin)

## Feature F3: Quality/Truncated の型分類

- **Rule**: `Exact` = view 内全格納点を verbatim 返却+全 feature
  発行。`Reduced` = verbatim 超過だが feature 欠落なし。
  `Truncated` = feature 欠落あり。envelope は全 quality で厳密。
- **Examples**:
  - bins>=points の zoom で Exact・全点 verbatim
  - `max_points` 縮小で Reduced・StepEdge/Extremum は残る
  - `max_features` 縮小で Truncated・`truncated_features>0`

## Feature F4: LOD の非破壊性 — revision/Undo/graph 不変

- **Rule**: view query は `Project::control` を読むだけで
  `ProjectRevision`・Undo 履歴・`EngineStub.graph_builds` を
  変化させない。Zoom/Scroll(=query 反復)でも不変。
- **Examples**:
  - query 前後で `session().revision()` 不変・`undo` 可能数不変
  - Inspector/SignalFlow/`lane_view`/`automation_view` を同一
    target に反復呼び出しても graph build 回数不変

## Feature F5: RT bounded queue — push/drain/overflow

- **Rule**: `record_event_rt` は gates closed/invalid で typed
  `RecordPushError`、queue 満杯で `QueueFull`。drain は FIFO に
  取り出し verbatim 格納。queue 満杯 drop は health 累計+gap、
  点列に捏造しない。
- **Examples**:
  - `record_event_rt` 成功で drain 後 point が verbatim 格納
  - queue 容量超過 push で `QueueFull`・`dropped` 累計・drain 後
    `DataGap` notification+`record_gaps` に区間、喪失分の点列なし
  - gates closed(sealed/disarmed/record off/not playing)で
    `GatesClosed`、値不正で `Invalid`
  - event が push 時点の txn id を刻印 — txn 跨ぎ event は drain
    で `WrongTxn` stale

## Feature F6: drain budget・point budget・Rejected 分類

- **Rule**: drain の `max_events_per_drain` 超過は残 event を
  queue に残し `Partial`(次 drain が retry)。point budget 超過は
  `RecordOverflow`+drop+gap。validation 失敗は `Rejected` を
  kind 別に分類。
- **Examples**:
  - drain budget 2 で 5 event: 1 回目 `Partial`(accepted=2,
    remaining=3)、2 回目で残り全て accepted(retry 契約)
  - `max_points` を小さく設定し budget 到達以降の event が
    `Overflowed` + `record_dropped` 累計 + gap 区間
  - gates closed で pop された event が `Rejected{GatesClosed}`
    + drop+gap 計上
  - loop 設定中 `pos >= loop_end` の drain event が
    `Rejected{OutsidePass}`

## Feature F7: 通知・2 Window・gap provenance

- **Rule**: QueueFull/RecordOverflow/DataGap/Rejected/Partial は
  bounded notification log + `RecordDrainReport` + `record_health`
  の累計カウンタで非 RT UI へ型付き通知。log は消費されず 2 つの
  window が独立 cursor で読む。
- **Examples**:
  - QueueFull 発生→drain で `QueueFull{target,dropped_total}` と
    `DataGap{lane,start,end,dropped}` notification
  - 2 cursor が同一 log の全通知を独立に走査できる
  - `record_health(target)` が dropped/queue_fulls/overflowed を
    累計報告(log 溢れても総量不変)

## Feature F8: session lane view — pending・delivery・revision token

- **Rule**: `ControlSession::automation_view` は committed LOD に
  open txn の pending segment(view 内 verbatim)・delivery span・
  scope 解決済み record gap・`ProjectRevision` token・health を
  重ねる。読み取り専用。
- **Examples**:
  - 録音中の view が `pending` に収集済み点列を返し commit 前
    series に混ざらない
  - gesture_end batch の delivered span が `deliveries` に
    (first,last) で記録される(gesture start/end mark)
  - queue 喪失の gap が `gaps` に scope 座標で返る
  - view の `revision` が発行時点の project revision と一致し、
    commit 後は古い view が stale と判定できる

## Feature F9: RT 契約(`aut_04_rt.rs`)

- **Rule**: `record_event_rt` の成功・`QueueFull`・`GatesClosed`・
  `Invalid` 全経路が alloc/dealloc 0(stats_alloc 計測、pc_b_rt
  手法 — 準備・warmup は測定窓外)。
- **Examples**:
  - Region 内で push→満杯→QueueFull→gates closed 一連を実行し
    `allocations == 0`・`deallocations == 0`
  - queue cap 境界で満杯 push が `QueueFull` を返し event が
    戻される

## 除外(NotRun/Unsupported として記録)

- 実 GUI/実 SolidJS Automation Lane(PC-C5 #102)/実 Canvas・
  WebGL2・2 Window の実 Window — headless session の等価 pin で
  代替(2 window は同一 session 状態の独立 cursor で pin)。
- RF-GT-05(#314)の Golden/Visual 基盤による描画検証。
- PC-C6 oracle(#103)の同一シナリオ別 Oracle 判定。
- 実 Plugin binding(PC-C4 #101/TIM-03 #127)経由の記録。
- Browser/実 OS/HighDPI/CPU/GPU 負荷・AudioLens Native Fidelity
  非回帰・SLO 閾値(Issue 本文どおり実測後に決定)。
- 真のスレッド分割 RT callback(実 engine への producer 移譲は
  Integration 責務 — headless では `&mut self` 経路の alloc 0
  証拠のみ pin)。
- `eval_rt`/dispatch/publish の回帰(変更面なし — 既存 pc_b_rt
  無退行で確認)。
