---
id: "PLAN-034WVJFNJkmTS7M5lq1tjI"
type: "plan"
status: "approved"
title: "SYN-IO-02 HEAD CAS 実 2-process E2E 検証 — implementation plan"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/330"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/syn-io-02-head-cas-e2e.md"
  implemented-by: []
  depends-on:
    - "docs/designs/syn-io-02-head-cas-e2e-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "PLAN-034T9pLmNvRwXk2jHs7fQ"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/syn-io-02-head-cas-e2e-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/syn-io-02-head-cas-e2e.md"
  verified-by:
    - "docs/test-specs/syn-io-02-head-cas-e2e.md"
  references:
    - "crates/synvelis-project/tests/head_cas.rs"
    - "crates/synvelis-project/src/store.rs"
  defers: []
  deferred-by: []
---

# SYN-IO-02 HEAD CAS 実 2-process E2E 検証 — implementation plan

## Goal

`ProjectStore::save_cas` の実 2-process 協調を self re-exec
test で pin し、HEAD CAS の保証境界を実測付きで確定記録する。

## Scope

単一 implementation stream — test 追加と docs 更新のみで、
`src/` 変更・新規 dependency を伴わない。

## Upstream Documents

- `docs/specs/syn-io-02-head-cas-e2e.md`

## Design Inputs

- `docs/designs/syn-io-02-head-cas-e2e-design.md`
- `docs/designs/syn-io-02-head-cas-design.md`（保証境界の正本）

## File Map

- `crates/synvelis-project/tests/head_cas.rs` — 実 2-process
  協調テスト `two_process_save_cas_single_winner_typed_conflict`
  と worker/marker protocol を追加。
- `docs/designs/syn-io-02-head-cas-design.md` — Verification
  Notes の実 2-process 項目を実測結果へ更新。
- `docs/test-specs/syn-io-02-head-cas.md` — Non-goals の
  実 2-process 記述を本 slice のカバーへ整合。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  syn-io-02-head-cas-e2e*.md` — 本 chain 6 file。
- `docs/*/README.md` — doc-maintenance `index apply` で再生成。

## Implementation Sequence

- [x] `head_cas.rs` に env guard worker 分岐・marker 出力・
  go-file barrier・timeout 付き wait を持つ 2-process テストを
  追加し、`cargo test -p synvelis-project --test head_cas` で
  単独実行して pass を確認する（補助証拠）。
- [x] 親 design Verification Notes・親 test-spec Non-goals を
  実測整合する。
- [x] doc chain 6 file を完成させ、`doc_maintenance.js index
  apply` で index を再生成する。
- [x] review-protocol で chain + diff をレビューし、blocker
  が残っていないことを確認する。
- [x] `git fetch origin main && git merge origin/main` の後、
  canonical 検証（`mise run fmt-check`/`check`/`lint`/`test`
  synvelis-project）を実行し impl-ir に記録する。

## Dependencies and Risks

- 依存: `save_cas`/`HeadCasError`/`HEAD.lock`（PR #342
  `d653062` で main 統合済み — 本 branch の base に含まれる）。
- `current_exe()` self re-exec は libtest `--exact` filter に
  依存 — `import.rs` 先例あり。
- go-file barrier は best-effort の同時試行 — 結果は
  `HEAD.lock` 直列化により timing 非依存。
- macOS/Linux 実走は CI または NotRun — portable code のため
  CI lane で自動実行される見込み。

## Task Breakdown

- Slice A: `head_cas.rs` への 2-process テスト追加。
- Slice B: 親 chain docs の残件記述整合 + 本 chain 完成。
- Slice C: index 再生成・レビュー・canonical 検証・IR 記録。

## Verification

- [x] `cargo test -p synvelis-project --test head_cas`
  （補助証拠 — canonical は mise）。9 tests pass。
- [x] `mise run fmt-check` — exit 0。
- [x] `mise run check synvelis-project` — exit 0。
- [x] `mise run lint synvelis-project` — exit 0。
- [x] `mise run test synvelis-project` — exit 0（新規
  2-process テストを含む全 suite green）。
- [x] macOS/Linux = CI 実走または NotRun として impl-ir/PR
  に記録（NotRun — Windows 実測 + portable code）。

## Open Questions / Gaps

- なし（受入条件確定済みの是正タスク）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
  consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker dispatch 内で
  逐次実行）。
- Independent work streams: none（単一 stream）。
- User approval required before dispatch: yes — 本環境では
  review-protocol のレビュー通過を承認として扱う。
- If approved: task-by-task に実行し、各 Slice で検証する。

## Review Handoff

- reviewer focus: marker protocol の parse 堅牢性、timeout
  設計、round 間の expected 引き継ぎ、敗者 snapshot 非
  publish の assert、docs の実測/未検証の区別。
