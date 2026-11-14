---
id: "IMPL-034Xe6lRNc3atRxDvadhth"
type: "impl"
status: "completed"
title: "AUTX-02 CLI Trusted Local Adapter・Unattended認可・Headless Session Host (synvelis#261)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/261"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/src/cli.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/headless.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cli_adapter.rs"
      - type: "file"
        path: "docs/test-specs/autx-02-cli-adapter.md"
      - type: "file"
        path: "docs/impl/ir/autx-02-cli-adapter.md"
      - type: "file"
        path: "docs/impl/exp/autx-02-cli-adapter.md"
      - type: "file"
        path: "evidence/sv261/autx-02-cli-adapter-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/request.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/outcome.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/commit.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/mcp_permit.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/fixture.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/command-gateway.md"
    - "docs/test-specs/command-gateway.md"
    - "docs/test-specs/autx-02-cli-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/command-gateway.md"
    - "docs/impl/ir/gw-08-mcp-server-adapter-trusted-local-connection-midi-3mode-e2e.md"
    - "docs/automation/design-trust-boundary.md"
    - "evidence/sv45/command-gateway-verification.md"
    - "evidence/sv46/command-gateway-gates-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/test-specs/autx-02-cli-adapter.md"
  derived-by: []
  verifies:
    - "docs/test-specs/autx-02-cli-adapter.md"
  verified-by: []
  references:
    - "docs/test-specs/autx-02-cli-adapter.md"
    - "docs/impl/exp/autx-02-cli-adapter.md"
    - "evidence/sv261/autx-02-cli-adapter-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/272"
    - "https://github.com/scarecrow173/synvelis/issues/273"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-02-cli-adapter.md"
    rejected: []
---

# AUTX-02 CLI Trusted Local Adapter・Unattended認可・Headless Session Host (synvelis#261)

## Summary

`synvelis-gateway` に、ローカル CLI 向けの trusted adapter seam
（`cli.rs` の `LocalCliAdapter` + `CliSessionStore`）と、GUI 非依存の
`HeadlessSessionHost` composition（`headless.rs`）を追加する。
`OriginKind::LocalCli` と `CallerIdentity::authority_class()` を導入し、
verified interactive Human / unattended delegated / external agent /
fixture agent の権限クラスを fail-closed に分類する。
`AdapterSeal` の mint は引き続き crate-internal に限定し、外部 crate・
wire payload・CLI args/env/TTY から `CallerIdentity` を偽造できない
不変条件を維持する。`CommitGate` に既存 `CommitPort`/`AdminPort` を実装し、
headless composition が実 commit 境界をそのまま再利用できるようにする。

## Context

synvelis#261（AUTX-02）の headless authorization-boundary スライス。
Coordinator task spec の編集範囲は `crates/synvelis-gateway/`・
`docs/impl/{ir,exp}/autx-02-cli-adapter.md`・`docs/test-specs/autx-02-cli-adapter.md`・
`evidence/sv261/` に限定される。本スライスの達成範囲は Issue 受入条件の
(3) `CallerIdentity` 非偽造性と fail-closed 分類、(4) 分類ごとの
Grant/Approval/Scope/Expiry の Gateway 評価、(7) issuer 公開範囲の限定と
権限ロンダリング拒否。(1)(2) の Script 実バイナリ/Luau 経路、
(5) の実 Project open/commit/reopen（`synvelis-project` port adapter）、
(6) の実 2-process・実 OS lock/credential 検証は後続スライスに Deferred。

## Implementation

### Changes

- `src/request.rs`: `OriginKind::LocalCli`（wire 表現 `"local-cli"`）を追加。
  `CallerClass` enum（`VerifiedHuman` / `UnattendedDelegated` /
  `ExternalAgent` / `FixtureAgent`）と
  `CallerIdentity::authority_class()` を追加。`authority_class()` は
  crate-minted の `(PrincipalKind, OriginKind)` のみに写像し、wire・
  CLI 引数・env・TTY・script 入力を一切参照しない。`PrincipalKind::Agent`
  の doc を「task-grant authority または delegated local automation を
  持つ agent-equivalent principal」に拡張。
- `src/outcome.rs`: `GatewayError::Authentication`（fieldless）を追加。
  credential/session 検証失敗の blind verdict — unknown・expired・
  revoked・malformed・non-human record が全て同一の verdict を返し、
  probe が session の存在を推測できない。`ErrorCategory::Authentication`・
  `RetryAdvice::Never` に分類。
- `src/cli.rs`（新規）: `CliSessionStore` trait、`MemoryCliSessionStore`、
  `CliSessionSpec`/`CliSessionRecord`/`CliSessionIssue`、`CliSessionClaim`
  （`Unattended` / `Interactive { credential }`、`deny_unknown_fields`）、
  `LocalCliAdapter`。session record は random 256-bit secret の SHA-256
  hash のみ保持し、`live(now)` で expiry/revoke を判定する。
  `caller_delegated` は常に `(Agent, LocalCli)` を返し、
  `caller_verified` のみが `(Human, LocalCli)` を発行する。issue/verify
  の両方で Human principal のみを session に束縛可能とする
  （`check_session_kind`）。verify は existence-blind（unknown・expired・
  revoked・malformed・non-human が同一 `None`）。`CLI_ADAPTER_RAW`
  adapter id 定数と `CLI_SECRET_PREFIX`（`syv-cli-`）を定義。
- `src/headless.rs`（新規）: `HeadlessSessionHost` — `Gateway` +
  `LocalCliAdapter` + `CommitPort`/`AdminPort` を束ねる GUI 非依存の
  composition。`handle_claim` が claim → trusted caller →
  `Gateway::handle` の単一経路を提供し、`without_commit_boundary`
  は `NoCommitPort`/`NoAdminPort` の fail-closed 既定を返す。
- `src/commit.rs`: `CommitGate<F: CommitFence>` に `CommitPort` と
  `AdminPort` の impl を追加（fence 経由の既存 commit/reconcile/
  cancel_apply/policy issue・revoke をそのまま公開。
  `live_revision` は gate が所有する project のみ返す）。
- `src/mcp_permit.rs`: `sha256_hex`・`ct_eq` を `pub(crate)` 化、
  `mint_secret` を prefix 引数化して `SECRET_PREFIX` を呼出側へ移動。
  CLI session secret と MCP permit secret が同一の CSPRNG/hash/ct_eq
  実装を共有する（第二の crypto 実装を作らない）。
- `src/lib.rs`: `cli`/`headless` module 追加、export、trust-boundary doc
  更新（seal issuer は FixtureAdapter・McpHost・LocalCliAdapter の 3 系統）。
- `src/fixture.rs`: 「only issuer」コメントを実態（3 issuer）に合わせて修正。
- `tests/cli_adapter.rs`（新規）: fail-closed fixture 群 24 tests。

### Decisions During Implementation

- **Human 判定を `PrincipalKind` ではなく session credential に紐付ける**:
  unattended/TTY/env/args は `CliSessionClaim::Unattended` → 常に Agent
  principal に落とす。`Interactive { credential }` は発行済み session の
  secret を store が検証した場合のみ Human を発行する。TTY・`--yes`・
  env・JSON 引数を認証証拠として受け取る API を一切設けない。
  `CliSessionClaim` は `deny_unknown_fields` で `yes`/`tty`/`as_human`/
  `interactive` 等の authority 主張 field の混入を parse 段階で拒否する。
- **delegated 権限は新しい grant 機構を作らず既存 `Grant`/`Task`/
  `GrantPolicy` を再利用**: `caller_delegated` が返す Agent principal に
  対し、wire の `task`/`grant` 参照を `GrantPolicy` が毎回評価する。
  session expiry は認証層の失効であり、grant の scope/expiry/revoke/
  approval/budget 判定は従来どおり request-apply/commit stage で再評価
  される（`delegated_*` tests が per-call 再評価を実証）。
- **`CommitPort`/`AdminPort` を `CommitGate` に実装**: headless host が
  実 commit fence を迂回しないよう、既存の `CommitGate` が port 契約を
  直接満たす形にした（`mcp-poc` の `GatePorts` と同じ seam）。
  wrapper を介さないため port 経由と内部経路の verdict が乖離しない。
  `AdminPort::live_revision` は `CommitGate` が単一 project を所有する
  前提でその project の live revision のみを返す。
- **credential 検証失敗は新 variant `GatewayError::Authentication`**:
  既存 variant では authentication 失敗を正直に分類できなかったため
  fieldless variant を追加。全失敗が同一 verdict（存在を漏らさない）
  であり、`ErrorCategory::Authentication`/`RetryAdvice::Never` に写像。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-02-cli-adapter.md` — credential-backed Human
  判定・既存 GrantPolicy 再利用・CommitGate port 直実装の 3 方針を採用。

### Rejected

- 「local/interactive なら Human」の推論 — OS/session provenance を
  検証できない環境で Human を推測許可する設計は fail-closed 原則に反する
  ため棄却（exp E1）。
- CLI 専用の delegated grant 機構 — 既存 `Grant`/`Task`/`GrantPolicy`
  が同じ評価を既に提供するため棄却（exp E2）。

## Validation

実測結果の正本は `evidence/sv261/autx-02-cli-adapter-ledger.md`。
対象 SHA は PR head（branch `feat/261-cli-trusted-adapter`、base
`6ba9d3f`）。OS: Windows x86_64、toolchain は環境導入済み cargo
（`mise.toml` は Rust を pin しない）。

| コマンド | exit | 結果 |
|---|---|---|
| `mise run fmt-check` | 0 | `cargo fmt --all -- --check` 成功 |
| `mise run check synvelis-gateway` | 0 | `cargo check --locked --all-targets` 成功 |
| `mise run lint synvelis-gateway` | 0 | clippy `-D warnings` 成功 |
| `mise run test synvelis-gateway` | 0 | 26 test binary 全成功・**430 passed / 0 failed**（新規 `cli_adapter` 24 tests を含む。sv46 記録の 406 から +24） |

`mise run gw10`・`*-full`・`*-msrv`・実 OS credential/実 2-process・
Script 実バイナリ/Luau 経路は本スライス対象外（Deferred/NotRun —
ledger 参照）。cargo 直接実行（`cargo test -p synvelis-gateway
--test cli_adapter` 等）は開発中の補助証拠であり、canonical は
上記 mise タスク。

## Risks

- 実 OS session provenance（Windows logon session / macOS / Linux の
  console session 検証）は本スライスでは未実装。`CliSessionStore` の
  durable 実装と OS credential 連携は後続スライスに Deferred。
  本スライスの `LocalCliAdapter` は「検証済み provenance を持つ
  issuer のみが session を発行できる」契約を提供し、OS capability が
  Unsupported の環境では Human session を発行しない（fail-closed）。
- `MemoryCliSessionStore` は process restart で session を失う。
  durable store（`FilePermitStore` 相当）は runtime slice で実装する。
- 実 2-process commit・project lock・journal recovery の結合検証は
  `synvelis-project` port adapter 待ちで Deferred。
- `authority_class()` の `Human → VerifiedHuman` 写像は「Human
  principal を mint できるのは信頼 issuer のみ」という既存不変条件に
  依存する。seal mint が crate-internal に留まる限り安全だが、
  将来 issuer を増やす際は class 写像の再監査が必要。

## Follow-ups

- `docs/automation/design-trust-boundary.md`（SV-AUTX-00 contract set、
  synvelis#259 系）§2 の表は CLI session を `OriginKind::ExternalAdapter`
  に写像している。本スライスは Coordinator task spec の指示どおり
  `OriginKind::LocalCli` を新設した。behavior 差はない
  （`is_external`/`authorize_disclosure` は `PrincipalKind::Agent` で
  既に external 判定され、`Human` は disclosure 判定を short-circuit
  するため `Human+LocalCli` と `Human+ExternalAdapter` は全経路で
  同一 verdict）が、AUTX-00 文書鎖側で `LocalCli` 行への写像更新が
  必要 — 別 Worker 占有領域のため本スライスでは記録のみ。
- `synvelis-project` 側 port adapter（project open/lock/revision seam）。
- `synvelis-runtime` 新設（Script/Luau 統合、synvelis#272/#273）。
- 実 OS credential/session capability の Windows/macOS/Linux 判定と
  durable `CliSessionStore` 実装。
- 実 2-process commit・crash/restart の EXT-L3 検証。
