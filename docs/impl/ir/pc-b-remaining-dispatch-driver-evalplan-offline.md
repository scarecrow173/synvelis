---
id: "IMPL-034PC64RmwkXyZnPqRsTvWxA"
type: "impl"
status: "completed"
title: "PC-B 残件 — dispatch driver 配線・EvalPlan staleness・offline honesty"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/64"
    - "https://github.com/scarecrow173/synvelis/issues/43"
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/40"
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/197"
    - "https://github.com/scarecrow173/ruviform/issues/198"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/pc-b-remaining-dispatch-driver-evalplan-offline.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/dispatch.rs"
      - type: "file"
        path: "crates/synvelis-player/src/driver.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_b.rs"
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/controller_store.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/session.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_a.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/impl/ir/pc-a-parameter-control-headless-poc.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-B 残件 — dispatch driver 配線・EvalPlan staleness・offline honesty

## Summary

Issue #64 — PC-B (#43、b609b39) の残件。実 `ParamDispatch` を
`HeadlessDriver` に配線 (insert `Processor` slot への attach・
activation/mid-run reactivation/seek にまたがる transport
anchor 同期・per-block `observe`)、`EvalPlan::is_stale` の
recompile-compare 実装 (compile 失敗も stale)、PC-A review
発見の session 不具合修正、そして `offline_support` の upstream
確認を行った。upstream 確認の結果、ruviform 側
`PluginNode::offline_support()` が default `Unknown` のままなのは
**意図的契約**(instance-level `Verified` が拡張点)であり、
consumer 側恒久 workaround は導入せず、PC-B test-local wrapper
の扱いは継続。レビュープロトコルは cooperative/adversarial の
2 サイクルで実施し、確定 blocking 指摘は全て修正済み。

## Context

ベース `origin/main` = 26ad0b8 (`SCHEMA_VERSION=3`、PC-B #43 =
b609b39、ED-B #65 = 26ad0b8 着地済み)。ruviform 依存は既定
ブランチ追従、`Cargo.lock` pin = `258df5ae`。upstream 新規
checkout `b9f2a103` でも `PluginNode::offline_support` 契約に
変更なし。rf#198 (stateful sequential offline execution) は
CLOSED 済み — offline 経路自体は整備されているが、bridge node の
`Unknown` 宣言は「plugin 内部状態が不透明なので host が勝手に
Verified を名乗れない」という設計判断で、consumer が
instance-level で検証・宣言するのが契約上の正しい拡張点。
Synvelis 側 workaround (PC-B の `VerifiedOfflineNode` wrapper)
はその instance-level 検証に相当し、恒久 workaround ではなく
契約どおりの使用として据え置く。

## Implementation

### Changes

`crates/synvelis-player/src/dispatch.rs`

- `DispatchError::Empty` — `dispatch(&[])` は plan/install を
  staging しない明示エラー。
- `control()` / `frame_base()` / `stage()` accessor 追加。
- `in_range` ガードを `bind`/`revalidate`/`unbind`/`resolve`/
  `stage` の全 `ParameterTargetId` 変換経路に適用 —
  `u128 → u64` truncate による alias を防ぐ。
- `ParamDispatch::observe` を二段適用に変更:
  install/control-time report (`VaultExceeded`・`Fault`・
  stale-epoch・receipt) を **投影前** に適用し、call-window
  report (`UndeliverableSpan`・`EventScratchOverflow`・
  `EndpointError`) を **投影後** に適用。
- `scheduler_gen: Option<PlanGeneration>` 追加 — drain した最後の
  `NodeReport::Receipt` が報告する scheduler の live plan
  generation を追跡し、`last_plan` の generation と不一致なら
  `project_call` を skip。apply 時に rejected された
  `install_plan` (budget exceed 等) が `Dispatched` を捏造するの
  を防ぐ。
- `ParamDispatch::new` doc に frame_base == attach-time
  `AttachConfig::frame_base` の契約を明記。
- `control()` doc に out-of-band push (seek/device_restart/
  直接 install_plan) が driver の span/cursor 前提を壊す注意を
  明記。

`crates/synvelis-player/src/driver.rs`

- `AttachedNode { node, dispatch }`、`ActivateError::
  UnmatchedAttached(NodeId)`、`activate_with_attached` を追加 —
  `Processor` slot に対応する `ParamDispatch` を受け取り、
  publish 前に binding を一意性検証 (不一致/重複は last-good
  plan を保持したまま reject)。
- `anchor_attached_transport` — attach 直後の node を
  `set_clock(current_epoch, cursor)` → `Seek` → `Play` の FIFO
  順で driver cursor に anchor。mid-run 再 attach でも position
  が 0 からずれない。
- `observe_attached_block` — 処理済み block を `TransportSpan` +
  cursor で各 dispatch の `observe` に投影する helper。
- `seek()` — clips/attached 存在時に quiesce process pass を
  実行して pending controls を旧 cursor で drain してから
  re-anchor。attached がいる場合は 2 pass (32-control/callback
  drain cap × 2 = 64-channel 全覆盖)。`rt.process` 失敗時は
  observe を skip — 実在しない block を投影しない。
- `attached_dispatch(node_id)` / `param_observations()`
  accessor。

`crates/synvelis-project/src/control.rs`

- `EvalPlan::is_stale` を recompile-compare で実装:
  `ControlGraph::compile(project).map_or(true, |plan| plan != *self)` —
  compile 失敗も stale (Err arm が偽 stale を返さない)。

`crates/synvelis-workspace/src/control.rs` + `session.rs`

- `gesture_end` — Off mode で `points.is_empty()` の gesture は
  `GestureEnd::Returned` を返し、seed された評価値を base edit
  として persist しない。
- `record_points`/recording write 経路 — removed/unbound write
  lane を早期 reject (points が dead lane に溜まらない)。
- `build_commit_ops(&mut self, project: &Project)` — commit 前に
  全 segment lane の生存を検証。dead lane 検出時は transaction
  を restore して Err — commit 失敗が transaction 全体を黙って
  消費しない。
- `install_clock_anchor` — anchor 導入時に playback epoch を
  anchor の frame epoch に同期 (`EpochMismatch` 不発)。
- `param_observations`/gesture/seek doc を実装に合わせて修正。

`crates/synvelis-player/src/lib.rs` — `AttachedNode` 公開。

### Tests

- `controller_store.rs`:
  `eval_plan_is_stale_tracks_frozen_inputs` — enable/disable・
  assignment 差異・depth-source 逆辺による真の cycle
  (`compile.is_err()` → `is_stale` true)。
- `pc_b.rs` driver 系:
  `pc_b_05_headless_driver_dispatch_and_seek` (activation→
  dispatch→seek→run 全経路で endpoint 配送と ledger 一致)、
  `pc_b_05_unmatched_attached_rejected` (同一 insert への
  重複 claim を last-good 保持で reject)、
  `pc_b_05_midrun_activation_reanchors_node` (cursor>0 での
  再 attach が position 0 ずれしない)、
  `pc_b_05_dispatch_before_seek_drains_at_old_cursor`
  (dispatch→seek で旧 plan 再再生しない)、
  `pc_b_05_rejected_install_never_projects_dispatched`
  (`max_pending_applies=16` 越えの install rejection が
  `Dispatched` を捏造せず `Scheduled` のまま)。
- `pc_a.rs` workspace 系:
  `pc_a_05_empty_off_gesture_commits_nothing`、
  `pc_a_05_record_write_rejects_removed_write_lane` (write 時
  reject + commit 時 transaction 保持 + abort)、
  `pc_a_05_stamp_anchor_install_syncs_epoch`。

### Review Fixes Applied

review-protocol の 2 サイクル (cooperative A + adversarial B、
修正後に A′/B′ で再レビュー) で確定し修正したもの:

**サイクル 1 (blocking):**

- `is_stale` の `is_ok_and` が compile Err で `false` を返す
  反転バグ → `map_or(true, …)` に修正 + 真の cycle fixture で
  Err arm を実際にカバー。
- activation が `Play` しか送らず fresh node が position 0 で
  開始 → `set_clock`/`Seek`/`Play` trio に修正。
- dispatch→seek で queued control が旧 cursor で due 計算され
  rewind 後に旧 plan が再発火 → seek 前 quiesce drain に修正。
- quiesce block が observe されず unaccounted event を配送 →
  `observe_attached_block` を quiesce にも適用。
- `observe` の project-then-drain で vault-rejected plan が偽
  `Dispatched` を mint → 二段適用に修正。
- `record_points` が write 前に lane 生存を見ず points を
  蓄積 → write 時点 reject に修正。
- `install_clock_anchor` の epoch 非同期で stamp が
  `EpochMismatch` 不発 → anchor install で epoch 同期。
- `in_range` ガード未整備の `resolve`/`unbind`/`revalidate`/
  `stage` 経路 → 全経路に適用。

**サイクル 2 (blocking):**

- `build_commit_ops` が `take()` 後に validation し、dead lane
  混入で transaction 全体 + healthy lane の points を消失 →
  `take()` 前に lane 生存検証、失敗時 transaction restore。
- seek quiesce が 1 pass = 32 control/callback で、64-entry
  channel の深い queue (>32 pending) で ordering bug が再燃 →
  attached 存在時は 2 pass に拡大。
- `seek` が `rt.process` 失敗を握りつぶして偽 block を observe →
  `is_ok()` 時のみ observe。
- RT 側で rejected な `AppliedReceipt` が ledger で no-op のまま
  plan を `Dispatched` 化し得る → receipt の `plan_generation`
  (scheduler の真値) で投影を gate する `scheduler_gen` を追加。

**レビューで判断し変更しなかったもの (P2/P3、残課題として
記録):**

- `param_observations()` の cross-node 同一 target マージ —
  単一 node 前提では問題なし。複数 attach 時の帰属は要設計。
- activation 時点で pending >30 の anchor trio 分割 —
  activation 前の dispatch 洪水経路のみで発生、64-cap の
  半分まで安全。既知制約として記録。
- quiesce pass は巻戻し窓の event を 1 回分重複配送する —
  seek doc に記載済みの意図的 tradeoff。
- `dispatch` と attach-time `frame_base` の不一致 — bridge に
  accessor がなく検証不能。`ParamDispatch::new` doc で契約
  明記に留めた。
- `control()` 経由の out-of-band op — accessor doc で注意喚起
  に留めた (private 化は API 後退)。
- compile の dangling-ref panic — validated op/load 経路では
  到達不能 (既知制約)。
- 同値 base edit / port-shape 検証 / cancel semantics — caller
  契約または別 issue 候補。

## Related Experiments

### Adopted

なし — issue #64 スコープ内の残件実装。

### Rejected

- `PluginNode::offline_support` を Synvelis 側で Verified に
  強制する恒久 workaround — upstream 確認の結果、`Unknown`
  は意図的契約 (opaque plugin state) であり instance-level
  `Verified` が正しい拡張点。違反ではないため upstream 起票も
  不要と判断。
- receipt の `CommandId` との相関で個別 install を特定する案 —
  `install_plan` が `CommandId` を返さず特定不能。gen 一致
  gate の方が堅牢で簡潔。
- quiesce 深さの動的検出 (drain counter の追加) — bridge に
  drain-depth 公開 API がない。64-cap に対応する決定論的
  2 pass の方が単純かつ網羅的。

## Validation

| 項目 | 結果 |
| --- | --- |
| `mise run fmt-check` | PASS |
| `mise run check` | PASS (`--workspace --all-targets`) |
| `mise run lint` | PASS (`clippy --workspace --all-targets -- -D warnings`) |
| `mise run test` | PASS (35 test binaries、0 failures) |

受入対応: driver 配線 (`pc_b_05_*`)、EvalPlan staleness
(`eval_plan_is_stale_tracks_frozen_inputs`)、PC-A 残件
(`pc_a_05_*` 3 test)、既存 `pc_b_01`/`pc_b_02`/`pc_b_04`/
`rt_` 系は回帰なし。

## Risks

レビューで変更しなかった判断・既知制約 (P2/P3) として残る
リスク:

- `param_observations()` は cross-node 同一 target をマージ
  する — 単一 node 前提では問題ないが、複数 attach 時の帰属は
  未設計。
- activation 時点で pending >30 の anchor trio
  (`set_clock`/`Seek`/`Play`) は分割され得る — activation 前の
  dispatch 集中経路のみで発生し 64-cap の半分まで安全。既知制約。
- seek の quiesce pass は巻戻し窓の event を 1 回分重複配送する
  — seek doc 記載の意図的 tradeoff。
- `dispatch` の frame_base と attach-time `AttachConfig::
  frame_base` の不一致は bridge に accessor がなく検証不能 —
  `ParamDispatch::new` doc の契約明記に留まる。
- `control()` 経由の out-of-band op (seek/device_restart/直接
  install_plan) は driver の span/cursor 前提を壊し得る —
  accessor doc の注意喚起のみ (private 化は API 後退)。
- `EvalPlan::is_stale` は判定ごとに `ControlGraph::compile` を
  再実行する recompile-compare — project 規模に比例したコストが
  呼び出し側に残る。
- `PluginNode::offline_support` は upstream で意図的に
  `Unknown` — instance-level `Verified` 宣言は PC-B test-local
  wrapper に依存し、実 plugin 統合時に再検証が必要。
- compile の dangling-ref panic は validated op/load 経路では
  到達不能 — 直接 Project 構築のみ。既知制約。

## Follow-ups

- `param_observations()` の複数 attach node 間 target 帰属 —
  複数 plugin attach 実用化時に要設計 (別 issue 候補)。
- `ParamWriteMeans::PerNote` 真の note 寿命配送 (PC-A 引継ぎ
  残件のまま)。
- report 二段適用の順序それ自体を直接叩く regression —
  現状は `pc_b_05_rejected_install_*` と vault 系で間接
  カバー。直接 fault 注入経路は bridge mock 拡張待ち。
- activation 前の >30 pending control での anchor trio 分割 —
  既知制約 (activation 前 dispatch 集中を避ける caller 契約)。
- 実 VST3 plugin 統合・GUI・automation lane 表示 (スコープ外)。
