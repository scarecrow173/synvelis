# verified-Human OS-credential session 供給 — 検証・判定台帳 (synvelis#515)

Issue #515「[実装/autx] verified-Human OS-credential session 供給
（durable CliSessionStore + OS credential 検証経路）」の実測記録。
durable `FileCliSessionStore`（digest-only JSON、process restart
跨ぎ）を synvelis-gateway に追加し、synvelis-cli に OS credential
vault（Windows Credential Manager via `keyring` crate）+
`session issue/list/revoke` provisioning + `--session <id>` claim
解決 + `--session-store` durable 配線を実装した。

**採用経路（design/discovery に記録）**: session record は file
backend（`FilePermitStore` 先例と同型）、usable secret は OS
credential store（Windows: Credential Manager）に session id 単位で
保管。vault username は store path の hash で scope され、
`--session-store` をまたいだ credential 衝突・誤 revoke を防ぐ
（session id は store ごとの連番のため）。`unsafe_code=forbid`
のため OS API 直叩き不可 → safe wrapper
`keyring` v4 を `cfg(windows)` target 依存として採用（deps:
keyring-core/windows-native-keyring-store/windows-sys 0.61.2/
zeroize/byteorder/regex — GUI/GPU/WebView なし、
`check-headless-deps` で実測確認）。macOS/Linux は Unsupported。

## 環境・対象 Revision・実行コマンド

| 項目 | 値 |
|---|---|
| 対象 Repository | scarecrow173/synvelis |
| 対象 Revision | branch `feat/515-os-credential-session`、base `origin/main` `acc0198c`（fetch 済み）。検証は commit 前 working tree で実施 — PR head SHA は PR 本文に記録 |
| OS / Arch | Windows 11 / x86_64（Git Bash 経由） |
| Rust toolchain | cargo 1.99.x（`mise.toml` 非 pin、CI pin は `mise.ci.toml` 1.99.0） |
| 実行日 | 2026-11-17 |
| 作業 worktree | `G:\orca\workspaces\synvelis\sv515-os-credential` |

## AC → case → test → result

| AC | case | test / 証拠 | result |
|---|---|---|---|
| durable store が restart 後も expiry/revocation/一度きり返却を維持 | restart verify・expiry/revoke persist・digest-only | `cli_adapter.rs`: `file_session_store_verifies_across_process_restart` / `file_session_store_persists_expiry_and_revocation` / `file_session_store_file_never_contains_the_usable_secret` | Pass |
| interactive claim が OS credential 検証経由で verified-Human を供給・少なくとも 1 OS で動作 | `session issue` → `--session <id>` → attestation 通過 | `cli_e2e.rs`: `session_claim_reaches_verified_human_attestation`（実 Windows Credential Manager） | Pass (Windows) |
| unattended/wire/偽装系の fail-closed 非回帰 | 既存 suite 全件 | `cli_adapter.rs` 既存 24 tests + `cli_e2e.rs` `interactive_claim_fails_closed` 等 | Pass |
| secret が log/stdout/record/file に一度きり返却以外で流出しない | issue/list 出力・store file に `syv-cli-` 非含有 | `session_issue_list_revoke_roundtrip` + `file_session_store_file_never_contains_the_usable_secret` + 既存 `session_records_never_contain_the_usable_secret` | Pass |
| `mise run test synvelis-gateway` / `test-cli` pass | — | 下記「実行した正規検証コマンド」 | Pass |
| headless Pass を実 OS 受入に昇格しない・残件は NotRun 記録 | 実 credential prompt/2-process/他 OS | 下記「NotRun / Deferred」 | 記録済み |

## 実行した正規検証コマンド（実測）

| コマンド | exit | 結果 |
|---|---|---|
| `mise run fmt-check` | 0 | `cargo fmt --all -- --check` 成功 |
| `mise run check synvelis-gateway` | 0 | `cargo check --locked --all-targets -p synvelis-gateway` 成功 |
| `mise run lint synvelis-gateway` | 0 | clippy `-D warnings` 成功 |
| `mise run test synvelis-gateway` | 0 | `cargo test --locked -p synvelis-gateway` 全 suite 成功。`cli_adapter` 30/30（新規 FileCliSessionStore 6 tests 含む） |
| `mise run check-cli` | 0 | `cargo check --locked --all-targets -p synvelis-cli` 成功 |
| `mise run lint-cli` | 0 | clippy `-D warnings` 成功（初回 7 件指摘 → 修正後 pass） |
| `mise run test-cli` | 0 | `cargo test --locked -p synvelis-cli`。`cli_e2e` 28/28（新規 session 系 2 tests 含む、実 Windows Credential Manager で issue→--session→verified-Human attestation・revoke fail-closed を実測） |
| `mise run build-cli` | 0 | `cargo build --locked -p synvelis-cli` 成功 |
| `mise run check-headless-deps` | 0 | `check-headless-deps: no GUI framework deps in 12 headless crates` — keyring 系依存は GUI/GPU/WebView を引き込まない |
| `mise exec -- prek run --all-files` | 0 | 全 hook Passed/Skipped |

補足: `cargo test -p synvelis-gateway --test cli_adapter`・
`cargo test -p synvelis-cli --test cli_e2e session_` の直接実行は
補助証拠（canonical = mise）。

## NotRun / Deferred / Unsupported（POLICIES.md 語彙）

| 項目 | 判定 | 理由・owner・follow-up |
|---|---|---|
| 実 credential prompt / Windows Hello / Touch ID 等の対話存在証明 | Deferred | 本 slice は OS user account 束縛の credential store まで。対話存在証明は #114 系実 OS/GUI 受入 gate へ deferred |
| macOS Keychain / Linux Secret Service vault backend | Unsupported | `session.rs` の非 Windows vault は `CliError::Unsupported`（exit 5）で fail-closed。需要があれば別 slice |
| 実 2-process commit/credential 環境・GUI 結合受入 | NotRun | #267（拒否方向 E2E）/#114（実 OS/GUI 受入）lane の責務。本 slice の real-binary e2e は単一 process 内の実 OS vault 経路まで |
| `synvelis-runtime` crate | Deferred | 後続 AUTX slice の責務（本 Issue 範囲外） |

## 信頼境界メモ

- `session issue` は任意 local process が実行可能な operator
  provisioning（`mcp-poc permit` と同型）。実効的な Human mint には
  同一 OS user context での vault 読取が必要 — trust boundary 外の
  同一 user 行為として設計文書に記録。
- `--session`/`--session-store`/env（`APPDATA`/`HOME`/`XDG_STATE_HOME`/
  `USERNAME`/`USER`）は場所・ラベル解決のみ — 権限証拠にしない。
- 負側契約は無変更: `attest`/`caller_delegated`/`caller_verified`/
  `record_kind_ok`/`verify_in`/`AdapterSeal`/`CallerClass` は既存
  実装のまま。unattended claim は常に `Agent`、wire payload の
  authority 申告は `SelfDeclaredAuthority` で拒否。
