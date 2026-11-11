---
id: "SPEC-VCS04BkndWiringSpec001"
type: "spec"
status: "approved"
title: "VCS-04 production set_vcs_backend 配線 slice（synvelis#95）"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "ISSUE-95"
    - "ISSUE-94"
    - "ISSUE-92"
    - "ISSUE-7"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/vcs-04-backend-wiring-design.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/specs/vcs-03-git-read-provider.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/95"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-backend-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 production set_vcs_backend 配線 slice

## 目的

Issue #95（親設計 Issue #7 VCS-04）。merge 済み headless read-only
slice（PR #252）が残した follow-up のうち **production
`set_vcs_backend` 配線** を実装する。headless slice では
`SynvelisApp.vcs_backend: Option<Arc<dyn VcsHistoryBackend>>` と
`set_vcs_backend` 注入点だけが存在し test からしか呼ばれなかった。
本 slice で `project.new`/`project.open` が project root 単位で実
`GitVcsProvider` を detect → `VcsHistoryAdapter` 経由で bind /
re-bind し、`syn.vcs.*` op が実 git repo に対して動作する。

## 中核契約

### project root 単位の bind / re-bind

- `project_new_inner`/`project_open_inner`（`project_reload` は
  open 経由）の成功時に、bundle path を含む git workspace を
  `GitVcsProvider::detect_workspace` で検出し、
  `GitVcsProvider::open` → `VcsHistoryAdapter::new(provider, &info,
  project_file)` で `vcs_backend` を **毎回再検出して bind** する。
- workspace/store/routing_engine/monitoring は new/open で reset
  されるが `vcs_backend` は reset されない。したがって bind は
  **常に再検出**とし、別 project root の backend を保持しない
  （`bind_vcs_backend` は冒頭で `vcs_backend = None` にクリアして
  から検出する）。
- 非 repository（`VcsError::NotRepository`）・検出失敗・provider
  open/bind 失敗では `vcs_backend = None` を維持し、`syn.vcs.*` は
  `NoBackend` を正直に返す。**fake backend を bind しない**。
- bind は infallible-by-contract: 失敗は `tracing::warn!` に記録
  するのみで project new/open を失敗させない（VCS 不在は degraded
  mode であり project error ではない）。

### VCS-facing project file 規約

- `DirectoryBundleStore` の layout は `<bundle>/snapshots/r<N>.json`
  + `<bundle>/HEAD` であり、snapshot 名は revision 連動で不変 path
  を提供しない。そこで bundle 直下に **`main.json`** を VCS-facing
  project file として materialize する: store の HEAD が指す
  snapshot の canonical bytes をそのまま `<bundle>/main.json` に
  publish する（`refresh_project_file`、tmp + rename）。
- `project.new`/`open`/`save` の成功ごとに refresh し、committed /
  staged endpoint が解決する `<project_root>/main.json` が saved
  HEAD を lag しないことを保証する（stale file は Saved endpoint
  との fake equality を生むため）。
- adapter の `project_path` は **repository-relative file path**:
  bundle が worktree root にあるとき `main.json`、`proj/` 等の
  subdirectory にあるとき `<project_root>/main.json`（
  `WorkspaceInfo.project_root` 連結、repo root では `""`）。
  history/status の `ProjectScope` も同じ `project_root` に bind
  され、project bundle 配下のみが対象になる。

## 既知制限（正直な記録）

- **unborn HEAD（commit 0 件の repo）で `syn.vcs.visual
  {action:"refresh"}` は `service_error` を返す。** 原因は
  `synvelis-library` `GitVcsProvider::head_oid`/`resolve_oid` が
  `--quiet` 付き `git rev-parse --verify` を使い、unborn HEAD の
  exit 1 + 空 stderr が `classify_kind` で `TransientIo` に誤分類
  される既存欠陥（意図された `Ok(None)`/`RevisionGone` 経路に到達
  しない）。staged/status 系 read は正常動作し、view 側は
  `last_error` → `Failed` status で誠実に表示する。再現: `git
  init` した repo 内に bundle を `project.new` し commit 前に
  `syn.vcs.visual refresh` を呼ぶ。本 slice の範囲外（library 側の
  別欠陥）として記録し、別 Issue で修正する。

## 受入条件

- [x] `project.new`/`project.open` が bundle を含む git workspace
  を検出し、`VcsHistoryAdapter` 経由の実 backend を bind する
- [x] 別 project root への new/open で backend が re-bind され、
  `repository_id` が project root ごとに変わる
- [x] 非 repository bundle で `vcs_backend = None`・`syn.vcs.*` が
  `NoBackend`/`no-backend` を返し、直前の repo backend が残らない
- [x] committed/staged endpoint が実 git の `main.json` 内容で
  resolve され、実 commit を跨いだ A/B diff が `different` を返す
- [x] `set_vcs_backend` 注入点は test 用に維持される

## 非目標

- VCS write 系（checkout/stage/index/worktree 変更）は実装しない
  — read-only provider 契約のまま。
- native mount/dock 配線（`screens.rs`/`synvelis-native.rs` の
  `VcsHistory` view 実機 mount）は本 slice 対象外の後続 slice。
- 実 OS GUI/IME/2Window= 実機 gate（VCS-UI-05 → synvelis#114）は
  `NotRun` のまま。
- Overlay / Evolution Map / Rendered Audio Diff 等の view mode
  拡張は spec 非目標のまま後続 slice。
- unborn HEAD の provider 分類バグ修正（`synvelis-library` 側、
  別 Issue）。
