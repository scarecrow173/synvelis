---
id: "TSPEC-034X1GK6Bkure6mY3kxHZJ"
type: "test-spec"
status: "approved"
title: "RCD-03 monitor/punch fake session fixture test spec"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added: []
    modified: []
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
    - "docs/test-specs/rcd-03-monitor-engine.md"
    - "docs/test-specs/rcd-03-monitor-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-03-monitor-punch-design.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-03-monitor-punch.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  defers: []
  deferred-by: []
---

# RCD-03 monitor/punch fake session fixture test spec

## Purpose

`DeviceRig` + `MonitorProbe` 合成の Fake Session fixture が
保証すべき monitor/punch の session・staging・commit 層契約を
固定する。実 device・実 GUI 受入の代替ではなく、crate 内契約の
fixture 検証である。monitor/arm が `InputTapSpec.binding` の
metadata であり capture path に入らない構造を、session 全体を
通した観測可能な振る舞いとして pin する。

## Feature

Fake recording sessions driven through the real
`InputSession`/`StreamWriter`/`StagingArea` path produce
monitor route regions, fault behavior, verified prefixes, and
commit payloads that are provably independent of the declared
`MonitorState`.

## Rules

- Fake Session fixture は `DeviceRig` と `MonitorProbe` を同一
  block 刻み・同一 frame 数で駆動し、両者の session 位置を
  1:1 に保つ。`MonitorProbe` は capture 側と同一内容の別
  `PreparedRecordingPlan` instance を借用する（plan は非
  Clone・所有権競合のため）。
- `TransportPhase` は block 境界でのみ供給し、block 内相変化は
  対象外。coherence 主張は clean 駆動経路に限定する。
- monitor route（Input/Playback/Muted）の区間は `decide` 規則
  どおりであり、`gate_open` 区間は `evaluate_gate` が返す
  session-frame 精度の区間と一致する。gate ロジックの第二実装
  を fixture に持ち込まない。
- `MonitorState` を Off/Auto/On で切替えても、capture される
  raw media bytes・journal span・PCM 内容・fault bits・
  verified prefix は同一である。
- `MonitorState::On` + `input_output_ready` では route Input が
  gate 閉区域間にも及ぶが、capture は gate-open 区間のみに
  留まる（Raw 独立性）。
- device frame gap・disconnect は monitor 状態に依らず
  fail-stop し、verified prefix（session 0 からの連続 span）と
  capture-fault marker を残す。`build_payload` は当該 stream を
  `capture-fault:` 理由で recoverable 分離する。
- punch gate-open 区間内部での device loss 後、新規 session
  （回復 device 相当の新規 `FakeInputDevice` + 新規 staging +
  新規 session id）は clean に完了し commit payload を構成
  できる。旧 session の staging/prefix は変更されない。
- 2 つの独立 session（別 device・別 staging・別 session id）
  の fault は相互に漏れない。`FaultLatch` は session 共有で
  あり session 間分離は構造的契約として pin する。
- commit payload の segment session 範囲・pass 構成・take
  参照は monitor 状態に依らず gate-open 区間を反映する。
- すべて headless・deterministic・bounded であり、RT callback
  内の確保・blocking を導入しない。

## Examples

- Given `GateSpec::Punch` [12,28) tick（session frame
  [300,700)）・`MonitorState::Auto`・Arm On・
  `TransportPhase::Playing`、When Fake Session を 1000 frame
  駆動する、Then probe の gate_open 区間は [300,700) に merge
  され、route Input 区間は [300,700)、Playback は残り、
  journal span の session 被覆は [300,700) と一致する。
- Given 同一 punch session を `MonitorState::On` +
  `input_output_ready` で駆動、When 完了する、Then route
  Input は全域 [0,1000) に及ぶが capture/journal/media は
  [300,700) のみであり、Off/Auto/On で media bytes と
  `span_obs` が一致する。
- Given `MonitorState` ∈ {Off, Auto, On} の punch session、
  When frame 600 通過後に `jump_to(+777)` で gap を注入する、
  Then 3 状態すべて `FAULT_INPUT_DISCONTINUITY` で fail-stop
  し、verified prefix・fault bits・`span_obs` が一致する。
- Given punch gate-open 内部（frame 500）で
  `DeviceNotFound` を raise した session、When finish と
  `build_payload` を実行する、Then stream は `capture-fault:`
  で recoverable 分離され `payload.streams`/`passes` は空、
  新規 session は Complete finalize で segments が [300,700)
  を cover する commit に成功する。
- Given Monitor On の session A と Monitor Off の session B
  を並行駆動し A に frame gap を注入、When B を完走させる、
  Then B は Complete finalize・[300,700) 被覆・PCM verified・
  commit 成功であり、A の fault は B の area/session/payload
  に現れない。

## Guarantees

- Fake Session fixture が `synvelis-recording` の test として
  存在し、上記 Examples をすべて検証して pass する。
- monitor/punch の route・fault・recovery・分離・commit 契約が
  `decide`/`GateScheduler`/`evaluate_gate` の既存権威への
  副作用なしに session 層で pin される。
- fake/headless Pass と実機・実 GUI の NotRun/Deferred が
  証拠文書と PR 本文で区別される。

## Non-goals

- 実 device capture/monitor・実機 latency・実 GUI・実 plugin
  の受入（L2/L3・別 gate）。
- in-session device resume・faulted session の継続録音。
- 実 GUI の 2Window/Focus・app Command/状態投影配線
  （`synvelis-app` 範囲）。
- `synvelis-player` の monitor PCM bus の session 統合
  （player 範囲）。
- block 内の phase/arm/monitor 相変化。
- Loop/PunchInLoop・tempo・mute/solo の網羅行列
  （`monitor_gate.rs`/`monitor_engine.rs` が担う）。

## Risk

- fixture の route/capture 一致を実機 Pass と誤認する →
  test-spec・evidence・PR 本文で fake/headless 検証であること
  を明記する。
- 2 plan 構成で capture 側と monitor 側の gate schedule が
  乖離する → 両 plan は同一 spec の決定的 `preflight` であり、
  probe の `gate_open` 区間が journal span 被覆と一致すること
  を test で直接検証する。
- fail-stop 系 test で coherence を誤って主張する →
  coherence 検証は clean 経路の test に限定し、fault 系は
  fault bits・prefix・commit 分類のみを pin する。
