---
id: "SPEC-034AUT02Kw7xRn3vBz9sQm5t"
type: "spec"
status: "implemented"
title: "AUT-02 Region Replace 境界曲線保存・Reject/Replace 原子的編集"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/aut-02-region-replace-boundary-design.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
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
    - "https://github.com/scarecrow173/synvelis/issues/133"
  defers: []
  deferred-by: []
---

# AUT-02 Region Replace 境界曲線保存・Reject/Replace 原子的編集

## What Should Be Built

Issue #132「2026-11-04 合意・初期製品」を実装する。既存
`ConflictPolicy::{Reject, Replace}` と `AutomationRegion/Step/Linear`
を維持したまま、`cut_range_points()` が切断範囲外の曲線を再現
できない欠陥に対し、**境界値処理(Sentinel/Boundary Point)**を追加
する。Project Control Model 以外に Automation 点列を複製しない。

### 現行の欠陥(診断)

評価は target 全 lane の region を 1 つの merged series へ平坦化し、
chase(直前 point の値保持)+ segment interpolation(Step/Linear)で
求める。region の `range` は競合検出と所有 domain のみに使われ、
評価範囲を制限しない。したがって `[c0,c1)` の cut は系列そのものを
書き換え、範囲外にも影響する:

- **右側(復帰値)**: `t ∈ [c1, p1)`(`p1` = c1 より後の最初の
  point)で、旧曲線は `q_S`(c1 未満の最終 point)からの segment
  を引くが、cut 後の系列は別の chase 源(Replace 新 region の末尾
  point 等)を引き、Step では保持値が、Linear では傾斜が変わる。
- **左側(接近 segment)**: `t ∈ [pa, c0)`(`pa` = c0 未満の最終
  point)で、旧曲線は `r0`(c0 以降の最初の point)へ向かう Linear
  segment を引く。cut は `r0` を除去し、接近先が変わる。

### 契約

- 通常の Create/Paste/Import が既存 Region と重複 →
  `Reject`(`Error::AutomationConflict`)/`RequiresUserChoice`
  (caller が policy を明示する既存契約)。Arm 済
  Touch/Latch/Write の録音 commit は**固定された Target と半開
  記録区間 `[start,end)` だけ** `Replace`。非 Arm target、
  区間外の音楽時間は不変。
- Step/Linear の `v_before(boundary)` と右側復帰値を旧曲線から
  計算し、前後 region に必要な Sentinel/Boundary Point を有理
  Beat 座標で挿入する:
  - 左境界: 新 region 内に `(c0, v_S(c0))` を補う(c0 に caller
    明示 point がある場合は caller が境界を所有する)。
  - 右境界: `q_S` が存在し c1 に残存 point が無い場合、
    `(c1, v_S(c1))` を `q_S` の segment interpolation とともに
    c1 を含む region(切断後 region または隣接 region)へ
    挿入、または専用 sentinel region で復帰 segment を再構成する。
- 半開 `[start,end)` と同時刻 point の Ownership:
  boundary point は**その位置に明示 point が存在しない場合にのみ**
  挿入する。Replace 開始位置に caller point があれば caller が
  境界値を所有し sentinel は挿入しない。c1 に残存 point があれば
  それが境界を所有し sentinel は挿入しない。合成 point と明示
  point の同時刻衝突を構造上発生させず、silent last-wins を行わ
  ない。
- 時間正本: 境界 point の座標は `cut.start()`/`cut.end()` の有理
  `BeatPoint` をそのまま使う。Tick/Frame への往復変換・PPQN
  由来の量子化を挟まない(TIM-01 #125 の PPQN 変更コマンド自体は
  本 Issue の範囲外 — 全 point が rational Beat で保持されるため
  将来の PPQN 変更にも不変)。
- CTL-01 の `AutomationTimeScope`: cut/sentinel は target の
  保存座標(scope 解決前の格納系列)上の操作であり、scope 解決は
  評価側の既存契約に委ねる。座標系の混同を追加しない。
- `evaluate_control`/`merged_series` の chase 意味は変更しない。
  評価・RT 経路・永続化スキーマ(新規フィールドなし —
  sentinel は通常の `AutomationPoint`)は変更しない。
- Editor Preview → `ProjectController` scratch/CAS → Commit で
  1 Undo。失敗時は元 Curve/Live/Revision 不変(既存 apply
  原子性契約の維持)。

### 契約上明示する例外(∀ 不変条件の境界)

`∀t ∉ [c0,c1): old_curve(t) == new_curve(t)` は以下の明示例外を
除き成立する。例外は契約で明示し、Property/Fixture 検証も例外を
認識して判定する:

- **E-1 caller 境界所有**: 新 region が c0 に明示 point を持つ
  場合、その値が境界値を所有する。`pa` の segment が Linear の
  とき `[pa, c0)` の接近傾斜は caller 値へ向かい、`v_S(c0)` と
  異なり得る(呼出し側が境界 anchor を明示した編集)。
- **E-2 旧系列未到達**: `q_S`(c1 未満の旧 point)が存在しない
  場合、`[c1, p1)` の旧値は `None`(系列未到達)。point は
  absence を表現できないため、新 region の chase-forward がその
  まま残ることを認める(Step/Linear 両方)。
- **E-3 Erase 左接近**: `EraseAutomationRange` には右側 anchor
  のみ適用する。c0 に point を host できる region が残存しない
  ため、`pa` が Linear のとき `[pa, c0)` の接近は右 anchor
  `(c1, v_S(c1))` へ向かう segment に変わる(Step は chase で
  完全保持)。

## Why

- #6 の AUT-02 行で合意された初期製品 scope。「既存曲線の区間外
  Step/Linear 境界値保存」は本件の新規部分で、Touch 録音の
  punch-out で旧曲線へ正しく復帰するために必須。
- `cut_range_points` は現状 series の点集合だけを操作し、chase/
  補間の文脈を考慮しない。Replace は録音 commit(PC-C1)の書き
  戻し経路でもあり、境界を保存しないと録音のたびに区間外曲線が
  破壊される。
- Region 同士の同時刻 point 競合を「構造上発生させない」形に
  することで、評価の暗黙優先順位をこれ以上増やさない(#6/#40 の
  「silent priority なし」原則)。

## Acceptance Criteria

(Issue #132 §Acceptance が正本。本 PoC で担保する範囲を明記)

- Step/Linear で範囲中央・Point 同時刻・開始/終端・空/単一点・
  複数 Lane・双方向重複・Reverse 区間不正を Property Test。
- `∀t outside replace_range: old_curve(t)==new_curve(t)` を
  正確な Beat 計算の Fixture で確認(上記 E-1/E-2/E-3 例外は
  契約どおり除外・期待値化する)。
- GUI Paste Cancel/Confirm・1 Project Undo・Plugin Control
  RT/Offline Eval・複数 Window Stale/Temp Ref で誤編集 0 —
  headless 実施可能な部分(Undo 1 step 完全復元・CAS stale
  拒否・`eval_rt` と `evaluate_control` の一致・preview 破棄で
  系列不変)を pin。実 GUI/実 Plugin 依存は NotRun/Unsupported
  として理由を記録。
- 既存 `cut_range_points` の全利用箇所を監査し、他の Region
  編集を退行させない。

## Scope Boundary(本 dispatch の範囲)

- **In scope**: `cut_range_points` の境界補完を行う shared 実装と
  `WriteAutomationRegion{Replace}`/`EraseAutomationRange` への
  適用、境界契約の doc/test 固定、利用箇所監査。
- **Out of scope**: PPQN 変更コマンド(TIM-01 #125)、Loop write
  1-pass 規則(AUT-03 #133)、実 Plugin binding(PC-C4 #101)、
  GUI 接続(PC-C5 #102 / SV-GUI-05 #114)、ControlTrace oracle
  (PC-C6 #103)。

## Non-Functional Requirements

- cargo コマンドは `mise run <task>` 経由。時刻演算は
  `checked_add`/`checked_sub` 等の checked API のみ(ruviform
  `Tick`/`TickDuration` unchecked `Add`/`Sub` 禁止)。
- Automation 点列は `Project::control` のみが正本 — session/
  projection 側に複製を作らない。
- `eval_rt` 成功経路の非割当と snapshot 契約を維持する。本件は
  non-RT 側(op apply)のみの変更で、RT 経路の型・契約を変更
  しない。
- 永続化 schema は不変(sentinel は通常 `AutomationPoint`)。
  新規 ID 採番・新エンティティを追加しない。
