---
id: "PLAN-034YXTpsA7X2LIoy8uNpK2"
type: "plan"
status: "approved"
title: "issue-515-os-credential 実装計画"
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
    - "docs/specs/issue-515-os-credential.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-515-os-credential.md"
    - "docs/designs/issue-515-os-credential.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-515-os-credential.md"
  references: []
  defers: []
  deferred-by: []
---

# issue-515-os-credential 実装計画 Implementation Plan

## Goal

`FileCliSessionStore`（durable digest-only file store）と OS
credential vault 経由の `session issue/list/revoke` +
`--session <id>` claim 解決を synvelis-cli に実装し、verified-
Human session 供給経路を開通する。

## Scope

単一 implementation stream — gateway store 追加 → CLI
provisioning/vault/claim 解決 → boot 配線 → tests → docs/
evidence の順に依存するため分割しない。

## Upstream Documents

- `docs/specs/issue-515-os-credential.md`
- `docs/designs/issue-515-os-credential.md`（approved）
- `docs/discovery/issue-515-os-credential.md`

## Design Inputs

- `docs/designs/issue-515-os-credential.md`

## File Map

- `crates/synvelis-gateway/src/cli.rs` — `FileCliSessionStore` 追加、
  module doc の deferred 記録更新
- `crates/synvelis-gateway/src/headless.rs` — module doc 更新
- `crates/synvelis-gateway/src/lib.rs` — export 追加（必要時）
- `crates/synvelis-gateway/tests/cli_adapter.rs` — durable store
  restart/expiry/revocation/tamper/secret 非含有 test
- `crates/synvelis-cli/src/session.rs`（新規）— `OsCredentialVault`
  seam + Windows keyring 実装 + 非対応 OS Unsupported
- `crates/synvelis-cli/src/args.rs` — `--session` / `--session-store`
  flag、`session` command 追加
- `crates/synvelis-cli/src/commands.rs` — `session` verb dispatch、
  claim 解決 seam
- `crates/synvelis-cli/src/host.rs` — `FileCliSessionStore::open`
  配線、`--session-store` path 解決
- `crates/synvelis-cli/src/main.rs` — help text 更新
- `crates/synvelis-cli/src/lib.rs` — module 追加
- `crates/synvelis-cli/tests/cli_e2e.rs` — real binary e2e 追加
- `crates/synvelis-cli/Cargo.toml` — `keyring` cfg(windows) dep
- `docs/automation/design-trust-boundary.md` — 採用経路の記録のみ
- `evidence/sv515/` — 採用経路・判定 ledger
- `docs/impl/ir|exp/issue-515-os-credential*` — impl 記録

## Implementation Sequence

- [ ] T1: `FileCliSessionStore` を `cli.rs` に実装し gateway tests
  で pin（restart/expiry/revocation/tamper/secret 非含有）
- [ ] T2: `OsCredentialVault` seam + Windows keyring 実装、
  `session issue/list/revoke` verb、`--session` claim 解決
- [ ] T3: `host.rs` durable store 配線 + `--session-store` path
- [ ] T4: real binary e2e（issue → --session → verified Human、
  revoke fail-closed、secret 非流出、Unsupported OS）
- [ ] T5: module doc 更新、design-trust-boundary 記録、evidence
  ledger、impl ir/exp、正規検証一式

## Dependencies and Risks

- `keyring` v4 API（`v1::Entry`）の正確な signature は
  source-driven-development で docs.rs 確認してから実装。
- `check-headless-deps` で GUI/GPU/WebView 非含有を確認。
- session store 既定 path は env（`%APPDATA%` 等）を場所解決の
  みに使用 — 権限証拠にしない。

## Task Breakdown

- T1 → T2 → T3 → T4 → T5（逐次。T1/T2 は独立だが同 crate 群の
  ため逐次実施）。

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check synvelis-gateway` / `lint` / `test`
- [ ] `mise run check-cli` / `lint-cli` / `test-cli` / `build-cli`
- [ ] `mise run check-headless-deps`
- [ ] `mise exec -- prek run --all-files`

## Open Questions / Gaps

- `keyring` 4.x `Entry` API の delete_credential 有無は実装時に
  docs.rs で確認（`delete_credential` または `delete_password`）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution
- Independent work streams: none
- User approval required before dispatch: yes（review-protocol で代替）

## Review Handoff

- reviewer focus: 負側契約の非回帰、secret 非流出、OS 非対応の
  fail-closed、docs index 未同期の明記。
