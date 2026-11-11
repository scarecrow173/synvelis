---
id: "DSGN-PCc4040bs3vN1pQ8wXyZ2aC"
type: "design"
status: "approved"
title: "PC-C4-04 app composition design — vend cell・所有 dispatch・pump 駆動・feed 橋渡し"
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
  implements:
    - "docs/specs/pc-c4-04-app-composition.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/designs/pc-c4-03-param-observation-design.md"
    - "docs/designs/pc-c4-02-plugin-instance-binding-design.md"
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
---

# PC-C4-04 app composition design

## catalog vend（`synvelis-player/src/host.rs`）

pinned ruviform（b02942c2）の `PluginHost` は attached
（`Shared`/L1）instance の `params()` に公開 accessor を持たず、
`plugin_mut` は `Plain` slot のみを返す — attach 可能な instance
の parameter 列挙は host 経路では取得不能。したがって vend は
**caller 登録 + host 集約** の形を取る:

- `PluginHostProcessors::register_params(plugin_id, Vec<ParamInfo>)`
  — composition root（factory 登録者＝plugin 作者が静的に知る
  table の保持者）が plugin descriptor id ごとに登録する。
- `connect` 成功時に `ProcessorBinding::Resolved` の descriptor で
  引き、`vendored: Rc<RefCell<ParamCatalog>>`（`ProcessorId →
  Vec<ParamInfo>`）へ蓄積する。未登録は空 table — bind は typed
  に失敗し、列挙を捏造しない。
- `param_catalog_cell()` が共有 cell を vend する。composition
  root は host を `Box<dyn ProcessorHost>` へ移す**前**に cell を
  取り、engine へ渡す — `Rc` により後続 generation の connect も
  live に観測できる。
- `ProcessorConnection` への field 追加は行わない
  （`routing_engine.rs`/`export.rs`/player tests が literal 構築・
  分解しており、本 slice 範囲外の破壊になる）。蓄積 catalog が
  dispatch surface の必要とする `ProcessorId` keyed evidence で
  あり、per-connection vend は不要。

## dispatch 所有（`synvelis-app/src/engine.rs`）

- `Inner.dispatch: Option<RoutingControlDispatch>` を新設。
- `rebuild` の `publish` 直後に `sync_control_dispatch` を呼ぶ:
  live published generation を `source_map().generation` から読み、
  `dispatch` が無ければ `attach`、有れば `sync`（同世代 refresh /
  世代 swap の one-shot control 再 vend）。catalogs は共有 cell を
  毎回 clone して渡す — connect 時 vend が常に最新。
- publish 失敗時も live plan（last-good）の世代で `sync` = refresh
  のみ走り害はない。attach/sync 失敗は `last_control_error` に
  typed 記録し、surface は fail-closed のまま。
- 公開面: `set_param_catalog(cell)`、`control_tick(project, pos,
  position)`、`control_fast_control(project, write)`、
  `control_observe_cycle(processor, generation, span, call_start,
  call_frames)`、`with_control_dispatch(f)`（host 型を名指せない
  drain/boundary 操作の scoped seam）、`control_report()`/
  `control_generation()`/`last_control_error()`。
- `active_generation` は `rt_status().active_generation` の
  `0 → None` 規約（未観測 = 常に gate）を player test と同じく
  正直に写す。

## tick 駆動・observation 橋渡し（`synvelis-app/src/ui_ops.rs`）

- `ui_pump` tail（`drain_probe` の直前）に `pump_control` を追加:
  session `playback().position`（`BeatPoint`）と `clock_now()`
  （`MediaTime`）の正直な instant 対で `control_tick` を駆動し、
  `UiOpsState::last_control_tick` に report を保持する。
- `SynvelisApp::control_fast_control(write)` — op 層が
  `FastControlWrite` を組み立てて委譲する面（GUI op 登録は
  service.rs 占有のため本 slice 範囲外、headless 面として露出）。
- `SynvelisApp::feed_control_cycle(&DispatchCycle) -> usize` —
  `Reported` stage の observation のみ
  `ControlSession::feed_plugin_report` へ橋渡しする。stamp は
  `MediaTimestamp::estimated(obs.position)` — plugin report の
  測定 instant は backend 推定であり `Estimated` を名乗るのが
  正直（`observation_state` は `Stale` を返し `Live` を捏造しない）。
  `Scheduled`/`Dispatched`（host 解決値）・`Dropped`・
  `Unavailable` は skip — plugin report として再ラベルしない。
- `observe_cycle` の span/call window は物理 call partition が
  RT runtime 内にあるため caller供給 — `control_observe_cycle`
  はその pass-through。headless fixture は `run()` 幾何から正直な
  window を組み立てる。

## 依存追加

`feed_control_cycle` の `DispatchStage::Reported` 選別には
`ruviform-audio-plugin-bridge` の型名が必要（`DispatchStage`/
`ParamValueObservation` は同クレートにのみ存在し、synvelis-player
 は再公開しない）。`synvelis-app/Cargo.toml` への workspace 依存
1行追加は coordinator が承認済み（`[dependencies]`、理由は PR に
記録）。headless fixture が実 `PluginHost` + テスト plugin
factory を必要とするため `[dev-dependencies]` への
`ruviform-plugin-core`/`ruviform-plugin-host` 追加も照会中 —
`PluginNodeControl::new` は `pub(crate)` で外部構築不可、
`ParamInfo`/`ParamValue`/`PluginHandle` は他クレートから再輸出が
なく、bridge dep のみでは fixture を組めない（pc-c4-03 player
test と同一の plugin fixture を app test に置く形）。拒否された
場合は fixture を「attach/tick/fast_control/generation gate +
合成 `DispatchCycle` による feed 橋渡し」へ縮退し、実 plugin
drain→Reported の end-to-end は player 側 pc-c4-03 test の証拠に
委ねる。

## generation gate 証明

fixture は publish→attach→fast_control→run→drain→observe→feed の
全経路を 2 世代で駆動し、旧世代 surface への dispatch/report が
gate（`generation`/`active_generation` 不一致）で fail-closed に
なること、feed に到達する値が新世代の plugin report のみである
ことを検証する。
