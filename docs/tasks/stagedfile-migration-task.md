---
id: "TASK-034X5Fyit9UxommAteCHe0"
type: "task"
status: "done"
title: "Issue #84 StagedFile Migration — 証拠統合・受入検証 slice"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/stagedfile-migration.md"
  implemented-by: []
  depends-on:
    - "docs/plans/stagedfile-migration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/syn-io-02-task1-store-stagedfile-migration.md"
    - "docs/tasks/syn-io-02-head-cas-task.md"
    - "docs/tasks/syn-io-02-head-cas-e2e-task.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/stagedfile-migration.md"
  references:
    - "evidence/sv84/case-ledger.md"
  defers:
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/306"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/audio-lens/issues/143"
    - "evidence/sv84/case-ledger.md"
  deferred-by: []
---

# Issue #84 StagedFile Migration — 証拠統合・受入検証 slice

## Work

Issue #84 の残受入を証拠として確定する slice。実装本体は
先行 task（`syn-io-02-task1-store-stagedfile-migration`・
`syn-io-02-head-cas-task`・`syn-io-02-head-cas-e2e-task`）で
main 統合済みのため、本 task は:

1. fixture 網羅性の再監査 — test-spec
   `docs/test-specs/stagedfile-migration.md` の Rule 1–8 が
   `store_publish.rs`/`head_cas.rs`/`controller_store.rs` の
   実 test で pin されていることを確認し、gap があれば
   `crates/synvelis-project/` 内で最小 fixture を追加する。
2. Windows 実機検証 — `mise run fmt-check`/`check
   synvelis-project`/`lint synvelis-project`/`test
   synvelis-project` を実 FS（ローカル NTFS temp dir）で
   実行し、Snapshot/HEAD CAS/Recovery の状態遷移・
   Readback/再 Open・衝突/失敗復旧の結果を記録する。
3. OS-L1 証拠の台帳化 — 既存 ci.yml の対象 SHA/nearest-SHA
   run から Native Build/Link・対象 fixture の leg 別結果を
   `evidence/sv84/case-ledger.md` に Case ID・Expected/
   Observed/NotRun で記録する。
4. 責務境界（Publish ≠ Project Commit）と両 Consumer 独立
   Golden の確認を design/test-spec/IR に記録する。
5. 本 doc chain（spec/design/plan/task/test-spec/ir/exp）を
   完成させ、docs index を `doc_maintenance.js index apply`
   で再生成する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/stagedfile-migration.md`（plan）
- `docs/specs/stagedfile-migration.md`（spec）
- `docs/designs/stagedfile-migration-design.md`（design）
- `docs/test-specs/stagedfile-migration.md`（test-spec）

## Dependencies

- 実装 slice は完了済み（PR #178・#342・#382）。
- CI 証拠は既存 ci.yml run の収集（新規 Workflow 不設置）。
- 統合・受入の関係 Issue（RF #305/#294/#306・SV #28/#294・
  AL #143）は着手を止めない — `defers` で記録。

## Affected Files

- `evidence/sv84/case-ledger.md`（新規 — 受入台帳）
- `crates/synvelis-project/`（監査で gap がある場合のみ
  最小 fixture 追加 — 原則変更なし）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/stagedfile-migration*.md`（本 doc chain）
- `docs/*/README.md`（index 再生成・生成物）

## Done When

- [ ] test-spec Rule 1–8 と fixture の対応が監査され、
  `evidence/sv84/case-ledger.md` に Case ID・Expected/
  Observed/NotRun・OS・SHA・CI Run URL/実機結果が記録
  されている。
- [ ] `mise run fmt-check`/`check synvelis-project`/
  `lint synvelis-project`/`test synvelis-project` が
  Windows 実機で exit 0。
- [ ] Publish ≠ Project Commit の責務境界と両 Consumer
  独立 Golden の確認が文書化されている。
- [ ] docs index が再生成され、review-protocol レビューで
  blocker が残っていない。

## Verification

- [ ] `mise run fmt-check` exits 0
- [ ] `mise run check synvelis-project` exits 0
- [ ] `mise run lint synvelis-project` exits 0
- [ ] `mise run test synvelis-project` exits 0
- [ ] `evidence/sv84/case-ledger.md` exists and records
  Case ID / Expected / Observed / NotRun per OS・SHA・run
