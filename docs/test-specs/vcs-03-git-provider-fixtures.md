---
id: "TSPEC-K2QAlGFhpEHJiLP6StV4ta"
type: "test-spec"
status: "approved"
title: "VCS-03 Git read provider fixtures"
created: "2026-11-10"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "SPEC-PvLluOn4j6y1qpdtQ0aw8A"
    - "DESIGN-7jUOgXVjk3gpAYkbIzdUfe"
  verified_by: []
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-03 Git read provider fixtures

Spec `SPEC-PvLluOn4j6y1qpdtQ0aw8A` / Design
`DESIGN-7jUOgXVjk3gpAYkbIzdUfe` を検証する headless fixture test 仕様。
実装は `crates/synvelis-library/tests/vcs03_git_provider.rs`（20 test）、
fixture は `tests/common/vcs_git_fixture.rs`。

Fixture は実 `git` CLI で temp worktree を構築する（remote/network/
git-lfs binary 不要）。LFS object は `<repo>/.git/lfs/objects/<a>/<b>/<oid>`
へ手置きし、provider の local LFS read 経路のみを検証する。fixture 自体は
commit/branch/tag/index を mutate してよい — provider が mutate しない
ことは read-only oracle が証明する。

## Feature: fixed revision・history（VCS-03-FIX）

- **linear history** → `resolve_fixed_revision` が HEAD/branch 式/oid 式を
  同一 commit に固定。`read_revision_file` が pinned tree の bytes を返す。
  `list_history` は newest-first・single-parent chain・author/committer
  metadata 付き。
- **branch/tag force-move** → move 前に固定した `FixedVcsRevision` の
  file read は move 後も同一 bytes。selector は move 後の新先を返す。
- **merge commit** → `revision_relations`/`HistoryNode.parents` が
  **両 parent** を保持（first-parent-only 禁止）。root commit は parents 空。
- **rename/delete** → tree-level delete+add として旧 path `Absent`・
  新 path `Present` が revision ごとに正しく見える。
- **history pagination** → limit/cursor で bounded page・page 間重複なし・
  別 start への cursor replay は `InvalidRequest`。

## Feature: identity・object format（VCS-03-ID）

- **repository identity** → 別 repo は別 `repository_id`。foreign
  `FixedVcsRevision`/locator は `ForeignBinding`/`NotFound`。
  同一 repo の別 `project_root` binding は同じ commit を読む
  （project_root は scope metadata で object identity ではない）。
- **SHA-1/SHA-256** → `detect_workspace`/`resolve_fixed_revision` が
  `object_format` を正しく報告し oid 長が format と一致。SHA-256 repo の
  64-hex commit oid は `ContentDigest`（`sha256:` prefix 必須）として
  parse できず、逆も同様 — **両名前空間を同一視しない**。
- **capabilities** → history/working_status/staged_index/lfs_objects/
  submodule_entries/cancellation/resource_bounds が `Supported`、
  `object_formats` が検出 format のみを含む。

## Feature: degradation・conflict（VCS-03-DEG）— silent Equal 禁止

- **partial stage** → project file のみ staged で asset blob 未 stage の
  `resolve_vcs` は `SnapshotError::PartialStage`（error であり
  degraded snapshot ではない）。blob も stage すれば `Complete`/`Valid`。
- **untracked asset** → committed tree に blob が無い（worktree に
  file は存在）場合 `ContentStatus::Missing` + `Degraded`/`Partial`。
- **missing LFS** → pointer commit で local LFS object 不在は
  `Missing`/`Degraded`。pointer text は決して bytes として返らない。
  LFS object を後から配置すれば同じ commit が `Present` になる。
- **merge conflict** → unmerged index stage を持つ `StagedSnapshot` は
  `complete=false` + `conflicted_paths` 記録。
- **gitlink** → index/tree の submodule entry は `Gitlink` mode で
  報告され、file read は `Unknown`/`Unavailable`（内容 fetch しない）。
- **index generation** → staging 変更で fingerprint が変化、unstage で
  復元。未知 repository_id は `NotFound`。

## Feature: unborn HEAD（VCS-03-UNBORN）— synvelis#335

- **0-commit repo**（`VcsGitFixture::new()` は `git init` + config のみで
  commit 0 件）で `detect_workspace`/`open` が `head_commit: None` を
  報告し `NotRepository` にしない。
- **working_status** が `Ok` + `head_commit: None` を返し、staged file
  が `index_change: Added` で `WorkingStatusEntry` に現れる
  （`Unavailable`/`TransientIo` に誤分類されない — Issue #335 の
  回帰防止）。
- **resolve_fixed_revision** が `RevisionSelector::Head`・unborn
  branch 名・`HEAD` alias・未知 ref の `Expr` で `VcsError::
  RevisionGone` を返す。
- **list_history** が start 省略・明示 `Head` start の両方で
  `VcsError::RevisionGone` を返す。
- **staged/index** が unborn でも正常動作し、`resolve_staged_
  snapshot` が staged entry を含む `Ok` を返す。

## Feature: read-only・cancel（VCS-03-RO）

- **repo oracle** → provider 全 surface（detect/capabilities/status/
  history/resolve/relations/file/index/port reads + resolver の
  staged/committed resolve）実行の前後で `.git/index` bytes hash・
  `status --porcelain`・`HEAD`・`ls-files --stage` が一致。
- **session oracle** → provider 呼出が live `ProjectController` の
  revision・track 集合を変えない。
- **cancel** → cancel 済み `VcsCancel` で working_status/list_history/
  resolve_fixed_revision/resolve_staged_snapshot/read_revision_file が
  `VcsError::Cancelled`。
- **working status** → index 側 `Added`・worktree 側 `Modified`・
  untracked `??` が `WorkingStatusEntry` に正しく分類される。

## NotRun（Issue #97 へ送る行）

実 OS permission matrix・real remote fetch・auth/credential 実経路・
GC/objects 欠損の実 filesystem 条件・GUI/Visual Diff・Apply/Restore
（VCS-05）・RT 性能保証。headless fixture で検証不能な行は本 spec の
対象外として明示する。
