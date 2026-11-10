---
id: "SPEC-JQYNMchouBVGrijHyiBC2d"
type: "spec"
status: "approved"
title: "SF-XR-01 — CTL-01/CTL-02 cross-gate oracle fixture（headless slice）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
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
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/adr/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/designs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/plans/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/tasks/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/test-specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/impl/ir/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "evidence/reports/sf-06-cross-gate-ledger.md"
  defers: []
  deferred-by: []
---

# SF-XR-01 — CTL-01/CTL-02 cross-gate oracle fixture（headless slice）

## What Should Be Built

Issue #109 の SF-06 cross-gate ledger で `NotRun(#121/#122 依存)`
のまま残っていた **SF-XR-01 行**の headless 検証可能部分を、
project/player の test fixture として実測し、ledger を実測値で
更新する。CTL-01（#121・closed）と CTL-02（#122・PR #243）の
契約は実装済みであり、本 slice は `crates/*/src/` を一切変更
しない fixture-only slice である。

対象契約（Issue #109 SF-XR-01 scope 転記）:

- 1 Track/Bus → 1 Strip の cardinality
- Host `ParameterTargetId` が Send Gain の唯一正本
- Send Gain / PrePost / Mute の一貫性
- Project Serialization / Undo / Graph SourceMap の一致
- 異 Layout ChannelMap は明示 Consent

## Acceptance Criteria

- [ ] `crates/synvelis-player/tests/`（または project tests）に
      cross-gate oracle fixture を追加し、上記契約を persisted
      model → serialization/undo → `lower_semantic` の
      `RoutingSourceMap`/`GraphSpec` まで一気通貫で照合する
- [ ] `CreateSend` の channel-map consent matrix（map 無し・
      未承認 map・承認済み妥当 map・不正 map）と commit 経路の
      atomicity（revision/project bytes/undo stack 不変）を実測
- [ ] `evidence/reports/sf-06-cross-gate-ledger.md` の SF-XR-01 行を
      実測結果で更新し、headless evidence row を追加する
- [ ] fixture で計測できない lane（実 OS GUI・native plugin・
      consumer・実 channel 変換 DSP）は `NotRun` のまま残す

## Non-goals

- `crates/*/src/` の変更・新規契約型（契約不整合を見つけた場合は
  変更せず報告）
- `synvelis-library` / `synvelis-app` / `synvelis-workspace` /
  `synvelis-render` / `synvelis-recording`（別 dispatch 占有）
- ledger の他 caseId 行・T1/T3/T4 lane の計測
