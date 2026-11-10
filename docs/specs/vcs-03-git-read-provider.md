---
id: "SPEC-PvLluOn4j6y1qpdtQ0aw8A"
type: "spec"
status: "approved"
title: "VCS-03 Git Read Provider — fixed Commit/Tree・Index capture・History/meta（synvelis#94）"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source:
    - "ISSUE-94"
    - "ISSUE-92"
    - "ISSUE-93"
    - "ISSUE-88"
    - "ISSUE-7"
  verified_by:
    - "TSPEC-K2QAlGFhpEHJiLP6StV4ta"
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-03 Git Read Provider — fixed Commit/Tree・Index capture・History/meta

## 目的

Issue #94（親設計 Issue #7 VCS-03）。VCS-01（Issue #92）の Snapshot bytes と
VCS-02（Issue #93）の比較元を **Git から取得する最初の VCS Provider** を実装する。
`VcsReadProvider`（provider-neutral 契約）と `VcsBytesPort`（VCS-01 resolver への
byte read 境界）を `synvelis-project` に定義し、local worktree を対象とする
read-only Git adapter `GitVcsProvider` を `synvelis-library` に実装する。

所有境界（Issue #94 本文の合意）: Asset Library の ALIB-04（Issue #88）と
低レベル Git Object/credential/read transport は共用可。**Project History /
Working Status と Asset Catalog は別 Port** であり、本 slice はそれらを
provider-neutral 契約として `synvelis-project::vcs` に置き、Git 固有実装は
`synvelis-library::git` に閉じる。#88 の Asset Provider 本体は再実装しない。

本 slice は headless local fixture で検証可能な範囲のみ。実 OS permission /
real remote / auth / GC / GUI は `NotRun` として Issue #97 へ送る。

## 入力と前提

- provider-neutral 契約は `synvelis-project::vcs`（型 + `VcsReadProvider`
  trait + `VcsError` + `VcsCancel`）。Git 用語を持ち込まない
  （`object_format` が SHA-1/SHA-256 を区別するのみ）。
- `FixedVcsRevision` は `repository_id` + `object_format` + `commit_oid`
  （provider object namespace の hex ID）+ `project_root` + `context`
  （opaque binding）を持ち、mutable ref（branch/tag/HEAD/式）は
  `resolve_fixed_revision` で **1 度だけ** immutable commit に固定される。
  `commit_oid` は project asset の `ContentDigest`（常に SHA-256）とは
  **別名前空間**であり、比較・変換・同一視しない。
- `VcsBytesPort` は VCS-01 `SnapshotResolver::resolve_vcs` が使う既存境界
  （`index_generation` / `read_file` / `read_blob`）。Git adapter がこれを
  実装し、staged/committed locator の pinned 読みを供給する。
- 低レベル transport は既存 `GitRunner`（fixed argv・prompt/credential
  helper 無効・bounded stdout・cancel polling・sanitize 済み diagnostic）
  と `GitRemoteUrl`（local/file/http/https/ssh 検証）を再利用する。
  worktree object read は remote を必要としない。

## 中核契約

### provider-neutral 型（`synvelis-project::vcs`）

- `VcsObjectFormat`: `Sha1`（40 hex）/ `Sha256`（64 hex）。
- `FixedVcsRevision`: 上記 identity を持つ immutable revision。
  `oid_matches_format` で object-format 適合を検査。
- `RevisionSelector`: `Head` / `Expr(String)`。`Expr` は provider が理解する
  revision 式（`main`, `HEAD~2`, tag, full oid 等）で、resolve 時に一度だけ
  commit oid へ固定される。
- `RevisionRelations`: `parents` は **merge の全 parent を保持**
  （first-parent-only は表現不能）。
- `HistoryNode`/`HistoryPage`/`HistoryQuery`: bounded page・opaque cursor
  （発行 query に bind、別 query への replay は `InvalidRequest`）・
  merge-aware・`HistoryCompleteness`（parent object 不在は `Partial`）。
- `WorkingStatus`/`WorkingStatusEntry`: index 側・worktree 側の変更分類、
  untracked、conflict、`index_generation` fingerprint、観測時 HEAD。
- `StagedSnapshot`/`IndexEntry`: index（staging area）の固定 view。
  stage 0–3・mode（regular/executable/symlink/gitlink）・object id。
  `complete=false` は unmerged/undecodable を含み degraded 扱い必須。
- `VcsCapabilities`: `object_formats` + optional feature
  （history/working_status/staged_index/lfs_objects/submodule_entries/
  cancellation/resource_bounds）を `Supported`/`Unsupported{reason}` で宣言。
- `VcsError`: `NotRepository`/`RevisionGone`/`MissingProjectFile`/
  `IndexChanged`/`PartialStage`/`MissingObject`/`MissingLfs`/
  `PermissionDenied`/`AuthExpired`/`Offline`/`RateLimited`/
  `UnsupportedCapability`/`InvalidProjectPath`/`BudgetExceeded`/
  `Cancelled`/`InvalidRequest`/`ForeignBinding`/`Unavailable`。
  missing/unknown/degraded は **明示** であり silent Equal は禁止。

### `VcsReadProvider` trait

- `detect_workspace(ProjectPath) -> WorkspaceInfo`: repository 検出・
  identity・object format・project_root 提案・capabilities。
- `capabilities`, `working_status(scope, cancel)`,
  `list_history(query, cancel)`,
  `resolve_fixed_revision(selector, cancel)`,
  `revision_relations(fixed, cancel)`,
  `read_revision_file(fixed, path, cancel)`,
  `resolve_staged_snapshot(cancel)`。
- 全 method は read-only・non-RT。`VcsCancel` を operation 境界で honor
  し、in-flight git read は provider の resource bound + runner の
  cancel polling で中断される。

## 受入条件（Issue #94 から転記 + fixture 対応）

| 受入行 | fixture 検証 |
| --- | --- |
| Local fixture Git repo で linear/branch/merge(two parents)/rename/delete/force-moved branch/tag、別 Project path/Repo identity の正しい固定 | `vcs03_git_provider.rs` の各 test が実 temp git repo で検証 |
| branch move 後も固定 Commit の比較同一。Index midcapture update/partial stage/missing LFS/asset untracked は明示 Degraded/Conflict、無言 Equal 禁止 | `branch_and_tag_moves_do_not_change_fixed_reads`・`committed_snapshot_is_stable_across_branch_moves`・`staged_snapshot_reads_index_and_detects_partial_stage`・`untracked_asset_is_missing_never_silent_equal`・`missing_lfs_object_is_degraded_never_pointer_bytes`・`index_generation_tracks_staging_changes` |
| checkout/index/Working Tree/file status が実行前後不変、Live Session/Transport/Engine Undo も不変 | `provider_is_read_only_across_the_whole_surface`（index hash・porcelain status・HEAD・staged set の oracle）・`provider_does_not_touch_project_session_or_undo` |
| optional capability/非 RT・secret redaction・cancel、SHA1/SHA256 差、Project Content Digest SHA256 との同一視禁止 | `detect_workspace_reports_identity_and_format`・`cancelled_token_aborts_every_operation`・`sha256_object_format_is_distinct_from_content_digest`・`history_pagination_is_bounded_and_cursor_bound`・`repository_identity_separates_repos_and_project_paths` |

実 OS permission matrix・real remote/auth・GC・GUI は本 slice 対象外で
`NotRun`（Issue #97 へ送る）。

## 非目標

- Git worktree/index/HEAD への書込・checkout・stage・commit・branch/tag
  更新・push/pull・index 更新。provider は観測のみ。
- remote fetch・credential 管理・LFS remote download（local LFS object
  store のみ解決。pointer text は content として決して返さない）。
- #88 Asset Provider（catalog/preview/import）の再実装。
- VCS-04 History/Visual Diff UI、VCS-05 Safe Apply/Restore。
- Ruviform 共通 Network/IO API の製品側 fork。
