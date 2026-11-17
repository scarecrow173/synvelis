---
id: "SPEC-034YXIoA2YsEXy4UBKPrXl"
type: "spec"
status: "draft"
title: "issue-515-os-credential — verified-Human OS-credential session 供給"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/discovery/issue-515-os-credential.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/impl/ir/sv261-headless-session-host.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/automation/design-trust-boundary.md"
    - "docs/test-specs/autx-02-cli-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-515-os-credential.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-515-os-credential — verified-Human OS-credential session 供給

## What Should Be Built

`CliSessionStore` trait に沿った durable（process restart 跨ぎ）
file-backed store `FileCliSessionStore` を synvelis-gateway に追加
し、synvelis-cli の durable boot で `MemoryCliSessionStore` を
置き換える。さらに operator provisioning verb `session issue` /
`session list` / `session revoke` と、OS credential store
（Windows Credential Manager）経由で secret を一度きり返却する
claim 解決 flag `--session <id>` を synvelis-cli に追加し、
interactive claim が verified-Human `CallerIdentity` を供給できる
経路を実装する。

## Why Now

AUTX-02（synvelis#261）は fail-closed の interactive claim 契約
だけを実装し、「実 OS credential/session capability」と durable
CLI session storage を deferred/unowned として残した。#267 は
拒否方向 E2E を所有するが、正方向の供給経路は未所有。Issue #515
はその残件を閉じる実装 slice であり、既存の fail-closed 契約を
弱めずに trusted session supply を追加する。

## Users and Value

- ローカル CLI operator: 自分の OS user account に束縛された
  session を発行・失効し、対話 claim として Human authority を
  使える。
- automation/agent 側: unattended claim は従来どおり Agent
  固定で、OS session を持たない限り Human を mint できない
  fail-closed を維持する。

## Scope

### In Scope

- `FileCliSessionStore`（digest-only JSON、tmp+fsync+rename 原子
  書込、Human kind invariant を open 時に再検証 —
  `FilePermitStore` 先例と同型）。
- synvelis-cli の session provisioning（`session issue` /
  `session list` / `session revoke`）と OS credential store
  （Windows Credential Manager via `keyring` crate、
  `cfg(windows)` 限定）への secret 一度きり返却/削除。
- `--session <id>` による claim 解決（OS vault から secret を
  in-process 読取 → `CliSessionClaim::Interactive`）。
- `--session-store <path>` による store path 指定と platform
  user-state 既定。
- module doc の deferred 記録を採用結果へ更新
  （`cli.rs`/`headless.rs`）。

### Out of Scope

- `synvelis-runtime` crate 新設（後続 AUTX slice）。
- macOS/Linux の OS keychain 経路（Unsupported を正直に記録）。
- 実 credential prompt / Windows Hello 等の対話存在証明
  （#114 系 gate へ deferred — OS user account 束縛まで）。
- GUI/実 2-process 受入（#267/#114 lane）。
- Ruviform 側の変更。

## Requirements

- durable store は process restart 後も session の expiry /
  revocation / 一度きり secret 返却契約を維持する。
- session record は usable secret を含まず digest のみ保存し、
  log/stdout/serialized record に secret が流出しない。
- interactive claim は OS credential 検証（OS vault 読取 +
  store 側 digest 検証）を通じてのみ verified-Human
  `CallerIdentity` を供給する。
- unattended claim・wire payload・偽装 OS/TTY/env/args では
  Human を mint できない fail-closed を維持する。
- 少なくとも Windows で OS credential 経路が動作し、他 OS は
  Unsupported/NotRun を正直に記録する。

## Acceptance Criteria

- [ ] durable `CliSessionStore` が実装され、restart 後の
  expiry/revocation/一度きり返却が test で pin される。
- [ ] interactive claim が OS credential 検証経由で verified-
  Human `CallerIdentity` を供給できる経路が存在し、少なくとも
  1 OS で動作する。
- [ ] unattended/wire/偽装系の fail-closed 非回帰が既存 suite
  で確認される。
- [ ] secret 非流出契約が維持される。
- [ ] `mise run test synvelis-gateway` / `mise run test-cli`
  が pass する。
- [ ] headless/fixture lane を実 OS 受入に昇格せず、残る実 OS
  keychain/実 2-process 環境は環境別に NotRun を記録する。

## Deferred Design Concerns

- 実 credential prompt / Windows Hello / macOS Keychain /
  Linux Secret Service の対話存在証明 — #114 系の実 OS/GUI
  受入 gate へ deferred。本 slice は OS user account 束縛の
  credential store まで。
- session record 本体を OS keychain backend にする設計 —
  列挙/expiry 判定が trait 契約に合わないため file store を採用。

## Sources and Relations

- `docs/discovery/issue-515-os-credential.md`（探索結果・採用経路の
  根拠）
- Issue: https://github.com/scarecrow173/synvelis/issues/515
- 契約正本: `docs/automation/design-trust-boundary.md` §2/§3
- 前提: `docs/impl/ir/sv261-headless-session-host.md`（AUTX-02）
- 先例: `FilePermitStore`（`mcp_permit.rs`）、`mcp-poc permit`
