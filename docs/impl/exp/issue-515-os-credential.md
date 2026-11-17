---
id: "EXP-034YY6GabcDEFghIJKLmN"
type: "experiment"
status: "completed"
title: "issue-515-os-credential durable store + OS credential 経路の実装試行"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/515"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/issue-515-os-credential-durable-clisessionstore-os-credential.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-515-os-credential.md"
    - "docs/designs/issue-515-os-credential.md"
    - "docs/test-specs/issue-515-os-credential.md"
    - "evidence/sv515/os-credential-session-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/issue-515-os-credential.md"
    - "docs/impl/ir/issue-515-os-credential-durable-clisessionstore-os-credential.md"
    - "evidence/sv515/os-credential-session-ledger.md"
  defers: []
  deferred-by: []
---

# issue-515-os-credential durable store + OS credential 経路の実装試行

## 問題設定

synvelis#515: AUTX-02（#261）で deferred された verified-Human
OS credential session 供給の正方向を、既存 fail-closed 契約を
弱めずに実装する。焦点は durable `CliSessionStore` backend の選定、
`unsafe_code=forbid` 下での OS credential アクセス手段、session
発行/revoke provisioning と `--session` claim 解決の CLI 表面。

## 試行と結果

- **FileCliSessionStore**: `FilePermitStore`（`mcp_permit.rs`）と
  同型の digest-only JSON + tmp/fsync/rename で実装。open 時に
  Human-kind invariant を全 record に再適用し、forged file は
  fail-closed。restart 跨ぎの expiry/revocation/一度きり返却を
  `tests/cli_adapter.rs` 6 tests で pin — 全 pass。
- **OS credential vault**: `keyring` v4（`v1::Entry` API →
  `windows-native-keyring-store` → Windows Credential Manager）を
  `cfg(windows)` target 依存として採用。`unsafe_code=forbid` の
  ため windows-sys 直叩き不可、safe wrapper が必須。service
  `synvelis-cli`、username=session id、secret=password。
  実 Windows Credential Manager で `session issue` → `--session`
  → verified-Human attestation の real-binary e2e が pass
  （`cli_e2e.rs` 2 tests）。
- **claim 解決**: `--session <id>` は commands 層で vault から
  secret を in-process 読取し `session_credential` へ写像 —
  secret は argv/env/log に出ない。`--session`/`--session-credential`
  同時指定は Usage error。非 Windows は `CliError::Unsupported`
  （exit 5）で fail-closed。
- **棄却した代替**: OS keychain を record backend にする案
  （trait の列挙/expiry 契約に不適合）、windows-sys/DPAPI 直叩き
  （unsafe forbid）、`--session-credential` のみ（argv 露出・
  AC 不充足）— いずれも discovery/design に記録。

## 残件（正直な記録）

- 対話存在証明（credential prompt/Windows Hello）は #114 系へ
  deferred — 本 slice は OS user account 束縛まで。
- macOS/Linux の vault 実装は Unsupported（NotRun）。
- 実 2-process/GUI 受入は #267/#114 lane。
