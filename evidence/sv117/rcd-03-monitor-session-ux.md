# RCD-03 Monitor Session UX — Loop/PunchInLoop + count-in/pre-roll Evidence Ledger (synvelis#117)

Headless fixture evidence for the `synvelis-app` slice of
Issue #117 (Monitoring / Punch / Latency UX — Auto標準・Raw独立・
Direct監視): session-scope の loop 宣言と count-in/pre-roll 宣言を
`syn.monitoring.session` start params で受け付け、
`GateSpec::Loop`/`GateSpec::PunchInLoop` の plan 構築と
`TransportPhase::CountIn`/`PreRoll` の位相投影を pin する。
Every row records an independently observed Expected/Observed pair
from `crates/synvelis-app/tests/monitor_arm.rs`（E8 fixture、
新規 6 test / 既存 14 test 非回帰）。`Pass` here means *the stated
headless/fake-fixture lane only* — it never implies real device
capture, real monitor output, real GUI, RT-thread measurement,
monitor bus PCM routing, or OS-level acceptance. Those lanes are
`NotRun`/`Deferred` with reasons, per `POLICIES.md`.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis branch | `feat/117-monitor-session-ux` |
| OS | Windows (win32) |
| Toolchain | rustc via `mise.toml`/`mise.ci.toml` pin（実測値は PR 本文に記録） |
| Fixture | `fixture_app()`（headless `SynvelisApp` + in-memory project）→ `syn.monitoring.*` op dispatch → `syn.monitoring.state` 投影。gate/route/phase 意味論は `synvelis-recording` の `preflight`/`GateScheduler`/`MonitorProbe`/`decide` が所有（app は宣言検証と投影のみ） |
| Session shape | 120 BPM / 960 PPQN / 48 kHz（1 tick = 25 frame）。loop `[240,480)` tick = frame `[6000,12000)`、stop 960 tick = 24000 frame → 3 pass。punch `[300,420)` tick = frame `[7500,10500)`（loop 内）。count-in `[-480,-240)`・pre-roll `[-960,-480)` tick（`transport_start` = 0 より前 = session 開始前位相） |
| Lane | headless unit/integration test のみ（`mise run test synvelis-app`、実機・GUI lane なし） |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Fail` = measured failure; `NotRun` = lane exists but was not
measured here; `Unsupported` = capability genuinely unavailable;
`Deferred` = intentionally out of scope for this slice.

## RCD-03-MSU-01 — Loop session の gate 種別・マーク・pass 構造投影

Fixture: `e8_loop_session_projects_gate_and_pass_segments`
（`loop_start=240`/`loop_end=480` 宣言、punch 未宣言、
arm/on + auto + ready の宣言 track 1 件、play 後に state 観測
+ seek による live route 確認）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| gate 種別 | `session.gate == "loop"`、`loop_start == 240`、`loop_end == 480`、`punch_in`/`punch_out` は `null` | 一致 | Pass |
| phase | playing 中 `session.phase == "playing"`（loop gate に punch 位相は無い） | 一致 | Pass |
| pass 構造 | segments が `[0,6000)` gate 閉 → `[6000,24000)` gate 開に merge（3 pass 連続で併合）、`truncated == false` | 一致 | Pass |
| loop 前 route | tick 100（loop 前）seek → `route == "playback"`（gate 閉） | 一致 | Pass |
| loop 内 route | tick 300・tick 900（複数 pass に跨る位置）seek → `route == "input"`（gate 開） | 一致 | Pass |

## RCD-03-MSU-02 — PunchInLoop session の pass ごとの punch 区間

Fixture: `e8_punch_in_loop_session_projects_per_pass_punch`
（authored `project.monitoring.punch` = `[300,420)` を loop
`[240,480)` 内に設定して session 開始）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| gate 種別 | `session.gate == "punch_in_loop"`、loop/punch マーク値が全て投影される | 一致 | Pass |
| pass ごとの punch 区間 | `gate_open == true` の segment が pass ごとに `[7500,10500)` `[13500,16500)` `[19500,22500)` の 3 区間のみ（loop pass 長 6000 frame で反復） | 一致 | Pass |
| live route（punch 内） | tick 360 seek → `route == "input"` | 一致 | Pass |
| live route（loop 内 punch 外） | tick 250 seek（loop 内だが punch 外）→ `route == "playback"`（gate 閉 — loop 全体が開くわけではない） | 一致 | Pass |

## RCD-03-MSU-03 — authored punch が loop 外 → fail-closed 拒否

Fixture: `e8_punch_outside_loop_is_rejected`
（punch `[600,700)` が loop `[240,480)` の外）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| session 開始 | `preflight` が拒否（error message が `loop` を含む）、`session.active == false` のまま | 一致 | Pass |

## RCD-03-MSU-04 — count-in/pre-roll 宣言の位相投影と gate 閉鎖

Fixture: `e8_count_in_pre_roll_phases_keep_gate_closed`
（pre-roll `[-960,-480)`・count-in `[-480,-240)` 宣言、
`transport_start = 0`、Normal gate、armed Auto track）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| 宣言投影・初期位置 | `pre_roll_*`/`count_in_*` が session json に投影、`session.position == -960`（宣言区間の先頭から開始） | 一致 | Pass |
| pre-roll 位相 | play 直後 `phase == "pre_roll"`、`position_frame < 0`、`in_session == false`、armed Auto route は `playback`（gate 閉 — 隠れた PCM を録音しない） | 一致 | Pass |
| count-in 位相 | tick -300 seek → `phase == "count_in"`、`in_session == false`、route `playback` | 一致 | Pass |
| 宣言外・session 前の隙間 | tick -100 seek（pre-roll/count-in でも transport_start でもない）→ `phase == "playing"` だが `in_session == false`・route `playback`（位相語彙と gate 開閉は独立） | 一致 | Pass |
| transport_start 到達 | tick 0 seek → `in_session == true`、Normal gate 開、route `input` | 一致 | Pass |
| pause | count-in 区間で pause → `transport == "paused"`、`phase == "stopped"`（`decide` 語彙に Paused は無く gate 閉の停止中として写像） | 一致 | Pass |

## RCD-03-MSU-05 — 宣言境界の fail-closed 検証

Fixture: `e8_declaration_boundaries_are_rejected`
（5 種の不正宣言を個別に start へ投入）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| `loop_start < transport_start` | 負 session frame を生じるため拒否（error が `loop_start` を含む） | 一致 | Pass |
| `count_in_end > transport_start` | session 開始前位相の宣言契約違反で拒否 | 一致 | Pass |
| 空区間（`pre_roll_end == pre_roll_start`） | `end <= start` で拒否 | 一致 | Pass |
| `pre_roll` が `count_in` に食い込む重複 | `pre_roll.end <= count_in.start` 違反で拒否 | 一致 | Pass |
| 対の片方のみ（`loop_start` のみ） | op 層 `opt_tick_range` が拒否（error が `loop_start` を含む） | 一致 | Pass |
| 全拒否後の状態 | `session.active == false` のまま（部分的に開始されない） | 一致 | Pass |

## RCD-03-MSU-06 — session-scope 宣言の非永続性

Fixture: `e8_session_scope_declarations_do_not_touch_project`
（loop + count-in 宣言で start → revision・`project.monitoring`
を観測 → stop）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| project 非消費 | session start で project revision が不変、`project.monitoring.punch` は `None` のまま（宣言は session-scope で authored punch を書き換えない） | 一致 | Pass |
| session stop | `session.active == false`、`session.gate == null`（宣言は session と共に破棄 — 永続化経路を持たない） | 一致 | Pass |

## Deferred / NotRun / Unsupported

| Lane | Status | Reason / Follow-up |
| --- | --- | --- |
| authored loop マークの project 永続化 | `Deferred` | `MonitoringData`（`tracks` + `punch` のみ）に loop フィールドが無く schema bump + migration が必要 — Issue 変更許可範囲外のため session-scope 宣言のみ。本 slice は session-scope で受入を満たす |
| RT graph への monitor node 搭載・`MonitorEngine` の app 組み込み | `Deferred` | monitor bus PCM 駆動は別 slice（`docs/impl/ir/rcd-03-monitor-engine.md` の Follow-up） |
| `RoutingEngine` `EngineBlocks` 経路の PCM 内容 pin | `Deferred` | 同上 — headless state/route 投影のみ検証 |
| 実 device monitor/latency 受入・実 GUI 受入・physical Direct Monitor | `Deferred` | 仮想環境で保証不可・実機 Pass を主張しない（Issue 受入条件どおり） |
| Windows GUI-L2 / EXT-L3（RF #628/#629/#631 系） | `NotRun` | 仮想 Endpoint・実 GUI は本 slice 範囲外。Headless のみで OS L3 を Pass にしない |
| macOS/Linux L1（Native Build/Link・Headless/RT） | `NotRun` | merge 後の対象 SHA の CI 証拠は Coordinator が確認。Worker は Windows ローカル検証のみ |
| `mise run test-msrv` | `NotRun` | ユーザー指定どおり Daily CI（daily-ci→ci.yml）へ委譲 |
| 実 transport タイミング精度 | `NotRun` | count-in/pre-roll 位相は宣言区間と位置の比較による投影。実機の transport 進行との同期精度は headless 範囲外 |
