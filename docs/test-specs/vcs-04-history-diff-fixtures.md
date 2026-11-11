---
id: "TSPEC-VCS04Hist0ryV1ewTst01"
type: "test-spec"
status: "approved"
title: "VCS-04 History/Visual Diff headless fixtures"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "SPEC-VCS04Hist0ryV1ewUI001"
    - "DESIGN-VCS04Hist0ryV1ewDsn01"
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
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
  derived-by: []
  verifies:
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 History/Visual Diff headless fixtures

Spec `SPEC-VCS04Hist0ryV1ewUI001` / Design
`DESIGN-VCS04Hist0ryV1ewDsn01` を検証する headless fixture test 仕様。
実装は `crates/synvelis-project/src/vcs/history.rs` の src 内 unit
test、`crates/synvelis-workspace/tests/vcs04_history.rs`、
`crates/synvelis-app/tests/vcs04_history.rs`。

Fixture は in-memory fake で構成する（実 Git・実 provider は
`synvelis-library` 側の VCS-03 fixture が担当済み）:

- `FakeVcsProvider` — `VcsReadProvider` + `VcsBytesPort` の in-memory
  実装。history page・revision pin・staged/committed bytes・index
  generation・working status を script 可能。read 系 method のみ
  存在するため read-only は型で保証される。
- `FakeVcsBackend` — workspace/app test 用の
  `VcsHistoryBackend` 実装。snapshot は `SnapshotResolver` +
  `FakeVcs`（`VcsBytesPort`）で実 bytes から解決する。
- `ProjectController` + `CounterIdGenerator` + `InMemoryStore` —
  実 project session/store（edit は fixture setup のみ、
  `ProjectSession::apply` 経由で revision を進める）。

## Feature: adapter（VCS-04-ADP）

- **committed resolve** → `resolve_committed(FixedVcsRevision)` が
  `SnapshotOrigin::VcsCommitted` の `ImmutableSnapshot` を返し、
  identity.locator が pinned revision を保持する。
- **staged resolve** → `resolve_staged` が `VcsStaged` origin・
  観測 `index_generation` pin の snapshot を返す。
- **foreign repository** → 別 `repository_id` の
  `FixedVcsRevision` は `SnapshotError` で拒否される
  （`NotFound`/`InvalidLocator` 系、silent fallback なし）。
- **history passthrough** → `list_history`/`resolve_fixed_revision`/
  `revision_relations`/`working_status` が provider の値を verbatim
  に返す。
- **index generation** → `index_generation()` が port の現在値を返し、
  staged pin の stale 判定入力になる。

## Feature: A/B pin・ChangeSet（VCS-04-AB）

- **任意 2 点 pin** → Committed×Committed・Committed×Live・
  Committed×Staged・Live×Saved の組合せで `HistorySelection` を構成
  し、`compare` が 1 つの `Arc<ChangeSet>` を返す。
- **ChangeSet 共有** → 同一 `(before, after, spec)` の 2 回目の
  compare は cache hit で同一 `Arc`（`Arc::ptr_eq` または同一
  `key.digest()`）を返し、view ごとの独立再計算がない。
- **identity pin** → `PinnedEndpoint.identity`（digest・locator・
  schema version）が `ComparisonSpec` にそのまま使われ、pin 解決後の
  Live 編集・index 変動で snapshot 内容が変わらない。
- **merge parent** → merge commit node の `parents` 全件が保持され、
  `chosen_parent` が selection metadata として残る。
- **tri-state 保持** → `ComparisonEquality::Unknown`/`Different`/
  `Equal`・`ComparisonCompleteness::Partial`・
  `SnapshotCompleteness::Partial` が read-model で潰れない
  （Unknown を 0 changes/Equal と表示しない）。

## Feature: Focus/Selection・stale（VCS-04-DOCK）

- **view 独立性** → 2 つの `VcsHistory` view が別 selection/pin/
  cursor/mode を持ち、互いに干渉しない。
- **dock 生存** → `spawn_view` → `move_view`/`begin_transfer`+
  `commit_transfer`/`detach_to_pool`+`reattach`/`detach_to_new_window`
  の全経路で `VcsHistoryState`（selection・pins・cursor・mode・
  observed revision）が不変に残る。
- **2-window** → `detach_to_new_window` + `Focused` event で
  `focus.active_view` が view を指し、state が保持される。
- **ReleaseViews** → window `Destroyed` + `ReleaseViews` disposition
  で `vcs` state が除去される（`DetachViews` では残る）。
- **remove_view** → view 削除で `vcs` state も除去される。
- **stale 区別** → Live pin は `live_revision` 前進で `Stale`、
  index 前進で `Stale`、Saved HEAD 移動で `Stale`、Committed pin は
  常に `Fresh`。`new_revision_available` は
  `observed_live_revision` と live revision の差で立つ。
- **persist 非対象** → `WorkspaceSnapshot` schema v2 に VCS state を
  追加しない（capture/restore 後は default state）。

## Feature: read-only oracle（VCS-04-RO）

- **project 不変** → pin/compare/select/mode/page/refresh の全 op
  前後で `session.revision()`・undo depth・`engine.graph_builds()`・
  transport が不変。
- **git 不変** → fake provider/backend の呼出が read 系
  （`list_history`/`resolve_*`/`index_generation`/`working_status`/
  `revision_relations`）のみで、write 系 API が存在しない/呼ばれない。
- **cancel** → `VcsCancel`/`SnapshotCancel` cancel で
  `Cancelled` status/error が返り、部分結果を残さない。

## Feature: app surface（VCS-04-APP）

- **op 登録** → `syn.vcs.view`/`syn.vcs.visual` が OPS/dispatch に
  登録され、`view_id` 未指定時は `view-3` を使う。
- **view JSON** → `syn.vcs.view` が status/rows/selection/diff
  summary を JSON で返し、NoBackend 時は `status: "no-backend"` を
  正直に返す（error ではなく degraded status）。
- **visual ops** → `select_node`/`pin`/`set_mode`/`load_more`/
  `clear`/`refresh` が per-view state を更新し、pin+compare で
  `current_diff` が更新される。
- **screen spec** → `history_screen_spec` が status/selection/
  history/diff/issues の ComponentSpec を生成し、`ViewHost::mount`
  が受理する（`selection: None` の read-only table のみ）。
- **dock_view_info** → `view-3` が `ViewType::VcsHistory` で登録され、
  `syn.view.attach`/`syn.window.dock_*` の対象になる。
- **app read-only** → 全 `syn.vcs.*` op 前後で project revision・
  undo・transport・engine builds が不変。
