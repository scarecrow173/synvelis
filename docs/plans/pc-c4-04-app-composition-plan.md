---
id: "PLAN-PCc4040bs3vN1pQ8wXyZ2aD"
type: "plan"
status: "approved"
title: "PC-C4-04 app composition plan — vend・所有・駆動・橋渡し・fixture"
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
    - "docs/designs/pc-c4-04-app-composition-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/pc-c4-04-app-composition-design.md"
    - "docs/specs/pc-c4-04-app-composition.md"
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

# PC-C4-04 app composition plan

## file map

| file | 変更 |
| --- | --- |
| `crates/synvelis-player/src/host.rs` | `PluginHostProcessors` に catalog vend（`register_params`/`param_catalog`/`param_catalog_cell` + connect 時 `vendored` 蓄積）のみ |
| `crates/synvelis-app/src/engine.rs` | `Inner.dispatch`/`param_catalog`/`last_control_error`、publish 後 `sync_control_dispatch`、`control_*`/`with_control_dispatch`/`set_param_catalog` 公開面 |
| `crates/synvelis-app/src/ui_ops.rs` | `pump_control`（ui_pump tail の tick 駆動）、`last_control_tick`、`control_fast_control`、`feed_control_cycle` |
| `crates/synvelis-app/Cargo.toml` | `ruviform-audio-plugin-bridge` 追加（coordinator 範囲拡張の承認待ち） |
| `crates/synvelis-app/tests/pc_c4_04_app_composition.rs` | 新規 headless fixture |
| `crates/synvelis-workspace/src/control.rs` | 変更なし（`feed_plugin_report` が既に橋渡し先として完備） |

## 順序

1. host.rs catalog vend（完了）
2. engine.rs dispatch 所有 + 公開面（完了）
3. ui_ops.rs tick 駆動 + feed 橋渡し（完了・dep 承認待ち）
4. headless fixture（TDD 証拠）
5. mise 検証 → merge main → commit/push/PR

## risk

- `feed_control_cycle` の dep 拡張が拒否された場合は primitive
  列受け取りへ縮退（design 参照）。
- `SessionEngineConfig` への field 追加は test literal を破壊する
  ため setter 方式を採用（確定）。
- `ProcessorConnection` field 追加は範囲外ファイルを破壊するため
  不採用（確定）。
