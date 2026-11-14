---
id: "IMPL-034XESj4wnQEqAF8AA5DUV"
type: "impl"
status: "completed"
title: "SF-04 loop discontinuity"
created: "2026-11-14"
updated: "2026-11-15"
owners: []
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
  implements:
    - "docs/tasks/sf-04-loop-discontinuity.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/sf-04-slice8-probe-faults-declare.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-04-slice8-probe-faults-declare.md"
    - "docs/test-specs/sf-04-slice8-probe-faults-declare.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-loop-discontinuity.md"
    - "docs/designs/sf-04-loop-discontinuity.md"
    - "docs/plans/sf-04-loop-discontinuity.md"
    - "docs/tasks/sf-04-loop-discontinuity.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/sf-04-loop-discontinuity.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sf-04-loop-discontinuity.md"
    rejected: []
---

# SF-04 loop discontinuity

## Summary

Issue #107 SF-PROBE-07 Loop 脚と `syn.probe.compare` wire 到達性を
実装した。`RoutingControlDispatch::observe_block` が観測 window 内の
loop wrap（scheduler-faithful `WrapSim::walk_window` の `LoopWrap`
point）と `SetLoop`/`ClearLoop` apply（mirror への実 fold）を
`observed_loop_discontinuity` として報告し、`SessionRoutingEngine::
run` が block ごとに `declare_probe_discontinuity` を呼んで probe
clock epoch を cut する。`ui/src/ipc.ts` の `OpName` union に
`"syn.probe.compare"` を追加し、service 側実装済みの比較 op が UI
wire から到達可能になった。

## Context

`docs/specs/sf-04-loop-discontinuity.md`（承認済み spec）と
`docs/designs/sf-04-loop-discontinuity.md`・
`docs/plans/sf-04-loop-discontinuity.md`・
`docs/tasks/sf-04-loop-discontinuity.md` に従う。SF-04 slice8 で
`RoutingEngine::declare_probe_discontinuity`（epoch cut・cursor 非
移動）と app passthrough は実装済み、sv#127 で `WrapSim`/
`boundary_loop_wraps`/`TransportCommand::SetLoop`/`ClearLoop` が
main 入り済み。本 slice は検出点（`observe_block`）と宣言呼出し
（`SessionRoutingEngine::run`）を繋ぐだけで、比較意味論・
transport 契約・probe 語彙は変更しない。

## Implementation

### `crates/synvelis-player/src/routing_control.rs`

- `RoutingControlDispatch` に `last_observed_loop_discontinuity:
  bool` を追加。`observe_block` が呼出しごとにリセットしてから
  per-node 集計し、generation 不一致・surface 不在では `false` の
  まま `None` を返す（stale `true` は次 window に漏れない）。
- `observe_node_block` の戻り値を `(DispatchCycle, bool)` に拡張。
  既存の `wrap_sim.walk_window` 戻り値から `BoundaryKind::LoopWrap`
  を検出し、`note_transport_command` が `SetLoop`/`ClearLoop` の
  実 apply（scheduler の validity rule を通った fold）を `true`
  で返すようにした。`Seek` apply は報告しない — session transport
  seek は `RoutingEngine::seek` が自前で declare 済みで、node 級
  seek は boundary transport 基盤であり本 slice の契約外。
- `observed_loop_discontinuity()` read-only accessor を
  `last_observed_span` の先例に倣って追加。検出のみで宣言は
  呼出し側の責務（observer から engine への逆行所有は作らない）。

### `crates/synvelis-app/src/engine.rs`

- `SessionRoutingEngine::run` を block ごとの `run_observed(1)`
  ループに書き換え、各 block の `observe_block` 直後に
  `observed_loop_discontinuity()` を読み、真なら
  `declare_probe_discontinuity` を呼ぶ。単一 N-block 呼出しでは
  run 末尾まで宣言を遅延できず post-wrap evidence が旧 epoch を
  名乗り続ける（fail-open）ため、block 粒度が必須。
- pump cadence は `MonitorEngine::run` の先例どおり外側 index で
  `Every(1)`/`Every(0)` を選び、`run_observed(1)` の per-call
  index=0 と組み合わせて `run(N, Every(n))` の `index % n` と
  完全一致させた。`PlayReport` は block ごとの pcm を結合して
  再構成し、crc・frames の契約は不変。block 途中の `ProcessError`
  はそれまでの観測 cycle を `pending_observe_cycles` に残して
  そのまま返す（従来どおり）。
- 宣言失敗（`CommandChannelFull`）は `seek` の best-effort bump
  と同じく握り潰すが、broker epoch は push 前に進むため
  `probe_time_context` と deck mark の乖離が観測可能なまま残る
  — `run` の `ProcessError` 契約は `ProbeRequestError` を運べ
  ないため error 伝播はしない（design doc で確定済み）。

### `ui/src/ipc.ts`

- `OpName` union に `"syn.probe.compare"` を追加。service 側
  （`service.rs` `OpInfo`/dispatch・`ui_ops.rs`
  `op_probe_compare`）は実装済みで変更不要。`store.ts` は
  probe 呼出し経路が型を要求しないため未変更（YAGNI）。

### `crates/synvelis-player/tests/sf04_loop_discontinuity.rs`（新規）

4 test。`tim03_loop_boundary.rs` の fixture 作法（plugin host・
boundary declaration・`boundary_stage_window` による transport
command staging・node cursor head 管理）と `sf04_s7`/`sf04_s9`
の probe fixture を合成し、`SessionRoutingEngine::run` の
per-block detect→declare 契約を player 層で再現する
（`crates/synvelis-app/tests/` は別 Worker 占有のため app 層
fixture は置かない）。

- `sf04_loop_wrap_windows_declare_and_close_compare`: 通常再生
  window は不連続なし → loop install 後の全 wrap window で
  検出→宣言（epoch が block ごとに cut・`cursor()` 不変）→
  跨ぎ compare が `DiscontinuityInWindow`。baseline `Aligned`
  を先に pin。
- `sf04_loop_set_and_clear_apply_declare_without_wrap`:
  未来 loop への `SetLoop` apply（window に wrap 無し・
  `boundary_loop_wraps` 空を確認）と `ClearLoop` apply が
  それぞれ epoch cut を起こし、開放 capture segment が
  `ClockEpoch` で閉じる。post-clear の通常 window は不検出。
- `sf04_loop_declare_channel_full_keeps_observable_divergence`:
  検出後に command channel を満杯にして declare →
  `Broker(CommandChannelFull)` を正直に返しつつ broker epoch
  は進行、drain 後 record は旧 epoch（deck 未達）で乖離が
  観測可能、次の成功 wire で resync。
- `sf04_loop_observe_block_stale_generation_clears_flag`:
  generation 不一致の `observe_block` は `None` + flag を
  `false` にクリアし、latched `true` が stale call を越えて
  残らない。

### `docs/specs|test-specs/sf-04-slice8-probe-faults-declare.md`

slice8 spec の deferred 表「loop transport … YAGNI」行を本
slice での解消として更新し、test-spec に解消記録節を追加。

## Decisions

- **検出は `observe_block` 内の集計に留める**: `walk_window`
  が既に scheduler-faithful な wrap を計算し、
  `note_transport_command` が既に apply を fold している —
  別 accessor で occurrence 差分を再計算すると transport 数学を
  二重化する。flag は `observe_block` の戻り値型を変えずに
  read-only accessor で公開し、observer→engine の逆行所有を
  作らない。
- **`SetLoop`/`ClearLoop` apply は wrap 無しでも宣言する**:
  loop region の変更は今後の wrap 軌道＝position domain の
  意味を変えるため、apply 時点で epoch を切るのが
  fail-closed 側の正しい選択（Coordinator 確定ピン）。
- **node 級 `Seek` apply は検出しない**: session の
  `RoutingEngine::seek` が cursor 再 anchor + declare を
  一体で行う既存契約があり、boundary transport の node 級
  seek command は本 slice の宣言対象外。
- **`run` を per-block `run_observed(1)` に分解**: 宣言を
  block 境界に置くには observe callback から engine を触れ
  ないため run loop 側でしか打てない。N-block 一括呼出しの
  ままでは宣言が run 末尾にずれ込み fail-open になる。
- **宣言失敗は best-effort（error を飲む）**: `seek` の
  bump と同じ契約 — epoch は進み deck との乖離が観測可能。
  `run` の `ProcessError` は probe error を運べない。

## Related Experiments

### Adopted

- `docs/impl/exp/sf-04-loop-discontinuity.md` — 検出配置・
  宣言粒度・apply 検出範囲の検討を記録。採用案は上記
  Decisions に反映済み。

### Rejected

- なし（exp は採用案 1 本の検討記録）。

## Validation

canonical = `mise run`（Windows 11、mise 管理 toolchain）。
`origin/main`（`29852e7`、sv#117 monitor session）merge 後の
HEAD で実施。詳細は
`evidence/sv107/verification-sf04-loop-discontinuity.md`。

- `mise run fmt-check` → Passed
- `mise run check synvelis-player` → Passed
- `mise run lint synvelis-player` → Passed
- `mise run test synvelis-player` → Passed（`sf04_loop_
  discontinuity` 4 件含む全 target green）
- `mise run test-rt` → Passed
- `mise run check synvelis-app` → Passed
- `mise run lint synvelis-app` → Passed
- `mise run test synvelis-app` → Passed（補助証拠 —
  `SessionRoutingEngine::run` 再構成の非退行確認）
- `mise run check-ui` → Passed（`sync-ui-deps` で stale
  vendor を lockfile rev `fd1d4d5f` へ再同期後）
- `mise run test-ui` → Passed（vitest 56 tests）
- `mise exec -- prek run --all-files` → Passed
- 補助: `cargo test -p synvelis-player --test
  sf04_loop_discontinuity` → 4 passed（canonical は上記
  `mise run test synvelis-player`）

外部 gate（実機 fault・#108 UI/session・#109 oracle・
`*-msrv`/`*-full`・3OS）は NotRun — 根拠は evidence の
「未検証」節と `evidence/sv107/external-gates.md`。

## Risks

- `SessionRoutingEngine::run` の per-block 分解は pump・pcm
  merge・error 経路を自前で再構成する — `MonitorEngine::run`
  の emulation 先例に倣ったが、`run(N, Every(n))` との
  byte-level 同等性は test でなくコードレビューと既存
  engine test の green で担保する。
- `observe_block` を呼ばない consumer（`run` ではなく
  `run_observed` を直接使う経路）は検出も宣言も行わない —
  宣言は app run loop の責務であり、player 層 API を直接
  使う caller は自前で `observed_loop_discontinuity` を
  読む契約（doc 明記済み）。

## Follow-ups

- `syn.probe.compare` の UI 側呼出し実装（#108 系 slice —
  `OpName` union のみ本 slice で wire 到達性を確保）。
- SF-PROBE-07 の Device Restart 脚・実機 oracle gate
  （`scarecrow173/ruviform#309`）は別 gate。
