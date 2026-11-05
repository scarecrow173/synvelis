---
id: "SPEC-034AUT04VJToXbqZEgrP85"
type: "spec"
status: "approved"
title: "AUT-04 Automation Curve 非破壊保持・Viewport LOD・有界記録 Event 予算"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/134"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
    - "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-04-e2e-automation-curve-lod-record-budget.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/ruviform/issues/257"
    - "https://github.com/scarecrow173/ruviform/issues/314"
  defers: []
  deferred-by: []
---

# AUT-04 Automation Curve 非破壊保持・Viewport LOD・有界記録 Event 予算

## What Should Be Built

Issue #134「2026-11-04 合意・初期製品」を実装する。**記録された
Automation 点列(永続 Source)と GUI 描画用 LOD(揮発投影)を分離**
し、記録入力に RT bounded queue と型付き予算分類を導入する。
横断依存正規化 v1.1 に従い、PC-C5(#102)の実 SolidJS Automation
Lane や RF-GT-05(#314)の Golden 基盤の完了を**着手条件にしない**
— 本 PR の範囲は headless/semantic 実装と決定的試験であり、実
Browser/実 Canvas/実 OS 評価は Integration 以降の責務。

### 契約(Issue #134 が正本)

- **記録値の非破壊保持**: 記録点は Source Frame/Beat timestamp・
  Normalized Value・Target/TimeScope・Plugin/Clock provenance を
  保持する。元値への暗黙 Quantize/Decimation/Resampling を行わない
  (同一 pos の last-wins dedup は既存の明示正規化として維持)。
- **Bounded Viewport LOD**: GUI 表示は Bounded Query で縮約する。
  Peak/Extrema・Step 境界・Curve 不連続・Gesture start/end・
  明示 Anchor を落とさず、近似と閲覧状態の Quality/Truncated を
  示す。View Position/Zoom 変更で `ProjectRevision`/Undo は不変。
- **永続的曲線 Simplification 禁止**: Exact/Quality Proof のある
  将来の明示機能としてのみ扱う。実 Plugin Param Range/Normalized
  Precision を考慮しない一律絶対誤差 Point 削減は行わない。
- **有界記録 Event 予算**: Record Rate/Point Budget・RT bounded
  queue・NonRT Write buffer・Failed/Overflow/Partial/Retry
  policy を型で区別する。RT callback で Unbounded alloc/IPC/IO
  を禁止。Queue loss を滑らかな曲線として捏造しない。
- **単一 ControlModel 投影**: Inspector/SignalFlow/Automation
  Lane は同じ `Project::control` の read-only projection を読み、
  任意 UI Frame ごとに Controller Commit/Graph Recompile しない。

### 本件で具体化する部分(Issue の headless 解釈)

- **`automation_view`(model 層の bounded LOD 投影)**:
  `Project::control` の merged series から、viewport
  `[start,end)`(scope 座標の lossy `f64` 表現)を `bins` 個の
  per-bin **exact min/max envelope** と、分類済み `CurveFeature`
  (StepEdge/Extremum/RegionBoundary — `BeatPoint` 正確位置)と
  verbatim 格納点(明示 Anchor channel)へ縮約する。envelope は
  評価曲線の厳密 min/max で近似ではない。`Quality =
  Exact | Reduced | Truncated` で忠実度を型付き報告する。
- **RT 記録 Event 経路**: `ruviform_rt::channel` の固定容量 SPSC
  を `record_event_rt`(RT-suitable、alloc なし)の ingress とし、
  `drain_record_events`(非 RT)が session 点列へ写す。push 側
  drop は QueueFull、drain 側 point-budget 超過は RecordOverflow、
  いずれも lane 単位の **DataGap(喪失区間)へ集計**され、喪失分の
  点列は一切捏造しない。
- **型付き通知・観測**: QueueFull/RecordOverflow/DataGap/
  Rejected/Partial(`DrainReport`+ bounded notification log)を
  非 RT UI が pull 可能にする。2 Window は同一 session 状態を
  独立 cursor で読む(push 型購読は持たない)。
- **session 層 view 組立**: `ControlSession::automation_view` が
  model LOD に `ProjectRevision` トークン・open transaction の
  pending segments/delivery span(gesture start/end 由来)・
  記録喪失 gap を重ねた read-only lane view を返す。
- **予算の区別**: `RECORD_POINT_BUDGET`(transaction の点列上限、
  既存)は永続側の非 RT memory bound、`RECORD_EVENT_QUEUE_CAP`
  (RT ring 容量)・drain budget(1 drain の処理上限)・notification/
  gap/health の各 cap はそれぞれ独立した定数/設定とする。

## Why

- AUT-01..03 は記録点列の非破壊・半開区間・境界保全を担保したが、
  **表示経路**(大量点列を viewport で縮約する方法と、その忠実度の
  型付き報告)と **RT 入力経路**(制御点が RT callback から session
  点列へ届く経路の上限と喪失分類)は未定義だった。
- 現行 `record_write`/`record_points` は非 RT 呼出で、RT callback
  からの直接呼出では `resolve_scope_pos`/`BTreeMap` 挿入などの
  unbounded alloc 経路を踏む — SPSC 分離が本質的に必要。
- Queue loss を点列へ補間して「滑らかな曲線」に見せることは
  「記録値の非破壊保持」と「失敗の型付き通知」の両方に反する。
  喪失は点列ではなく gap provenance として保持・表示する。

## Acceptance Criteria

(Issue #134 §Acceptance が正本。headless で pin する範囲を明記)

- 10^2〜大規模(10^4 級)点列・Zoom/Scroll・Step/Linear/Extrema
  で LOD がサンプル境界と対象意味を保持する: envelope min/max
  は評価曲線の厳密値、StepEdge/Extremum/RegionBoundary の位置は
  `BeatPoint` 正確、series 未到達 bin は NaN gap、Quality/
  Truncated は正確に分類される(全て `cargo test` 決定的検証)。
- Curve の Save/Reload/Undo/永続点列は LOD query で不変 — view
  query は `ProjectRevision`・Undo 履歴・graph build 回数を
  変化させない。
- QueueFull/Record Overflow/Data Gap/2 Window の状態が非 RT UI へ
  型付き通知される。喪失区間は lane view の `gaps` と health
  counter で観測可能で、点列には喪失分を補間した点が存在しない。
- RT 記録 push(`record_event_rt`)は成功・QueueFull・gate closed
  の全経路で alloc 0(stats_alloc 計測、既存 pc_b_rt 手法)。
- drain の rate budget は `Partial`(残 event は再 drain で回収)、
  point budget は `RecordOverflow`+gap で型分離される。
- Inspector/SignalFlow/Lane view は同一 `Project::control` の
  read-only projection を共有し、query 回数に依らず
  `ProjectRevision`・graph build 回数・Undo が不変。
- AUT-01/02/03・PC-C1 の全既存テスト無退行。

## Scope Boundary(本 dispatch の範囲)

- **In scope**: `synvelis-project` の `automation_view` bounded
  LOD 投影、`synvelis-workspace` `ControlSession` の RT SPSC
  ingress・drain・health/gap/notification・session lane view、
  `ruviform-rt` 依存追加、headless acceptance test(`aut_04.rs`
  + `aut_04_rt.rs`)、docs チェーン。
- **Out of scope**: Multi-pass Take/Punch 履歴(AUT-F01 #6)、実
  Plugin binding(PC-C4 #101/TIM-03 #127)、GUI 接続(PC-C5 #102/
  SV-GUI-05 #114)、PC-C6 oracle(#103)、PPQN 変更(TIM-01 #125)、
  永続化 schema 変更、永続曲線 Simplification、RT/engine 側の
  本番配線(実 engine への producer 分割は Integration 責務)、
  Browser/OS/HighDPI/CPU/GPU 負荷・AudioLens Native Fidelity の
  実測(SLO 閾値は実測後に決定 — Issue 本文どおり)。

## Non-Functional Requirements

- cargo コマンドは `mise run <task>` 経由。時刻演算は
  `checked_add`/`checked_sub`/`convert_exact` 等の checked API
  のみ。
- Automation 点列は `Project::control` のみが正本 — view/LOD は
  read-only projection で、series の複製・二次正本を作らない
  (LOD 出力は揮発 DTO であり永続化しない)。
- RT 経路(`record_event_rt`)は unbounded alloc/IPC/IO を行わない
  — `ruviform_rt::channel` の既存 RT 契約に載せ、BTreeMap 挿入・
  scope resolve・String 生成を push 経路に持ち込まない。
- 互換層・重複実装・原因不明の警告抑制・`#[allow]` による lint
  隠蔽を行わない(明示的な documented lossy `f64` 投影のみ既存
  規約どおり許容)。
- session transaction・health・notification は非永続 session 状態
  のまま(PC-C1 と同じ保証水準を超えない)。
