---
id: "TASK-Nw48LwleZYov2jWLb0O7Vg"
type: "task"
status: "done"
title: "Timeline Editing / Selection / Tool System — 設計文書チェーン作成（synvelis#11）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/11"
  changes:
    added:
      - type: "file"
        path: "docs/specs/timeline-editing.md"
      - type: "file"
        path: "docs/designs/timeline-editing-design.md"
      - type: "file"
        path: "docs/plans/timeline-editing.md"
      - type: "file"
        path: "docs/tasks/timeline-editing-task.md"
      - type: "file"
        path: "docs/test-specs/timeline-editing.md"
      - type: "file"
        path: "docs/impl/ir/timeline-editing.md"
      - type: "file"
        path: "docs/impl/exp/timeline-editing.md"
      - type: "file"
        path: "evidence/sv11/coverage.md"
    modified:
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
  implements:
    - "docs/plans/timeline-editing.md"
  implemented-by: []
  depends-on:
    - "docs/plans/timeline-editing.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/timeline-editing.md"
    - "docs/designs/timeline-editing-design.md"
    - "docs/test-specs/timeline-editing.md"
    - "docs/impl/ir/timeline-editing.md"
    - "docs/impl/exp/timeline-editing.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/11"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers: []
  deferred-by: []
---

# Task: Timeline Editing / Selection / Tool System — 設計文書チェーン

## 目的

synvelis#11 の共通 UX 仕様を文書チェーン `spec → design → plan →
task → test-spec → impl/ir + impl/exp` として固定し、
`evidence/sv11/` で受入条件 TE-01〜14 の既存被覆を分類する。
docs-only slice であり、crate/UI/CI の変更は行わない。

## タスク一覧

- [x] T-01 上流契約の調査 — Issue #11 本文・ED-A/ED-B・WS-A・
  workspace-model・MD-A・MIDI Source・Context Inspector・VCS・
  ALIB import の契約と既存テスト面（`ed_a.rs`/`ed_b.rs`/
  `midi_source.rs`/`md_a.rs`/`ws_a.rs`/`uic5*`/`vcs04*`/`import*`）
  を読み、契約と被覆を抽出
- [x] T-02 `docs/specs/timeline-editing.md` 作成 — What/Why/
  TE-01〜14/Non-goals/Sources
- [x] T-03 `docs/designs/timeline-editing-design.md` 作成 —
  Owner 境界表・既存実装対応付け・Selection/Tool/Snap/Gesture/
  Cross-View 契約・F-UX-01〜08・Decision Log(D-01〜12)・
  Scenario/Oracle・検証戦略
- [x] T-04 `docs/plans/timeline-editing.md` +
  `docs/tasks/timeline-editing-task.md` 作成
- [x] T-05 `docs/test-specs/timeline-editing.md` 作成 —
  Feature/Rule/Example/Guarantee・F-UX/TE-F Fixture 契約
- [x] T-06 `docs/impl/ir/` + `docs/impl/exp/` 作成 — 実装記録と
  実験記録（router blocked・ID 生成・設計判断）
- [x] T-07 `evidence/sv11/coverage.md` 作成 — TE-01〜14 × 既存被覆を
  `Pass (headless)`/`Partial`/`NotRun`/`Deferred`/`Unsupported` で分類
- [x] T-08 触れた dir の index を `doc-maintenance` で再生成し整合確認
- [x] T-09 `mise run fmt-check` + review-protocol（仕様整合 +
  反例/敵対的）で Confirmed P0/P1 を解消
- [x] T-10 `git fetch origin main && git merge origin/main` → commit
  → push → PR 作成 → `worker_done`

## 完了条件

- 7 文書 + `evidence/sv11/coverage.md` が存在し front matter/関係が valid。
- `evidence/sv11/coverage.md` が全 TE-01〜14 を分類し、設計のみの
  項目を `NotRun`/`Deferred`/`Unsupported` として誤って Pass に
  していない。
- 生成 index が `doc-maintenance` で `in-sync`。
- `mise run fmt-check` が exit 0。
- review-protocol で Confirmed P0/P1 blocker が残っていない。
- 変更が許可範囲（`evidence/sv11/`・7 文書・生成 index）のみ。
