---
id: "ADR-WWJ1nzLLj2Q6WEjt7ZQg3t"
type: "adr"
status: "accepted"
title: "SV-GUI-03 slice2: Arrangement rows は workspace read model + MainScreenState push、Undo/Redo は syn.project.* op 露出"
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
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
    - "docs/adr/sv-gui-03-01-transport-inspector-app-state-push.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
    - "docs/designs/sv-gui-03-02-arrangement-undo-slice2-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/sv-gui-03-02-arrangement-undo-slice2-design.md"
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

# ADR: SV-GUI-03 slice2 — Arrangement read model 配置と Undo/Redo op 露出経路

## Context and Problem Statement

Issue #112 slice 2 は Arrangement region の read-only Table mount と
既存 undo/redo の UI 露出を要求する。Arrangement rows の生成には
project の track→lane→clip 構造と clip span 解決
（`TimeAnchor`/`ClipExtent`/`TickTimeMapping`/`resolve_span`）が
必要で、undo/redo には `ProjectSession::undo`/`redo`（既存 pub）
への wire 経路が必要である。いずれも domain op の新規追加なしで
実現できるが、配置と露出経路の選択が必要。

## Decision

1. **Arrangement rows は `synvelis-workspace` の read model として
   実装する**。新規 module `arrangement_ui.rs` に
   `ArrangementRow` 型と `arrangement_rows(&ProjectSession,
   media_rate) -> Vec<ArrangementRow>` を置き、track→lane→clip
   の順序付き walk で行を生成する。clip span は `viz_ui.rs` の
   `timeline_items`（`TimelineAxis::Seconds`）を `pub(crate)` で
   再利用し、Musical/absolute anchor・tempo map 解決のロジックを
   一箇所に保つ。display への供給は slice 1 と同じ
   `MainScreenState` 共有 cell + `update_view` push —
   `publish_project_length`（service op 後の project 由来 state
   publish 経路）で rows を更新する。
2. **Undo/Redo は `syn.project.undo`/`syn.project.redo` op として
   露出する**。app 層に `op_project_undo`/`op_project_redo`
   （`ControlsUi::undo`/`redo` → broadcast + `ui_pump`）を追加し、
   `AppService` の `OPS`/dispatch に登録、`project_action_op` に
   `"undo"`/`"redo"` を追加して toolbar Button（`project` target）
   から既存の project Button 経路で呼ぶ。`ControlsUi::undo`/`redo`
   は既存 pub であり、workspace 層の変更は `timeline_items` の
   `pub(crate)` 化のみ。

## Consequences

### Positive

- clip span 解決の authoritative 実装が `timeline_items` 一箇所に
  留まり、Arrangement table と timeline surface で表示が乖離しない。
- undo/redo は `ControlsUi` の broadcast_all を通るため、購読
  binding の projection/receipt も既存経路で更新される。
- `syn.project.*` op 名は save/reload と同じ project 対象の命名で
  一貫し、frontend/他 surface からも同じ op で呼べる。

### Negative

- `service.rs` の `OPS`/dispatch への op 登録追加が必要
  （編集範囲の確認事項 — Coordinator 承認が前提）。
- Arrangement rows は全 clip を毎 op 走査する — 現行の project
  規模では問題にならないが、大規模 project では将来の最適化対象。

## Considered Options

- **`timeline_items` の戻り値を app 層で再構成**: track/lane の
  構造化 walk と span 解決を分離できず、app 層に domain 知識が
  漏れる。workspace read model が正しい境界。
- **`syn.control.dispatch` の action として undo/redo を露出**:
  `route_action` は transport/param target のみ解釈し、project
  履歴は control surface の責務ではない。`syn.project.*` op が
  意味的に正しい。
- **Arrangement rows を `syn.control.projection` の新 row kind
  で配信**: 新規 projection channel は domain/projection 層の
  変更を要し slice のスコープを超える。`MainScreenState` push で
  十分。

## References

- Spec: docs/specs/sv-gui-03-02-arrangement-undo-slice2.md
- Issue: https://github.com/scarecrow173/synvelis/issues/112
