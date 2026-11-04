---
id: "TASK-034AUT02Gt6kHr4nWy8sBp3x"
type: "task"
status: "done"
title: "AUT-02 task1: 境界 anchor 実装 + op 適用 + property/exact fixture テスト"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/132"
  changes:
    added:
      - "crates/synvelis-project/tests/aut_02.rs"
    modified:
      - "crates/synvelis-project/src/control.rs"
      - "crates/synvelis-project/src/ops.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/aut-02-region-replace-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
    - "docs/designs/aut-02-region-replace-boundary-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/designs/aut-02-region-replace-boundary-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-02-e2e-region-replace-boundary.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/131"
  defers: []
  deferred-by: []
---

# AUT-02 task1: 境界 anchor 実装 + op 適用 + テスト

## 目的

design §3–§5 のとおり、merged series 基準の境界 anchor 計画・
stitch を `synvelis-project` に追加し、`Replace`/`Erase` apply
経路へ適用する。test-spec F1–F6 を `tests/aut_02.rs` で pin する。

## チェックリスト

- [x] `BoundaryAnchor`・`boundary_anchors(series, cut)` 実装
      (control.rs、`prepared_lane_value` 共有)
- [x] `stitch_boundary_anchor(control, target, fallback_lane,
      anchor)` 実装(merge skip → host interp 一致 prepend /
      不一致 sentinel+host 縮小 / host 不在 sentinel region)
- [x] `merged_series` を `pub(crate)` 化(`&Project` 起点維持 —
      impl 記録 §設計からの差分)
- [x] `WriteAutomationRegion{Replace}` apply: anchor 計画→
      cut→左 anchor 畳み込み→右 stitch
- [x] `EraseAutomationRange` apply: 右 anchor のみ stitch
- [x] `tests/aut_02.rs`: F1(範囲外保存 property: Step/Linear ×
      中央・同時刻・端・空・単一点・多 lane・双方向・reverse
      不正)・F2(exact Beat fixture)・F3(undo/CAS/失敗不変)・
      F4(ownership・conflict 空)・F5(erase)・F6(RT 一致)
- [x] `cut_range_points` 利用箇所 grep 監査を記録
- [x] `mise run fmt`/`check`/`lint`/`test`/`test-rt`/`test-msrv`
      全緑

## 完了条件

- test-spec の Feature 群が `tests/aut_02.rs` で pin され全緑。
- 既存 pc_a/pc_b/pc_c1/aut_01/pc_b_rt テストが無退行。
- `detect_conflicts` が適用後 project で空であることを全 Replace
  経路テストが確認する。
