---
id: "IMPL-TiM01uIxYzAbCdEfGh6"
type: "impl"
status: "completed"
title: "TIM-01 PPQN 選択 UI read model + Snap/Grid ポリシー — PpqnUi + Project tab + explicit-tick preservation"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/ppqn_ui.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/tim01_ppqn_ui.rs"
      - type: "file"
        path: "docs/specs/tim-01-ppqn-ui.md"
      - type: "file"
        path: "docs/designs/tim-01-ppqn-ui-design.md"
      - type: "file"
        path: "docs/plans/tim-01-ppqn-ui-plan.md"
      - type: "file"
        path: "docs/tasks/tim-01-ppqn-ui-task.md"
      - type: "file"
        path: "docs/test-specs/tim-01-ppqn-ui.md"
      - type: "file"
        path: "docs/impl/ir/tim-01-ppqn-ui.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/snap.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/state.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/binding.rs"
  implements:
    - "docs/tasks/tim-01-ppqn-ui-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim-01-ppqn-ui-task.md"
    - "docs/impl/ir/tim-01-ppqn-preview.md"
    - "docs/impl/ir/tim-01-app-ppqn-slice.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/tim-01-ppqn-ui.md"
    - "docs/designs/tim-01-ppqn-ui-design.md"
    - "docs/test-specs/tim-01-ppqn-ui.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-01 PPQN 選択 UI read model + Snap/Grid ポリシー

## Summary

Issue #125 の UI 契約を headless/ViewSpec レベルで実装した。
`synvelis-workspace` に view 所有の `PpqnUi` read model
（select→preview→confirm/cancel 状態機械 + `PpqnPreviewView`
投影）を `src/ppqn_ui.rs` に追加し、Snap/Grid
`SnapUnit::Explicit` の PPQN 変更ポリシーを **tick 数
verbatim 維持（beat 間隔のみ変化を `snap_unit_ppqn_effect`
で報告）** と決定・実装した。`synvelis-app` に Project
tab（`PpqnScreenState` → status/preview/effects/sites の
read-only 表示）と `ppqn_{select,preview_pending,confirm,
cancel}` gesture helper・`ViewBinding` op mint を配線し、
headless fixture 11 件で証明した。実 GUI widget intent・
実機/製品 Oracle は NotRun 継続。

## Context

`docs/specs/tim-01-ppqn-ui.md` /
`docs/designs/tim-01-ppqn-ui-design.md` /
`docs/plans/tim-01-ppqn-ui-plan.md` /
`docs/tasks/tim-01-ppqn-ui-task.md` /
`docs/test-specs/tim-01-ppqn-ui.md` に従う。
TIM-01 の UI 契約（New Project 既定 960・選択可能範囲・
変更前 Preview・Explicit Confirm/Cancel・Undo）を
SV-GUI-02/03 の ViewSpec/Binding/Gesture を消費して
headless で実装する。第二の時間モデル・独自描画・旧
WebView は使わない。

## Implementation

### Changes

- `crates/synvelis-workspace/src/ppqn_ui.rs`（新規）:
  `PpqnUi { current_ppqn, pending }` が
  `Idle → Selected → Previewed` を遷移管理。
  `select`（`ppqn_in_range` で view 側 reject）→
  `preview_params`（`syn.project.preview_ppqn` wire 生成）
  → `record_preview`（candidate `new_ppqn` + live
  `old_ppqn` 一致のみ記録、`NoPending`/`PreviewMismatch`
  で拒否）→ `confirm_request`（`preview.applies` のみ、
  `expected_revision` = preview 時 revision を同梱）/
  `cancel`（非破壊）。`observe_current`/`committed` で
  外部変化を再同期。`PpqnPreviewView::from_preview` が
  `PpqnChangePreview` を表示投影（サイトは
  `PREVIEW_SITE_CAP=32` で bounded、`SnapGridEffect`、
  `SMF_EXPORT_NOTE`）。`RECOMMENDED_PPQN =
  [480, 960, 1000, 1920]`、`ppqn_policy_wire` 等の
  wire-name helper。
- `crates/synvelis-workspace/src/snap.rs`:
  `DEFAULT_SNAP_QUANTUM_TICKS = 960`（GridConfig/
  SnapConfig default と共有）、`SnapPpqnEffect`/
  `snap_unit_ppqn_effect(unit, old, new)` — Explicit は
  tick 数維持で beats before/after を報告、Auto は
  zoom-derived（全 `None`）。
- `crates/synvelis-workspace/src/state.rs`: `ViewState.
  grid`/`snap` に tick-preservation ポリシー明記。
  `#[path = "ppqn_ui.rs"] pub mod ppqn_ui` + re-export
  で暫定 mount（`lib.rs` は sv107 lease — coordinator
  が統合時に top-level module へ移設、canonical file
  配置は不変）。
- `crates/synvelis-app/src/screens.rs`: `PpqnScreenState
  { current_ppqn, pending_new_ppqn, pending_policy,
  preview }` + `main_screen_spec` に Project tab
  （`tab.project.panel` > `ppqn.status`/`ppqn.preview`/
  `ppqn.effects`/`ppqn.sites`）。全 component read-only
  （Label/Table、`selection: None`、action なし）。
- `crates/synvelis-app/src/app.rs`: `publish_project_
  length` が `observe_current` + pending preview
  （`project_revision` guard）を `screen.ppqn` に投影。
  `project_preview_ppqn` が preview 計算後に
  `observe_current` + `record_preview` を feed back
  （pending 不一致・bare preview は記録しない）。
  `project_set_ppqn` 成功・project new/open で
  `committed`。`ppqn_preview_json` の `effects.
  snap_grid` を `snap_unit_ppqn_effect` で定量化。
- `crates/synvelis-app/src/ui_ops.rs`: `UiOpsState.
  ppqn_ui: RefCell<PpqnUi>`（`publish_project_length
  (&self)` が唯一の全 mutation path 共通 re-sync 点の
  ため RefCell）+ `ppqn_{select,preview_pending,confirm,
  cancel}`/`ppqn_ui()` helper。`op_project_undo`/`redo`
  成功時に `observe_current`。
- `crates/synvelis-app/src/binding.rs`: `project_undo`/
  `project_redo`/`project_preview_ppqn(params)`/
  `project_set_ppqn(params)` intent mint 追加。
- `crates/synvelis-app/tests/tim01_ppqn_ui.rs`（新規）:
  11 tests。`UiRuntime` + 共有 `AppService` fixture
  （`tim01_ppqn_preview.rs` と同型）+ `SynvelisApp::ppqn_*`
  helper + `PpqnUi` 直接 + spec-tree assertion。

### Decisions During Implementation

- **Snap/Grid ポリシー = explicit-tick preservation**:
  `SnapUnit::Explicit` は gesture domain 基底単位
  （project tick）で固定された量子であり、PPQN 変更を
  跨いで tick 数を verbatim 維持する。rebinding は
  lossy（任意 tick 量子の beat→tick 往復が整数に
  ならない）・persisted preset（`preset.rs`）を project
  command から書き換えられない・`Explicit` の契約が
  tick-fixed である、の 3 点で棄却。`Auto` は
  zoom-derived で追従対象なし。根拠は design に記録。
- **`PpqnUi` を `UiOpsState` の `RefCell` に保持**:
  `publish_project_length(&self)` が `Service::call`
  tail・`main_screen_cell` hand-out・fixture-level
  `session.apply` を含む全 mutation path の後に走る
  唯一の共通点 — `observe_current` をそこに置くことで
  gesture 状態の drift を構造的に排除。
- **spec は display-only**: `rvf.ui.command.v1` の
  `Action` は params を運べず、parameterized confirm/
  cancel を fake button で誤魔化さない — spec は
  `PpqnScreenState` 投影のみ、gesture 駆動は typed
  `ppqn_*` helper + `ViewBinding` mint。実 GUI 配線は
  外部 gate。
- **confirm は preview 時 revision を CAS 同梱**:
  `PpqnConfirm.expected_revision` = `record_preview`
  時の revision — preview 後のあらゆる project 変化で
  `set_ppqn` が正直に reject。screen 表示も
  `project_revision` guard で stale preview を落とす
  二重防御。
- **`project_preview_ppqn` が record feed-back を兼ねる**:
  pending candidate と一致する preview のみ
  `record_preview`（不一致・bare preview は記録しない）
  — view が mint した op 結果がそのまま gesture を
  進める単一経路。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

- Snap/Grid explicit 量子の PPQN 追従 rebinding —
  lossy 変換・persisted preset 所有権・tick-fixed
  契約の 3 点で棄却（design に根拠記録）。

## Validation

- `cargo check -p synvelis-workspace -p synvelis-app
  --all-targets` — green（error/warning なし）
- `cargo test -p synvelis-app --test tim01_ppqn_ui` —
  11 passed（補助証拠。canonical は `mise run test`）
- `mise run fmt-check` — pass（`mise run fmt` 適用後）
- `mise run check` — pass（`cargo check --workspace
  --all-targets`）
- `mise run lint` — pass（`cargo clippy --workspace
  --all-targets -- -D warnings`）
- `mise run test` — pass（`cargo test --workspace
  --locked`、全 crate green。`tim01_ppqn_ui` 11/11 含む）
- `mise run test-rt` — pass（`cargo test --workspace
  --locked rt_`）
- `mise run check-app-native` — pass（`cargo check -p
  synvelis-app --features native --all-targets`）
- `mise run lint-app-native` — pass（`cargo clippy -p
  synvelis-app --all-targets --features native --
  -D warnings`）
- `mise run test-msrv` — pass（`mise exec rust@1.89.0 --
  cargo test --workspace --locked`。初回は C: ドライブ
  一時 full で link 失敗 → mbx sweep で容量回復後に
  retry し全 green、`tim01_ppqn_ui` 11/11 含む）
- `mise run build` — pass（`cargo build --workspace
  --locked`）
- OS/toolchain: Windows（worktree `sv125-tim01-ui`）、
  rust ツールチェーンは mise 管理（pin 1.99.0 /
  MSRV 1.89.0）
- NotRun: 実 GUI widget intent（parameterized
  confirm/cancel）・実 OS/実 GUI・実機・製品 Oracle・
  Presentation Driver 時刻正本移動・SMF export division
  連動（本 slice 対象外）

## Risks

- `synvelis-workspace/src/lib.rs` は sv107 lease のため
  `state.rs` の `#[path]` 暫定 mount — coordinator が
  統合時に `pub mod ppqn_ui` へ移設する handoff を
  design/plan に明記。`synvelis-app/src/lib.rs` の
  `screens::` re-export への `PpqnScreenState` 追加も
  同様に coordinator 作業。
- pending preview が非 PPQN 変化で stale 化する経路 —
  `project_revision` screen guard + `expected_revision`
  CAS の二重防御で stale confirm は正直に reject、
  view は再 preview で回復（fixture で検証済み）。

## Follow-ups

- `synvelis-workspace/src/lib.rs` への top-level
  `pub mod ppqn_ui` 移設 + `state.rs` の暫定 mount 撤去
  （coordinator 統合）。
- `synvelis-app/src/lib.rs` `screens::` re-export への
  `PpqnScreenState` 追加（coordinator 統合）。
- 実 GUI widget intent による select/confirm/cancel
  （`rvf.ui.command.v1` Action params 拡張または
  dedicated gesture channel、後続 slice・外部 gate）。
- SMF export division の project PPQN 連動・
  Presentation Driver 時刻正本移動・実機/製品 Oracle
  （別 slice、NotRun 継続）。
