---
id: "TSPEC-034YXSZ2kuWzgTf2b6XeLT"
type: "test-spec"
status: "draft"
title: "issue-515-os-credential — session 供給の検証仕様"
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
  implements: []
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
  verifies:
    - "docs/specs/issue-515-os-credential.md"
    - "docs/designs/issue-515-os-credential.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-515-os-credential — session 供給の検証仕様

## Purpose

durable session store と OS credential 供給経路が spec/design の
契約（expiry/revocation/一度きり返却/digest-only/secret 非流出/
fail-closed 非回帰）を満たすことを検証する。

## Feature

verified-Human CLI session 供給（durable `CliSessionStore` + OS
credential vault 経路）

## Rules

- Rule: session record は usable secret を含まず SHA-256 digest のみ
  永続化し、process restart 後も expiry/revocation/一度きり返却を
  維持する。
- Rule: interactive claim は store verify を通過した credential のみ
  が verified-Human `CallerIdentity` を供給し、unknown/malformed/
  expired/revoked/non-human は blind に fail する。
- Rule: unattended claim・wire payload authority・偽装 OS/TTY/env/
  args は Human を mint しない。
- Rule: OS 非対応環境では `--session`/vault 経路は Unsupported で
  fail-closed し、headless 成功を実 OS 受入に昇格しない。

## Examples

- Example: `session issue` → 新 process で `--session <id>` →
  verified Human claim 成功; `session revoke` → 同 claim が
  fail-closed。
- Example: file store 改竄（kind=Agent 混入・digest 改竄）→ open
  または verify が fail-closed。
- Example: `--session` と `--session-credential` 同時指定 → Usage
  error。
- Example: 存在しない session id / vault miss → blind auth failure。

## Guarantees

- restart 跨ぎの契約維持、secret が log/stdout/record/file に
  一度きり返却以外で出ないこと、負側非回帰。

## Non-goals

- 実 credential prompt/Windows Hello/2-process GUI 受入（#114/#267
  lane）。macOS/Linux keychain の動作保証。

## Risk

- store/ vault の fail-open 化や secret 漏洩は trust boundary の
  根幹を壊すため、全 fail 経路を test で pin する。
