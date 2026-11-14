---
id: "EXP-034Xe6mTq8vVw2NsBdYkLm"
type: "experiment"
status: "completed"
title: "AUTX-02 CLI Trusted Local Adapter fail-closed 設計試行 (synvelis#261)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/261"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/autx-02-cli-adapter.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/command-gateway.md"
    - "docs/test-specs/autx-02-cli-adapter.md"
    - "docs/impl/ir/autx-02-cli-adapter.md"
    - "evidence/sv261/autx-02-cli-adapter-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/autx-02-cli-adapter.md"
    - "docs/impl/ir/autx-02-cli-adapter.md"
    - "evidence/sv261/autx-02-cli-adapter-ledger.md"
  defers: []
  deferred-by: []
---

# AUTX-02 CLI Trusted Local Adapter fail-closed 設計試行 (synvelis#261)

## 問題設定

synvelis#261 の headless authorization-boundary スライスで、
ローカル CLI / script / CI / scheduler からの呼び出しが
`Human` 権限や Grant を偽装・昇格できないことを gateway 契約と
して実装する必要があった。試行対象は (a) Human 判定を何に
紐付けるか（TTY/`--yes`/env/引数 vs session credential）、
(b) unattended delegated 権限を既存 Grant/Task/GrantPolicy に
載せるか新機構を作るか、(c) headless composition が commit 境界を
どう再利用するか、(d) credential 検証失敗を既存
`GatewayError` でどう分類するか、の 4 点。

## 試行と結果

### E1: Human 判定の根拠 — credential-backed session vs signal 推論

**仮説**: 「ローカル実行 + TTY 存在 + `--yes` 不在」等の OS/shell
signal の組み合わせで interactive Human を推論できるのではないか。

**結果**: 棄却。Issue 受入条件が「OS/session capabilities that
cannot prove a verified Human must fail closed and classify the
caller as delegated/agent rather than guessing Human」を要求し、
TTY・env・`--yes`・JSON 引数は全て caller が制御可能な入力であり
認証証拠にならない。採用した設計は `CliSessionStore` が発行した
session credential（random 256-bit、SHA-256 hash のみ永続化）を
`caller_verified` が検証した場合のみ `(Human, LocalCli)` を mint
する方式。provenance 検証を持つ issuer のみが session を発行でき、
Unsupported 環境では session が発行されない = fail-closed。
`CliSessionClaim` は `deny_unknown_fields` で authority 主張 field
（`yes`/`tty`/`as_human`/`interactive` 等）の混入を parse 段階で
拒否する。

### E2: delegated 権限 — 既存 GrantPolicy 再利用 vs CLI 専用機構

**仮説**: unattended 実行には scope/expiry/revoke を持つ専用の
delegated grant 型が必要ではないか。

**結果**: 棄却。`caller_delegated` が返す `Agent` principal に対し、
wire の `task`/`grant` 参照は既存 `GrantPolicy` が request-apply /
commit stage で毎回評価する。session expiry は認証層の失効であり
権限層の失効ではない — 二層を混同すると「session が生きている
限り grant 失効を見逃す」設計ミスを生む。`delegated_scope_is_
evaluated_per_call`・`delegated_grant_revocation_stops_the_next_
call`・`delegated_grant_expiry_is_enforced_by_the_policy_clock`・
`delegated_apply_budget_is_not_bypassed`・`delegated_approval_
policy_is_not_bypassed` が per-call 再評価を実証した。

### E3: commit 境界の再利用 — CommitGate port 直実装 vs wrapper

**仮説**: `HeadlessSessionHost` 用に `CommitPort`/`AdminPort` の
別実装（in-memory や転送 wrapper）を用意すべきか。

**結果**: 棄却。`CommitGate<F: CommitFence>` に両 port を直接
実装した。fence 経由の既存 commit/reconcile/cancel_apply/policy
mutation をそのまま公開するため、port 経由と内部経路で verdict が
乖離しない。`AdminPort::live_revision` は gate が所有する project
のみを返し、他 project には `None`（wrong revision を返さない）。
`NoCommitPort`/`NoAdminPort` は fail-closed 既定として温存。

### E4: credential 検証失敗の分類 — 既存 variant vs 新 variant

**仮説**: `GatewayError::MalformedRequest` や `Denied` で
credential 検証失敗を済ませられるのではないか。

**結果**: 既存 variant では不適。`MalformedRequest` は wire 形式の
失敗、`Denied` は認可拒否であり、authentication 失敗を分類する
正直な variant がなかった。fieldless `GatewayError::Authentication`
を追加し、`ErrorCategory::Authentication`・`RetryAdvice::Never` に
写像。全検証失敗（unknown/expired/revoked/malformed/non-human
record）が同一 verdict を返すため、caller は session の存在を
推測できない（existence-blind）。

### E5: 実測 — `tests/cli_adapter.rs` 24 tests

fail-closed fixture 群を実装し全件 Pass（`mise run test
synvelis-gateway` exit 0・430 passed/0 failed）。分類
（`class_*` 4 tests）・minting fail-closed（`unattended_claim_*`・
`interactive_claim_*`・`claims_deserialize_fail_closed`・
`sessions_bind_human_principals_only`）・secret 非開示
（`session_records_never_contain_the_usable_secret`）・wire smuggling
拒否（`wire_payloads_cannot_assert_authority_or_identity`・
`host_handle_rejects_unattended_human_shaped_requests`）・delegated
policy 評価（`delegated_*` 8 tests）・verified interactive 経路
（`verified_interactive_session_reaches_human_authority`）・
port 配線（`commit_gate_satisfies_both_adapter_ports`・
`headless_host_exposes_the_wired_commit_and_admin_ports`）を網羅。

## 結論

4 つの設計判断全てが fixture で裏付けられ、Issue 受入条件の
(3)(4)(7) を gateway 内契約として実装できた。実 OS credential・
durable session store・`synvelis-project` port adapter・
`synvelis-runtime`・実 2-process 検証は後続スライスに Deferred
（`evidence/sv261/autx-02-cli-adapter-ledger.md` 参照）。
