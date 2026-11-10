---
id: "DESIGN-FQvwpPOrC7St8RoKqyYNMK"
type: "design"
status: "approved"
title: "SV-GUI-03 slice3 Mixer strip Toggle 設計 — role+owner 走査・pump reconcile・atomic Set"
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
  implements:
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sv-gui-03-03-mixer-strip-target-discovery-subscription.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
    - "docs/adr/sv-gui-03-03-mixer-strip-target-discovery-subscription.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice3 設計: Mixer strip — master Mute/Solo Toggle

## Context

slice1/2 で確立した app-state push 契約（`MainScreenState` cell →
`ViewHost::update`）と `syn.control.*` subscription/projection/
receipt 経路の上に、Mixer タブの channel toggle を追加する。
master strip の `Mute`/`Solo` は issue #122 で `ControlModel` に
seeded 済みの実 `ParameterTarget` であり、除去不能な well-known
entity。Ruviform pinned rev は `ComponentSpec::Toggle` と GPUI
driver の `toggle_element`（1 click = atomic `Set`）を実装済み。

## Goals / Non-Goals

**Goals**

- Mixer タブに master `Mute`/`Solo` の `Toggle` を mount し、
  atomic `Set` / gesture が既存 `syn.control.dispatch` 経路の
  gesture lifecycle（receipt・projection・authoritative 復帰）を
  通る。永続 write は domain の automation mode 契約に従う
  （既定 `Read` では書き込まない — Fader と同一）。
- toggle target を `ControlModel` の role+owner 走査で発見し、
  app 層に well-known raw ID を複製しない。
- subscription を pump で reconcile し、attach 順序・project
  open/reload に依らず収束させる。
- Inspector の表示 target を従来どおり gain fader に維持する。

**Non-Goals**

- track strip 複数列、Pan mount、send、meter、automation GUI。
- `binding.rs`/`service.rs`/`synvelis-project`/`synvelis-workspace`
  の変更。新規 op・新規投影 channel の追加。
- 実 GPUI 入力/IME/Focus/DPI/実 Window 検証（#114、NotRun 記録）。

## Design

### `screens.rs`

```rust
/// Master strip channel-toggle state — tagged `param-<hex>` ids
/// discovered from the project's ControlModel. `None` fields mean
/// the target is absent (degenerate control model) and the strip
/// omits that toggle rather than binding a phantom target.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MixerScreenState {
    /// Tagged id of the master strip's `ParameterRole::Mute` target.
    pub mute_target: Option<String>,
    /// Tagged id of the master strip's `ParameterRole::Solo` target.
    pub solo_target: Option<String>,
}
```

- `mixer_strip_targets(control: &ControlModel) -> MixerScreenState`:
  `targets[ParameterTargetId::new(MASTER_FADER_RAW)]` の `owner`
  を master strip として、同一 owner の `role == Mute|Solo` を
  `id.to_tagged()` で収集。fader target が無ければ default
  （両方 `None`）。import 追加: `ControlModel`, `ParameterRole`。
- `strip_toggle(key_name, label, target) -> ComponentSpec`:
  `ComponentSpec::Toggle(ControlSpec { control_id: Some(t),
  target: Some(t), descriptor: {default 0.0, step 1.0,
  ranged(0,1)}, orientation: Horizontal, detach_policy: Cancel,
  a11y: {role: Switch, label} })` — ruviform の binary 検証
  `{min:0,max:1,step:1}` + Linear を満たす。
- `mixer_strip(state: &MixerScreenState)`:
  `panel("tab.mixer.strip", Some("Master"), [mixer_fader(),
  mute?, solo?])` — target が `Some` のものだけ push。
- Mixer タブ child を `panel("tab.mixer.panel", None,
  [mixer_strip(&state.mixer)])` に変更。
- `MainScreenState` に `pub mixer: MixerScreenState` を追加。
- モジュール doc を slice3 内容に更新。

### `app.rs`

`publish_project_length` に 1 行追加:

```rust
screen.mixer = mixer_strip_targets(&self.workspace.session().project().control);
```

project 由来の投影なので service op 後・UI pump 後の両方で
最新化される（同関数は op tail と pump から呼ばれる既存配置）。

### `ui_ops.rs`

`ui_pump` 内、`push_subscription_rows()` の直前に
`self.reconcile_mixer_subscriptions();` を挿入:

```rust
fn reconcile_mixer_subscriptions(&mut self) {
    let view = main_view_id();
    // attached（window 確定）な main view のみ — detach 中・未 attach
    // では subscription を作らない。
    if !self.ui.views.get(&view).is_some_and(|e| e.window.is_some()) {
        return;
    }
    let desired = mixer_strip_targets(&self.workspace.session().project().control);
    for target in [desired.mute_target, desired.solo_target].into_iter().flatten() {
        if self.ui.subscriptions.contains(&(view.clone(), target.clone())) {
            continue;
        }
        let session = self.workspace.session();
        if self.ui.controls.subscribe(session, &view, &target).is_ok() {
            self.ui.subscriptions.insert((view.clone(), target));
        }
    }
}
```

- subscribe 失敗（target が project に無い等）は握りつぶさず
  `is_ok()` で skip — spec 側も同じ discovery を見るため整合する。
- 除去経路は持たない: mute/solo well-known target は
  `RemoveParameterTarget` で除去不能（control.rs 契約）で、
  view close は既存 `subscriptions.retain` が処理する。

`capture_screen_state` の inspector param target 選択で、
`mixer_strip_targets` の結果（mute/solo tagged id）を
`param_targets` から除外してから sort+先頭選択する。

### `lib.rs`

`pub use screens::{...}` に `MixerScreenState`,
`mixer_strip_targets` を追加（`MixerScreenState` は
`MainScreenState` の pub field 型なので export 必須）。

### 変更しないもの

- `binding.rs`: atomic Set は synthesized begin+commit（`atomic`
  印）に分解され、既存 `gesture_to_intent` がそのまま decode
  する。`"set"` kind は wire に存在しない。
- `service.rs`: 既存 op 不変。`syn.control.*`・`syn.project.*` を
  そのまま使用。
- `screens.rs` の `main_view_targets()`: 静的一覧不変。
  toggle target は project 依存のため reconcile 経路。
- `bin/synvelis-native.rs`: composition root 不変（state cell・
  pump・update 経路は既存のまま）。

## Risks / Trade-offs

- **Inspector 選択の暗黙依存**: tag 文字列 sort で fader が先に
  来る現状に依存せず、明示除外で契約を固定する。除外を忘れると
  mute/solo が inspector に出て既存表示が変わる回帰になるため、
  test-spec に固定ケースを置く。
- **reconcile の subscribe 失敗**: `subscribe` は
  `ProjectSession` の `control.targets` に無い target で
  `NotFound` を返す。discovery と subscribe が同一 project を
  見るため通常は起きないが、open/reload の隙間では起こり得る。
  `is_ok()` skip で次 pump が再試行する（収束的）。
- **`ControlModel` 走査コスト**: pump 毎に全 target を舐めるが、
  master strip 判定は fader owner 1 件の比較のみで targets 数は
  プロジェクト規模に比例。UI pump 頻度（操作毎）では無視可能。

## Verification

- `tests/view_mount.rs`: mount→attach→reconcile で mute/solo の
  subscription/projection row が wire に出ること、atomic `Set` で
  receipt `Applied` + projection 反転、`syn.project.undo` で復元、
  gesture Begin/Preview/Commit でも同経路、target 未発見 spec で
  toggle 非含有、を headless 検証。
- `tests/ui_ops.rs` または `screens.rs` unit: `mixer_strip_targets`
  の role+owner 解決・degenerate model で None、
  `main_screen_spec` の Toggle 形状（binary descriptor・
  control_id==target）。
- mise canonical 検証一式（fmt-check/check/lint/test/test-rt/
  check-app-native/build-app-native）。
