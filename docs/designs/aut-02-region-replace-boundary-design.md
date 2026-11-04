---
id: "DESIGN-034AUT02Qm8wXs5vLn3jKp9r"
type: "design"
status: "approved"
title: "AUT-02 Region Replace 境界曲線保存 設計 — merged series 境界 anchor・host 選択・stitch"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/132"
  changes:
    added: []
    modified:
      - "crates/synvelis-project/src/control.rs"
      - "crates/synvelis-project/src/ops.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
  implemented-by: []
  depends-on:
    - "docs/designs/pc-a-parameter-control-design.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
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

# AUT-02 Region Replace 境界曲線保存 設計

## 1. 目的・非目的

- **目的**: `cut_range_points` を含む op apply 経路で、切断範囲
  `[c0,c1)` の**外側**にある旧曲線値(merged series 評価値)を
  Step/Linear ともに保存する。保存の手段は既存
  `AutomationPoint`(有理 `BeatPoint` 座標・`[0,1]` 値)のみ —
  schema/wire/永続化の変更はしない。
- **非目的**: PPQN 変更コマンド(TIM-01)、Loop write(AUT-03)、
  GUI Paste wiring、新規 Control エンティティ。

## 2. 前提となる評価不変条件(監査結果)

- `merged_series(project, target)` は全 lane の全 region point を
  `(pos, value, interp)` に平坦化し pos で sort。同 pos は
  `validate_references` の `detect_conflicts` が保証する非重複
  不変条件により存在しない。
- `prepared_lane_value(series, pos)` = chase(`pos <= t` の最終
  point `q`)→ 次 point `n` との segment 評価(Step: `v_q`、
  Linear: `lerp(v_q → v_n)`、n 無しは `v_q` 保持)。
- したがって series `S` と cut `[c0,c1)` に対し、範囲外曲線を
  破壊するのは次の 2 系統だけ:
  1. **右側** `[c1, p1)`: `p1` = c1 以降の最初の残存 point。
     旧は `q_S`(c1 未満の最終旧 point)起点、新は切断後の
     chase 源 `q'_S` 起点。`q'_S ≠ q_S` で Step 保持値・Linear
     傾斜が変わる。`q'_S` が無い場合は旧値が `None` に化ける。
  2. **左側** `[pa, c0)`: `pa` = c0 未満の最終残存 point。旧
     Linear 接近先 `r0`(c0 以降の最初の旧 point)が除去され、
     新系列の次 point(`N` 先頭 or 右 anchor)へ向かう。

## 3. 境界 anchor 計画 — `boundary_anchors`

`crates/synvelis-project/src/control.rs` に追加する純粋関数
(pub(crate))。入力は cut 前の merged series と `BeatRange` —
Series 取得元・cut 種別に依存しない。

```rust
/// c0 所有 anchor(新 region 先頭へ挿入する候補)と
/// c1 復帰 anchor(sentinel/boundary point + 復帰 segment interp)。
fn boundary_anchors(
    series: &[(BeatPoint, f64, Interpolation)],
    cut: &BeatRange,
) -> (Option<AutomationPoint>, Option<BoundaryAnchor>)
```

- `left`: `prepared_lane_value(series, c0)` が `Some(v)` →
  `Some(AutomationPoint { pos: c0, value: v.clamp(0,1) })`。
  `v_S(c0)` は c0 に位置する旧 point を chase に含める点に注意 —
  c0 ちょうどの旧 point は除去されるが、その値は anchor 経由で
  接近 segment を保存する。
- `right`: 系列に pos == c1 の point が存在 → `None`(残存 point
  が境界を所有する = E 所有規則)。それ以外で `q_S` が存在 →
  `Some(BoundaryAnchor { pos: c1, value: v_S(c1).clamp(0,1),
  interp: I_qS, source: q_S triple })`。`source` は stitch の
  skip 判定に使う。`q_S` 不在 → `None`(E-2 例外 — absence を
  point で表現しない)。

`BoundaryAnchor` は `pos`, `value`, `interp`, `source` を持つ内部
struct。`AutomationPoint` への変換は挿入時。

## 4. stitch — `stitch_boundary_anchor`(pub(crate))

cut 実行後(Replace では新 region 挿入も済)の `ControlModel`
に対して右 anchor を適用する:

```rust
fn stitch_boundary_anchor(
    control: &mut ControlModel,
    target: ParameterTargetId,
    fallback_lane: AutomationLaneId,
    anchor: &BoundaryAnchor,
) -> Result<(), Error>
```

1. **merge skip**: post-edit merged series の c1 未満最終 point が
   `anchor.source` と同一(pos/value/interp 全一致)→ return。
   消去対象外 lane に `q_S` が残る erase 等で重複挿入しない。
2. **host 選択**: target 全 lane を走査し `c1 ∈ region.range` の
   region(非重複不変条件により高々 1 件)を探す。
   - host の `interpolation == anchor.interp` →
     `host.points.insert(0, anchor_point)`。host 先頭 point は
     `> c1`(c1 の残存 point は anchor 計画で除外済)のため
     厳密昇順を維持。
   - 不一致 → sentinel region `{ range: [c1, p1.pos),
     interpolation: anchor.interp, points: [anchor_point] }` を
     host lane へ追加し、host range を `[p1.pos, host.end)` に
     縮める。`p1` = host 先頭 point(= merged 上 c1 直後 point)。
   - host の点列が空の退避ケース(region 先頭 point < c1 を
     含み得ない構造証明に反する状態)は到達不能 — 不変条件は
     `validate_references` が担保するため防御コードを置かない。
3. **host 不在** → sentinel region を `fallback_lane` へ追加。
   `end = min(s_next, c1 + ε)`、`s_next` = c1 より後に start を
   持つ最初の region、`ε = BeatPoint::new(1, u32::MAX)`。加算は
   `BeatPoint::checked_add` で行い、`None`(理論上の飽和)は
   `Error::Overflow` として op apply 失敗 → scratch 破棄。
   `s_next` が存在すれば `end = s_next` で sentinel region と
   その region が `[c1,s_next)` `[s_next,..)` で隣接し評価上の
   point 到達は `p1` chase に一致する。
4. lane.regions Vec は range.start 昇順を維持して挿入(現行
   `cut_range_points` の出力順と同じ保存形)。

## 5. op 適用の変更(`crates/synvelis-project/src/ops.rs`)

### WriteAutomationRegion{Replace}

```rust
// apply (scratch) — validate 済前提
let target = lane.target;
let series = merged_series_for(control, target);          // pre-edit
let (left, right) = boundary_anchors(&series, &region.range);
for l in control.lanes.values_mut().filter(|l| l.target == target) {
    cut_range_points(&mut l.regions, &region.range);
}
let mut region = region.clone();
if let Some(a) = left
    && region.points.first().is_none_or(|p| p.pos > a.pos)
{
    region.points.insert(0, a);                            // caller point at c0 があれば所有(E-1)
}
control.lanes.get_mut(lane).regions.push(region);
if let Some(anchor) = right {
    stitch_boundary_anchor(control, target, *lane, &anchor)?;
}
```

`merged_series` は現状 `&Project` 起点の private ヘルパ。本件で
`&ControlModel` 起点に揃え(stitch の post 系列取得と共有)、
`pub(crate)` 化する。

### EraseAutomationRange

```rust
let target = lane.target;                  // lane 解決済
let series = merged_series_for(control, target);
let (_left, right) = boundary_anchors(&series, range);
cut_range_points(&mut lane.regions, range);
if let Some(anchor) = right {
    stitch_boundary_anchor(control, target, *lane, &anchor)?;
}
```

Erase は左 anchor を挿入しない(E-3 — c0 を host できる残存
region が無く、residue を消去範囲へ残す代替より範囲外の右側
保存を優先)。

### Reject / 他 op

変更なし。Reject の `AutomationConflict` 検査・`ErasePoints` 等の
他編集経路は従来どおり。`cut_range_points` 本体の signature/
意味(per-lane 機械的切断)は維持し、外部公開名も維持する。

## 6. 契約整理 — Ownership と例外

- **同時刻 point Ownership**: 明示(caller/永続)point が位置を
  所有し、sentinel は明示 point 非存在時のみ挿入される。これは
  `boundary_anchors` の除外条件そのものであり、生成後の同時刻
  競合は非重複不変条件により発生しない。
- **E-1**: caller point at c0 → 左 anchor 挿入なし。`pa` Linear
  接近は caller 値へ向かい得る。
- **E-2**: `q_S` 不在 → 右 anchor なし。`[c1,p1)` の旧値
  `None` は保持できず新系列の chase-forward を認める。
- **E-3**: Erase 左 Linear 接近は右 anchor へ向かう。
- 上記を除き `∀t ∉ [c0,c1)`: `lane_value_old(t) == lane_value_new(t)`
  を成立させる。範囲内は新 region の責務(Replace)/消去(Erase)。

## 7. 原子性・Undo・他経路の不変条件

- 変更は `ProjectOp::apply`(scratch clone 上)内に閉じ、
  `ProjectController::apply` の validate→clone→apply→
  `validate_references`→swap→undo push 契約は非変更。anchor 計算
  `Result` の失敗は op apply 失敗として scratch 破棄 = 元
  Curve/Live/Revision 不変。
- Composite(pc-c1 録音 commit の per-lane Replace)は op 逐次
  適用で各 Replace が進行中の scratch 系列から anchor を計画 —
  lane 間の逐次 stitch と整合。
- `eval_rt`/`EvalPlan` 経路は point 列を読むだけ — sentinel は
  通常 point として snapshot に入る。RT 契約・snapshot 再現性は
  変更なし。

## 8. 監査 — `cut_range_points` 全利用箇所

- `ops.rs` `WriteAutomationRegion{Replace}`: 上記 §5。
- `ops.rs` `EraseAutomationRange`: 上記 §5。右 anchor 追加は意図的
  改善、E-3 例外を契約記載。
- `crates/synvelis-project/src/lib.rs` の re-export 以外に利用箇所
  なし(grep 監査済)。`prepare_record_commit`(workspace)は
  op 構築のみで直接呼ばない — 変更不要。

## 9. 検証方針(抜粋・test-spec 参照)

- 下位層: `boundary_anchors` 計画の unit 相当を op 経由で
  カバー(Step/Linear × 境界点有無 × host interp 一致/不一致 ×
  host 不在)。
- 不変条件 property: 生成ケースで
  `∀t ∉ [c0,c1) old==new`(E-1/E-2/E-3 を oracle 条件で除外)・
  `detect_conflicts == []`・region validate 全通過・undo 1 step
  完全復元・CAS stale 拒否。
- Exact Beat fixture: 有理(1/3・5/6・7/12 系)座標で境界値が
  二進近似を経ないことを pin。
- RT/offline: `EvalPlan` compile 後 `eval_rt` と `evaluate_control`
  の一致(post-replace 系列で)。
