# evidence/sv232 — 棚卸し網羅性証拠（synvelis#232 AC1 棚卸し slice）

Issue #232 AC1 の棚卸し列挙を、第三者が同じ手順で再現検証できるよう、
実行した列挙コマンド・exit code・ヒット件数・対象 SHA を記録する。
台帳本体は `docs/discovery/sv232-legacy-webview-inventory.md`。

## 対象 SHA

| 項目 | 値 | 取得方法 |
| --- | --- | --- |
| Synvelis 作業 HEAD | `4b899b3ce1def2db1feddc1d7d3c5266ad1b91b7` | `git rev-parse HEAD`（worktree `sv232-webview-inventory`、2026-11-24） |
| Ruviform 解決 rev | `cd4213a1d665600f6adfefa0c3a90f2be9b4e4ce` | `grep -oE 'scarecrow173/ruviform\.git[^"]*#[0-9a-f]{40}' Cargo.lock \| sort -u`（1 件に収束 — 単一 revision guard と一致） |

## 列挙コマンドと結果

実行場所は worktree ルート（`G:\orca\workspaces\synvelis\sv232-webview-inventory`）。
`git grep` は追跡ファイルのみを対象とし、`ui/vendor/`・`ui/node_modules/`・
`crates/synvelis-app/frontend/dist/` などの gitignore 済み生成物は対象外
（生成物は `mise run sync-ui-deps` / `build-ui` で lock rev から再生成されるため
棚卸し対象は生成元の定義側）。

| # | コマンド | exit | ヒット件数 | 出力先 |
| --- | --- | --- | --- | --- |
| E1 | `git grep -n -i webview` | 0 | 762 行 / 188 ファイル | `git-grep-webview.txt` / `git-grep-webview-files.txt` |
| E2 | `git grep -l -i webview` | 0 | 188 ファイル | `git-grep-webview-files.txt` |
| E3 | `git ls-files ui/` | 0 | 34 ファイル | `ls-files-ui.txt` |
| E4 | `git ls-files 'docs/mockups/**' 'assets/**'` | 0 | 12 ファイル | `ls-files-mockups-assets.txt` |
| E5 | `git grep -n "ruviform_ui_webview\|ruviform-ui-webview"` | 0 | 48 行 | `grep-ruviform-ui-webview.txt` |
| E6 | `git grep -n "rvf://"` | 0 | 31 行 | `grep-rvf-scheme.txt` |
| E7 | `git grep -n "webview\|ui-verify\|frontend\|ui/\|sync-ui\|check-ui\|test-ui\|build-ui\|install-ui\|dev-ui" .github/workflows/ci.yml .github/workflows/daily-ci.yml` | 0 | 42 行 | `grep-ci-ui.txt` |
| E8 | `git grep -n "webview\|ui/" mise.toml` | 0 | 24 行 | `grep-mise-ui.txt` |
| E9 | `git grep -n -i "webview\|wry\|tao\b" tools/` | 0 | 12 行 | `grep-tools.txt` |
| E10 | `git grep -n "BackendReport::available" crates/synvelis-app/tests/` | 0 | 34 ファイル（fixture 文字列 `"webview2"` 利用の所在確認） | 本表 E11 と併記 |
| E11 | `git grep -ln "webview" crates/synvelis-app/tests/` | 0 | 11 ファイル | E1 出力の `crates/synvelis-app/tests/` 部分集合 |
| E12 | `grep -rn "undo\|redo" ui/src/ ui/test/` | 0 | 2 行（signal-flow view-model の doc コメントのみ — webview frontend が `syn.project.undo`/`redo` を op surface に含まないことの実測） | 本書 §3 |
| E13 | `grep -rhoE '"syn\.[a-z_.]+"\|"sys\.capabilities"' ui/src/ ui/test/ \| sort -u` | 0 | 39 種の op 名（frontend の `syn.*` 使用面） | 本書 §3 |

## 列挙結果と台帳項目の対応（網羅性の追跡）

台帳（discovery 文書）の各区分がどのコマンドで捕捉されたかの対応表。

| 台帳区分 | 捕捉コマンド | 備考 |
| --- | --- | --- |
| `Cargo.toml` `ruviform-ui-webview` dep | E1・E5 | E5 が crate 名の直接ヒットを列挙 |
| `crates/synvelis-app/Cargo.toml`（webview feature・bin・optional dep） | E1・E5 | |
| `src/bin/synvelis.rs` | E1・E5 | `use ruviform_ui_webview` が E5 で捕捉 |
| `src/assets.rs` | E1・E5 | `EmbedAssets` import が E5 で捕捉 |
| `src/revision.rs`・`build.rs`（rev guard・bundle check） | E6・E1 | `rvf://app` namespace 言及が E6 で捕捉。`SYNVELIS_RUVIFORM_REV` は `git grep -n SYNVELIS_RUVIFORM_REV` で別途確認（build.rs・revision.rs・app.rs・syn_ctl.rs・synvelis-native.rs） |
| 共有モジュールの doc 言及（host.rs/app.rs/error.rs/shell.rs/ui_ops.rs/binding.rs/composition.rs/lib.rs） | E1 | doc コメント中の `WebView` 文言 |
| `crates/synvelis-app/tests/` 11 ファイル | E1・E11 | fixture 文字列利用は E10 で切り分け |
| `ui/` 一式 | E3・E13 | E3 が追跡ファイル全件、E13 が op 使用面 |
| `mise.toml` lanes | E8 | `sync-ui-deps`/`install-ui`/`check-ui`/`test-ui`/`build-ui`/`dev-ui`/`build-app`/`check|lint|test-app-webview` |
| `.github/workflows/ci.yml` `ui-verify`/`ui-verify-macos` | E7 | `dep-guard` の lockfile guard は `git grep -n lockguard ci.yml` で別途確認 |
| `tools/` | E9 | `sync-ruviform-ui.mjs`/`check-headless-deps.mjs`/`ci-summary.mjs`/`check-ruviform-rev.mjs` |
| `docs/mockups/`・`assets/` | E4 | README 参照は `git grep -n "mockup" README.md` で確認 |
| `.gitignore`/`.worktreeinclude` の frontend 生成物行 | E1 | `cargo check --features webview` 言及 |
| `Cargo.lock`（`ruviform-ui-webview`/`webview2-com` 系 package） | E1 | lockfile は依存削除時に再生成される派生物 |
| 歴史文書（docs/adr・docs/specs・docs/designs・docs/discovery・docs/knowledge・docs/evidence・docs/tasks・docs/impl・evidence/sv519 等） | E1・E2 | 撤去対象外の記録文書。E2 の 188 ファイルの大部分 |
| `.agents/skills/verify-synvelis` | E1 | エージェント資産 — #232 撤去予定を既に honest blocked として記録済み |

## 補足

- `git grep -n -i webview` の 762 行のうち、棚卸しで撤去候補・共有区分・
  要維持の判定対象となるのはコード・設定・CI・ツール側のヒット。
  `docs/` 以下と `evidence/` のヒットは歴史・証拠記録であり、Issue 本文
  「歴史的 ADR/PoC/Issue/数値は保管」（RF#465 §5 D9 判定と同根拠）に従い
  撤去対象から除外する。
- `ui/vendor/ruviform` は gitignore 済み生成物で E1〜E13 の対象外。
  正本は `Cargo.lock` 解決 rev `cd4213a1` と `tools/sync-ruviform-ui.mjs` の
  `PACKAGES = ["ui/core","ui/components","ui/binding","ui/viz","ui/guitest"]`
  定義側（E9 の出力に含まれる）。
