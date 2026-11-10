---
id: "ADR-8q5UeMmw8CNH1gg0aPvZlr"
type: "adr"
status: "accepted"
title: "SV-GUI-03 slice3: strip toggle target は role+owner 走査で発見し subscription は pump で reconcile"
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
    - "docs/adr/sv-gui-03-01-transport-inspector-app-state-push.md"
    - "docs/adr/sv-gui-03-02-arrangement-readmodel-undo-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
    - "docs/designs/sv-gui-03-03-mixer-strip-toggle-slice3-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/sv-gui-03-03-mixer-strip-toggle-slice3-design.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ADR: SV-GUI-03 slice3 — strip toggle target の発見と subscription 配置

## Context and Problem Statement

Mixer タブに master strip の `Mute`/`Solo` `Toggle` を mount するに
あたり、2 つの配置決定が必要:

1. toggle の bind target（tagged `param-<hex>` id）を app 層がどう
   得るか。master の mute/solo raw ID は synvelis-project の
   `pub(crate)` 定数（`MASTER_MUTE_RAW`/`MASTER_SOLO_RAW`）で、
   app 層からは参照できない。`WELL_KNOWN_BASE + 7/8` の算術複製は
   private 契約への依存であり、番号が動いた場合に黙って壊れる。
2. `(view, target)` subscription をどこで張るか。
   `main_view_targets()` は project 非依存の静的一覧で、
   `syn.view.attach` 時に 1 回だけ subscribe される。toggle target
   は project の `ControlModel` 依存なので同じ経路に乗せられない。

## Decision

- **Target 発見は role+owner 走査**: `screens.rs` の
  `mixer_strip_targets(&ControlModel)` が、well-known fader
  （`MASTER_FADER_RAW`、pub）target の `owner`（master `StripId`）を
  引き、同一 owner の `ParameterRole::Mute`/`Solo` target を
  `id.to_tagged()` で解決する。strip 同定は fader 経由なので
  master strip の well-known ID 群を app 層に複製しない。
  発見結果は `MainScreenState.mixer`（`MixerScreenState`）に保持し、
  `publish_project_length` が project 変化のたびに更新する
  （slice1/2 と同じ app-state push 契約）。
- **Subscription は `ui_pump` で reconcile**: attach 済み main view
  に対し、発見済み target のうち `subscriptions` に無いものを
  `ControlsUi::subscribe` + `subscriptions.insert` する。静的
  `main_view_targets` は変更しない。pump 内で
  `push_subscription_rows` の前に置くため、初回 projection row が
  同一 pump で outbox に出る。
- **送信側は変更しない**: Toggle の atomic `Set` は ruviform が
  synthesized begin+commit（`atomic` 印）に分解し、既存の
  `gesture_to_intent` → `syn.control.dispatch` → param gesture
  path にそのまま乗る。gesture 終端の write 可否は domain の
  automation mode 契約（既定 `Read` = 非書込み）に従い、
  Fader と同一。`binding.rs`/`service.rs`/workspace は無変更。
- **Inspector 候補から strip toggle を除外**: `capture_screen_state`
  の param target 選択で、発見済み mute/solo tagged id を除く。
  これにより Inspector の表示 target は従来どおり gain fader に
  固定される（target 選択 UI は後続 slice）。

## Consequences

### Positive

- well-known raw ID の算術を app 層に複製せず、project の
  `ControlModel` が唯一の真実になる。seeded でない project
  （degenerate control model）では toggle が出ず、phantom target
  への bind を作らない。
- subscription reconcile が pump に載るため、attach 順序・project
  open/reload のどちらが先でも最終的に収束する。view close は
  既存の `subscriptions.retain` がそのまま効く。
- domain/ops/service/workspace に一切触れず、app 層の投影追加のみで
  完結する。

### Negative

- pump ごとに `ControlModel` 走査（targets は BTreeMap、master
  strip 分のみ比較）が走る。対象は高々 strip 数×role 数で、UI
  pump 頻度に対して無視できる。
- strip toggle target は well-known で除去不能なため、reconcile は
  実質「不足分の追加」のみで、target 消失時の自動 unsubscribe 経路
  を持たない（契約上発生しない前提を doc に明記）。

## Considered Options

- **`main_view_targets()` に mute/solo を静的追加**: project 非依存
  の署名を崩すか、well-known ID を app 層に複製する必要があり、
  degenerate project で subscribe が `NotFound` を返す。却下。
- **`syn.control.targets` op の結果を app が利用**: JSON 経由の
  自己呼び出しで型安全性を失う。`ControlModel` 直接走査の方が
  同一プロセス内で厳密。却下。
- **attach 時に一度だけ subscribe（reconcile なし）**: project が
  attach 後に open された場合や将来の strip 追加に追従できない。
  pump reconcile は自己修復的でコストも小さい。採用。
- **Pan も同時に mount**: Pan は `STRIP_SEED_ROLES` に含まれず
  first-use 登録のため、bootstrap project では target が存在しない。
  存在しない target への bind は rejected mint になるため、
  別 slice（first-use 登録 UX）に分離。却下。

## References

- `docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md`
- `crates/synvelis-app/src/screens.rs`（`mixer_strip_targets`）
- `crates/synvelis-app/src/ui_ops.rs`（`reconcile_mixer_subscriptions`）
- `crates/synvelis-project/src/control.rs`（`STRIP_SEED_ROLES`、
  `master_channel_raw`、well-known ID 規約）
