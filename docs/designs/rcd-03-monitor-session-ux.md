---
id: "DESIGN-034XCSxsFES1gDT3HrKxUY"
type: "design"
status: "approved"
title: "RCD-03 monitor session UX design — Loop/PunchInLoop + count-in/pre-roll 宣言"
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
  implements:
    - "SPEC-034XCSmeXcGanytWaJhPum"
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

# RCD-03 monitor session UX design — Loop/PunchInLoop + count-in/pre-roll 宣言

## Overview

`synvelis-app` の monitor session 宣言経路を拡張する。session
開始 params に session-scope の loop マーク・punch マーク・
count-in/pre-roll 宣言を追加し、`MonitorUxState::start_session`
が `GateSpec::Loop`/`GateSpec::PunchInLoop` を構築できるように
する。位相投影・route 決定・segment probe は recording crate の
`GateScheduler`/`decide`/`MonitorProbe` をそのまま消費し、app
で gate 意味論を再実装しない。

## Design Decisions

### D1: 宣言経路は `syn.monitoring.session` start params 拡張

session-scope の宣言は session 開始時に plan へ凍結される契約
（稼働中の undo/変更で投影が gate と乖離しない）に整合するため、
新 op や `syn.monitoring.punch` 拡張ではなく
`op_monitoring_session` の `start` params に追加する。

追加 params（すべて `MusicalTick` 値、tick 単位）:

- `loop_start` / `loop_end`: 省略時は loop 宣言なし。
  両方指定時のみ `GateSpec::Loop` または `GateSpec::PunchInLoop`。
- `punch_in` / `punch_out`: session-scope の punch 宣言。
  `project.monitoring.punch` の authored マークを override せず、
  session params で明示された場合のみ session gate に使う。
  loop 宣言がある場合は `GateSpec::PunchInLoop`（punch は loop
  内に含まれる必要あり — `preflight` が検証）。loop 宣言がなく
  punch 両端がある場合は従来どおり `GateSpec::Punch`。
- `count_in_start` / `count_in_end` / `pre_roll_start` /
  `pre_roll_end`: count-in/pre-roll 宣言区間（tick）。省略時は
  宣言なし。`transport_start` より前の tick 範囲を指す。

`MonitorSessionParams` に `loop_marks: Option<(MusicalTick,
MusicalTick)>`、`session_punch: Option<(MusicalTick, MusicalTick)>`、
`count_in: Option<(MusicalTick, MusicalTick)>`、
`pre_roll: Option<(MusicalTick, MusicalTick)>` を追加する。

### D2: gate 構築の優先順位

`start_session` の gate 選択:

1. `loop_marks` あり + `session_punch` 両端あり →
   `GateSpec::PunchInLoop { loop_start, loop_end, punch_in,
   punch_out }`
2. `loop_marks` あり → `GateSpec::Loop { loop_start, loop_end }`
3. `session_punch` 両端あり → `GateSpec::Punch { punch_in,
   punch_out }`
4. `project.monitoring.punch` 両端あり（session_punch 未指定時）→
   `GateSpec::Punch`
5. それ以外 → `GateSpec::Normal { open: transport_start }`

`preflight` が loop/punch の半開 `[in,out)` 正規化・検証を担う。
app は `loop_start >= transport_start` を境界で検証する
（`SessionFrame` は非負で、loop_start < transport_start は
scheduler が負の session frame を構成できず fault するため、
明確なエラーで早期拒否する）。

### D3: count-in/pre-roll の位相投影

`MonitorSessionState` に `count_in`/`pre_roll` の宣言区間
（tick 範囲）を保持する。位相導出は session 内の現在 tick 位置
（`state.position`）と宣言区間の比較で行う:

- `playing` かつ position が count_in 区間内 → `CountIn`
- `playing` かつ position が pre_roll 区間内 → `PreRoll`
- それ以外は従来の punch 位相導出（Punch/PunchInLoop の
  マーク集合）→ `Playing`
- 非 playing → `Stopped`（paused 含む、従来どおり）

count-in/pre-roll 区間は `GateSpec` の open 区間外（session
開始 tick より前）なので、`GateScheduler`/`evaluate_gate` は
その区間で gate を閉じる。app は gate を再実装せず、fixture で
`gate_open == false` を pin する。

### D4: `probe_segments` の feed phase

`probe_segments` は `MonitorProbe` に feed する位相を、宣言区間
を考慮した位相導出（D3 と同じ規則）で決定する。count-in/
pre-roll 区間では `TransportPhase::CountIn`/`PreRoll` を feed
し、`decide` が `Stopped` 以外の非停止相として扱いつつ
`gate_open` は scheduler 由来で閉じることを利用する。

> **実装時の修正（2026-11-15）**: `probe_segments` の駆動域は
> session frame `[0, session_end)`（`[transport_start, stop)` の
> tick 区間）であり、count-in/pre-roll 宣言はその外（session
> 開始前の負 session frame）にあるため feed 位相を反映する
> 対象が存在しない。実装は feed 位相 `Playing` 固定を維持し、
> `CountIn`/`PreRoll` は `live_routes` の `decide` 呼出しと
> `session_json` の位相投影（`transport_phase` 共有導出）で
> 反映する。`decide` は `Stopped` 以外の非停止相を同一視する
> ため、feed 位相の違いは segment 出力に現れない（gate 開閉は
> scheduler 由来の `gate_open` が担う）。D4 の意図 — roll 区間で
> `decide` に非停止相を渡し gate 閉を投影する — は
> `live_routes` 経路で達成される。

### D5: `session_json` の投影拡張

`session_json` の gate 分岐に loop/punch マーク値を追加する:

- `"loop"`: `loop_start`/`loop_end` を tick 値で返す
- `"punch_in_loop"`: `loop_start`/`loop_end`/`punch_in`/
  `punch_out` を tick 値で返す
- `"punch"`/`"normal"`: 従来どおり

`phase` は D3 の位相導出結果を返す。`segments` は
`MonitorProbe` が pass 分割を担うため、app は probe 出力を
そのまま投影し pass 構造を偽らない。

## File Map

- `crates/synvelis-app/src/monitoring.rs`: params/state 拡張、
  gate 構築、位相導出、session_json/probe_segments/live_routes
  更新。
- `crates/synvelis-app/src/ui_ops.rs`: `op_monitoring_session`
  の start params パース拡張。
- `crates/synvelis-app/tests/monitor_arm.rs`（または新規 test
  file）: Loop/PunchInLoop/count-in/pre-roll fixture pin。

## Risks

- `loop_start < transport_start` で負の SessionFrame → scheduler
  fault。境界検証で早期拒否する（D2）。
- count-in/pre-roll 宣言が transport_start より後を指す誤用。
  `count_in_end <= transport_start` 等の整合を境界で検証する。
- 既存 Punch/Normal session の投影後退。既存テスト非回帰で担保。
