---
id: "SPEC-034VWsjfgfnLgo8ntUpQH6"
type: "spec"
status: "approved"
title: "SF-03 slice9 — host capability generation live firing"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  derived-by:
    - "docs/designs/sf-03-slice9-capability-firing-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice9-capability-firing.md"
  references:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  defers: []
  deferred-by: []
---

# SF-03 slice9 — host capability generation live firing

## What Should Be Built

host 側 capability 状態（`ProcessorHost`/`PluginHostProcessors` の
boundary/catalog 登録・factory/route 登録）を表す capability
generation を新設し、`RoutingEngine` の prepare/publish/reconcile
の各 gate に fold する。`RoutingBuildKey.capability_generation`
（project 側 `port_capability_generation` のみを含む persisted
capability 和）は変更せず、host 側 generation は engine が
prepared bundle ごとに pin する別軸として扱う。

- `ProcessorHost::capability_generation`（default `0`）を trait に
  追加し、`PluginHostProcessors` が registration epoch +
  composite `PluginHost` の audio `Scan` descriptor を畳み込んだ
  決定的 generation を返す。
- `RoutingEngine::prepare` が host generation を capture して
  `PreparedRouting` に保持、`publish` が project 側 capability
  sum と host generation の両方を再検証し、不一致を
  `RoutingFault::PluginCapabilityChanged` で fail closed する。
- publish 成功時に host generation を ledger に pin し、
  `reconcile` が project 側 sum + host 側 generation の両方を
  比較して `RoutingReconcileMismatch::CapabilityGeneration` →
  `PluginCapabilityChanged` を報告する。
- `NoRebuild` equivalence probe も host generation を比較対象に
  含め、host drift 下で semantic 同一 revision が stale な
  capability 証拠の plan に shortcut しないようにする。
- `crates/synvelis-player/tests/sf03_l9.rs`（新規）で
  `PluginCapabilityChanged` の publish live 発火・reconcile
  `CapabilityGeneration` mismatch・equivalence 敗退・host swap
  を headless に固定する。

## Why Now

slice2 の fault taxonomy は `PluginCapabilityChanged` を
capability drift の typed fault として定義したが、現行の
`port_capability_generation` は project 内 `capability_revision`
の和のみを含み host 側状態を観測しない。project fingerprint も
host 登録を見ないため、host 側 drift（factory 追加・boundary/
catalog 登録変更・host 差し替え）は検出経路を持たず、
`PluginCapabilityChanged` は `SpeculativeMismatch` に shadow
されて live 発火しない defensive-only fault だった
（design §8.2 残件・slice2 design L254-267）。本 slice で
host 側 generation を engine の capability pin に fold し、
fault taxonomy の残る live 発火可能 fault を実測で固定する。

## Users and Value

- 後続の activation/reconcile consumer: publish 済み plan の
  capability 証拠が host 登録状態を含むことが保証され、
  host drift が `PluginCapabilityChanged` として表面化する。
- fixture 作者: `PluginHostProcessors` の全登録軸
  （boundary/catalog/factory）を 1 つの generation で観測する
  deterministic 経路。

## Scope

### In Scope

- `ProcessorHost::capability_generation` trait method（default
  実装 `0`）。
- `PluginHostProcessors` の registration epoch + `Scan` 畳み込み
  による capability generation 実装。
- `RoutingEngine` の prepare capture / publish 再検証・pin /
  reconcile 比較 / equivalence probe への host generation fold。
- `RoutingBuildKey.capability_generation` の key 契約文書化
  （project 側半分であることの明記。構造変更なし）。
- `crates/synvelis-player/tests/sf03_l9.rs`（新規・5 fixture）。
- docs チェーン（spec/design/plan/task/test-spec/impl）。

### Out of Scope

- `RoutingBuildKey` への field 追加・`port_capability_generation`
  の意味変更（workspace/render 側の key 構築・比較を壊すため）。
- `EnginePort` trait・`synvelis-workspace`・`synvelis-app` の
  変更（session 経路 wiring は既存 API で足りる）。
- `ActivationNotObserved` の発火経路新設 — engine の
  epoch/instance は構築後不変で、RT activation を正当に
  withhold する seam は存在しない。NotRun 記録。
- `RetireOverflow`（Ruviform retire channel 溢れが前提・
  headless 決定的再現不可）・reconcile `DeviceEpoch`（epoch
  遷移 API なし）— NotRun 記録。
- SF-ACT-07 preview policy・実 VST3・実機・GUI・RT 実測の外部
  gate — NotRun 記録。
- Issue #106 の Close（Coordinator が行う）。

## Requirements

- `ProcessorHost::capability_generation` は登録状態の決定的な
  変更検出値を返し、mutation が無ければ同一値を返す。
- `PluginHostProcessors` の generation は boundary declaration
  登録・param catalog 登録・factory/route 登録（`Scan` で
  観測可能な descriptor 変化）の全てを反映する。
- `StaticProcessorHost` は default `0` — provisioning 証拠は
  compiled plan に消費され registration catalog を持たない。
- `prepare` は host generation を bundle に pin し、`publish`
  は live host generation との不一致を
  `PluginCapabilityChanged` で拒否する（fingerprint が一致して
  いても）。
- publish 成功時に pin した generation が ledger に保存され、
  `reconcile` は project 側 sum と host 側 generation の双方の
  不一致を `CapabilityGeneration` mismatch として報告し、
  fault は `PluginCapabilityChanged` に写像される。
- `NoRebuild` equivalence は host generation が一致する場合の
  み成立し、drift 時は `Built` に倒れる。
- connect による instance 消費は capability drift と見なさない
  （build の staging/compile の一部）。
- 失敗時の振る舞い: publish 拒否は `Blocked` receipt +
  last-good 保持、reconcile mismatch は `Blocked` receipt +
  live plan 継続 — いずれも fail closed。

## Acceptance Criteria

- [ ] `sf03_l9_01`: generation が登録軸全てで決定的に変化し、
  mutation 無しで安定。
- [ ] `sf03_l9_02`: prepare→publish 間の factory drift で
  publish が `PluginCapabilityChanged` を返し、last-good が
  保持され、drift 後の再 prepare→publish が成功する。
- [ ] `sf03_l9_03`: publish/activate 後の host drift で
  reconcile が `CapabilityGeneration` mismatch +
  `PluginCapabilityChanged` fault を報告し、plan は live の
  まま。
- [ ] `sf03_l9_04`: host drift 下の semantic 同一 revision が
  `Equivalent` ではなく `Built` を返し、drift 無しでは
  `Equivalent` のまま。
- [ ] `sf03_l9_05`: prepare→publish 間の host 差し替えが
  `PluginCapabilityChanged` になる。
- [ ] `ActivationNotObserved`/`RetireOverflow`/`DeviceEpoch`/
  外部 gate が IR/matrix で NotRun と正直に記録される。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`check-app-native`/`lint-app-native`/`build` が
  exit 0。

## Deferred Design Concerns

- `ActivationNotObserved` の headless 発火 — engine epoch/
  instance が構築時 pin で遷移 API が無く、RT activation の
  withhold に正当な seam が無い。発火させるには fault-
  injection seam の新設が必要であり本 slice では行わない。
  | 理由: defensive-only fault の live firing 義務は
  「正当な経路が存在する場合」に限る | 再評価 trigger: RT
  activation を制御可能な正当な観測経路が設計されたとき |
  無視時 risk: fault が defensive のまま残る（仕様上許容）。
- `RetireOverflow` — Ruviform retire channel の溢れが前提で
  headless 決定的再現不可。upstream fault-injection 要求は
  ruviform 側 Issue 候補。
- reconcile `DeviceEpoch` — engine が epoch/instance を
  構築時に pin し遷移 API が無い。device epoch 遷移の設計は
  別 slice。
- 実 VST3 での host drift 発火 — 外部 gate。実 plugin host の
  factory unload/scan 変化を伴う検証は GUI/実機 lane で行う。

## Sources and Relations

- Issue #106（SF-03）。
- `docs/specs/sf-03-map-semantic-runtime-contract.md`（親
  contract）。
- `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
  §8.2（残件記名）・`docs/designs/sf-03-slice2-fault-reconcile-
  design.md` L254-267（defensive-only fault の記述）。
- `docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md`・
  `docs/specs/sf-03-slice3-plugin-host-connect.md`（先行 slice）。
- 実装: `crates/synvelis-player/src/host.rs`、
  `crates/synvelis-player/src/routing_engine.rs`、
  `crates/synvelis-player/tests/sf03_l9.rs`。
