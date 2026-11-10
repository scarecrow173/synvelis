---
id: "IMPL-034V30Z5uYklMrResgr0lS"
type: "impl"
status: "completed"
title: "RCD-01 実Audio Device録音統合 headless slice — fake port→実StreamWriter fixture + recording state projection"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/tests/device_recording.rs"
      - type: "file"
        path: "crates/synvelis-app/src/recording.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/recording_projection.rs"
      - type: "file"
        path: "docs/specs/rcd-01-device-recording-headless-fixture.md"
      - type: "file"
        path: "docs/designs/rcd-01-device-recording-headless-fixture.md"
      - type: "file"
        path: "docs/plans/rcd-01-device-recording-headless-fixture.md"
      - type: "file"
        path: "docs/test-specs/rcd-01-device-recording-headless-fixture.md"
      - type: "file"
        path: "docs/tasks/rcd-01-device-recording-headless-fixture.md"
      - type: "file"
        path: "docs/impl/ir/rcd-01-audio-device-headless-slice-fake-port-streamwriter-fixture-recording-stat.md"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/src/gate.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-01-device-recording-headless-fixture.md"
    - "docs/designs/rcd-01-device-recording-headless-fixture.md"
    - "docs/specs/rcd-01-device-recording-headless-fixture.md"
    - "docs/tasks/rcd-01-device-recording-headless-fixture.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "実 Device backend・OS 別ドライバ・実機受入（ruviform#251 待ち、別 Gate）"
    - "録音 command/UI（#114 / SV-GUI-05）"
    - "cross-clock multi-device（#119）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-01 実Audio Device録音統合 headless slice — fake port→実StreamWriter fixture + recording state projection

## Summary

scripted fake `Device`/`InputStream`（Ruviform device port）→
`InputAdapter` → 実 `StreamWriter`（staging media/journal）→
checkpoint/finalize までを一貫駆動する統合 fixture
（`tests/device_recording.rs`、16 test）と、`Project.recording`
persisted entity の read-only projection `syn.recording.info`
（`synvelis-app/src/recording.rs` + fixture）を実装した。
併せて `gate.rs` の session_end clamp で末尾 span の `project_out`
が template 終端を名乗り短い区間を gate 全域へ引き伸ばす欠陥を
修正した。これは fake port 契約の検証であり、実 Device Pass・
実機受入・実 GUI ではない。

## Context

Issue #115 / SV#115 RCD-01。前段の fake-port adapter slice
（commit `70d6b21`、PR #205）は `InspectionWriter` 止まりで、
実 media/journal との統合証拠がなかった。本 slice は spec →
design → plan → test-spec → task の doc chain
（`docs/{specs,designs,plans,test-specs,tasks}/rcd-01-*`）に従い、
headless で成立する契約層のみを対象とする。実 Device backend は
ruviform#251 待ちで `Unsupported`、cross-clock multi-device は
#119、録音 command/UI は #114/SV-GUI-05、AudioLens f64 非回帰は
#294 に defer。

## Implementation

### Changes

- `tests/common/mod.rs`: `DeviceRig` 統合 harness を追加 —
  `plan_gated`/`staging_streams`/`open`/`open_one`/`deliver_all`/
  `pump_all`/`pump_engines`/`drive`/`verify_formats`/`finish`。
  `FakeInputDevice::instance_id` で同一 device の複数 stream を
  区別し、`InputSession` open → `TapWriterEnds` → 実
  `StreamWriter` までを結線。`capture_of`/`verify_capture_pcm`/
  `all_journal_spans`/`span_obs`/`assert_session_coverage`/
  `assert_source_contiguous`/`assert_prefix_contiguous` の
  Expected/Observed 検証 helper を追加。
- `tests/device_recording.rs`: 16 test — Mono/Stereo/
  MultiChannel/2Track（1 stream 2 tap）/same-clock 2 device の
  PCM bit 照合（`MockPcm` + channel map、`to_bits` 経由の
  `verify_capture_pcm` で media bytes + `pcm_crc32`/`media_offset`/
  `media_len` を journal record と照合）、Punch `[in,out)` と
  PunchInLoop/Loop の pass 境界 Sample 単位検証（`span_obs` 完全
  一致 + span tiling + `pass_index` 単調性 + source 連続性）、
  Arm Off + Monitor On の writer 不作成（media/journal 非存在）、
  Monitor On/Off で media bytes 一致、fail-stop 6 系（Input Drop・
  Clock Epoch 変更・Rate 変更・Device 切断・RT queue full・
  filled queue full）で fault bit・capture-fault marker・verified
  prefix 保持・commit 層 recoverable 分類を検証。
- `src/gate.rs`: `GateScheduler::extend` で `session_end` に
  clamp された span の `project_out` を、template の
  session↔project 対応で打ち切り位置へ補間し直すよう修正
  （`interp_project` を `project_at` と共用化）。従来は clamp 後も
  template 終端を名乗り、`project_at` が残り frame を gate 全域へ
  引き伸ばしていたため、commit の segment/clip 配置がずれ得た。
- `synvelis-app/src/recording.rs`: `Project.recording` の
  sessions/passes/streams/segments/takes を tagged id・state・
  range 付きで投影し、recorder slot status・`transaction_open`
  を付す read-only `recording_info`。`app.rs` に
  `SynvelisApp::recording_info`、`service.rs` に `syn.recording.info`
  op を追加（additive、既存 op/画面/service 非接触）。
- `synvelis-app/tests/recording_projection.rs`: 空 registry・
  entity 投影・service op wiring の 3 test。

### Decisions During Implementation

- PCM 照合は float 近似ではなく media bytes の `f32::from_le_bytes`
  と `MockPcm::sample` の厳密一致 + journal `pcm_crc32` 検算で行う
  （Expected/Observed を bit 単位で固定）。
- fail-stop の verified prefix は「fault 確定前に journal へ書けた
  分」— queue-full が最初の chunk 確定前に起きる系では空 prefix
  を許容し、fault 後入力が記録されないことを検証する。
- `syn.recording.info` は persisted entity の投影のみとし、
  recording command・arm 状態・実 Device 情報は載せない
  （command/UI は #114、device は #115 後続 gate）。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

Windows 11 / rustc 1.99.0（mise pin）で実施:

- `mise run fmt-check` — Passed
- `mise run check`（`cargo check --workspace --all-targets`）— Passed
- `mise run lint`（`cargo clippy --workspace --all-targets -- -D warnings`）— Passed
- `mise run test`（`cargo test --workspace --locked`）— Passed
  （初回実行で `synvelis-library` webdav
  `browse_rejects_file_scope` が 1 件 flaky 失敗
  （TransientIo vs InvalidRequest）。本変更と無関係の領域で、
  単独再実行・workspace 全体の再実行ともに Passed）
- `mise run test-rt` — Passed
- 補助証拠: `cargo test -p synvelis-recording`（16 device_recording
  test 含む全 binary Passed）、`cargo test -p synvelis-app --test
  recording_projection`（3 test Passed）
- review-protocol: 独立 2 pass（cooperative acceptance coverage +
  adversarial defect scan）+ candidate validation + judgment を
  実施。CONFIRMED P0/P1 なし → ACCEPT WITH NON-BLOCKING NOTES
  （temp dir 残存は既存 fixture と同運用、helper doc の軽微な
  不整合のみ）。

## Risks

- fake port 契約の証拠であり、実 Device の callback timing・
  driver 挙動・OS backend は未検証（ruviform#251 待ち）。
- `gate.rs` の project_out 補間修正は clamp された末尾 span のみ
  に影響し、既存 test（vs_b/vs_d/vs_c/monitor_gate）は全て通過。

## Follow-ups

- 実 Device Mono/Stereo/MultiChannel/2Track の受入（OS 別
  ドライバ、#115 後続 gate / #83・#114 証拠リンク）。
- `syn.recording.info` を消費する録音 UI（#114/SV-GUI-05）。
- cross-clock multi-device（#119）。
