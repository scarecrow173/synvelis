---
id: "ADR-G3aQRfIh47FQ5jU2AaDhjd"
type: "adr"
status: "accepted"
title: "SV-GUI-04 slice1: 2nd window は shell.attach_view + create_view_window 経路、view-2 は transport deck spec"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
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
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
    - "docs/adr/sv-gui-03-01-transport-inspector-app-state-push.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/sv-gui-04-01-two-window-slice1-design.md"
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

# ADR: SV-GUI-04 slice1 — 2 window mount の経路と第 2 view の構成

## Context and Problem Statement

Issue #113 slice 1 は「2 つ目の window を実際に mount する最小
経路」を要求する。`NativeShell` は既に `windows: BTreeMap` と
per-window `attach_view`・`origin_view` の owner 検証を持ち、
`GpuiDriver::create_view_window`/`update_view` と
`MainScreenState` 共有 cell + spec push が main に存在する。
`host.rs` の multi-window WindowHandle 抽象は別 dispatch
（precursor）が編集中のため、本 slice はそれに依存しない。

## Decision

1. **2nd window は既存の `create_view_window` + `shell.attach_view`
   経路で mount する**。新しい window/host 抽象を導入せず、
   `synvelis-native` が env flag `SYNVELIS_SECOND_WINDOW` 設定時に
   第 2 `ViewHost` を組み立てて `create_view_window` し、各 window
   の `Ready` で `attach_view` する。view→window 帳簿・
   `Destroyed` teardown・`origin_view` 検証は shell の既存経路が
   そのまま担う。
2. **第 2 view は `view-2`（`ViewInstanceId(2)`）の transport
   deck spec** とする。`Panel > [transport status Label,
   Play/Stop Buttons, Seek]` で `main_screen_spec` と別 region
   構成。同一 `MainScreenState` cell と同一 Project session を
   参照し、購読 target は `transport` + `transport/position` のみ。
3. **状態 push は window ごとの spec 関数で行う**。native lane の
   `update_view` は main window に `main_screen_spec`、2nd window
   に `secondary_screen_spec` を適用する。state cell は共有のまま
   （project length・transport readout は単一 Project の truth）。

## Consequences

### Positive

- shell/service 層の変更が不要 — view id は既存の
  `syn.view.attach`/`syn.control.*` 契約がそのまま受理する。
- cross-window `origin_view` 拒否・window 別 teardown は
  shell.rs の既存 validation/teardown が両 window に効く。
- env flag 未設定時はコードパスが 1 窓のまま — 既存動作を壊さない。

### Negative

- `capture_screen_state` は `view-1` の購読 row のみを読むため、
  view-2 単独購読時の screen state 反映は対象外（本 slice では
  main view が常に attach されるため実害なし）。
- 第 2 spec は transport のみ — mixer/inspector/arrangement の
  2 window 目表示は後続 slice。

## Considered Options

- **`main_screen_spec` をそのまま 2 窓目に mount**: 最小だが
  「2nd screen spec」の追加という slice 範囲を満たさず、
  region 構成差による独立 view state の実演も弱い。
- **`DockWorkspace` wrap（`detach_to_new_window` 等）を使う**:
  detach/reattach の app UX 露出は別 slice の範囲。slice 1 は
  mount 経路の実証が目的。
- **`host.rs` の新 WindowHandle 型に依存**: precursor dispatch
  稼働中で依存不可。既存 `attach_view`/`windows` map で十分。

## References

- Spec: docs/specs/sv-gui-04-01-two-window-slice1.md
- Issue: https://github.com/scarecrow173/synvelis/issues/113
