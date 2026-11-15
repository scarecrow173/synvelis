# SV-GUI-03-07 keymap dispatch — Verification Record

keymap dispatch 実装 slice(sv-gui-03-06 contract の headless
実装)の検証記録。実行コマンド・OS/toolchain・exit code・
TS-EX-01〜11 突合結果・Deferred 項目・配置選択を記録する。
既存 `case-ledger.md`/`verification.md` は slice6
docs-only の証拠であり、本ファイルは別スライスの新規記録
(上書き・改変なし)。

## 実行環境

- OS: Windows 11(MSYS2/Git Bash、`MINGW64_NT-10.0-26300`)
- worktree: `G:\orca\workspaces\synvelis\sv112-keymap-dispatch`
- toolchain: mise 管理 rustc/cargo **1.99.1**(`mise current
  rust` = 1.99.1。`mise.ci.toml` pin 1.99.0 系)
- node: mise 管理(doc-*/impl-doc script 実行用)
- 実行日: 2026-11-15
- base: `origin/main` `194436b`(sv-gui-03-06 contract chain
  merge 済みを `git log` で確認)

## 実行した検証(canonical = mise)

| # | コマンド | 目的 | exit | 結果 |
| --- | --- | --- | --- | --- |
| V-1 | `mise run fmt-check` | `cargo fmt --all -- --check` | 0 | 差分なし |
| V-2 | `mise run check synvelis-workspace` | `cargo check --locked --all-targets -p synvelis-workspace` | 0 | warning なし |
| V-3 | `mise run lint synvelis-workspace` | clippy(pedantic 含む workspace lint) | 0 | warning なし |
| V-4 | `mise run test synvelis-workspace` | `cargo test --locked -p synvelis-workspace` | 0 | 全 test binary Pass。`tests/sv112_keymap.rs` 11/11 Pass、既存 suite(ws_a/ed_a/sv47/sv48/uic5/vcs04 等)も全 Pass・回帰なし |
| V-5 | `mise run test-rt` | `cargo test --workspace --locked rt_`(workspace 全体 rt_ 契約) | 0 | 全 rt_ test Pass |
| V-6 | `mise run check-headless-deps` | `node tools/check-headless-deps.mjs` | 0 | "no GUI framework deps in 12 headless crates" |
| V-7 | `mise exec -- prek run --all-files` | pre-commit 全 hook | 0 | 全 hook Pass |
| V-8 | `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir --apply` / `--kind exp --apply` | impl index 再生成 | 0 | ir/exp README を生成 index へ reconcile |
| V-9 | `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` | IR/EXP 監査 | 0 | 本 slice 由来の error/finding なし(結果は PR 前に再確認) |
| V-10 | `git diff --check` / `git status --short` | 空白エラー・変更範囲確認 | 0 | 許可範囲のみ(下記「変更ファイル」) |

補助証拠(canonical ではない直接 cargo 実行): 実装中の
`cargo fmt --all`・`cargo test -p synvelis-workspace --test
sv112_keymap` 等は反復用。最終判定は上記 mise タスクの結果。

## TS-EX-01〜11 突合(test-spec → 実走結果)

| TS | 内容 | 対応 test | 結果 |
| --- | --- | --- | --- |
| TS-EX-01 | lane 内一意性・既定バインド登録 | `ts_ex_01_default_bindings_are_unique_and_registered` | **Pass** |
| TS-EX-02 | §7.2 網羅(全操作に contract command) | `ts_ex_02_section_7_2_operations_have_contract_commands` | **Pass** |
| TS-EX-03 | OS 正規化写像(Win/Linux=Ctrl・macOS=Cmd) | `ts_ex_03_mod_normalizes_per_os` | **Pass** |
| TS-EX-04 | suppression 発火行列(InputContext×lane) | `ts_ex_04_input_context_firing_matrix` | **Pass** |
| TS-EX-05 | Undo lane 配送(context 別) | `ts_ex_05_undo_redo_lane_routing` | **Pass** |
| TS-EX-06 | K-2 Preview/Esc(transient 取消・編集抑制) | `ts_ex_06_gesture_preview_suppresses_edit_and_esc_cancels` | **Pass** |
| TS-EX-07 | K-4 tool.activate(origin view 限定) | `ts_ex_07_tool_activate_scoped_to_origin_view` | **Pass** |
| TS-EX-08 | auto-repeat(scroll/undo のみ許容) | `ts_ex_08_auto_repeat_policy` | **Pass** |
| TS-EX-09 | リマップ競合拒否(KG-C-5) | `ts_ex_09_remap_conflict_rejection` | **Pass** |
| TS-EX-10 | メニュー網羅(§6.2 coverage) | `ts_ex_10_catalog_has_menu_coverage_and_accelerators` | **Pass**(headless 契約面のみ。menu-model 依存部分は Deferred) |
| TS-EX-11 | outcome 正直さ(Suppressed/Unresolved/Rejected 区別) | `ts_ex_11_outcomes_stay_distinguishable` | **Pass** |

## 変更ファイル(許可範囲の確認)

- `crates/synvelis-workspace/src/keymap.rs`(新規)
- `crates/synvelis-workspace/src/command.rs`
- `crates/synvelis-workspace/src/workspace.rs`
- `crates/synvelis-workspace/src/lib.rs`
- `crates/synvelis-workspace/tests/sv112_keymap.rs`(新規)
- `docs/impl/ir/sv-gui-03-07-keymap-dispatch.md`(新規)
- `docs/impl/exp/sv-gui-03-07-keymap-dispatch.md`(新規)
- `docs/impl/ir/README.md`・`docs/impl/exp/README.md`
  (sync_impl_index 生成物)
- `evidence/sv112/keymap-dispatch-verification.md`(本ファイル・
  新規)

`crates/synvelis-app/`・他 crate・`ui/`・`tools/`・
`Cargo.toml`/`Cargo.lock`/`mise.toml`・contract chain
(sv-gui-03-06 docs)・既存 evidence ファイルは不変。

## 配置選択の記録(design §4.4 KG-D-4 / spec Deferred)

契約が Deferred とした suppression 配置は **keymap 層ゲート**
を選択:`keymap.rs` の `lane_fires` が §4.1 行列の唯一の
権威コピー。`WorkspaceManager::dispatch` の既存 suppression
(Project/Transport/Editing の `captures_input`)は非 keymap
caller 用に維持し、構成上一致をコードコメントで明示。
`dispatch` の suppression 集合を拡張する案は Undo lane の
context 別配送(`dispatch_undo`)と二重化するため棄却。
詳細は `docs/impl/exp/sv-gui-03-07-keymap-dispatch.md`。

## 未検証の項目(NotRun / Deferred / Unsupported)

| 項目 | 状態 | 理由・委譲先 |
| --- | --- | --- |
| 実 GPUI key dispatch(実 Window/実キーイベント) | **NotRun** | headless 層のみ。#114 GUI-L2 lane |
| 実 OS 既定バインド確認(Win Ctrl/macOS Cmd の実機挙動) | **NotRun** | `OsFamily` 正規化は論理層で検証済み。実機は #114 |
| 実 IME/実 Plugin GUI でのキー所有 | **NotRun** | InputContext routing は headless で検証。実機は #114/#176 |
| OS 予約キーの実機衝突 | **NotRun** | `os_reserved` は契約の予約表を論理 chord へ適用。実 WM 衝突は #114 |
| TS-GUI-01〜03(実機 scenario) | **Deferred** | test-spec が #114/#176 へ委譲 |
| TS-EX-10 の menu-model 依存部分 | **Deferred** | menu model 本体は `synvelis-app` 別 slice。keymap 層は §6.2 coverage inventory + accelerator 表記のみ提供 |
| `project.save`/`project.export` の実保存/export | **Deferred** | workspace 層に save/export op なし(`persist.rs` は WorkspaceSnapshot のみ)。本 slice は `Unresolved` を返す。app/service slice で op 実装後に接続 |
| `transport.stop-return`/`transport.record` | **Deferred** | playhead return API・#12 recording 状態機械が session 層に未存在。本 slice は `Unresolved` |
| `edit.copy`/`edit.paste` | **Deferred** | clipboard lane 未実装。本 slice は `Unresolved` |
| リマップ永続化・設定 UI・chord/多段キー・非 US 配列物理キー名 | **Deferred** | 後続 slice。`try_bind` の拒否判定のみ本 slice で実装 |
| `*-msrv`・`*-full` 系 | **NotRun** | Daily Full CI 委譲(タスク指定で必須検証に非含有) |
| macOS/Linux GUI-L2/EXT-L3 | **Deferred** | POLICIES.md: GUI-L2/EXT-L3 は使用要件発生まで Deferred |

## headless 検証と実機 Pass の区別

本スライスで Pass としたのは **headless 契約層の振る舞い**
(keymap pipeline・発火行列・suppression・Esc・repeat・
リマップ拒否・coverage inventory・outcome 区別)のみ。
実 OS/実 GPUI/実 IME/実 Plugin GUI の挙動は一切検証して
おらず、headless 成功を実機 Pass に昇格しない
(POLICIES.md §2/§3、KG-AC-07)。

## 既知の範囲外事項

- doc-driven-dev-graph router の既存 broken relation(repo
  全体の graph 債務)は slice6 evidence に記録済み・本 slice
  範囲外。実装作業は dispatch spec のファイル範囲で実施。
- `edit.delete` の非 Clips domain は #11/#12 語彙確定待ちで
  `Unresolved` — 語彙確定後に domain 写像を拡張する余地あり
  (推測 fallback は実装していない)。
