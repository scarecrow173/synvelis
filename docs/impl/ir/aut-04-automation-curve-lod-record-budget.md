---
id: "IMPL-AUT04CurveLodRecordBudget"
type: "impl"
status: "completed"
title: "AUT-04 Automation Curve 非破壊保持・Viewport LOD・有界記録 Event 予算 実装記録"
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
      - type: "file"
        path: "docs/specs/aut-04-automation-curve-lod-record-budget.md"
      - type: "file"
        path: "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
      - type: "file"
        path: "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
      - type: "file"
        path: "docs/plans/aut-04-automation-curve-lod-record-budget.md"
      - type: "file"
        path: "docs/tasks/aut-04-task1-automation-curve-lod-record-budget.md"
      - type: "file"
        path: "docs/test-specs/aut-04-e2e-automation-curve-lod-record-budget.md"
      - type: "file"
        path: "docs/impl/ir/aut-04-automation-curve-lod-record-budget.md"
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
    - "docs/tasks/aut-04-task1-automation-curve-lod-record-budget.md"
  implemented-by: []
  depends-on:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
    - "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
    - "docs/plans/aut-04-automation-curve-lod-record-budget.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/aut-02-region-replace-boundary.md"
    - "docs/impl/ir/aut-03-loop-write-one-pass.md"
    - "docs/impl/ir/pc-c1-recording-transaction-prepare-apply-finalize.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-04-e2e-automation-curve-lod-record-budget.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUT-04 実装記録: Automation Curve 非破壊保持・Viewport LOD・有界記録 Event 予算

## Summary

Issue #134。記録 Automation 点列の非破壊保持(source truth は
`Project::control` のみ、LOD は永続データを一切持たない揮発
projection)、bounded viewport LOD(厳密 min/max envelope +
分類済み feature + verbatim 点 channel + `Exact/Reduced/
Truncated` quality + NaN uncovered bin)、および RT bounded
SPSC 記録イベント経路(queue overflow / drain budget overflow /
reject / partial / data gap をすべて型付き分離)を headless で
実装した。

## Context

- Spec: `docs/specs/aut-04-automation-curve-lod-record-budget.md`
- ADR: `docs/adr/aut-04-viewport-lod-bounded-record-queue.md`
- Design: `docs/designs/aut-04-automation-curve-lod-record-budget-design.md`
- Plan: `docs/plans/aut-04-automation-curve-lod-record-budget.md`
- Test-spec: `docs/test-specs/aut-04-e2e-automation-curve-lod-record-budget.md`
- 前提: AUT-02 sentinel anchor(#132)・AUT-03 write txn(#133)・
  PC-C1 recording txn(#98)・Ruviform SPSC(#257 系 RT primitive)。
- 横断依存正規化 v1.1: #102 SolidJS lane・#314 Golden/Visual・
  #103/#114 Oracle は Integration 段階 — 本件は headless
  semantic 契約のみ(実 Browser/実 Canvas は NotRun)。

## Implementation

### Model 層 `automation_view` (synvelis-project)

`Project::control` の merged series(既存 `merged_series`、
one-logical-series)から viewport `[start,end]`(scope 座標の
lossy `f64` 表示軸)を bounded 縮約する read-only 関数:

- **`ViewBin` 厳密 envelope** — 半開 `[s_i, e_i)`、最終 bin のみ
  `end` 含む。候補は `eval_at(s)` + `eval_before(e)`(最終は
  `eval_at(e)`)+ bin 内各格納点 — Linear は affine のため
  端点/点で極値が出るため厳密。`covered = series 到達`、未到達は
  `min/max = NaN`(`0` や補間値を捏造しない)。
- **`CurveFeature`** — `StepEdge`(`series[i-1].interp == Step`
  の不連続 — review で「入る点の interp ではなく前 segment の
  interp」と修正)、`Extremum`(厳密方向反転)、`RegionBoundary`
  (view 交差 region の range.start/end — AUT-02 sentinel
  anchor もここで捕捉。`Anchor` kind は verbatim channel と
  重複のため review で削除)。位置は `BeatPoint` 正確保持。
- **`points`** — verbatim 格納点 channel(明示 Anchor)。
  `max_points` 超過時は feature 点を優先保持し残りを先頭から
  埋める — `truncated_points` で正直に報告。
- **`CurveQuality`** — feature drop → `Truncated`、verbatim
  縮約のみ → `Reduced`、全保持 → `Exact`。

`eval_at_f64`/`eval_before_f64` は `f64` series 上の
`pos <= x` / `pos < x` partition — Step edge で bin をまたぐ値を
混入しない。Project は不変・undo なし・永続列は唯一の真実。

### Session 層 record queue + health (synvelis-workspace)

- **`record_event_rt`** — `ruviform_rt::channel(4096)` SPSC への
  `try_push` のみの RT-suitable 経路。stats_alloc 計測で成功・
  invalid-reject 両パス 0 alloc を pin(aut_04_rt)。QueueFull は
  typed error + health 計数 + gap run 開始 — silent drop なし。
- **`drain_record_events`** — 非 RT、event budget 有界。
  `WrongTxn / GatesClosed / OutsidePass / BadValue / ScopeLost /
  LaneGone / OverBudget / Failed` を `RecordRejectKind` で分離。
  drain 側 point budget(`RecordBudgets.max_points`、既定
  `RECORD_POINT_BUDGET` で既存契約不変)超過は `overflowed` +
  `OverBudget` — 記録済み点は保持(非破壊)、失敗分だけ gap。
- **`RecordTargetHealth`** — target 毎の
  `dropped / queue_fulls / overflowed / rejected` +
  gap run(`gap_open/gap_last/gap_dropped`)+ `notified_fulls`
  watermark(QueueFull 通知は drain 毎 1 回・delta のみ)。
- **`RecordGap`** — drop run は tx の `record_gaps[lane]`(project
  beats、view で scope resolve)に fold — 穴を曲線として捏造
  しない。cap 256、超過は末尾 merge + `record_gaps_merged`。
- **`deliveries`** — record_points が受けた batch の delivered
  extent(半開区間の gesture flush = delivered points のみ —
  AUT-03 の半開契約と整合)。
- **`AutomationLaneView`** — model LOD + `pending`(未 commit 記録
  点 overlay)+ `deliveries` + `gaps`(scope resolve 済み、
  不能は `gaps_unresolved` 計数)+ `health` + `revision`(stale
  検出用)の read-only projection。
- **`RecordNotification`** — QueueFull / DataGap / Rejected /
  Partial の bounded log(cap 256、超過は
  `record_notifications_dropped` 計数 — review で追加)。

### 設計からの差分

- `drain_record_events` を `drain_record_event`(1 event 分類)+
  `drain_reject`(reject 集約)+ `fold_record_gaps` +
  `emit_drain_notifications` へ分割 — clippy too_many_lines
  対応と pipeline の明示性。语义は design どおり。
- verbatim 縮約の優先順位は design の「StepEdge>Extremum>
  Boundary 近接>Anchor」を「feature 点優先 + 残り先頭順」へ
  単純化(Anchor kind 削除に伴う整理 — 機能差なし)。

## Validation

### 検証結果(実施済み・全緑)

- `mise run fmt` / `fmt-check`: pass
- `mise run lint` (`cargo clippy --workspace --all-targets -D
  warnings`): pass
- `mise run test` (`cargo test --workspace --locked`): 全 binary
  緑(`tests/aut_04.rs` 19 件・`aut_04_rt.rs` 1 件を含む。
  aut_01/02/03・pc_a/b/c1・sv47/48・ws_a 無退行)
- `mise run test-rt`: 全緑(`record_event_rt_alloc_free_...`)
- `mise run test-msrv` (rust 1.89): 全緑
- `mise run check-headless-deps`: pass(GUI 依存なし 8 crate)

### `tests/aut_04.rs` + `aut_04_rt.rs` が pin するもの

- F1 非破壊保持: 有理数 BeatPoint(1/3, 7/11)+ 高精度値が
  verbatim 保持、view は project/revision/格納点を不変にする。
- F2 bounded LOD: bins 数・厳密 envelope・先頭未到達 bin は
  covered=false + NaN・empty series 全 uncovered・invalid
  window 拒否。
- F3 features/quality: StepEdge(前 segment interp 起点)・
  Extremum・RegionBoundary・Reduced(feature 点は verbatim
  縮約でも保持)・Truncated(feature drop)。
- F4 session overlay: pending/deliveries/gaps(scope resolve)/
  health/revision。
- F6 分類: GatesClosed push・WrongTxn(abort 後 stale)・
  GatesClosed(stop 後 drain)・OverBudget(点 budget)・
  health の overflowed/queue_fulls 分離・tx.record_gaps に
  fold・view.gaps に出る。
- F7 Partial: drain budget < queued → partial=true + Partial 通知、
  retry で全 drain。
- F8 bounded: queue 満杯 → typed QueueFull、health 計数、
  DataGap 通知、tx.record_dropped、record_gaps fold。
- RT: 成功/invalid push の 0 alloc(stats_alloc)、queue-full も
  bounded health row のみ。

## Risks

- `CurveQuery` は `f64` 軸 — 表示専用と文書化。scope 座標の
  正確保持は `BeatPoint` feature/点列側が担う(契約どおり)。
- `record_health` は cap 64 有界 — 超過 target の drop は
  `record_health_overflows` 集計行にのみ残る(per-target
  分解はなし。通知/honest 性は維持)。
- RT push が `&mut ControlSession` を取る設計のため、実 host
  では producer 共有構造(`try_clone` 相当 or session 分離)が
  Integration で必要 — headless 契約の範囲では問題なし。

## Follow-ups

- PC-C5 #102 SolidJS Automation Lane 統合 — `AutomationLaneView`
  を SolidJS 側へ写す read-only wiring(Integration、本件外)。
- RF-GT-05 #314 Golden/Visual — envelope/feature の実描画
  golden(Integration、本件外)。
- #103/#114 Oracle — 実 GUI/plugin/OS acceptance(Integration、
  NotRun)。
- AUT-F01 multi-pass take — 本件の対象外(issue #6、
  Unsupported)。
- 永続的曲線 Simplification — 今回の非対象(将来の明示機能)。
