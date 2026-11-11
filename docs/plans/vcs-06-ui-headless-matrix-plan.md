---
id: "PLAN-034VpCx6TPoO1WOEqV19u4"
type: "plan"
status: "approved"
title: "VCS-06 UI headless matrix — 実装 plan"
created: "2026-11-10"
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
    - "docs/specs/vcs-06-ui-headless-matrix.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-06-ui-headless-matrix-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-06-ui-headless-matrix.md"
    - "docs/designs/vcs-06-ui-headless-matrix-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-06-ui-headless-matrix.md"
  verified-by:
    - "docs/test-specs/vcs-06-ui-headless-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 UI headless matrix — 実装 plan

## Goal

`vcs06_ui_matrix.rs` の headless fixture 4 tests と ledger の
VCS-UI-01..05 分割記録を追加する（src 変更なし）。

## Scope

単一 stream。test 追加 + docs/evidence のみ。

## Upstream Documents

- `docs/specs/vcs-06-ui-headless-matrix.md`
- `docs/specs/vcs-06-fixture-fault-matrix.md`（親 matrix）

## Design Inputs

- `docs/designs/vcs-06-ui-headless-matrix-design.md`

## File Map

- `crates/synvelis-app/tests/vcs06_ui_matrix.rs` — 新規 fixture
  4 tests（FakeProvider + fixture + pin/history/screen-spec 観測）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` — VCS-UI-01..05
  行分割 + fixture 一覧更新
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  vcs-06-ui-headless-matrix*.md` — 本チェーン

## Implementation Sequence

- [x] `vcs06_ui_matrix.rs` 実装 → `cargo test -p synvelis-app
  --test vcs06_ui_matrix`（補助）→ fmt/clippy 修正
- [x] ledger VCS-UI 行分割 + fixture 一覧更新
- [x] doc チェーン作成 → index 再生成
- [x] `git fetch origin main && git merge origin/main` → canonical
  `mise run fmt-check check lint test test-rt test-msrv
  check-app-native lint-app-native`（全 exit 0、impl-ir に記録）

## Dependencies and Risks

- `op_vcs_view` は `blobs: None` で `VcsSession` を構築する既存契約
  — live 側 asset は常に `Unknown` となり `UnresolvedSource` が出る。
- staged fixture は asset 未登録 snapshot を指す（asset 入りだと
  `PartialStage` で pin 自体が失敗）。
- `index_probe_fail` は pin 解決後に立てる（resolve 時の probe を
  壊さないため）。

## Task Breakdown

- `TASK-034VpC1mzY9bzJDEjAIDKv`（本 plan の implemented_by）。

## Verification

- [x] `cargo test -p synvelis-app --test vcs06_ui_matrix` — 4 pass
  （補助証拠）
- [x] `cargo clippy -p synvelis-app --all-targets -- -D warnings` —
  clean（補助証拠）
- [x] `cargo fmt --all -- --check` — clean（補助証拠）
- [x] canonical `mise run` 群（impl-ir に実測を記録）

## Open Questions / Gaps

- なし。全観測点は既存 pub API で到達可能と確認済み。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
  consistent.

## Implementation Handoff

- Recommended mode: inline execution（完了済み）。
- Independent work streams: none.
- User approval required before dispatch: yes（worker dispatch 経由で
  実施済み）。

## Review Handoff

- reviewer focus: fixture が外部観測契約のみを oracle にしているか、
  ledger の Pass*/NotRun 分割が実態と一致するか。
