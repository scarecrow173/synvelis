---
id: "PLAN-034W91UxBVqeOCrJpWA1Gw"
type: "plan"
status: "approved"
title: "RCD-01 実機 capture coverage 拡張 plan"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/tests/real_input.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/device_recording.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-01-real-input-coverage.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-01-real-backend-readiness-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-01-real-input-coverage.md"
    - "docs/designs/rcd-01-real-input-coverage-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-01-real-input-coverage.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 実機 capture coverage 拡張 plan Implementation Plan

## Goal

`tests/real_input.rs` の opt-in 実機 lane を、fake/headless 契約
証拠から実機 capture 証拠（実 `StreamWriter`/staging・2 track・
fail-stop・punch 境界・長時間連続性）へ拡張し、実行不能行を
`NotRun`/`Unsupported` として正直に記録する。

## Scope

1 本の実装 stream。変更は `tests/common/mod.rs`（rig/plan helper
一般化）と `tests/real_input.rs`（実機 test 追加）に限定し、
`src/`・`mise.toml`・`Cargo.toml`・`Cargo.lock`・
`synvelis-app/**` は変更しない。doc chain
（spec/design/plan/task/test-spec/impl IR）を同 slice で作成する。

## Upstream Documents

- `docs/specs/rcd-01-real-input-coverage.md`
- `docs/designs/rcd-01-real-input-coverage-design.md`
- `docs/specs/rcd-01-real-backend-readiness.md`（先行 slice）
- `docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md`

## Design Inputs

- `docs/designs/rcd-01-real-input-coverage-design.md`（approved）
- `docs/designs/rcd-01-real-backend-readiness-design.md`
- `docs/designs/rcd-01-device-recording-headless-fixture.md`

## File Map

- `crates/synvelis-recording/tests/common/mod.rs` —
  `open_rig_core` 抽出（`DeviceRig`/`RealDeviceRig` 共有）、
  `RealDeviceRig` 追加（`&dyn Device` inputs・`start_all`/
  `stop_all`/`run_for`/rate 対応 `finish`）、`plan_at` 追加 +
  `plan_for`/`plan_gated` 委譲化、`verify_capture_integrity`
  追加、`tmp_dir` 昇格。
- `crates/synvelis-recording/tests/real_input.rs` — `real_lane`
  module に実機 test 8 件追加（2-track・e2e writer・RT
  queue-full・format/rate reject・punch 境界・長時間・multi-device
  ・PCM fidelity NotRun）。
- `crates/synvelis-recording/tests/device_recording.rs` — local
  `tmp_dir` を common 版へ置換（重複解消のみ）。
- `crates/synvelis-recording/src/*` — 原則変更なし（API gap が
  出た場合のみ最小変更）。
- docs — spec/design/plan/task/test-spec/impl IR。

## Implementation Sequence

- [x] doc chain 骨格作成（spec/design/plan/task/test-spec）。
- [ ] `tests/common/mod.rs`: `open_rig_core` 抽出 + `DeviceRig`
  委譲化（既存 fake test が無変更で通ることを先に確認）。
- [ ] `tests/common/mod.rs`: `RealDeviceRig`・`plan_at`・
  `verify_capture_integrity`・`tmp_dir` 追加。
- [ ] `tests/device_recording.rs`: `tmp_dir` 重複解消。
- [ ] `tests/real_input.rs`: 実機 test 8 件追加。
- [ ] `cargo check`/`clippy`（feature 有無両方）で compile 確認。
- [ ] canonical 検証一式を実走。
- [ ] impl IR + task 完了記録、commit/PR/Issue コメント。

## Dependencies and Risks

- 依存: 先行 slice（`rcd-01-real-backend-readiness`）の
  composition root・stamp fallback・opt-in lane task。
- `BufferSize::Default` の可変 block: `chunk_frames=
  max_block_frames=8192` で吸収、超過は既存 FAULT で fail-stop。
- queue-full 誘発は pump 停止で決定的に行うが、callback が来ない
  環境では timeout fail（Pass にしない）。
- 44.1k 等非 48k rate: `plan.mapping.tick_to_media` で期待値を
  rate 別計算し、session/staging/finalize の rate を negotiated
  値で一致。
- multi-device は `same_clock_domain` 検証を緩和せず、
  UNKNOWN/不一致は `NotRun` 記録（#119 先取り禁止）。

## Task Breakdown

- `docs/tasks/rcd-01-real-input-coverage-task.md`（単一 task、
  上記 sequence を内包）。

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `build` — 既定 lane 全 pass。
- [ ] `mise run lint-recording-real-input` — feature clippy pass。
- [ ] `mise run test-recording-real-input` — 実機 lane 実行
  （iD4 接続時 Pass/NotRun/Unsupported 記録、不在時 loud SKIP）。
- [ ] `mise run check-app-native` / `lint-app-native` — app 側
  無変更の確認。
- [ ] fake `DeviceRig` 既存 test（`device_recording.rs`/
  `input_adapter.rs`）が無変更で pass — rig 一般化の非破壊確認。

## Open Questions / Gaps

- deterministic PCM source（OS loopback）の有無は実機環境依存 —
  無ければ `NotRun`/`Unsupported` 記録で確定。
- multi-device same-clock が通るかは hardware/driver 依存 —
  `NotRun` 記録で確定。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（dispatched worker が直接実装）。
- Independent work streams: none。
- User approval required before dispatch: no（worker spec で承認済み）。

## Review Handoff

- reviewer は「Mock Pass を実機 Pass に昇格していないか」「
  `NotRun`/`Unsupported` が正直に記録されているか」「fake rig
  の非破壊」を重点確認。
