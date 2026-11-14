---
id: "SPEC-uiMdnhfyQmDLz69Ganfp5Y"
type: "spec"
status: "approved"
title: "SF-04 slice — syn.probe.compare UI wire OpName + loop→probe 不連続宣言（SF-PROBE-07 Loop 脚）"
created: "2026-11-15"
updated: "2026-11-15"
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
  implemented-by: []
  depends-on:
    - "docs/specs/sf-04-slice8-probe-faults-declare.md"
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
    - "docs/specs/tim-03-loop-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/specs/sf-04-slice6-probe-compare.md"
    - "docs/specs/sf-04-slice8-probe-faults-declare.md"
    - "docs/specs/sf-04-slice10-probe-compare-op.md"
  defers: []
  deferred-by: []
---

# SF-04 slice — syn.probe.compare UI wire OpName + loop→probe 不連続宣言（SF-PROBE-07 Loop 脚）

## What Should Be Built

Issue #107（SF-04 Semantic Probe Adapter / SF-PROBE-07）の
2 つの bounded deliverable を 1 slice として実装する。

### A. `syn.probe.compare` UI wire OpName

slice10（`docs/specs/sf-04-slice10-probe-compare-op.md`）で
service 側 op（`OpInfo` 登録・dispatch・`op_probe_compare`）
は実装済みだが、UI wire の `OpName` union
（`ui/src/ipc.ts`）に `"syn.probe.compare"` が無く UI から
呼べない。本 slice で union へ追加し、service 実装との
wire 一致（op 名の綴り・dispatch 到達）を TS 側から
保証する。`ui/src/store.ts` は呼出し経路が型を要求する
場合に限り最小限で追従し、不要なら変更しない（YAGNI）。

### B. loop→probe 不連続宣言（SF-PROBE-07 Loop 脚）

TIM-03（sv#127、`f47e024` merge 済み）で node transport の
loop wrap・`SetLoop`/`ClearLoop` apply が scheduler-faithful
な `WrapSim` と `RoutingControlDispatch::observe_block` の
per-block 観測を通じて追跡可能になった。SF-04 slice8 の
`RoutingEngine::declare_probe_discontinuity`（epoch cut・
cursor 非移動）がその宣言点として既に存在する。本 slice
は「loop wrap 発生ごと**および** `SetLoop`/`ClearLoop`
apply 時に discontinuity を宣言する」結線を完成させる:

- `crates/synvelis-player/src/routing_control.rs` に、直近の
  `observe_block` が probe 関連 transport 不連続（node の
  `WrapSim` window walk が報告した loop wrap、または node
  transport mirror へ fold された `SetLoop`/`ClearLoop`
  apply）を 1 件以上観測したかを返す read-only accessor
  を追加する。accessor は検出のみ — observer から engine
  への逆行所有はさせず、宣言呼出しは呼び出し側の責務。
- `crates/synvelis-app/src/engine.rs` の
  `SessionRoutingEngine::run` が per-block で検出 accessor
  を読み、検出時に `declare_probe_discontinuity` を呼ぶ。
  宣言は block 境界ごとに行い、wrap/apply を含む block の
  次の block から新 epoch の証拠が流れる（epoch cut が
  run 終了まで遅延すると post-wrap 証拠が旧 epoch を名乗り
  fail-open になるため、block 単位の宣言が必須）。
- 宣言は engine cursor・PCM・seek を動かさない
  （`declare_probe_discontinuity` の既存契約どおり）。
  宣言失敗（`CommandChannelFull`）でも fail-closed —
  broker epoch は push 失敗前に進み、deck との乖離は
  `probe_time_context` で観測可能（seek 先例
  `routing_engine.rs` の best-effort bump と同じ扱い）。
- `crates/synvelis-player/tests/` に新規 fixture を 1 本
  追加し、wrap/apply 検出・宣言の発火・cursor 非移動・
  `CommandChannelFull` fail-closed・wrap/apply 跨ぎ
  compare の fail-closed を pin する（app 層 test は
  sv#117 占有のため置かない）。

### C. slice8 deferred の解消記録

`docs/specs/sf-04-slice8-probe-faults-declare.md` の
deferred 表「loop transport | `RoutingEngine` に
loop/restart API は存在しない・YAGNI」行を、本 slice
での解消（要件発生 = SF-PROBE-07 Loop 脚）として更新し、
`docs/test-specs/sf-04-slice8-probe-faults-declare.md` も
整合させる。

## Why Now

- `syn.probe.compare` は service 側まで完成しているが
  UI wire union の欠落で UI consumer（#108 系）が呼べない
  — SF-PROBE-07 比較経路の最後の繋ぎ。
- SF-PROBE-07 の Loop 脚は「loop wrap・`SetLoop`/`ClearLoop`
  apply で probe clock epoch を切る」ことを要求する。
  検出機構（`WrapSim`/`observe_block`/transport mirror）と
  宣言機構（`declare_probe_discontinuity`）は両方実装済みで、
  本 slice は所有権境界を正しく結ぶ検出 accessor と
  run-loop 呼出しだけを足す。
- slice8 spec が deferred として記録した「loop transport」
  項目の要件が本 slice で発生したため、deferred 表の
  解消記録が必要。

## Users and Value

- #108 UI/製品 session 実装者が `syn.probe.compare` を
  `OpName` 経由で呼び、2 semantic probe point 間の整列
  品質を取得できる。
- session/app 層が loop wrap・loop 領域変更を probe clock
  epoch の不連続として自動宣言し、loop 跨ぎ compare が
  `Aligned` を偽装しない fail-closed 証拠を得られる。

## Scope

### In Scope

- `ui/src/ipc.ts`: `OpName` union へ `"syn.probe.compare"`
  追加。`ui/src/store.ts` は必要時のみ最小追従。
- `crates/synvelis-player/src/routing_control.rs`:
  不連続検出 accessor + `observe_block`/`observe_node_block`
  内の検出配線（`WrapSim::walk_window` 戻り値の利用・
  `SetLoop`/`ClearLoop` apply 検出）。
- `crates/synvelis-app/src/engine.rs`:
  `SessionRoutingEngine::run` の per-block 観測→検出→宣言
  結線（PumpPolicy cadence の等価維持・`PlayReport` の
  merge 等価維持を含む）。
- `crates/synvelis-player/tests/` 新規 test file 1 本
  （`sf04_loop_discontinuity.rs`）。
- `docs/` 文書チェーン（slug `sf-04-loop-discontinuity`:
  spec/design/plan/task/test-spec/impl-ir/impl-exp）。
- `docs/specs/sf-04-slice8-probe-faults-declare.md`・
  `docs/test-specs/sf-04-slice8-probe-faults-declare.md` の
  deferred 解消記録。
- `evidence/sv107/` 検証記録。
- docs index の generator 再生成。

### Out of Scope

- `crates/synvelis-app/tests/`（sv#117 占有）・
  `crates/synvelis-app/src/monitoring.rs`/`ui_ops.rs`/
  `service.rs`（sv#117 占有 — `op_probe_*`・dispatch・
  `OpInfo` は実装済みで変更不要）。
- `crates/synvelis-player/src/` の `routing_control.rs`
  以外（`probe.rs`/`routing_engine.rs`/`driver.rs`/
  `process_boundary.rs` 等 — 宣言機構・occurrence/apply
  検出に必要な API は実装済み）。
- per-node `DispatchTraceAdapter` 実登録（SF-05・#108 系）。
- SF-PROBE-07 の Seek 脚（slice3/8 で実装済み）・Device
  Restart 脚（engine 再生成 = fresh probe context で宣言点
  不要の既存契約）。
- node transport `Seek` command の probe 宣言（session
  transport seek は `RoutingEngine::seek` が自前で宣言済み。
  node-level transport command 経路の seek は boundary
  transport 基盤の将来要件として design doc に記録）。
- SF-PROBE-08/09・SF04-RCP-01・実機/plugin/3OS・#108 UI・
  #109 oracle（外部 gate）。
- `ProbeClockMark`/`ProbeTimeContext` への occurrence
  フィールド追加（occurrence は wrap 検出の内部手がかりで
  あり wire context の語彙ではない）。
- `*-msrv` 系検証・`mise.toml` 変更・Issue auto-close
  （`Refs #107` のみ）。

## Requirements

- `syn.probe.compare` が `ui/src/ipc.ts` の `OpName`
  union に含まれ、`mise run check-ui`/`test-ui` が green。
- loop wrap 発生ごとおよび `SetLoop`/`ClearLoop` apply
  時に probe discontinuity が宣言される（`bump_clock_epoch`
  による epoch cut）。
- wrap/apply 境界を跨ぐ compare が `Aligned` のまま残らず
  `EpochMismatch`/`DiscontinuityInWindow`/`Incomparable`
  で fail-closed することを player 層 fixture で pin。
- 宣言は engine cursor・PCM・seek を動かさない
  （cursor 非移動を fixture で pin）。
- 宣言失敗（`CommandChannelFull`）でも fail-closed —
  epoch は進み deck との乖離が観測可能。
- 検出は `observe_block` の post-window 状態から読み、
  `boundary_loop_wraps` の pre-window pure read とは
  混同しない（両者の呼出し順序契約を維持）。
- `SessionRoutingEngine::run` の外部契約（`PlayReport`
  の pcm/crc/frames・pump cadence・error 伝播・
  `pending_observe_cycles` の drain 意味）は不変。

## Acceptance Criteria

- [ ] `ui/src/ipc.ts` `OpName` union に
  `"syn.probe.compare"` が含まれ、`mise run check-ui`/
  `test-ui` が green（service 側 op との wire 一致）。
- [ ] `RoutingControlDispatch` の検出 accessor が、
  `observe_block` が wrap または `SetLoop`/`ClearLoop`
  apply を観測した block でのみ true を返す
  （generation mismatch・非該当 block では false）。
- [ ] `SessionRoutingEngine::run` が block ごとに検出→
  宣言を行い、wrap/apply を含む block の次の block から
  新 epoch 証拠が流れる（run 末尾への遅延宣言がない）。
- [ ] player 層新規 fixture が以下を pin:
  (i) wrap 跨ぎ compare が fail-closed、
  (ii) `SetLoop`/`ClearLoop` apply でも宣言が起きる、
  (iii) 宣言が cursor・PCM・seek を動かさない、
  (iv) `CommandChannelFull` でも fail-closed
  （epoch 進行 + deck 乖離の観測可能性）。
- [ ] `SessionRoutingEngine::run` の `PlayReport`・
  pump cadence・`pending_observe_cycles` 意味が
  変更前と等価（既存 app/player test が green）。
- [ ] slice8 spec/test-spec の deferred「loop transport」
  行が本 slice での解消として更新される。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`check synvelis-app`/`lint synvelis-app`/
  `check-ui`/`test-ui`/`prek` が green。
- [ ] 外部 gate（実機・#108 UI・#109 oracle・app op
  統合の残件・SF-PROBE-08/09・consumer 照合・MSRV
  レーン）は NotRun として記録され、Pass 扱いしない。

## Deferred Design Concerns

- node transport `Seek` command の probe 宣言 | session
  transport seek は `RoutingEngine::seek` が cursor
  re-anchor + epoch bump を自前で行う。node-level
  transport command 経路（`boundary_stage_window`）の
  `Seek` は TIM-03 boundary transport 基盤の一部で、
  probe 配線は別 slice の要件確定待ち | 要件発生時 |
  本 slice は coordinator 確定の loop wrap +
  `SetLoop`/`ClearLoop` のみに限定。
- epoch cut の block 粒度 | 宣言は observe_block 後の
  block 境界で行われ、wrap/apply を含む block の記録は
  旧 epoch を保持する。chunk 粒度の証拠では block 内
  部の sub-block 精度は表現しない — compare が fail-
  closed になるのは旧/新 epoch 証拠を跨ぐ window であり、
  wrap を含む block 単体の compare は chunk 不可分性の
  範囲で Aligned になりうる（seek 宣言と同じ粒度契約）|
  より細かい粒度が要件化した場合 | probe chunk が
  block 単位である現行契約では block 境界 cut が正直な
  精度。
- `syn.probe.compare` の UI consumer 実装 | #108 系 slice
  | OpName union 追加は wire 到達性のみを保証し、
  呼出し側の実装は含まない。
- 実機・#109 oracle・#108 UI・SF-PROBE-08/09・
  SF04-RCP-01 | 外部 gate | NotRun。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/107>
  （SF-PROBE-07 Loop 脚・`syn.probe.compare` UI wire）。
- 前提: SF-04 slice3（seek → `bump_clock_epoch`・
  `ClockEpoch` cut）、slice6（`compare_probes`/
  `ProbeCoverage`）、slice8（`declare_probe_discontinuity`
  — PR merge 済み）、slice10（`syn.probe.compare` app
  op — service 側実装済み）、TIM-03（sv#127 `f47e024`:
  `LoopOccurrence`/`WrapSim`/`boundary_loop_wraps`/
  `ctx.occurrence`）。
- 本 spec は Issue #107 のうち `syn.probe.compare` UI
  wire OpName と SF-PROBE-07 Loop 脚の 2 deliverable
  のみを対象とし、Issue は Open 継続が前提。
