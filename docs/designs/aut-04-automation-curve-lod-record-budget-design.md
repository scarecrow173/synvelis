---
id: "DESIGN-034AUT04CvbbR3Qzx1aKwY"
type: "design"
status: "approved"
title: "AUT-04 Automation Curve 非破壊保持・Viewport LOD・有界記録 Event 予算 設計"
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
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
  implemented-by:
    - "docs/plans/aut-04-automation-curve-lod-record-budget.md"
  depends-on:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-04-e2e-automation-curve-lod-record-budget.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/ruviform/issues/257"
    - "https://github.com/scarecrow173/ruviform/issues/314"
  defers: []
  deferred-by: []
---

# AUT-04 Automation Curve 非破壊・Viewport LOD・有界記録予算 設計

## 1. 目的

Issue #134 の headless 契約を実装する:

1. `synvelis-project` に **bounded Viewport LOD 投影**
   `automation_view`(read-only、揮発 DTO、永続 schema 非接触)。
2. `synvelis-workspace` `ControlSession` に **RT bounded SPSC
   ingress** + 非 RT drain + 型付き喪失 provenance。
3. session 層の **lane view 組立**(revision token・pending
   segments・delivery span・record gaps・health)。

判断の正本は `docs/adr/aut-04-viewport-lod-bounded-record-queue.md`。

## 2. model 層 Viewport LOD(`synvelis-project/src/control.rs`)

### 2.1 公開型(全て揮発 DTO、serde 派生・永続化しない)

```rust
pub struct CurveQuery {
    pub start: f64,          // scope 座標の viewport 始端(lossy 投影)
    pub end: f64,            // 同終端(start < end)
    pub bins: usize,         // >0
    pub max_points: usize,   // verbatim 返却点の上限
    pub max_features: usize, // feature 発行の上限
}

pub struct ViewBin {          // 半開 bin(最終 bin は end を含む)
    pub start: f64,
    pub end: f64,
    pub min: f64,            // 未到達(NaN gap)は NaN
    pub max: f64,
    pub count: usize,        // bin 内の格納点数(0=covered held もあり得る)
    pub covered: bool,       // 評価曲線が bin に達するか
}

pub enum CurveFeatureKind { StepEdge, Extremum, RegionBoundary }

pub struct CurveFeature {
    pub pos: BeatPoint,      // 正確座標(precision 契約)
    pub value: f64,          // 点の値(Boundary では eval 値・未到達は NaN)
    pub kind: CurveFeatureKind,
}

pub enum CurveQuality { Exact, Reduced, Truncated }

pub struct AutomationCurveView {
    pub bins: Vec<ViewBin>,
    pub points: Vec<AutomationPoint>,   // verbatim 格納点(優先選択済)
    pub features: Vec<CurveFeature>,
    pub coverage: Vec<(f64, f64)>,      // view 内 region extent
    pub quality: CurveQuality,
    pub source_points: usize,           // view 内の格納点総数
    pub truncated_points: usize,        // 返却を断念した格納点数
    pub truncated_features: usize,      // 発行を断念した feature 数
}
```

### 2.2 `automation_view` 手続き

```text
target 検索 → merged_series(既存の one-logical-series)
series 空 → bins 全未 covered・quality Exact で返す
view 内格納点 = pos ∈ [start,end]
per-bin envelope(bin i は半開 [s_i, s_{i+1})、最終 bin は end を含む):
    covered = bin 内に x >= first_pos_f64 が存在 ⟺ e > first_pos_f64
    候補 = eval_at(s) + eval_before(e) + eval_at(p) ∀格納点 p ∈ [s,e)
       (最終 bin は eval_before(end) に加え eval_at(end) — ちょうど
        end の点を含む。eval_at は `pos <= x` の直前片、eval_before は
        `pos < x` の左極限片 — Step edge で bin をまたぐ値を混入しない)
    → min/max 厳密(Linear 区間は affine のため端点/点で極値)
    count = bin 内格納点数、未 covered は min/max=NaN
feature 分類(view 内):
    点 i が StepEdge ⟺ series[i-1].interp == Step && v_i != v_{i-1}
       (不連続は点 i に入る片の Step で発生 — i>0 が必要)
    点 i が Extremum ⟺ (v_i - v_{i-1})*(v_{i+1} - v_i) < 0(厳密方向反転)
    RegionBoundary = view に交差する各 region の range.start/end
                     (value = eval_at(pos)、未到達は NaN)
    「明示 Anchor」は `points` verbatim channel が保持する —
    feature kind としては重複させない(AUT-02 sentinel anchor は
    sentinel region の range 端で RegionBoundary として捕捉される)
quality:
    source_points <= max_points かつ全 feature 発行 → Exact
    verbatim 超過のみ → Reduced(feature 付き格納点を優先保持し、
    残 budget を先頭から順に無印点で埋めて max_points まで返す)
    feature 発行が max_features を超過 → Truncated
coverage: view に交差する region range を (f64,f64) で列挙
```

- 評価は `merged_series` を f64 へ lossy 投影した走査 eval —
  `prepared_lane_value` と同じ chase/Step/Linear 意味の f64 版。
  正本の `BeatPoint` 値は変更しない(投影は display-only)。
- `RegionBoundary` は格納点を持たない range 端でも発行する
  (coverage 縁は曲線の意味的境界)。

## 3. session 層(`synvelis-workspace/src/control.rs`)

### 3.1 RT ingress — 型

```rust
pub struct AutomationRecordEvent {      // Copy、SPSC payload
    pub txn: RecordingTransactionId,
    pub target: ParameterTargetId,
    pub pos: BeatPoint,                  // project beat(push 側座標)
    pub value: f64,
}

pub enum RecordPushError { GatesClosed, Invalid, QueueFull } // Copy

pub struct RecordBudgets {
    pub max_points: usize,          // 既定 RECORD_POINT_BUDGET
    pub max_events_per_drain: usize, // 既定 RECORD_DRAIN_BUDGET
}

pub struct RecordGap {                  // project beats で保持
    pub start: BeatPoint,
    pub end: BeatPoint,
    pub dropped: u64,
}
```

新定数: `RECORD_EVENT_QUEUE_CAP = 4096`・`RECORD_DRAIN_BUDGET =
4096`・`RECORD_NOTIFY_CAP = 256`・`RECORD_GAP_CAP = 256`・
`RECORD_HEALTH_CAP = 64`(pre-reserved、超過は `health_overflows`
カウンタ)。

### 3.2 `ControlSession` 追加フィールド

```rust
record_producer: Producer<AutomationRecordEvent>,
record_consumer: Consumer<AutomationRecordEvent>,
record_budgets: RecordBudgets,
record_health: Vec<RecordTargetHealth>,        // with_capacity(64)
record_notifications: VecDeque<RecordNotification>, // with_capacity(256)
record_health_overflows: u64,
record_notifications_dropped: u64,   // log cap 超過で落とした通知数
```

`RecordTargetHealth { target, dropped, queue_fulls, overflowed,
rejected, gap_open, gap_last }`(全て Copy フィールド)。

### 3.3 `record_event_rt`(RT-suitable、alloc 0)

```text
write_gates(target) 不成立 → Err(GatesClosed)
pos >= loop_end → Err(Invalid)
value 非有限/範囲外 → Err(Invalid)
ev = {txn: 現在の open txn id, target, pos, value}
try_push(ev) → Err → Err(QueueFull) + health(target):
    dropped+=1, queue_fulls+=1, gap_open??=pos, gap_last=pos
```

- project 参照なし — gate は session 状態のみ(BTreeMap 読取・
  `Option`/定数比較)。health entry は線形走査+Vec 内書込み。
- `&mut self` は headless 境界の API — 実 engine での producer
  分割は Integration(PC-C4 系)の責務として design に留める。

### 3.4 `drain_record_events`(非 RT)

```text
rep = RecordDrainReport::default()
pop <= record_budgets.max_events_per_drain の間:
    ev.txn != open txn id → rep.stale+=1、Rejected 通知
    gates closed / 値不正 / loop 境界 → rep.rejected.push(typed)
        + drop 計上(gap 開延)
    record_points(ev) → Ok → rep.accepted+=1
    Err(budget) → rep.overflowed+=1 + drop 計上(gap 開延)
    Err(other) → rep.rejected.push + drop 計上(gap 開延)
rep.remaining = queue 残量
health ごとに gap_open を tx.record_gaps[lane] へ閉じる
    (resolve_scope_pos で lane scope へ — 失敗は unresolvable
     gap として gaps_unresolved+=1)
通知を emit(QueueFull 累計 / Overflow 累計 / DataGap / Rejected
    / Partial)
```

`RecordDrainReport { accepted, rejected: Vec<RecordRejection>,
rejected_total, overflowed, stale, remaining, partial }`。

`record_rejection` kinds: `WrongTxn`・`GatesClosed`・`OutsidePass`・
`BadValue`・`LaneGone`・`ScopeLost`・`OverBudget`。

### 3.5 `RecordingTransaction` 追加(session 内のみ)

```rust
pub record_gaps: BTreeMap<AutomationLaneId, Vec<RecordGap>>,
pub deliveries: BTreeMap<AutomationLaneId, Vec<(BeatPoint, BeatPoint)>>,
pub record_dropped: u64,     // 本 txn 起因の喪失累計
```

- `deliveries`: `record_points` が `written > 0` の batch ごとに
  `(first_resolved_pos, last_resolved_pos)`(scope 座標)を記録 —
  gesture_end batch が gesture の delivered span(gesture start/
  end mark)を与える。cap 4096、超過は `deliveries_dropped`。
- fingerprint には含めない(payload=点列+固定 context のまま)。

### 3.6 `RecordNotification`(bounded log、pull 型)

```rust
pub enum RecordEventKind {
    QueueFull { target, dropped_total },
    RecordOverflow { target, dropped_total },
    DataGap { lane, start, end, dropped },   // project beats
    Rejected { target, reason: RecordRejectKind },
    Partial { accepted, remaining },
}
pub struct RecordNotification { pub txn, pub kind }
```

`record_notifications() -> &VecDeque<_>`(消費しない — 2 window は
各自 cursor を持つ)。

### 3.7 session lane view 組立

```rust
pub struct AutomationLaneView {
    pub target, pub revision: ProjectRevision,
    pub curve: AutomationCurveView,          // model LOD
    pub pending: Vec<(AutomationLaneId, Vec<AutomationPoint>)>,
    pub deliveries: Vec<(f64, f64)>,         // gesture run span(scope)
    pub gaps: Vec<ResolvedRecordGap>,        // scope 座標へ resolve 済
    pub gaps_unresolved: usize,
    pub health: Option<RecordHealthSummary>, // dropped/overflowed 等
    pub pending_truncated: bool,
}
```

`ControlSession::automation_view(&self, project, target, query)`
→ model `automation_view` + open txn の fixed lane が target を
向く segments を view 範囲で filter(scope 座標= query 座標)+
deliveries + record_gaps を `resolve_scope_pos` で scope 座標へ。
revision は呼出側が渡す `ProjectRevision`。

## 4. 変更しないもの(設計判断)

- `record_points` の sort+同 pos last-wins dedup(明示正規化)、
  永続 schema、`WriteAutomationRegion` Replace、exactly-once
  apply、AUT-01/02/03 の境界契約は不変。
- `writer_closes`・observation・eval_rt/dispatch 経路は変更しない。
- pending segments は view で `Linear` 補間として扱う(prepare と
  同じ仮定 — prepare が `Interpolation::Linear` で書戻す既存契約)。
- queue 損失の placeholder/補間点は生成しない(D-5)。

## 5. 例外・境界条件

- **bin 端の点**: `pos == bin.end` は次 bin 所属(最終 bin のみ
  `end` を含む)。Step 点が bin 端に乗るとき jump は次 bin で表現。
- **series 無し/全て view 外**: bins は全て未 covered(NaN gap)、
  quality Exact・source_points=0。
- **queue 一杯 + txn seal**: seal で write_gates 閉 → push は
  GatesClosed。queue に残った未 drain event は次 drain で
  GatesClosed Rejected + gap 計上(喪失として正直に計上)。
- **txn abort/commit 後の残留 event**: 次 txn の drain で
  `WrongTxn` stale(そのデータは旧 txn のもので現 txn の喪失では
  ない — gap に計上しない)。
- **health cap 超過**: 新規 target の drop 計上は
  `record_health_overflows` に集約(typed、隠れない)。
- **drain budget=0**: `Partial` に留まる。0 event drain は no-op。
- **view query の `start>=end`/`bins==0`**: `Error::Validation`。

## 6. テスト計画(`tests/aut_04.rs` + `tests/aut_04_rt.rs`)

test-spec の Feature F1..F8 に対応:

- F1 非破壊保持: queue→drain→commit で点列 verbatim・provenance
  (scope/epoch/pass/txn)・同 pos dedup・undo。
- F2 LOD 正確性: 10^2〜10^4 点・envelope 厳密値・Step/Linear・
  Extremum 位置・NaN gap・coverage・Exact/Reduced/Truncated。
- F3 不変性: zoom/scroll クエリで revision/undo/graph build 不変。
- F4 queue: 容量超過で QueueFull typed・drop 累計・喪失分捏造無し。
- F5 drain/budget: rate budget Partial+retry・point budget
  Overflow+gap・Rejected 分類・stale event。
- F6 gap/health/通知: DataGap 発行・2 window 独立 cursor・累計値。
- F7 session view: pending segment/delivery/gap resolve・revision
  token・pending_truncated。
- F8 RT 契約(`aut_04_rt.rs`): `record_event_rt` 全経路 alloc 0
  (stats_alloc)、queue cap 境界。
- 無退行: aut_01/02/03・pc_c1・pc_a/pc_b 系。
