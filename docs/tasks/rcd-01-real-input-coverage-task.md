---
id: "TASK-034W91vSDozzYDSG2XM7je"
type: "task"
status: "completed"
title: "RCD-01 実機 capture coverage 拡張 task"
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
        path: "crates/synvelis-recording/tests/real_input.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-01-real-input-coverage-plan.md"
  implemented-by: []
  depends-on:
    - "TASK-GJlWE8j5emvVOKFMtbTXHR"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/rcd-01-real-input-coverage-plan.md"
    - "docs/specs/rcd-01-real-input-coverage.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-01-real-input-coverage.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 実機 capture coverage 拡張 task

## Work

`tests/real_input.rs` の `#[ignore]` opt-in 実機 lane を拡張し、
実 device → `InputSession`/`InputAdapter` → 実 `StreamWriter` →
staging media/journal → checkpoint/finalize の経路で 2 track・
fail-stop・punch 境界・長時間連続性を実機検証可能にする。
`tests/common/mod.rs` に `&dyn Device` を受ける `RealDeviceRig`
と rate/budget 一般化 plan helper・実機 PCM 構造検証 helper を
追加し、既存 fake `DeviceRig` は非破壊で維持する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-01-real-input-coverage-plan.md`
- `docs/specs/rcd-01-real-input-coverage.md`
- `docs/designs/rcd-01-real-input-coverage-design.md`
- `docs/test-specs/rcd-01-real-input-coverage.md`

## Dependencies

- `TASK-GJlWE8j5emvVOKFMtbTXHR`（先行 slice: composition root +
  opt-in lane + stamp fallback）。
- Issue #115（RCD-01 実機受入 gate）。

## Affected Files

- `crates/synvelis-recording/tests/common/mod.rs` —
  `open_rig_core`/`RealDeviceRig`/`plan_at`/
  `verify_capture_integrity`/`tmp_dir`。
- `crates/synvelis-recording/tests/real_input.rs` — 実機 test
  8 件追加。
- `crates/synvelis-recording/src/*` — 変更なし（API gap 無し）。

## Done When

- [x] `RealDeviceRig` が `&dyn Device` inputs で staging +
  `StreamWriter` rig を構築し、`start_all`/`stop_all`/`run_for`/
  rate 対応 `finish` を提供する。
- [x] `plan_at` が negotiated rate/budget/max_block_frames を
  受け、`plan_for`/`plan_gated` が `RATE`/`budget()`/1024 で
  委譲する（既存 caller 不変）。
- [x] `verify_capture_integrity` が chunk crc32・media 連続性・
  span 範囲・PCM 有限性を検証する。
- [x] 実機 test 8 件が `real_lane` module に追加され、
  `#[ignore]` + `real-device` feature で opt-in を維持する。
- [x] 実機不在・format 交渉不能・channel 不足・multi-device
  前提不成立は loud SKIP/`NotRun` 記録で binary を落とさない
  （device 脱落 run で全 hardware test の loud SKIP・
  suite non-failing 終了を実測）。
- [x] canonical 検証一式が実走・pass（実機 lane は接続時のみ
  Pass、未接続は loud SKIP/NotRun 記録）。workspace 全体の
  `check`/`lint`/`test`/`test-rt`/`test-msrv`/`build`/
  app-native は変更が synvelis-recording test + docs に限定
  されるため scoped 相当（`cargo test -p synvelis-recording
  --locked` 全 target pass・clippy all-targets `-D warnings`
  pass）で代替 — 詳細は impl IR の Validation を参照。

## Verification

- [x] `mise run fmt-check` — pass。`mise run check`/`lint`/
  `test`/`test-rt`/`test-msrv`/`build`（workspace 全体）は
  test-only 差分のため scoped 検証で代替（下記）。
- [x] `mise run lint-recording-real-input` — pass。
- [x] `mise run test-recording-real-input` — 実機 lane
  10/10 ok（iD4/WASAPI・44100 Hz stereo F32）。nocapture で
  e2e・format reject・timestamp probe・long capture（clean
  Pass: 1,323,000 frames/30.00 s）を実測。multi-device・
  deterministic PCM は NotRun、device 脱落時は loud SKIP。
- [ ] `mise run check-app-native` / `lint-app-native` —
  synvelis-app 無変更のため未実施（外部 gate として記録）。
- [x] fake `DeviceRig` 既存 test が無変更で pass
  （`cargo test -p synvelis-recording --locked` 全 target）。
