---
id: "IMPL-kBHWiP6FFajB356aw7WOHn"
type: "impl"
status: "completed"
title: "Timeline Editing / Selection / Tool System — 実装記録（docs slice・synvelis#11）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
metadata:
  experiments:
    adopted: []
    rejected: []
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
  implements:
    - "docs/tasks/timeline-editing-task.md"
    - "docs/plans/timeline-editing.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/timeline-editing-task.md"
    - "docs/test-specs/timeline-editing.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/timeline-editing.md"
    - "docs/specs/timeline-editing.md"
    - "docs/designs/timeline-editing-design.md"
    - "docs/test-specs/timeline-editing.md"
    - "evidence/sv11/coverage.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/11"
  derived-by: []
  verifies:
    - "docs/specs/timeline-editing.md"
    - "docs/designs/timeline-editing-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/243"
    - "https://github.com/scarecrow173/synvelis/issues/12"
  deferred-by: []
---

# 実装記録: Timeline Editing / Selection / Tool System（docs slice）

## 実装したもの

**docs のみ**。crate・UI・CI・Cargo/mise 設定の変更はない。
synvelis#11 の共通 UX 仕様と受入 Fixture を文書チェーン 7 文書 +
`evidence/sv11/` として固定した。

| 成果物 | 内容 | 実装状態 |
| --- | --- | --- |
| `docs/specs/timeline-editing.md` | What/Why・TE-01〜14・Non-goals・Sources | 設計文書（`completed`=文書作成完了） |
| `docs/designs/timeline-editing-design.md` | Owner 境界・既存実装対応付け・Selection/Tool/Snap/Gesture/Cross-View 契約・F-UX-01〜08・Decision Log(D-01〜12)・S-N/S-F シナリオ | 設計文書 |
| `docs/plans/timeline-editing.md` | docs-only scope・file map・依存・リスク・検証行列・handoff・rollback | 計画文書 |
| `docs/tasks/timeline-editing-task.md` | T-01〜T-10・完了条件 | タスク文書 |
| `docs/test-specs/timeline-editing.md` | TE-F1〜F8・R1〜R24・E1〜E8・G1〜G8 | テスト仕様文書 |
| `docs/impl/ir/timeline-editing.md` | 本記録 | 記録 |
| `docs/impl/exp/timeline-editing.md` | 実験記録 | 記録 |
| `evidence/sv11/coverage.md` | TE-01〜14 × 既存被覆の受入台帳 | 証拠分類 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | 生成 index | `doc-maintenance` で再生成 |

## 実装しなかったもの（設計のみ・後続 Owner）

- 製品 Timeline 編集 UX のコード実装 — #112/#114・Project/
  Workspace/GUI の各実装 Issue。
- 実 Window/OS GUI・実機・実 plugin の受入 — #112/#114・EXT-L3。
- Ruviform GPUI Driver・共通 Pointer/HitTest/Viewport/Gesture の
  再実装 — Ruviform 責務。
- px Hit Size/Drag Threshold・OS keybind・Snap 数値 — Issue #11
  §9 の PoC 後決定事項。
- Take/Comp Editor の UI 詳細 — #12 §14/§15 の後続設計。

## 証拠と検証

- **設計文書の検証**: `mise run fmt-check`（docs/Markdown の軽量
  チェック。crate 変更なしのため Rust build/test は対象外）。
- **受入被覆**: `evidence/sv11/coverage.md` が TE-01〜14 を既存
  headless テストへ写像し、`Pass (headless)`/`Partial`/`NotRun`/
  `Deferred`/`Unsupported` で分類。設計のみの項目は Pass にしない。
- **index 整合**: `doc-maintenance` の `index plan`/`index apply` で
  触れた dir を `in-sync` にする。
- **review-protocol**: 仕様整合 + 反例/敵対的レビューで
  Confirmed P0/P1 blocker を解消。

## 状態

- 本 docs slice の `status: completed` は **文書チェーン作成の
  完了**を意味し、製品実装の完了ではない。
- 実装・GUI・実機の受入は下流 Issue の Gate であり、本記録は
  `Required`（設計契約）を固定するが `Pass`（製品動作）を
  主張しない。
