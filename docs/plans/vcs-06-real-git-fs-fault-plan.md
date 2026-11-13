---
id: "PLAN-034WTxvADcuVkmyiazhAKi"
type: "plan"
status: "approved"
title: "VCS-06 real-git/FS fault rows — implementation plan"
created: "2026-11-13"
updated: "2026-11-13"
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
    - "docs/specs/vcs-06-real-git-fs-fault.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-06-real-git-fs-fault-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "PLAN-4RRd44dWWEduAvHmubiVw"
    - "SPEC-034VNNGsOaycnOBjCAH3m1"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/vcs-06-real-git-fs-fault-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-06-real-git-fs-fault.md"
  references:
    - "evidence/reports/vcs-06-compat-evidence.md"
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
  defers: []
  deferred-by: []
---

# VCS-06 real-git/FS fault rows — implementation plan

## Goal

VCS-06 matrix の未消化行のうちローカル検証可能な分（real-git
4 行・Windows FS fault 5 行）を test として追加し、CI 証拠と
ledger を実測で更新する test-only slice。

## Scope

単一 slice。`src/` 変更・新規 dependency を含まず、既存
fixture/helper と公開 API の外部観測のみを使う。

## Upstream Documents

- `docs/specs/vcs-06-real-git-fs-fault.md`

## Design Inputs

- `docs/designs/vcs-06-real-git-fs-fault-design.md`

## File Map

- `crates/synvelis-library/tests/vcs06_git_matrix.rs` — real-git
  4 test（packed/GC・pruned RevisionGone・file:// remote・
  linked worktree）
- `crates/synvelis-project/tests/vcs06_restore_matrix.rs` —
  Windows FS fault 3 test（locked manifest・locked HEAD・
  readonly manifest）
- `crates/synvelis-project/tests/vcs06_apply_matrix.rs` —
  Windows FS fault 2 test（locked/readonly journal record）
- `evidence/reports/vcs-06-compat-evidence.md` — covering run
  追記・COMPAT verdict 更新
- `evidence/reports/vcs-06-fixture-matrix-ledger.md` — 新規行
  追加・fixture test 数更新
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  vcs-06-real-git-fs-fault*.md` — doc chain 6 file

## Implementation Sequence

- [x] Slice A: `vcs06_git_matrix.rs` に real-git 4 test を追加
  （`VcsGitFixture` + `common::git_fixture` helper のみ）。
- [x] Slice B: `vcs06_restore_matrix.rs` / `vcs06_apply_matrix.rs`
  に `#[cfg(windows)]` FS fault test を追加。
- [x] Slice C: `gh run list/view/api` で covering run を実取得し
  compat evidence を更新、ledger に新規行を記録。
- [x] doc chain 6 file 作成 + index 再生成。
- [x] canonical 検証（fmt-check/check/lint/test × 2 package）。

## Dependencies and Risks

- `VcsGitFixture` は変更不要（remote 操作は `common::
  git_fixture` helper で完結）。fixture/helper の変更が必要に
  なった場合のみ `common/vcs_git_fixture.rs` を編集可。
- `file://` remote は local transport — 認証付き実 remote lane
  を Pass にしない境界を test コメント・ledger・spec で維持。
- Windows share mode の意味論は Win32 固有 — `#[cfg(windows)]`
  で POSIX compile を除外し lane を混ぜない。
- `set_readonly(false)` は clippy `permissions_set_readonly_
  false` 対象 — 捕捉済み `Permissions` の復元で解消（`#[allow]`
  不使用）。
- CI 証拠は `gh` 実測の snapshot — 推測補完しない。

## Task Breakdown

- `docs/tasks/vcs-06-real-git-fs-fault-task.md`（単一 task —
  3 slice + doc chain + 検証を 1 coherent work item として扱う）

## Verification

- [x] `cargo test -p synvelis-library --test vcs06_git_matrix`
  — 7 passed（実 cargo は補助証拠、canonical は mise）
- [x] `cargo test -p synvelis-project --test vcs06_restore_matrix
  --test vcs06_apply_matrix` — 11 + 10 passed
- [x] `mise run fmt-check` — exit 0
- [x] `mise run check synvelis-library` / `synvelis-project` —
  exit 0
- [x] `mise run lint synvelis-library` / `synvelis-project` —
  exit 0（clippy `-D warnings`）
- [x] `mise run test synvelis-library` / `synvelis-project` —
  exit 0（全 suite green）
- [x] `gh run list --workflow ci.yml` / `gh run view` /
  `gh api` の実測転記 — run 43428177865 全 leg success

## Open Questions / Gaps

- 本 branch の新規 test 行に対する ci.yml covering run は
  merge 後の次回 dispatch まで存在しない（`pull_request`
  trigger 無しのため PR head CI は NotRun by design）。
- POSIX 側 FS fault 行・認証付き remote・実 OS crash
  durability・実 GUI/plugin host/RT/consumer は外部 gate として
  NotRun 残存。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
  consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker dispatch済み）
- Independent work streams: none
- User approval required before dispatch: review-protocol で代替
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- reviewer focus: `file://` remote を認証付き remote の Pass に
  していないか、`#[cfg(windows)]` guard と NotRun 分離が
  正直か、COMPAT 記録が `gh` 実測値のみか、ledger verdict が
  実測範囲を超えていないか。
