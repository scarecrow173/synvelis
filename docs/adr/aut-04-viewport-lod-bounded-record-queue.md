---
id: "ADR-034AUT04yQRFZlLO2ytJpt"
type: "adr"
status: "accepted"
title: "AUT-04 Viewport LOD・有界記録キューの表現 — model 層 LOD 投影・RT SPSC・型付き喪失 provenance"
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
  implements:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
  implemented-by:
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
    - "docs/plans/aut-04-automation-curve-lod-record-budget.md"
  depends-on:
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
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
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/ruviform/issues/257"
    - "https://github.com/scarecrow173/ruviform/issues/314"
  defers: []
  deferred-by: []
---

# AUT-04 Viewport LOD・有界記録キューの表現

## Status

Accepted — Issue #134 の 2026-11-04 合意(初期製品・横断依存正規化
v1.1)に基づく初期実装方針。

## Context

- Issue #134 は「記録点列と GUI 描画用 LOD の分離」「RT bounded
  queue・NonRT Write buffer・Failed/Overflow/Partial/Retry の
  型区別」を要求する。前提は #6 の非破壊 Source 契約(AUT-01..03
  で実装済み)と ruviform #257 の generic Viz API(実装済み)。
- 既存 `merged_series`/`lane_value` は `Project::control` の
  唯一の論理系列を評価するが、viewport 縮約の仕組みは無い。
  ruviform-viz の `WaveformLod` は PCM 波形の min/max/RMS 統計で、
  Step 補間・chase hold・region coverage・`BeatPoint` 正確位置の
  意味を持たない — automation 曲線の LOD は waveform 統計の
  転用では意味を保持できない。
- `record_write`/`record_points` は非 RT 呼出で、`resolve_scope_pos`
  ・`BTreeMap`/`Vec` 挿入を踏む — RT callback から直接呼べない。
  喪失を点列で埋める(捏造)のは契約違反。

## Decision

### D-1: LOD は model 層の read-only projection として `synvelis-project::control` に置く

- `automation_view(project, target, &CurveQuery) ->
  Result<AutomationCurveView, Error>` を `control.rs` に追加し、
  `merged_series` の one-logical-series 契約を共有する。session 層
  (workspace)は系列の merge を再実装しない — 同一ドメイン規則の
  重複実装を避け、正本の投影は正本の層に置く。
- 出力は**揮発 DTO**であり永続化しない。`ControlSession::
  automation_view` が revision トークン・pending segments・
  delivery span・記録 gap を重ねて返す(契約4: Inspector/
  SignalFlow/Lane は同一 `Project::control` の投影を共有)。

### D-2: Viewport query は scope 座標の lossy `f64` 窓 + exact envelope

- `CurveQuery { start, end, bins, max_points, max_features }`。
  `start`/`end` は target の `AutomationTimeScope` 座標を
  documented lossy `f64` 投影した表示窓(`lane_view`/`viz_ui` の
  `TimeRange` と同じ f64 規約)。project beat→scope 変換は
  `resolve_scope_pos` の既存機構に委ね、view は scope 座標で完結
  させる(ProjectMusical では両者一致)。
- bin は半開 `[s_i, s_{i+1})`(最終 bin は `end` を含む)。bin の
  `min`/`max` は**評価曲線の厳密値** — Linear 区間は端点で
  極値を取るため「bin 端の評価 + bin 内全点の評価 + bin 侵入
  値(chase)」で厳密。series 未到達 bin は `count=0`・`min=max=
  NaN`(viz の NaN-gap 規約に合わせる)。chase により最終点以降は
  値が保持されるため、covered でも `count=0` の「held」bin と
  未到達の「missing」bin を区別できる。
- `CurveFeature` は `BeatPoint` 正確位置を保持する(envelope は
  f64 表示座標、feature は正確座標 — precision 契約の分離)。
  kind: `StepEdge`(Step 補間の値不連続位置)・`Extremum`(方向
  反転の局所極値)・`RegionBoundary`(region range 端 — coverage
  縁)・`Anchor`(明示保持点 — view 内の全格納点が anchor 候補)。
- `view.points` は格納点を**verbatim**(pos/value)返す。
  `max_points` 超過時は feature 保持点(StepEdge/Extremum/
  Boundary 近接)を優先して残し、残 budget を先頭から順に anchor
  で埋める — 一律絶対誤差削減ではなく「形を運ぶ点」の優先保持。

### D-3: Quality は 3 値 — envelope はいかなる budget でも厳密

- `Exact`: view 内の全格納点が verbatim で返る + 全 feature 発行。
- `Reduced`: verbatim 点数を超過したが feature 欠落なし —
  envelope+feature で曲線形状は完全に保存される。
- `Truncated`: feature budget に達し StepEdge/Extremum/Boundary
  が落ちた — 表示が曲線形状を失い得る事実を型で示す。
- 「落としてよいのは shape-redundant な anchor のみ、かつ envelope
  がその値寄与を厳密に保持する場合」。feature 欠落は常に
  Truncated で表に出す — 視覚的に滑らかでも欠落を隠さない。

### D-4: RT ingress は session 内蔵 `ruviform_rt` SPSC、event に txn id を刻印

- `RECORD_EVENT_QUEUE_CAP` 固定容量の `Producer`/`Consumer` を
  `ControlSession` が保持する(headless では session が両端を
  保持 — 実 engine での producer 分割は Integration 責務)。
- `record_event_rt(&mut self, target, pos, value)` は push 側
  session 状態のみで gate 判定(`write_gates`・loop 境界・値
  検証 — 全て BTreeMap 読取/定数比較で alloc 0)し `try_push`。
  返り値は `Result<(), RecordPushError>`(`GatesClosed`/`Invalid`/
  `QueueFull`)— String を持たない Copy 型。
- event は `{txn, target, pos( project beat), value}`(Copy)。
  `record_start` が開いた txn id を push 時に刻印し、drain 時に
  `event.txn == 現在の open txn` を検査 — txn を跨いだ誤帰属を
  構造で防ぐ。

### D-5: 喪失は点列ではなく gap provenance — 集計は pre-reserved で alloc 0

- push 側 drop(QueueFull)は `RecordTargetHealth`(pre-reserved
  `Vec`、上限 `RECORD_HEALTH_CAP`)へ `dropped`/`queue_fulls`/
  `gap_open`/`gap_last` を更新するのみ — alloc なし。通知生成は
  drain 側(非 RT)に寄せる。
- drain 側は pop した各 event を `record_points` へ流す:
  - `event.txn` 不一致 → `stale`(旧 txn の残留 — 現 txn の喪失
    ではないため gap 不計上、Rejected 通知)。
  - gates closed / loop 境界 / 値不正 → `Rejected` + drop 計上。
  - `record_points` 成功 → `accepted`(非破壊、verbatim 格納)。
  - point budget 超過 → `RecordOverflow` + drop 計上。
- drop 計上は `gap_open`(最初の drop pos)→`gap_last` で喪失区間
  を開き、drain 境界で `tx.record_gaps[lane]`(project beats、
  `RecordGap{start,end,dropped}`)へ閉じる。隣接区間は併合、
  `RECORD_GAP_CAP` 超過は最後の gap を延長し `gaps_merged` を
  立てる。**喪失分の点列は一切生成しない**。
- drain 自体の rate budget(`max_events` per drain)超過は残
  event を queue に残して `Partial` — 次の drain で再送を試みる
  (retry は呼出側の明示再 drain)。

### D-6: 通知は bounded pull 型 log — 2 Window は独立 cursor

- `record_notifications: VecDeque<RecordNotification>`(cap
  `RECORD_NOTIFY_CAP`、超過は最古を落とし累計値で喪失を隠さない)。
  各 variant は累計カウンタを持ち、log 落ちても総量は観測可能。
- UI は `record_notifications()`/`record_health(target)`/
  `automation_view` を pull — 購読・push 通知は持たない。2 つの
  window が各自の cursor で同一 log を読む(消費されない)。

### D-7: 予算は定数のままではなく session 設定 — 既存既定値を保持

- `RECORD_POINT_BUDGET`(262144)は既存公開定数のまま既定値とし、
  `set_record_budgets(RecordBudgets { max_points, max_events_per_
  drain })` で session 毎に調整可能にする — 試験が小予算で
  Overflow/Partial を pin できるようにするため。queue 容量は
  構築時固定(`channel` 契約)で `RECORD_EVENT_QUEUE_CAP` 定数。

## Considered Options

- **Option A(採用)**: model 層 `automation_view` + session 組立
  (D-1〜D-3)。merged_series の正本契約を共有し、session 固有情報
  は組立層に隔離。
- **Option B(却下)**: ruviform-viz `WaveformLod` 転用。Step 補間・
  chase hold・`BeatPoint` 正確位置・region coverage の意味を
  失い、StepEdge/Anchor 保存を表現できない — generic 波形統計と
  automation 曲線意味は別契約。
- **Option C(却下)**: workspace 側で series を再 merge。公開
  `regions` から組立は可能だが、「one logical series」規則の
  重複実装になり正本境界を損なう。
- **Option D(却下)**: RT push で project を参照し scope resolve
  まで行う。`resolve_scope_pos` は project 参照+alloc 経路で RT
  不適 — push 側は session 状態のみで gate、resolve は drain に
  寄せる分割が正しい。
- **Option E(却下)**: drop 時に placeholder 点(線形補間等)を
  点列へ挿入。契約の「Queue loss を滑らかな曲線として捏造しない」
  に真っ向から反する。
- **Option F(却下)**: push 型 notification 購読。pull log +
  累計カウンタで 2 window を満たし、subscriber 管理・寿命・順序
  の複雑さを持ち込まない(既存 `writer_closes` と同規約)。

## Consequences

- `synvelis-project::control` に `CurveQuery`/`AutomationCurveView`/
  `CurveFeature`/`CurveFeatureKind`/`CurveQuality`/`ViewBin` と
  `automation_view` が増える — read-only projection で永続化・
  評価経路・schema に影響しない。
- `synvelis-workspace` が `ruviform-rt` へ依存する(workspace 依存
  として既に宣言済み、`Cargo.lock` は再解決で更新)。
- `RecordingTransaction` に `record_gaps`/`deliveries` が増える
  (session 内のみ、永続化 schema 無関係。fingerprint は
  payload = 点列+固定 context のまま変えない — delivery span は
  観測 provenance で永続 payload ではない)。
- `record_points` の budget 判定が `RECORD_POINT_BUDGET` 定数から
  `self.record_budgets.max_points` 参照へ変わる — 既定値は同一で
  既存挙動は不変。
- `record_event_rt` は `&mut self` 呼出(headless session 境界)。
  実 engine での真のスレッド分割は Integration 時に producer を
  移譲する設計 — 本件は semantic 契約と alloc 0 証拠を pin する。
