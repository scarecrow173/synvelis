---
id: "PLAN-034VWvifs0UbP2Diwh0JiN"
type: "plan"
status: "completed"
title: "SF-03 slice9 — host capability generation live firing plan"
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
  implements:
    - "docs/specs/sf-03-slice9-capability-firing.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-03-slice9-capability-firing-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/sf-03-slice8-revision-fixture-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice9-capability-firing.md"
    - "docs/designs/sf-03-slice9-capability-firing-design.md"
  derived-by:
    - "docs/tasks/sf-03-slice9-capability-firing-task.md"
    - "docs/test-specs/sf-03-slice9-capability-firing.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice9-capability-firing.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice9 — host capability generation live firing plan

## Goal

host 側 capability generation を `ProcessorHost` に公開し、
`RoutingEngine` の prepare/publish/reconcile/equivalence に
fold して `PluginCapabilityChanged` と reconcile
`CapabilityGeneration` を headless で live 発火させる。

## Scope

単一実装 stream。`synvelis-player` の host/engine と
`synvelis-project` の key 契約文書化、新規 test 1 ファイル、
docs チェーンのみ。`EnginePort`・workspace・app・render は
変更しない。

## Upstream Documents

- `docs/specs/sf-03-slice9-capability-firing.md`
- `docs/designs/sf-03-slice9-capability-firing-design.md`

## Design Inputs

- `docs/designs/sf-03-slice9-capability-firing-design.md`

## File Map

- `crates/synvelis-player/src/host.rs` —
  `ProcessorHost::capability_generation`（default `0`）、
  `PluginHostProcessors.registration_epoch`、`register_boundary`/
  `register_params` の epoch bump、`Scan` descriptor + epoch の
  FNV 畳み込み実装。
- `crates/synvelis-player/src/routing_engine.rs` —
  `PreparedRouting.host_capability_generation`、prepare capture、
  publish 2半分再検証 + ledger pin、equivalence `host_current`、
  reconcile `CapabilityGeneration` 比較、doc 更新。
- `crates/synvelis-project/src/routing_build.rs` —
  `RoutingBuildKey.capability_generation` が project 側半分で
  あることの契約文書化（構造不変）。
- `crates/synvelis-project/src/routing_tx.rs` —
  `port_capability_generation` が project 側のみであることの
  文書化。
- `crates/synvelis-player/tests/sf03_l9.rs` — 新規 5 fixture。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  sf-03-slice9-capability-firing*.md` — docs チェーン。

## Implementation Sequence

- [x] `ProcessorHost::capability_generation` default +
      `PluginHostProcessors` registration_epoch/Scan 実装。
- [x] `PreparedRouting`/`Ledger` に host generation を追加し、
      prepare capture・publish 検証/pin・equivalence・
      reconcile を fold。
- [x] `routing_build.rs`/`routing_tx.rs` の key 契約文書化。
- [x] `sf03_l9.rs` 5 fixture 実装・green 確認。
- [x] docs チェーン作成・impl record。
- [ ] canonical verification lanes 全実行・PR/Issue 報告。

## Dependencies and Risks

- `RoutingBuildKey` は workspace/render の test が literal 構築
  するため field 追加不可 → host generation は key の外の
  engine pin として設計（design §Overview）。
- `PluginHost::register_factory` は adapter を介さず共有
  `Rc<RefCell<PluginHost>>` handle 経由 → `Scan` descriptor
  畳み込みが必須（epoch では捕捉不可）。
- `ActivationNotObserved`/`RetireOverflow`/`DeviceEpoch` は
  発火経路が存在しないか headless 再現不可 → NotRun 記録、
  fault-injection seam は新設しない。

## Task Breakdown

- task 1（`sf-03-slice9-capability-firing-task`）: host
  generation 実装・engine fold・test・docs・impl record。

## Verification

- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
      `mise run test` / `mise run test-rt` / `mise run test-msrv` /
      `mise run check-app-native` / `mise run lint-app-native` /
      `mise run build` が全て exit 0。
- [ ] `cargo test -p synvelis-player --test sf03_l9` が 5/5
      green（補助証拠）。
- [ ] `PluginCapabilityChanged` publish live 発火・reconcile
      `CapabilityGeneration` mismatch・equivalence 敗退・host
      swap を headless に固定。

## Open Questions / Gaps

- `ActivationNotObserved` の headless 発火可否 — engine
  epoch/instance が構築時 pin で遷移 API 無し。正当な発火経路が
  無いため NotRun（IR/matrix に根拠記録）。
- `RetireOverflow` — Ruviform retire channel overflow 前提、
  upstream seam 必要。NotRun。
- reconcile `DeviceEpoch` — epoch 遷移 API 無し。NotRun。
- 実 VST3・実機・GUI・RT 実測 — 外部 gate。NotRun。

## Self-Review

- spec Requirements は host trait 追加・engine fold・key 文書化・
  test 5 fixture に写像済み。placeholder なし。

## Review Handoff

- 前提: slice2（fault taxonomy）・slice3（plugin host connect）・
  slice8（fixture パターン）が main に存在。
- 未解決の残件（scope 外）: `ActivationNotObserved`/`RetireOverflow`/
  `DeviceEpoch` の defensive-only 継続、実 VST3/実機 gate、
  SF-ACT-07 policy。
