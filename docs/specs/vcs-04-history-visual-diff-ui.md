---
id: "SPEC-VCS04Hist0ryV1ewUI001"
type: "spec"
status: "approved"
title: "VCS-04 History/Visual Diff UI — headless read-only projection slice（synvelis#95）"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "ISSUE-95"
    - "ISSUE-7"
    - "ISSUE-92"
    - "ISSUE-93"
    - "ISSUE-94"
    - "ISSUE-97"
    - "ISSUE-114"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/vcs-03-git-read-provider.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/95"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-history-diff-fixtures.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 History/Visual Diff UI — headless read-only projection slice

## 目的

Issue #95（親設計 Issue #7 VCS-04）。VCS-01（Issue #92）の immutable
Snapshot、VCS-02（Issue #93）の semantic `ChangeSet`、VCS-03
（Issue #94）の read-only VCS provider 契約を消費し、History 参照と
A/B Visual Diff を **driver-neutral な headless UI projection** として
実装する。

本 slice の範囲（task spec 転記）: `synvelis-project` の
Snapshot/ChangeSet を読み取る Adapter と、`synvelis-workspace` /
`synvelis-app` の VCS History/A-B Visual Diff read-only ViewSpec・
Focus/Selection 保持の headless 実装と fixture。実 GPUI Window・
実 Git 操作・製品 E2E は `NotRun`（Issue #114・実 OS gate）として
記録し、headless Pass を実 GUI Pass と偽装しない。

## 入力と前提

- `synvelis_project::vcs::VcsReadProvider` — read-only provider 契約
  （history listing・fixed revision pin・working status・staged
  snapshot）。`VcsBytesPort` は同一 provider が実装する byte 境界。
- `synvelis_project::snapshot::SnapshotResolver` — Live/Saved/
  Recovery/VcsStaged/VcsCommitted の 5 系統を `ImmutableSnapshot`
  （`SnapshotIdentity` + read-only projection + completeness）へ解決。
- `synvelis_project::diff::{compare_snapshots, ChangeSetCache}` —
  2 snapshot を 1 つの deterministic `ChangeSet`（tri-state equality・
  unresolved issues・summary）へ比較。`Arc<ChangeSet>` 共有が前提。
- `synvelis-workspace` — `ViewType`/`ViewRegistry`/`ViewStateRegistry`/
  `FocusState`/`DockWorkspace`/`WindowHandle` の headless 基盤。
  `signal_flow_ui` が「session を読んで DTO を返す read-only 投影 +
  per-view visual state」の先例。
- `synvelis-app` — `screens.rs` の `ComponentSpec`/`ViewSpec` 投影と
  `syn.*` op 面（`syn.signal_flow.*` 先例）。

## 中核契約

### read-only 不変条件

- 本 slice の全経路は **read のみ**。Project Commit/Save/Apply/
  Restore、Git index/worktree/branch/HEAD 変更、transport/recording/
  undo/redo への作用を持たない。検証は oracle（revision/undo 不変・
  provider 呼出が read 系のみ）で行う。
- `synvelis-workspace::ProjectSession`/`ProjectController` が唯一の
  project writer のまま。view は session を read するだけで第 2 の
  権威を作らない。
- `Unknown`/`Partial`/`Stale`/`Unsupported`/`Missing`/`Ambiguous`/
  `Unobserved` は明示状態として表示し、空・無変更・Equal に潰さない
  （VCS-02 の tri-state を verbatim に運ぶ）。

### 層構成

1. `synvelis-project::vcs::history` — `VcsHistoryBackend`（object-safe
   read surface）と `VcsHistoryAdapter<P: VcsReadProvider +
   VcsBytesPort>`。history page・committed/staged snapshot 解決・
   index generation 観測を 1 つの binding（repository_id +
   project_path）へ束ねる。`ChangeSetCache` はここに置かず、共有
   cache は workspace session 側が所有する（後述）。
2. `synvelis-workspace::vcs_ui` — per-view `VcsHistoryState`
   （history cursor・選択 node・A/B pin・view mode・観測 live
   revision）、`HistorySelection`/`PinnedEndpoint`/`EndpointPin`、
   解決 orchestration（`VcsSession` context 経由で backend +
   controller + store を合成）、`VcsHistoryViews` registry、
   `VcsHistoryView` read-model 投影、共有 `ChangeSetCache`。
3. `synvelis-app` — `ViewType::VcsHistory` の screen spec
   （`history_screen_spec`）・`syn.vcs.view` / `syn.vcs.visual`
   op（`visual` は `open`/`close` で detached VCS view の
   headless lifecycle を提供）・`SynvelisApp::set_vcs_backend`
   注入点。

### A/B pin と stale

- `PinnedEndpoint` は `EndpointPin`（再解決レシピ: Live revision /
  Saved head / Staged index_generation / Committed FixedVcsRevision）
  と解決済み `SnapshotIdentity` + `ImmutableSnapshot` を保持する。
  pin は解決時点の bytes に固定され、Live project の前進や index
  変動で内容が書き換わることはない。
- stale 判定は pin の種類ごとに観測値と比較する: Live =
  `controller.live_revision()`、Saved = `store.head()`、Staged =
  `backend.index_generation()`、Committed = immutable（常に fresh、
  解決失敗は `RevisionGone` 系 error として明示）。
- 「History 閲覧中の Live 更新は『新 Revision あり』」は
  `observed_live_revision`（view が最後に観測した project revision）
  と `session.revision()` の差で表示する。固定済み A/B は変わらない。

### ChangeSet 共有

- `VcsHistoryViews` が 1 つの `ChangeSetCache` を所有し、全 view の
  compare が同一 cache を通る。同一 `(before, after, spec)` の
  `Arc<ChangeSet>` は view 間で共有され、独立再計算しない。

### view mode

- 初期 mode は `SideBySide` と `ChangeFocus` の 2 種。Overlay /
  Evolution Map / Rendered Audio Diff は本 slice 対象外（後続）。
- `chosen_parent`（merge parent 選択）は `HistorySelection` の
  metadata として保持し、merge commit を after に取る比較で
  どの parent 系統を辿るかの表示根拠にする。parent 解決は
  `VcsHistoryBackend::revision_relations` の全 parent 保持を使う。

## 受入条件（Issue #95 本 slice 転記）

- [ ] History が immutable ChangeSet として読み取れ、A/B pin で
  任意 2 点の Visual Diff を表示できる（read-only）
- [ ] Dock/2Window で History view が Focus/Selection を保持し、
  Stale を区別する
- [ ] View は Project Commit/Git Index/WorkTree を一切変更しない
  （read-only 契約の検証）

## 非目標

- 実 GPUI/OS window・実 Git repo・実 remote・製品 E2E（#114・実 OS
  gate、`NotRun`）。
- Overlay / Evolution Map / Rendered Audio Diff / waveform delivery。
- Project apply/restore（VCS-05, Issue #96 系）・Git 書き込み系
  （checkout/stage/commit/push/worktree 変更）。
- WorkspaceSnapshot への VCS view state 永続化（session-only、
  SF-05 visual state と同じ扱い）。
