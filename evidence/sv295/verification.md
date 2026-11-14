# Verification — synvelis#295 RCP-06 headless/driver-neutral 投影スライス

Date: 2026-11-15. Branch: `feat/295-rcp06-render-gui`
(base `origin/main` = `c147ad6`)。

## 計測環境

| 項目 | 値 |
| --- | --- |
| OS | Windows 11 (Git Bash) |
| Toolchain | mise 管理 `rustc 1.99.1`（`mise exec` 経由） |
| 検証入口 | `mise run` タスクのみ（cargo 直接実行は補助として clippy 事前確認に使用） |

## 実行結果（実測）

| コマンド | 結果 | 内容 |
| --- | --- | --- |
| `mise run fmt-check` | Pass (exit 0) | `cargo fmt --all -- --check` |
| `mise run check synvelis-workspace` | Pass (exit 0) | `cargo check --locked --all-targets` |
| `mise run lint synvelis-workspace` | Pass (exit 0) | `cargo clippy --locked --all-targets -p synvelis-workspace -- -D warnings` |
| `mise run test synvelis-workspace` | Pass (exit 0) | `tests/rcp06_render_ui.rs` 17/17 green を含む全 workspace test |
| `mise run check synvelis-app` | Pass (exit 0) | 同上 |
| `mise run lint synvelis-app` | Pass (exit 0) | 同上 |
| `mise run test synvelis-app` | Pass (exit 0) | `tests/rcp06_render.rs` 11/11 green を含む全 app test |
| `mise run check synvelis-theme` | Pass (exit 0) | 同上 |
| `mise run lint synvelis-theme` | Pass (exit 0) | 同上 |
| `mise run test synvelis-theme` | Pass (exit 0) | `status::tests` 4/4 + `theme_contract` 14/14 green |
| `mise run check synvelis-render` | Pass (exit 0) | 同上（render crate は read-only 消費・変更なし） |
| `mise run lint synvelis-render` | Pass (exit 0) | 同上 |
| `mise run test synvelis-render` | Pass (exit 0) | 同上 |

下流 build integrity: `synvelis-workspace`/`synvelis-theme` の
直接依存先は `synvelis-app` のみ（`crates/*/Cargo.toml` 確認済み）
で、`check`/`lint`/`test synvelis-app` が緑のため網羅済み。

13 レーン連続実行の最終 exit code: **0**（`fmt-check` 失敗・
clippy 失敗を 2 度検出・修正後の再実行で全緑）。

## doc 検査

- `docs/impl/ir/README.md`・`docs/test-specs/README.md`:
  `doc-maintenance` Skill の `doc_maintenance.js` で再生成
  （`docs/impl/exp` index は file 列挙型でないため in-sync、
  書込なし）。手編集なし。
- `doc_status.js lint`: 既存 repo 全体に 77 件の blocking
  finding（前置から存在・本スライスと無関係）。本スライスの
  文書に固有の finding は `test-spec-invalid-verifies-target`
  1 件のみで、`verifies` から impl 文書を外して解消済み。
  `invalid-id-prefix` は既存 36 文書と同一の ID 慣行
  （`TSPEC-Rcp06RenderGuiTestX` は `TSPEC-Rcp03…` と同型）で
  あり、慣行変更は本 lease 範囲外として未対応。

## NotRun / Deferred（Case ID 付き・acceptance-ledger.md 参照）

- `RCP-UX-06-GPUI`: 実 GPUI/実 OS/IME/DPI/2Window/pixel/RT —
  専用 real GUI 自動レーンなし。NotRun。
- `RCP-UX-04-CLI`: 実 CLI/Workflow/Luau/Rhai/Fake AI surface —
  sv#273 lease。Deferred（typed action/grant/reason 契約の
  fixture 検証まで実施済み）。
- 実 Plugin・実 VCS provider・PCM Golden・AudioLens Consumer:
  NotRun（`RCP-UX-01-RT`）。cross-gate RCP-07: Deferred。
- `*-msrv` レーン: タスク指定により未実行。
