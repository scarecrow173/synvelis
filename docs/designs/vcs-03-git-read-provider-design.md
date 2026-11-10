---
id: "DESIGN-7jUOgXVjk3gpAYkbIzdUfe"
type: "design"
status: "approved"
title: "VCS-03 Git Read Provider design"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source:
    - "SPEC-PvLluOn4j6y1qpdtQ0aw8A"
  verified_by: []
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-03 Git Read Provider design

Spec: `docs/specs/vcs-03-git-read-provider.md`。本書は module 構成・binding・
read 経路・degradation 規則の詳細設計。

## Module 構成

### `synvelis-project`（provider-neutral、Git 非依存）

- `src/vcs/types.rs` — `VcsObjectFormat`/`FixedVcsRevision`/
  `RevisionSelector`/`RevisionRelations`/`HistoryNode`/`HistoryPage`/
  `HistoryQuery`/`WorkingStatus(Entry)`/`StagedSnapshot`/`IndexEntry(Mode)`/
  `VcsCapabilities`/`VcsFeature`/`WorkspaceInfo`/`ProjectScope`/
  `ProjectRelativePath`/`ProjectPath`。
- `src/vcs/mod.rs` — `VcsReadProvider` trait、`VcsError`、`VcsCancel`
  （`Arc<AtomicBool>` 共有 flag、`shared_flag()` で外部 cancel 型へ bridge）。
- `lib.rs` — `pub mod vcs;` 追加のみ。既存 `snapshot::VcsBytesPort` は
  変更しない（VCS-01 境界の再利用）。

### `synvelis-library`（Git adapter、read-only）

- `src/git/vcs.rs` — `GitVcsProvider` + `GitVcsConfig`。
  `VcsReadProvider` と `VcsBytesPort` の両方を実装。
- `src/git/objects.rs` — `classify_kind` に `"not a git repository"` を
  `NotFound` として追加（detect の `NotRepository` 写像に必要）。
  それ以外の runner/transport は ALIB-04 のまま再利用。
- `src/context.rs` — `CancellationToken::from_shared(Arc<AtomicBool>)`
  を追加（`VcsCancel` → runner cancel の bridge。doc(hidden) adapter seam）。
- `src/git/mod.rs` / `src/lib.rs` — export のみ。

## Binding と identity

- `GitVcsProvider::open(&GitVcsConfig)` が `probe_workspace` を実行:
  `git -C <root> rev-parse --path-format=absolute --show-toplevel
  --git-dir --git-common-dir --show-object-format`（+ `HEAD^{commit}`）。
  worktree/git-dir/common-dir を canonicalize し、object format を
  `sha1`/`sha256` に写像。`root` の worktree 相対 path を `project_root`
  として導出（明示 `project_root` 指定時は `ProjectScope` grammar で検証）。
- `repository_id = "git:" + sha256(git_common_dir canonical path)`。
  object store ごとに安定・別 repo では衝突しない。**Git object ID では
  ない**（`sha256:` prefix の project `ContentDigest` とも無関係）。
- `FixedVcsRevision.context` = canonical `git_common` path。
  `check_binding` は `repository_id` + `context` + `object_format` の
  一致を要求し、他 repo/binding の revision は `ForeignBinding`。
- `blob_root` 既定 = `<project_root>/blobs`（root では `blobs`）。
  `sha256:<64hex>` digest → `<blob_root>/sha256/<hex>`、legacy filename-
  safe digest → `<blob_root>/<digest>`。DOS device 名は reject。

## Read 経路（すべて fixed-argv plumbing、shell 不使用）

- **resolve**: `rev-parse --verify --quiet <expr>^{commit}` →
  object-format 長の lowercase hex を検証。`RevisionSelector::Head` は
  `HEAD^{commit}`（unborn → `RevisionGone`）。`Expr` は
  `validate_rev_expr`（長さ・option 形・空白・制御文字・`..`・`@{` を拒否、
  単一 argv 要素として渡す）。
- **tree read**: `ls-tree -z -l <commit> -- :(literal)<path>` を
  `parse_ls_tree_zl`（ALIB-04 既存 parser 再利用）で解析し、name が
  path と一致する単一 entry のみ受理。mode が blob/symlink なら
  `cat-file blob <oid>`（`max_blob_bytes` bound）で bytes 取得。
  tree/gitlink → `Unavailable`（file でない）、absent → `Absent`。
- **index read**: `ls-files --stage -z -- <scopes>`（scope =
  project_root + blob_root が project 配下でなければ両方、root binding
  では全 index）。stage 0–3・mode・oid を parse。`index_generation` は
  同一 listing bytes の `sha256` fingerprint（`lsfiles-sha256:` prefix）。
- **status**: `git status --porcelain=v1 -z --no-renames
  --untracked-files=all -- :(literal)<scope>` を `GIT_OPTIONAL_LOCKS=0`
  で実行（index refresh-write を抑止）→ XY を `WorkingChangeKind` に写像。
- **history**: `rev-list --topo-order --skip=<off> --max-count=<n+1>
  <start> -- <scope>` で page 取得 → 各 oid を `cat-file commit` で
  parse（parents 全保持・author・committer timestamp・message）→
  parent object の存在を `cat-file -e` で確認し `HistoryCompleteness`
  を決定。cursor = `v1:<offset>:<fingerprint16>`（repo+start+scope に
  bind、replay 不一致は `InvalidRequest`）。
- **LFS**: blob が pointer サイズ以下かつ `parse_lfs_pointer` が通り、
  かつ `check-attr [--source <commit>] filter` が `lfs` を返す場合のみ
  pointer とみなし、`<git_common>/lfs/objects/<a>/<b>/<oid>` を
  size 一致で読む。**pointer text は決して content として返さない**。
  check-attr 失敗時は LFS 側へ fail-safe（pointer を content にしない）。
- **digest 検証**: `sha256:` 形式 digest は bytes の SHA-256 が一致した
  場合のみ `Present`。不一致・LFS object 不在・truncated は
  `Absent`/`Unknown` — VCS-01 resolver が `Missing`/`Degraded`/
  `PartialStage` へ写像し、silent `Equal` にはならない。

## Degradation / conflict 規則

- index unmerged（stage 1–3）: `StagedSnapshot.complete=false`、
  `conflicted_paths` 記録。file/blob read は `Unknown`（存在を推測しない）。
- committed tree の gitlink/tree entry: `Unknown`（submodule 内容は
  fetch しない。`submodule_entries` capability は検出・報告のみ）。
- LFS pointer + local object 不在: `Absent`（remote fetch は scope 外）。
- parse 不能 record / truncated output: `Unavailable` または
  `BudgetExceeded`（degraded を clean と誤認しない）。
- `VcsCancel`: `ProviderContext.cancel` に `CancellationToken::
  from_shared` で bridge。各 git 呼出前・page step 間で
  `check_interrupts` → `Cancelled`。runner は in-flight child を kill。

## Read-only 保証

- 使用 git コマンドは `rev-parse`/`ls-tree`/`cat-file`/`ls-files`/
  `check-attr`/`rev-list`/`status` のみ。checkout・add・commit・
  branch/tag 更新・push/pull・update-index・write-tree 等は一切呼ばない。
- `GIT_OPTIONAL_LOCKS=0`・`GIT_TERMINAL_PROMPT=0`・credential helper/
  askpass 無効は runner が全呼出に適用。secret は context 経由のみで
  snapshot/diagnostic に永続化しない。
- 検証: `provider_is_read_only_across_the_whole_surface` が index bytes
  hash・porcelain status・HEAD・staged set の前後一致を oracle として
  固定。`provider_does_not_touch_project_session_or_undo` が live
  `ProjectController` revision/track の不変を固定。

## 依存方向

`synvelis-library → synvelis-project`（既存方向を維持）。
`recording-poc → synvelis-recording → synvelis-project` は不変。
新規 crate 依存なし（`sha2` は既存 workspace dep）。
