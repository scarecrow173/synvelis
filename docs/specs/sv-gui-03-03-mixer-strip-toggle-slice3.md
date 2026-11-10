---
id: "SPEC-FM394bM9ITIDUkUMhHB3de"
type: "spec"
status: "approved"
title: "SV-GUI-03 slice3: Mixer channel strip — master Mute/Solo Toggle"
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
  depends-on:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
    - "docs/adr/sv-gui-03-01-transport-inspector-app-state-push.md"
    - "docs/adr/sv-gui-03-02-arrangement-readmodel-undo-ops.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-03-03-mixer-strip-target-discovery-subscription.md"
    - "docs/designs/sv-gui-03-03-mixer-strip-toggle-slice3-design.md"
    - "docs/tasks/sv-gui-03-03-mixer-strip-toggle-slice3.md"
    - "docs/test-specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  refines:
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice3 仕様: Mixer channel strip — master Mute/Solo Toggle

## What Should Be Built

Issue #112 の続 slice として、Main 画面 Mixer タブに master strip の
channel toggle（`Mute` / `Solo`）を Ruviform `ComponentSpec::Toggle`
で mount する。slice1 の Transport/Inspector、slice2 の
Arrangement/Undo-Redo に続く 3 つ目の画面要素で、driver-neutral
契約（RF387 View/Component・RF386 Host/Binding）だけを使い、
headless に検証可能な範囲で実装する。

- Mixer タブの strip に `Fader`（既存）に加え、`Mute`・`Solo` の
  `Toggle` を追加する。対象は master strip の seeded
  `ParameterRole::Mute` / `ParameterRole::Solo` target で、
  bootstrap `ControlModel` が well-known ID で seed し、除去不能な
  domain entity である。
- Toggle の controlled display は既存の `syn.control.projection`
  → `StateProjection` 経路（`host_normalized` の binary `{0,1}`
  descriptor）をそのまま使う。専用の投影 channel・第二の Project
  状態は作らない。
- Toggle 操作は Ruviform の atomic `Set`（synthesized
  begin+commit）として `syn.control.dispatch` に流れ、既存の
  gesture → receipt → broadcast projection 経路にそのまま乗る。
  永続 write は domain の automation mode 契約（既定 `Read` では
  gesture 終端で記録系列へ復帰し書き込まない）に従い、Fader と
  同一の意味論を持つ。Undo/Redo は slice2 の
  `syn.project.undo`/`redo` がそのまま効く。
- toggle target の発見は role+owner による `ControlModel` 走査で行い、
  well-known raw ID の算術複製を app 層に持ち込まない。
- toggle target の `(view, target)` subscription は app 層の pump で
  reconcile し、attach 時の静的 target 一覧（`main_view_targets`）
  には project 依存値を混ぜない。

## Why Now

- Issue #112 の受入は「Arrangement/Mixer/Transport/Inspector の実
  画面」を求めており、Mixer は現在 gain `Fader` 1 本だけで channel
  strip として不完全。`Mute`/`Solo` は issue #122 で domain に
  seeded 済みの実 target であり、fake でも placeholder でもなく
  実 Query/Command に接続できる唯一残った Mixer 要素。
- `Toggle` は pinned ruviform rev の `ComponentSpec`/GPUI driver
  双方に実装済みで、新規 driver・第二 UI ライブラリなしに mount
  できる。

## Users and Value

- ユーザーは Mixer タブで master bus の mute/solo を実際の
  parameter commit として操作でき、receipt/projection/Undo の
  意味論が Fader と同一に保たれる。

## Scope

- `crates/synvelis-app/` のみ:
  - `screens.rs`: `MixerScreenState`（発見済み tagged target id）、
    `mixer_strip_targets` resolver、`strip_toggle` spec、Mixer タブ
    の strip panel 化。
  - `app.rs`: `publish_project_length` で `screen.mixer` を更新。
  - `ui_ops.rs`: `ui_pump` で main view の strip-toggle
    subscription を reconcile、Inspector の param target 選択から
    strip toggle を除外。
  - `lib.rs`: 新規 public 型の export。
  - `tests/view_mount.rs` 他: headless 検証の追加。
- docs チェーン（本 spec・ADR・design・plan・task・test-spec）と
  生成 index。

## Non-Goals

- track/channel strip の複数列化、Pan の mount（Pan は
  first-use 登録のため seeded でなく、別 slice の論点）。
- send strip、meter/level 表示、automation lane の GUI。
- `service.rs` 既存 op の変更、synvelis-project/workspace の変更。
- 実 GPUI 入力/IME/Focus/DPI/実 Window の検証（#114 Acceptance、
  NotRun で記録）。
- Table selection や他の未 mount ComponentSpec の追加。

## Acceptance Criteria

- `main_screen_spec` が `state.mixer` の発見済み target に対し
  `ComponentSpec::Toggle`（binary descriptor `{0,1,step:1}`、
  `control_id == target`）を Mixer タブに含む。target 未発見時は
  toggle を出さない。
- attach 後の pump で `(view-1, mute/solo target)` が
  `syn.control.subscribe` され、`syn.control.projection` row が
  `syn.control.projection` notify として outbox に出る。
- `dispatch_input` の atomic `Set`（および Begin/Preview/Commit
  gesture）が `syn.control.dispatch` → `Applied` receipt を通り、
  既定 `Read` mode の契約どおり authoritative 値へ復帰する
  （Fader と同一意味論。`Off` mode での永続 write は workspace
  側で既検証・app op 未露出のため本 slice 対象外）。
- Inspector の表示 target が strip toggle 追加後も従来どおり
  gain fader を指す（strip toggle は inspector 候補から除外）。
- view detach/close で subscription・gesture が既存契約どおり
  cancel/解放される（既存機構のまま、追加経路なし）。
- `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native` / `build-app-native` が成功する。

## Open Questions

- なし（review-protocol で blocker なしを確認済み）。
