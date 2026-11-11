---
id: "DESIGN-VCS04Hist0ryV1ewDsn01"
type: "design"
status: "approved"
title: "VCS-04 History/Visual Diff UI design"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "SPEC-VCS04Hist0ryV1ewUI001"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
  implemented-by:
    - "docs/plans/vcs-04-history-visual-diff-ui.md"
  depends-on:
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-history-diff-fixtures.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 History/Visual Diff UI design

Spec: `docs/specs/vcs-04-history-visual-diff-ui.md`。module 構成・
型・解決 orchestration・stale 規則・view 統合の詳細設計。

## `synvelis-project::vcs::history`（新規 module）

`src/vcs/history.rs`。provider-neutral read adapter。

### `VcsHistoryBackend`（object-safe trait）

UI 層が消費する read surface。全 method は `&self`（compare cache は
workspace 側が持つため `&mut` 不要）。

```rust
pub trait VcsHistoryBackend: Send + Sync {
    fn repository_id(&self) -> &str;
    fn project_path(&self) -> &str;              // repo-relative project file
    fn scope(&self) -> &ProjectScope;            // history/status scope
    fn capabilities(&self) -> VcsCapabilities;
    fn list_history(&self, query: &HistoryQuery, cancel: &VcsCancel)
        -> Result<HistoryPage, VcsError>;
    fn resolve_fixed_revision(&self, selector: &RevisionSelector,
        cancel: &VcsCancel) -> Result<FixedVcsRevision, VcsError>;
    fn revision_relations(&self, fixed: &FixedVcsRevision,
        cancel: &VcsCancel) -> Result<RevisionRelations, VcsError>;
    fn working_status(&self, cancel: &VcsCancel)
        -> Result<WorkingStatus, VcsError>;
    fn index_generation(&self) -> Result<IndexGeneration, VcsPortError>;
    fn resolve_committed(&self, revision: &FixedVcsRevision,
        cancel: &SnapshotCancel) -> Result<ImmutableSnapshot, SnapshotError>;
    fn resolve_staged(&self, cancel: &SnapshotCancel)
        -> Result<ImmutableSnapshot, SnapshotError>;
}
```

### `VcsHistoryAdapter<P>`

`P: VcsReadProvider + VcsBytesPort` を保持し、`WorkspaceInfo` から
`repository_id`/`object_format`/`project_root` を bind、
`project_path`（repo-relative project file）と `scope` を constructor
で受ける。`SnapshotResolver::new()` を内蔵。

- `list_history` — `query.path_scope` 未指定時は bind scope を補う
  passthrough（provider の既定と二重にしない: `query` をそのまま渡し、
  caller が scope を決める。adapter は補完しない — YAGNI 回避のため
  query は verbatim）。
- `resolve_committed` — `revision.repository_id != self.repository_id`
  → `SnapshotError::NotFound`（foreign binding の早期拒否）。
  `SnapshotLocator::VcsCommitted { repository_id, fixed_revision:
  revision.commit_oid, project_path }` → `resolver.resolve_vcs(&provider)`。
- `resolve_staged` — `SnapshotLocator::VcsStaged { repository_id,
  project_path, index_generation: None }` → resolver が観測世代を pin。
- `index_generation` — `provider.index_generation(repository_id)` の
  passthrough（staged pin の stale 判定用）。
- `resolve_fixed_revision`/`revision_relations`/`working_status`/
  `capabilities`/`list_history` — provider passthrough。

`ChangeSetCache` は adapter に置かない: cache は「session が所有する
導出物」であり provider binding の責務ではない（spec 参照）。

## `synvelis-workspace::vcs_ui`（新規 module）

`signal_flow_ui` と同じ「read model + per-view state」分離。

### pin / selection 型

```rust
pub enum EndpointPin {
    Live { project_id: ProjectId, live_revision: ProjectRevision },
    Saved { project_id: ProjectId, head: String },
    Staged { repository_id: String, index_generation: IndexGeneration },
    Committed { revision: FixedVcsRevision },
}

pub struct PinnedEndpoint {
    pub pin: EndpointPin,
    pub identity: SnapshotIdentity,
    pub snapshot: ImmutableSnapshot,
}

pub struct HistorySelection {
    pub before: PinnedEndpoint,
    pub after: PinnedEndpoint,
    pub chosen_parent: Option<FixedVcsRevision>,
    pub view_mode: VcsViewMode,
}

pub enum VcsViewMode { SideBySide, ChangeFocus }
```

`PinnedEndpoint` は pin 種別に依らず `identity`（digest・locator・
schema version）を持つため `ComparisonSpec::new(before.identity,
after.identity)` をそのまま構成できる。

### per-view state と registry

```rust
pub struct VcsHistoryState {
    pub nodes: Vec<HistoryNode>,        // loaded pages, bounded
    pub next_cursor: Option<String>,
    pub selected: Option<String>,       // selected commit_oid
    pub selection: Option<HistorySelection>,
    pub current_diff: Option<Arc<ChangeSet>>,
    pub observed_live_revision: Option<ProjectRevision>,
    pub last_error: Option<String>,     // verbatim failure detail
}

pub struct VcsHistoryViews {
    states: BTreeMap<ViewInstanceId, VcsHistoryState>,
    changesets: ChangeSetCache,         // shared across views
}
```

`nodes` は `MAX_HISTORY_NODES`（例 512）で bounded — 超過時は
最古側を落とさず `next_cursor` を残して追加 load を拒否する
（silent drop 禁止、`last_error` に記録）。

### 解決 orchestration

```rust
pub struct VcsSession<'a> {
    pub backend: Option<&'a dyn VcsHistoryBackend>,
    pub controller: &'a ProjectController,
    pub store: Option<&'a dyn ProjectStore>,
    pub blobs: Option<&'a dyn BlobProbe>,
}

pub enum VcsUiError {
    NoBackend, NoStore,
    Vcs(VcsError), Snapshot(SnapshotError), Diff(DiffError),
}
```

`resolve_endpoint(ctx, pin, cancel) -> Result<PinnedEndpoint,
VcsUiError>`:

- `Live` → `SnapshotResolver::capture_live(controller,
  live_revision, cancel, blobs)`（revision 不一致は
  `StaleLiveRevision` error → stale として surface、pin は書き換え
  ない）。
- `Saved` → `resolve_saved(store, Saved{project_id, head_ref: Some
  (head)})`。`store` 無しは `NoStore`。
- `Staged` → `backend.resolve_staged`。pinned generation との比較は
  呼び出し側が `index_generation()` で先に行う。
- `Committed` → `backend.resolve_committed`。

`compare(ctx, selection, cancel)` — 両 `PinnedEndpoint.snapshot` に
対し `changesets.get_or_compare(before, after,
ComparisonSpec::new(..), cancel)`。`&mut self`（cache 更新）なので
`VcsHistoryViews::compare` は `&mut self`。

### stale / freshness

```rust
pub enum EndpointFreshness { Fresh, Stale { reason: String }, Unknown { reason: String } }
```

`endpoint_freshness(pin, ctx)`:
- Committed → `Fresh`（immutable。read 失敗は resolve 時 error）。
- Live → `controller.live_revision() == pin.live_revision` ? Fresh :
  Stale。
- Saved → `store.head()` が pin.head と一致 ? Fresh : Stale /
  store 無しは Unknown。
- Staged → `backend.index_generation()` が pin.index_generation と
  一致 ? Fresh : Stale / 失敗は Unknown。

`new_revision_available` = `observed_live_revision` が Some かつ
`session.revision()` と異なる。

### read-model

```rust
pub struct VcsHistoryView {
    pub project_revision: ProjectRevision,
    pub status: VcsViewStatus,       // Ready | NoBackend | Failed{detail} | Empty
    pub rows: Vec<VcsHistoryRow>,
    pub next_cursor: Option<String>,
    pub selected: Option<String>,
    pub selection: Option<VcsSelectionView>,
    pub new_revision_available: bool,
}

pub struct VcsSelectionView {
    pub before: EndpointLabel, pub after: EndpointLabel,
    pub chosen_parent: Option<String>,
    pub view_mode: VcsViewMode,
    pub before_freshness: EndpointFreshness,
    pub after_freshness: EndpointFreshness,
    pub diff: Option<VcsDiffView>,   // digest/equality/completeness/summary/issues
}
```

`VcsHistoryRow`/`VcsChangeRow`/`VcsIssueRow`/`EndpointLabel` は全て
display 用の plain data（`commit_oid`・短縮表示・author・message
headline・parents 数・completeness・merge flag、change の
domain/kind/object kind/stable_id/field 数/range 有無、issue の
kind+detail）。`ComparisonEquality::Unknown(issue)`・
`ComparisonCompleteness::Partial`・`HistoryCompleteness::Partial`・
`SnapshotCompleteness::Partial` はそのまま label 化し、Equal/空に
丸めない。

## WorkspaceManager 統合

`WorkspaceManager` に `vcs: VcsHistoryViews` を追加（`states`/
`links` と同じ view-scoped registry の位置付け）。

- `spawn_view`/`spawn_detached_view`/`adopt_view`/`restore` で
  `vcs.ensure(id)`（`states.ensure` と同じタイミング、全 view に
  default state — VcsHistory 以外は未使用のまま）。
- `remove_view` と `on_window_event` の `ReleaseViews` 分岐で
  `vcs.remove(id)`（`states.remove` と同じ cleanup 点）。
- accessor `vcs()`/`vcs_mut()`。
- `capture`/`restore` は触らない（session-only、schema v2 不変）。

これにより Dock move・detach_to_pool・reattach・detach_to_new_window・
window DetachViews/ReleaseViews の全経路で VCS view state の生死が
`ViewState` と同一規則になる — Focus/Selection 保持は構造的保証。

`ViewType` に `VcsHistory` variant を追加（serde `vcs-history`）。
`HeadlessView` stub は kind 不問なので変更不要。

## `synvelis-app` 統合

- `screens.rs`: `VcsScreenState`（`VcsHistoryView` を verbatim に
  保持する display state）と `history_screen_spec(&VcsScreenState)
  -> ViewSpec` — `Panel > [status Label, selection Label, diff
  Label, history Table, changes Table, issues Table]`。全 table は
  `selection: None`（read-only、intent を発行しない）。enum →
  文字列の表示語彙（status/freshness/kind/mode/equality）は
  `pub(crate)` helper として screens.rs に置き、`syn.vcs.view`
  の JSON 投影と共有する（wire/screen の語彙 drift を防ぐ）。
  `dock_view_info` への登録と native mount 配線は後続 slice
  （view-1/view-2 の composition-root 経路と同じ attach 駆動で
  行う — 本 slice では `syn.vcs.visual{action:"open"}` が detached
  `VcsHistory` view を mint する headless 経路を提供する）。
- `SynvelisApp`: `vcs_backend: Option<Arc<dyn VcsHistoryBackend>>` +
  `set_vcs_backend`/`vcs_backend`（composition root の注入点。
  backend 無しでは履歴系 action が `NoBackend` を正直に返し、
  Live endpoint の pin のみ backend 無しで動く）。
- `op_vcs_view`/`op_vcs_visual` は `VcsSession` を都度合成する
  （backend + `session.controller()` + `store.as_ref()` を
  `&dyn ProjectStore` + blobs None）。`with_vcs` の split borrow
  で view state の mutable と session の shared を両立する。
- `ui_ops.rs`: `op_vcs_view`（投影 JSON）/ `op_vcs_visual`
  （`open`/`close`/`refresh`/`load_more`/`select_node`/`pin`/
  `compare`/`view_mode`/`clear`）。`view_id` は登録済み
  `view-{n}` を要求する。`open` は `registry` の最大 id と予約済み
  `view-1`/`view-2` の大きい方 +1 で `adopt_view`（detached pool、
  kind `VcsHistory`）し wire id を返す — composition-root id との
  衝突を構造的に排除する。`close` は kind が `VcsHistory` の view
  のみ `remove_view` し、view-1/view-2 を誤って閉じない。
- `service.rs`: `syn.vcs.view`/`syn.vcs.visual` を OPS + dispatch に
  additive 登録（`syn.signal_flow.view`/`visual` と同じ語彙）。

op の endpoint 指定（wire）:
`{"kind":"live"}` / `{"kind":"saved","head":"<rev>"}`（`head` 省略可）/
`{"kind":"staged"}` /
`{"kind":"committed","expr":"<rev>"}`（`expr` 省略時 HEAD。
`resolve_fixed_revision(Expr)` で 1 度だけ固定）/
`{"kind":"node","index":N}`（load 済み `HistoryNode.revision` を
そのまま pin — merge parent は node の `parents` から
`chosen_parent` に記録）。

## Error / status 写像

- `VcsUiError::NoBackend` → view status `NoBackend`（op は
  `SERVICE_ERROR` ではなく status field で返す — read op は
  「backend 未接続」は正常な degraded 状態）。
- `Snapshot`/`Vcs`/`Diff` error → `VcsHistoryState.last_error` +
  status `Failed{detail}`（detail は sanitize 済み provider text を
  そのまま運ぶ。path/secret を新たに載せない）。
- cancellation → `VcsError::Cancelled`/`SnapshotError::Cancelled`/
  `DiffError::Cancelled` を `Cancelled` status に写像。

## 非目標（design 確定）

- `VcsHistoryState` の `WorkspaceSnapshot` 永続化、native GPUI mount
  配線、実 Git provider の app への既定接続（`set_vcs_backend` の
  呼び出し側は composition root の後続 slice）、Overlay/Evolution/
  Rendered Audio、media/waveform byte 配送。
