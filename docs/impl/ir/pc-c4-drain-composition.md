---
id: "IMPL-a1d4f7c2e9b30586d1a4f7c2f0a4"
type: "impl"
status: "completed"
title: "PC-C4 drain composition — drain_output_reports 周期配線と production composition root"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c4-drain-composition.md"
      - type: "file"
        path: "docs/designs/pc-c4-drain-composition-design.md"
      - type: "file"
        path: "docs/plans/pc-c4-drain-composition-plan.md"
      - type: "file"
        path: "docs/tasks/pc-c4-drain-composition-task.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-drain-composition.md"
      - type: "file"
        path: "docs/impl/ir/pc-c4-drain-composition.md"
      - type: "file"
        path: "crates/synvelis-app/src/composition.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/pc_c4_drain_composition.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/host.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/engine.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis-native.rs"
  implements:
    - "docs/plans/pc-c4-drain-composition-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-drain-composition-plan.md"
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/designs/pc-c4-drain-composition-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C4 drain composition — implementation record

## Summary

synvelis#101 残件 slice: `drain_output_reports` の周期配線と
production composition root を実装した。player 側に
`PluginReportDrain`（共有 `PluginHost` cell への clone handle）と
`PluginHostProcessors::product`/`report_drain` を追加し、app 側に
`SessionRoutingEngine::control_drain_reports`・
`SynvelisApp::drain_control_reports`/`set_control_report_drain`/
`last_control_drain`・`ui_pump` 末尾の周期呼び出し・
`compose_routing_engine` production composition root を追加した。
`synvelis-native` は `SYNVELIS_PROJECT_DIR` の boot project で
composition を呼ぶ（空 plugin registry — 実 backend は
ruviform#253 の外部 gate）。fixture
`pc_c4_drain_composition.rs` が composition・周期 drain・
Reported→Observed 伝播・inactive gate fail-closed・queue 飽和
typed 応答を pin する。

## Context

- `RoutingControlDispatch::drain_output_reports` は PC-C4-03/04/06
  で実装済みだったが、呼び出しは headless fixture の
  `with_control_dispatch` 手動駆動のみ — app 周期経路と製品
  composition root の両方が未配線だった。
- `PluginHost` は `Rc<RefCell>` cell として `PluginHostProcessors`
  内に留める既存契約を維持し、app/bin が host 型を名指さない
  seam（`PluginReportDrain`）で drain を駆動する設計とした。
- doc-driven-dev-graph の route は repo 全体の既存 broken
  `defers` relation（artifact-graph audit、他 slice 由来・本 slice
  範囲外）で `blocked` を返す状態が続いている。先行 PC-C4 slice
  と同じく canonical doc-chain を手順どおり作成して進めた。

## Implementation

- `crates/synvelis-player/src/host.rs`:
  - `PluginReportDrain` — `Rc<RefCell<PluginHost>>` の clone のみを
    持つ `Clone` newtype。`drain(dispatch, endpoints, generation,
    active_generation)` が `dispatch.drain_output_reports` に
    `|handle, out| host.borrow_mut().drain_output_reports(handle,
    out)` の borrow closure を渡す。generation/active gate は
    dispatcher 内に残る。
  - `PluginHostProcessors::product(factories)` — fresh
    `PluginHost` + factory 登録の composition 構築口。
  - `PluginHostProcessors::report_drain()` — 同一共有 cell から
    `PluginReportDrain` を vend。
- `crates/synvelis-app/src/engine.rs`:
  `control_drain_reports(&self, drain)` — surface 自身の
  generation で `plugin_state_handles` を vend、`rt_status()` の
  RT 観測 `active_generation` を解決し `with_control_dispatch`
  内で 1 pass。surface 無しは `None`。
- `crates/synvelis-app/src/app.rs`: `control_report_drain` field・
  `set_control_report_drain`・`project_new_inner`/
  `project_open_inner` で `routing_engine` と共にクリア。
  review-protocol 指摘を受けた fail-closed 強化:
  `attach_routing_engine` が engine 差替え時に drain handle と
  `last_control_drain` evidence を無効化（engine と drain の
  host cell ペアは convention ではなく attach が強制）、
  両 recreate 経路で `last_control_drain` を default に reset
  （`engine: false` の隣に dead session の `attached: true` が
  残らない）。
- `crates/synvelis-app/src/ui_ops.rs`: `drain_control_reports`
  （pub、決定的 1 pass）を `ui_pump` 末尾（`pump_control` の後・
  `drain_probe` の前）に周期配線。`last_control_drain` getter 追加。
  `note_control_drain` doc を実態に更新。
- `crates/synvelis-app/src/composition.rs`（新規）:
  `ProductPlugin { factory, params }` と
  `compose_routing_engine(app, config, plugins)` — factories →
  `PluginHostProcessors::product`、descriptor id 毎の
  `register_params`、`param_catalog_cell`/`report_drain` を
  engine/app に配線し `attach_routing_engine`。
- `crates/synvelis-app/src/bin/synvelis-native.rs`:
  `compose_product_routing` — `SYNVELIS_PROJECT_DIR` 存在時のみ
  `media_root=<dir>/media`・48 kHz/stereo/256・Realtime・
  instance/epoch/gen=1 の `SessionEngineConfig` で
  `compose_routing_engine(app, config, vec![])` を呼ぶ。
  untitled boot と project 再生成後の再 attach は対象外
  （design の deferred 記述どおり）。
- `crates/synvelis-app/tests/pc_c4_drain_composition.rs`（新規）:
  pc_c4_04/06 の reporting plugin・project 構築を踏襲し、engine は
  `compose_routing_engine` 経由のみで組み立てる（fixture は
  `PluginHost`/`PluginHostProcessors` を名指さない）。4 test:
  composition+周期 drain、Reported→Observed feed、inactive gate +
  queue 飽和 fail-closed、`project.new` recreate での
  drain/evidence クリア。

## Related Experiments

なし。

## Validation

- `mise run check` — workspace check exit 0。
- `mise run test` — `pc_c4_drain_composition` 4 test を含む
  workspace test（結果は PR 本文に実測を記載）。
- canonical 9 lane は PR 本文「実行した検証」に実測値で記録。

## Risks

- `RefCell` 再入: drain は engine `inner.borrow_mut()` 内で host
  cell を `borrow_mut` するが、host→plugin hook は app/engine を
  触らないため逐次借用で安全。fixture が実経路で pin。
- `ui_pump` 毎の drain は automation tick と同じ周期 — drain は
  plan を進めないため observe の plan_generation 会計に影響しない
  （fixture は pump を挟まない決定的系列で pin）。

## Follow-ups

- L1 wire schema 検証・実 VST3/AU/CLAP・OS backend・実 GUI・
  AudioLens/Ruviform consumer 照合・oracle — synvelis#101 残件、
  後続 slice。
- `project.new`/`project.open` 後の engine 再 attach
  （SF-03 slice 5 deferred「project 切替時の engine wiring」）。
- `synvelis`（webview）bin への composition 配線。
- 製品 plugin factory registry の実装（ruviform#253 解消後）。
