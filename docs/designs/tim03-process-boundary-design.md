---
id: "DESIGN-034UyEjdALaapQRSk5hAdk"
type: "design"
status: "approved"
title: "TIM-03 plugin process boundary design — capability/degraded policy oracle"
created: "2026-11-09"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/ruviform/issues/307"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim03-process-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
  defers: []
  deferred-by: []
---

# TIM-03 plugin process boundary design — capability/degraded policy oracle

## Context

Ruviform（pin `0f5b573`）は process 境界の generic 機構を既に所有する:
`BlockContract`（min/max process frames・in-block transport support）、
`CallPartition`（span → legal call 列）、`TransportScheduler`
（`DeliveryPolicy::Exact`/`Degraded`、bounded `find_legal_boundary`、
`DeferredApply` receipt、undeliverable span + `ScheduleFault`）、
`PluginNode`/`GraphRuntime`（物理 call・generation 照合・bounded
`NodeReport`）。Synvelis 側に不足しているのは consumer 固有の
**precision/degraded/offline ポリシー判定**であり、それを
`synvelis-player` に閉じた非 RT oracle として実装する。

境界の具体例: 256f callback・boundary offset 100・plugin
`min_frames=128` では `[0,100)` が illegal であり、100f の物理
process call を発行してはならない。Exact では typed reject、
consent 付き degraded では合法 frame への defer、Offline Exact
では fail-closed error、という Synvelis ポリシーを oracle が
事前に決定する。実際の RT 配送は引き続き Ruviform scheduler/
`PluginNode` が行い、本 oracle はその入力となる plan を返す。

## Scope

### In Scope

- `process_boundary.rs`: `TimingPrecision`/`ExecutionMode`/
  `ProcessCapability`/`DegradedConsent`/`BoundaryKind`/
  `BoundaryPoint`/`ProcessBoundaryRequest`/`SegmentPlan`/
  `BoundaryDegradation`/`ProcessBoundaryPlan`/
  `ProcessBoundaryError` と `ProcessCapability::evaluate`。
- oracle の境界 simulation が upstream `run_callback` の
  boundary 解決順序（head apply → pending apply vs wrap の
  tie は apply 優先 → wrap 再評価 → deferral scan）と一致する
  ことの設計保証と test 証拠。
- 3 層 test: evaluator unit、実 `TransportScheduler` span、
  実 `PluginNode`/`GraphRuntime` 物理 call + `stats_alloc` RT 証拠。

### Out of Scope

- oracle の RT 経路組込み（boundary offset 列を伴う `evaluate`
  の RT callback 内呼出し）。attach 時点の `AttachDelivery` 投影は
  task2（`tim03-task2-attach-wiring`）、per-request 配線は task3
  （`tim03-task3-request-plan` の `ProcessBoundarySession`）で
  実装済み — session は orchestration 層の API であり、RT
  callback 内では oracle を評価しない。
- Ruviform 型の変更・独自 scheduler の再実装。
- 実 plugin/OS matrix・AudioLens・GUI（spec の Out of Scope と同一）。

## Components and Boundaries

```
ProcessBoundaryRequest ──► ProcessCapability::evaluate ──► ProcessBoundaryPlan
  (window, boundary offsets,    (non-RT oracle)               (policy, precision,
   mode, consent, budget)                                    segments+CallPartition,
                                                            degradations)
        │  typed ProcessBoundaryError: InvalidRequest /
        │  Unsupported / ConsentRequired / OfflineExact /
        │  OfflineUncertified / DeferralExceeded
        ▼
TransportScheduler::request/install_plan (DeliveryPolicy,
ScheduleBudget は plan から導出) ──► PluginNode::process ──► plugin
```

- **所有権**: Synvelis は precision 分類・consent 要求・offline
  fail-closed・degradation 記録を所有。Ruviform は span 分割・
  deferral scan・generation 照合・物理 call legality を所有。
- **非 RT 保証**: `evaluate` は `Vec` 確保を伴う preflight であり
  RT path から呼ばない。RT 側の no-alloc は `PluginNode`/`CallSink`
  が担い `test-rt` で実測。

## Data and Control Flow

1. 呼出側が activated `ProcessConfig`（`sample_rate`/`min_frames`/
   `max_frames`）と in-block support・certified precision から
   `ProcessCapability` を構成（`ProcessCapability::new` または
   `unmeasured`）。
2. `ProcessBoundaryRequest` に window 長・`BoundaryPoint` 列
   （offset は window 内 0..=window、0 は head apply として
   cut から除外、同一 offset は kind 決定的順で coalesce）・
   `ExecutionMode`・optional `DegradedConsent`・`ScheduleBudget`
   を詰める。
3. `evaluate` が offset を正規化・検証し、cut 列から segment を
   列挙。各 segment について `legal_span_len`（`max_frames==0`
   の degenerate contract は事前に `Unsupported` で拒否 —
   upstream `legal_span_len` は `max_frames==0` で除算 panic
   し得るため oracle 側で guard）で legality を判定し、
   `CallPartition` を構成。
4. illegal segment がある場合:
   - `Unsupported` precision → `Unsupported`。
   - `OfflineExact` → `OfflineExact{boundary,len}`（先頭の
     illegal segment）。
   - `Realtime` + consent なし → `ConsentRequired{boundaries}`
     （全 illegal segment start を列挙）。
   - `Realtime` + consent あり → upstream `find_legal_boundary`
     相当の scan（`prev`=直前 applied boundary、`next_bound`=
     次の pending non-wrap cut または window end — wrap は
     position-driven で scan を bound しない）で
     `min(consent.max_deferral, budget.max_deferral)` 内の合法
     boundary を探索。発見時は `BoundaryDegradation` を記録し
     `DeliveryPolicy::Degraded` の plan を返す。見つからなければ
     `DeferralExceeded{boundary}`。
5. legal のみ → `DeliveryPolicy::Exact`。`OfflineExact` はさらに
   certified exact-class（`ExactWithinVerifiedCapability`/
   `FrameAccurateEvents`）を要求し、未認定なら
   `OfflineUncertified{precision}`。
6. plan の `precision` は「実際に配送された」精度を報告:
   degraded 適用時は `DegradedWithConsent`、exact は
   `in_block==Verified` なら `ExactWithinVerifiedCapability`、
   それ以外は `FrameAccurateEvents`。capability の certified
   precision は `Unknown` のまま昇格しない。

## Non-Functional Constraints

- **Fail-closed**: 未対応・未認定・defer 不能は全て typed error。
  illegal call を黙って quantize/縮退しない。
- **決定性**: 同一入力に同一 plan。boundary の coalesce/deferral
  は upstream scheduler の境界解決と同じ規則に従う。
- **RT 分離**: oracle は non-RT。RT hot path
  （`input_writer`/`rt.process`/`PluginNode`/`CallSink`/plugin
  `process`）は upstream RT contract に従い、`stats_alloc` で
  allocations=deallocations=0 を実測。
- **generation 安全性**: oracle は generation を直接扱わないが、
  event 配送の generation 照合は upstream `event_engine_frame`/
  `SpanSink` が担い、stale tag は `StaleEventGeneration` fault +
  非配送であることを test で固定。

## Risks and Trade-offs

- **oracle と scheduler の semantics 乖離**: deferral scan の
  `next_bound` を「全 cut」と誤ると wrap で過剰に bound される。
  upstream は pending apply のみを bound に使うため、本実装も
  non-wrap cut のみで `next_bound` を計算し、test
  `tim03_pb_degraded_coalescing_and_wrap_bounds` で固定。
- **consent の粒度**: `DegradedConsent` は per-request の
  `max_deferral` 上限を持つ token であり、plugin 永続の能力
  ではない。広すぎる deferral は `DeferralExceeded` で拒否。
- **Offline Exact の厳しさ**: certified exact-class を要求する
  ため `Unknown`/`BlockBoundaryOnly`/`DegradedWithConsent` は
  offline exact を通らない — 仕様どおりの fail-closed であり、
  通したい場合は capability 認定を別途行う。

## Attach-time projection（task2 で実装）

`ProcessCapability::attach_delivery(mode, consent, budget)` が
oracle の attach 時点投影を担う: activated `ProcessConfig` から
`from_process_config` で contract を導出し、`ExecutionMode`・
optional `DegradedConsent`・`ScheduleBudget` を Ruviform
`AttachConfig::{policy,budget}` へ写像する。

- Realtime: 無同意 → `DeliveryPolicy::Exact`。同意あり →
  `DeliveryPolicy::Degraded` で deferral 上限を
  `min(consent.max_deferral, budget.max_deferral)` に clamp。
- `OfflineExact`: 常に `Exact` を要求し、certified exact-class
  （`ExactWithinVerifiedCapability`/`FrameAccurateEvents`）以外を
  `OfflineUncertified` で拒否。consent は offline を救済しない。
- `Unsupported` capability・degenerate contract は `Unsupported`
  で事前拒否。

`PluginHostProcessors` は `BoundaryDeclaration`（precision/
in-block/consent/budget）を `ProcessorId` 単位で保持し、
`connect` が `Instantiate` 前に投影を評価する。失敗は
`ProcessorConnectError::{CapabilityUnsupported,
OfflineUncertified}` → `RoutingFault::{PluginUnavailable,
OfflineCapabilityMismatch}` に決定的写像され、plugin は
instantiate されない。宣言なし processor は従来どおり
`Exact` + `ScheduleBudget::default()`。

per-request の boundary legality/deferral 判定は
`ProcessBoundarySession`（task3）経由で `evaluate`（non-RT
preflight）と upstream `TransportScheduler`/`PluginNode` が担い、
attach 投影は「その processor が取り得る delivery 精度の上限」を
固定するだけである。

## Per-request session + precision provenance（task3 で実装）

`ProcessBoundarySession` は 1 つの attached `PluginNodeControl`
に対する orchestration 層 API。consumer 側の精度 provenance を
pin し、request ごとの evaluate/stage/dispatch を仲介する。

- **`PrecisionProvenance`**: `StablePluginInstanceId` + `NodeId` +
  capability generation + graph generation の 4 要素 pin。
  caller が `PrecisionProvenance::new` で mint し、session 生成時に
  固定、各 request で caller が渡す現行 provenance と照合する。
  `PluginBindingSession`（PC-C4）の stale 拒否パターンを踏襲。
  transport/plan generation は event-scoped な upstream scheduler
  所有軸であり、`ParamDispatch` の receipt-gated observation が
  既に fault するためここでは複製しない（設計判断を明記）。
- **`ProcessBoundarySession::open`**: `control` から
  `AttachClockContext::of` で verified clock context を取得し、
  `attach_delivery` を再評価 — `Unsupported`/`OfflineUncertified`
  は session 生成前に fail-closed（`connect` と同じ typed
  refusal）。`mode`/`consent`/`budget` は session に固定され、
  全 `plan_window` が同一入力で評価されるため oracle と
  scheduler の delivery policy が乖離しない。
- **request API**（全て provenance 照合を staging/install の前に
  実行、mismatch は typed refusal で何も残さない）:
  - `plan_window(current, window_frames, boundaries)` — 純粋な
    evaluate。window 内 offset を持つ `BoundaryPoint` 列
    （caller 供給の loop wrap 等 + `stage_window`/`stage_plan`
    が command/install 由来で追加する Seek/Other/PlanInstall）を
    session 固定の mode/consent/budget で評価し `ProcessBoundaryPlan`
    を返す。staging しない。
  - `stage_window(control, current, window_frames, boundaries,
    commands)` — `plan_window` 成功後に `(TransportCommand,
    due_frames)` を順に `control.request` へ push。window 内 due の
    command は境界として legality 検査済み。queue full は残りを
    abort（先行分は plan でカバー済み — upstream bounded-queue
    契約）。
  - `stage_plan(control, current, window_frames, boundaries, plan,
    due_frames)` — `install_plan` 版。window 内 due の install は
    `plan_generation` bump・span 分割・note release/chase を伴う
    実 discontinuity として `PlanInstall` 境界で検査。
  - `dispatch_params`/`dispatch_midi(dispatch, current, …)` —
    provenance 照合 + `dispatch.node()` が pin node と一致 +
    `check_frame_base(dispatch.frame_base())`（verified attach
    clock context、Ruviform #307）を通してから `ParamDispatch`
    の generation-tagged plan minting/ledger/receipt 観測へ委譲。
    foreign node や stale clock mapping の `ParamDispatch` は
    write 解決前に typed refusal。
- **`BoundarySessionError`**: `Capability`/`Evaluate`（upstream
  `ProcessBoundaryError` を保持）・`InstanceMismatch`/
  `NodeMismatch`/`CapabilityGeneration`/`GraphGeneration`/
  `FrameBaseMismatch`・`QueueFull`/`InstallPlan`/`Dispatch`
  （upstream typed error を潰さず伝播）。
- **head install の扱い**: param/MIDI dispatch は callback head
  （due=0、offset-0 apply）に install されるため span を分割せず
  window 評価は不要 — window legality は `plan_window` 系が担う
  という責務分離を test で固定。

provenance の照合は全て orchestration 層（呼出側 thread）で行い、
RT callback・plugin `process` には一切触れない。generation
照合の最終防衛は引き続き upstream `TransportScheduler`/
`SpanSink`/`event_engine_frame` が担う。

## Deferred Design Concerns

- ~~oracle → `AttachConfig`/`PluginNodeControl` への plan 反映
  （`DeliveryPolicy`/`ScheduleBudget`/`DegradedConsent` →
  consented degraded の wiring）~~ — task2
  （`tim03-task2-attach-wiring`）で `AttachDelivery` 投影、
  task3（`tim03-task3-request-plan`）で `ProcessBoundarySession`
  として実装済み。
- `TimingPrecision` の capability 認定 provenance（どの計測が
  `ExactWithinVerifiedCapability` を正当化するか）の定義 —
  task3 の `PrecisionProvenance` は配送文脈の generation pin で
  あり、精度クラス自体の認定根拠ではない。認定 provenance は
  実機 gate と併せて後続。
- boundary offset の Tempo map/Loop occurrence 由来の自動導出
  （本 slice は caller が offset を与える）。

## Verification Notes

- `tim03_pb_*`: evaluator の window/boundary/min-max/consent/
  offline/precision/invalid/coalescing 網羅。
- `tim03_sched_*`: 実 `TransportScheduler` で Exact reject・
  undeliverable wrap・`DeferredApply`・同時境界・event
  exactly-once・stale generation fault を固定。
- `tim03_node_*`: 実 `PluginNode`/`GraphRuntime` で物理 call
  length legality・deferral・per-occurrence event・stale 世代
  非配送を固定。
- `tim03_process_boundary_rt.rs`: `stats_alloc` Region で
  64 callback 分の hot path allocations=deallocations=0。
- `tim03_aw_*`（task2）: `attach_delivery` 投影の mode/consent/
  budget clamp・offline certified gate・typed refusal と、
  `connect` の instantiate 前 fail-closed（fake `PluginHost` +
  instance counter）・宣言 scoping・undeclared 後方互換。
- `tim03_req_*`/`tim03_cap_*`/`tim03_prov_*`（task3）:
  session open の fail-closed・window/boundary 行列・同時
  loop/seek/tempo/plan-install/param・min/max legality・
  capability 3 分岐・stale provenance 全軸・foreign dispatch・
  誤配送 0 を実 `PluginNode`/`ParamDispatch` fixture で固定。
