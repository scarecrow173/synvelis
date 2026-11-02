---
id: "ADR-Gw01GateWayCrateBoundary"
type: "adr"
status: "accepted"
title: "GW-01 — synvelis-gateway crate 配置と契約境界"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/53"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-gateway"
    modified:
      - type: "file"
        path: "Cargo.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/53"
  implemented-by:
    - "docs/designs/gw-01-gateway-contract-design.md"
  depends-on:
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
    - "docs/designs/midi-source-note-editing-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  derived-by: []
---

# ADR: GW-01 — synvelis-gateway crate 配置と契約境界

## Status

Accepted (Issue #53 実装判断; review-protocol blocking 指摘なし)

## Context

Issue #53 は #45 GTC-001/002/007/008 の型付き内部 Gateway を、後続
GW-02〜10 が利用する契約境界として先行実装する。新 crate 名は未固定と
明示されており、配置先と所有範囲の決定が必要。

## Considered Options

- **Option 1: `synvelis-gateway` 新設** — Capability Registry・Typed
  Invocation・Result/Error 契約を独立 crate に集約。依存は
  `synvelis-project` の値型(stable ID / Revision / time)のみ。
- **Option 2: `synvelis-project` に同居** — persisted value の所有 crate
  に Gateway 契約を混在させ、GTC-001 の「Project の正本を所有しない」
  境界を曖昧にする。却下。
- **Option 3: 各 consumer crate に散在** — workspace / player 等へ
  contract 片を分散。型の authoritative な単一実装を失い、Issue の
  「GW-02〜09 から利用できる契約」目的を満たさない。却下。

## Decision

Option 1 を採用し、以下の責務境界を確定する。

- **所有する契約**: `CapabilityDescriptor` / versioned `CapabilityRef`、
  `WireRequest` parse と `OperationRequest` 構築の分離、`Consistency` の
  domain 別型分岐、`GatewayOutcome` / `ErrorEnvelope`、`AuthorizationStage`
  /`AuthorizationDecision`/`PolicyEvaluator` seam、static fixture。
- **所有しない契約**: Project 正本・Revision 変更(GW-06 が
  ProjectController へ接続)、Grant/Policy 評価実装(GW-02)、MCP
  transport(GW-08)、UI(GW-09)。
- **Trusted Caller の構築権限**: `CallerIdentity` は crate 発行の
  `AdapterSeal` を必須とし、wire payload・外部 crate の双方から捏造
  不能(GT-002)。`WireRequest` は caller 系 field を型上持たず、
  authority 系 key の含有を `SelfDeclaredAuthority` で拒否する。
- **既定認可は fail-closed**: `DenyAllPolicy` が既定 evaluator。
  capability の registry 存在は permission を与えず、新 capability は
  既存 permission set を暗黙拡大しない(GT-003)。
- **Mutation の revision 必須化**: `Consistency::ExpectedRevision` は
  `ProjectMutation` 専用 variant。Query の `Latest`/`ReadAt`/
  `FixedSnapshot` と型で分離し、初期 agent mutation の revision 省略・
  latest 推測を型上存在させない。
- **Idempotency は in-memory ledger で契約検証**: 永続 Record・Retry・
  Recovery journal は GW-06(ACG-004)へ defer。

## Consequences

- GW-02〜10 は `synvelis-gateway` の型に依存し、boundary 内側の
  registry/evaluator/handler seam を差し替えて拡張する。
- wire 層の authority field 拒否・Consistency 分岐・typed error
  envelope が GW-08 (MCP adapter) でも再利用される同一契約となる。
- `CallerIdentity` 発行の本番経路(接続認証→seal 発行)は GW-08 で
  追加する。GW-01 時点では `FixtureAdapter` が唯一の発行元であり、
  外部 crate から caller を mint する API は意図的に存在しない。
