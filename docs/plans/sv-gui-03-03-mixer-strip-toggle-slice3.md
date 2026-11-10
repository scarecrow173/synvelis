---
id: "PLAN-tdVwzz5xn6cX22L90jPXXT"
type: "plan"
status: "approved"
title: "SV-GUI-03 slice3 実施計画 — Mixer strip master Mute/Solo Toggle"
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
    - "docs/designs/sv-gui-03-03-mixer-strip-toggle-slice3-design.md"
    - "docs/adr/sv-gui-03-03-mixer-strip-target-discovery-subscription.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
    - "docs/designs/sv-gui-03-03-mixer-strip-toggle-slice3-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice3 実施計画

## 前提確認

- slice1/2 は main 統合済み（PR #199/#201）。本 slice は Mixer タブ
  の channel toggle 追加のみで、Transport/Inspector/Arrangement/
  Signal Flow の既存要素に触れない。
- master strip の `Mute`/`Solo` target は `ControlModel::bootstrap`
  が well-known ID で seed 済み（issue #122）。除去不能。
- ruviform pinned rev は `ComponentSpec::Toggle`・GPUI
  `toggle_element`・atomic `Set`（synthesized begin+commit）を実装
  済み。`binding.rs` の gesture decode は変更不要。
- `doc-driven-dev-graph` router は main の既存 broken-relation で
  blocked を返すため、slice1/2 と同じ手動 doc chain で進める
  （本書がその記録）。

## 手順

1. `screens.rs`: `MixerScreenState`・`mixer_strip_targets`・
   `strip_toggle`・`mixer_strip` を追加し、Mixer タブを strip
   panel 化。`MainScreenState.mixer` 追加。module doc 更新。
2. `app.rs`: `publish_project_length` で `screen.mixer` を
   `mixer_strip_targets(&project.control)` から更新。
3. `ui_ops.rs`: `ui_pump` に `reconcile_mixer_subscriptions` を追加
   （`push_subscription_rows` 前）。`capture_screen_state` の
   inspector 候補から strip toggle を除外。
4. `lib.rs`: `MixerScreenState`・`mixer_strip_targets` を export。
5. `tests/view_mount.rs`: attach→reconcile→projection row、
   atomic `Set`/gesture→receipt+反転、undo 復元、未発見 spec、
   inspector 非回帰の各ケースを追加。
6. spec unit test（`screens.rs` または既存 test 配置に倣う）:
   `mixer_strip_targets` の role+owner 解決と degenerate model。
7. docs index 再生成（doc-maintenance rebuild-index 相当）。
8. mise canonical 検証一式を実行し、失敗はスコープ内で修正。
9. `git fetch origin main && git merge origin/main` 後に最終検証、
   Conventional Commits で論理分割して push、PR 作成
   （本文日本語・`Refs #112`）。

## 判断ポイント

- toggle target 発見を role+owner 走査にした理由: well-known raw
  ID は `pub(crate)` で app 層から参照不可、算術複製は private
  契約依存。fader owner 経由なら master strip 同定も domain の
  公開情報のみで完結する（ADR 参照）。
- subscription を `main_view_targets` ではなく pump reconcile に
  した理由: project 依存値を静的一覧に混ぜられない・attach 後の
  project open/reload に収束的に追従できる（ADR 参照）。
- Inspector 除外を明示実装した理由: tagged id sort 順への暗黙依存を
  排し、表示 target の契約を固定するため。
