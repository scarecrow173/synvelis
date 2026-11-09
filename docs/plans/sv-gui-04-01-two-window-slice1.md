---
id: "PLAN-XQcxlqc1zMCahjjNxAlbO8"
type: "plan"
status: "approved"
title: "SV-GUI-04 slice1 実施計画 — 実 2-window mount"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-04-01-two-window-slice1-design.md"
    - "docs/adr/sv-gui-04-01-two-window-second-view.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
    - "docs/designs/sv-gui-04-01-two-window-slice1-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-04-01-two-window-slice1.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice1 実施計画

## 前提確認

- slice 1/2（PR #199 `009291d`・#201 `3b82404`）merge 済み —
  `ViewSpec → ViewHost → update_view`、`MainScreenState` 共有
  cell、`NativeShell` の `windows` map + `attach_view` が main に存在。
- `host.rs` は別 dispatch（`ctx_f4fa7da66b08`）が編集中 — 触らない。
- `service.rs`・他 crate・`Cargo.toml`/`Cargo.lock` は変更しない。

## 手順

1. **screens**: `secondary_view_id`/`secondary_view_instance`/
   `secondary_view_targets`/`secondary_screen_spec`/
   `mount_secondary_view` を additive に追加。`lib.rs` export。
2. **native bin**: `SYNVELIS_SECOND_WINDOW` env gate +
   mount list 化した run loop（Ready→attach、state 差分→
   window 別 spec push）。
3. **test**: `tests/two_window.rs` — 2 window/2 view の
   FakeDriver fixture と検証ケース。
4. **検証**: `mise run fmt-check`/`check`/`lint`/`test`/`build`
   + `check-app-native`/`lint-app-native`/`build-app-native`。
   `SYNVELIS_SECOND_WINDOW=1` で `run-app-native` を試行し結果を記録。
5. **統合**: `git fetch origin main && git merge origin/main` →
   最終検証 → review-protocol → commit/push/PR/worker_done。

## 判断ポイント

- native 実機試行が環境要因（display/GPU）で不可なら NotRun と
  理由を記録し、fake/headless fixture の Pass をもって slice の
  headless lane 完了とする。両 lane を混同しない。
