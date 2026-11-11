---
id: "TSPEC-VCS04BkndWiringTspec01"
type: "test-spec"
status: "approved"
title: "VCS-04 backend wiring fixtures"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "SPEC-VCS04BkndWiringSpec001"
    - "DESIGN-VCS04BkndWiringDsn0001"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/vcs-04-backend-wiring.md"
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
  verifies:
    - "docs/specs/vcs-04-backend-wiring.md"
    - "docs/designs/vcs-04-backend-wiring-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 backend wiring fixtures

Spec `SPEC-VCS04BkndWiringSpec001` / Design
`DESIGN-VCS04BkndWiringDsn0001` を検証する fixture test 仕様。
実装は `crates/synvelis-app/tests/vcs_backend_wiring.rs`（2 test）。
headless slice の fake provider とは異なり、**real git CLI** で
構築した worktree に対して production `bind_vcs_backend` 経路を
通す（`synvelis-library/tests/common/{git_fixture,vcs_git_fixture}`
と同じ fixture 規約: git CLI 必須・不在時は honest skip・
`core.hooksPath`/`filter.lfs.*`/`commit.gpgsign` を neutralize・
Windows `\\?\` verbatim path は `plain_path` で除去）。

## Feature: production bind（VCS-04W-BIND）

- **Rule VCS-04W-01** `project.new` が git worktree 内の bundle で
  `vcs_backend = Some` を bind し、`<bundle>/main.json` が saved
  HEAD の `SnapshotEnvelope` bytes で materialize される。
  Example: `project_new_binds_real_backend_and_serves_committed_diff`
  の `vcs_backend().is_some()` + `main.json` 存在 assert。
- **Rule VCS-04W-02** bind された backend は実 repository を指し、
  `syn.vcs.view` が `backend_attached: true`・`status: "ready"`・
  `repository_id` が `git:` prefix を返す。
  Example: 同 test の refresh 後 view assert。
- **Rule VCS-04W-03** committed endpoint（`{kind:"committed",
  expr:<sha>}`）と staged endpoint（`{kind:"staged"}`）が
  `<project_root>/main.json` の実 git 内容で resolve され、
  commit/index を跨いだ A/B diff が `overall: "different"` と
  domain 付き changes を返す。
  Example: 同 test の c1→HEAD(c2)・HEAD(c2)→staged pin 比較。

## Feature: re-bind / clear（VCS-04W-REBIND）

- **Rule VCS-04W-04** 非 repository bundle の `project.new` は
  `vcs_backend = None` を保ち、`syn.vcs.view` が
  `status: "no-backend"` を返す（fake backend を bind しない）。
  Example: `backend_rebinds_per_root_and_clears_on_non_repo` の
  plain bundle 区間。
- **Rule VCS-04W-05** repo A → repo B への `project.new` で
  backend が re-bind され `repository_id` が変わる（stale backend
  を保持しない）。repo → 非 repo で `None` にクリアされ、再度
  repo bundle の `project.open` で同じ `repository_id` に
  re-bind される。Example: 同 test の A/B/plain/project.open
  シーケンス。
- **Rule VCS-04W-06** fixture は git CLI 不在環境で fail せず
  honest skip する（`Repo::new` が `None` → early return）。

## Feature: 既知制限の非回帰（VCS-04W-KNOWN）

- **Rule VCS-04W-07** unborn HEAD（commit 0 件）の repo では
  `syn.vcs.visual refresh` が `service_error` を返し、view の
  `last_error`/`Failed` status で誠実に表示される — 現行の
  provider 分類バグを「修正済み」と偽装する test を置かない。
  本 slice の fixture は born-HEAD 状態（初回 commit 後）で
  検証し、library 修正は別 Issue の責務とする。
