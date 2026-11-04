---
id: "ADR-034AUT02Xy4mKp8wQe2sVn7j"
type: "adr"
status: "accepted"
title: "AUT-02 境界 Sentinel/Anchor 配置 — merged series 単位の境界復元・host region 選択"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/132"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
  implemented-by:
    - "docs/designs/aut-02-region-replace-boundary-design.md"
  depends-on:
    - "docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md"
    - "docs/adr/ctl-01-control-owner-time-scope-schema-v7.md"
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
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
  defers: []
  deferred-by: []
---

# AUT-02 境界 Sentinel/Anchor 配置

## Status

Accepted — Issue #132 の 2026-11-04 合意に基づく初期実装方針。

## Context

- `lane_value`/`prepared_lane_value` は target 全 lane の全 region
  point を merged series へ平坦化し、chase + segment interpolation
  で評価する。region `range` は評価範囲を制限しない。
- `cut_range_points(regions, cut)` は 1 lane の `Vec<AutomationRegion>`
  だけを見るため、merged series 上の chase/segment 文脈(他 lane の
  point・host region の有無・segment interpolation)を扱えない。
- point の所属先は「その pos を range に含む region」のみ。
  同時刻 point は同一 target 上で構造上 1 つしか存在できない
  (region 非重複不変条件)ため、境界 pos の所有規則を明示すれば
  coincident 競合は発生しない。

## Considered Options

- **Option A(採用)**: op apply 層で merged series から境界 anchor
  を計画し、`cut_range_points` は per-lane の機械的切断 primitive
  のまま維持、stitch(anchor 挿入 + host 選択)を target 全体を見る
  別関数に分離する。
- **Option B(却下)**: `cut_range_points` の signature を拡張して
  boundary anchor を受け取らせる。per-lane 関数が他 lane の系列を
  知らないまま anchor 値だけ受け取る形になり、host region 選択・
  merge skip 判定を誤った層へ押し込むため不採用。
- **Option C(却下)**: `AutomationPoint` に per-point interpolation
  または `sentinel` フラグを加える schema 変更。cross-interp 境界
  以外は不要で、wire 変更・migration・sentinel 可視性の追加契約に
  見合わない。anchor は通常 point のままで十分。
- **Option D(却下)**: 削除 point を残す「墓碑」方式(消さずに
  region 外へ退避)。消えた point が別の chase に影響し得る上、
  「range 内の point を除去する」既存契約を壊すため不採用。

## Decisions

### D1: 境界 anchor は pre-edit merged series 上で計画する

`boundary_anchors(series, cut)` は純粋関数として系列スライスだけを
受け、`(left_anchor, right_anchor)` を返す。cut 前の merged
series(target 全 lane)を op apply が取得して渡す。評価が
merged なので anchor 値・host 選択・skip 判定も全て merged 基準。

### D2: 左 anchor は新 region の先頭に畳み込む

Replace では `v_S(c0)` が存在し、かつ新 region の先頭 point が
`pos > c0` のとき、`(c0, v_S(c0))` を新 region 先頭へ挿入する。
新 region の range `[c0,c1)` が c0 を含む唯一の host であるため、
これが構造上唯一の配置先。先頭 point が `pos == c0` の場合は
caller が境界値を所有し sentinel を挿入しない(E-1 例外 —
Linear 接近が caller 値へ向かうことを契約で明示)。

### D3: 右 anchor は c1 を含む region、または専用 sentinel region に置く

`q_S` が存在し c1 に残存 point が無い場合のみ挿入(残存 point は
境界を所有する)。host 選択:

1. c1 を含む唯一の region(切断 right piece または非切断の隣接
   region)の interpolation が `I_qS` と一致 → 先頭へ挿入。
2. interpolation 不一致 → `I_qS` の専用 sentinel region
   `[c1, p1)` を生成し、host の range を `[p1, end)` に縮める
   (`p1` = その region の先頭 point = merged 上 c1 直後の point)。
3. host 不在 → `I_qS` の sentinel region
   `[c1, min(s_next, c1+ε))` を編集 lane へ追加(`s_next` = c1
   より後に始まる最初の region start、`ε` = `1/u32::MAX` beat —
   recording commit の半開拡張と同じ最小刻み)。

post-edit 系列で c1 未満の最終 point が `q_S` と同一(pos/value/
interp)のときは挿入しない — 曲線は既に保存されている(erase で
`q_S` が未編集 lane に残る場合の silent 重複挿入を防ぐ)。

### D4: Erase は右 anchor のみ適用する

`EraseAutomationRange` は編集 lane のみ切断するが、anchor/stitch は
Replace と同じ shared 経路を通る。左側は c0 を含む残存 region が
存在し得ないため、`pa` が Linear のとき `[pa, c0)` 接近は右
anchor への segment に変わる(E-3 例外)。消去範囲内に point/
region の residue を残す代替は、「range 内の point を除去する」
消去契約の方を優先した。

### D5: coincident point は生成しない。Ownership は位置の明示所有で決める

- sentinel は「その位置に明示 point が無い」ときだけ挿入する。
- c0: caller 新 region の point が所有。c1: 残存旧 point が所有。
- これにより評価系列上の同時刻競合は構造上発生せず、並び順依存の
  last-wins は存在しない。

### D6: anchor 値は `prepared_lane_value` の評価値を `[0,1]` clamp する

`v_S(boundary)` は評価系と同一の chase/lerp で計算する(Step で
右側が前値を保持する特性と Linear 接続点を共有実装から取る)。
f64 丸めで `[0,1]` を僅かにはみ出す場合に備え clamp する —
region invariant を apply 後 oracle で破らないための境界処理で
あり、意味変更ではない。

## Consequences

- `WriteAutomationRegion{Replace}` は新 region に最大 1 点、系列に
  最大 1 点(+専用 region 最大 1 件・host range 縮小)を追加する。
  これらは意図する変更であり、Undo/Redo/save-reload で通常 point
  として往復する。
- `EraseAutomationRange` は右 anchor を挿入し得る — 消去契約は
  変わらず、範囲外曲線の保存が改善する意図的変更(監査対象)。
- 実装は non-RT の op apply 層に限定され、`EvalPlan`/snapshot
  契約・wire schema は不変。
