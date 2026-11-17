---
id: "TASK-034YXTWjUPI81SOi9gb2W3"
type: "task"
status: "todo"
title: "issue-515-os-credential — durable store + OS credential 実装"
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
    - "docs/plans/issue-515-os-credential.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-515-os-credential.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-515-os-credential.md"
  references: []
  defers: []
  deferred-by: []
---

# issue-515-os-credential — durable store + OS credential 実装

## Work

plan T1-T5 を逐次実施: gateway `FileCliSessionStore` → CLI
`OsCredentialVault`/`session` verbs/`--session` 解決 → host 配線
→ real binary e2e → docs/evidence/検証。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/issue-515-os-credential.md`
- `docs/specs/issue-515-os-credential.md`
- `docs/designs/issue-515-os-credential.md`

## Dependencies

- `docs/plans/issue-515-os-credential.md`
- 前提 slice: AUTX-02（`docs/impl/ir/sv261-headless-session-host.md`）

## Affected Files

- `crates/synvelis-gateway/src/cli.rs` / `headless.rs` / `lib.rs`?
- `crates/synvelis-gateway/tests/cli_adapter.rs`
- `crates/synvelis-cli/src/{session,args,commands,host,main,lib}.rs`
- `crates/synvelis-cli/tests/cli_e2e.rs`
- `crates/synvelis-cli/Cargo.toml`（cfg(windows) keyring）
- `docs/automation/design-trust-boundary.md`（記録のみ）
- `evidence/sv515/`、`docs/impl/{ir,exp}/issue-515-os-credential*`

## Done When

- [ ] durable store の restart/expiry/revocation/tamper/secret
  非含有が test で pin される
- [ ] `session issue/list/revoke` + `--session` が real binary で
  verified-Human claim を供給する（Windows 実測）
- [ ] fail-closed 非回帰（unattended/wire/偽装/unknown/expired/
  revoked）が維持される
- [ ] 正規検証一式が pass、未検証は NotRun/Unsupported 記録

## Verification

- [ ] `mise run fmt-check` / `check`/`lint`/`test` gateway・cli /
  `build-cli` / `check-headless-deps` / `prek run --all-files`
