---
id: "IMPL-AUT02RegionReplaceBoundary"
type: "impl"
status: "completed"
title: "AUT-02 Region Replace 境界曲線保存・境界 anchor 実装記録"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/132"
  changes:
    added:
      - "crates/synvelis-project/tests/aut_02.rs"
      - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
      - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
      - "docs/designs/aut-02-region-replace-boundary-design.md"
      - "docs/plans/aut-02-region-replace-boundary.md"
      - "docs/tasks/aut-02-task1-region-replace-boundary.md"
      - "docs/test-specs/aut-02-e2e-region-replace-boundary.md"
    modified:
      - "crates/synvelis-project/src/control.rs"
      - "crates/synvelis-project/src/ops.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/aut-02-task1-region-replace-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/designs/aut-02-region-replace-boundary-design.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
    - "docs/plans/aut-02-region-replace-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/aut-01-touch-return-writer-state.md"
    - "docs/impl/ir/pc-c1-recording-transaction-prepare-apply-finalize.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-02-e2e-region-replace-boundary.md"
  references: []
  defers: []
  deferred-by: []
---

# AUT-02 実装記録: Region Replace 境界曲線保存

## 実施内容

Issue #132。`cut_range_points` が切断範囲外の曲線を再現できない
欠陥に対し、**merged series 基準の境界 anchor**を追加した。
`cut_range_points` 本体(per-lane 機械的切断 primitive)は不変で、
anchor 計画と stitch を op apply 層に置いた — 評価が target 全
lane の merged series 上で行われるため、境界値・host 選択・重複
挿入抑止も全て merged 基準にする必要があった。

### 追加 API(synvelis-project/control.rs・全て crate 内限定)

- `BoundaryAnchor`: `(pos, value, interp, source)` —
  `value`/`interp` は `prepared_lane_value` で旧系列から評価した
  `v_old(cut.end)` と `q_S` の segment interpolation、`source` は
  post-edit の merge-skip 判定に使う旧 chase 源 triple。
- `boundary_anchors(series, cut)`: pre-edit merged series から
  `(left, right)` anchor を計画する純粋関数。左は
  `(cut.start, v_old(cut.start))`(呼出側が新 region 先頭へ畳み
  込む)、右は `BoundaryAnchor`。同時刻明示 point / `q_S` 不在は
  計画時点で除外(Ownership: 明示 point が境界を所有し、sentinel
  は明示 point 非存在時のみ挿入 — 同時刻競合は構造上発生しない)。
- `stitch_boundary_anchor(project, target, fallback_lane, anchor)`:
  post-edit chase 源が `source` と一致すれば skip(他 lane 残存・
  N 自身の point が `q_S` triple と一致する場合の silent 重複を
  防ぐ)。`pos` を含む唯一の region(host)へは interp 一致なら
  昇順挿入、不一致なら host 分割で `anchor.interp` の sentinel
  region `[pos, end_bound)` を作る。host 不在なら
  `[pos, min(s_next, pos+1/u32::MAX))` の sentinel region を
  `fallback_lane` に追加。regions Vec は `range.start` 昇順維持。

### op 適用(synvelis-project/ops.rs)

- `WriteAutomationRegion{Replace}`: pre-edit series 取得 → anchor
  計画 → 全 lane cut → 左 anchor を新 region 先頭へ畳み込み
  (caller point at `cut.start` は E-1 で所有) → 新 region 挿入 →
  右 anchor stitch。
- `EraseAutomationRange`: 同じ anchor 計画で右側のみ stitch
  (E-3: 左 Linear 接近は `cut.start` を host できる残存 region
  が無いため右 anchor へ向かうことを契約で明示)。
- Reject 経路・他 op は不変。validate/CAS/undo 機構にも変更なし —
  anchor は全て scratch clone 上で完結し、失敗時は op apply 全体が
  破棄される。

### 契約上の例外(test-spec どおり)

- E-1: caller point at `cut.start` が境界値を所有(Linear 接近は
  caller 値へ向かい得る)。
- E-2: `q_S` 不在の `[c1,p1)` は旧値 `None` を point で表現
  できず chase-forward を認める。
- E-3: erase の左 Linear 接近は右 anchor への segment に変わる
  (Step は chase で完全保存)。

## 設計からの差分

- design §5 は `merged_series` の `&ControlModel` 起点化を計画
  したが、ops apply が `&mut Project` を持つため `&Project` のまま
  `pub(crate)` 化に留めた(機能差なし・diff 最小)。
- `stitch_into_host` を独立関数に分割(lint の too_many_lines 対応)。
- design の「host 縮小 = 先頭 point」記述を erase の straddling
  host(他 lane 未編集 region が `c1` を跨ぐケース)を包含する
  一般形(左 piece 温存 + sentinel + 右 piece の 3 分割)に拡張。

## 監査 — `cut_range_points` 利用箇所

- `ops.rs` `WriteAutomationRegion{Replace}`(全 lane):
  anchor 適用 — 本件の対象。
- `ops.rs` `EraseAutomationRange`(単 lane): 右 anchor 適用 —
  右側保存を改善する意図的変更、E-3 例外を契約記載。
- `lib.rs` re-export のみ、他利用箇所なし(grep 監査済)。
- `prepare_record_commit`(workspace)は op 構築のみで直接呼ばず、
  PC-C1 の 1-commit/1-undo 経路はそのまま anchor 恩恵を受ける。

## 検証結果(実施済み・全緑)

- `mise run check`: pass
- `mise run lint`: pass
- `mise run fmt`: 適用済み(fmt-check 相当)
- `mise run test`: 78 テストバイナリ全緑(`tests/aut_02.rs`
  15 件を含む、pc_a/pc_b/pc_c1/aut_01/sv47/sv48/ws_a 等無退行)
- `mise run test-rt`: rt_ 系全緑(71 スイート)
- `mise run test-msrv` (rust 1.89 `cargo test --workspace --locked`):
  全緑

### `tests/aut_02.rs` が pin するもの

- F1: Step/Linear 範囲中央・境界 point 同時刻・region 全域・単一
  point・複数 lane・双方向重複・E-1 caller 所有・reverse/empty 拒否。
- F2: `frac(1,2)`/`frac(2,3)`/`frac(1,3)`/`frac(4,5)`/`frac(9,10)`
  有理座標の anchor pos 厳密一致 + 1/12 刻み probe grid 全範囲外
  一致 + 同時刻重複 0。
- F3: 1 undo で `control` 完全復元・revision +2、Reject/
  StaleRevision/Validation 失敗で `control`・revision 不変。
- F4: `detect_conflicts` 空・全 region `validate()` 通過を全
  ケースで確認(`assert_well_formed`)。
- F5: erase 右側保存・範囲内 residue 0・merge skip(chase 源が
  未編集 lane に残るとき anchor 非挿入)。
- F6: post-replace 系列で `eval_rt` ≡ `evaluate_control` 全 probe
  一致。

## NotRun/Unsupported

- 実 GUI Paste Cancel/Confirm、実 Plugin binding 録音経路 —
  headless の op/atomicity/preview-discard 相当で代替 pin
  (PC-C5/SV-GUI-05・PC-C4 未実装のため)。
- 複数 Window stale/temp ref — session CAS(StaleRevision 拒否)と
  preview 破棄の headless 契約で pin。実 GUI マルチウィンドウは
  未配線。
