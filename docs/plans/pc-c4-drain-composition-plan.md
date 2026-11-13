---
id: "PLAN-a1d4f7c2e9b30586d1a4f7c2f0a1"
type: "plan"
status: "approved"
title: "PC-C4 drain composition plan"
created: "2026-11-13"
updated: "2026-11-13"
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
  implements:
    - "docs/designs/pc-c4-drain-composition-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/designs/pc-c4-drain-composition-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/designs/pc-c4-drain-composition-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-drain-composition.md"
  verified-by:
    - "docs/test-specs/pc-c4-drain-composition.md"
  references:
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/designs/pc-c4-drain-composition-design.md"
  defers: []
  deferred-by: []
---

# PC-C4 drain composition plan

## ファイルマップ

### synvelis-player（`crates/synvelis-player/src/host.rs`）

- `PluginReportDrain`（新規 `Clone` newtype）: `Rc<RefCell<PluginHost>>`
  の clone のみを保持し、`drain(&self, dispatch, endpoints, generation,
  active_generation) -> ReportDrainReport` が内部で
  `dispatch.drain_output_reports(..., |h, out| host.borrow_mut()
  .drain_output_reports(h, out))` を呼ぶ。app が `PluginHost` を
  名指さないための seam。
- `PluginHostProcessors::product(factories)` — 新規 `PluginHost` に
  factory を登録して包む composition 構築口（app が `PluginHost::new`
  を名指さずに済む）。
- `PluginHostProcessors::report_drain()` — 同一共有 cell から
  `PluginReportDrain` を vend。

### synvelis-app

- `src/engine.rs`: `SessionRoutingEngine::control_drain_reports
  (&self, drain: &PluginReportDrain) -> Option<ReportDrainReport>` —
  surface generation・`plugin_state_handles`・RT 観測
  `active_generation` を解決し `with_control_dispatch` 内で drain。
  surface 無しは `None`。
- `src/app.rs`: `control_report_drain: Option<PluginReportDrain>`
  field、`set_control_report_drain`、`project_new_inner`/
  `project_open_inner` で `routing_engine` と共にクリア。
- `src/ui_ops.rs`: `SynvelisApp::drain_control_reports`（pub、
  決定的 1 pass — fixture/明示 driver 用）と `ui_pump` 末尾への
  周期呼び出し。`note_control_drain` の doc を実態に合わせ更新。
- `src/composition.rs`（新規）: `ProductPlugin { factory, params }`
  と `compose_routing_engine(app, config, plugins) ->
  SessionRoutingEngine`。
- `src/lib.rs`: module 公開。
- `src/bin/synvelis-native.rs`: `SYNVELIS_PROJECT_DIR` がある boot
  で `compose_routing_engine` を呼ぶ（`media_root=<dir>/media`、
  空 plugin registry）。

### テスト

- `crates/synvelis-app/tests/pc_c4_drain_composition.rs`（新規）:
  pc_c4_04/06 fixture パターンを踏襲し、composition root 経由で
  engine を組み立てる headless fixture。

## 順序

1. player seam（`PluginReportDrain`/`product`/`report_drain`)。
2. app engine drain 駆動 + app 登録 + ui_pump 周期。
3. composition root + native bin 配線。
4. fixture テスト（TDD: 先に赤を確認してから配線を完了）。
5. canonical 9 lane → doc index 再生成 → impl/ir 記録。

## リスク

- `RefCell` 二重借用: drain は `inner.borrow_mut()` 内で host cell
  を借りるが、host→plugin hook は app/engine を触らないため安全。
  fixture が実経路で pin。
- automation plan_generation の決定性: `ui_pump` 毎に tick が plan
  を進めるため、observe span は receipt 駆動で合わせる
  （pc_c4_06 と同じ規則）。
