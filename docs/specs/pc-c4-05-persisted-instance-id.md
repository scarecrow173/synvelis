---
id: "SPEC-034VvDn4o6zcn7dFSGRGZC"
type: "spec"
status: "approved"
title: "PC-C4-05 persisted StablePluginInstanceId — project schema v13 + player hydration"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
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
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/rcd-03-monitor-persistence.md"
  refines:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/test-specs/pc-c4-02-e2e-plugin-instance-binding.md"
  defers: []
  deferred-by: []
---

# PC-C4-05 persisted StablePluginInstanceId — project schema v13 + player hydration

## What Should Be Built

Issue #101 の PC-C4 残件 slice。PC-C4-02（`docs/specs/pc-c4-02-plugin-instance-binding.md`）
が導入した runtime-only の `StablePluginInstanceId` を、project schema v13 として
**永続化**し、save→reload・engine restart を跨いで同一の stable instance
identity を復元する。第二 Parameter 正本・第二 identity 正本は作らない。

具体的には:

- **Persisted identity store**（`synvelis-project`）: `ProcessorId` ごとの
  `{ instance_id: u64, plugin: PluginRef }` レコードと単調増加する u64
  watermark を持つ `Project::plugin_instances` を新設する。watermark は
  次に発行する id を指し、発行済み id は決して再利用しない
  （entity `id_watermark` とは別ドメインの独立カウンタ）。
- **Schema v13 + v12→v13 migration**: `SCHEMA_VERSION` を 13 に進め、
  v12 body は `#[serde(default)]` で空 store に deserialize した上で、
  `ProcessorBinding::Resolved` を持つ全 processor へ `ProcessorId` 順の
  決定的採番で identity を backfill する。読み取り専用 projection で、
  既存 snapshot bytes は書き換えない。
- **不変条件（fail-closed oracle）**: `Resolved` processor ⇔ identity
  entry の全単射 — 全 `Resolved` processor が entry を持ち、entry の
  `plugin`（descriptor/version/**capability_revision** 全体）が authored
  `PluginRef` と一致し、id は一意かつ `1 <= id < watermark`、
  `Unresolved` processor は entry を持たない。`Project::validate_references`
  が load・apply 双方でこの oracle を適用する。
- **採番・維持経路**: `InsertProcessor`（Resolved で採番）・
  `RemoveProcessor`（entry 除去、id は watermark により不回復）・
  VCS-05 apply の entity write 経路（`ensure` 意味論: plugin 不変の
  Modified は既存 id 維持、Added/再 bind は新規採番、Unresolved は除去）・
  migration（backfill）が全て同じ store を維持する。Coordinator 承認済みの
  範囲拡張として `crates/synvelis-project/src/apply/entity.rs` を含む。
- **Player hydration**（`synvelis-player`）: `RoutingControlDispatch`
  の `vend_nodes` は `next_instance` counter による ephemeral 採番をやめ、
  `project.plugin_instances` の persisted identity を
  `StablePluginInstanceId` として session/boundary provenance に流す。
  entry 欠落・plugin 不一致・id が watermark 範囲外は typed
  `without_control` evidence で fail-closed（silent skip しない）。
  `PluginBindingSession` の generation/plugin/frame-base pin 意味論は不変。

## Why Now

`StablePluginInstanceId` は第一 slice で runtime-only wrapper として導入
され、`RoutingControlDispatch` 内部の `next_instance` counter が attach
ごとに 1 から採番していた。engine restart・project reload 後は同一
processor が別 id を得るため、永続化された binding 記録や provenance を
restart 横断で照合できない。PC-C4-02 spec の Out of Scope に
「persisted `StablePluginInstanceId`・binding snapshot は deferred」と
明記されていた残件を本 slice で解消する。schema v12 は SV#117（RCD-03
monitoring）で確定済みのため、本 slice は v13 として追加する。

## Users and Value

- **Composition root / driver 層**: `RoutingControlDispatch` が project
  正本の persisted identity をそのまま hydration でき、独自採番の
  二重正本を持たない。
- **将来の binding snapshot / provenance 照合**: restart・reload を跨いだ
  `StablePluginInstanceId` の安定性が前提条件になる消費者
  （PC-C4-03 observation、pc-c4-04 app 配線）の土台。
- **Project 永続化の信頼性**: save→reload で同一 id が復元され、
  unknown/stale identity は load/apply/runtime の各境界で typed reject
  される。

## Scope

### In Scope

- `crates/synvelis-project`: 新規 identity ファイル、`project.rs`、
  `store.rs`（SCHEMA_VERSION=13）、`migration.rs`（v12→v13 step）、
  `ops.rs`（Insert/RemoveProcessor の identity 維持）、`lib.rs` export、
  `tests/`、および Coordinator 承認済みの `apply/entity.rs`（VCS apply
  write 経路の identity sync）。
- `crates/synvelis-player`: `src/binding.rs`（doc 更新）、
  `src/routing_control.rs`（`vend_nodes` hydration・`next_instance`
  撤去）、`tests/`。
- docs チェーン（spec/design/plan/task/test-spec/impl IR）。

### Out of Scope

- app/workspace/gateway 層の配線（pc-c4-04 別 slice — 本 slice は
  headless `RoutingControlDispatch` まで）。
- 実 VST3/OS plugin backend、実 GUI、実 device（Ruviform #253 gate）。
- `HeadlessDriver` の clip-insert FX attachment が使う instance id
  （`driver.rs` の `next_instance`）— project processor ではない別
  domain であり ephemeral のまま維持する。
- binding snapshot 形式・`PluginNodeControl` への identity query 追加
  など Ruviform 側変更、第二 Parameter 正本。
- entity `id_watermark`（u128）との統合 — instance id は独立した u64
  ドメイン。

## Requirements

- `Project::plugin_instances` は `#[serde(default)]` で v12 以前の body
  を受理し、v13 body では watermark と全 entry を round-trip する。
- `InsertProcessor` の `Resolved` binding は apply 時に次の watermark
  値を採番し、`RemoveProcessor` は entry を除去する（id は再発行
  しない）。undo/redo は project snapshot ごと identity state を復元
  する。
- v12→v13 migration は `ProcessorId` 昇順で `Resolved` processor に
  1 始まりの連番を割り当て、watermark を `個数+1` に設定する決定的
  projection とする。
- `validate_references` は entry→processor の存在・`PluginRef` 完全一致
  （capability_revision を含む）・id 一意性・`1 <= id < watermark`・
  Resolved⇔entry 全単射を検査し、違反は `Error::Validation`/`NotFound`。
- `RoutingControlDispatch::vend_nodes` は persisted entry が無い、
  `plugin` が entry の `PluginRef` と一致しない、または id が発行済み
  範囲外の場合に `without_control` へ typed 記録して session を作ら
  ない。`control.plugin == None`・`take_processor_control == None` の
  既存 fail-closed 経路は維持する。
- `PluginBindingSession::attach`/`bind`/`dispatch` の pin・generation・
  frame-base 契約は変更しない。

## Acceptance Criteria

- [ ] `PC-C4-05-01` `Resolved` processor を持つ project の save→load で
  `plugin_instances` の entry・id・watermark が完全一致で復元され、
  v12 body からの load では migration が決定的に backfill する
  （migration round-trip 検証）。
- [ ] `PC-C4-05-02` 採番は persisted watermark と単調整合する:
  Remove→Insert で id が再利用されず、undo で除去前の id が復元
  される。`Unresolved` processor は entry を持たない。
- [ ] `PC-C4-05-03` `validate_references` は dangling entry・plugin
  不一致（capability_revision 差分を含む）・id 重複・watermark 範囲外
  id・Resolved processor の entry 欠落を全て typed reject する。
- [ ] `PC-C4-05-04` `RoutingControlDispatch` の session/boundary
  provenance が persisted id を引き継ぎ、reload 後の再 attach で同一
  `StablePluginInstanceId` が観測される。entry 欠落・plugin 不一致は
  `without_control` evidence で delivery 0。
- [ ] `PC-C4-05-05` VCS-05 apply が `Resolved` processor を追加しても
  apply が reject されず、追加 processor に新規 persisted id が採番
  されて binding 可能である（回帰 test）。plugin 不変の Modified
  apply は既存 id を維持する。
- [ ] `PC-C4-05-06` app 側接続（pc-c4-04）・実 plugin backend・実 GUI
  は NotRun。既存 PLUG-C4/SF-ACT/CTL 系 fixture は green を維持する。

## Deferred Design Concerns

- binding snapshot 形式（binding record の永続化）— 本 slice は
  instance id の永続化のみ。record snapshot は pc-c4-04 以降の
  app 配線 slice で扱う。
- `ProcessorBinding` の post-insert rebind を正式 op として開放するか
  — 本 slice では apply の Modified transplant 経由のみで、identity
  は `ensure` 意味論で再採番される。専用 op が必要になれば別 spec。

## Sources and Relations

- Issue #101（PC-C4 plugin parameter binding 系列）。
- `docs/specs/pc-c4-02-plugin-instance-binding.md`: runtime-only
  `StablePluginInstanceId` と deferred 明記の出所。本 spec はその
  deferred を解消する refines 関係。
- `docs/specs/pc-c4-plugin-param-binding.md`: stage-1 semantic binding
  正本（`semantic_plugin_binding`/`plugin_instance_binding`）。
- `docs/specs/rcd-03-monitor-persistence.md`: schema v12 追加の前例
  （`#[serde(default)]` + identity migration step）。
