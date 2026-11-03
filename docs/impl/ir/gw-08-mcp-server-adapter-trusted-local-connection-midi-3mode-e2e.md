---
id: "IMPL-0Gw08McpAdapterEdge01"
type: "impl"
status: "completed"
title: "GW-08 MCP Server Adapter・Trusted Local Connection・MIDI 3-Mode E2E"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/59"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/src/mcp.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/mcp_permit.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/mcp_server.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/mcp_tools.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/mcp_adapter.rs"
      - type: "file"
        path: "crates/mcp-poc/Cargo.toml"
      - type: "file"
        path: "crates/mcp-poc/src/main.rs"
      - type: "file"
        path: "crates/mcp-poc/tests/stdio_e2e.rs"
      - type: "file"
        path: "docs/impl/ir/gw-08-mcp-server-adapter-trusted-local-connection-midi-3mode-e2e.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-gateway/src/lib.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/59"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/gw-01-gateway-contract.md"
    - "docs/impl/ir/gw-02-authorization-policy.md"
    - "docs/impl/ir/gw-03-context-projection.md"
    - "docs/impl/ir/gw-04-semantic-operation-preparation-effect-evaluation-linked-midi-scope.md"
    - "docs/impl/ir/gw-05-immutable-prepared-changeset-semantic-preview-approval-receipt-state-machines.md"
    - "docs/impl/ir/gw-06-authorized-commit-gate-revoke-linearization-idempotency-crash-recovery.md"
    - "https://github.com/scarecrow173/synvelis/issues/45"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/59"
  derived-by: []
  verifies:
    - "https://github.com/scarecrow173/synvelis/issues/59"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    - "PoC — line-oriented stdio JSON-RPC transport(client channel)+ loopback TCP control socket"
    - "PoC — FilePermitStore(SHA-256 hashed secrets)+ CommitGate journal による restart reconcile"
---

# GW-08 MCP Server Adapter・Trusted Local Connection・MIDI 3-Mode E2E

GW-01..06 の internal gateway surface を外部 MCP(Model Context
Protocol)boundary として公開する adapter を実装する。MCP は protocol
edge であり、Project / Policy / Job / approval / commit 意味論の権威は
すべて内部(GW-01 registry・GW-02 grant/policy・GW-03 context・GW-04
prepare・GW-05 proposal/approval・GW-06 commit gate)に留まる。
tool 名は内部 capability の 1:1 写像ではなく semantic name
(`project.inspect`・`midi.notes.query`・`midi.edit.prepare`・
`proposal.preview`・`proposal.apply`・`operation.get`・
`operation.cancel`・control-only `proposal.approve`)を採る。

trusted local connection は explicit **connection permit** で確立する:
`synvelis.connect` が唯一の credential 経路で、verified permit record +
adapter attestation(seal)から `CallerIdentity` を構成する。MCP
`clientInfo`・agent 自己申告・tool 引数は trust に一切寄与しない。
session は `McpChannel::Client`(agent principal)と
`McpChannel::Control`(human principal)に分離し、許可失効・kind 不一致・
unknown secret はすべて同一の blind `PERMIT_INVALID` verdict で閉じる。

## 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `mcp.rs`(新規) | protocol 定数(`PROTOCOL_VERSION="2025-06-18"`、`SERVER_NAME="synvelis-mcp"`、`TOOL_CONTRACT="1.0.0"`、`CONNECT_METHOD="synvelis.connect"`)、`McpChannel`(Client/Control)、`McpBounds`(message/args/page/range/operation/selector の上限)、JSON-RPC error code(std + `PERMIT_INVALID=-32001`/`PERMIT_REQUIRED=-32002`/`FORBIDDEN=-32003`/`UNKNOWN_RESOURCE=-32004`/`ADAPTER_DISABLED=-32005`)、`AdapterFault`(`invalid-arguments`/`limit-exceeded`/`unavailable`/`unknown-operation`/`not-cancellable` の safe vocabulary)、`CallToolResult`(`isError`+`structuredContent`)、`rpc_result`/`rpc_error` |
| `mcp_permit.rs`(新規) | `PermitSpec`/`PermitRecord`/`PermitIssue`、`PermitStore` trait、`MemoryPermitStore`/`FilePermitStore`。secret は `OsRng` 由来の 256-bit CSPRNG 値で hash 比較は constant-time、file 永続化は SHA-256 hash のみ平文を残さず `sync_all`→rename で durable。verify は existence-blind かつ channel-bound、revoke は record を消さず revocation を刻む。`check_channel_kind` で Client↔agent・Control↔human を発行・永続化 load・verify の各点で強制(offline provisioning と control issue が同一検証を共有) |
| `mcp_server.rs`(新規) | `McpHost`(gateway・permit store・`CommitPort`/`AdminPort`・clock・`McpConfig{adapter,bounds,enabled}`)+ `McpSession`(bound identity)+ `BoundIdentity`(record+attested `CallerIdentity`)。method dispatch: `initialize`(version negotiation、非対応は `INVALID_PARAMS`)/`synvelis.connect`/`tools/list`/`tools/call`/`resources/list`/`resources/templates/list`/`resources/read`/`prompts/list`/`logging/setLevel`/`completion/complete`/`ping`/notification は response を返さない。`resources/read` は bound project の `synvelis://project/<tagged>/inspect` のみを認め foreign URI は blind `UNKNOWN_RESOURCE`。control method(`synvelis.issue-permit`/`issue-task`/`issue-grant`/`revoke-permit`/`revoke-grant`/`revoke-task`/`status`)は client channel では method-not-found — channel→bound→method vocabulary の順で絞り、`issue-*` の project 引数は bound project に限定。op book は operation→(principal,project,task,grant,status) を owner-scoped で保持し、foreign handle は blind `unknown-operation`、eviction は scope 内でのみ起きる。`settle_op`/`reconcile_op`/`cancel_op` が retry・reconnect・restart の durable verdict を支える。`translate_outcome` が `GatewayOutcome`/`ErrorEnvelope` を safe MCP shape へ射影 |
| `mcp_tools.rs`(新規) | tool catalog(JSON-Schema、`proposal.approve` は control のみ)+ dispatch。`check_keys` で unknown/missing field を deny、`str_arg`/`u64_arg`/typed `ResourceRef` で wrong-kind tagged ID を existence-blind に拒否。`page_args` が cursor continuation(original param を引き継がない)と `at_revision` pinning を統一。`wire_for` が唯一の wire producer: request_id・capability・schema・project・args・consistency を組み立て、`task`/`grant` は Client channel かつ permit record 由来のみ注入 — client 引数は authority field に一切触れない。query 系は fixed-snapshot/read-at、prepare/apply は expected-revision consistency。`proposal.apply` は accepted handle を返し `CommitPort::commit`→`reconcile` で durable verdict(`committed`/`rejected`/`indeterminate`/`conflicted`/`in-flight`/`commit-requested`)を報告、commit の内部失敗は reconciled verdict を優先。`operation.cancel` は `CommitPort::cancel` の tri-state(`Ok(true)`=cancel が勝った / `Ok(false)`=apply が先に進んだ)を報告し、負けた場合は実際の settled status を返す |
| `commit.rs` | `CommitGate::cancel_apply` を commit と同一 fence 配下に追加 — race する cancel/commit は直列化され、`AwaitingApproval`/`CommitRequested` は `Rejected` へ、settled 済みは `Ok(false)` を返し `operation.cancel` が嘘の cancelled verdict を報告しない |
| `lib.rs` | `mcp`/`mcp_permit`/`mcp_server`/`mcp_tools` module export + trust boundary 記述 |
| `crates/mcp-poc`(新規 crate) | `serve` — `boot` で durable project store・GW-01..05 stack(`gateway_stack`)・`CommitGate`(journal)+ `McpHost` を組み立て、stdin/stdout 上の line-oriented JSON-RPC を `McpChannel::Client` で流す。`--control-port` で 127.0.0.1 loopback TCP に Control session の listener を立てる(authenticated control PoC — permit が認証、同時接続は cap)。`read_frame` が allocation 前から inbound byte cap を強制し、超過 frame は drain して framing を維持したまま拒否。`permit issue/list/revoke` CLI — offline provisioning は `check_channel_kind` を共有。restart 時 `CommitGate::open` が journal replay で ledger を復元し `operation.get` が durable truth に reconcile |
| `tests/mcp_adapter.rs`(新規) | in-process 27 テスト:connect identity binding・blind verdict・channel/kind・discovery≠permission・control blindness・disabled adapter・version negotiation・notification・malformed message・capability allowlist・typed ID・unknown/missing field・bounds・wire authority・revoke linearization・3 モード(Suggest/ApproveToApply/DelegatedApply)・commit failure verdict・restart reconcile・op handle scope/blindness・control admin・permit issuance・file store 永続化・secret tamper |
| `tests/stdio_e2e.rs`(新規) | real-transport 5 テスト:`suggest_mode_prepares_but_apply_is_denied_over_real_transport`、`approve_to_apply_needs_the_control_receipt_over_real_transport`(client stdio + control TCP)、`delegated_apply_commits_retries_without_duplicates_and_survives_restart`(real `ProjectController` commit → reconnect reconcile → restart journal replay → 重複 commit 無し)、`boundary_attacks_fail_closed_over_real_transport`(identity injection・wrong-kind ID・foreign op handle・oversized message・resource disclosure)、`permit_cli_provisions_lists_and_revokes` |

## 設計上の主要判断

| 判断 | 理由 |
| --- | --- |
| identity は verified permit record + adapter seal のみから構成 | `clientInfo`・agent 申告・tool 引数を信じると identity injection が成立する — `synvelis.connect` 以外に credential 経路を置かず、wire の task/grant/project は permit record 由来の値だけが入る |
| channel/kind mismatch・unknown・expired・revoked を同一 `PERMIT_INVALID` で閉じる | verdict 差異は permit existence と状態の oracle になる — secret hash も結果/error/record に出さない(file store は hash のみ永続化) |
| control method は client channel で method-not-found | blind surface: client が admin method の存在を探れないよう discovery 自体を絞る(SEC 系 attack battery で検証) |
| tool discovery ≠ execution permission | `tools/list` は semantic contract の公開であり、各 call は現在の grant/policy/disclosure を `Gateway::handle` が再評価する — revoke は次の call で即 deny(GRANT-05) |
| `wire_for` が唯一の wire producer | task/grant 注入・consistency・typed ID の責務を一箇所に集約し、tool 毎のバイアス漏れを防ぐ(DRY/boundary) |
| `proposal.apply` は accepted handle + reconcile で答える | MCP task を job 権威にしない — apply は「受理」までを返し、`operation.get` が `CommitPort::reconcile`(durable journal)で Settled/InFlight/Unknown を報告する。retry は settle 済み verdict を返し再 apply しない → duplicate commit を防ぐ |
| commit の内部失敗は reconciled verdict を優先 | gate が `Rejected`/`Conflicted`/`Indeterminate` を durable 記録する以上、`commit` 返り値の envelope より reconcile の durable outcome を信じる(settled_verdict) |
| op handle は (principal,project,task,grant) owner scope + blind | foreign handle 参照は `unknown-operation` 一択 — ID 形式(parse できる kind)と存在確認を分け existence-blind を維持。committed receipt の reconcile でも project を照合し、session 間 eviction は起きない |
| `operation.cancel` は `CommitGate::cancel_apply` で fence 直列化 | fenced commit と cancel が race しても honest winner が決まる — cancel が負けたら実際の settled status を報告し、false "cancelled" を出さない |
| adapter は `enabled:false` が既定、MCP 無効で DAW は独立動作 | `ADAPTER_DISABLED` で surface 全体を閉じる — GW 内側の authority は MCP の有無に依存しない |
| transport は PoC(stdio client + loopback control)として明示 | Streamable HTTP との比較・remote exposure・production transport の決定は本 impl の外 — `main.rs` の doc と本 record で「PoC transport、決定ではない」と宣言 |

## 検証エビデンス

| 分類 | エビデンス |
| --- | --- |
| real-transport(MIDI 3 モード・boundary・reconnect・CLI) | `cargo test -p mcp-poc` — 5 tests pass。real `ProjectController`+`CommitGate`+journal+`FilePermitStore` 上で stdio/TCP を経由して検証。`delegated_apply_..._survives_restart` は commit→restart→ledger replay→no-duplicate を durable に確認 |
| fixture/in-process(adapter・permit・dispatch・op book) | `cargo test -p synvelis-gateway --test mcp_adapter` — 27 tests pass |
| 全体 | `mise run fmt-check`/`check`/`lint`(-D warnings)/`test` すべて green |

注記: real-transport で検証したのは「line-oriented stdio + loopback TCP」
の 2 channel であり、Streamable HTTP/remote transport は未実装・未検証。
MIDI 3 モードは real `ProjectController` 経由で commit まで検証済みだが、
transport は PoC である — production transport の決定は本 issue の外。

## 残件・既知の制限

- Streamable HTTP transport との比較・選定・remote exposure は PoC scope 外。
- control listener は loopback+permit 認証の PoC posture; mutual-TLS /
  platform IPC(named pipe 等)への移行は transport 決定時に扱う。
- `FilePermitStore` は secret hash を永続化するが store file 自体の
  ACL/integrity は host 環境責任(本 PoC で tamper detection は
  hash mismatch→invalid permit で fail closed、integrity 署名は未実装)。
