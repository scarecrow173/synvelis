---
id: "IMPL-034XPPHljpLlwZnXXOhLBE"
type: "impl"
status: "completed"
title: "AUTX-08 headless common E2E slice — same-wire 3-surface contract test (synvelis#267)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/267"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx08_e2e.rs"
      - type: "file"
        path: "docs/test-specs/autx-08-common-e2e.md"
      - type: "file"
        path: "docs/impl/ir/autx-08-headless-common-e2e-slice-same-wire-3-surface-contract-test-synvelis-267.md"
      - type: "file"
        path: "evidence/sv267/case-definitions.md"
      - type: "file"
        path: "evidence/sv267/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv267/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/tests/rig/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/test-specs/autx-08-common-e2e.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/autx-07-mount-wiring.md"
    - "docs/impl/ir/autx-02-cli-adapter.md"
    - "docs/impl/ir/autx-03-headless-cli.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-08-common-e2e.md"
    - "evidence/sv267/acceptance-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/test-specs/autx-08-common-e2e.md"
  derived-by: []
  verifies:
    - "docs/test-specs/autx-08-common-e2e.md"
  verified-by: []
  references:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-trust-boundary.md"
    - "docs/automation/tasks.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUTX-08 headless common E2E slice — same-wire 3-surface contract test (synvelis#267)

## Summary

Issue #267 の headless common E2E slice（Leg A 定義 freeze + Leg B
同一 wire 3-surface contract test）を実装した。

Leg A: `docs/test-specs/autx-08-common-e2e.md` — GUI automation
surface・local CLI session host・MCP adapter が同一 wire
semantics（capability/schema/project/args/consistency/target/
idempotency・task/grant 伝播・fail-closed authorization・typed
outcome/error）で同一 `Gateway` pipeline を通る契約を Rule
AUTX-E2E-SAME-01/FAILCLOSED/IDEM/NOAUTHORITY として freeze した。
`relations.verifies` は upstream spec/design/ADR
（`docs/specs/command-gateway.md`、`docs/designs/
command-gateway-design.md`、`docs/automation/
design-capability-action.md`、`docs/automation/
design-trust-boundary.md`）を指し、task へは戻らない。

Leg B: `crates/synvelis-workspace/tests/autx08_e2e.rs` — 4 test。
各 surface を独立 `compose()` stack で包み（GUI =
`AutomationCatalog::lower` + human caller、CLI =
`HeadlessSessionHost::handle_claim` unattended → delegated agent
caller、MCP = `McpHost` client permit + `midi.edit.prepare`）、
同一論理操作（`synvelis.operation.prepare` による
`synvelis.midi.note.add` on `source10`）の wire semantic field
一致・`GatewayOutcome`/`ErrorEnvelope` 一致・fail-closed parity・
idempotent replay parity を実測で固定した。

`tests/rig/mod.rs` の `boot()` から gateway stack 組立を
`compose()`/`GatewayStack` として抽出 — 3 surface が同一 compose
recipe の独立 stack（独自 policy store・`CommitGate`+journal・
durable project store・`ProposalStore`・idempotency ledger）を得る
ため。`boot()` は薄い wrapper として既存挙動を維持する。

## Context

`docs/automation/tasks.md` §AUTX-08（T-08-1〜T-08-3、Issue
#267）。前提 slice: `autx-07-mount-wiring`（catalog seam +
`lower()` の wire request JSON 化）、`autx-02-cli-adapter`
（`LocalCliAdapter`/`CliSessionClaim` attestation）、
`autx-03-headless-cli`（`HeadlessSessionHost` composition）、
GW-08（`McpHost`/`mcp_tools::wire_for`）。境界: 実 OS/GUI/
provider 受入（#114/#267 acceptance lane）・live adapter
（#264/#265）は範囲外 — headless Pass を実機 Pass に昇格しない。

## Implementation

### Changes

- `crates/synvelis-workspace/tests/rig/mod.rs`: `GatewayStack`
  struct + `compose(jobs)` を追加（`boot()` の gateway/store/
  clock/source/proposals/proposal_engine/gate/ports/
  project_store/jobs/provider/dir 組立を移動）。`boot()` は
  `compose()` を呼んで `GatewayUi`/`WorkspaceManager` を被せる
  薄い wrapper に変更 — 既存 test の挙動は不変。
- `crates/synvelis-workspace/tests/autx08_e2e.rs`（新規、4 test）:
  - `autx08_same_wire_lands_same_outcome_across_surfaces` —
    GUI/CLI/MCP で wire semantic field（capability/schema/
    project/args/consistency/target）一致を assert し、
    `GatewayOutcome::Completed` result が 3 surface で完全一致、
    MCP `structuredContent` が gateway result と verbatim 一致
    することを確認。`request_id` は surface-minted、GUI human
    wire は task/grant を持たず CLI agent wire は `t1`/`g1` を
    持つことを対比 assert。
  - `autx08_unavailable_revision_fails_identically_across_surfaces` —
    `expected-revision:99` で 3 surface 同一 `ErrorEnvelope`、
    MCP は `ExternalError` 投影として同一 code/category/retry/
    message が観測されることを確認。
  - `autx08_agent_without_authority_fails_closed_gui_human_
    permits` — task/grant なし agent wire が CLI/MCP で同一
    `Denied`、同一 wire の GUI human は `Completed` —
    human/agent authority 境界の保存を確認。
  - `autx08_idempotent_replay_names_same_operation` —
    `idempotency_key` 付き prepare の replay（fresh
    `request_id` 付き — production retry と同じ形）が GUI/CLI
    で `DuplicateRequest{original}` を返し、独立 stack でも
    指名 operation id が一致することを確認。
- `docs/test-specs/autx-08-common-e2e.md`（新規）: Leg A 契約
  freeze（Purpose/Feature/Rules/Examples/Guarantees/Non-goals/
  Risk）。
- `evidence/sv267/case-definitions.md`、
  `evidence/sv267/acceptance-ledger.md`、
  `evidence/sv267/verification.md`（新規）: case 定義・
  受入対応・実測検証記録。lane を headless/real-os/ext-l3/
  live-adapter に分離し、headless Pass を実機 Pass に昇格しない。

### Decisions During Implementation

- **代表操作に `synvelis.operation.prepare` + `midi.note.add`
  on `source10` を選択**: engine 登録 capability であり、
  fixture descriptor でも MCP tool 内部 capability でもない
  共通語彙。result payload に caller 由来 field を含まず、
  linked-source write（clip12/clip13 への伝播）を伴う実 mutation
  で effect/authorization 経路も実際に通る。`context.query`
  は `data_access` class が human/agent で正当に異なるため
  全 payload 一致の比較対象に不適と判断。
- **surface ごとに独立 `compose()` stack**: 同一 gateway instance
  を逐次共有すると operation/proposal/snapshot の per-instance
  counter がずれ、result 内の ref が比較不能になる。独立 stack
  なら counter が整合し、かつ ORACLE-01（独立 policy/commit
  oracle）の証明にもなる — 共有 mutable authority state がなく
 ても同一 verdict が出ることを示す。
- **GUI leg は `AutomationCatalog::from_json` + `lower()` +
  `Gateway::handle`**: production の `syn.autx.action` →
  `lower_autx_requests` → gateway lane と同じ経路。GUI が
  gateway 型を直接触らない seam 境界を維持したまま wire 等価を
  証明する。catalog document は `gw.registry()` の実登録
  descriptor から合成（grant verdict は display-only のため
  `satisfied` 固定で executable row を得る）。
- **CLI leg は `HeadlessSessionHost::handle_claim`**:
  `LocalCliAdapter::attest` の実 seal/attestation を通し、
  unattended claim が必ず `Agent`/`LocalCli` caller になる
  ことを利用。wire document は task/grant 付きで構築
  （`LocalCli` origin でも wire の task/grant 参照が
  `GrantPolicy` で評価される — authority は wire + caller
  principal の組で決まる）。
- **MCP leg は `McpHost` + client permit `(t1,g1)`**:
  `midi.edit.prepare` tool が内部で `synvelis.operation.prepare`
  wire を構築する実 adapter 経路。permit record の task/grant が
  `wire_for` 経由で wire field に乗ることを利用。wire は
  adapter 内部で生成されるため直接観測できず、等価性は
  `structuredContent` == gateway `result` の verbatim 一致で
  証明する。
- **idempotency は `proposal.apply` ではなく prepare 系で検証**:
  `midi.edit.prepare` は idempotency 引数を公開しない
  （adapter contract の境界）。GUI `Submit` と CLI wire は
  key を運べるため、両者の replay parity を本 slice で固定し、
  MCP 側の key 公開面は GW-08 adapter contract の責務として
  Non-goals に記録した。replay は fresh `request_id` で行い、
  dedup が `request_id` ではなく key+fingerprint に依存する
  ことを実測で固定した（review-protocol 指摘対応）。
- **review-protocol で確定した境界の明記**: CLI leg の wire は
  rig helper 産であり `synvelis-cli` の `build_request` 出力
  ではない（crate 依存境界）。MCP leg の wire field 一致は
  adapter 内部生成のため outcome/error parity で証明する。
  live `step_wire`（#264）と GUI dispatch hop の consumer も
  deferred — いずれも spec Non-goals と evidence ledger に記録。

## Related Experiments

### Adopted

なし — approach が明確だったため experiment log は作成しなかった
（impl-doc skill の granularity rule: known-solution task は ir のみ）。

### Rejected

なし。

## Validation

- `cargo test -p synvelis-workspace --test autx08_e2e`:
  4 pass（`autx08_same_wire_lands_same_outcome_across_surfaces`、
  `autx08_unavailable_revision_fails_identically_across_surfaces`、
  `autx08_agent_without_authority_fails_closed_gui_human_
  permits`、`autx08_idempotent_replay_names_same_operation`）。
- canonical lane（`mise run fmt-check` / `check` / `lint` /
  `test synvelis-workspace` / `check-headless-deps` / `test-rt` /
  `prek` / doc audits）の実測値は
  `evidence/sv267/verification.md` と PR 本文に記録。

## Risks

- headless 等価は実 OS での同値を保証しない。実 window・実
  credential store・実 transport・実 provider でのみ現れる差異は
  本 slice の対象外（`docs/test-specs/autx-08-common-e2e.md`
  Non-goals/Risk、`evidence/sv267/acceptance-ledger.md`
  NotRun/Deferred）。
- `compose()` 抽出は `boot()` の組立を移動したのみでロジックは
  不変だが、`GatewayStack` の field 追加時に `boot()` 側の
  mapping を更新する責務が生じる — rig は test 専用で影響範囲は
  `synvelis-workspace` test binary に限定される。

## Follow-ups

- #267 受入 lane: `evidence/sv267/` の real-os/ext-l3 行を
  Required/Deferred/NotRun で埋める（本 slice は headless のみ）。
- #264 live adapter・#265 workflow coordinator 接続後、catalog
  実供給・macro preset live 投入・live `step_wire` の
  3-surface 等価を再検証。
- `synvelis-cli` の `build_request`/`claim_for` 出力と GUI
  lowering の wire field 一致 — 別 crate での cross-check
  または共有 wire-builder 抽出が必要（`synvelis-workspace`
  は `synvelis-cli` に依存しない）。
- GUI `lower` → `Gateway::handle` の production dispatch hop
  consumer（#264 mount wiring 接続後に seam 経由の実経路で
  再検証）。
