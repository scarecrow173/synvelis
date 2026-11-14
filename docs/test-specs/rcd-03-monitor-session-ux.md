---
id: "TSPEC-034XCVeJS7zQuQ96ia4I2z"
type: "test-spec"
status: "proposed"
title: "RCD-03 monitor session UX test spec"
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
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/rcd-03-monitor-session-ux.md"
    - "docs/designs/rcd-03-monitor-session-ux.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---
## Purpose

`rcd-03-monitor-session-ux` の spec/design が要求する Loop/PunchInLoop
session 宣言・count-in/pre-roll 位相投影・gate 閉鎖の契約を headless
fixture で pin し、実装が recording crate の gate 意味論を正しく消費
していることを保証する。

## Feature

Monitor session Loop/PunchInLoop + count-in/pre-roll 宣言の投影と gate 閉鎖

## Rules

- Rule: `syn.monitoring.session` start params の `loop_start`/`loop_end`/`punch_in`/`punch_out` 宣言で `GateSpec::Loop`/`GateSpec::PunchInLoop`/`GateSpec::Punch` の plan が開始できる
- Rule: `loop_start < transport_start` は負の SessionFrame を生じるため session 開始を拒否する
- Rule: count-in/pre-roll 宣言区間は `TransportPhase::CountIn`/`PreRoll` として位相投影・`decide` feed・`probe_segments` feed に反映される
- Rule: count-in/pre-roll 区間では monitor gate が開かない（`gate_open == false`）— `GateScheduler`/`evaluate_gate` 由来で app は再実装しない
- Rule: `syn.monitoring.state` の `session.gate` は `"normal"`/`"punch"`/`"loop"`/`"punch_in_loop"` を返し、loop/punch マーク値を tick で投影する
- Rule: `segments` は `MonitorProbe` の pass 分割をそのまま投影し、Loop の各 pass・PunchInLoop の pass ごとの punch 区間を偽らない
- Rule: session active 中の宣言変更は拒否される（gate は開始時に plan へ凍結）
- Rule: count-in/pre-roll/loop 宣言は session-scope であり project persistence に漏れない

## Examples

- Example: Loop session（loop_start=240, loop_end=480, transport_start=0, stop=960）で segments が loop pass ごとの open 区間を投影し、phase が位置に応じて `count_in`/`pre_roll`/`playing` を返す
- Example: PunchInLoop session（loop 240–480, punch_in=300, punch_out=420）で各 pass の punch 区間のみ gate が開き、count-in/pre-roll 区間では `gate_open == false`
- Example: `loop_start < transport_start` の start params はエラーで拒否される
- Example: count-in 宣言（例: -480..0）で position が区間内の間 phase が `count_in`、gate は閉じたまま

## Guarantees

- Loop/PunchInLoop session が `syn.monitoring.session` start で開始できる
- `syn.monitoring.state` が gate 種別・マーク・pass 構造・位相を正直に投影する
- count-in/pre-roll 区間で gate が開かず隠れた PCM を録音しない
- 既存 Punch/Normal session・monitor_arm・monitor_gate・monitor_engine テストが非回帰

## Non-goals

- authored loop マークの project 永続化（schema bump は別 slice）
- RT graph monitor node・MonitorEngine の app 組み込み
- 実 device/実 GUI/physical Direct Monitor の受入
- recording/player の gate 意味論変更

## Risk

Loop/PunchInLoop の pass 構造や count-in/pre-roll の gate 閉鎖を pin しないと、
app が gate を再実装したり虚偽の位相を投影する回帰を検出できない。
