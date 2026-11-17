---
id: "SPEC-513a1b2c3d4e5f6g7h8i9j0k1"
type: "spec"
status: "approved"
title: "issue-513-variant-apply-ops — variant/VCS apply op surface 拡張"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/513"]
  derives-from: ["docs/discovery/issue-513-variant-apply-ops.md"]
  derived-by:
    - "docs/designs/issue-513-variant-apply-ops.md"
    - "docs/plans/issue-513-variant-apply-ops.md"
    - "docs/test-specs/issue-513-variant-apply-ops.md"
---

# Spec: issue-513-variant-apply-ops

## 目的

Creative Branch / VCS apply の deferred 残件（synvelis#96/#279/#281/
#282/#283 由来）を 1 slice で解消する。fail-closed 安全モデル
（typed `Unsupported`・1 CAS/Undo・CAS/fingerprint guard）を維持し、
実装可能な面は typed atomic op として実装、 unsafe な面は
`Unsupported` disposition を ADR/設計に確定する。

## スコープ

- `apply/`: comping 4 kind + `VariantSet`/`VariantBranch` の
  `apply_selected`/`target_fingerprint` 対応。
- `ops.rs`: `ProjectOp::SetProcessorBinding`（candidate scope 内の
  plugin binding 差替）。
- `variant/`: `add_branch`（set 内 branch 追加、N>2 candidate）。
- capability 報告=強制一致の維持、CB-FX-04・`rcd_04_vcs_stable_id`
  不変条件の非回帰。

## スコープ外

- `Removed` cascade apply（`Unsupported` 維持 — ADR で確定）。
- preset 差替（typed seam なし — `Unsupported` 維持）。
- routing/send/sidechain/multi-output/mid-chain end の候補 op
  （`Unsupported` 維持 — 設計で確定）。
- 3-way semantic merge、自動 blend、VCS Write 本体、実 OS/Plugin/
  Render/GUI lane。

## 受入条件

Issue #513 本文の 7 項目 + Coordinator 確定 slice AC 8 項目どおり。
headless fixture lane を Pass 判定とし、実機 lane は NotRun 記録。
