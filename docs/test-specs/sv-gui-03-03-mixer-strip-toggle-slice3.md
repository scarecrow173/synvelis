---
id: "TSPEC-6H25Xejdwkw6VwpWCW2abc"
type: "test-spec"
status: "approved"
title: "SV-GUI-03 slice3 テスト仕様 — Mixer strip master Mute/Solo Toggle"
created: "2026-11-10"
updated: "2026-11-10"
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
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
    - "docs/designs/sv-gui-03-03-mixer-strip-toggle-slice3-design.md"
    - "docs/tasks/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice3 テスト仕様

## 範囲

`crates/synvelis-app` の headless 検証。実 GPUI 入力/IME/Focus/
DPI/実 Window は NotRun（#114 Acceptance、後続検証）。

## ケース

### `mixer_strip_targets` 単体（`screens.rs` test または既存配置）

- bootstrap `ControlModel` で master strip の `Mute`/`Solo`
  tagged id が role+owner 一致で解決される。
- `ControlModel::default()`（target 無し）で両方 `None`。
- 別 strip owner の `Mute`/`Solo` は拾わない（master fader owner
  のみ）。

### `main_screen_spec` 形状

- `state.mixer` に target があるとき Mixer タブ配下に
  `ComponentSpec::Toggle` が `toggle.mute`/`toggle.solo` key で
  存在し、descriptor が `{min:0,max:1,step:1}`、
  `control_id == target == tagged id`、`a11y.role == Switch`。
- target が `None` のとき対応 toggle が spec に現れない
  （phantom bind なし）。

### `view_mount.rs`（mount→attach→wire end-to-end）

- attach 後の pump で `(view-1, mute/solo)` の
  `syn.control.subscribe` 結果と `syn.control.projection` notify
  row（`control_id` = tagged target、`value.normalized` が
  `{"normalized": …}` 形）が outbox/driver events に出る。
- `dispatch_input` の atomic `Set`（`gesture_id: None`）で
  begin+commit が mint され、receipt が `Applied` になる。
  既定 `Read` mode では commit 後に authoritative 値（mute off
  `0.0`）へ復帰する — Fader と同一契約であり、永続 write は
  `Off` mode 前提（app op 未露出、workspace 側既検証）。
- Begin/Preview/Commit gesture 列でも同じ commit 経路を通る
  （gesture lifecycle の通過確認。preview 中は controlled
  display が preview 値を示す）。
- gesture `Cancel` で preview が破棄され authoritative display に
  戻る（書き込みなし）。
- strip toggle 追加後も Inspector の `inspector.target` が
  gain fader tagged id のまま（除外の非回帰）。

### `ui_ops.rs`（reconcile 単体、配置により `view_mount.rs` でも可）

- 未 attach の main view では reconcile が subscription を作らない。
- attach→detach→再 attach で subscription が二重に増えない
  （`subscriptions` は `(view,target)` 集合で冪等）。

## Non-Goals（NotRun）

- 実 GPUI `toggle_element` の描画・クリック・IME/Focus/DPI・実
  Window（#114 Acceptance）。FakeDriver headless のみ。
- track strip 複数列・Pan・send・meter の GUI。
- project reload を跨ぐ subscription の stale 観測（well-known
  target は除去不能で desired set が不変のため、契約上発生しない
  前提。発生した場合は別途報告）。
