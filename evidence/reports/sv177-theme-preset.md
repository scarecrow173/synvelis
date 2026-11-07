# SV177 Worker Report — Driver-neutral Product Theme / Workspace Presets

- Run: run_e9e66b833704 / Task: task_ab87061835b1 / Dispatch: ctx_0a5d4791c4ef
- Repo: scarecrow173/synvelis / Branch: feature/native-product-theme-workspace-presets
- Base SHA: d8bf8cab982b2ee539c29d468b0ebd347dedf714（origin/main と同一、merge 不要）
- ruviform pin: rev 49de02c（ResolvedTheme 未公開 → seam のみ）
- OS/toolchain: Windows / rust 1.99.0 (workspace pin)

## 実施内容

- `synvelis-theme` leaf crate 新設（serde/serde_json のみ）:
  DTCG 文書 parse（`$extensions["synvelis.productTheme"]` envelope）、
  fail-closed validation（known `$type` subset・value shape・alias 構文・
  同一文書内循環）、`ThemeCatalog`（layered `documents_for`）、
  `ThemePreferences`（独立 schema）、`ResolvedThemeSink`/
  `UnresolvedThemeResolver` seam（明示 `Unsupported`、第2 interpreter 不作）。
- `assets/themes/` 5 DTCG 文書（base/dark/light/compact/comfortable）。
  dark は既存 style 値 literal 化、light は provisional provenance 明記。
- `synvelis-workspace::preset` 新設: PRESET_SCHEMA=2 slot 形式、
  `PresetViewState` 部分集合（session/entity 状態排除）、
  `apply_preset`（破壊前 `LayoutSnapshot::restore` 検証 → 全置換 →
  link/focus/detached 復元）、`from_snapshot`/legacy import。
  `ViewType::PluginDock` 語彙追加（host 実装は SV176 所有）。
- `assets/workspaces/` 3 preset（editing/mixing/plugin-dock）。
- `tools/check-headless-deps.mjs` に `synvelis-theme` 登録（9 crates）。
- Doc chain: discovery → spec(implemented) → ADR(accepted) →
  design(approved) → plan(approved) → test-spec(approved) →
  task(done) → IR(completed)。index 全同期済み。

## Review protocol

- 独立 Reviewer A（cooperative）/ B（adversarial）subagent 実施。
- Validator/Judge 確認済み blocking findings を全修正:
  - 破壊的 window close 前の `LayoutSnapshot::restore` 明示検証へ変更
    （`restore_or_default` の silent layout wipe 解消）
  - preset state を `PresetViewState` 部分集合へ限定
    （session/entity 状態・実 ViewInstanceId の template 混入排除）
  - catalog を `TokenTree` 保持 + `documents_for` 層選択へ変更
    （vacuous resolve・暗黙 merge 解消）
  - `synvelis-theme` を headless-deps gate へ登録
  - dark token literal 化、focus/link/dock validation 強化、
    alias edge case（`{` 始まりのみ BadRef、`"Fira {Code}"` literal）、
    doc-drift 同期

## 実行済み検証（実測）

| 検証 | コマンド | 結果 |
| --- | --- | --- |
| fmt | `mise run fmt` / `fmt-check` | Pass |
| lint | `mise run lint`（workspace clippy -D warnings） | Pass（warning 0） |
| theme tests | `cargo test -p synvelis-theme` | Pass（11/11） |
| workspace tests | `cargo test -p synvelis-workspace` | Pass（preset 14 含む全 suite、0 fail） |
| headless gate | `node tools/check-headless-deps.mjs` | Pass（9 crates、GUI deps なし） |
| workspace test | `mise run test`（--locked、jobs=2） | Pass（workspace 全 suite 0 fail） |

## 未検証 / NotRun

- Native 受入（実 GPUI window・実 plugin editor・実 OS window 配置・
  pixel/golden）: **NotRun**。RF390/RF313/RF314 + SV114 gate 待ち。
- RF385 `ResolvedTheme` 消費: **Unsupported**（pin 49de02c に型未公開。
  `UnresolvedThemeResolver` が明示 error を返す seam のみ）。
- 真の schema-1 preset ファイル移行: 既知 limitation
  （legacy import は現行 `WorkspaceSnapshot` 形状のみ受理）。
- preset apply の mutation 開始後 host/dock 失敗時は部分的 layout
  残置の可能性（view/dock 不変は維持、doc 明記済み）。

## 残 Gate / Follow-up

- RF385 公開契約着地時に `ResolvedThemeSink` 実装 adapter で接続
  （DTCG profile/envelope 再照合）。
- Native design review（light theme 値の確定、screen spec の実機検証）。
- Preset apply atomicity 強化は将来改善候補。
