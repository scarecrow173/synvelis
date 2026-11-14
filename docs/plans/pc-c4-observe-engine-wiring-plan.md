---
id: "PLAN-b2e5a8c1f4d90736e5b2a8c1d4e5f"
type: "plan"
status: "approved"
title: "PC-C4 observe engine wiring plan"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-observe-engine-wiring.md"
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/pc-c4-drain-composition-plan.md"
    - "docs/plans/pc-c4-07-transport-mirror-observe-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-observe-engine-wiring.md"
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-observe-engine-wiring.md"
  verified-by:
    - "docs/test-specs/pc-c4-observe-engine-wiring.md"
  references:
    - "docs/specs/pc-c4-observe-engine-wiring.md"
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
    - "docs/plans/pc-c4-07-transport-mirror-observe-plan.md"
  defers: []
  deferred-by: []
---

# PC-C4 observe engine wiring plan

## ファイルマップ

### synvelis-player（`crates/synvelis-player/src/routing_engine.rs` のみ）

- `ObservedBlock { generation, call_start, call_frames }`（新規 pub
  struct）: 1 block 分の node cursor domain call window。
- `RoutingEngine` field `observe_gen: u64` / `observe_head: u64`
  （node cursor domain の追跡状態、初期値 0）。
- `note_processed_block(frames, ran_generation)` — `rt.process`
  成功直後に head 追跡を更新し `ObservedBlock` を組み立てる。
  `ran != observe_gen` のとき activation block として head を 0
  から数え直す。全 `rt.process` 成功を対象にする（`run` block と
  `seek` quiesce pass の両方）。
- `run_observed(blocks, policy, observe: &mut dyn FnMut(ObservedBlock))`
  — `run` と同じ pump/process/capture ループに block 完了ごとの
  observer 呼出しを追加。`run` は no-op observer へ delegate。
- `seek_observed(engine_frame, observe)` — quiesce pass 成功時に
  同じ window を報告。`seek` は no-op observer へ delegate。

### synvelis-app

- `src/engine.rs`
  - `Inner.pending_observe_cycles: Vec<DispatchCycle>`（buffer）。
  - `SessionRoutingEngine::run` を `run_observed` 化し、observer
    内で `inner.dispatch.observe_block(...)` を駆動。per-node
    `DispatchCycle` を block ごとに merge して buffer へ push。
    dispatch 無しは `observe_block` を呼ばない。
  - `control_drain_observations(&self) -> Vec<DispatchCycle>` —
    buffer を take して返す pub accessor（決定的 drain）。
- `src/app.rs`: project 再生成（`project_new_inner`/
  `project_open_inner`）で engine 破棄に伴い buffer は engine 側
  state のため自動的にクリアされることを確認（追加 field 不要の
  見込み）。`drain_control_observations` pub メソッドを追加。
- `src/ui_ops.rs`
  - `drain_control_observations`（pub、決定的 1 pass）— 各 cycle を
    `feed_control_cycle` へ流し fed 数を返す。
  - `ui_pump` 末尾で `drain_control_reports` の直後に呼ぶ。
  - `syn.control.status` の `observe` 節（`last_observed_span` /
    `plan_generation`）と `transport_span_json` helper。
  - `syn.control.observe` op の deferred 記述と
    `ui_ops.rs:830`/`:4925` の deferred コメントを実態に更新。
- テスト `crates/synvelis-app/tests/pc_c4_observe_engine_wiring.rs`
  （新規）: pc_c4_04/06/07 fixture パターン踏襲。

### 文書・証跡

- `docs/{specs,designs,plans,tasks,test-specs}/pc-c4-observe-engine-wiring*`（作成済み/作成中）
- `docs/impl/ir/pc-c4-observe-engine-wiring.md` + `docs/impl/exp/`（実装後）
- `docs/impl/ir/pc-c4-07-transport-mirror-observe.md` の
  「production 経路未配線」記述を本 slice で更新（編集許可範囲内）。
- `evidence/sv101/`（検証ログ）。

## 順序

1. doc chain（spec/design/plan/task/test-spec）。
2. player: `ObservedBlock` + head 追跡 + `run_observed`/`seek_observed`。
3. app engine: `run` observer 配線 + buffer + drain accessor。
4. app: `drain_control_observations` + `ui_pump` + status `observe` 節。
5. fixture テスト（TDD: 赤確認 → 配線完了）。
6. canonical 検証 lane → doc index 再生成 → impl/ir 記録 → review。

## リスク

- node head 追跡の誤り: generation 切替・publish 前 block・seek
  quiesce pass の各ケースを fixture で pin する。`observe_gen`/
  `observe_head` の更新は `rt.process` 成功後・`active_generation`
  snapshot 読み取りで一貫させる。
- `RefCell` 二重借用: observer は `inner.borrow_mut()` 内で
  `dispatch.observe_block`（内部 channel drain のみ、host cell
  不 touch）を呼ぶ — host drain とは別経路で安全。
- buffer 無制限増大: `run` は caller cadence で駆動され、各 block
  で 1 cycle 追加。app の `ui_pump` が drain する前提だが、pump を
  呼ばない caller では蓄積し得る — `DispatchCycle` は bounded な
  report 数を持つため実害は限定的。UI pump 周期で drain される
  production 経路では問題にならない。
