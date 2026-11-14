---
id: "IMPL-034XD06AYOPMbFGpbS8g2a"
type: "impl"
status: "completed"
title: "rcd-03-monitor-session-ux"
created: "2026-11-14"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-03-monitor-session-ux.md"
      - type: "file"
        path: "docs/designs/rcd-03-monitor-session-ux.md"
      - type: "file"
        path: "docs/plans/rcd-03-monitor-session-ux.md"
      - type: "file"
        path: "docs/tasks/rcd-03-monitor-session-ux.md"
      - type: "file"
        path: "docs/test-specs/rcd-03-monitor-session-ux.md"
      - type: "file"
        path: "docs/impl/ir/rcd-03-monitor-session-ux.md"
      - type: "file"
        path: "evidence/sv117/rcd-03-monitor-session-ux.md"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/monitoring.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/monitor_arm.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/rcd-03-monitor-session-ux.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-03-monitor-session-ux.md"
    - "docs/specs/rcd-03-monitor-session-ux.md"
    - "docs/impl/ir/rcd-03-monitor-engine.md"
    - "docs/impl/ir/rcd-03-app-arm-monitoring-ux-slice.md"
    - "docs/impl/ir/rcd-03-monitor-punch.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/rcd-03-monitor-punch.md"
    rejected: []
---

# rcd-03-monitor-session-ux


## Summary

`syn.monitoring.session` の start params に session-scope の
loop 宣言と count-in/pre-roll 宣言を追加し、
`GateSpec::Loop`/`GateSpec::PunchInLoop` の plan 構築と
`TransportPhase::CountIn`/`PreRoll` の位相投影を
`synvelis-app` で実現する（Issue #117 残 headless slice）。
route/gate/phase の意味論は `synvelis-recording` の
`decide`/`GateScheduler`/`MonitorProbe`/`preflight` に委譲し、
app は宣言の境界検証・plan 構築・投影のみを担う。

## Context

- Task: `docs/tasks/rcd-03-monitor-session-ux.md`
- Plan: `docs/plans/rcd-03-monitor-session-ux.md`
- Spec: `docs/specs/rcd-03-monitor-session-ux.md`
- Design: `docs/designs/rcd-03-monitor-session-ux.md`（D1–D5）
- Test spec: `docs/test-specs/rcd-03-monitor-session-ux.md`
- Issue: `https://github.com/scarecrow173/synvelis/issues/117`

## Implementation

### Changes

- `crates/synvelis-app/src/monitoring.rs`
  - `MonitorSessionParams` に `loop_marks`・`count_in`・
    `pre_roll`（いずれも `Option<(MusicalTick, MusicalTick)>`）
    を追加。`MonitorSessionState` に `count_in`/`pre_roll` を保持
    （session 開始時に凍結 — 稼働中の project 変更で投影が
    gate と乖離しない）。
  - `start_session`: `session_declarations` で宣言を検証後、
    gate を `loop + authored punch → PunchInLoop`、
    `loop のみ → Loop`、`punch のみ → Punch`、
    `それ以外 → Normal` の優先順で構築し `preflight` へ渡す。
    punch の正本は `project.monitoring.punch` のみ
    （session-scope の別 punch 宣言は持たない — design D1）。
    初期 `position` は宣言された roll 区間の最小 start
    （無ければ `transport_start`）。
  - `session_declarations`/`roll_range` helper:
    `loop_start >= transport_start`（負 session frame は
    `GateScheduler` が構成できない）、roll 区間の非空・
    `end <= transport_start`・`pre_roll.end <= count_in.start`
    を app 境界で拒否。gate 区間の形状検証は `preflight` に
    委譲（二重実装しない）。
  - `transport_phase` helper を新設し、`session_json` の
    `phase` 投影と `live_routes` の `decide` 呼出しが同一導出を
    共有する。playing 中は count-in/pre-roll 宣言区間と
    plan 凍結 punch マークとの位置比較で
    `CountIn`/`PreRoll`/`PunchWait`/`PunchOpen`/`PostRoll`/
    `Playing` を導き、非 playing（paused 含む）は `Stopped`。
  - `session_json`: gate 名 `"loop"`/`"punch_in_loop"` と
    `loop_start`/`loop_end`/`count_in_*`/`pre_roll_*` マーク値を
    投影。inactive session も同じキーを `null` で返す。
  - `probe_segments` は feed 位相 `Playing` 固定を維持 —
    probe の駆動域は session frame `[0, session_end)` であり
    count-in/pre-roll 宣言はその外。`decide` 上 punch 系位相も
    `Stopped` 以外で同一視されるため segment 出力に差は出ない
    （根拠を doc comment に明記）。
- `crates/synvelis-app/src/ui_ops.rs`
  - `op_monitoring_session` start params に
    `loop_start`/`loop_end`・`count_in_start`/`count_in_end`・
    `pre_roll_start`/`pre_roll_end` を追加。
    `opt_tick_range` helper が対の tick 区間として読み、
    片方のみの指定は `Err`（不完全な宣言を黙って受理しない）。
- `crates/synvelis-app/src/service.rs`
  - `syn.monitoring.session` の `OpInfo.summary` を宣言対応に
    更新（op 追加・dispatch 変更なし — 既存 op の params 拡張）。
- `crates/synvelis-app/tests/monitor_arm.rs`
  - E8 fixture 6 test を追加（下記 Validation）。既存 E1–E7 は
    非回帰で維持。

### Decisions During Implementation

- 宣言経路は `syn.monitoring.session` start params 拡張
  （design D1）。新 op は追加せず、session active 中の宣言
  変更は既存の `require_no_session` 契約で拒否される
  （gate は開始時に plan へ凍結）。
- punch マークの正本は authored `project.monitoring.punch`
  のみ。design 初版は session punch params も検討したが、
  単一の正本を保つため session-scope punch 宣言は採用しない
  （D1 の確定）。
- `loop_start < transport_start` は負の `SessionFrame` で
  scheduler が fault するため app 境界で早期拒否（design D2）。
- count-in/pre-roll 区間の gate 閉鎖は `GateScheduler`/
  `evaluate_gate` 由来 — 宣言区間は session frame 域
  `[0, session_end)` の外にあり `in_session == false` →
  `gate_open == false` となる構造を fixture で pin し、
  app 側で gate を再実装しない。
- 位相導出を `transport_phase` に集約し `session_json` と
  `live_routes` で共有 — 表示位相と `decide` 入力位相の
  乖離を構造的に防ぐ（design D4）。
- `SessionDeclarations` struct で検証済み宣言を束ね、
  `start_session` の複雑度（clippy `too_many_lines`）と
  戻り値型（`type_complexity`）を両立させた。

## Related Experiments

### Adopted

- `docs/impl/exp/rcd-03-monitor-punch.md` — `PreparedRecordingPlan`
  は non-Clone、`MonitorProbe` 借用と mutable capture が競合する
  ため同一 `RecordingSpec` から `preflight` を二度呼び独立 plan を
  得る方針を踏襲。本 slice も `preflight` を一度だけ呼び
  plan を `MonitorSessionState` に保持する既存構造を維持。

### Rejected

- （なし — アプローチは先行 slice で確立済み。session 宣言の
  境界検証と位相導出は design doc の決定に従い実装）

## Validation

すべて `G:\orca\workspaces\synvelis\sv117-monitorux`
（branch `feat/117-monitor-session-ux`）、Windows、
rustc は `mise.toml`/`mise.ci.toml` pin で実行。

| 検証 | コマンド | 結果 |
| --- | --- | --- |
| format | `mise run fmt-check` | Pass（exit 0） |
| check | `mise run check synvelis-app` | Pass（exit 0） |
| lint | `mise run lint synvelis-app` | Pass（exit 0、`-D warnings`） |
| test | `mise run test synvelis-app` | Pass（exit 0、全 test binary 緑） |
| build | `mise run build synvelis-app` | Pass（exit 0） |
| RT | `mise run test-rt` | Pass（exit 0） |
| native check | `mise run check-app-native` | Pass（exit 0） |
| native lint | `mise run lint-app-native` | Pass（exit 0、`--features native -D warnings`） |
| 非回帰（補助証拠: cargo 直接実行） | `cargo test --locked -p synvelis-recording --test monitor_gate --test monitor_punch_session` | Pass（20 + 4 test） |
| 非回帰（補助証拠: cargo 直接実行） | `cargo test --locked -p synvelis-player --test monitor_engine` | Pass（10 test） |
| MSRV | `mise run test-msrv` | NotRun — ユーザー指定どおり Daily CI 委譲 |

E8 fixture（`monitor_arm.rs`、全 20 test 中の新規 6 件）:

- `e8_loop_session_projects_gate_and_pass_segments` —
  `session.gate == "loop"`、loop マーク値、pass ごとの
  `gate_open` 区間が投影される。
- `e8_punch_in_loop_session_projects_per_pass_punch` —
  `session.gate == "punch_in_loop"`、pass ごとの punch 区間のみ
  `gate_open`。
- `e8_punch_outside_loop_is_rejected` — authored punch が loop 外
  → `preflight` 拒否で session 非開始（fail-closed）。
- `e8_count_in_pre_roll_phases_keep_gate_closed` — 宣言区間で
  `phase == "count_in"`/`"pre_roll"`、`in_session == false`、
  `gate_open == false`、armed Auto route は `Playback`。
- `e8_declaration_boundaries_are_rejected` — `loop_start <
  transport_start`、roll 区間が `transport_start` 以降、空区間、
  `pre_roll`/`count_in` 重複、対の片方のみを全て拒否。
- `e8_session_scope_declarations_do_not_touch_project` — 宣言は
  project revision・`project.monitoring` を消費せず、session
  stop で消える（非永続）。

## Risks

- `loop_start < transport_start` の負 SessionFrame fault は
  境界検証で早期拒否するが、`preflight` 側の正規化との差異は
  recording crate の契約に依存する。
- count-in/pre-roll の位相投影は宣言区間と現在位置の比較であり、
  実 transport のタイミング精度は session-scope の headless 範囲外。
- Loop 系 gate の punch 位相は project tick の位置比較のみで
  導出する — pass ごとの実 gate 開閉は `GateScheduler` が担い、
  位相語彙は transport 位置の表示に留まる（`gate_open` が
  実際の録音区間を正直に運ぶ）。

## Follow-ups

- authored loop マークの project 永続化（`MonitoringData` schema
  bump + migration）は Issue 変更許可範囲外のため別 slice。
- RT graph monitor node・`MonitorEngine` の app 組み込み、実
  device/実 GUI/physical Direct Monitor 受入は後続 slice/gate。
- `docs/impl/exp/rcd-03-monitor-session-ux.md` は未作成 —
  アプローチが先行 slice で確立済みのため exp は optional と判断。
