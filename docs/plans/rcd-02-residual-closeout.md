---
id: "PLAN-034XHOtc6Apy81qWgzzTph"
type: "plan"
status: "completed"
title: "RCD-02 residual closeout — Issue #116 AC 照合と証拠固定の実装計画"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/designs/rcd-02-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/test-specs/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/rcd-02-residual-closeout.md"
      - type: "file"
        path: "docs/impl/exp/rcd-02-residual-closeout.md"
      - type: "file"
        path: "evidence/sv116/rcd-02-closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/tasks/rcd-02-task3-stagedfile-publish-migration.md"
      - type: "file"
        path: "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
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
    - "docs/specs/rcd-02-residual-closeout.md"
  implemented-by:
    - "docs/tasks/rcd-02-residual-closeout.md"
  depends-on:
    - "docs/designs/rcd-02-residual-closeout-design.md"
  blocks:
    - "docs/tasks/rcd-02-residual-closeout.md"
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
    - "docs/test-specs/rcd-02-blob-sha256-digest.md"
    - "docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-residual-closeout.md"
    - "docs/designs/rcd-02-residual-closeout-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-02-residual-closeout.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 residual closeout — Issue #116 AC 照合と証拠固定の実装計画

## Goal

Issue #116 の受入条件 4 行を実在 test・既存 ledger・canonical
lane 実測と 1:1 照合し、doc chain・evidence・IR/exp に簿記として
固定する。プロダクションコード・test・fixture の変更は含まない。

## Scope

単一 stream の docs/evidence-only 変更。分割不要。
編集境界は worker spec の占有リストに限定する。

## Upstream Documents

- `docs/specs/rcd-02-residual-closeout.md`（本 plan が実装する
  spec）
- `docs/designs/rcd-02-residual-closeout-design.md`（設計入力）
- Issue: <https://github.com/scarecrow173/synvelis/issues/116>

## Design Inputs

- `docs/designs/rcd-02-residual-closeout-design.md`

## File Map

| File | 変更 |
| --- | --- |
| `docs/specs/rcd-02-residual-closeout.md` | AC 照合 ledger（新規） |
| `docs/designs/rcd-02-residual-closeout-design.md` | 照合・gate 分類の設計（新規） |
| `docs/plans/rcd-02-residual-closeout.md` | 本計画（新規） |
| `docs/tasks/rcd-02-residual-closeout.md` | task checklist（新規） |
| `docs/test-specs/rcd-02-residual-closeout.md` | 照合・lane・gate 棚卸しの検証仕様（新規） |
| `docs/impl/ir/rcd-02-residual-closeout.md` | 照合結果・lane 結果・残存 gate の IR（新規） |
| `docs/impl/exp/rcd-02-residual-closeout.md` | 照合手法の実験記録（新規・任意 additive） |
| `evidence/sv116/rcd-02-closeout-verification.md` | AC 照合表・lane 実測・gate 棚卸し（新規） |
| `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md` | status `in-progress`→`done`（PR #305 merge 済みの実態追従） |
| `evidence/reports/rcd-02-durability-recovery-review-ledger.md` | closeout verification への additive note のみ（任意・既存記述は不変） |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | 管理 index の再生成（doc-maintenance `index apply`） |

## Implementation Sequence

- [x] Issue #116 本文と既存 doc chain・ledger・実在テストを照合し
      AC 対応表を作成
- [x] canonical lane（`mise run test synvelis-recording` /
      `synvelis-player` / `synvelis-project` / `test-rt`）と軽量
      チェック（`fmt-check`・`prek run --all-files`）を実走し結果を
      記録
- [x] spec/design/plan/task/test-spec/IR/exp の新規 doc chain を
      作成
- [x] `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md` の
      status を `done` に修正
- [x] `docs/*/README.md` 管理 index を doc-maintenance で再生成
      （test-specs の managed index row を正本 frontmatter 側
      `approved` に揃える。marker 管理外 tail 領域の stale
      `proposed` 重複行は generator が verbatim 保存するため
      残存 finding として記録）
- [x] `evidence/sv116/rcd-02-closeout-verification.md` を作成
- [x] doc_status lint/audit・audit_impl_record・doc_maintenance
      index plan を実走し新規 finding なしを確認
- [x] Issue #116 への進捗コメント・PR 作成（Issue close/merge は
      しない）

## Dependencies and Risks

- 並行 Worker が同 repo で稼働中のため、占有リスト外の file に
  触れないことを diff で最終確認する。
- `docs/test-specs/README.md` の dup row が index 再生成で解消
  しない場合は手編集で回避せず、PR 本文「未対応の作業」または
  Coordinator への question で記録する。
- `mise run test-rt` は workspace 全 test を `rt_` filter で実行
  する。toolchain は `mise install` 済みを確認。
- doc 系監査は repo-wide baseline に既存 finding（legacy
  invalid-id 系・一部 reciprocal 警告）を抱える。判定基準は
  「変更ファイルに新規 finding を増やさないこと」。

## Task Breakdown

- `docs/tasks/rcd-02-residual-closeout.md` — 本 plan の task
  checklist（単一 task、docs+evidence のみ）。

## Verification

- `mise run fmt-check` / `prek run --all-files` — 軽量チェック。
- `mise run test synvelis-recording` / `synvelis-player` /
  `synvelis-project` / `test-rt` — canonical、結果は IR
  Validation と evidence に記録。
- `mise run test-recording-real-fs` — opt-in 実 NTFS lane（任意
  再実行。実行する場合は環境条件・結果を正直に記録）。
- `doc_status.js lint` / `audit`（対象 type 絞り込み）・
  `audit_impl_record.js`・`doc_maintenance.js index plan` —
  変更 doc の整合確認。
- `git diff --stat` で占有リスト外の変更が無いことを確認。

## Open Questions / Gaps

- なし（AC は Issue #116 本文で確定済み。外部 gate は owner
  Issue 記名の NotRun/Deferred として記録）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
      consistent.

## Implementation Handoff

- Recommended mode: inline execution（docs+evidence の小 slice、
  単一 stream）
- Independent work streams: none
- User approval required before dispatch: yes（worker spec の
  review-protocol で代替 — blocker 0 を承認として扱う）
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute
  task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- AC-3（CRC32/SHA-256 分離・Legacy v5/Asset/Project Schema
  migration との相互参照・旧 Snapshot/HEAD 保護）は RCD-02 単体
  ではなく ALIB-02（synvelis#86）・project 側 HEAD CAS
  （synvelis#84）にまたがる横断行。RCD-02 側の責務分のみを
  Pass とし、横断部分は gate 記名の NotRun/Deferred として分離
  した — reviewer はこの分離の妥当性を確認されたい。
