---
id: "TASK-S4P772tAsxug74Ku5MA5AI"
type: "task"
status: "in-progress"
title: "SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約 実装タスク"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/plans/sf-04-probe-adapter-plan.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-04-probe-adapter-plan.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/designs/sf-04-probe-adapter-design.md"
    - "docs/plans/sf-04-probe-adapter-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/designs/sf-04-probe-adapter-design.md"
    - "docs/plans/sf-04-probe-adapter-plan.md"
  defers: []
  deferred-by: []
---

# SF-04 probe adapter — Issue #107 SF-PROBE-01..08 受入集約 実装タスク

## タスク一覧

- [x] T1: SF-PROBE-01..08 と既存 slice 実装/テストの被覆監査
  （`crates/synvelis-project/src/probe.rs`・
  `crates/synvelis-player/src/probe*.rs`・`tests/sf04_*`）
- [x] T2: 未被覆 headless 分岐の特定 — bound probe の entity 消失
  （SourceMap 失効 → `Unresolved` unbind）と per-key subscription
  dedup 境界
- [x] T3: `crates/synvelis-player/tests/sf04_probe_acceptance.rs`
  新規（2 テスト、RT-observed binding で検証）
- [x] T4: `evidence/sv107/` に被覆表・検証結果・外部 Gate
  NotRun/Unsupported を記録
- [x] T5: docs index 再生成（doc-maintenance）＋ fmt/check/lint/
  test（synvelis-project・synvelis-player）・test-rt・
  check-ruviform-rev の検証
- [ ] T6: Conventional Commits で論理分割コミット → push →
  日本語 PR 作成 → worker_done

## 完了条件

- spec の Acceptance Criteria 全項目を満たす。
- `mise run fmt-check`/`check`/`lint`/`test`（両 crate）が green。
- 外部 Gate は NotRun/Unsupported を evidence と PR に明記。
