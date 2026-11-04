---
id: "TSPEC-034AUT02Fh3jDs8mXq5vWk2n"
type: "test-spec"
status: "approved"
title: "AUT-02 E2E 検証仕様 — Region Replace 境界曲線保存・Reject/Replace 原子的編集"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/132"
  changes:
    added:
      - "crates/synvelis-project/tests/aut_02.rs"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/test-specs/pc-c1-e2e-recording-transaction.md"
    - "docs/test-specs/aut-01-e2e-touch-return-writer-state.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/designs/aut-02-region-replace-boundary-design.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
    - "docs/designs/aut-02-region-replace-boundary-design.md"
    - "docs/plans/aut-02-region-replace-boundary.md"
    - "docs/tasks/aut-02-task1-region-replace-boundary.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/131"
  defers: []
  deferred-by: []
---

# AUT-02 E2E 検証仕様

## 対象・基準

Issue #132 Acceptance が正本。headless で pin できる部分は
`crates/synvelis-project/tests/aut_02.rs`(+ 既存 pc_* 系テストの
無退行)として実施する。実 GUI/実 Plugin 依存は
NotRun/Unsupported として記録する。

## Feature F1: 範囲外曲線保存(Step/Linear)

- **Rule**: `WriteAutomationRegion{Replace}` 適用後、
  `∀t ∉ [c0,c1)` で `lane_value_old(t) == lane_value_new(t)`。
  契約例外 E-1/E-2/E-3 の区間は oracle 条件で除外して判定する。
- **Examples**:
  - 範囲中央の replace(Step・Linear 各)
  - 旧 region が cut 開始/終端に point を持つ(Point 同時刻)
  - cut が既存 region の開始/終端と一致
  - 単一点 region / cut 内 point 無し region
  - 複数 lane 同一 target(他 lane 由来の `q_S`/`p1` を含む)
  - 双方向重複(複数 region を跨ぐ cut)
  - host region の interp 一致/不一致/不在の 3 分岐
  - Reverse 区間不正(`BeatRange::try_new`/`validate` 拒否)
  - 空 cut 相当(validate 拒否 — `start >= end` は不正 range)

## Feature F2: Exact Beat fixture

- **Rule**: `BeatPoint` 有理座標(1/3・5/6・7/12 等、二進有限
  近似を許容しない分母)で anchor 座標・値が厳密一致し、
  Tick/Frame 往復変換を経由しないこと。
- **Examples**:
  - `frac(1,3)` / `frac(5,6)` / `frac(7,12)` 境界の anchor pos が
    `num/den` 正規形で厳密等しい
  - Linear lerp が有理位置で期待値を返す(許容誤差なしの等価
    判定、または既知の lerp 精度内で固定)

## Feature F3: 原子性・Undo・CAS

- **Rule**: 成功時 1 Undo step で全変更(新 region・sentinel・
  host 縮小)が元に戻る。失敗時は元 Curve/Live/Revision 不変。
- **Examples**:
  - Replace 成功 → `undo()` 1 回で region 列・series が byte 等価
  - `try_apply` が `AutomationConflict`(Reject)で失敗 →
    revision・`project.control` 完全不変
  - stale `expected_revision` → `RevisionConflict` で series 不変
  - validate 失敗(不正 range・未知 lane/target)→ live 不変

## Feature F4: coincident point Ownership / silent last-wins なし

- **Rule**: 明示 point が boundary 位置を所有し sentinel は挿入
  されない。適用後 `detect_conflicts` が空、全 region が
  `validate()` を通る。
- **Examples**:
  - 新 region 先頭 point at c0 → 左 anchor 非挿入(E-1)
  - 残存 point at c1 → 右 anchor 非挿入
  - 適用後全 region で `validate()` 通過・`detect_conflicts` 空

## Feature F5: Erase 境界処理・利用箇所監査

- **Rule**: `EraseAutomationRange` は右 anchor を適用し
  `[c1, p1)` を保存する(E-3 で `[pa,c0)` Linear 接近は例外)。
  Reject 経路・非 Arm target は不変。
- **Examples**:
  - erase 後 `t >= c1` で旧曲線復帰(`q_S` が切断 lane に居た
    ケース / 未編集 lane に残るケース = merge skip)
  - erase で消去範囲に point/region residue を残さない
  - `cut_range_points` 利用箇所 grep 監査結果の記録

## Feature F6: RT/Offline 評価一致

- **Rule**: post-replace 系列で `EvalPlan` compile → `eval_rt`
  と `evaluate_control` が一致する。
- **Examples**: sentinel を含む post-replace project で
  `pc_b_rt` 系の compile/eval 経路が既存と同じ結果を返す。

## 除外(NotRun/Unsupported として記録)

- 実 GUI Paste Cancel/Confirm wiring(PC-C5/SV-GUI-05 未実装 —
  headless の op/preview 破棄相当で代替 pin)
- 実 Plugin binding 経由の録音置換(PC-C4 未実装 — prepared
  commit 経路は pc_c1 で pin 済)
