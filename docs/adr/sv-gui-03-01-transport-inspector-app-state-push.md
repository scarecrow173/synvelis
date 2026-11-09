---
id: "ADR-034Ul4shgS8qAsLvnSHlkQ"
type: "adr"
status: "accepted"
title: "SV-GUI-03 slice1: Transport/Inspector の表示値は app 層 MainScreenState push で供給し domain/ops を変更しない"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
    - "docs/designs/sv-gui-03-01-transport-inspector-slice1-design.md"
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
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# ADR: SV-GUI-03 slice1 — Transport/Inspector の表示値は app 層 MainScreenState push で供給し domain/ops を変更しない

## Context and Problem Statement

Issue #112 slice 1 は、Transport 操作（Play/Stop）と Inspector
表示（`InspectorBreakdown`）を既存の driver-neutral
GPUI/ViewHost 経路に接続することを要求する。表示に必要なデータ
（`TransportProjection`、param target の `InspectorBreakdown`）は
`ControlsUi` の既存 projection が既に運んでおり、新しい
`StateProjection` チャンネルや domain op を追加せずに view へ
届ける必要がある。

## Decision

表示専用のスナップショット型 `MainScreenState` を `synvelis-app`
（composition 層）に置き、`SynvelisApp` が所有する `Rc<Cell>` 共有
セルに保持する。`ui_pump`（service op 後の publish 経路）で購読
binding の `last_observed` projection から status/position/loop/
Inspector 行を抽出してセルへ書き込み、native entry point が
`Rc<Cell>` の変化を検出したら既存の `update_view` push
（project-length 更新と同じ経路）で `main_screen_spec` を再構築
して `ViewHost::update` に渡す。

- Transport 操作は `Button`（`target = "transport"`,
  `action = "play"|"stop"`）として mount し、wire は既存の
  `rvf.ui.command.v1` → `syn.control.dispatch` をそのまま使う。
- 状態 readout は spec 再構築で `Label`/`Table` の内容を更新する
  （projection の `apply_projection` は control 値の更新であり、
  spec 内の静的表示テキストの更新は `update` の責務）。
- `main_view_targets()` に `TRANSPORT_TARGET` を追加して
  `transport` の `TransportProjection` 購読を有効化する。

## Consequences

### Positive

- domain/ops 層（`synvelis-workspace`/`synvelis-player`）を一切
  変更せず、既存の projection・receipt・command 経路だけで
  Transport/Inspector を表示できる。
- `update_view` は spec の契約が変わらない限り mounted identity を
  保持するため、seek/fader の gesture 状態を壊さない。
- 後続 slice（Arrangement/Dock 等）も同じ `MainScreenState` push
  パターンを再利用できる。

### Negative

- `app.rs`/`ui_ops.rs`/`lib.rs`/`bin/synvelis-native.rs` への最小
  配線変更が必要（Coordinator 承認済み）。
- Inspector の表示対象は現行の購読 param target（mixer gain fader
  bootstrap）に限定され、選択 UI は後続 slice になる。

## Alternatives Considered

1. **新規 `StateProjection` チャンネルで Inspector を配信**:
   domain/projection 層の変更が必要で slice のスコープを超える。
   既存 `syn.control.projection` param row の `breakdown` を再利用
   する方が整合的。
2. **`ViewHost::apply_projection` で Label/Table を直接更新**:
   `apply_projection` は control_id 単位の値更新であり、静的な
   表示テキスト（Label の content、Table の rows）を動的に変える
   には spec 更新が必要。`update_view` push が既存パターン。
3. **`ControlsUi` に view 向け snapshot getter を追加**:
   `ui_ops.rs` が `ControlsUi` の公開 API 経由で binding の
   `last_observed` を読むだけで足り、workspace 層の変更は不要。

## References

- Spec: docs/specs/sv-gui-03-01-transport-inspector-slice1.md
- Issue: https://github.com/scarecrow173/synvelis/issues/112
