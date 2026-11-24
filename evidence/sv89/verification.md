# sv#89 verification evidence — ALIB-05 Browser/Import UX headless view-model + syn.library.* ops + ViewSpec

Slice: `synvelis-workspace::library_ui` に Asset Library
Browser/Import UX の headless view-model（per-view
`LibraryViewModel`・typed wire DTO・`LibraryViews`
registry）と driver-neutral `ViewSpec` projection
（`library_spec`）を実装し、`synvelis-app` に
`syn.library.providers`/`visual`/`view`/`action` の 4 op
を配線。browse/coverage/search/stat/pin/preview/import/
drag ghost を typed state として投影し、unsupported/
offline/missing/stale/cancelled を empty・成功・generic
error に collapse させない。HEADLESS ONLY — 実 GPUI
mount（#114）・実 provider/network/credential（#91）・
v5 real-data import（#86）は対象外。

## Environment

| Field | Value |
| --- | --- |
| Synvelis commit | `7bedcbaa44cae7f496adf59452846324f5d94da0`（branch `scarecrow173/sv89-alib05-browser` base = origin/main HEAD）+ this change set（uncommitted） |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.1 (58cf8f9c3, mise managed; CI pin 1.99.0 in mise.ci.toml) |
| Cargo.lock Ruviform SHA | `ac10a2f7761c08db73aebd861c0a5c1f53f269b7`（変更なし — `synvelis-workspace` → `synvelis-library` dep edge のみ追加） |
| Schema | `SCHEMA_VERSION = 14` |
| Provider / Import ports | scripted in-memory `FakeProvider`（`tests/common/library.rs`）・temp-dir `ImportStore` + `LocalBlobDir` + `StagedBlobPublisher`。network/credential/実データ不使用 |

## Commands run (canonical = mise)

| Command | Exit | Result |
| --- | --- | --- |
| `mise run fmt` | 0 | pass — `cargo fmt --all` で整形適用 |
| `mise run check synvelis-workspace` | 0 | pass（`cargo check --locked --all-targets -p synvelis-workspace`） |
| `mise run check synvelis-app` | 0 | pass（`cargo check --locked --all-targets -p synvelis-app`） |
| `mise run lint synvelis-workspace` | 0 | pass（`cargo clippy --locked --all-targets -p synvelis-workspace -- -D warnings`、warning 0） |
| `mise run lint synvelis-app` | 0 | pass（`cargo clippy --locked --all-targets -p synvelis-app -- -D warnings`、warning 0） |
| `mise run test synvelis-workspace` | 0 | pass — 全 test binary 成功。`library_ui` unit 14 件を含む（`search_mode_survives_refresh_and_more`/`preview_cancel_does_not_poison_later_plan` regression 含む） |
| `mise exec -- cargo test -p synvelis-app --test library_browser` | 0 | pass — 16/16（op 登録/dispatch・view lifecycle・unbound backend typed・browse paging・search/stat/pin/preview・unsupported typed・provider switch invalidation・drag ghost・malformed params・verified import CommittedLive + idempotent replay・import boundary faults・stale-revision rebase/retry・backend persistence across project.new） |
| `mise run check-headless-deps` | 0 | pass — "no GUI framework deps in 12 headless crates"（`library_ui.rs` は `ruviform_ui::view` の driver-neutral spec 型のみ使用・GUI/GPU/WebView 依存なし） |
| `mise run check-app-native` | 0 | pass（`cargo check -p synvelis-app --features native --all-targets`。`proc-macro-error2 v2.0.1` の future-incompat warning は既存依存のもので本 change set と無関係） |
| `mise exec -- prek run --all-files` | 0 | pass（trailing-whitespace/end-of-file/check-yaml/check-toml/mixed-line-ending/byte-order-marker/detect-private-key 全 pass・check-illegal-windows-names は対象なし skip・actionlint pass） |

## Auxiliary evidence (canonical = mise, cargo direct)

`cargo test -p synvelis-app --test library_browser` —
16 passed / 0 failed（`mise exec --` 経由で実行。direct
cargo は開発中の反復確認用で、正式記録は mise 経路）。

Doc 仕上げ pass で direct cargo 相当を再確認:
`cargo test --locked -p synvelis-workspace`（全 suite
green・`library_ui` unit 14/14）、`cargo test --locked
-p synvelis-app`（全 suite green・`library_browser`
16/16）、`cargo check --locked --all-targets
-p synvelis-workspace`、`cargo clippy --locked
--all-targets -p synvelis-workspace -p synvelis-app --
-D warnings`、`cargo fmt --all -- --check`、
`node tools/check-headless-deps.mjs`、
`cargo check -p synvelis-app --features native
--all-targets` — 全て exit 0。

## Doc index 再生成

- `node .agents/skills/impl-doc/scripts/sync_impl_index.js
  --kind ir --apply` → `docs/impl/ir/README.md` に
  `alib-05-browser-import-ux.md` 追加（exit code は実行結果
  を確認）
- `node .agents/skills/doc-maintenance/scripts/
  doc_maintenance.js index apply --dir docs/test-specs` →
  `docs/test-specs/README.md` 更新
- `sync_impl_index.js --kind exp --apply` → exp `.md`
  memo は index 対象外（`.jsonl` のみ管理）のため
  IN-SYNC 見込み

## NotRun

- `*-msrv` 系・`lint-full`/`test-full`/`build-full`/
  `verify-full` — Daily Full CI 委譲（POLICIES.md §5:
  ローカル検証は変更範囲に比例）。
- `mise run check-app-native` 以外の feature lane —
  `lint-app-native`/`build-app-native`/`test-app-gpui`/
  `check-app-gpui`/`check-app-webview` 等は未実行
  （`check-app-native` は pass）。実 GPUI mount・実
  window/IME/accessibility は synvelis#114。
- 実 provider・実 network・実 credential store —
  synvelis#91（ALIB-07 fault matrix）と #88/#90/#331 が
  Owner。本 slice の provider は scripted `FakeProvider`。
- v5 real-data import・`inspect_imports` の stage-level
  manifest recovery op — synvelis#86。
- `syn.library.jobs` cross-view recovery ledger・
  `CallOutcome::Job` wrapping（mid-flight cancel の job
  thread 化）— deferred、別 slice。
- `LibraryCache`-backed preview（ALIB-03/#87 領域）。
- `test-rt`/`test-recording-real-input`/
  `test-recording-real-fs`/`test-project-real-fs` 等の
  real-device lane — 本 slice と無関係・hardware 依存の
  ため未実行。
- OS-L1/GUI-L2/EXT-L3 の Windows 実機 lane・macOS/Linux —
  POLICIES.md の利用条件成立まで Deferred。

## Known limitation（修復しない・正直記録）

- `syn.library.*` op は全て service thread 同期実行
  （`syn.vcs.*` と同じ discipline）。長時間 preview
  fetch/import で thread を占有し得る。preview read loop
  は協調 `CancellationToken` を chunk 間で見るため、
  `CallOutcome::Job` 化は contract 変更なしに後続可能。
- preview は `provider.open_preview` 直読み（bounded・
  unverified、`Ready{cached:false}` 固定）—
  `LibraryCache` 経由 preview ではない。
- `import.submit` の `Io`/`Corrupt`/`PublishFailed`/
  `JobState` `ImportError` は `ImportUiState::Failed
  {reason:"internal"}` に丸められる（`IntegrityMismatch`/
  `CommitRejected`/`PublishIndeterminate`/`Provider`/
  `StaleRevision`/`IntegrityConflict` は typed）—
  stage manifest の `Recoverable` resume 等の細かい
  typed 回復は `syn.library.jobs` slice の責務。
- `ProviderContext` は `LibraryBackend` が principal のみ
  を持ち budget/deadline/credential は未配線 — 実
  provider 配線（#91）で拡張が必要。
- `import.cancel` action は存在しない — 同期 op のため
  import 途中 cancel の UI 経路はなく、`ImportUiState::
  Cancelled` は provider/import 層の cancelled の写像のみ。
- `library_ui.rs` は view-model・action・wire DTO・ViewSpec
  projection・registry を単一モジュールに収める（約 3000
  行）。責務は型で分離済みだが、実 provider 配線・jobs
  ledger 追加時に分割を検討する余地がある。

## ALIB-05 native mount slice（2026-11-24、追記）

Slice: `synvelis-native` の製品 composition が env-gated で
実 `LocalFolderProvider`（`SYNVELIS_LIBRARY_DIR`）と実
`ImportStore`/`LocalBlobDir`/`StagedBlobPublisher`
（`SYNVELIS_PROJECT_DIR`）を bind し、`SYNVELIS_ASSET_LIBRARY`
gate で `syn.library.visual{action:"open"}` → `syn.library.view`
返却 ViewSpec を verbatim で `ViewHost`/`MountSpec::Library`
に mount する経路を headless 検証で固定。shell 側は
`synvelis.library` target action を `syn.library.action` へ
route し、`syn.library.*` dispatch で dirty mark → run loop /
uitest adapter が spec 再読み → 変更時のみ `update_view`
push。HEADLESS ONLY — 実 OS window/GUI（#114）は対象外。

### Environment

| Field | Value |
| --- | --- |
| Synvelis commit | `d151716e66d69e89cebbcd93aedb06f53c424906`（branch `feat/89-alib05-native-mount` base = origin/main HEAD）+ this change set（uncommitted） |
| OS | Windows (win32), NTFS |
| Toolchain | rustc 1.99.1（mise managed; CI pin 1.99.0 in mise.ci.toml） |
| Cargo.lock Ruviform SHA | 変更なし（synvelis-app crate 内変更のみ） |
| Provider / Import ports | 実 `LocalFolderProvider`（temp-dir fixture root — 製品と同じ provider 種別・fake 不使用）・実 `ImportStore`/`LocalBlobDir`/`StagedBlobPublisher`（temp-dir project media root） |

### Commands run (canonical = mise)

| Command | Exit | Result |
| --- | --- | --- |
| `mise run fmt` | fail | **`cargo fmt --all` が `os error 206`（ファイル名または拡張子が長すぎます）で失敗** — workspace 全 member を 1 回の rustfmt 呼び出し（約 30KB コマンドライン）で処理するため Windows コマンド長制限超過。本 change set 起因ではない環境制約（下記 NotRun 参照） |
| `mise exec -- cargo fmt --package synvelis-app -- --check` | 0 | pass — 本 slice が変更した全ファイル（synvelis-app crate）は rustfmt 準拠を確認 |
| `mise run check synvelis-app` | 0 | pass（`cargo check --locked --all-targets -p synvelis-app`） |
| `mise run lint synvelis-app` | 0 | pass（`cargo clippy --locked --all-targets -p synvelis-app -- -D warnings`、warning 0。`large_enum_variant` 対応で `MountSpec::Library` は `Box<ViewSpec>`、`too_many_lines` 対応で `dispatch_window_action` 抽出） |
| `mise run test synvelis-app` | 0 | pass — 全 suite green。`library_native_mount` 6/6（uitest-gated 1 件は下記 lane） |
| `mise run check-app-native` | 0 | pass（`cargo check -p synvelis-app --features native --all-targets`） |
| `mise run lint-app-native` | 0 | pass |
| `mise run build-app-native` | 0 | pass — `synvelis-native` binary link 成功 |
| `mise run check-app-uitest` | 0 | pass（`--features uitest`） |
| `mise run lint-app-uitest` | 0 | pass |
| `mise run test-app-uitest` | 0 | pass — `library_native_mount` 7/7（`uitest_adapter_pushes_refreshed_library_spec` 含む） |
| `mise exec -- prek run --all-files` | 0 | pass — 全 hook green |

### NotRun（本 slice・正直記録）

- `mise run fmt`（`cargo fmt --all`）— `os error 206` で実行
  不能。Windows のコマンドライン長制限に由来する環境制約で、
  base commit でも同一（本 change set 無関係）。代替として
  `cargo fmt --package synvelis-app -- --check`（変更全
  ファイルを含む）で rustfmt 準拠を確認済み。
- 実 OS window/GPU/IME/focus/keyboard の受入 — synvelis#114
  deferred（headless 検証のみ）。
- `synvelis.library` mounted control のコマンド経路 —
  `binding.rs::decode_view_command` が params 付き Action
  intent を `view`/`autx.*` 以外で reject するため、spec
  control 発行の action は decode 段で落ちる（shell routing
  は実装済み・op 駆動検証は green）。admission 拡張は
  binding.rs（本 slice リース外）への最小変更として
  Coordinator に相談中 — 結果は PR・ledger に反映。

### Known limitation（本 slice・正直記録）

- 上記 NotRun と同じ binding admission 制約 — mounted
  spec control（Button/TableSelection）からの
  `synvelis.library` action intent は `binding.rs` の現行
  admission では decode 時に `invalid_params` reject。
  ライブラリ操作は op surface（`syn.library.action` 直接
  dispatch）経路では完全に機能する。
- `SYNVELIS_LIBRARY_DIR`/`SYNVELIS_PROJECT_DIR`/
  `SYNVELIS_ASSET_LIBRARY` は dev 向け env gate —
  未設定時は backend unbound のまま typed `unavailable`
  fault を返し、session は継続する（fail-closed・殺さない）。
