---
id: "SPEC-034XCSmeXcGanytWaJhPum"
type: "spec"
status: "proposed"
title: "RCD-03 monitor session UX — Loop/PunchInLoop + count-in/pre-roll 宣言 slice"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source: []
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  defers: []
  deferred-by: []
---

# RCD-03 monitor session UX — Loop/PunchInLoop + count-in/pre-roll 宣言 slice

## What Should Be Built

Issue #117 の残 headless slice として、`synvelis-app` の monitor
session UX 宣言を拡張し、session-scope で次を実現する。

- `syn.monitoring.session` の start params で session-scope の
  loop マーク（`loop_start`/`loop_end`）と punch マーク
  （`punch_in`/`punch_out`）を宣言でき、`GateSpec::Loop` /
  `GateSpec::PunchInLoop` の plan を開始できる。
- count-in / pre-roll の宣言区間（session 開始位置より前の
  tick 範囲）を session params で宣言でき、transport 位相投影
  （`TransportPhase::CountIn` / `PreRoll`）と `live_routes` の
  `decide` 呼出し、`probe_segments` の feed phase に反映される。
- `syn.monitoring.state` が gate 種別（`"loop"` /
  `"punch_in_loop"`）、loop/punch マーク値、pass 構造を持つ
  segments、位相を正直に投影する。
- count-in / pre-roll 区間で monitor gate が開かない
  （隠れた PCM を録音しない）ことを headless fixture で pin
  する。

本 spec は Issue #117 を Close しない（RT graph monitor node・
実機・実 GUI 受入は後続 slice/gate）。

## Why Now

先行 slice（PR #269 app Arm/Monitoring UX、PR #307 persistence、
MonitorEngine + Loop/PunchInLoop gate）で recording/player 側の
`GateSpec::Loop`/`PunchInLoop` と `TransportPhase::CountIn`/
`PreRoll` 語彙は実装済みだが、app の session 開始経路
（`MonitorUxState::start_session`）は `GateSpec::Punch` /
`GateSpec::Normal` のみ構築し、count-in/pre-roll 宣言経路を
持たない。`session_json` には `"loop"`/`"punch_in_loop"` の
分岐が既にあるが到達不能であり、位相投影は
playing/stopped の二値に留まる。本 slice は宣言経路と投影を
session-scope で接続する。

## Users and Value

- 将来の GUI/RT 統合層: loop/punch-in-loop の monitor session
  宣言と count-in/pre-roll 位相を session-scope で扱える API
  契約を得る。
- 検証者: gate 種別・pass 構造・位相・count-in/pre-roll で
  gate が閉じることを headless fixture で実測できる。

## Scope

### In Scope

- `syn.monitoring.session` start params への session-scope
  loop マーク・punch マーク・count-in/pre-roll 宣言の追加
  （`crates/synvelis-app/src/monitoring.rs`、
  `crates/synvelis-app/src/ui_ops.rs`）。
- `GateSpec::Loop`/`GateSpec::PunchInLoop` の plan 構築と
  `session_json`/`live_routes`/`probe_segments` への投影。
- count-in/pre-roll 宣言の transport 位相投影と
  `decide`/`GateScheduler` への feed。
- `crates/synvelis-app/tests/` への fixture pin 追加
  （既存 `monitor_arm.rs` の拡張または新規 test file）。
- `docs/` の本 doc-chain と `evidence/sv117/` の証拠記録。

### Out of Scope

- `crates/synvelis-project/` の変更（loop マークの authored
  永続化は schema bump + migration が必要で Issue 変更許可
  範囲外）。
- `crates/synvelis-recording/`・`crates/synvelis-player/` の
  変更（`GateSpec`/`decide`/`MonitorProbe`/`MonitorEngine` は
  消費するのみ）。
- RT graph への monitor node 搭載、`MonitorEngine` の app
  組み込み（monitor bus PCM 駆動）。
- 実 device monitor/latency 受入、実 GUI 受入、physical
  Direct Monitor（アナログ経路）。

## Acceptance Criteria

- `syn.monitoring.session` で Loop/PunchInLoop session を
  開始できる（session-scope の loop マーク宣言経路）。
- `syn.monitoring.state` が gate 種別名・loop/punch マーク・
  pass 構造を持つ segments・位相を正直に投影する。
- count-in/pre-roll 宣言が `TransportPhase::CountIn`/`PreRoll`
  の位相投影に反映される。
- count-in/pre-roll 区間で monitor gate が開かないことを
  fixture で pin する。
- 既存 `monitor_arm`（synvelis-app）・`monitor_gate`
  （synvelis-recording）・`monitor_engine`（synvelis-player）
  テストを非回帰で維持する。
- 残 NotRun（実 device / 実 GUI / RT graph monitor node /
  monitor bus PCM 駆動 / physical Direct Monitor）を
  Requirement × Result 直交で PR 本文に記録する。

## Open Questions

なし（session-scope 宣言経路は `syn.monitoring.session` の
start params 拡張で確定。design doc 参照）。
