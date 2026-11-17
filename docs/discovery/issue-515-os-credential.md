---
id: "DISC-034YXHqBzrCYhc5YBzqLj3"
type: "discovery"
status: "resolved"
title: "issue-515-os-credential — verified-Human OS-credential session 供給の探索結果"
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
  implemented-by: []
  depends-on:
    - "docs/impl/ir/autx-02-cli-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/automation/design-trust-boundary.md"
    - "docs/impl/ir/autx-02-cli-adapter.md"
    - "docs/test-specs/autx-02-cli-adapter.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-515-os-credential.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-515-os-credential — verified-Human OS-credential session 供給の探索結果

## Summary

synvelis#515 の正方向 session 供給（durable `CliSessionStore` + OS
credential 検証経路）を、既存契約だけで実装できるかを探索した。
durable store は `FilePermitStore`（`mcp_permit.rs`）と同型の
digest-only JSON file store で `CliSessionStore` trait に沿って実装
可能と確認。OS credential 経路は workspace の `unsafe_code =
"forbid"` lint により windows-sys 直叩き不可のため、safe wrapper
crate `keyring`（Windows: Credential Manager）を synvelis-cli の
`cfg(windows)` target 依存として採用する方針で確定。session 発行/
revoke は operator provisioning verb、claim 解決は `--session <id>`
で OS vault から secret を in-process に読み `Interactive` claim を
構成する。

## Exploration Goal

AUTX-02（#261）で deferred とされた「verified-Human の OS credential
session 供給」の正方向を実装する。焦点は (a) `CliSessionStore` の
durable 実装形態、(b) `unsafe_code=forbid` 下での OS credential
（Windows Credential Manager）アクセス手段、(c) session 発行/
revoke provisioning と `--session` claim 解決の CLI 表面、(d) 負側
契約（Agent 固定・fail-closed・secret 非流出）の非回帰維持。

## Key Issues and Assumptions

- **durable store**: `FilePermitStore`（`crates/synvelis-gateway/
  src/mcp_permit.rs` L314-）が digest-only JSON + tmp/fsync/rename
  原子書込の先例。`CliSessionRecord` は `secret_hash` private・
  `deny_unknown_fields` で同じ耐性を持つ → `FileCliSessionStore`
  を `cli.rs` に同型実装 → confirmed。
- **unsafe 禁止**: workspace `[lints.rust] unsafe_code = "forbid"`
  （root `Cargo.toml` L28）で windows-sys/DPAPI の unsafe FFI は
  crate 内に書けない → safe wrapper crate が必須 → confirmed。
- **keyring 採用**: `keyring` v4（`v1::Entry` API,
  `pub use v1::*`）は Windows で `windows-native-keyring-store`
  （deps: keyring-core + windows-sys 0.61.2 既存 + zeroize +
  byteorder + regex — GUI/GPU/WebView なし）を経由し Credential
  Manager generic credential を safe API で操作できる。
  `[target.'cfg(windows)'.dependencies]` に限定すれば Linux/macOS
  の dep 木は無増加 → confirmed（採用理由は PR 記録）。
- **発行経路の信頼モデル**: `session issue` は operator
  provisioning（`mcp-poc permit` と同じ「wire capability でない
  offline 発行」モデル）。OS credential store が secret を OS user
  account に束縛し、対話存在証明（GUI prompt/Windows Hello）は
  #114 系 gate へ deferred と正直に記録 → confirmed（design で
  記録）。
- **claim 解決**: `--session <id>` で OS vault の secret を
  in-process に読み `CliSessionClaim::Interactive` を構成 —
  secret が argv/env/log に出ない。`--session-credential` は
  out-of-band 経路として残す → confirmed。
- **store path**: session は project ではなく user 単位 →
  `--session-store <path>` global flag + platform user-state 既定
  （Windows `%APPDATA%\synvelis\cli-sessions.json` 等、env は
  場所解決のみに使用・権限証拠にしない）→ confirmed。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| `FileCliSessionStore`（file, digest-only） | `FilePermitStore` 先例と同型・OS 非依存・process restart 跨ぎ契約をそのまま pin 可能・新規依存なし | file は同一 user の他 process から読める（digest のみなので実害なし） | yes |
| OS keychain を session record 本体の backend にする | secret を vault に置ける | `CliSessionStore` trait は record 返却・列挙・expiry 判定を要求し、keychain は列挙/digest 管理に不向き。secret のみ vault・record は file の分離が自然 | no |
| `keyring` crate（採用） | safe API・Windows Credential Manager・`cfg(windows)` 限定で dep 木を抑制 | 新規依存（理由は PR 記録）・非 Windows は Unsupported | yes |
| `windows-sys`/DPAPI 直叩き | 依存最小 | `unsafe_code=forbid` で crate 内実装不可。safe wrapper 自前新設は過剰 | no |
| `--session-credential` のみ（OS vault なし） | 変更最小 | secret が argv に露出・OS credential 検証経路の AC を満たさない | no |

## Tentative Conclusions and Hypotheses

- confirmed: `CliSessionStore`/`LocalCliAdapter`/`CliSessionClaim`
  の契約は変更不要 — durable store は trait 実装の追加、OS 経路は
  synvelis-cli 側の provisioning/claim 解決で完結する。
- confirmed: 負側契約（`attest`/`caller_delegated`/`caller_verified`/
  `record_kind_ok`/`verify_in`）は無変更で非回帰 — 変更は store
  追加・module doc 更新・CLI 表面のみ。
- hypothesis: 実 Windows Credential Manager の e2e は本機
  （Windows）で `session issue` → `--session` → Human
  `CallerIdentity` 到達まで実測可能。macOS/Linux は Unsupported
  を正直に記録（NotRun）。
- hypothesis: `session issue` を任意 local process が実行可能な
  ことは `mcp-poc permit` と同じ operator provisioning の信頼
  モデルであり、claim 契約（証拠なき Human mint 禁止）は維持
  される — 同一 OS user context 内の行為は trust boundary 外と
  して設計文書に明記する。

## Open Questions

- なし（Coordinator Spec で対象ファイル・AC・依存方針は確定済み。
  session verb 形状・store 既定 path・exit code 写像は design
  doc で確定する）。

## Promotion Candidates

- [x] spec-doc needed: `docs/specs/issue-515-os-credential.md`
- [ ] adr-doc needed: なし（採用経路は既存契約内の実装選択であり
  不可逆な architecture decision ではない — file store +
  Credential Manager は `FilePermitStore`/`mcp-poc permit` の
  先例の踏襲）

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/515
- 契約正本: `docs/automation/design-trust-boundary.md` §2/§3
- seam: `crates/synvelis-gateway/src/cli.rs`（`CliSessionStore`/
  `MemoryCliSessionStore`/`LocalCliAdapter`）、`headless.rs`
  （deferred 記録）、`crates/synvelis-cli/src/host.rs` L346 配線
- 先例: `crates/synvelis-gateway/src/mcp_permit.rs`
  （`FilePermitStore` digest-only + 原子書込）、
  `crates/mcp-poc/src/main.rs`（`permit` provisioning verb）
- 依存調査: `keyring` 4.2.0 / `windows-native-keyring-store` 1.1.0
  （crates.io/docs.rs — deps に GUI/GPU/WebView なし、
  windows-sys 0.61.2 は既存 graph 内）
- 前提 slice 証拠: `evidence/sv261/`（AUTX-02 ledger）

## Evidence

- 2026-11-17: `git fetch origin` → `origin/main` = `acc0198c`
  （worktree HEAD と一致）。前提契約を `cli.rs` L126-269、
  `headless.rs` L33-37、`host.rs` L344-348、`mcp_permit.rs`
  L307-425、`tests/cli_adapter.rs` L407-479/553-576 で確認。
- 2026-11-17: root `Cargo.toml` L28 `unsafe_code = "forbid"` を
  確認 — OS API 直叩き不可、safe wrapper 必須。
- 2026-11-17: `cargo info windows-native-keyring-store` —
  deps = byteorder/keyring-core/windows-sys 0.61.2/zeroize/
  regex(optional)。`keyring` 4.2.0 は `pub use v1::*` で
  `keyring::Entry` API（`new/set_password/get_password/
  delete_credential`）を提供、MSRV 1.88 ≤ workspace 1.89。
- 2026-11-17: `tools/check-headless-deps.mjs` — 禁止 list は
  GUI/WebView/GPU framework のみ。keyring 系は非該当。
- 2026-11-17: `crates/synvelis-cli/tests/cli_e2e.rs` L999-1024 —
  `interactive_claim_fails_closed` が実 binary で bogus
  credential → exit 3（Denied-class）を既に pin。
- 2026-11-17: `mise.toml` — `check-cli`/`lint-cli`/`test-cli`/
  `build-cli`/`check-headless-deps`/`fmt-check` が正規検証入口。
  `test synvelis-gateway`/`test synvelis-cli` は package 引数必須。
