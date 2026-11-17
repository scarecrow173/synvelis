---
id: "DESIGN-034YXStSteikAVmmfTjwB3"
type: "design"
status: "approved"
title: "issue-515-os-credential — durable session store + OS credential 経路の設計"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/issue-515-os-credential.md"
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
  depends-on:
    - "docs/impl/ir/autx-02-cli-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/automation/design-trust-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-515-os-credential.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-515-os-credential — durable session store + OS credential 経路の設計

## Context

AUTX-02（#261）は `CliSessionStore` trait・`MemoryCliSessionStore`・
`LocalCliAdapter`・`CliSessionClaim`・`CliSessionRecord` を
fail-closed 契約と共に実装済みだが、store は process 内メモリのみ、
OS credential 供給経路と session 発行/revoke verb は未存在。
`docs/automation/design-trust-boundary.md` §3 は「CLI flags・
env・TTY・stdin・wire payload は Human の provenance を mint しない」
と定め、credential 検証結果のみを信頼根拠とする。本設計はその契約を
変えずに、trusted supply 経路を追加する。

## Scope

### In Scope

- `FileCliSessionStore`: `CliSessionStore` trait の file-backed
  durable 実装（`crates/synvelis-gateway/src/cli.rs`）。
- OS credential vault 抽象（`crates/synvelis-cli/src/session.rs` 等）
  と `session issue/list/revoke` verb、`--session <id>` claim 解決。
- `Host::shared_handles`/`boot_*` の session store 配線
  （`--session-store` 指定または user-state 既定 path）。
- gateway/cli tests の拡張と module doc 更新。

### Out of Scope

- `synvelis-runtime` crate、macOS/Linux OS keychain 実装、
  GUI/実 2-process/実 credential prompt 受入、Ruviform 変更。

## Components and Boundaries

- `CliSessionStore`（trait, `cli.rs`）: session record の
  issue/verify/revoke/列挙。実装は memory / file の 2 系統。
  `FileCliSessionStore` は digest-only JSON を tmp+fsync+rename で
  原子的に永続化（`FilePermitStore` 先例踏襲）。open 時に全 record
  の Human kind invariant を再検証し、不正 file は fail-closed。
- `OsCredentialVault`（synvelis-cli 内部 seam）: session id を
  username、usable secret を password とする OS credential store
  への set/get/delete。Windows 実装は `keyring` crate
  （`windows-native-keyring-store` → Credential Manager）。
  非 Windows は `Unsupported` を返す fail-closed。
- `session` verb 群（synvelis-cli commands）: operator
  provisioning。`issue` は store.issue で secret を一度だけ受け取り
  OS vault に格納（stdout には record metadata のみ、secret 非出力）。
  `revoke` は store.revoke + vault delete。`list` は digest-only
  record の列挙。
- claim 解決（`commands::run` → Args）: `--session <id>` 指定時、
  vault から secret を in-process 読取し `CliSessionClaim::
  Interactive { credential }` を構成。`--session-credential` との
  同時指定は Usage error。vault 読取失敗・非対応 OS は
  Setup/Unsupported error で fail-closed。
- `host.rs`: `HeadlessSessionHost::new` に渡す store を
  `FileCliSessionStore::open(path)` に変更。path は `--session-
  store` flag または platform user-state 既定
  （Windows: `%APPDATA%\synvelis\cli-sessions.json`）。open 失敗は
  `CliError::Setup` で fail-closed。

## Data and Control Flow

1. `synvelis-cli session issue [--ttl-ms N] [--user ID]`:
   `FileCliSessionStore::open` → `issue`（secret 生成・digest 保存・
   一度だけ返却）→ `OsCredentialVault::set(session_id, secret)` →
   record metadata を JSON 出力（secret 非含有）。
2. `synvelis-cli --session cli-XXXX <command>`:
   args parse → `resolve_session_claim` が vault.get(session_id)
   → `Interactive { credential }` → `claim_for` →
   `HeadlessSessionHost::handle_claim` → `attest` → store.verify
   → verified Human `CallerIdentity`（expiry/revocation は store
   側で fail-closed）。
3. `synvelis-cli session revoke cli-XXXX`:
   store.revoke（persist）→ vault.delete（best-effort、失敗は
   warning のみ — record 側 revocation が権威）。
4. process restart: file store が record を復元し、expiry/
   revocation/digest 検証は従来契約どおり。

## Non-Functional Constraints

- `unsafe_code = "forbid"`（workspace lint）: OS API 直叩き不可の
  ため safe wrapper `keyring` を採用。`[target.'cfg(windows)'.
  dependencies]` 限定で非 Windows dep 木を増やさない。
- secret は argv/env/log/stdout/session record に出さない。
  `--session` は id のみを取り secret は in-process のみ。
- session record file は user-state dir（他 user から読めない
  前提）。file 改竄は open 時 kind 再検証 + digest 不一致で
  fail-closed。
- 既存負側契約（Agent 固定・delegated 経路・wire payload 拒否・
  `AdapterSeal`/`CallerClass`）は無変更。

## Risks and Trade-offs

- **任意 local process が `session issue` を実行可能**: OS
  credential store が secret を OS user account に束縛するため、
  実効的な Human mint には同一 OS user context での vault 読取が
  必要 — `mcp-poc permit` と同じ operator provisioning モデル。
  対話存在証明（GUI prompt/Hello）は #114 系へ deferred と明記。
- **keyring 新規依存**: `windows-native-keyring-store` は
  windows-sys/zeroize/byteorder/regex のみ（GUI/GPU/WebView なし）
  — `check-headless-deps` で確認。採用理由は PR に記録。
- **file store の同時書込**: tmp+rename で atomic、複数 process
  の同時 issue は稀 — 先例 `FilePermitStore` と同じ許容範囲。
- **OS vault delete 失敗時**: record revocation が権威のため
  best-effort。孤児 credential は session id 単位で無害。

## Deferred Design Concerns

- 対話存在証明（Windows Hello / credential prompt / Touch ID）:
  #114 系実 OS/GUI 受入 gate へ deferred。本 slice は OS user
  account 束縛まで。
- macOS Keychain / Linux Secret Service 実装: Unsupported として
  記録、需要があれば別 slice。
- session record の OS keychain backend 化: trait の列挙/expiry
  契約に不適合のため不採用（discovery 参照）。

## Verification Notes

- [ ] `mise run test synvelis-gateway`: FileCliSessionStore の
  restart/expiry/revocation/tamper/secret 非含有 test。
- [ ] `mise run test-cli`: real binary で `session issue` →
  `--session` → verified Human claim、revoke 後 fail-closed、
  非対応 OS の Unsupported、secret 非流出。
- [ ] `mise run check-headless-deps`: keyring 依存が GUI/GPU/
  WebView を引き込まないこと。
