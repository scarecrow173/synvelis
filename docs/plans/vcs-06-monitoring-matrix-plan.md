---
id: "PLAN-FdgAC8xT08aXjvUlT4GwSz"
type: "plan"
status: "approved"
title: "VCS-06 monitoring-domain matrix — 実装 plan"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-06-monitoring-matrix.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-06-monitoring-matrix-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-06-monitoring-matrix.md"
    - "docs/designs/vcs-06-monitoring-matrix-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-06-monitoring-matrix.md"
  verified-by:
    - "docs/test-specs/vcs-06-monitoring-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 monitoring-domain matrix — 実装 plan

## Goal

`vcs06_monitoring_matrix.rs` の headless fixture 4 tests と
ledger の VCS-DIFF-09/10・VCS-APPLY-07・VCS-RESTORE-07 行を
追加する（src 変更なし）。

## Scope

単一 stream。test 追加 + docs/evidence のみ。

## Upstream Documents

- `docs/specs/vcs-06-monitoring-matrix.md`
- `docs/specs/vcs-06-fixture-fault-matrix.md`（親 matrix）
- `docs/specs/rcd-03-vcs-diff-monitor-classification.md`
  （monitor kind 分類規約の oracle）

## Design Inputs

- `docs/designs/vcs-06-monitoring-matrix-design.md`

## File Map

- `crates/synvelis-project/tests/vcs06_monitoring_matrix.rs` —
  新規 fixture 4 tests（diff 2・apply 1・restore 1）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` —
  VCS-DIFF-09/10・VCS-APPLY-07・VCS-RESTORE-07 行追加、
  Environment ブロック更新、fixture 一覧更新
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  vcs-06-monitoring-matrix*.md` — 本チェーン

## Implementation Sequence

- [ ] `vcs06_monitoring_matrix.rs` 実装 →
  `cargo test -p synvelis-project --test vcs06_monitoring_matrix`
  （補助）→ fmt/clippy 修正
- [ ] ledger 行追加 + Environment ブロック更新 + fixture 一覧更新
- [ ] doc チェーン作成 → `doc_maintenance.js apply` で index 再生成
- [ ] `git fetch origin main && git merge origin/main` → canonical
  `mise run fmt-check check lint test test-rt test-msrv`
  （全 exit 0、impl-ir に記録）

## Dependencies and Risks

- monitor-only changeset は asset/processor 無し fixture で
  `Complete` + `Different` を満たす必要がある（そうでなければ
  `resolve_verified` の `NotApplicable` で fingerprint 段に
  到達しない）。
- apply の実測 fail 点は fingerprint CAS 段 — `change` field は
  常に `Modified` を報告する。assert はこの実測値に合わせる。
- `PunchMarks` の `affected_range` は `[in,out)` が有効なときのみ
  `Some` — partial/inverted 宣言は `None`（assert は有効宣言の
  範囲に限定）。

## Task Breakdown

- `TASK-4Sfq6WWBaX1avImCs5OcLh`（本 plan の implemented_by）。

## Verification

- [ ] `cargo test -p synvelis-project --test
  vcs06_monitoring_matrix` — 4 pass（補助証拠）
- [ ] `cargo clippy -p synvelis-project --all-targets --
  -D warnings` — clean（補助証拠）
- [ ] `cargo fmt --all -- --check` — clean（補助証拠）
- [ ] canonical `mise run` 群（impl-ir に実測を記録）

## Open Questions / Gaps

- なし。全観測点は既存 pub API で到達可能と確認済み（
  `supported_kind`・`fingerprint`・`plan_changes`・
  `fork_project`/`SnapshotEnvelope` を code review で実測予測を
  固定）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
  consistent.

## Implementation Handoff

- Recommended mode: inline execution。
- Independent work streams: none。
- User approval required before dispatch: yes（worker dispatch 経由で
  実施済み）。

## Review Handoff

- reviewer focus: fixture が外部観測契約のみを oracle にしている
  か、apply `Unsupported` の assert が実測挙動（fingerprint CAS
  段・`change: Modified` 報告）を偽装していないか、ledger 行が
  実測値のみを記録しているか。
