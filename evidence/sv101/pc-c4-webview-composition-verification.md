# PC-C4 webview composition verification results (Issue #101 / synvelis#101)

OS: Windows 11（worktree `sv101-webview-compose`）。toolchain は
`mise.toml` 管理（Rust stable、edition 2024、CI pin 1.99.0）。
canonical = `mise run` 経由。cargo 直接実行は補助証拠。

slice: `synvelis`（webview）bin への routing engine composition
配線（`docs/specs/pc-c4-webview-composition.md`）。

## 実行結果

| コマンド | OS | toolchain | exit | 結果 |
| --- | --- | --- | --- | --- |
| `mise run fmt-check` | Windows | mise rust stable | 0 | Passed（`cargo fmt --all -- --check`） |
| `mise run check synvelis-app` | Windows | 同上 | 0 | Passed（`--locked --all-targets`、headless lane） |
| `mise run check-app-webview` | Windows | 同上 | 0 | Passed（`--features webview --all-targets`。`synvelis.rs` bin compile を含む） |
| `mise run lint synvelis-app` | Windows | 同上 | 0 | Passed（clippy `-D warnings`。`doc_markdown` 指摘 2 件 — `composition.rs` doc comment・test `//!` 内の裸 `NotRun` — を backtick 修正して解消） |
| `mise run lint-app-webview` | Windows | 同上 | 0 | Passed（`--features webview` clippy `-D warnings`） |
| `mise run test synvelis-app` | Windows | 同上 | 0 | Passed（全 test target green。`pc_c4_webview_composition` 3 test を含む） |
| `mise run test-app-webview` | Windows | 同上 | 0 | Passed（`--features webview` 全 test target green） |
| `mise exec -- prek run --all-files` | Windows | 同上 | 0 | Passed（trim/eof/yaml/toml/utf-8bom 等全 hook） |

## 補助証拠

- `cargo test --locked -p synvelis-app --test
  pc_c4_webview_composition`: 3/3 pass
  （`project_dir_gate_keeps_engine_stub`・
  `project_dir_attaches_engine_and_control_surface`・
  `project_recreate_detaches_engine`）。反復用の早期確認であり、
  canonical 記録は上記 `mise run test synvelis-app` /
  `test-app-webview`。

## 検証した契約

- gate 閉（`compose_product_routing(app, None)` =
  `SYNVELIS_PROJECT_DIR` 未設定相当）: `syn.control.status` が
  `engine:false`・`generation`/`surface` null・
  `drain.attached:false` を返し、revision・`ui_pump` 後も stub
  のまま（意図された degrade であり fail ではない）。
- gate 開（`Some(dir)`）: composition 時点で `engine:true`、
  最初の session revision で generation 1 の control surface が
  attach、`ui_pump` が composition 登録の report drain を
  1 pass 駆動（`inactive_gated:true`・`drained:0` —
  RT 未観測のため host を呼ばない fail-closed）。empty plugin
  registry のため `frame_base`/`active_generation` は null の
  まま — 捏造しない。
- `project.new` recreate: engine・surface・drain evidence が
  session と共に reset され、dead session の evidence を残さ
  ない。再 attach は synvelis#106 の deferred 責務。

## 未検証（NotRun/Unsupported）

- `*-msrv`・`*-full`・Feature Matrix・`test-rt`・3OS 横断・
  Golden/長時間回帰: 本 slice の必須検証に含めず未実施
  （player 変更なし・Daily/Manual Full CI の責務）。
- `check-app-native`/`lint-app-native`/`build-app-native`:
  `synvelis-native.rs` は無変更・`composition.rs` の追加は
  feature 非依存 lib コードのため必須 lane 対象外。native
  feature build での compile は未実施（NotRun）。
- 実 VST3/AU/CLAP plugin backend・実 OS device backend・
  plugin GUI・実 WebView2 boot・wire-schema/L1・AudioLens
  f64・Domain Oracle: `external-gates.md` 参照。headless
  fixture の green を実機 Pass にしない。
- bin の `std::env::var_os("SYNVELIS_PROJECT_DIR")` 読み自体:
  process boundary のため fixture 対象外。`check-app-webview`
  の compile と code review で担保（test-spec Risk 節）。
