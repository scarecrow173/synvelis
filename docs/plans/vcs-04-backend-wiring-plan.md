---
id: "PLAN-VCS04BkndWiringPln0001"
type: "plan"
status: "approved"
title: "VCS-04 production backend wiring — 実装 plan"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "DESIGN-VCS04BkndWiringDsn0001"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/vcs-04-backend-wiring-design.md"
  implemented-by:
    - "docs/tasks/vcs-04-backend-wiring-task.md"
  depends-on:
    - "docs/designs/vcs-04-backend-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-04-backend-wiring.md"
    - "docs/designs/vcs-04-backend-wiring-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-backend-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 production backend wiring — 実装 plan

## File map

| file | 内容 |
| --- | --- |
| `crates/synvelis-app/Cargo.toml` | `synvelis-library` path 依存追加 |
| `Cargo.lock` | `cargo build` で再生成（依存辺 1 本） |
| `crates/synvelis-app/src/app.rs` | `VCS_PROJECT_FILE`・`bind_vcs_backend`・`refresh_project_file`・new/open/save への配線・import 追加 |
| `crates/synvelis-app/tests/vcs_backend_wiring.rs` | real git CLI fixture + 2 acceptance test |
| `docs/specs|designs|plans|tasks|test-specs|impl/ir/vcs-04-backend-wiring*.md` | doc chain 一式 |

`src/ui_ops.rs`/`src/service.rs` は `syn.vcs.*` op が既に
`self.vcs_backend` を consume するため変更不要（編集許可範囲内
だが差分なし）。`synvelis-library`/`synvelis-project`/
`synvelis-workspace`/`src/diff`/`src/apply`/`docs/*/README.md` は
範囲外。

## Dependency graph

単一 commit 単位の薄い slice:

1. `feat(app): VCS-04 production set_vcs_backend wiring` —
   Cargo.toml/lock + app.rs + tests + doc chain。

`bind_vcs_backend` は `project_new_inner`/`project_open_inner` に
後置し、`refresh_project_file` は save 成功直後に置く（store
確定後・field 代入前で local 変数を使う既存構造を維持）。

## Risk register

- **共有ファイル変更**: `synvelis-app/Cargo.toml` は workspace
  内の共有 manifest。依存追加は additive で public API・feature・
  MSRV に影響しない。`synvelis-library` は既に workspace member
  で lock 済み dep 集合を持つため lock 差分は synvelis-app の
  deps 行のみ。
- **`main.json` mirror**: bundle dir に新規 file が増える。git
  repo 内では commit 対象になり得るが、それが意図（VCS-facing
  project file）。非 repo では冗長 mirror に留まる。
- **bind の re-detect コスト**: `project.new`/`open` 毎に git
  subprocess 2 回（rev-parse probe ×2）。non-RT 境界の project
  操作であり許容範囲。
- **unborn HEAD**: `synvelis-library` の既知分類バグにより
  commit 0 件 repo で refresh が `service_error` になる。
  本 slice では誠実な失敗として記録し、fixture は born-HEAD 状態
  で検証する（spec 既知制限）。
- **Windows verbatim path**: `TempTree` canonicalize の
  `\\?\` prefix は git CLI 引数に使えないため fixture は
  `plain_path` で非 verbatim spelling に戻す
  （library fixture と同じ対処）。

## Verification matrix

| 検証 | 手段 |
| --- | --- |
| bind/re-bind/clear | `tests/vcs_backend_wiring.rs::backend_rebinds_per_root_and_clears_on_non_repo`（real git×2 + 非 repo） |
| committed/staged diff | `tests/vcs_backend_wiring.rs::project_new_binds_real_backend_and_serves_committed_diff`（実 commit/index 横断） |
| NoBackend 正直性 | 同上 test の非 repo 区間（`status: "no-backend"`） |
| fmt/lint/test | `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`/`build` |
| native lane | `mise run check-app-native`/`lint-app-native` |
| 実 OS GUI/IME | NotRun（#114 gate） |
