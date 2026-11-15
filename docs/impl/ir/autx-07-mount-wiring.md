---
id: "IMPL-AUTX07MountWiring"
type: "impl"
status: "completed"
title: "AUTX-07 composition-mount wiring + autx.* action dispatch（synvelis#266 SV266-NR-05）"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/266"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/sv266_wiring.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx07_catalog_seam.rs"
      - type: "file"
        path: "docs/test-specs/autx-07-mount-wiring.md"
      - type: "file"
        path: "docs/impl/ir/autx-07-mount-wiring.md"
      - type: "file"
        path: "docs/impl/exp/autx-07-mount-wiring.md"
      - type: "file"
        path: "evidence/sv266-wiring/README.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/automation_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/automation.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/binding.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis-native.rs"
      - type: "file"
        path: "evidence/sv266/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/test-specs/autx-07-mount-wiring.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/autx-07-gui-views.md"
    - "docs/test-specs/autx-07-gui-views.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-07-mount-wiring.md"
    - "docs/impl/exp/autx-07-mount-wiring.md"
    - "evidence/sv266-wiring/README.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/test-specs/autx-07-gui-views.md"
  derived-by: []
  verifies:
    - "docs/test-specs/autx-07-mount-wiring.md"
  verified-by: []
  references:
    - "crates/synvelis-app/src/ui_ops.rs"
    - "crates/synvelis-app/src/shell.rs"
    - "crates/synvelis-app/src/app.rs"
    - "crates/synvelis-app/src/automation.rs"
    - "crates/synvelis-app/src/bin/synvelis-native.rs"
    - "crates/synvelis-workspace/src/automation_ui.rs"
    - "crates/synvelis-app/tests/sv266_wiring.rs"
    - "crates/synvelis-workspace/tests/autx07_catalog_seam.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-07-mount-wiring.md"
    rejected: []
---

# AUTX-07 composition-mount wiring + autx.* action dispatch（synvelis#266 SV266-NR-05）

## Summary

Issue #266 の deferred case SV266-NR-05 を実装した。前 slice
（`docs/impl/ir/autx-07-gui-views.md`、PR 済み headless 基盤）が
用意した driver-neutral projection・`AutomationViewState`・
`decode_automation_command`・4 surface の `mount_*_view` helper を、
composition root（`SynvelisApp`/`AppService`/`NativeShell`/
`synvelis-native`）へ実配線し、shell の `autx.*` logical target を
`syn.autx.action` op へ dispatch する経路を完成させた。

`ViewType::{ActionPalette, MacroBrowser, WorkflowEditor, RunMonitor}`
の 4 surface は `syn.autx.visual{action:"open", surface:...}` で
detached view として adopt され、`view-{n}` を返す。app は
`automation_screen_cell`（`Rc<RefCell<BTreeMap<ViewInstanceId,
AutomationScreenPublish>>>`）を `publish_project_length`（全 op の
post-op tail）から publish し、composition root が publish revision
stamp で変更を検出して mounted `ViewSpec` を再構成する。閉じた view
の entry は publish で落ち、消えた view の spec を捏造しない。

catalog は `synvelis-workspace::AutomationCatalog` という中立 seam
として追加した。authority-pushed JSON 文書
（`{seq, capabilities:[{descriptor, grant?}], macros:[...]}`）を
`#[serde(deny_unknown_fields)]` で厳密 parse し、grant 欠落は
`GrantState::Unknown`、macro preset の `executable`/`blocked` 表示
field は authority field から再導出する（push 値を信用しない）。
`synvelis-app` は gateway 型を一切 import せず、projection 参照と
`lower()` による wire request JSON だけを受け取る — catalog は
表示/request 入力であり authority ではなく、最終検査は Gateway が
行う。`syn.autx.catalog` op がこの文書の唯一の producer で、
live adapter（#264）までは test/composition root が注入する。

`syn.autx.action{view_id, target, action, params?}` は
`decode_automation_command` の単一分類表で decode し、
`ViewLocal` は `ViewState.automation` のみを mutate、`Request` は
catalog 不在なら `unavailable` で typed reject、存在すれば
`AutomationCatalog::lower` で wire request JSON へ lowering して
`requests` として返す。consistency は preview 系が `latest`、
mutation 系が live revision の `expected-revision`。macro apply は
`<key>/<index>` idempotency suffix と step 毎 request id を維持する。
`syn.autx.visual{action:"close"}` は VCS と同じ spawned-view
teardown（`close_view_of_kinds`）を共有する。

native composition root（`src/bin/synvelis-native.rs`）は
`SYNVELIS_AUTX_VIEWS` env gate で 4 surface を `syn.autx.visual`
経由の実 op path から open し、返された `view_id` を
`ViewInstanceId` として解釈、正しい state/spec で mount して初回
`Ready` で attach、publish 更新を revision stamp で検出して
`update_view` する。既存の `AttachEpochAllocator` を共有し、
第 2 allocator は作らない。

## Context

Implements `docs/test-specs/autx-07-mount-wiring.md`（synvelis#266
SV266-NR-05 deferred case）。前提 slice は
`docs/impl/ir/autx-07-gui-views.md`（projection/decode/mount
helper、headless Pass）と `docs/impl/ir/vcs-04-native-mount.md`
（detached view spawn/attach/publish cell の先例）。

境界: live `synvelis-automation` adapter（#264）、実 GPUI/OS 受入
（#114/#267）、Script Editor 照合（#274）、theme parity
（SV266-NR-03）は範囲外。`synvelis-app` → `synvelis-gateway` の
直接依存は禁止（AC-05）のため catalog seam を workspace 側に置いた。

## Changes

- `synvelis-workspace/src/automation_ui.rs`: `AutomationCatalog`
  （`from_json`/`seq`/`capability_count`/`preset_count`/`is_empty`/
  `action_catalog`/`macro_browser`/`preset_at`/`capture`/`lower`）、
  `wire_request_json`/`wire_target_json`/`wire_consistency_json` による
  手書き wire 形 serialize（`WireRequest` は inbound deserialize-only
  のため）。macro view 型へ serde derive 追加。
- `synvelis-workspace/src/lib.rs`: `AutomationCatalog` export。
- `synvelis-app/src/automation.rs`: `AutomationScreenState` enum、
  `AutomationScreenPublish`（state + publish rev）、
  `automation_screen_spec`、`mount_automation_view`（variant dispatch）、
  `is_autx_target`、palette category filter（`palette_display_catalog`/
  `palette_categories`）。
- `synvelis-app/src/binding.rs`: `decode_view_command` の action
  `params` 許可を `view` + 5 `autx.*` target に拡張。domain action の
  params は従来どおり reject。
- `synvelis-app/src/app.rs`: `automation_catalog`/`autx_request_seq`/
  `automation_screens`/`autx_publish_rev` field、
  `automation_screen_cell()`、`publish_automation_screens()`
  （`publish_project_length` から呼出、登録 autx view のみ投影・
  closed view を retain で除去）。
- `synvelis-app/src/ui_ops.rs`: `AUTX_VIEW_KINDS`、`autx_surface`、
  `autx_err`、`op_autx_visual`/`open_autx_view`/`close_autx_view`/
  `close_view_of_kinds`（`close_vcs_view` から抽出した共有 teardown）、
  `op_autx_catalog`、`op_autx_action` + helper
  （`autx_view_instance`/`autx_catalog`/`autx_state_mut`/
  `autx_view_state_json`/`apply_autx_view_local`/
  `lower_autx_requests`/`cancel_autx_selected_run`/
  `autx_project_tag`）。
- `synvelis-app/src/service.rs`: `syn.autx.visual`/`syn.autx.catalog`/
  `syn.autx.action` の OPS 登録と dispatch。
- `synvelis-app/src/shell.rs`: `on_view_command` に `is_autx_target`
  分岐、`dispatch_autx_action`（`syn.autx.action` intent へ変換し
  到達 channel へ Result を返す）。
- `synvelis-app/src/bin/synvelis-native.rs`: `SYNVELIS_AUTX_VIEWS`
  gate、`MountSpec::Automation`、`open_autx_views`、automation screen
  cell の run-loop 追従（`BTreeMap<ViewInstanceId,u64>` rev stamp
  比較 — projection 型は `PartialEq` を持たないため）、
  `PendingMount::spec` の `Option` 化（publish entry 消失時に spec を
  捏造しない）。
- `synvelis-app/src/lib.rs`: automation 公開面の export 追加。
- tests: `sv266_wiring.rs`（12 test、mounted lane 含む）、
  `autx07_catalog_seam.rs`（10 test）。

## Verification

- `cargo test -p synvelis-workspace --test autx07_catalog_seam`:
  10 pass。
- `cargo test -p synvelis-app --test sv266_wiring`: 12 pass
  （op-level 10 + mounted dispatch 2）。
- `cargo test -p synvelis-app --test autx07_automation --test binding
  --test shell --test ui_ops`: 回帰なし。
- canonical lane は `mise run verify-full`（fmt-check / check-full /
  lint-full / test-full）+ `mise run check-headless-deps` +
  native feature lane（`check-app-native`/`lint-app-native`/
  `build-app-native`）— 結果は `evidence/sv266-wiring/README.md` と
  PR 本文に記録。

## Boundaries

- catalog は表示/request 入力であり authority ではない。grant・
  availability・target・consistency の最終検査は Gateway が行う。
- `ViewState.automation` が view-local state の唯一の正本。
  project truth・grant・job は GUI state に第二コピーを持たない。
- Workflow plan / Run monitor の producer は未接続（#264）。両
  surface は honest empty state を publish し、run select/cancel は
  monitor projection 不在を typed reject で表明する。
- request lowering の結果は wire request JSON であり、実行・
  receipt は caller の gateway lane の責務 — op は fake success を
  返さない。
