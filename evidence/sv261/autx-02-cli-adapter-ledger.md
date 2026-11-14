# AUTX-02 CLI Trusted Local Adapter — 検証・Deferred 台帳 (synvelis#261)

Issue #261「[impl/security][AUTX-02] CLI Trusted Local Adapter・
Unattended認可・Headless Session Host」の **headless
authorization-boundary スライス**の実測記録。`synvelis-gateway`
内に trusted local CLI adapter（`cli.rs`）・`OriginKind::LocalCli`・
`CallerClass` 分類・`HeadlessSessionHost` composition（`headless.rs`）・
`CommitGate` の `CommitPort`/`AdminPort` 実装・fail-closed fixture
（`tests/cli_adapter.rs` 24 tests）を追加した。

**本スライスの達成範囲**は Issue 受入条件の (3) `CallerIdentity`
非偽造性と fail-closed 分類、(4) 分類ごとの Grant/Approval/
Scope/Expiry の Gateway 評価、(7) issuer 公開範囲の限定と権限
ロンダリング拒否。(1)(2) Script 実バイナリ/Luau 経路、(5) 実
Project open/commit/reopen（`synvelis-project` port adapter 依存）、
(6) 実 2-process・実 OS lock/credential 検証は後続スライスに
Deferred — 本台帳に `Deferred`/`NotRun` として記録する。

## 環境・対象 Revision・実行コマンド

| 項目 | 値 |
|---|---|
| 対象 Repository | scarecrow173/synvelis |
| 対象 Revision | branch `feat/261-cli-trusted-adapter`、base `6ba9d3f07f10d0e728c6835131015e22fde65717`（`origin/main` HEAD at branch time）。検証は commit 前 working tree で実施 — PR head SHA は PR 本文に記録 |
| OS / Arch | Windows / x86_64（Git Bash 経由） |
| Rust toolchain | rustc/cargo `1.99.1`（環境導入済み。`mise.toml` は Rust を pin しない。CI pin は `mise.ci.toml`） |
| Node | `v26.12.1`（doc 検証 script 用） |
| 実行日 | 2026-11-14 |
| 作業 worktree | `C:\Users\akiyoshi\orca\workspaces\synvelis\sv261-autx2` |

### 実行した正規検証コマンド（実測）

| コマンド | exit | 結果 |
|---|---|---|
| `mise run fmt` → `mise run fmt-check` | 0 | `cargo fmt --all` 適用後 `cargo fmt --all -- --check` 成功 |
| `mise run check synvelis-gateway` | 0 | `cargo check --locked --all-targets -p synvelis-gateway` 成功 |
| `mise run lint synvelis-gateway` | 0 | clippy `-D warnings` 成功（`result_large_err` の `#[allow]` は既存 crate 慣行に一致・新規 warning なし） |
| `mise run test synvelis-gateway` | 0 | `cargo test --locked -p synvelis-gateway`。26 test binary 全成功・**430 passed / 0 failed**（lib 7・bin 0・integration 22 file + 新規 `cli_adapter` 24 tests・doc-tests 0）。sv46 記録の 406 から +24 |

補足: 開発中に `cargo test -p synvelis-gateway --test cli_adapter`
（cargo 直接実行）で新規 fixture を反復検証した — 補助証拠であり
canonical は上記 mise タスク。初回実行で 3 件の test 期待値ずれ
（deny reason の具体値 `"grant revoked"`/`"grant expired"`・MCP
Control channel の origin `ManualUi`）を検出し、test 側の期待値を
実契約へ修正して全件 Pass を確認した（実装側の緩和ではない）。

## 受入条件カバレッジ（本スライス範囲）

| Issue 受入条件 | 本スライスでの扱い | 証拠 |
|---|---|---|
| (3) `CallerIdentity` を外部 crate/入力 JSON/args/env/TTY 単独で Mint できず、信頼できない手動・CI/無人実行は `Human` の無条件許可に到達しない | **実装+検証済み** | `AdapterSeal` crate-internal mint 維持・`cli.rs`/`headless.rs` 新設・`class_*`/`unattended_claim_*`/`interactive_claim_*`/`claims_deserialize_fail_closed`/`wire_payloads_cannot_assert_authority_or_identity`/`host_handle_rejects_unattended_human_shaped_requests` tests Pass |
| (4) Verified Human/Delegated CI/Agent/External MCP/Test Fixture を独立 Fixture で分類し、必要な Grant/Approval/Scope/Expiry/Budget を毎回 Gateway が評価する | **実装+検証済み** | `CallerClass` 4 分類・`class_*` 4 tests・`delegated_*` 8 tests（scope/revoke/expiry/budget/approval per-call 評価）Pass |
| (7) Trusted Issuer/AdapterSeal の公開範囲が限定され、CLI 経由の Agent→Human 権限ロンダリング、MCP→CLI の昇格、Secret stdout 流出がない | **実装+検証済み（in-crate 契約）** | seal issuer は 3 系統に限定・`session_records_never_contain_the_usable_secret`・`delegated_authority_is_bound_to_its_principal`・`class_mcp_channels_classify_client_agent_and_control_human` tests Pass。Secret の stdout/ログ流出は「secret を返す API が issue 時の一度きり」をコード契約で担保 — 実 transport での流出検査は後続スライス |
| (1)(2) Script Action/Workflow の CLI 実バイナリ・Luau Script からの偽装防止 | **Deferred** | `synvelis-runtime` 新設・Script/Luau 統合は synvelis#272/#273・ruviform#559 側。本スライスはその前段の認可境界契約のみ |
| (5) GUI/WebView/GPUI を初期化しない Headless Session Host で Project open/query/prepare/commit/reopen | **部分実装+残件 Deferred** | `HeadlessSessionHost` composition・`handle_claim` 単一経路・`CommitGate` port 実装は in-crate 契約として実装済み。実 project open/lock/revision は `synvelis-project` port adapter が必要で別 Worker 占有領域 — Deferred |
| (6) Open Project Lock/Revision stale・2Process 同時 Commit・Revocation race・Retry・Crash/Restart | **Deferred** | 実 2-process・実 OS lock/credential・実 journal recovery 結合検証は `synvelis-project` port adapter + 実行環境待ち。in-crate の `CommitGate` fence/journal 契約は既存 gw10/REC suite で非回帰確認済み |

## OS capability 判定（Issue 検証方法の要求）

| OS | OS credential/session/lock capability | 判定 | 理由 |
|---|---|---|---|
| Windows | 未検証 | **NotRun** | 実 OS session provenance（logon session/console session）と durable credential store の連携は後続スライス。`CliSessionStore` 契約は実装済みだが実 OS capability へは未配線 |
| macOS | 未検証 | **NotRun** | 同上 |
| Linux | 未検証 | **NotRun** | 同上 |

非対応・未検証環境で Human を推測許可しないことは契約として
実装済み: session を発行できない issuer は `Human` caller を
mint できず、unattended 経路は常に `Agent` に落ちる
（`unattended_claim_cannot_mint_human_regardless_of_principal_id`・
`interactive_claim_requires_a_verified_credential` が fail-closed
を実証）。

## 契約文書との照合（merge 後確認）

`origin/main` 取り込み後に `docs/automation/design-trust-boundary.md`
（SV-AUTX-00 contract set、#259 系で merge 済み）§2 を照合した。
同表は CLI session を `OriginKind::ExternalAdapter` に写像するが、
本スライスは task spec 指示どおり `OriginKind::LocalCli` を新設した。
**behavior 差なし**: `is_external`/`authorize_disclosure` は
`PrincipalKind::Agent` で既に external 判定され（delegated CLI
caller は全て Agent）、`Human` principal は disclosure 判定を
`PermitForStage` で short-circuit するため、`Human+LocalCli` と
`Human+ExternalAdapter` は全 engine 経路で同一 verdict。
`authority_class()` は `(kind, origin)` のみに写像し wire・引数・
env・TTY を参照しない。AUTX-00 文書鎖側の表更新は別 Worker 領域の
ため follow-up として IR/PR に記録する。

## Deferred / NotRun 一覧

| 項目 | 状態 | 後続 Owner / 条件 |
|---|---|---|
| `synvelis-runtime` 新設 crate（CLI binary・session process） | Deferred | 後続 AUTX スライス。`HeadlessSessionHost` が embed 契約 |
| `synvelis-project` port adapter（project open/lock/revision seam） | Deferred | 別 Worker 占有領域（task spec 編集禁止範囲） |
| Script Action/Workflow 実行経路（CLI 実バイナリ・Luau VM） | Deferred | synvelis#272/#273・ruviform#559 |
| durable `CliSessionStore`（file/OS keychain） | Deferred | runtime slice。`MemoryCliSessionStore` は restart で session 消失（fail-closed・可用性低下のみ） |
| 実 OS credential/session capability（Win/macOS/Linux） | NotRun | 実 credential 連携 slice |
| 実 2-process commit・crash/restart・project lock・journal recovery | NotRun | EXT-L3 検証 slice |
| `mise run gw10`（任意の非回帰確認） | NotRun | 今回は `mise run test synvelis-gateway` で既存 suite 非回帰を確認（430/0）。gw10 は transport E2E で本変更の直接対象外 |
| `mise run check-full`/`lint-full`/`test-full`/`build-full`/`verify-full`・`*-msrv`・3OS 横断・Feature Matrix | NotRun | POLICIES.md §5・task spec により日常必須検証の対象外 |
| 実 GUI/実 Provider/第三者 MCP Host 受入 | NotRun | Product Accepted 軸 — 本スライスは headless 契約のみ |

## Security review 観点の確認

- **権限ロンダリング**: `caller_delegated` の戻り値は引数で
  `PrincipalKind` を選べない構造（`Agent` 固定）。`CliSessionClaim`
  に Human を主張する variant は存在しない。
- **MCP→CLI 昇格**: `LocalCliAdapter` の seal は `CLI_ADAPTER_RAW`
  の `AdapterId` に紐付き、MCP permit/session credential は CLI
  store では verify されない（prefix `syv-cli-` と hash 照合で
  分離）。逆方向も同様。
- **Secret 流出**: `CliSessionIssue.secret` は issue 時の一度きりの
  戻り値。`CliSessionRecord`/`CliSessionStore::record` は hash のみ。
  `PartialEq`/`Debug` derive は secret を含まない record のみ。
- **blind verdict**: unknown/expired/revoked/malformed/non-human
  credential が全て `GatewayError::Authentication`（fieldless）に
  落ち、reason 文字列・error kind で区別できない。

## 参照

- 実装記録: `docs/impl/ir/autx-02-cli-adapter.md`
- 試行記録: `docs/impl/exp/autx-02-cli-adapter.md`
- 検証仕様: `docs/test-specs/autx-02-cli-adapter.md`
- Fixture: `crates/synvelis-gateway/tests/cli_adapter.rs`
