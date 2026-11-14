---
id: "TASK-034X9CAFse5Zv3M2AASrrA"
type: "task"
status: "in-progress"
title: "Asset Library 設計契約タスク (synvelis#9)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/9"
  changes:
    added:
      - type: "file"
        path: "docs/specs/asset-library.md"
      - type: "file"
        path: "docs/designs/asset-library-design.md"
      - type: "file"
        path: "docs/plans/asset-library.md"
      - type: "file"
        path: "docs/tasks/asset-library-task.md"
      - type: "file"
        path: "docs/test-specs/asset-library.md"
      - type: "file"
        path: "docs/impl/ir/asset-library.md"
      - type: "file"
        path: "docs/impl/exp/asset-library.md"
      - type: "file"
        path: "evidence/sv9/asset-library-case-ledger.md"
      - type: "file"
        path: "evidence/sv9/asset-library-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/plans/asset-library.md"
  implemented-by: []
  depends-on:
    - "docs/plans/asset-library.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/alib-01-task1-contract-types.md"
    - "docs/tasks/alib-07-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/asset-library.md"
  derived-by:
    - "docs/impl/ir/asset-library.md"
  verifies: []
  verified-by:
    - "docs/test-specs/asset-library.md"
  references:
    - "evidence/sv9/asset-library-case-ledger.md"
    - "evidence/sv9/asset-library-verification.md"
  defers: []
  deferred-by: []
---

# Asset Library 設計契約タスク (synvelis#9)

## Work

Issue #9 の Asset Library 要件を canonical 文書鎖 7 文書と
evidence 台帳として確立する docs-only slice。spec → design
→ plan → test-spec → task → impl-ir/exp を作成し、既存
ALIB-01〜07・#89・#91・#85〜#90・#97・#114・#331・Ruviform
共通契約との境界を relations と本文で固定する。

## Classification

- 種別: docs-only（文書鎖 + evidence）。crate・CI・Cargo・
  Ruviform 共通契約は変更しない。
- 検証レベル: OS-L1 相当の docs 検証（`mise run fmt-check`・
  doc-status/doc-maintenance/impl-doc audit）。実装の
  OS-L1/GUI-L2/EXT-L3 は #85〜#91 の別 gate。

## Upstream

- `docs/plans/asset-library.md`（approved）— 本タスクが実装する
  計画。
- `docs/specs/asset-library.md`・`docs/designs/asset-library-design.md`
  — 製品契約・設計正本。
- `docs/test-specs/asset-library.md` — 検証保証。
- `https://github.com/scarecrow173/synvelis/issues/9` — 正本。

## Dependencies

- `docs/plans/asset-library.md` — 実施計画。
- `docs/test-specs/asset-library.md` — verified-by。
- 既存 ALIB 文書鎖（alib-01〜07）— 参照・非重複の前提。

## Affected Files

- `docs/specs/asset-library.md`（新規）
- `docs/designs/asset-library-design.md`（新規）
- `docs/plans/asset-library.md`（新規）
- `docs/tasks/asset-library-task.md`（本書・新規）
- `docs/test-specs/asset-library.md`（新規）
- `docs/impl/ir/asset-library.md`（新規）
- `docs/impl/exp/asset-library.md`（新規）
- `evidence/sv9/asset-library-case-ledger.md`（新規）
- `evidence/sv9/asset-library-verification.md`（新規）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md`
  （`doc-maintenance index apply` で再生成）

## Done When

- 7 文書が canonical front matter・relations・status を持ち、
  spec→design→plan→task→test-spec→impl の鎖が双方向に link
  されている。
- spec の AC-1〜16 が LIB-* case class・実装 Owner・evidence
  台帳へ写像されている。
- `evidence/sv9/` に case ledger と verification があり、
  実装 case は `NotRun`・docs 検証 case は実行結果を持つ。
- 触れた dir の index が `doc-maintenance` で再生成され、
  audit/lint が新規 error なし。
- `mise run fmt-check` が exit 0。
- review-protocol の self-review で blocker が残っていない。

## Verification

- [ ] `node .agents/skills/doc-status/scripts/doc_status.js lint --type spec --json` exit 0
- [ ] `node .agents/skills/doc-status/scripts/doc_status.js lint --type design --json` exit 0
- [ ] `node .agents/skills/doc-status/scripts/doc_status.js lint --type plan --json` exit 0
- [ ] `node .agents/skills/doc-status/scripts/doc_status.js lint --type task --json` exit 0
- [ ] `node .agents/skills/doc-status/scripts/doc_status.js lint --type test-spec --json` exit 0
- [ ] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply` で触れた dir が in-sync
- [ ] `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` error なし
- [ ] `node .agents/skills/impl-doc/scripts/audit_experiment_log.js --json` error なし
- [ ] `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir --apply` / `--kind exp --apply` in-sync
- [ ] `mise run fmt-check` exit 0
- [ ] Manual: spec AC-1〜16 が ledger の LIB-* case・Owner・status に写像（supplement）

## Out of Scope

- Provider/cache/importer/Project Controller/UI/test fixture
  の実装（#85〜#91 Owner）。
- 実機/実 network/実 OS/実 GUI の受入実行（Deferred gate）。
- Ruviform Rooted File/IO/Security 共通契約の変更。
- Cargo.toml/Cargo.lock/mise.toml/CI/本番 crate。
