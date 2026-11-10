---
id: "TASK-034V2wJc28eBSJp9D68CuX"
type: "task"
status: "in-progress"
title: "SF-06 headless — sf06_oracle.rs Domain Oracle + fault fixture + ledger"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-06-cross-gate-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-06-cross-gate-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/test-specs/sf-06-cross-gate-oracle.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-06-cross-gate-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-06 headless — sf06_oracle.rs Domain Oracle + fault fixture + ledger

## Work

Issue #109 の headless slice。`crates/synvelis-project/tests/
sf06_oracle.rs` に独立 Domain Oracle（`AudibilityOracle`・
`routing_invariant_violations`）・fixture topology（mix/chain）・
exhaustive mute×solo sweep・fault matrix・v5→v6 migration oracle
の 6 test を実装し、`evidence/reports/sf-06-cross-gate-ledger.md`
に Case ID registry と lane 別結果（headless Pass・実機/
consumer NotRun）を記録する。#106/#107/#108 の実装本体・
Ruviform generic primitive には触れない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sf-06-cross-gate-oracle.md`
- `docs/specs/sf-06-cross-gate-oracle-case-ledger.md`
- `docs/adr/sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md`
- `docs/designs/sf-06-cross-gate-oracle-domain-evidence.md`

## Dependencies

- #104 Routing 契約（merged）— persisted `RoutingModel`/
  `ControlModel`/`ProjectOp`/`RoutingEditIntent` を利用。
- #106/#107/#108 は並行実装中 — 本 task は persisted 層のみを
  対象とし、それらの実装確定を待たない。

## Affected Files

- `crates/synvelis-project/tests/sf06_oracle.rs` — 新規 test。
- `evidence/reports/sf-06-cross-gate-ledger.md` — 新規 ledger。
- `docs/specs|adr|designs|plans|test-specs|tasks|impl/ir/
  sf-06-*.md` — docs チェーン。

## Done When

- [x] `sf06_oracle.rs` の 6 test が全て Pass。
- [x] ledger が Issue #109 schema 全 column と全 Case family
  を持ち、実機/consumer 行が `NotRun`。
- [ ] docs チェーン + 索引再生成済み。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt` が
  exit 0。

## Verification

- [x] `cargo test -p synvelis-project --test sf06_oracle`
  exit 0（6 test 全 Pass）。
- [ ] `mise run fmt-check` exit 0。
- [ ] `mise run check` exit 0。
- [ ] `mise run lint` exit 0。
- [ ] `mise run test` exit 0。
- [ ] `mise run test-rt` exit 0。
- [ ] `node .agents/skills/doc-maintenance/scripts/
  doc_maintenance.js plan --type all` → `apply` で索引再生成。
