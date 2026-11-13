---
id: "TASK-5HWsXY7AMzlpvzSSGuZTx6"
type: "task"
status: "done"
title: "Target Architecture 設計レビュータスク — TA-001〜012 + GUI 方針の docs chain・evidence 台帳"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/22"
  changes:
    added:
      - type: "file"
        path: "docs/tasks/target-architecture-task.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/target-architecture.md"
  implemented-by: []
  depends-on:
    - "docs/plans/target-architecture.md"
    - "docs/designs/target-architecture-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/target-architecture.md"
    - "docs/test-specs/target-architecture.md"
    - "docs/impl/ir/target-architecture.md"
    - "docs/impl/exp/target-architecture.md"
    - "evidence/sv22/case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/target-architecture.md"
  derived-by: []
  verifies:
    - "docs/test-specs/target-architecture.md"
  verified-by: []
  references:
    - "evidence/sv22/case-ledger.md"
    - "evidence/sv22/dependency-dag.md"
  defers: []
  deferred-by: []
---

# Target Architecture 設計レビュータスク

## タスク一覧

- [x] T-01: v0.7.1・関連 spec/design/ADR・Issue 群の収集と Owner 境界の写像
- [x] T-02: Cargo resolve グラフの cycle 検査と辺分類（`dependency-dag.md`）
- [x] T-03: Ruviform resolved rev の型漏出走査（TA-002）
- [x] T-04: TA-003〜TA-012 の契約・fixture 存在 pin と証拠分類
- [x] T-05: GUI 採用方針（#384/#175/旧 ADR）と Native Dock Gate（#388/#389/#176）の照合
- [x] T-06: canonical 検証（fmt-check・check-ruviform-rev・check-headless-deps）
- [x] T-07: docs chain 7 文書 + evidence/sv22 台帳の執筆
- [x] T-08: index 再生成・doc_status lint・fmt-check 再実行
- [x] T-09: review-protocol によるレビュー（blocker 解消）
- [x] T-10: merge origin/main・commit・push・PR 作成

## 完了条件

- [x] TA-001〜TA-012 の全 Case が `evidence/sv22/case-ledger.md`
  に Pass/Fail/NotRun/Unsupported/Deferred で記録されている
- [x] GUI 採用方針 2 条件が C13/C14 として分離記録されている
- [x] docs chain 7 文書の front matter relation が全て実在する
- [x] canonical 検証が実行・記録されている
- [x] review-protocol で blocker が残っていない

## スコープ外（本タスクで行わないこと）

- コード・manifest・lockfile・CI の変更
- Ruviform/AudioLens の実装・受入
- GPUI/実 Plugin OS Gate の完了宣言
- sv#20/#21 docs の編集
