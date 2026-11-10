---
id: "TASK-034V2kE9l91elvsqfnsx5s"
type: "task"
status: "in_progress"
title: "RCD-01 device-recording headless fixture"
created: "2026-11-10"
updated: "2026-11-10"
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
    - "docs/plans/rcd-01-device-recording-headless-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-01-device-recording-headless-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-01-device-recording-headless-fixture.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 device-recording headless fixture

## Work

fake Ruviform port → `InputAdapter` → 実 `StreamWriter` → staging media/journal → finalize/recovery を結合する決定的 fixture を `crates/synvelis-recording/tests/` に実装し、`synvelis-app` に recording 状態投影と fixture を追加する。`src/` への変更は行わない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-01-device-recording-headless-fixture.md`
- `docs/specs/rcd-01-device-recording-headless-fixture.md`
- `docs/designs/rcd-01-device-recording-headless-fixture.md`
- `docs/test-specs/rcd-01-device-recording-headless-fixture.md`
- Issue `synvelis#115`

## Dependencies

- 先行 slice `TASK-LRVMzVtzazw5gBnDcKYhLk`（RCD-01 fake port 契約層、実装済み commit `70d6b21`）。
- なし（Blocked by なし）。

## Affected Files

- `crates/synvelis-recording/tests/common/mod.rs`
- `crates/synvelis-recording/tests/device_recording.rs`（新規）
- `crates/synvelis-app/src/recording.rs`（新規）
- `crates/synvelis-app/src/lib.rs`
- `crates/synvelis-app/src/service.rs`
- `crates/synvelis-app/tests/recording_projection.rs`（新規）
- `docs/impl/ir/`、各 `README.md` index

## Done When

- [ ] PCM fidelity（Mono/Stereo/MultiChannel/2Track）が media 実バイトと journal CRC で Expected と bit 一致する fixture が Pass する。
- [ ] Punch `[in,out)`・Loop Pass の Sample 単位境界が span tile で検証される。
- [ ] Arm Off + Monitor On で writer 不作成・media/journal 空が検証される。
- [ ] fail-stop 6 種（Input Drop/Epoch/Rate/Device 切断/RT queue full/filled queue full）で Verified Prefix + `capture-fault` marker + Partial/Unknown 観測が検証される。
- [ ] `synvelis-app` recording 投影と fixture が Pass する。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt` が Pass する。

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check`
- [ ] `mise run lint`
- [ ] `mise run test`
- [ ] `mise run test-rt`
- [ ] review-protocol レビューで blocker 無し
