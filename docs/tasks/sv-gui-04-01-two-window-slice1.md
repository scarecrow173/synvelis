---
id: "TASK-NFJWSkocsf8xf7WcgpPdL0"
type: "task"
status: "in-progress"
title: "SV-GUI-04 slice1 タスク分解 — 実 2-window mount"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added:
      - "crates/synvelis-app/tests/two_window.rs"
      - "docs/specs/sv-gui-04-01-two-window-slice1.md"
      - "docs/adr/sv-gui-04-01-two-window-second-view.md"
      - "docs/designs/sv-gui-04-01-two-window-slice1-design.md"
      - "docs/plans/sv-gui-04-01-two-window-slice1.md"
      - "docs/tasks/sv-gui-04-01-two-window-slice1.md"
      - "docs/test-specs/sv-gui-04-01-two-window-slice1.md"
    modified:
      - "crates/synvelis-app/src/screens.rs"
      - "crates/synvelis-app/src/lib.rs"
      - "crates/synvelis-app/src/bin/synvelis-native.rs"
      - "docs/specs/README.md"
      - "docs/adr/README.md"
      - "docs/designs/README.md"
      - "docs/plans/README.md"
      - "docs/tasks/README.md"
      - "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv-gui-04-01-two-window-slice1.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv-gui-04-01-two-window-slice1.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-04-01-two-window-slice1.md"
    - "docs/designs/sv-gui-04-01-two-window-slice1-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-04-01-two-window-slice1.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice1 タスク

| # | タスク | 完了条件 |
| --- | --- | --- |
| T1 | `screens.rs` secondary view（id/instance/targets/spec/mount）+ `lib.rs` export | `secondary_screen_spec` が transport deck を生成し `mount_secondary_view` が `view-2` host を返す |
| T2 | `synvelis-native.rs` env-gated 2 窓 mount | `SYNVELIS_SECOND_WINDOW` 有効時に 2 window が `create_view_window`→`Ready`→`attach_view` され、無効時は従来通り |
| T3 | `tests/two_window.rs` fake fixture | 同一 project・独立 view state・cross-window `origin_view` 拒否・per-window receipt/projection の検証が通る |
| T4 | 検証一式 + native 実機試行 | mise タスク全緑 + `run-app-native` 結果を記録 |

## Verification

- [x] `mise run fmt-check && mise run check && mise run lint && mise run test && mise run build` が exit 0（`origin/main` `ba0755f` merge 後に再実行）
- [x] `mise run check-app-native && mise run lint-app-native && mise run build-app-native` が exit 0（同上）
- [x] `cargo test -p synvelis-app --test two_window` が exit 0（補助証拠、canonical は `mise run test` — 8/8 Pass）
- [x] env-gated native 実機試行: `SYNVELIS_SECOND_WINDOW=1` で `w2` 予約・`view-1`/`view-2` attach を log 観測、env 未設定で 1 窓 boot 非回帰を確認（PR #203 本文に記録）
- [x] review-protocol 1 回・CONFIRMED P0/P1 なし → PR 作成済み
