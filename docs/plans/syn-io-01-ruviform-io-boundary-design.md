---
id: "PLAN-034UvHKP6YrkCOD8vDGrJj"
type: "plan"
status: "approved"
title: "SYN-IO-01 Ruviform 共通 I/O 適合境界設計 文書化計画"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/82"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
  implemented-by:
    - "docs/tasks/syn-io-01-task1-boundary-design-docs.md"
    - "docs/tasks/syn-io-01-task2-verification-gate-docs.md"
  depends-on: []
  blocks:
    - "docs/tasks/syn-io-01-task1-boundary-design-docs.md"
    - "docs/tasks/syn-io-01-task2-verification-gate-docs.md"
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
    - "docs/designs/syn-io-01-ruviform-io-boundary-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-io-01-io-boundary-verification.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/288"
  defers: []
  deferred-by: []
---

# SYN-IO-01 Ruviform 共通 I/O 適合境界設計 文書化計画 Implementation Plan

## Goal

Issue #82 の 8 受入条件を満たす設計文書チェーンを `docs/` に
整備し、実装 Issue（#84/#86/RF#288）への引き継ぎ可能な
境界・Owner・失敗写像・検証計画・gate 証拠項目を確定する。

## Scope

### In Scope

- `docs/` 配下の文書チェーン作成・更新:
  - `docs/discovery/syn-io-01-io-boundary-inventory.md`
  - `docs/specs/syn-io-01-ruviform-common-io-boundary.md`
  - `docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md`
  - `docs/designs/syn-io-01-ruviform-io-boundary-design.md`
  - `docs/test-specs/syn-io-01-io-boundary-verification.md`
  - `docs/plans/syn-io-01-ruviform-io-boundary-design.md`（本書）
  - `docs/tasks/syn-io-01-task{1,2}-*.md`
- managed index 更新（`docs/*/README.md`）。
- review-protocol による文書レビューと blocker 解消。

### Out of Scope

- `crates/` 以下のコード変更、manifest/lockfile/CI。
- 実装（#84/#86/RF#288 へ委譲）。
- 実機 FS crash/durability 実測（NotRun）。

## File Map

| File | Action | Purpose |
| --- | --- | --- |
| `docs/discovery/syn-io-01-io-boundary-inventory.md` | Create | 現行実装と Ruviform 公開契約の棚卸し |
| `docs/specs/syn-io-01-ruviform-common-io-boundary.md` | Create | 境界の要求事項・受入条件（AC-1..8） |
| `docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md` | Create | Backup/Import archive 初期採用範囲の決定 |
| `docs/designs/syn-io-01-ruviform-io-boundary-design.md` | Create | 責務分担・Owner・失敗写像・状態遷移・Backup/Import・gate 証拠 |
| `docs/test-specs/syn-io-01-io-boundary-verification.md` | Create | Golden/Negative/Crash test 計画・gate 証拠対応 |
| `docs/plans/syn-io-01-ruviform-io-boundary-design.md` | Create | 本計画 |
| `docs/tasks/syn-io-01-task1-boundary-design-docs.md` | Create | task1: 境界 Owner・失敗写像・Backup/Import 設計文書 |
| `docs/tasks/syn-io-01-task2-verification-gate-docs.md` | Create | task2: 検証計画・gate 証拠・文書チェーン完結 |
| `docs/{discovery,specs,adr,designs,plans,tasks,test-specs}/README.md` | Update | managed index |

## Dependency Graph

```text
discovery ──► spec ──► design ──► test-spec
                │         │
                └──► adr ─┘
                     │
                     ▼
                   plan ──► task1, task2
```

- discovery が spec/adr/design の derives-from。
- spec が design の implements/derives-from。
- adr が design/test-spec の derives-from。
- design が test-spec の verifies/derives-from。
- plan が spec+design を derives-from、test-spec を verified-by。
- task が plan を implements/depends-on、test-spec を verified-by。

## Task Breakdown

### task1: 境界 Owner・失敗写像・Backup/Import 設計文書

- discovery/spec/adr/design の作成と内容確定。
- 責務分担表・Owner 固定表・失敗写像表・状態遷移図・
  Backup/Import 設計を含む。
- verified-by: test-spec。

### task2: 検証計画・Gate 証拠・文書チェーン完結

- test-spec（Golden/Negative/Crash case・gate 証拠対応）の
  作成と内容確定。
- plan/task の作成、managed index 更新、review-protocol
  実施、最終検証（doc lint + fmt-check）。
- verified-by: test-spec。

## Risk Register

| Risk | Mitigation |
| --- | --- |
| 境界設計が Ruviform API の実際の保証範囲を超える | discovery で実コード・設計文書を直接確認済み。design は確認済み契約のみを参照。 |
| Backup/Import format 決定が実装 Issue と矛盾 | ADR で「初期採用範囲」と明示し、最終 format 凍結は実装 Issue へ委譲。 |
| 既存文書チェーン（syn-io-02 等）との relation 不整合 | 新規文書は既存文書へ related/depends-on を設定し、既存文書の pre-existing audit finding は baseline として記録（範囲外）。 |
| 実機 FS crash/durability を成功と誤記 | test-spec・gate 証拠で明示的に NotRun と記録。 |

## Verification

- `doc_status`/`doc_maintenance` lint で新規文書の front matter・
  relation・index 整合を検証。
- `mise run fmt-check`（docs のみの変更だが canonical 検証
  として実行）。
- review-protocol で blocker がないことを確認。
- 実装 test（cargo test 等）は docs のみの変更のため対象外。

## Rollback

- 文書のみの変更のため、`git revert` で当該 commit を
  戻せばよい。コード・生成物への影響なし。
