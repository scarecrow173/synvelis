---
id: "EXP-loopdiscontinuity01a2b3c4d5"
type: "experiment"
status: "completed"
title: "SF-04 loop discontinuity — 検出配置・宣言粒度・apply 検出範囲の検討"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/sf-04-loop-discontinuity.md"
  depends-on:
    - "docs/impl/ir/sf-04-loop-discontinuity.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/pc-c4-observe-engine-wiring.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-loop-discontinuity.md"
    - "docs/designs/sf-04-loop-discontinuity.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/sf-04-loop-discontinuity.md"
  defers: []
  deferred-by: []
metadata:
  experiments: []
---

# SF-04 loop discontinuity — experiment log

## EXP-1: loop 不連続の検出をどこに置くか

### 問い

loop wrap と `SetLoop`/`ClearLoop` apply を「この block で起きた」
こととして誰が検出するか。`RoutingControlDispatch::observe_block`
は既に per-node `TransportSpan` を射影し `WrapSim` を window 分
進めているが、検出結果をどこへ公開するかが実装前の分岐点だった。

### 検討した選択肢

1. **`observe_block` 内で flag を集計し read-only accessor で公開**
   — `observe_node_block` が既に呼ぶ `wrap_sim.walk_window` の
   戻り値（`LoopWrap` point のみ）と、`note_transport_command`
   の fold 結果をそのまま使う。戻り値型 `Option<BTreeMap<..>>`
   は不変で、observer→engine の逆行所有を作らない。
2. **別 accessor で occurrence/position 差分を再計算**
   — `last_observed_span` の `ctx.occurrence` や driver の
   `occurrence_before`/`position_at` から前回観測との差分を
   読む。transport 数学を第二の経路で再実装することになり、
   `WrapSim` の pending-command 解決（staged/scheduled・due tie・
   window end 除外）を再現する責任が発生する。
3. **`observe_block` が直接 declare する** — 検出と宣言を一体化。
   observer は `&mut self`（dispatch）のみを持ち engine へ
   到達する所有経路が無く、責務逆転（control surface が probe
   session を知る）になる。

### 結果

選択肢 1 を採用。`observe_node_block` の戻り値を
`(DispatchCycle, bool)` に拡張し、`walk_window` 戻り値の
`BoundaryKind::LoopWrap` 有無と `note_transport_command` が
`SetLoop`/`ClearLoop` の実 apply を fold したかを OR 集計、
`observe_block` が per-node 集計して
`last_observed_loop_discontinuity` に保持、
`observed_loop_discontinuity()` で公開した。flag は呼出しごとに
リセットされ、generation 不一致では `None` と同時に `false`
へ戻るため stale `true` は残らない。

### 却下理由（選択肢 2・3）

- 選択肢 2: `WrapSim` が既に計算した結果を捨てて別経路で
  再計算すると、staged/scheduled command の due 解決規則が
  二重実装になり、片方だけが正しい状態を生む。
- 選択肢 3: `RoutingControlDispatch` は engine を所有しない
  （`attach` は `&mut RoutingEngine` を一時借用するだけ）。
  observer から `declare_probe_discontinuity` を呼ぶには
  dispatch に engine 参照を持たせる設計変更が必要で、
  「検出は player・宣言は composition root」の責務境界を
  崩す。呼出し側（`SessionRoutingEngine::run`）が accessor を
  読んで宣言する方が既存の所有構造に合う。

## EXP-2: 宣言を block どの粒度で打つか

### 問い

`SessionRoutingEngine::run(blocks, policy)` は従来
`run_observed(blocks, policy, observe)` を一括で呼び、
observe callback は block ごとに走る。loop wrap を含む block の
直後・次 block の record より前に epoch を cut するには、
宣言をどの粒度・どの場所で打つか。

### 検討した選択肢

1. **observe callback 内で declare** — callback は
   `inner.bridge` を `run_observed` が借用中で、同じ
   `RefCell` borrow 内から `engine_mut()` を再取得できない
   （`inner` 全体が `borrow_mut` で占有され、field 分割借用は
   callback を越えて生きられない）。
2. **run 末尾でまとめて declare** — wrap を含む block が
   何個あっても 1 回。しかし run 中の後続 block の probe
   record は既に旧 epoch で emit 済みで、post-wrap evidence が
   pre-wrap epoch を名乗り続ける = fail-open。
3. **`run` を block ごとの `run_observed(1)` に分解し、各
   block の observe 直後に declare** — callback 内では flag を
   記録するだけにし、`run_observed` 復帰後（borrow 解放後）に
   `inner.bridge.engine_mut().declare_probe_discontinuity()`
   を呼ぶ。pump cadence は `MonitorEngine::run` の先例どおり
   `Every(1)`/`Every(0)` の emulation で `index % n` と一致。

### 結果

選択肢 3 を採用。`SessionRoutingEngine::run` を per-block
ループに書き換え、pcm は block ごとの `PlayReport` を結合して
crc/frames 契約を維持、`ProcessError` は途中 block で打ち切り
それまでの観測 cycle を `pending_observe_cycles` に残す。
宣言失敗は `seek` の best-effort bump と同じく error を
飲むが、broker epoch は push 前に進むため乖離は観測可能。

### 却下理由（選択肢 1・2）

- 選択肢 1: `run_observed` が `&mut self`（engine）を借用中に
  callback が走るため、callback 内から同じ engine の
  `declare_probe_discontinuity` を呼ぶ所有経路が無い。
  unsafe や第二の engine 参照を導入する価値がない。
- 選択肢 2: wrap を含む block と後続 block が同じ run に
  ある限り、run 末尾の宣言では後続 block の record が旧
  epoch を名乗る。fail-closed 要件（wrap/apply 境界を跨ぐ
  compare が `Aligned` のまま残らない）を満たせない。

## EXP-3: `SetLoop`/`ClearLoop` apply を wrap 無しで宣言するか

### 問い

`SetLoop`/`ClearLoop` の apply は loop region＝今後の wrap
軌道を変えるが、その window に実際の wrap が起きないことも
ある（未来 region への install・position 未到達の clear）。
apply のみで epoch を cut するか、wrap 発生時のみに絞るか。

### 検討した選択肢

1. **apply も宣言する** — `note_transport_command` が実際に
   mirror へ fold した `SetLoop`/`ClearLoop` を検出入力に
   含める。region 変更は position domain の意味を変えるため、
   apply 時点で epoch を切るのが保守側。
2. **wrap のみ宣言する** — `walk_window` の `LoopWrap` point
   だけを見る。apply 直後で wrap 未発生の window は連続扱い。

### 結果

選択肢 1 を採用（Coordinator 確定ピン「loop wrap 発生ごと
および `SetLoop`/`ClearLoop` apply 時に宣言」に一致）。
`note_transport_command` の戻り値を `bool` にして実 apply
のみに限定し、scheduler が reject する不正 command
（空 loop・負 seek 相当）は fold されないため検出にも
乗らない。`Seek` apply は `RoutingEngine::seek` が declare
済みのため対象外とした。

### 却下理由（選択肢 2）

- 選択肢 2: loop region の変更自体が transport 意味の不連続
  であり、wrap が後で起きるまで旧 epoch を名乗らせると、
  apply 前後の position 比較が「同じ loop 下の連続区間」と
  誤分類される。fail-closed 側に倒す。
