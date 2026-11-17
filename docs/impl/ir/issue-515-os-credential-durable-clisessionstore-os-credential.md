---
id: "IMPL-034YY6FE3030pd5QC8HIrQ"
type: "impl"
status: "completed"
title: "issue-515-os-credential — durable CliSessionStore + OS credential 供給"
created: "2026-11-17"
updated: "2026-11-17"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-515-os-credential.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# issue-515-os-credential — durable CliSessionStore + OS credential 供給

## Summary

synvelis#515 の verified-Human OS-credential session 供給を実装:
durable `FileCliSessionStore`（digest-only JSON、process restart
跨ぎ）を synvelis-gateway に追加し、synvelis-cli に OS credential
vault（Windows Credential Manager via `keyring`）+ `session
issue/list/revoke` provisioning verb + `--session <id>` claim 解決
+ `--session-store` durable 配線を追加した。負側契約（Agent 固定・
fail-closed・secret 非流出）は無変更で非回帰。

## Context

`docs/plans/issue-515-os-credential.md` T1-T5、
`docs/designs/issue-515-os-credential.md`（approved）、
`docs/specs/issue-515-os-credential.md`、AUTX-02（#261,
`docs/impl/ir/sv261-headless-session-host.md`）の deferred 残件。

## Implementation

### Changes

- `crates/synvelis-gateway/src/cli.rs`: `FileCliSessionStore` 追加
  （`{"sessions":[...]}` digest-only JSON、tmp+fsync+rename 原子
  書込、open 時 Human-kind invariant 再検証で forged file を
  fail-closed）。module doc の deferred 記録を採用結果へ更新。
- `crates/synvelis-gateway/src/lib.rs`: `FileCliSessionStore` export。
- `crates/synvelis-gateway/src/headless.rs`: deferred 記録更新。
- `crates/synvelis-gateway/tests/cli_adapter.rs`: FileCliSessionStore
  6 tests（restart verify・expiry/revocation persist・digest-only
  file・forged kind/malformed/agent-spec fail-closed）。
- `crates/synvelis-cli/src/session.rs`（新規）: store path 解決
  （`--session-store`/platform user-state 既定）、`OsCredentialVault`
  seam（Windows=keyring Credential Manager、非 Windows=Unsupported）、
  `issue`/`revoke`/`list`/`resolve_credential`。
- `crates/synvelis-cli/src/args.rs`: `--session`/`--session-store`
  global flags、`session issue|list|revoke` verbs、`--session` と
  `--session-credential` の相互排他。
- `crates/synvelis-cli/src/commands.rs`: session verb dispatch +
  `--session` → vault 読取 → `session_credential` 写像（claim 解決）。
- `crates/synvelis-cli/src/host.rs`: `boot_*` が
  `FileCliSessionStore::open` を `HeadlessSessionHost` に配線
  （corrupt file は `CliError::Setup` fail-closed）。
- `crates/synvelis-cli/src/{lib,main,analyze,script}.rs`: module 追加、
  help text、call site 更新。
- `crates/synvelis-cli/Cargo.toml` + root `Cargo.toml` + `Cargo.lock`:
  `keyring = "4.2"`（`cfg(windows)` target dep — 採用理由は PR 記録:
  `unsafe_code=forbid` 下での safe OS credential API、deps は
  windows-sys/zeroize/byteorder/regex のみで GUI/GPU/WebView なし）。
- `crates/synvelis-cli/tests/cli_e2e.rs`: Windows real-binary e2e
  2 tests（issue→list→revoke roundtrip + `--session` verified-Human
  attestation + fail-closed 非回帰）+ 非 Windows Unsupported test。
- `docs/automation/design-trust-boundary.md`: 採用経路の記録追記
  （§3 契約は不変）。
- `evidence/sv515/os-credential-session-ledger.md`: 判定 ledger。

### Decisions During Implementation

- `--session <id>` は commands 層で vault secret を in-process
  読取し既存 `session_credential` claim 経路へ写像 — secret が
  argv/env/log に出ない。`CliSessionClaim`/attest/verify は無変更。
- vault miss / 非対応 OS / revoke 済みは fail-closed:
  miss→`cli.invalid-input`(exit 2)、非対応→`cli.unsupported`(5)、
  record revoked→gateway `Authentication`。
- `session issue` は store.issue 成功後 vault 書込失敗時に record を
  revoke する補償 — live だが secret 未配備の孤児 session を残さない。
- `session revoke` は record revocation が権威、vault delete は
  best-effort（失敗しても record が fail-closed）。
- store 既定 path は env（`%APPDATA%`/`$HOME`/`XDG_STATE_HOME`）を
  場所解決のみに使用 — 権限証拠にしない。

## Related Experiments

### Adopted

- `docs/impl/exp/issue-515-os-credential.md` — FileCliSessionStore +
  keyring vault 採用、real Windows e2e pass を確認。

### Rejected

- OS keychain を session record backend にする案 — trait の
  列挙/expiry 契約に不適合（discovery 記録）。
- windows-sys/DPAPI 直叩き — `unsafe_code=forbid` で実装不可。
- `--session-credential` のみ — secret が argv に露出し AC 不充足。

## Validation

- `cargo test -p synvelis-gateway --test cli_adapter`: 30/30 pass
  （新規 6 FileCliSessionStore tests 含む）。
- `cargo test -p synvelis-cli --test cli_e2e session_`: 2/2 pass
  （実 Windows Credential Manager での issue→--session→verified-
  Human attestation、revoke 後 fail-closed）。
- 正規検証一式（mise fmt/check/lint/test/build/headless-deps/prek）
  は evidence/sv515 ledger に実測記録。

## Risks

- `session issue` は任意 local process が実行可能 — OS credential
  store が secret を OS user account に束縛する operator
  provisioning モデル（`mcp-poc permit` と同型）。対話存在証明は
  #114 系へ deferred。
- file store の同時書込は tmp+rename atomic、先例と同じ許容範囲。
- macOS/Linux は Unsupported — 実装需要があれば別 slice。

## Follow-ups

- #114 系: 実 credential prompt/Windows Hello 等の対話存在証明。
- macOS/Linux vault backend（需要があれば）。
- #267/#114 lane: 実 2-process・GUI 受入。
