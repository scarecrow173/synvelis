# synvelis#176 S0 — ruviform pin bump review

Slice S0 (SV-GPUI-02 前提整備): `Cargo.lock` の ruviform pin を upstream `main` HEAD へ前進させ、
`ui/vendor/ruviform` を新 pin で再同期した。本メモは upstream 差分評価と検証結果を記録する。

## Pin 変遷

| 項目 | old | new |
| --- | --- | --- |
| ruviform (30 package 単一 SHA) | `cd4213a1d665600f6adfefa0c3a90f2be9b4e4ce` | `20dc41f71c47674b3e51d1eb0921124b94f8bfaf` |
| gpui (transitive via ruviform-ui-gpui) | `?tag=v0.2.2-rf1#f77e44e64b66c1bcc4757e68ee5dfc903fb96139` | `?tag=v0.2.2-rf2#b3144f286b942fbb5f478da73db13a826cef054a` |

- 新 pin `20dc41f7` は実行時点の upstream `origin/main` HEAD。`merge-base --is-ancestor` で
  `ee66bd57`（必要含有点）および `cd4213a1`（旧 pin）の双方の子孫であることを確認済み。
- `Cargo.lock` diff は 31 行の `source =` fragment 更新（ruviform×30 + gpui×1）のみ。
  crates.io version drift・新規 package・依存追加はゼロ。
- `Cargo.toml` 変更なし（既定ブランチ追従規約どおり pin の正本は lock のみ）。

## Upstream 差分評価（cd4213a1..20dc41f7）

157 commits / 583 files (+81498/-6515)。大半は `evidence/`・`docs/`・CI・test lane 記録。
コード差分は `crates/` 配下 101 ファイル、`ui/` は **ゼロ**（vendor 内容は REV 以外同一）。

### 消費 crate（synvelis lock 含有）の差分

| crate | 差分 | consumer 影響判定 |
| --- | --- | --- |
| ruviform-plugin-core | `WIRE_VERSION` 5→8（v6 GUI floating 動詞族 tag 24-28 + `Hello.capabilities` GUI_FLOATING bit、v8 tag 29/30 `SetGuiFocusTarget`/`SetGuiPopup`）。boundary 語彙 `GuiFocusTarget`/`GuiPopupKind`/`GuiRequest`/`GuiRequestVerdict`/`GuiRequestRefusal`（rf#630 editor verb）、`DirtyNotice`/`DirtyNoticeQueue`（rf#891 state dirty 通知）。`Plugin` trait に GUI/drain 系メソッド追加（**全て default 実装付き**） | 影響なし。`WIRE_VERSION` は host/plugin 両端が同一 rev の wire protocol 内部値で synvelis は非参照。trait 追加は default 付きで implementor 非破壊 |
| ruviform-plugin-host | `EditorSessionOps` に `set_focus_target`/`set_popup`/`drain_gui_requests`/`adjudicate_gui_request`/`dispatch_gui_requests` 追加（**全て default 実装**）。`L1EditorOps` unit struct→`L1EditorOps<'a>`（stateful・L1 floating 動詞族の実駆動 — rf#630/631 seam の到達）。`GuiRequestArbiter`/`GuiRequestArbiterHandle`/`pump_notifications` 新規 export。`PluginHost` に `set_gui_request_sink`/`drain_gui_requests`/`drain_dirty_notices`/`negotiated_gui_capabilities` 追加。`HostCommand::OpenGui` に `size: GuiSize` field 追加、`HostReply::Opened` に `capabilities: u32` field 追加（両 enum `#[non_exhaustive]`）、variant `SetGuiFocusTarget`/`SetGuiPopup` 追加 | **破壊的形状変更あり・synvelis 非影響**。synvelis-app は `EditorSessionOps` を `OpsBridge` で impl するが新規メソッドは全て default のため impl 変更不要。`HostCommand::OpenGui`/`HostReply::Opened` の構築・非ワイルドカード match は synvelis 内に存在しない（`crates/` grep 確認）。`L1EditorOps` は doc comment 参照のみで具象使用なし。既存 `PluginEditorSessions` メソッド群のシグネチャ変更なし |
| ruviform-ui | `CommandParam`/`CommandParams`/`dispatch_action` mint 経路（rf#944 typed action params）— 追加のみ | 影響なし（追加のみ） |
| ruviform-ui-gpui | `gpui` tag `v0.2.2-rf1`→`v0.2.2-rf2`（**rf#532 DComp non-topmost fix** — SV-GPUI-02 前提）。新 module `gpui_driver::os_ime`（rf#389 IMM32 IME composition drive/観測、Windows のみ・他 OS `Unsupported`）。`gpui_driver/surface.rs`・`real_window.rs` 更新。`windows-sys` に `Win32_UI_Input_Ime` feature 追加（既存 dep の feature 追加で lock package 数不変） | 影響なし（追加 + gpui pin 前進は本 slice の目的物） |
| ruviform-ui-test | `ScenarioCapability::OutOfProcessPluginHost` variant 追加（rf#886 L1 host route lane 語彙） | 影響なし（追加のみ） |
| ruviform-io-wav | wire format gating・convert 追加（`pub(crate)` harness + wire gate） | 影響なし（公開 API 削除なし） |
| ruviform-audio-test | `benches/frozen`・Cargo.toml のみ（dev-side） | 影響なし |
| 残り 23 crate | src 差分なし（lock rev のみ更新） | — |

### dep-tree-external（synvelis lock 非含有）の差分

`ruviform-plugin-clap`/`ruviform-plugin-vst3`/`ruviform-plugin-test-clap`/
`ruviform-plugin-test-vst3`/`ruviform-plugin-dock-test`/`ruviform-plugin-export`/
`ruviform-io-structured`/`ruviform-io-archive` — rf#630 editor verb の
VST3/CLAP adapter・test plugin fixture（`IRvfEditorOps` seam・fault mode 拡張）と
dock-test real lane。synvelis 依存木に入らないため本 slice では非影響として分離。
これらは SV-GPUI-02 後続 slice が dock 対象 test plugin として利用する upstream 成果物。

### 判定

- **consumer 影響のある破壊的差分: なし**（静的評価 — `--locked` 全 lane の compile 通過が実証）。
- 形状変更が存在する API（`OpenGui.size`・`Opened.capabilities`・`L1EditorOps<'a>`・
  `WIRE_VERSION` 値）はいずれも synvelis が構築・参照しない面。trait 追加は全て default 実装。
- 新規 upstream 機能（editor verb family・IME・typed action params・OutOfProcessPluginHost lane
  語彙）の採用は本 slice では行わない（YAGNI — 後続 slice で個別に採用判断）。
- `ruviform-ui` は lock に残存（`crates/synvelis-app/build.rs` の `SYNVELIS_RUVIFORM_REV` 埋め込み契約を維持）。

## 検証記録

実施日: 2026-11-25 / OS: Windows / 検証入口: `mise run <task>`（`mise.toml` 正本）。

| Lane | Command | Result | Note |
| --- | --- | --- | --- |
| pin/vendor 整合 | `mise run sync-ui-deps` | Pass | `synced ruviform ui/* @ 20dc41f71c` |
| pin/vendor 整合 | `mise run check-ruviform-rev` | Pass | `resolved 20dc41f7 (30 lockfile sources, lockfile-primary, REV match)` |
| pin/vendor 整合 | `mise exec -- node tools/check-ruviform-rev.mjs --require-vendor` | Pass | 同上 |
| baseline 記録 | `mise run check-ruviform-rev`（bump 前・作業開始時） | Fail（想定内） | stale vendor `20a24a66` != lock `cd4213a1` — synvelis#230 の fail-closed 検出。`sync-ui-deps` で復旧済み |
| 衛生 | `mise run fmt-check` | Pass | exit 0 |
| 衛生 | `mise exec -- prek run --all-files` | Pass | exit 0 |
| workspace 全 member check（synvelis-app headless 既定 lane 含む） | `mise run check-full` | Pass | exit 0（`cargo check --workspace --all-targets --locked`） |
| workspace 全 member lint | `mise run lint-full` | Pass | exit 0（`clippy -D warnings`） |
| workspace 全 member test | `mise run test-full` | Pass | exit 0（全 suite 0 failed） |
| native feature（GPUI Build/Link — AC 行 4 L1 観点） | `mise run check-app-native` / `lint-app-native` / `build-app-native` | Pass | 各 exit 0。`synvelis-native` bin が gpui `v0.2.2-rf2` で link |
| gpui-test feature（GPUI-simulated lane） | `mise run check-app-gpui` / `lint-app-gpui` / `test-app-gpui` | Pass | 各 exit 0 |
| uitest feature（`rvf.uitest.v1` seam） | `mise run check-app-uitest` / `lint-app-uitest` / `test-app-uitest` | Pass | 各 exit 0 |
| webview feature | `mise run check-app-webview` / `lint-app-webview` / `test-app-webview` | Pass | 各 exit 0 |
| contract: rt_ | `mise run test-rt` | Pass | exit 0 |
| contract: headless 依存木 GUI 混入検査 | `mise run check-headless-deps` | Pass | exit 0 |
| contract: real-device feature compile/lint | `mise run lint-recording-real-input` | Pass | exit 0（lint のみ — 実機不要求） |
| opt-in 実機/実 FS/実窓 lane（`test-recording-real-input`・`test-recording-real-fs`・`test-project-real-fs`・`test-app-uitest-ctl`・`test-formal-kani`・`bench-frozen`） | — | NotRun | 本 slice は pin 整備のみで実機/実 GUI/実 Plugin 機能の利用条件は発生しない（POLICIES §2/§5 — GUI-L2/EXT-L3 の Required case なし）。対象 SHA の Daily Full CI が統合検証証拠 |

※ `frontend/dist` は gitignored 生成物で worktree に未存在。`check-ruviform-rev` が manifest 存在時のみ照合するため現状態で green。`build-ui` は不要求（dist 生成なし・差分にも出ない）。
