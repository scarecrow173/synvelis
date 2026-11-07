---
id: "TASK-pIf6c5QRXRonGNwoqfvJgY"
type: "task"
status: "done"
title: "SV175-02: Native view mount — 最小 Tab/Panel/Fader 縦slice"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/175"
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/view_mount.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis-native.rs"
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "ui/vendor/ruviform"
  implements:
    - "docs/plans/sv175-native-shell.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/designs/sv175-native-shell-design.md"
    - "docs/tasks/sv175-01-native-shell-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv175-02-native-view-mount.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv175-02-native-view-mount.md"
  references: []
  defers: []
  deferred-by: []
---

# SV175-02 実装タスク

SV175-01 で `NotRun` と記録した最小 Tab+Fader slice を、RF387
(`68bb8710` で実装、現 lock は `79d1db0a`) の実 API で実装する。#112 の全4画面完成・#113 一般
Dock・#176 PluginDock 受入は開始条件にしない。

## 変更点

- `crates/synvelis-app/src/screens.rs` 新設: `main_screen_spec()`
  (`Panel > Tabs`、Transport tab の `Seek`→`transport/position`、
  Mixer tab の `Fader` を `unsupported` マウント)、
  `main_view_instance()`/`main_view_id()`(`ViewInstanceId(1)` →
  `"view-1"`)、`main_view_targets()`、`mount_main_view()`。
- `crates/synvelis-app/src/app.rs`: `SynvelisApp` に共有 cell の
  session-owned `AttachEpochAllocator`(`attach_epoch_allocator()`/
  `mint_attach_epoch()`)。app が `AppService`/`UiRuntime` へ封入
  された後も composition root 側から mint 可能。
- `crates/synvelis-app/src/shell.rs`: `NativeShell` が allocator
  handle を app から capture し `attach_view()` を追加 — window
  作成後に `syn.view.attach`+`syn.control.subscribe` を runtime
  dispatch 経路で発行し、`views`/`view_generation` を従来通りの
  result 観測で reconcile + outbox drain。
- `crates/synvelis-app/src/bin/synvelis-native.rs`:
  `mount_main_view` → `GpuiDriver::create_view_window` → `Ready` で
  `attach_view`。製品 theme は `synvelis-theme` カタログ
  (`synvelis/dark/comfortable`) を embed した DTCG 文書から解決し
  `ViewMount.theme` へ(RF385 解釈は upstream に委譲、解釈実装は
  持たない)。`synvelis-theme` を `native` feature の optional dep
  として追加。
- `crates/synvelis-app/tests/view_mount.rs` 新設: FakeDriver 上で
  mount→attach→subscribe→projection→gesture(Begin/Preview/Commit/
  Cancel)→receipt→save/reload→close→teardown→再 attach/host 再生
  の意味照合。`deliver_to_host` は `RvfView::drain_inbox` と同一の
  `Result`→`CommandReceipt`/`Notify`→`StateProjection` decode を
  再現し、Fake/GPUI 両 lane が同一 wire を踏むことを保証する。
- `Cargo.lock`: ruviform `27698cb1` → `68bb8710`(RF406) → `79d1db0a`(RF409/RF411 含む main、`test-msrv` 1.89 実測 exit 0)。
  `ui/vendor/ruviform` を `sync-ui-deps` で同 REV に再生成。

## 設計決定

- `control_id == target` を mount 側の規約とする:
  `notify_to_view_event` は `control_id = target` で投げ、
  `ViewHost::apply_projection` は `control_id` で相関するため、
  乖離させると projection が黙って落ちる。
- Mixer `Fader` は `unsupported: true`: 起動 project には
  `mixer.*` param target が存在しない(`syn.control.targets` は
  `transport`/`transport/position` のみ)。存在しない target へ
  command を mint させない正直な「未対応」表示。RF410 P2 の
  shared-`control_id` 未確定問題を避けるため全 control_id を
  一意にする。
- `attach_view` は subscribe 失敗を `warn` に留める(attach 成功は
  Err 伝播): view mount は attach が必須、subscription は target
  単位の degradable 要素。
- Epoch と logical id の分離: `ViewInstanceId`(永続 identity、
  `origin_view = "view-N"` へ写像)と `AttachEpoch`(runtime 保証、
  session allocator が mint)は別物。host 再生時は同一
  allocator から新 epoch を mint し、command-id namespace
  (`view-{id}:a{epoch}:m{mount}:{control}`)が dead namespace に
  隔離される。

## 完了条件

- `mise run fmt`/`check`/`lint`/`test` green、`check-app-native`/
  `lint-app-native` 実測。
- `tests/view_mount.rs` 全件 green(FakeDriver 意味照合)。
- 実 GPUI 起動 smoke(mount 描画・close)を実行可能範囲で実施し
  evidence 保存。OS render/input/IME/GPU の未観測項目は
  `NotRun`/`Blocked` +理由で記録、`Pass` にしない。

## 残存・境界

- RF410 P2:`ViewHost::command_index`/`CommandBinding::outbox` の
  retention は本 slice の fader 利用(有限 gesture)では実害なし、
  `push_event` 512 cap の drop、projection 相関が `control_id`
  のみ(reattach 後の最初の projection は旧 revision も一度は受け
  入れ自己修復)は既知制限として RF issue 境界で記録。恒久
  workaround は入れない。
- IME=`Unsupported`、`native_surface_slot`=backend 依存は
  capability 出力通り。実 OS/製品 E2E・親 Issue #175/#112 完全
  受入は未達。
- 一般 Dock(#113)・PluginDock(#176)・実外部 editor 受入は後段。
