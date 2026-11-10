---
id: "TASK-1WNuXmcz8F5r40PEfPLP3u"
type: "task"
status: "in-progress"
title: "SV-GUI-03 slice3 タスク分解 — Mixer strip master Mute/Solo Toggle"
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
    - "docs/plans/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
    - "docs/designs/sv-gui-03-03-mixer-strip-toggle-slice3-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-03-mixer-strip-toggle-slice3.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice3 タスク

- [ ] `screens.rs`: `MixerScreenState` / `mixer_strip_targets` /
      `strip_toggle` / `mixer_strip` 追加、Mixer タブ strip 化、
      `MainScreenState.mixer` 追加、module doc 更新
- [ ] `app.rs`: `publish_project_length` で `screen.mixer` 更新
- [ ] `ui_ops.rs`: `reconcile_mixer_subscriptions` を `ui_pump` に
      追加、inspector 候補から strip toggle 除外
- [ ] `lib.rs`: 新規 public 型の export
- [ ] `tests/view_mount.rs`: subscription/projection、atomic Set、
      gesture、undo 復元、未発見 spec、inspector 非回帰
- [ ] spec unit test: `mixer_strip_targets` 解決・degenerate
- [ ] docs index 再生成
- [ ] mise 検証一式（fmt-check/check/lint/test/test-rt/
      check-app-native/build-app-native）
- [ ] origin/main merge → 最終検証 → push → PR（`Refs #112`）
