---
id: "DESIGN-a1d4f7c2e9b30586d1a4f7c2f"
type: "design"
status: "approved"
title: "PC-C4 drain composition design — drain_output_reports 周期配線と production composition root"
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
    - "docs/specs/pc-c4-drain-composition.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/designs/pc-c4-04-app-composition-design.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-drain-composition.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-drain-composition.md"
  verified-by:
    - "docs/test-specs/pc-c4-drain-composition.md"
  references:
    - "docs/designs/pc-c4-04-app-composition-design.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
  defers: []
  deferred-by: []
---

# PC-C4 drain composition design

## 概要

2 つの独立した配線を 1 slice で行う。どちらも「既に実装済みの
player 側機構を、所有境界を越えずに app/製品入口へ接続する」
ことだけが仕事であり、新しいドメイン規則は導入しない。

1. **drain 周期配線**: `RoutingControlDispatch::drain_output_reports`
   を `ui_pump` の周期（control tick と同じ 16 ms UI tick + op tail）
   で駆動し、結果を既存の `note_control_drain` →
   `syn.control.status.drain` 経路に載せる。
2. **production composition root**: 製品 plugin 一覧
   （factory + 静的 `ParamInfo` table）から
   `PluginHostProcessors`（共有 `Rc<RefCell<PluginHost>>` cell を内包）
   を組み立て、engine へ `set_processor_host`/`set_param_catalog`、
   app へ `attach_routing_engine` + drain 登録までを 1 関数で行う。
   native bin がこれを呼ぶ。

## 責務境界

- **synvelis-player** が `PluginHost` cell を所有する
  （`PluginHostProcessors`）。app/player の上位層は `PluginHost`
  型を名指さない — drain は player 側の新しい handle
  `PluginReportDrain`（`Rc<RefCell<PluginHost>>` の clone のみを
  持つ `Clone` newtype）が担う。
- **synvelis-app の engine** が generation 解決を担う:
  `plugin_state_handles(generation)`・`rt_status().active_generation`
  を読み、`with_control_dispatch` 内で
  `drain_output_reports` を呼ぶ。generation は常に attach 済み
  surface 自身のものであり、app 側が別 generation を捏造する経路は
  作らない（stale generation の fail-closed は player 内部 gate が
  既に保証し、pc_c4_04 fixture が pin 済み）。
- **SynvelisApp** は登録済み drain handle を保持し、`ui_pump` から
  1 pass 駆動して `note_control_drain` に渡す。`project.new`/
  `project.open` の session 再生成では `routing_engine` と共に
  drain 登録も破棄する（古い host cell への drain が残らない）。
- **composition root**（`synvelis_app::compose_routing_engine`）は
  `PluginHostProcessors::product(factories)` で host を新規構築し、
  `register_params` で catalog を vend、`param_catalog_cell` と
  `report_drain` を engine/app に配線する。`PluginHost` の所有は
  依然として player adapter 内の `Rc<RefCell>` に留まり、app も
  bin も host を直接触らない。

## RT / 非 RT 契約

- `PluginHost::drain_output_reports` は upstream で non-RT only
  （append/allocate・shared lock を取り得る）。呼び出し経路は
  `ui_pump`（UI スレッド）と明示的 `SynvelisApp::drain_control_reports`
  のみで、RT callback・`process` 内からは到達しない。
- `RefCell` 再入: drain は `with_control_dispatch` の
  `borrow_mut` 内で host cell を `borrow_mut` する。host の
  `process`/`drain_output_reports` が dispatch や engine を
  再借用する経路は存在しない（host → plugin hook のみ）ため、
  単一スレッド逐次借用で安全。fixture が実経路で検証する。

## composition root の値

- `SessionEngineConfig`: native bin は
  `media_root = <SYNVELIS_PROJECT_DIR>/media`（verified blob store
  は bundle 兄弟配置）、engine 幾何は 48 kHz/2ch/256 frames、
  `BuildMode::Realtime`、runtime instance/epoch/generation = 1。
- plugin registry: 製品出荷 factory はまだ存在しない
  （実 VST3/OS backend は ruviform#253 の外部 gate）。registry は
  空で組み立て、plugin-bound processor は typed
  `PluginUnavailable` で失敗する — 値を捏造しない。
- untitled boot（`SYNVELIS_PROJECT_DIR` 未設定）と
  `project.new`/`open` 後の再 attach は本 slice 対象外:
  media_root の無い untitled session には engine を付けず、
  session 再生成で detach された engine の再構成は SF-03 slice 5
  の deferred「project 切替時の engine wiring」slice が担う。

## 失敗時の振る舞い

- surface 無し / generation 不整合 / active generation 未観測:
  `drain_output_reports` が `attached=false`/`generation_gated`/
  `inactive_gated` の typed report を返し、host drain は一切
  呼ばれない。report はそのまま `syn.control.status.drain` に出る。
- queue 飽和: fast-control 書込みは従来どおり typed
  `Blocked(QueueFull)` — drain 配線はこの経路を変更しないが、
  fixture が op surface 経由で「成功誤報なし」を pin する。
