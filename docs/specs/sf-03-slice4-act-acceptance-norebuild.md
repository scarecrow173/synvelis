---
id: "SPEC-034UgK3ZuEty10uP1PNjT6"
type: "spec"
status: "approved"
title: "SF-03 slice4 — SF-ACT-05〜09 系受入消化・NoRebuild Equivalence・世代連動 dispatch（player/project 境界）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
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
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sf-03-slice3-plugin-host-connect-design.md"
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
  refines:
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  derived-by:
    - "docs/designs/sf-03-slice4-act-acceptance-design.md"
    - "docs/plans/sf-03-slice4-act-acceptance.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice4-act-acceptance.md"
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
  defers: []
  deferred-by: []
---

# SF-03 slice4 — SF-ACT-05〜09 系受入消化・NoRebuild Equivalence・世代連動 dispatch（player/project 境界）

## 目的

Issue #106 の SF-03 を slice 分割して進める本 dispatch は **slice 4 のみ** を
対象とする。slice 1（PR #191）で確立した semantic lowering・
`RoutingSourceMap`・`RoutingBuildKey`・activation receipt・last-good、
slice 2（PR #192）で確立した `RoutingFault` 18 分類・publish gate・
`reconcile`、slice 3（PR #195）で確立した `ProcessorHost` boundary・
generation-scoped lease・世代連動 teardown を土台に、SF-ACT-05〜09 系の
うち **player/project 境界内で headless 契約として検証できる受入項目**
を消化する。

具体的には次の 3 系統を扱う。

1. **NoRebuild Equivalence（SF-ACT-09 の headless 部分）** — Project
   名・ViewState のみの変更や undo/redo で routing fingerprint が
   同値のとき、再 compile を省略し `ExecutionEquivalence` を別証拠と
   して返す経路。新 Revision が実 RT Active になったという偽 receipt
   は出さない。
2. **世代連動 dispatch（SF-ACT-06 の headless 部分）** — 接続済み
   chain processor の非 RT 制御半体（`PluginNodeControl`）を世代
   スコープで engine が保持・払い出し、`ParamDispatch` が旧世代の
   plugin instance へ誤配送しないことを headless で検証する。
3. **SF-ACT-05 の headless 残件** — rapid R1/R2/R3・superseded
   prepare/publish・queue-full/partial stale command・reconcile
   drift の既存契約を `sf03_l4.rs` で固定し、未カバーの隙間があれば
   埋める。

SF-ACT-07 の deterministic failure/fail-closed は slice 1/2 の
`skipped_fault` 昇格で既に両 mode fail-closed が確立済みであり、本
slice はその契約を明示的に再検証するに留める（preview 許容 policy の
分岐は後続送り — 範囲外参照）。SF-ACT-08 は ruviform RT/synvelis-render/
AudioLens 側の責務が主であり、本境界での追加実装は行わず評価結果を
記録する。

## 前提（slice 1〜3 からの継承契約）

- `RoutingBuildKey` は project id/revision・routing fingerprint・
  capability generation・engine/source-format generation・lowering
  policy version・build mode・device clock epoch・runtime instance を
  束ね、`semantic_eq` は `project_revision` のみを除く同値比較を
  提供する（slice 1/2 契約・変更しない）。
- `Published` は非 RT 証拠、`Active` は RT が新世代を非零 frame で
  観測した場合のみ。`last_good_generation` は最後に RT 観測された
  Active 世代（slice 1/2 契約・変更しない）。
- `ProcessorHost::connect` は `(GraphProcessor, ProcessorLease)` を
  返し、lease は drop で best-effort teardown を実行する。接続失敗は
  `ProcessorConnectError::fault()` で既存 18 分類へ決定的に写像する
  （slice 3 契約・変更しない）。
- `RoutingFault` は Issue #106 §5 の 18 区分を維持し、新 variant を
  追加しない。
- runtime `NodeId`/`PortId`・`PluginHandle`・host instance 世代は
  graph-local の runtime-scoped 値であり Project に永続化しない。
- `ParamDispatch`（PC-B、#43）は `PluginNodeControl` を所有して
  generation-tagged `TransportPlan` を node へ stage し、ledger で
  `Scheduled→Dispatched/Dropped` を観測する既存契約を持つ。

## 要件

### R1. NoRebuild Equivalence（SF-ACT-09）

`RoutingEngine::prepare` は build 前に、最後に publish された plan の
`RoutingBuildKey` と今回の request から計算した key を `semantic_eq`
で比較し、次を満たす場合に **compile を行わず** equivalence を返す。

- 最後に publish された plan が存在する（`source_map` がある）。
- `live_key.semantic_eq(&new_key)` が真 — すなわち
  `project_revision` 以外の全 field（project id・routing
  fingerprint・capability/engine/source-format generation・lowering
  policy version・build mode・device clock epoch・runtime instance）
  が一致する。
- `live_key.project_revision != new_key.project_revision` — **同一
  revision の再 prepare は常に通常 build を行う**。同一 revision の
  再 build は runtime 状態の明示的な再構築経路として残す
  （既存 test 契約・l2_09 の連続 publish を含む）。

- `prepare` の戻り値を `RoutingPrepare` enum とし、
  `Built(PreparedRouting)` / `Equivalent(ExecutionEquivalence)` を
  区別する。`Equivalent` は publish するものを持たず、receipt も
  更新しない — live plan の状態記述（`Active`/`Published` 等）を
  そのまま維持する。新 Revision が Active になったという偽証拠は
  出さない。
- `ExecutionEquivalence` は synvelis-project の契約型として、
  `configured_project_revision`（今回 configure された revision）・
  `active_built_from_revision`（live plan が実際に build された
  revision）・`matching_semantic_build_key`（semantic 一致した live
  plan の build key）・`plan_generation`（live plan の世代）を保持
  する。
- equivalence 判定は `checked_key`（expected revision・runtime
  instance/epoch・resource budget の検査）の**後**に行う — stale
  candidate・誤 epoch・budget 超過は equivalence でも拒否される。
- equivalence 時は generation を予約せず、lowering・host connect・
  compile のいずれも実行しない（`planned_generation`・host connect
  回数・receipt が不変であることを test で固定する）。
- `reconcile` は「revision のみが drift し semantic 内容が同値」
  の場合を drift とみなさない — live state から再構成した key が
  publish 済み key と `semantic_eq` なら `ProjectRevision`
  mismatch を報告しない。semantic が実際に drift した場合は従来
  どおり `StaleProjectSnapshot`（優先順位先頭側）を報告する。
- undo/redo は新 revision を発行するため、semantic 内容が
  publish 済み plan と同値なら equivalence、異なれば通常 build
  となる — fingerprint 判定のみで無駄な compile を防ぐ。

### R2. 世代連動 processor control 払い出し（SF-ACT-06 の headless 部分）

chain processor 接続の非 RT 制御半体を世代スコープで engine が保持
し、公開済み世代に限り払い出す。

- `ProcessorConnection` に `control: Option<PluginNodeControl>` を
  追加する。`PluginHostProcessors` は `attach` が返す control を
  ここへ移し、lease の teardown closure は host lifecycle
  （StopProcessing→Deactivate→Destroy）のみを担う。control の
  drop は generation ledger の寿命に従う。
  `StaticProcessorHost` は `None` を返す。
- engine は publish 成功時に接続ごとの `{ProcessorId, NodeId,
  ProcessorLease, Option<PluginNodeControl>}` を世代別 ledger
  （既存 `connections` map の値型を拡張）へ移す。既存の世代連動
  teardown（pending 置換・Active 観測・engine drop）の規則は
  変更しない — untaken control も lease と同時に drop される。
- `RoutingEngine::take_processor_control(generation,
  processor_id)` を追加し、公開済み世代の chain processor に
  ついて one-shot で `ProcessorControl`（`NodeId`・
  `PluginNodeControl`・attach 時と同一の `frame_base`）を返す。
  `StaticProcessorHost` 接続・未知の世代/processor・既に払い出し
  済みの場合は `None`。`frame_base` は `PluginNodeControl` が
  accessor を持たず `ParamDispatch::new` が attach 時と同一値を
  要求するため、engine が attach 時に使った値をそのまま同梱する。
- 払い出した control はその世代の plugin instance 専用の channel
  を持ち、他世代の plugin へ配送される経路は存在しない
  （wrong-plugin 0 は接続構造で保証）。払い出し後に世代が退役
  しても control 自体は caller 所有のまま残り、死んだ node の
  bounded channel への push は `QueueFull` として型付き拒否される
  か配送先のない buffer に留まる — 別世代・別 plugin への転送は
  起きない。
- `ParamDispatch` への接続は caller（test/app 層）が行う —
  engine は control の払い出しのみを責務とし、bind/dispatch/
  observe の既存契約は変更しない。

### R3. SF-ACT-05 headless 残件の固定

既存契約の headless 検証可能な部分を `sf03_l4.rs` で固定する。

- rapid R1/R2/R3: 連続 prepare→publish で各世代の receipt が
  `Published` を記録し、RT 未観測の pending 世代が次の publish で
  置き換わること、`last_good_generation` が RT 観測まで後退しない
  こと。
- superseded: 既 publish 世代以下の generation を持つ prepared
  plan の publish は `Superseded` で拒否され旧 plan を維持する。
- queue-full/partial stale command: publish 連打による
  `PublishQueueFull`（last-good 維持）と、未活性化世代の install
  command が RT 側で stale discard され `stale_commands`/
  `stale_events` counter が前進、`reconcile` が `EventStale` を
  報告すること（l2_08/l2_09 の契約を slice4 test からも再確認
  する範囲で重複は避ける）。
- device epoch/runtime instance 不一致の request は
  `DeviceEpochChanged` で拒否される（既存契約の再確認）。
- retry は常に fresh prepare から始まり、失敗した `PreparedRouting`
  の再利用・旧 generation/source map の再利用を行わない
  （slice2 R3 契約の再確認を含む）。

### R4. SF-ACT-07/08 の評価記録

- SF-ACT-07: Missing Plugin Port/Audio Source/Unsupported mapping
  の deterministic failure と fake silence 禁止は slice1/2 の
  `skipped_fault` 昇格で確立済み。「Partial Preview vs Export
  fail-closed」の mode 別 policy 分岐は未規定のため本 slice では
  実装せず、現行の両 mode fail-closed を維持する判断を設計文書に
  記録する。
- SF-ACT-08: Ruviform RT retired processor の single-owner/noalloc/
  fault counters は upstream の rt_ contract（`mise run test-rt`）
  の責務、Offline Render/PDC/Codec/Format は synvelis-render 側、
  AudioLens は別 consumer — player/project 境界での追加実装なし。
  `reconcile` が retire/stale counter を surface する既存経路を
  受入 evidence として記録する。

## 受入条件

- [ ] `prepare` が `RoutingPrepare::{Built, Equivalent}` を返し、
      Project 名変更・ViewState 相当の変更・undo/redo で semantic
      同値の新 revision に対し compile/host connect/generation
      消費を行わず `ExecutionEquivalence` を返す test が green。
- [ ] 同一 revision の再 prepare・engine/source-format/capability
      generation や build mode/epoch/instance が異なる request は
      equivalence にならず通常 build される test が green。
- [ ] `reconcile` が revision のみの drift（semantic 同値）を
      fault とせず、semantic drift 時は従来どおり報告する test が
      green。
- [ ] `take_processor_control` が公開済み世代の chain processor
      control を one-shot で払い出し、実 `PluginHost` 接続の
      param event が正しい世代の plugin instance にのみ配送され、
      世代交代後の旧 control が新世代 plugin へ届かない
      （wrong-plugin 0）test が green。
- [ ] `sf03_l4.rs`（新規）を含む `mise run test` が green。
- [ ] `mise run check`/`lint`/`fmt-check`/`test`/`test-rt`/
      `check-headless-deps` が全て exit 0。
- [ ] spec → design → plan → task → test-spec の文書チェーンが
      `docs/` に存在し、doc-status audit の blocker がない。

## 範囲外（後続 slice / 別 Issue）

- GUI 表示・`synvelis-app`/`session.rs`/`EngineStub` の置き換え・
  app 層への equivalence/control 配線（本 slice は headless 契約
  のみ。戻り値の消費側は後続）。
- SF-ACT-05 の実機部分: RT latency/underrun・実 device epoch
  restart・実 OS/実デバイス検証。
- SF-ACT-06 の残り: automation/control plan → `TimedParamEvent`
  lowering の engine 配線、Fast Control の active-generation gate
  （Blocked/有界 Queue 方針）、GUI fast-control、L1 plugin での
  実機検証。
- SF-ACT-07 の preview 許容 policy 分岐（Partial Preview vs Export
  fail-closed を分ける明示 policy が未規定）。
- SF-ACT-08 の実装: ruviform RT/scheduler/render 側の責務、
  AudioLens consumer 連携。
- `capability_revision`/`version` の port 単位深い照合（slice3 から
  の継続 defer）。
- legacy `lower()`/`lower_with_inserts`/`HeadlessDriver` の変更
  （`RoutingEngine` 経路のみ）。
