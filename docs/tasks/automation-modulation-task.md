---
id: "TASK-1DDemfS68i5F8Lkla2LRN9"
type: "task"
status: "done"
title: "Automation / Modulation 統合 Parameter Control — 設計文書チェーン作成（synvelis#6）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  changes:
    added:
      - "docs/specs/automation-modulation.md"
      - "docs/designs/automation-modulation-design.md"
      - "docs/plans/automation-modulation.md"
      - "docs/tasks/automation-modulation-task.md"
      - "docs/test-specs/automation-modulation.md"
      - "docs/impl/ir/automation-modulation.md"
      - "docs/impl/exp/automation-modulation.md"
      - "evidence/sv6/coverage.md"
    modified:
      - "docs/specs/README.md"
      - "docs/designs/README.md"
      - "docs/plans/README.md"
      - "docs/tasks/README.md"
      - "docs/test-specs/README.md"
      - "docs/impl/ir/README.md"
      - "docs/impl/exp/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - "docs/specs/README.md"
      - "docs/designs/README.md"
      - "docs/plans/README.md"
      - "docs/tasks/README.md"
      - "docs/test-specs/README.md"
      - "docs/impl/ir/README.md"
      - "docs/impl/exp/README.md"
  implements:
    - "docs/plans/automation-modulation.md"
  implemented-by: []
  depends-on:
    - "docs/plans/automation-modulation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/automation-modulation.md"
    - "docs/designs/automation-modulation-design.md"
    - "docs/test-specs/automation-modulation.md"
    - "docs/impl/ir/automation-modulation.md"
    - "docs/impl/exp/automation-modulation.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/99"
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/synvelis/issues/134"
  defers: []
  deferred-by: []
---

# Task: Automation / Modulation 統合 Parameter Control — 設計文書チェーン

## 目的

synvelis#6 の製品 UX・設計仕様を文書チェーン `spec → design →
plan → task → test-spec → impl/ir + impl/exp` として固定し、
`evidence/sv6/` で受入条件 AM-01〜13 の既存被覆を分類する。
docs-only slice であり、crate/UI/CI の変更は行わない。

## タスク一覧

- [x] T-01 上流契約の調査 — CTL-01/02・PC-A/B・PC-C1〜06・AUT-01〜04・
  SF-04・context-inspector 先例・実テスト面（`pc_*`/`aut_*`/`ctl_*`/
  `rel_c5`/`uic5*`/`sf04*`/`pc_c6_oracle_*`/`trace_c3`）を読み、
  契約と被覆を抽出
- [x] T-02 `docs/specs/automation-modulation.md` 作成 — What/Why/
  AM-01〜13/Non-goals/Sources
- [x] T-03 `docs/designs/automation-modulation-design.md` 作成 —
  Owner 境界表・Scope/Target/Time Mapping・画面操作 Fixture・
  Decision Log(D-01〜20)・Scenario/Oracle・検証戦略
- [x] T-04 `docs/plans/automation-modulation.md` +
  `docs/tasks/automation-modulation-task.md` 作成
- [x] T-05 `docs/test-specs/automation-modulation.md` 作成 —
  Feature/Rule/Example/Guarantee・AM-UX-* Fixture 契約
- [x] T-06 `docs/impl/ir/` + `docs/impl/exp/` 作成 — 実装記録と
  実験記録（router blocked・ID 生成・設計判断）
- [x] T-07 `evidence/sv6/coverage.md` 作成 — AM-01〜13 × 既存被覆を
  `Pass (headless)`/`Partial`/`NotRun`/`Deferred`/`Unsupported` で分類
- [x] T-08 触れた dir の index を `doc-maintenance` で再生成し整合確認
- [x] T-09 `mise run fmt-check` + review-protocol（仕様整合 +
  反例/敵対的）で Confirmed P0/P1 を解消
- [x] T-10 `git fetch origin main && git merge origin/main` → commit
  → push → PR 作成 → `worker_done`

## 完了条件

- 7 文書 + `evidence/sv6/coverage.md` が存在し front matter/関係が valid。
- `evidence/sv6/coverage.md` が全 AM-01〜13 を分類し、設計のみの
  項目を `NotRun`/`Deferred`/`Unsupported` として誤って Pass に
  していない。
- 生成 index が `doc-maintenance` で `in-sync`。
- `mise run fmt-check` が exit 0。
- review-protocol で Confirmed P0/P1 blocker が残っていない。
- 変更が許可範囲（`evidence/sv6/`・7 文書・生成 index）のみ。
