---
id: "IMPL-Sv89Alib05NativeMount01"
type: "impl"
status: "in-progress"
title: "ALIB-05 native mount — 製品 composition への実 provider/import 配線 + env-gated Asset Library view mount（synvelis#89 ALIB-UX-OS enabler slice）"
created: "2026-11-24"
updated: "2026-11-24"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/89"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/library_native_mount.rs"
      - type: "file"
        path: "docs/impl/ir/alib-05-native-mount.md"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis-native.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/uitest.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/common/library.rs"
      - type: "file"
        path: "docs/test-specs/alib-05-browser-import-ux.md"
      - type: "file"
        path: "evidence/sv89/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv89/verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/asset-library.md"
    - "docs/designs/asset-library-design.md"
  implemented-by: []
  depends-on:
    - "docs/test-specs/alib-05-browser-import-ux.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/alib-05-browser-import-ux.md"
    - "docs/test-specs/asset-library.md"
    - "evidence/sv89/acceptance-ledger.md"
    - "evidence/sv89/verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/asset-library-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "crates/synvelis-app/tests/library_native_mount.rs"
  references:
    - "evidence/sv89/acceptance-ledger.md"
    - "evidence/sv89/verification.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/91"
    - "https://github.com/scarecrow173/synvelis/issues/90"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  deferred-by: []
---

# ALIB-05 native mount — env-gated 実 backend + verbatim spec mount（synvelis#89）

## Summary

`synvelis-native`（GPUI 実 shell binary）の製品 composition に
Asset Library の mount 経路を追加した。`SYNVELIS_LIBRARY_DIR`
で実 `LocalFolderProvider` を、`SYNVELIS_PROJECT_DIR` で実
`ImportStore`/`LocalBlobDir`/`StagedBlobPublisher` import
ports を app に bind し、`SYNVELIS_ASSET_LIBRARY` gate が
`syn.library.visual{action:"open"}` → `syn.library.view` の
返却 ViewSpec を verbatim で `ViewHost`/`MountSpec::Library`
に mount する。mount 形は `mount_variant_diff_view` と同型
（wire spec を第二の形へ再解釈しない）。shell は
`synvelis.library` target の action を `syn.library.action`
へ route し、`syn.library.*` dispatch で dirty mark →
run loop / uitest adapter が spec 再読み → 変更時のみ
`update_view` push する。

## Context

synvelis#89（ALIB-05 / ALIB-UX-OS enabler slice）。ALIB-UX-H
headless slice（PR #485/#489）で `library_ui` view-model・
`syn.library.*` 4 op・driver-neutral `library_spec`・注入点
`set_library_backend`/`set_library_import` は main に着地
済みだが、注入点は test fixture からしか呼ばれておらず製品
composition は未配線だった。本 slice はその有効化と、
env-gated な detached mount（VCS-04 `open_vcs_history_view`
と同型の opt-in）を担う。実 OS GUI 受入（#114）・実
provider network/credential（#90/#91）・v5 実データ接合
（#86 系）は別 gate であり、本 slice の成功はそれらを意味
しない。

## Implementation

### Changes

- `crates/synvelis-app/src/bin/synvelis-native.rs`:
  - env 契約: `SYNVELIS_LIBRARY_DIR`（provider root）、
    `SYNVELIS_PROJECT_DIR`（import ports root・既存）、
    `SYNVELIS_ASSET_LIBRARY`（mount opt-in gate）を文書化。
    `env_flag` 先例どおり truthy 判定。library 層に
    `std::env` を持ち込まない — root の canonicalize・存在
    検証は bin の process boundary で行う。
  - `compose_library_backend`/`compose_library_import`:
    env 値が未設定または不正な folder を指すときは
    backend/ports を unbound のまま返す（session を殺さない
    fail-closed — `syn.library.*` op が typed `unavailable`
    fault を返す）。
  - `open_library_view`: `app.op_library_visual
    {action:"open"}` → wire `view_id` → `ViewInstanceId` →
    `app.op_library_view{view_id}` の返却 spec →
    `MountSpec::Library(instance, Box::new(spec))` で
    `PendingMount` を構成（`mount_variant_diff_view` と同じ
    spec 保持型）。`open_windows` で `SYNVELIS_ASSET_LIBRARY`
    gate 時に pending mount へ追加。
  - `compose_app`: backend/ports を `set_library_backend`/
    `set_library_import` で注入。`run_loop` に dirty mark →
    `shell.library_view_spec` で `syn.library.view` 再読み
    → 返却 spec が変わったときのみ `update_view` push の
    refresh tail を追加。

- `crates/synvelis-app/src/screens.rs`:
  - `MountSpec::Library(ViewInstanceId, Box<ViewSpec>)` を
    追加（`ViewSpec` 328B の `large_enum_variant` 対応で
    `ImportTargetWire` と同じ `Box` 化）。`PendingMount::
    spec`/`mount` 経路は variant-diff と同型 — 保持 spec を
    そのまま返す。

- `crates/synvelis-app/src/shell.rs`:
  - `LIBRARY_TARGET = "synvelis.library"` const。
    `on_view_command` で同 target の Action intent を
    `dispatch_library_action` へ route（autx 判定の直後）。
    params `{view_id, action, ...}` を `syn.library.action`
    へ `rt.dispatch_op` で forward。
  - `library_row_locator`: entries table の `select{row}`
    は op が期待する `entry.select{locator}` と形が違う
    （`LibraryAction` は `#[serde(tag="action",
    deny_unknown_fields)]`）ため、live `syn.library.view`
    spec を `rt.dispatch_op` で再評価し `ComponentSpec`
    tree から `library.entries` table の `locator` 列を
    row index で引いて解決する。Button の params は
    verbatim pass-through。
  - `library_dirty` flag: `syn.library.*` dispatch と
    `dispatch_library_action` で mark。`take_library_dirty`
    /`library_view_spec` を追加。`on_windowed` の肥大に
    対して `dispatch_window_action` を抽出
    （`too_many_lines` 対応）。

- `crates/synvelis-app/src/uitest.rs`:
  - `SynvelisTestApp::on_event` に run loop と同一の
    dirty→re-read→push semantics を追加（uitest adapter は
    native run loop の配線を TestApp として表現する契約
    — mount 種別追加時は同意味論に追従）。

- `crates/synvelis-app/tests/common/library.rs`:
  - `app_with_local_provider` helper を追加 — temp-dir
    fixture root の実 `LocalFolderProvider` と temp-dir
    project media root の実 `ImportStore`/`LocalBlobDir`/
    `StagedBlobPublisher` を bind（fake 不使用・製品と同じ
    provider 種別で証拠を取る）。既存 helper は additive
    に保持。

- `crates/synvelis-app/tests/library_native_mount.rs`（新規）:
  7 test — wire spec verbatim mount・実 provider 投影・
  real import commit・project authority 非汚染・unbound
  fail-closed・NativeShell lifecycle/refresh・uitest
  adapter push（`uitest` feature gate）。

### Decisions During Implementation

- **push/update は (a) shell 側 `rt.dispatch_op` 再評価を
  採用**: task spec が挙げた (a)/(b) のうち、library model
  は sealed app の `LibraryViews` 内にあり shared screen-
  state cell を持たないため、(b) app 公開 cell 型への変更は
  `lib.rs`/`app.rs`（lease 外）を要する。`rt.dispatch_op`
  で `syn.library.view` を再評価する (a) は shell.rs 内で
  完結し、返却 spec が変わったときのみ push するため無駄
  push もない。
- **row→locator 解決は live spec walk**: table 発行の
  `select{row}` を `entry.select{locator}` へ変換する情報は
  wire spec 側の `library.entries` locator 列にあるため、
  再投影せず返却 spec を参照して解決する。
- **`binding.rs` admission はリース外として STOP→ask**:
  `decode_view_command`（`binding.rs:974-982`）が params
  付き Action intent を `view`/`autx.*` 以外で reject
  するため、mounted spec control 発行の `synvelis.library`
  action は現行 decode で落ちる。admission 拡張は
  `binding.rs` の最小変更（target admission + doc
  comment）として Coordinator に相談（ask 保留中）。

## Validation

`evidence/sv89/verification.md`「ALIB-05 native mount slice」
節にコマンド・exit code・結果を記録。`mise run check`/
`lint`/`test synvelis-app`、`check/lint/build-app-native`、
`check/lint/test-app-uitest`、`prek run --all-files` は全て
pass。`library_native_mount` 7/7（default feature では 6/6
+ uitest lane で gated 1 件）。`mise run fmt`（`cargo fmt
--all`）は `os error 206`（Windows コマンド長制限 —
workspace 全 member 単一 rustfmt 呼出し）で環境的に実行
不能。`cargo fmt --package synvelis-app -- --check` で変更
全ファイルの準拠を代替確認済み。

## Risks

- `binding.rs` admission 未解決のまま着地すると mounted
  Asset Library の全 spec control（Button/TableSelection）
  が dead になる — op surface 経路は全機能するため slice
  の受入は満たすが、UX 面のコマンド経路は follow-up 必須。
- refresh は `syn.library.*` dispatch / library action の
  dirty mark 起点 — op 外経路（engine notify 等）で
  library model が変わる変更が将来入った場合、mark 網羅を
  再確認する必要がある。
- `SYNVELIS_*` env gate は dev opt-in — 未設定時の挙動
  （unbound fail-closed）は test で固定済みだが、実 shell
  での起動手順は runbook 未整備。

## Follow-ups

- `binding.rs::decode_view_command` の `synvelis.library`
  params admission 拡張（Coordinator 判断 — 承認なら別
  commit で最小変更 + check/lint/test 再検証）。
- `syn.library.jobs` / `import.cancel` action /
  `CallOutcome::Job` 化 — synvelis#126 TIM-02（service.rs/
  app.rs 占有）着地後の直列 slice 候補。
- 実 OS GUI 受入（実 Window/別 DPI/focus/keyboard/IME/
  drag&drop）— synvelis#114 GUI-L2 gate。
- Git/network/credential provider の mount — synvelis#90/
  #91 領域（本 slice は `LocalFolderProvider` のみ）。
- synvelis#86 v5 実データ接合（Browser UX 経由の要否は
  uncertain として残す）。Ruviform/AudioLens consumer
  回帰 — cross-consumer gate 側の責務。
