---
id: "IMPL-034X3k17SZNVWh4K947RiS"
type: "impl"
status: "completed"
title: "Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価計画 実装記録"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/1"
  changes:
    added:
      - type: "file"
        path: "docs/specs/product-value-core.md"
      - type: "file"
        path: "docs/designs/product-value-core-design.md"
      - type: "file"
        path: "docs/plans/product-value-core-plan.md"
      - type: "file"
        path: "docs/tasks/product-value-core-task.md"
      - type: "file"
        path: "docs/test-specs/product-value-core.md"
      - type: "file"
        path: "docs/impl/ir/product-value-core.md"
      - type: "file"
        path: "docs/impl/exp/product-value-core.md"
      - type: "dir"
        path: "evidence/sv1"
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
      - type: "file"
        path: "docs/impl/exp/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "dir-index"
        path: "docs/specs/README.md"
      - type: "dir-index"
        path: "docs/designs/README.md"
      - type: "dir-index"
        path: "docs/plans/README.md"
      - type: "dir-index"
        path: "docs/tasks/README.md"
      - type: "dir-index"
        path: "docs/test-specs/README.md"
      - type: "dir-index"
        path: "docs/impl/ir/README.md"
      - type: "dir-index"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/product-value-core-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/product-value-core.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価計画 実装記録

## Summary

synvelis#1 の製品価値・中核体験・Gate A/B/C/AI・UX 評価計画を
`docs/` canonical 文書チェーン 7 file と `evidence/sv1/` の根拠
3 file として正本化した。コード・CI・mise 設定の変更はない。

## Context

- Task: `docs/tasks/product-value-core-task.md`（synvelis#1）。
- Plan: `docs/plans/product-value-core-plan.md`。
- Spec: `docs/specs/product-value-core.md`（製品定義の正本）。
- Design: `docs/designs/product-value-core-design.md`（Gate 構造化・
  証拠状態機械・Owner 境界・Decision Log）。
- Test spec: `docs/test-specs/product-value-core.md`（AM-1〜6 +
  EV-1〜3 の Feature/Rule/Example）。

## Implementation

### Changes

- `docs/specs/product-value-core.md`: 三層構造・中核体験
  （Where/What/Why/How）・Gate A/B/C/AI・UX 原則・説明可能性の
  境界・シナリオ A/B/C・対象ユーザー・差別化仮説・UX 比較計画・
  Creative Branch 関係・Acceptance Matrix 6 行・未決定事項を
  正本化。
- `docs/designs/product-value-core-design.md`: Gate 依存関係・
  証拠状態機械（Not started→…→Product accepted）・Owner 境界・
  UX フロー図・状態語彙対応表（製品語彙 ↔ PC-C3/SF-04 実装語彙）・
  代表 Normal/Failure シナリオと Expected Oracle・Decision Log・
  既存 docs 重複/矛盾調査の要約。
- `docs/plans/product-value-core-plan.md`: 文書のみの実施計画・
  file map・検証 matrix。
- `docs/tasks/product-value-core-task.md`: 実施タスクと Done When。
- `docs/test-specs/product-value-core.md`: AM-1〜6・EV-1〜3 の
  Rule と EX-1〜4 の Example。
- `evidence/sv1/acceptance-matrix.md`: 6 行の根拠（Issue 節・
  既存証拠・Owner・NotRun 宣言）と証拠状態の現在値。
- `evidence/sv1/issue-1-scope-map.md`: 既存 docs との重複・矛盾
  調査と解消方針。
- `evidence/sv1/verification.md`: 本変更の検証記録。
- `docs/*/README.md` 7 file: `doc_maintenance index apply` による
  生成 index の再生成。

### Decisions During Implementation

- `route_graph.js` は repo 全体の既存 broken relation で
  fail-closed `blocked` を返すため、宣言済み delegate 順
  （spec→design→plan→test-spec→task→impl）を手動で踏襲した
  （`docs/impl/exp/svt-product-e2e.md` と同じ既知の制約）。
- `docs/impl/exp/product-value-core.md` は JSONL experiment log
  ではなく、先行例（`docs/impl/exp/svt-product-e2e.md`）に倣った
  Markdown の検討記録とした。exp index は `.jsonl` のみを
  追跡する契約のため index への影響なし。
- 製品語彙「Configured/Active/Observed/Unknown」と実装側 PC-C3
  slot（Configured/Prepared/Applied/Reported/Observed +
  Missing/Unsupported/Dropped/Expired）の差異は、design §5 の
  対応表で吸収し既存 docs を書き換えない方針とした。

## Related Experiments

### Adopted

- なし（`docs/impl/exp/product-value-core.md` は JSONL log では
  なく Markdown の検討記録。`related` で参照）。

### Rejected

- なし。

## Validation

- `mise run fmt-check`: docs/監査のみの変更のため cargo fmt の
  対象差分なし。実行結果は `evidence/sv1/verification.md` と
  PR 本文を正本とする。
- `doc_status.js lint`/`audit`、`doc_maintenance.js index plan`:
  結果は `evidence/sv1/verification.md` に記録。
- review-protocol: Reviewer A/B 独立パス → Validator → Judge。
  結果は PR 本文を正本とする。

## Risks

- 本書群は製品定義の合意であり、実 OS/RT/実 GUI/実 Plugin の
  製品受入を意味しない。各 Gate の受入は Owner Issue
  （#114/#109/#103/#8/#23 等）の証拠で別途判定が必要。
- UX 比較の数値閾値・比較対象最終リスト・参加者条件は未決定
  （Baseline 計測後に別途合意）。

## Follow-ups

- AM-6 UX 比較の Baseline 計測計画と閾値合意（製品 Owner）。
- 初期リリースでの B/C/AI 必須化判断（製品 Owner の別途決裁）。
- Creative Branch の Feature Acceptance（#277/#288 が正本）。
- 下流 docs から本書への参照追加は各 Owner の別途対応
  （本タスクの編集範囲外）。
