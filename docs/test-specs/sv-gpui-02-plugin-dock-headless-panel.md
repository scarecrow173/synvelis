---
id: "TSPEC-3kN7eQ3gW0xY6uC9nR7zKjPd"
type: "test-spec"
status: "approved"
title: "SV-GPUI-02 headless PluginDock panel テスト仕様 — 4-ID 状態機械 + syn.plugin_dock.* op (synvelis#176)"
created: "2026-11-24"
updated: "2026-11-24"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/176"
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
    - "docs/specs/sv-gpui-02-plugin-dock-headless-panel.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gpui-02-plugin-dock-headless-panel-design.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
  verifies:
    - "docs/specs/sv-gpui-02-plugin-dock-headless-panel.md"
---

# SV-GPUI-02 headless PluginDock panel テスト仕様

## Purpose

`crates/synvelis-app/tests/plugin_dock.rs` が検証する case を固定する。
2 tier の決定的 headless 検証 — OS window・plugin binary 不使用。

- **Panel tier** — `PluginDock` + app 共有 `PluginEditorSessions` +
  scripted `PluginEditorHost`/`EditorSessionOps` fake を直接駆動。
- **Op tier** — 同一語彙を `NativeShell` + `FakeDriver` 経由で
  `syn.plugin_dock.*` op として end-to-end 駆動。

## Cases

### Panel tier（4-ID 所有・世代・状態機械）

| case | test | 検証点 |
| --- | --- | --- |
| PD-HP-01 | `open_mints_four_ids_and_lands_attached` | `StablePluginInstanceId → PluginEditorSessionId → SurfaceSlotId → ViewInstanceId` の mint と `Attached{Docked}` 着地 |
| PD-HP-02 | `hide_show_round_trip_mirrors_slot_visibility` | Hide→`Hidden`/slot suppress、Show→`Attached`/slot visible |
| PD-HP-03 | `detach_hides_editor_without_closing_session` | dock detach で slot suppress + session hide、session は close しない |
| PD-HP-04 | `relocate_republishes_slot_and_revokes_old_lease` | window 跨ぎで新 slot mint・旧 lease revoke・同一 session で `Attached` 復帰 |
| PD-HP-05 | `relocate_keeps_old_parent_on_adapter_refusal` | adapter 拒否時に旧 parent seat を保持・session は `Failed` に正直化（Attached 偽装なし） |
| PD-HP-06 | `revoked_lease_is_stale_at_the_ledger` | close 後の lease が `SurfaceSlotRegistry` で `LeaseRevoked`/`InvalidLease`/`StaleGeneration`/`UnknownSlot` 系に拒否 |
| PD-HP-07 | `session_busy_is_refused_across_panels` | app 共有 registry の `SessionBusy` が panel 横断で 1 handle=1 editor を保証 |
| PD-HP-08 | `unsupported_open_lands_failed_honestly` | `EditorOpError::Unsupported` が `Failed` に正直着地（Pass 偽装なし） |
| PD-HP-09 | `adapter_lend_refusal_is_honest_unavailable` | host の ops lend 拒否が `unavailable` 系エラーに写像 |
| PD-HP-10 | `window_lost_converges_session_and_frees_slot` | window 喪失で slot 解放・session 終端収束・instance は温存 |
| PD-HP-11 | `instance_termination_converges_and_frees_lease` | plugin instance 終了で session 収束・lease 解放 |
| PD-HP-12 | `save_restore_round_trip_persists_only_logical_state` | snapshot が論理状態のみ保持（runtime id/lease/OS handle 非含有） |
| PD-HP-13 | `restore_honors_saved_visibility` | restore が保存時の visibility を復元 |
| PD-HP-14 | `close_retires_session_and_frees_panel` / `view_close_retires_the_panel` | close で session 退役・panel/binding 解放 |
| PD-HP-15 | `occupied_and_unknown_instance_guards` / `unknown_instance_is_refused` / `duplicate_instance_across_views_is_refused` | invalid param 系拒否 |
| PD-HP-16 | `close_without_host_converges_host_lost` | host 喪失下の close が `HostLost` 系に収束 |
| PD-HP-17 | `placement_vocabulary_round_trips_honestly` | Docked/Floating ↔ `AttachMode` の双方向写像 |

### Op tier（`syn.plugin_dock.*` end-to-end）

| case | test | 検証点 |
| --- | --- | --- |
| PD-HO-01 | `plugin_dock_ops_drive_the_full_editor_lifecycle` | `open→hide→show→focus→resize→dock_prepare→dock_commit（再dock）→dock_detach→dock_reattach→close` の op 語彙駆動 |
| PD-HO-02 | `save_restore_ops_round_trip` | `save`→`restore` op が論理 snapshot を往復 |
| PD-HO-03 | `window_destroy_converges_editor_and_preserves_project` | window destroy op が editor session を収束させ project/DSP を温存 |
| PD-HO-04 | `plugin_dock_ops_do_not_touch_project_or_dsp_state` | 全 op が project/transport/DSP state に非干渉 |
| PD-HO-05 | `unknown_instance_is_refused` / `duplicate_instance_across_views_is_refused`（op 経路） | wire error code への正直写像 |

### 非回帰

- `plugin_dock_contract{,_gpui}.rs` — upstream semantic mount と
  `{NativePluginDock, VisualCapture}` capability gate は変更しない
  （fake semantic adapter は capability を持たず `Unsupported` を維持）。
- `dock_ops.rs` / `sv113_dock.rs` — `syn.window.dock_*` 語彙の非回帰。

## Validation（実測 — canonical = mise）

| コマンド | 結果 |
| --- | --- |
| `mise run test synvelis-app` | pass — `tests/plugin_dock.rs` 25 tests + 全回帰 |
| `mise run lint synvelis-app` | pass（`-D warnings`） |
| `mise run fmt-check` | pass |
| `mise run check-app-native` / `lint-app-native` | pass |
| `mise run check-app-gpui` / `test-app-gpui` | pass — `plugin_dock_contract_gpui` capability gate 非回帰 |
| `mise run check-headless-deps` | pass — headless 12 crates に GUI framework dep なし |
| `mise exec -- prek run --all-files` | pass |

## Non-claims

- 本 spec の全 case は headless/fake の証拠。実 OS/native-plugin 受入
  （Issue 行 1・行 7）と merge 後 CI 実走（行 4・行 8）は
  `evidence/sv176/acceptance-ledger.md` で `NotRun`/外部 Owner/
  Coordinator 証拠工程のまま — 本 spec は `Pass` を主張しない。
