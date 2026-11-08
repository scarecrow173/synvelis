---
id: "SPEC-034UfvosLz4uB6QhaNTi2s"
type: "spec"
status: "approved"
title: "SF-03 slice3 — 実 plugin host 接続（headless 契約・SF-ACT-05 系）"
created: "2026-11-08"
updated: "2026-11-08"
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
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  refines:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  derived-by:
    - "docs/designs/sf-03-slice3-plugin-host-connect-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice3-plugin-host-connect.md"
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
  defers: []
  deferred-by: []
---

# SF-03 slice3 — 実 plugin host 接続（headless 契約・SF-ACT-05 系）

## 目的

Issue #106 の SF-03 を slice 分割して進める本 dispatch は **slice 3 のみ** を対象とする。
slice 1（PR #191）で確立した semantic lowering・`RoutingSourceMap`・
`RoutingBuildKey`・activation receipt・last-good、slice 2（PR #192）で
確立した `RoutingFault` 18 分類・publish gate・`reconcile` を土台に、
lowered typed graph の chain processor node を **実 plugin host 接続経路**
（instantiate → initialize → activate → start-processing → endpoint attach）
へ接続する headless 契約を実装する。SF-ACT-05 系のうち、generation 連動する
plugin instance の接続・切断・失敗 fault・last-good 維持を対象とし、
GUI 表示・`synvelis-app`/`session.rs`/`EngineStub`・NoRebuild 最適化・
実 OS/実デバイス検証は後続 slice に残す。

## 前提（slice 1/2 からの継承契約）

- `RoutingBuildKey`/`RoutingSourceMap`/generation 予約・
  `Configured→Prepared→Published→Active` receipt・last-good・
  stale publish 拒否・reconcile は slice 1/2 の契約を変更しない。
- `RoutingFault` は Issue #106 §5 の 18 区分を維持し、新 variant を
  追加しない。host 接続失敗は既存区分へ決定的に写像する。
- runtime `NodeId`/`PortId`・`PluginHandle`・host instance 世代は
  graph-local の runtime-scoped 値であり Project に永続化しない。
  Project が保持するのは `ProcessorBinding::Resolved(PluginRef)` の
  semantic identity（descriptor・version・capability_revision）と
  `PortBinding::PluginDeclared` の宣言済み bus 情報のみ。
- chain processor node は lowering で必ず emit され、instance が
  無ければ fail closed（`PluginUnavailable`）— silent bypass・
  fake silence は禁止（slice 1 契約の維持）。
- `GraphProcessor` は `Send`・RT-safe（`process` は bounded・
  nonallocating・nonblocking）。host 側の lifecycle 操作
  （instantiate/activate/deactivate/destroy）は全て非 RT で行い、
  RT に出るのは detach 済み endpoint を内包する node のみ。
- `PluginHost`・生の plugin object・child handle・`PluginNodeControl`
  は RT graph processor に入れない。endpoint の退役・破棄は plan
  retirement/collector 経由の非 RT 側責務。

## 要件

### R1. Processor host 接続 boundary

chain processor の接続を担う注入 boundary を `synvelis-player` に
定義する。境界は「semantic `Processor` + 解決済み semantic port 一覧 +
build 世代 + engine 形式」から「RT legs（`GraphProcessor`）+
非 RT lease」を返す契約とし、呼び出し側（`RoutingEngine`）は
plugin host 内部を知らなくてよい。

- boundary は `connect` を提供し、成功時に RT 用 `GraphProcessor`
  と非 RT teardown を内包する lease を返す。
- lease は drop 時に host 側 teardown（stop-processing → deactivate →
  destroy の best-effort 順序）を自己完結で実行する。publish 成功後は
  engine が世代ごとに lease を保持し、世代の置き換え・Active 観測・
  engine 破棄で drop する。`PreparedRouting` が publish されずに
  捨てられた場合も lease drop で接続が解放される。
- `ProcessorBinding::Resolved(PluginRef)` の `descriptor` を
  plugin instance identity として host の instantiate に使う。
  `Unresolved` binding は接続不可（`PluginUnavailable`）。
- semantic port 一覧（`processor.port_ids` 順）から bus 宣言を導出し、
  `ruviform-audio-plugin-bridge` の `attach` 契約
  （declared bus table の validate → host bus report 照合 →
  endpoint mint → `PluginNode` + `PluginNodeControl` 生成）を使う。
  bus 導出・照合・port index 対応を synvelis 側で再実装しない。
- `ProcessConfig`・`EndpointBounds`・`frame_base`・delivery policy は
  engine 形式（sample rate・channel layout・block frames）から決定的に
  導出する。`input`（live MIDI leg）は本 slice では無効
  （`input: None`）。
- headless fake/stub boundary（`StaticProcessorHost` 相当）を提供し、
  実 host が無い環境でも接続契約（順序・失敗時 fault・generation
  連動）を検証できる。

### R2. 実 `ruviform-plugin-host` 接続 adapter

`PluginHost` を所有する adapter を提供し、R1 の boundary を実 host
lifecycle で実装する。

- connect は `Instantiate` → `Initialize` → `Activate(ProcessConfig)`
  → `StartProcessing` → `attach` の順で host API を駆動する。
  途中失敗時は best-effort で destroy まで戻し、fault を返す。
- host の bus report が declared table と矛盾する場合
  （direction/role/layout の per-direction 不一致・port 欠落・
  bounds 不適合）は `UnsupportedFormatOrTimeMap` 系 fault で
  fail closed する。未検証（L1 `Unavailable` report）は bridge 契約
  どおり unverified として扱う。
- `capability_revision` の host 側照合は bus report 照合に含まれる
  範囲で行い、port 単位の `PluginDeclared.capability_revision` と
  host 公開値の深い照合は後続 slice（SF-ACT-06 系）の責務とする。
- adapter は `PluginHost` を `Rc<RefCell<…>>` で共有し、lease の
  teardown closure が host を借用して lifecycle command を発行する。
  `PluginHost`/生 plugin object は `GraphProcessor` に入らない。

### R3. Engine 統合と generation 連動

`RoutingEngine` は chain processor node ごとに boundary の `connect`
を呼び、返された `GraphProcessor` を compile factory へ供給する。

- `RoutingDeps` の `processors` map（呼び出し側が instance を直接
  注入する経路）は廃止し、host boundary に置き換える。既存の
  「instance 未供給 → compile `PluginUnavailable`」契約は
  「connect 失敗 → `PluginUnavailable`」に移る。
- 接続は `stage_build` 内・非 RT で行い、失敗時はそれまでの接続を
  lease drop で解放してから typed fault を返す。receipt は
  `Failed{fault, last_good_generation}` を記録し、last-good plan は
  維持される。
- publish 成功時、当該 generation の lease 群を engine の
  generation 別 ledger に移す。直前の published generation が
  未 Active（pending のまま置き換え）ならその lease を即時解放する。
  Active 観測（`observe_activation`）で新世代の Active が確定した
  時点で、それ以外の世代の lease を全て解放する（旧世代の plugin
  instance は新世代 Active 後に deactivate/destroy）。
- `BuildMode::Offline` の capability gate は接続済み instance の
  `offline_support()` を見る（`PluginNode` は `Unknown` を報告する
  ため、実 host plugin は offline build で `OfflineCapabilityMismatch`
  となる — fail closed を維持し、verified 宣言の扱いは後続 slice）。
- rapid publish（R1/R2/R3 相当の連続 prepare→publish）で、各世代の
  plugin instance 接続・切断が generation に連動することを
  stub/実 host 双方で検証する。

### R4. Fault 写像（18 分類内での決定的割当）

host 接続失敗は新 variant を作らず既存区分へ写像する。

| 失敗 | `RoutingFault` |
| --- | --- |
| binding `Unresolved`・plugin id 未登録・instance 枯渇・endpoint 能力なし | `PluginUnavailable` |
| Initialize/Activate/StartProcessing の host/lifecycle 拒否 | `PluginUnavailable` |
| bus table 不一致・port 欠落・bounds/format 不適合 | `UnsupportedFormatOrTimeMap` |
| request 内部不整合（port 解決不能など到達不能な防御経路） | `InvalidRouting` |

- `OfflineCapabilityMismatch` は接続後の `offline_support()` gate
  （slice 2 契約）で mint される。host 接続自体は offline/realtime
  で同一手順。

## 受入条件

- [ ] lowered typed graph の chain processor node が注入 boundary
      経由で実 `PluginHost` 接続経路（instantiate/activate/
      start-processing/attach）に接続され、graph 出力に実際の
      plugin 処理結果が現れる headless テストが green。
- [ ] 実 host が無い環境で stub boundary を使い、接続契約
      （connect 順序・失敗時 fault・generation 連動 teardown）を
      検証するテストが green。
- [ ] host 接続失敗（未解決 binding・未知 plugin id・bus 不一致・
      lifecycle 拒否・instance 枯渇）が対応する `RoutingFault` で
      mint され、receipt `Failed{…, last_good_generation}` で
      last-good plan が維持されることをテストで確立する。
- [ ] generation 連動: publish 成功世代の lease 保持、pending 世代の
      置き換え時解放、Active 観測時の旧世代解放、未 publish
      `PreparedRouting` 破棄時の解放を stub 記録で検証する。
- [ ] `sf03_l3.rs`（新規）を含む `mise run test` が green。
- [ ] `mise run check`/`lint`/`fmt-check`/`test`/`test-rt`/
      `check-headless-deps` が全て exit 0。
- [ ] spec → design → plan → task → test-spec の文書チェーンが
      `docs/` に存在し、doc-status audit の blocker がない。

## 範囲外（後続 slice / 別 Issue）

- GUI 表示・`synvelis-app`/`session.rs`/`EngineStub` の置き換え・
  app 層への host 配線（接続経路は注入 boundary 経由で完結）。
- SF-ACT-06 系: parameter/event dispatch・`PluginNodeControl` 経由の
  automation 紐付け・stale dispatch 防御。
- SF-ACT-05 の残り: queue-full/partial stale command・RT latency/
  underrun・device epoch restart の実機検証、reconciliation/retry の
  実 host 経路での追加検証。
- NoRebuild fingerprint 最適化（SF-ACT-09）。
- 実 OS・実デバイス・L1 out-of-process plugin の検証
  （adapter は composite `PluginHost` 経由のため L1 spec 登録も
  可能だが、本 slice の検証は in-process factory のみ）。
- `capability_revision` の port 単位深い照合・plugin `version`
  照合（semantic identity の pin は Project 側責務、照合強化は
  後続）。
- legacy `lower()`/`lower_with_inserts`/`HeadlessDriver` の変更
  （`RoutingEngine` 経路のみ）。
