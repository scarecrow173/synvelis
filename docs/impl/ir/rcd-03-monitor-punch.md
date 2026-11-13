---
id: "IMPL-rcd03-monitor-punch-session"
type: "impl"
status: "completed"
title: "RCD-03 synvelis-recording fake session fixture（Monitor×DeviceRig×Commit）実装記録"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/tests/monitor_punch_session.rs"
      - type: "file"
        path: "docs/specs/rcd-03-monitor-punch.md"
      - type: "file"
        path: "docs/designs/rcd-03-monitor-punch-design.md"
      - type: "file"
        path: "docs/plans/rcd-03-monitor-punch-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-03-monitor-punch-task.md"
      - type: "file"
        path: "docs/test-specs/rcd-03-monitor-punch.md"
      - type: "file"
        path: "docs/impl/ir/rcd-03-monitor-punch.md"
      - type: "file"
        path: "docs/impl/exp/rcd-03-monitor-punch.md"
      - type: "file"
        path: "evidence/sv117/rcd-03-monitor-punch-session.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-03-monitor-punch-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-03-monitor-punch-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-03-monitor-engine.md"
    - "docs/impl/ir/rcd-03-monitoring-punch-latency-crate-slice.md"
    - "docs/impl/ir/rcd-03-app-arm-monitoring-ux-slice.md"
    - "docs/impl/ir/rcd-03-monitor-persistence-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/rcd-03-monitor-punch-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-03-monitor-punch.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/rcd-03-monitor-punch.md"
    rejected: []
---

# RCD-03 synvelis-recording fake session fixture 実装記録

## Summary

Issue #117（RCD-03 Monitoring / Punch / Latency）の残
`synvelis-recording` slice として、fake device →
`InputSession`/`InputAdapter` → `StreamWriter` → `StagingArea`
（`DeviceRig`）と `MonitorProbe`（`GateScheduler`/`decide`
共有）を同一 block 駆動で合成する headless fixture を追加した。
punch gate `[300,700)` session frame・session_end 1000 frame・
block 384（punch 境界を block 途中に跨ぐ）で、Off/Auto/On ×
Arm × Transport × Mute/Solo の route 観測が実 capture
journal/staging/commit と整合し、かつ monitor 状態が raw
capture bytes・journal span・fault bits・verified prefix・
commit payload を一切変更しないことを 4 test で pin した。
punch 中の device loss は fail-stop + durable
capture-fault + verified prefix 保存を示し、新 session での
clean recovery と旧 staging の非破壊を確認した。2 つの
interleave session 間で fault が漏洩しないことも検証した。
production `src/` は無変更 — 既存の monitor/capture 境界が
正しい意味論を既に所有していることを fixture が証明する。
本 slice は Issue #117 を Close しない（実 device monitor・
実 GUI・plugin 受入は別 gate）。

## Context

- spec `docs/specs/rcd-03-monitor-punch.md`
- design `docs/designs/rcd-03-monitor-punch-design.md`
- plan `docs/plans/rcd-03-monitor-punch-plan.md`
- task `docs/tasks/rcd-03-monitor-punch-task.md`
- test-spec `docs/test-specs/rcd-03-monitor-punch.md`
- evidence `evidence/sv117/rcd-03-monitor-punch-session.md`

先行 slice（crate slice・engine 搭載・app UX・persistence・
VCS diff 分類）で `MonitorProbe`/`decide`/`MonitorAdapter`/
`DeviceRig`/`build_payload` の各意味論は確定済みだったが、
「実 staged capture を伴う session レベルで monitor 状態と
raw capture が独立である」ことを示す統合 fixture が無く、
device fault 系 test はすべて `MonitorState::Off` で駆動されて
いた。本 slice はその gap を埋める。`synvelis-recording`
以外の crate・app/UI 層・docs（本 doc-chain 以外）は無変更。

## Implementation

### Changes

- `crates/synvelis-recording/tests/monitor_punch_session.rs`
  （新規、4 test）:
  - fixture 定数 — punch `[12,28)` tick = session frame
    `[300,700)`、stop 40 tick = `session_end` 1000 frame
    （25 f/tick）、block 384 で punch 境界を block 途中に跨ぐ。
    明示 `CaptureStreamId::new(500)` を `Project.recording
    .streams` に登録し `DeviceRig::open` の `stream_ids` で
    staging stream と commit 契約を一致させる。
  - `open_session` — 同一 spec から `preflight` を 2 回呼び、
    capture 側（`DeviceRig` が所有する `PreparedRecordingPlan`）
    と monitor 側（`MonitorProbe<'a>` が `&plan` を借用する
    別 instance）の plan を構築する。実系でも monitor
    session は録音側と同一の prepared plan を参照する。
  - `SessionProbe`/`drive_with_probe` — `DeviceRig::drive`
    と同一 block 刻みで `MonitorProbe::feed` を回し、
    half-open・連続な monitor segment を収集する
    （非 RT・headless、gate 権威は `GateScheduler` に残る）。
  - `session_monitor_routes_track_gate_and_raw_stays_gated`
    （T1）— Off/Auto/Auto+muted/On+ready/On+!ready の 5 case
    を clean session で駆動し、probe の `gate_open` merge が
    `[300,700)`・route 区間が `decide` 規則どおり（On+ready
    は gate 外へ Input 拡張、On+!ready は Playback fallback +
    `input_unavailable` notice）であること、かつ media bytes・
    journal span・`build_payload` の segment/pass/take 構造が
    monitor 状態に依らず一致することを検証。capture 側は
    常に gate-open 区間のみを journal cover し PCM が
    `MockPcm` と一致する（raw は gate に留まる）。
  - `session_device_faults_do_not_depend_on_monitor_state`
    （T2）— Off/Auto/On で punch gate-open 内部（frame 600）
    まで正常駆動後に frame gap を注入し、`FAULT_INPUT_
    DISCONTINUITY`・fail-stop・verified prefix（gate-open
    `[300,700)` のみ）・PCM・finish report が monitor 状態に
    依らず一致することを検証。
  - `session_punch_device_loss_fail_stop_and_new_session_
    recovery`（T3）— punch 録音中の device loss で fail-stop、
    `finish` が durable capture-fault marker を書き、旧
    session の verified prefix が保存されること、新しい
    device/staging root/session での clean recovery が
    Complete として commit できること、旧 staging が recovery
    で変更されないことを検証。
  - `session_faults_do_not_leak_between_monitoring_sessions`
    （T4）— faulted session A と clean session B を interleave
    駆動し、B の capture/finalize/commit が A の fault に
    影響されず、A は引き続き `capture-fault:` Recoverable
    として分類されることを検証。
  - 共有 helper — `span_obs`/`commit_shape`（monitor 非依存
    な観測形への射影）、`assert_segment_tiling`/
    `merged_regions`/`route_regions`（segment 証拠の検査）、
    `assert_commit_coverage`（commit segment が gate-open
    区間を隙間・重複なく cover）。
- `docs/` doc-chain 5 文書 + 本 impl record + exp log +
  `evidence/sv117/rcd-03-monitor-punch-session.md`。

### Decisions During Implementation

- gate/route 判定の権威は recording 側（`GateScheduler`/
  `evaluate_gate`/`decide`）に残し、fixture は `MonitorProbe`
  を第二 gate 実装として使わず plan の scheduler を共有する
  だけ。`MonitorState::On` の Input route が gate 外へ拡張
  するのは意図された製品意味論であり、raw capture は gate
  内に留まる — この非対称性が「monitor は raw を変更しない」
  の直接証拠になる。
- `PreparedRecordingPlan` は非 `Clone` で `MonitorProbe<'a>` が
  `&plan` を借用するため、capture 側と monitor 側に同一 spec
  の `preflight` を 2 回呼ぶ（borrow conflict の回避 — 詳細は
  exp log）。
- `build_payload` は staging `capture_stream_id` が
  `Project.recording.streams` に登録済みであることを要求する
  — fixture は `CaptureStreamId::new(500)` を明示登録して
  commit 契約と一致させる。
- fixture 駆動はすべて fake device/headless であり、実
  device・実 GUI・実 plugin の受入を主張しない
  （`evidence/sv117/` で Pass/NotRun を分離記録）。

## Related Experiments

### Adopted

- `docs/impl/exp/rcd-03-monitor-punch.md` — `MonitorProbe`
  の `&plan` 借用と `DeviceRig` の plan 所有の競合を、同一
  spec の `preflight` 2 回呼出し（plan の複製）で解決。

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク、Windows 11 x64・
rust 1.99.0 pinned）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check synvelis-recording` — Pass
  （`cargo check --locked --all-targets -p
  synvelis-recording`、exit 0）
- `mise run lint synvelis-recording` — Pass
  （`cargo clippy`、exit 0）
- `mise run test synvelis-recording` — Pass
  （`cargo test --locked -p synvelis-recording`、exit 0。
  `monitor_punch_session.rs` 4 test を含む全 test 緑）

補助証拠（canonical = mise）:
`evidence/sv117/rcd-03-monitor-punch-session.md` に
Expected/Observed を記録。fake/headless fixture の Pass と
実 device・実 GUI・plugin 受入の NotRun を分離。

## Deferred / Follow-up

- 実 device monitor/latency 受入・実 GUI 受入・実 plugin
  受入（Issue #117 残件、Open 継続）。
- `synvelis-player`/`synvelis-app` 側の monitor 配線・UX
  fixture（別 Worker 占有のため本 slice 対象外 — PR 本文の
  「未対応の作業」に記録）。
- 2Window/Focus 変化の UI scenario（app 層 fixture が必要）。
- count-in UX・AudioLens・RT graph monitor node（別 slice）。
