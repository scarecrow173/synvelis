# sv566 fix notes — synvelis-cli unix lint-full dead_code

Issue: `synvelis#566` — unix lane の `lint-full` が
`crates/synvelis-cli/src/session.rs` の `VAULT_SERVICE` /
`vault_user` dead_code 2 件で失敗する問題の修正記録。

- Base: `origin/main` `b73018bd`
- Branch: `bugfix/566-cli-dead-code`
- 実施日: 2026-11-18
- 文書チェーン: 本 slice は trivial lint hotfix のため
  `docs/specs`・`docs/designs`・`docs/plans`・`docs/tasks`・
  `docs/test-specs`・`docs/impl` の作成は行わず、本ファイルに
  集約する（タスク指定どおり minimal doc footprint）。

## 原因

`session.rs` の OS credential vault 実装（`synvelis#515`、
`c4182c66` / PR `synvelis#522` で導入）は caller 側を
`#[cfg(windows)]` / `#[cfg(not(windows))]` で分割しているが、
helper 2 item が gate されていなかった。

- `VAULT_SERVICE`（const、旧 :34）の唯一の参照は
  `#[cfg(windows)]` の `vault_entry`（:233）。
- `vault_user`（fn、旧 :43-50）の唯一の参照は
  `#[cfg(windows)]` の `vault_set` / `vault_get` /
  `vault_delete`（:239-270）。
- `#[cfg(not(windows))]` stub（:272-301）は引数を `_` で
  受けるだけで 2 item を参照しない。

そのため unix build では 2 item が到達不能となり、
`dead_code` warning → workspace `lint-full` の `-D warnings`
で error 化する。main `b73018bd` の CI run `44267395469`
（ubuntu lane）で `error: constant VAULT_SERVICE is never
used` / `error: function vault_user is never used` を観測済み。
Windows lane は caller が存在するため Pass していた。

## 修正

`session.rs` に `#[cfg(windows)]` を 2 行追加するのみ
（doc comment は残し、周辺の `vault_entry` 以下と同じ
gating 方式に揃えた）。`#[allow(dead_code)]` 等の警告抑制は
不使用。

```diff
 /// filed under (per-OS-user on Windows Credential Manager).
+#[cfg(windows)]
 const VAULT_SERVICE: &str = "synvelis-cli";
@@
 /// (The hash is a namespace discriminator, not a secret.)
+#[cfg(windows)]
 fn vault_user(store_path_arg: Option<&Path>, session_id: &str) -> Result<String, CliError> {
```

`vault_user` を gate しても新たな dead_code は生じない:

- `use std::collections::hash_map::DefaultHasher` 等の import
  は fn 本体内にあるため item ごと unix で除外される。
- `store_path` は `pub` で `open_store` / `issue` / `revoke` /
  `resolve_credential` から unix でも参照される。
- `vault_supported` は `cfg!(windows)` マクロで両 platform に
  残り、変更不要。
- `keyring` 依存は `crates/synvelis-cli/Cargo.toml` の
  `[target.'cfg(windows)'.dependencies]` で既に gate 済み
  （unix build では compile 対象外）。
- テスト module は `store_path` のみ検査し、2 item への参照
  なし。`crates/synvelis-cli/tests/cli_e2e.rs` の "vault" 言及
  はコメントのみ。

## red→green（unix 実測）

Windows ホストからの
`cargo clippy --target x86_64-unknown-linux-gnu` は
`mlua-sys` build script が `x86_64-linux-gnu-g++` を要求し
cross C++ toolchain 不在で失敗するため、WSL2 Ubuntu
（rustc 1.99.0、CI pin `mise.ci.toml` と同一）で実施した。
補助証拠（canonical = mise タスク）。

| 段階 | コマンド | 結果 |
| --- | --- | --- |
| 修正前 | `cargo clippy --locked -p synvelis-cli --all-targets -- -D warnings`（WSL2、`x86_64-unknown-linux-gnu`） | **Fail** — `error: constant VAULT_SERVICE is never used`（session.rs:34）・`error: function vault_user is never used`（session.rs:43）の 2 件で `synvelis-cli` lib / lib test が compile error |
| 修正後 | 同上 | **Pass** — exit 0、warning なし |

WSL 側の `CARGO_TARGET_DIR` は `/home/akiyoshi/target-sv566`
（Windows 側 mbx managed target への linux artifact 混入を
避けるため。リポジトリの canonical 検証出力先は変更していない）。

## 実行した検証（Windows ローカル・canonical = mise）

OS: Windows（worktree `sv566-cli-dead-code`）
Toolchain: mise 管理 `rustc 1.99.1 (58cf8f9c3 2026-10-13)` /
`cargo 1.99.1`

| コマンド | 結果 |
| --- | --- |
| `mise run check-cli` | Pass（exit 0） |
| `mise run lint-cli` | Pass（exit 0、`-D warnings` 回帰なし） |
| `mise run test-cli` | Pass（exit 0、unit + e2e 28 件全 pass） |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run lint-full` | Pass（exit 0、workspace 全体 `-D warnings`、Windows local での非回帰を実測） |
| `mise exec -- prek run --all-files` | Pass（exit 0、全 hook） |

## NotRun / Unsupported

- `docs/**/README.md` index 再生成: lease 外。Coordinator が
  統合時に `doc-maintenance` で再生成する。
- GUI-L2 / EXT-L3 / MSRV・3OS 横断: POLICIES.md §5 により
  日常必須検証外。Daily/手動 Full CI に委譲。
- unix 実環境での OS credential store 動作検証: Issue スコープ
  外（AC は lint clean のみ）。

## 残件

- PR の ubuntu CI lane で `lint-full` green を確認することが
  unix 修正の統合証拠となる（ローカルでは WSL 実測で代替済み）。
- Issue `synvelis#566` は OPEN 維持。PR は `Refs #566` のみとし、
  close 判定は Coordinator に委ねる。
