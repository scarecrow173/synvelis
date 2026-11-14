---
id: "TASK-6gOTxmdtactknQD5p2jwe8"
type: "task"
status: "done"
title: "Context Inspector 設計定義タスク — spec/design/test-spec 文書チェーン + UX Fixture + 被覆台帳（synvelis#5）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/5"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/context-inspector-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/context-inspector-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/context-inspector.md"
    - "docs/designs/context-inspector-design.md"
    - "docs/test-specs/context-inspector.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/context-inspector.md"
  references: []
  defers: []
  deferred-by: []
---

# Context Inspector 設計定義タスク

| # | タスク | 完了条件 |
| --- | --- | --- |
| 1 | `docs/specs/context-inspector.md` 作成 | CI-01〜13 が Issue #5 §9 と 1:1 対応し、Non-goals が #112/#108/#107/#114 の Owner 境界を明示する |
| 2 | `docs/designs/context-inspector-design.md` 作成 | Owner 境界表・既存実装対応表・`ContextInspectionTarget` union・Follow/Explore/Pinned/Unresolved 状態機械・情報階層・一括編集・Probe・永続化・Decision Log D1〜D19・Scenario/Oracle を含む |
| 3 | `docs/plans/context-inspector-plan.md` 作成 | スコープ・ファイルマップ・リスク・検証マトリクス・ロールバックを含み、製品実装を対象外と明記する |
| 4 | `docs/tasks/context-inspector-task.md` 作成 | 本 slice の作業分解が完了条件付きで記録される |
| 5 | `docs/test-specs/context-inspector.md` 作成 | CI-01〜13 に対応する UX Fixture の Rule/Example/Guarantee が定義され、既存被覆と NotRun が区別される |
| 6 | `evidence/sv5/coverage.md` 作成 | 全受入条件の既存 Fixture/実装被覆を `Pass`/`Partial`/`NotRun`/`Deferred`/`Unsupported` で台帳化し、根拠ファイル・テスト名を示す |
| 7 | `docs/impl/ir/context-inspector.md` + `docs/impl/exp/context-inspector.md` 作成 | 実施記録と実験メモが front matter 契約に従う |
| 8 | docs index 再生成 | `doc_maintenance.js index plan` → `index apply` で触れた dir の `README.md` を再生成し、in-sync を確認する |
| 9 | `review-protocol` レビュー | 仕様整合・反例レビューを実施し、Confirmed P0/P1 blocker が残らない |
| 10 | 検証 | `mise run fmt-check` が pass。docs-only のため Rust build/test は対象外 |
