---
id: "PLAN-034X6iKc1PdhVoA25hznHo"
type: "plan"
status: "in-progress"
title: "Project Model Plan — contract mapping + headless verification"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/10"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/project-model-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/project-model-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/project-model.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/project-model-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/project-model.md"
  references: []
  defers: []
  deferred-by: []
---

# Project Model Plan — contract mapping + headless verification

## Goal

Issue #10 の Project Model 設計契約を、既存 `synvelis-project` 実装と承認済み
上流文書に整合する 7 文書チェーン + `evidence/sv10/` 台帳として確定し、
PM-AC-01〜13 を既存 headless fixture・設計レビュー・NotRun/Deferred に
写像する。本 slice は文書・証跡のみで、crate 実装は変更しない。

## Scope

### In Scope

- `docs/specs|designs|plans|tasks|test-specs|impl/ir|impl/exp/project-model*`
  の 7 文書。
- `evidence/sv10/project-model-case-ledger.md`・
  `evidence/sv10/project-model-verification.md`。
- 触れた docs dir の index 再生成（`doc_maintenance.js index apply`）。
- `mise run fmt-check`・doc-status lint・impl audit・review-protocol。

### Out of Scope

- `crates/` 以下のコード・test 変更、`Cargo.toml`/`Cargo.lock`/`mise.toml`/
  CI の変更。
- 実機 GUI/RT/plugin/storage の検証。
- Ruviform 側の変更。

## Upstream Documents

- `docs/specs/project-model.md`（SPEC-034X6inBx3ortD0y20pxqk）
- `docs/designs/project-model-design.md`（DESIGN-034X6iC9O7gEAUDL5Sxqqv）
- 依存 spec: VS-A・TIM-01・ED-A/ED-B・MD-A・CTL-01・VCS-01/02/05・
  RCD-02・ALIB-01/02・Issue #86・SYN-IO-02・PC-C4-05・RC-D-04・TIM-02。

## Design Inputs

- `crates/synvelis-project/src/` の `project.rs`/`id.rs`/`ops.rs`/
  `controller.rs`/`store.rs`/`migration.rs`/`time.rs`/`tempo.rs`/
  `placement.rs`/`comp.rs`/`midi.rs`/`routing.rs`/`recording_data.rs`/
  `range.rs`/`snapshot/`/`restore/`/`diff/`/`vcs/`/`apply/`。
- `crates/synvelis-project/tests/` の 39 fixture file。
- 上流設計: `project-asset-persistence-architecture.md`・
  `configurable-ppqn-musical-time-architecture.md`・`ed-a`/`ed-b`/`mda`/
  `vs-a`/`vcs-*`/`tim-*`/`rcd-*`/`alib-*` design 群。

## File Map

| File | 種別 | 内容 |
| --- | --- | --- |
| `docs/specs/project-model.md` | spec | PM-R-01〜16・PM-AC-01〜13 |
| `docs/designs/project-model-design.md` | design | entity/編集/時間/ID/CAS/永続化/失敗/境界契約 |
| `docs/plans/project-model.md` | plan | 本書 |
| `docs/tasks/project-model-task.md` | task | 作業分解・完了条件 |
| `docs/test-specs/project-model.md` | test-spec | PM-T case 写像 |
| `docs/impl/ir/project-model.md` | impl IR | 実施記録 |
| `docs/impl/exp/project-model.md` | impl EXP | 試行記録 |
| `evidence/sv10/project-model-case-ledger.md` | evidence | PM-AC→PM-T→test→結果 |
| `evidence/sv10/project-model-verification.md` | evidence | 実行検証記録 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/README.md` | generated | index 再生成物（`impl/exp` は `.jsonl` のみ index 対象のため差分なし） |

## Implementation Sequence

1. spec → design → plan → task → test-spec の文書チェーンを確定。
2. `evidence/sv10/` に case-ledger（PM-AC→PM-T→既存 test/NotRun）と
   verification（実行コマンド・結果・baseline finding）を作成。
3. impl-ir・impl-exp を作成。
4. `doc_maintenance.js index apply` で触れた dir の index を再生成。
5. `doc_status.js lint`・`audit_impl_record.js`・`audit_docs.js` で整合確認。
6. review-protocol で spec+design+全体をレビューし blocker を解消。
7. `git fetch origin main && git merge origin/main` → index 再生成 →
   `mise run fmt-check` → commit → push → PR。

## Dependencies and Risks

- 依存: 上流 spec/design 群・`synvelis-project` 既存 test。本 slice は
  それらを変更しない。
- リスク: (a) doc-driven-dev-graph router が pre-existing broken-relation
  で blocked → baseline 記録で対処。(b) relation 先の docs パス誤記 →
  doc-status lint で検出・修正。(c) index 生成が baseline 問題で失敗 →
  範囲内 dir のみ再生成し、失敗 dir は PR に記録。

## Task Breakdown

| # | Task | 対応 doc |
| --- | --- | --- |
| 1 | spec/design/plan/task/test-spec 確定 | `docs/{specs,designs,plans,tasks,test-specs}/project-model*.md` |
| 2 | impl-ir/impl-exp + evidence 台帳 | `docs/impl/{ir,exp}/project-model.md`・`evidence/sv10/` |
| 3 | index 再生成 + doc lint/audit | `docs/*/README.md`（generated） |
| 4 | review-protocol + 検証 + commit/push/PR | — |

## Verification

- `mise run fmt-check`（canonical、docs slice）。
- `node .agents/skills/doc-status/scripts/doc_status.js lint --type {spec,design,plan,task,test-spec,impl} --json`。
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply`。
- `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json`。
- `node .agents/skills/doc-status/scripts/audit_docs.js`（該当あれば）。
- review-protocol（spec+design+chain 全体）。
- PM-T case の oracle は `docs/test-specs/project-model.md`・
  `evidence/sv10/project-model-case-ledger.md` を参照。

## Open Questions / Gaps

- Creative Branch の完全 schema/UI は #277/#278 系の別 slice（本書は
  契約のみ）。
- 実機/実 plugin/実 RT/実 storage の受入は #112/#114/#176/#294/#84/#330
  系の別 Gate で NotRun。
- doc-driven-dev-graph router の blocked は repo baseline（pre-existing
  broken-relation）であり、本 slice の新規文書起因ではない。
