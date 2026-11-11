---
id: "IMPL-034Vzz1zYMrt96iWDBlii8"
type: "impl"
status: "in-progress"
title: "SV-GUI-03 slice4: Arrangement row selection → Inspector + Meter/Waveform viz mount"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added:
      - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
      - "docs/designs/sv-gui-03-04-selection-inspector-viz-design.md"
      - "docs/plans/sv-gui-03-04-selection-inspector-viz-plan.md"
      - "docs/tasks/sv-gui-03-04-selection-inspector-viz.md"
      - "docs/test-specs/sv-gui-03-04-selection-inspector-viz.md"
    modified:
      - "crates/synvelis-app/src/binding.rs"
      - "crates/synvelis-app/src/lib.rs"
      - "crates/synvelis-app/src/screens.rs"
      - "crates/synvelis-app/src/service.rs"
      - "crates/synvelis-app/src/shell.rs"
      - "crates/synvelis-app/src/ui_ops.rs"
      - "crates/synvelis-app/tests/view_mount.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv-gui-03-04-selection-inspector-viz.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
    - "docs/designs/sv-gui-03-04-selection-inspector-viz-design.md"
    - "docs/plans/sv-gui-03-04-selection-inspector-viz-plan.md"
    - "docs/test-specs/sv-gui-03-04-selection-inspector-viz.md"
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-GUI-03 slice4: Arrangement row selection → Inspector + Meter/Waveform viz mount

## Summary

Issue #112（SV-GUI-03 Core Workspace GUI Slice）の最終 headless
slice。Arrangement `Table` の row selection intent を view-local
`ObjectSelections`/inspection focus に反映し Inspector の表示
target を選択対象へ rebinding する経路と、`syn.viz.*` 基盤経由で
Waveform（Arrangement）/Meter（Mixer）を mount し、`VizUi` の
surface meta/view/frame を `MainScreenState.viz` へ正直に投影
する経路を実装した。slice 1–3 の Transport/Inspector・
Arrangement+Undo/Redo・Mixer Mute/Solo の構造は変更せず拡張した。

## Context

`docs/tasks/sv-gui-03-04-selection-inspector-viz.md` /
`docs/plans/sv-gui-03-04-selection-inspector-viz-plan.md` /
`docs/specs/sv-gui-03-04-selection-inspector-viz.md` /
`docs/designs/sv-gui-03-04-selection-inspector-viz-design.md` /
`docs/test-specs/sv-gui-03-04-selection-inspector-viz.md` の
チェーンに従う。doc-driven-dev-graph router は main 上の
既存 broken relation（本 slice 範囲外）で `blocked` を返した
ため、slice 3 と同じく手動維持の文書チェーンで進めた。

## Implementation

### Changes

- `crates/synvelis-app/src/screens.rs`: Arrangement `Table` に
  `TableSelectionSpec{target: "view"}` を付与し、Arrangement tab に
  `Waveform` component + status `Label`、Mixer tab に `Meter`
  component + status `Label` を追加。`MainScreenState.viz`（
  `WaveformScreenState`/`MeterScreenState`）を追加し、tab 一覧を
  `main_tabs` helper に分離（`too_many_lines` 対応）。
- `crates/synvelis-app/src/binding.rs`: `ViewCommand` に view-local
  action の `params` を追加し、`ViewHost` の table select intent
  （`action="select"`, `params={"row": n}`）を shell まで運ぶ。
- `crates/synvelis-app/src/shell.rs`: `answer_view_state_action`
  で `select` action を `dispatch_view_select` → `syn.view.select`
  op に route。rejected 時は既存 receipt 経路で ViewHost の
  optimistic row 選択を復元。
- `crates/synvelis-app/src/service.rs`: `syn.view.select` op を
  OPS 登録・dispatch。
- `crates/synvelis-app/src/ui_ops.rs`: `op_view_select`（view
  解決・lazy workspace re-adoption・row→clip tag 解析・
  `ObjectSelections`/inspection focus 更新・pump・tagged ack）、
  `op_view_list` の `selection`/`inspection` 露出、viz surface
  reconcile（selected clip / master source への bind 再解決）、
  `waveform_screen_state`/`meter_screen_state` 投影
  （`axis_unit_to_frames`/`frame_edge`/`index_units`/
  `waveform_view_bins` helper、bounded cast は viz_ui.rs 先例の
  文書化 `#[allow]`）、meter history（`ui.meter_levels`、
  detach/close で cleanup）、`semantic_inspector_rows`（entity
  readout、対象消失時は `state` 行で正直に報告）。
- `crates/synvelis-app/src/lib.rs`: 新規 pub 型の re-export。
- `crates/synvelis-app/tests/view_mount.rs`: slice-4 の 4 test
  （component mount/unbound 正直状態・malformed/out-of-range
  reject・selection→Inspector/view.list 投影・viz fixture
  bind/frame 投影）と slice-2 の read-only table 前提 assert の
  更新。

### Decisions During Implementation

- `syn.project.open` が `WorkspaceManager` を丸ごと置き換えるため
  view registry が消失し selection が `not a registered workspace
  view` で reject された。`sf_view_instance` と同じ lazy
  re-adoption（`view-<n>` parse → `dock_view_info` →
  `adopt_view` detached pool）を `op_view_select` に適用し、
  placement は後続 attach/move に委ねた。
- `WaveformSpec`/`MeterSpec` に status field が無いため、status は
  隣接 `Label`（`viz.*.status` state key）で正直に表示する形に
  した。
- seconds-axis waveform source で `sample_rate` 未宣言の場合は
  frame 変換せず空 payload を返す（rate を仮定して捏造しない）。
- meter history は `MAX_METER_BINS`/`METER_HISTORY_CAP` で bounded、
  unbound 状態は空 levels + `"no source"` を維持。
- bounded cast（`usize`/`f64`→`u64`/`f64`）は `viz_ui.rs` の
  既存先例に倣い、bound/invariant を明記した局所
  `#[allow]` とした。manual clamp は `.clamp(1, CAP)` に置換。

## Related Experiments

### Adopted

<!-- なし — known-solution の機械的実装のため exp は未作成。 -->

### Rejected

<!-- なし。 -->

## Validation

- `mise run fmt-check` — Pass
- `mise run check` — Pass
- `mise run lint` — Pass（`cargo clippy --workspace --all-targets
  -- -D warnings`）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  `view_mount.rs` 27 test 全 Pass を含む workspace 全体）
- `mise run test-rt` — Pass（`rt_` filter、対象 test Pass）
- `mise run test-msrv` — Pass（rust 1.89.0 lane。初回実行時に
  `synvelis-library` webdav の `same_origin_redirect_is_followed`
  が OS error 10053 で 1 件失敗したが、同 toolchain での単体
  retry と全量 rerun で Pass — 変更範囲外の flaky と判定）
- doc audit — slice-4 の 5 文書 + IR に findings なし
  （main 既存の無関係 finding は範囲外として未対応）
- review-protocol: spec/design/test-spec と diff の突合レビュー
  を実施。結果は PR 本文に記録。

## Risks

- 実 pixel 描画・実 OS GUI E2E は後続 gate（#114 系）に帰属 —
  headless wire/spec 契約のみを検証済み。
- `WaveformView::Samples` の集約は bounded bin 化の近似 —
  decimation 品質は viewport/query 契約側の責務として残る。

## Follow-ups

- 実 OS GUI E2E/IME/pixel・Dock/2 window・Theme・PluginDock・
  RF#464 scenario（NotRun・後続 Issue 帰属）。
- Inspector の param subscription target と entity selection の
  将来的な統合 UI（本 slice は selection 優先・fallback 維持）。
