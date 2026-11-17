---
id: "EXP-513a1b2c3d4e5f6g7h8i9j0k8"
type: "experiment"
status: "completed"
title: "issue-513-variant-apply-ops — 代替実装の実験なし記録"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/513"]
  derives-from: ["docs/impl/ir/issue-513-variant-apply-ops.md"]
---

# EXP: issue-513-variant-apply-ops

代替実装の実験なし。採否はすべて spec/ADR レベルで確定
（discovery → spec → adr → design → plan）。comping replay は既存
`write_entity` arm の再利用、variant replay は既存 transplant 機構の
kind 拡張、binding 差替は既存 `sync_processor` 経路の利用、
add_branch は `fork_variant_set` と同型の engine op — いずれも
既存 seam のみで実装し、新規依存・feature flag なし。
