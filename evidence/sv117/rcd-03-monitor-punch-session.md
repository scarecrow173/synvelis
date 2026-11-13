# RCD-03 Monitoring/Punch Fake Session Fixture — Evidence Ledger (synvelis#117)

Headless fixture evidence for the `synvelis-recording` slice of
Issue #117 (Monitoring / Punch / Latency UX — Auto標準・Raw独立・
Direct監視). Every row records an independently observed
Expected/Observed pair from
`crates/synvelis-recording/tests/monitor_punch_session.rs`.
`Pass` here means *the stated headless/fake-fixture lane only* —
it never implies real device capture, real monitor output, real
GUI, RT-thread measurement, or OS-level acceptance. Those lanes
are `NotRun`/`Deferred` with reasons, per the vocabulary in
`docs/specs/sf-06-cross-gate-oracle-case-ledger.md` and
`POLICIES.md`.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis branch | `scarecrow173/sv117-monitor-punch` |
| OS | Windows (win32) |
| Toolchain | rustc via `mise.toml`/`mise.ci.toml` pin（実測値は PR 本文に記録） |
| Fixture | scripted `FakeInputDevice`/`FakeStreamShared`（`tests/common`）→ real `InputSession`/`InputAdapter` → real `StreamWriter` → real `StagingArea`（temp dir の media/journal/finalized）→ `build_payload` commit 層。`MonitorProbe` は同一 spec の別 `PreparedRecordingPlan` instance を借用して同一 block 刻みで feed |
| Session shape | `GateSpec::Punch` [12,28) tick = session frame [300,700)、stop 40 tick = session_end 1000 frame、block 384（境界を block 途中に跨ぐ）、mono 1ch、`ChannelMap::mono(0)` |
| Lane | headless unit/integration test のみ（`mise run test synvelis-recording`、実機・GUI lane なし） |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Fail` = measured failure; `NotRun` = lane exists but was not
measured here; `Unsupported` = capability genuinely unavailable;
`Deferred` = intentionally out of scope for this slice.

## RCD-03-MPS-01 — route/capture coherence + Raw File 独立性

Fixture: `session_monitor_routes_track_gate_and_raw_stays_gated`
（5 case: Off / Auto / Auto+muted / On+ready / On+!ready、
`TransportPhase::Playing` 固定・block 384 で 1000 frame 駆動）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| probe gate_open 区間 | `[300,700)` に merge、segment が `[0,1000)` を隙間・重複・はみ出しなく tile | 全 case で一致 | Pass |
| Off route | gate-open `[300,700)` は `Muted`、他は `Playback` | 一致 | Pass |
| Auto+Arm+Playing route | gate-open `[300,700)` は `Input`、他は `Playback` | 一致 | Pass |
| Auto+muted route | mute は Playback route のみ作用 — gate 閉は `Muted`、gate-open は `Input` のまま | 一致 | Pass |
| On+ready route | arm/gate に依らず全域 `[0,1000)` が `Input`、かつ capture は `[300,700)` のみ（Raw 独立性） | 一致 | Pass |
| On+!ready route | 全域 `Playback` + 全 segment `input_unavailable` notice | 一致 | Pass |
| journal span 被覆 | 全 case で `assert_session_coverage(&[(300,700)])`・source 連続 400 frame | 一致 | Pass |
| Raw File 独立性 | 5 case で media bytes・`span_obs` が完全一致、`verify_capture_pcm`（MockPcm 全 sample）Pass | 一致 | Pass |
| finalize | `FinalizedOutcome::Complete`・`source_frames == 400` | 一致 | Pass |
| commit payload | `payload.streams[0].segments` の session 被覆が `[300,700)`、commit shape（segment 範囲・pass index・take 参照）が monitor 状態間で完全一致、pass `[0]`・take 1 件 | 一致 | Pass |

## RCD-03-MPS-02 — device fault の monitor 非依存性

Fixture: `session_device_faults_do_not_depend_on_monitor_state`
（frame 600 通過後に `jump_to(+777)` で frame gap を注入、
Off/Auto/On の 3 case + Auto で `raise(DeviceNotFound)`）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| frame gap × Off/Auto/On | 3 状態すべて `FAULT_INPUT_DISCONTINUITY` で fail-stop、verified prefix = gate-open ∩ [0,600) = `[300,600)`、capture-fault marker 存在、journal intact（CleanEnd/TornTail）、PCM verified、`(fault bits, span_obs, prefix 総 frame)` が状態間で完全一致 | 一致 | Pass |
| device disconnect（Auto） | `raise(DeviceNotFound)` で同じ `FAULT_INPUT_DISCONTINUITY` fail-stop、verified prefix = `[300,500)`、PCM verified | 一致 | Pass |

## RCD-03-MPS-03 — punch 中 device loss → fail-stop → recovery

Fixture: `session_punch_device_loss_fail_stop_and_new_session_recovery`
（Session A: frame 500 = gate-open 内部で `DeviceNotFound` →
fail-stop。Session B: 新規 `FakeInputDevice` + 新規 staging +
新規 session id で同一形状を clean 駆動）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| Session A fail-stop | `FAULT_INPUT_DISCONTINUITY`、capture-fault marker、verified prefix = `[300,500)` 連続、PCM verified | 一致 | Pass |
| Session A commit 分類 | `build_payload` が stream を `capture-fault:` 理由で recoverable 分離、`payload.streams`/`passes` 空 | 一致 | Pass |
| Session B recovery | 新規 session が fault なしで完走、`FinalizedOutcome::Complete`・`source_frames == 400`、span 被覆 `[300,700)`、PCM verified | 一致 | Pass |
| Session B commit | `payload.streams[0]` が commit、segment 被覆 `[300,700)` | 一致 | Pass |
| 旧 session 保持 | Session A の media/journal/verified prefix・capture-fault marker が recovery 後も不変 | 一致 | Pass |

## RCD-03-MPS-04 — 2 session 並行の fault 分離

Fixture: `session_faults_do_not_leak_between_monitoring_sessions`
（Session A: Monitor On 宣言、Session B: Monitor Off 宣言、
別 device・別 staging root・別 session id。A に frame gap を
注入して fail-stop させた後 B を完走）。

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| Session B 非影響 | B は `FinalizedOutcome::Complete`・`source_frames == 400`・span 被覆 `[300,700)`・PCM verified・capture-fault marker 無し・`build_payload` commit 成功（recoverable 空） | 一致 | Pass |
| Session A prefix | A は verified prefix `[300,500)` を保持して終了、capture-fault marker 存在 | 一致 | Pass |
| fault 分離 | A の fault bits/marker が B の session・area・payload に一切現れない（`FaultLatch` は session 共有の構造的契約） | 一致 | Pass |

## Deferred / NotRun / Unsupported

| Lane | Status | Reason / Follow-up |
| --- | --- | --- |
| 実 Audio Capture device での monitor/punch | `Deferred` | L2 実機 gate。fake `MockPcm` は実デバイス PCM ではない。実機 slice で `real-device` opt-in lane を拡張 |
| 実機 Direct Monitor（hardware direct）の可聴確認 | `Deferred` | `HardwareDirectState::Unknown` は観測値として正直に報告する契約のみ検証済み。実機での direct monitor 有無・二重監視の聴取は L2/L3 |
| 実機 Latency 実測・実 plugin | `Deferred`/`NotRun` | `LatencyReport`/`MonitorAdapter` の engine 経路は PR #373 で検証済み。実機 latency・plugin 計測は別 gate |
| 実 GUI 2Window/Focus 変化 | `Deferred` | `synvelis-app` は別 Worker 占有。fake-session 分離（本 ledger RCD-03-MPS-04）のみ pin。GUI Scenario は #114 系列 |
| in-session device resume | `Deferred` | fail-stop + verified prefix + 新規 session recovery が製品契約。本 slice は resume を主張しない |
| Windows/macOS/Linux CI | `NotRun`（本 Worker 範囲） | merge 後の対象 SHA の CI 証拠は Coordinator が確認。Worker はブランチ CI/ローカル検証を PR 本文に記録 |
| RT callback 内での monitor 処理 | `NotRun` | `MonitorProbe`/`SessionProbe` は非 RT 観測経路。RT-safe 化・graph node 搭載は別 slice |
