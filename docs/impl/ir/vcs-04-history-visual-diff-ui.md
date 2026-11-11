---
id: "IMPL-034VGuS8MYmQ9PYiMy81Gt"
type: "impl"
status: "completed"
title: "VCS-04 History/Visual Diff UI — headless read-only projection slice (synvelis#95)"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/95"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/vcs/history.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/vcs_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/vcs04_history.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/vcs04_history.rs"
      - type: "file"
        path: "docs/specs/vcs-04-history-visual-diff-ui.md"
      - type: "file"
        path: "docs/designs/vcs-04-history-visual-diff-ui-design.md"
      - type: "file"
        path: "docs/plans/vcs-04-history-visual-diff-ui.md"
      - type: "file"
        path: "docs/tasks/vcs-04-task1-history-diff-headless-slice.md"
      - type: "file"
        path: "docs/test-specs/vcs-04-history-diff-fixtures.md"
      - type: "file"
        path: "docs/impl/ir/vcs-04-history-visual-diff-ui.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/vcs/mod.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/view.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/workspace.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vcs-04-task1-history-diff-headless-slice.md"
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
    - "docs/plans/vcs-04-history-visual-diff-ui.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/impl/ir/vcs-03-git-read-provider.md"
    - "docs/impl/ir/vcs-05-safe-apply-selected-change-restore-as-copy-headless.md"
    - "docs/impl/ir/vcs-06-ui-headless-matrix-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-04-history-visual-diff-ui.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-04-history-diff-fixtures.md"
  verified-by: []
  references:
    - "crates/synvelis-project/src/vcs/history.rs"
    - "crates/synvelis-workspace/src/vcs_ui.rs"
    - "crates/synvelis-workspace/tests/vcs04_history.rs"
    - "crates/synvelis-app/tests/vcs04_history.rs"
    - "crates/synvelis-app/tests/vcs06_ui_matrix.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-04 History/Visual Diff UI — headless read-only projection slice (synvelis#95)

## Summary

Issue #95 の headless 完結 slice として、VCS-01 immutable
Snapshot・VCS-02 semantic `ChangeSet`・VCS-03 read-only provider
を消費する driver-neutral な History/A-B Visual Diff read model を
3 層（project adapter → workspace state/projection → app op/screen
spec）で実装した。コードは PR `#252`（merge commit `cac19b2`、
2026-11-10・18 files/+5453）で main へ merge 済みであり、本
record は merge 時に欠落していた implementation record の backfill
と doc chain 閉鎖（task `done` 化・relation frontmatter 充填・
index 再生成）を記録するもの。backfill 作業では `crates/` 配下を
一切変更せず、canonical mise lane を既 merge コードに対して
再実測した結果をここに記録する。

## Context

Implements `docs/tasks/vcs-04-task1-history-diff-headless-slice.md`
per `docs/specs/vcs-04-history-visual-diff-ui.md` and
`docs/designs/vcs-04-history-visual-diff-ui-design.md`, planned by
`docs/plans/vcs-04-history-visual-diff-ui.md`, verified by
`docs/test-specs/vcs-04-history-diff-fixtures.md`（synvelis#95 の
headless scope。実 GPUI/実 OS/実 Git は #114 gate、Issue #95 は
後続 slice 残存のため Open のまま）。

## Implementation

### Changes

PR `#252` で merge された実装（本 record で再実測した対象）:

- `synvelis-project` — `src/vcs/history.rs` 新規（738 行）:
  object-safe `VcsHistoryBackend` trait と `VcsHistoryAdapter<P:
  VcsReadProvider + VcsBytesPort>`。provider/bytes port を単一
  repository_id + project_path に bind し、history page・fixed
  revision 解決・revision relations・working status・index
  generation・committed/staged snapshot 解決を passthrough。
  foreign repository_id の revision は resolve 前に拒否。
  `src/vcs/mod.rs` に export。src 内 unit test 6 件。
- `synvelis-workspace` — `src/vcs_ui.rs` 新規（1238 行）:
  `EndpointPin`/`PinnedEndpoint`/`HistorySelection`/`VcsViewMode`・
  per-view `VcsHistoryState`・`VcsSession` 解決 orchestration・
  `VcsHistoryViews` registry（全 view 共有の bounded
  `ChangeSetCache`）・`EndpointFreshness` stale 判定・
  `VcsHistoryView`/`VcsSelectionView`/`VcsDiffView` read-model
  投影。`view.rs` に `ViewType::VcsHistory`（serde `vcs-history`）、
  `workspace.rs` に `vcs` registry 統合（spawn/adopt/restore/
  remove/ReleaseViews と lockstep）。session-only、
  `WorkspaceSnapshot` 永続化なし。`tests/vcs04_history.rs` 13 test。
- `synvelis-app` — `screens.rs` に `VcsScreenState` +
  `history_screen_spec`（status/selection/diff label + history/
  changes/issues table、全 `selection: None` の read-only
  ViewSpec）、`app.rs` に `vcs_backend: Option<Arc<dyn
  VcsHistoryBackend>>` + `set_vcs_backend` 注入点、`ui_ops.rs` に
  `op_vcs_view`/`op_vcs_visual`（refresh/load_more/select_node/
  pin/compare/view_mode/clear + detached `open`/`close`）、
  `service.rs` に `syn.vcs.view`/`syn.vcs.visual` 登録。
  `tests/vcs04_history.rs` 9 test。
- docs チェーン一式（spec/design/plan/task/test-spec）。

Doc chain 閉鎖 slice（本 record を含む、コード変更なし）:

- 本 record `docs/impl/ir/vcs-04-history-visual-diff-ui.md` を
  backfill 作成。vcs-01/02/03/05/06 には存在した IR が vcs-04 のみ
  欠落していた。
- task doc: status `in-progress` → `done`、`changes`/
  `implemented-by` 充填、Done When/Verification checkbox を実測に
  整合。
- spec/design/plan/test-spec: relations を現行 contract（新
  frontmatter 形式）へ移行し `implemented-by`/`verified-by`/
  `verifies`/`implements`/`derives-from`/`depends-on` を充填。
- `docs/impl/ir/README.md` に Settled 行を手動追記（generated
  marker を持たない hand-curated index のため）。
- `docs/*/README.md` 管理 index を doc_maintenance で再生成。

### Decisions During Implementation

- **relation 形式の統一**: task doc と本 record は現行の full
  relations 形式を使うのに対し、spec/design/plan/test-spec は旧
  compact 形式（`source`+`verified_by`+`implemented_by`）のまま
  残っていた。旧形式には audit が要求する `implements`/
  `derives-from`/`verifies`/`verified-by`（kebab）key がなく、
  `plan-missing-test-spec-evidence`・`traceability-missing-upstream`・
  `test-spec-missing-verifies` の blocking finding を生じていた。
  vcs-06 チェーンと同じ full relations 形式へ揃え、下流方向は
  `implemented-by`（spec→design→plan→task→IR の chain で
  `implements` と pairwise reciprocal に整合）、上流方向は
  `implements`/`derives-from`/`depends-on` を充填した。
- **test-spec の `implemented-by` は空のまま**: fixture test コード
  は document でないため指す対象が存在せず、`verifies` →
  spec+design（required relation）と `derives-from`/`depends-on`
  で実態を表現。`verified-by` も空のまま（test-spec を検証する
  上流 doc は存在しない）。
- **`defers` は frontmatter に入れない**: 自由記述 `defers` は
  `broken-relation-link` finding を生む既知の挙動（vcs-05/vcs-06
  IR の既存 finding と同型）のため、後続 slice 項目は frontmatter
  ではなく Follow-ups 節に記録した。
- **legacy artifact ID は据え置き**: `TASK-VCS04Hist0ryV1ewTsk01`
  等の `invalid-id-prefix` finding は repo 全体の migrate 対象
  （vcs-01..06 全チェーン・他多数 doc で同型）であり、ID 変更は
  relation 参照更新を伴う repo-wide migration の責務。本 slice
  では変更しない（TIM-02 verification slice と同じ判断）。
- **実測件数の正直記録**: workspace `vcs04_history.rs` は 13 test、
  app `vcs04_history.rs` は 9 test（dispatch spec に記載の
  12/8 件は PR 後の追加を含まない値。実ファイルの `#[test]`
  カウントと `mise run test` 出力で再確認した値を記録する）。

## Related Experiments

### Adopted

<!-- none -->

### Rejected

<!-- none -->

## Validation

`origin/main` @ `7822857`（PR `#321` merge 済み head）に
`git merge origin/main` で追従した merged tree 上で、既 merge
コードに対する canonical mise lane を再実測（Windows worktree、
pinned rustc 1.99.0、MSRV lane は rust 1.89.0）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check` — Pass（exit 0）
- `mise run lint` — Pass（exit 0・warning 0）
- `mise run test` — Pass（exit 0・workspace 全 test binary ok。
  `vcs04_history.rs`（workspace）13/13、`vcs04_history.rs`（app）
  9/9、`vcs06_ui_matrix.rs` 4/4 を含む）
- `mise run test-rt` — Pass（exit 0）
- `mise run test-msrv` — Pass（exit 0・rust 1.89.0）
- `mise run check-app-native` — Pass（exit 0）
- `mise run lint-app-native` — Pass（exit 0）
- 補助証拠（canonical = mise）: `mise exec -- cargo test -p
  synvelis-workspace --test vcs04_history` 13 pass、`mise exec --
  cargo test -p synvelis-app --test vcs04_history` 9 pass、
  `mise exec -- cargo test -p synvelis-project vcs::history` 6 pass
  — Windows/rustc 1.99.0。

PR `#252` merge 時点の証拠は PR 本文に記録済み（本 record は
merge 後の再実測を正規証拠とする）。

Doc 検証（本 slice 作業の検証）:

- `doc_status.js lint` — 対象 chain の relation 系 blocking
  finding を解消（plan `plan-missing-test-spec-evidence`・plan
  `traceability-missing-upstream`・test-spec
  `test-spec-missing-verifies`）。残存は legacy `invalid-id-prefix`
  のみ（repo-wide migrate 対象・据え置き）。
- `audit_impl_record.js` — 本 record の findings 0 件を確認。
- `doc_maintenance.js plan|apply` — task `done` 化に伴う
  `docs/tasks/README.md` 等の管理 index 再生成を適用。
- `docs/impl/ir/README.md` — hand-curated index へ `## Settled`
  行を手動追記（filename 順の末尾 tier）。

## Risks

- 実 GPUI Window・実 OS window/dock・IME/keyboard/a11y・実 Git
  provider 経路は `NotRun`（#114 gate・実機検証）。headless Pass
  を実 GUI Pass と偽装しない。
- VCS-UI-05 は `NotRun` のまま（実機 lane で計測予定、Pass 扱い
  しない）。
- canonical lane は全 workspace 対象のため、他領域の既存 flaky
  が混入し得る。混入した場合は vcs-04 対象 test の個別結果と
  切り分けて記録する（今回の再実測では混入なし・全 Pass）。
- repo 全体の既存 finding（legacy `invalid-id-prefix`・旧形式
  `defers` 自由記述・他 chain の relation drift）は本 slice の
  対象外であり据え置き。

## Follow-ups

- VCS-UI-05（実 OS two-window dock・実機計測）— synvelis#114
  gate の実機 lane 側 slice。
- `VcsHistory` view の native mount 配線（`dock_view_info`・
  composition-root attach 駆動）— design 非目標欄で明示された
  後続 slice。
- production `set_vcs_backend` 配線（composition root で実 Git
  provider を既定接続）— design 非目標欄で明示された後続 slice。
- Overlay / Evolution Map / Rendered Audio Diff・media/waveform
  byte 配送 — spec 非目標、後続 slice。
- Issue #95 は上記後続 slice 残存のため Open 維持（Close は
  Coordinator 判断）。
