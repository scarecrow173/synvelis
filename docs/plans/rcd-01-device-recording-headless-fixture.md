---
id: "PLAN-034V2jiWN4B0YR7h0PVI1y"
type: "plan"
status: "approved"
title: "RCD-01 device-recording headless fixture 実装計画"
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
    - "docs/specs/rcd-01-device-recording-headless-fixture.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-01-device-recording-headless-fixture.md"
    - "docs/designs/rcd-01-device-recording-headless-fixture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 device-recording headless fixture 実装計画 Implementation Plan

## Goal

fake Ruviform port → `InputAdapter` → 実 `StreamWriter` → staging media/journal → finalize/recovery を結合する決定的 fixture と、`synvelis-app` の recording 状態投影 fixture を実装する。

## Scope

単一の実装 stream。`crates/synvelis-recording/tests/` の fixture/harness と `crates/synvelis-app` の投影 module + fixture、および文書チェーン・index 更新を含む。`src/` への変更は行わない（既存 API のみ使用）。

## Upstream Documents

- `docs/specs/rcd-01-device-recording-headless-fixture.md`
- `docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md`（先行 slice）
- Issue `synvelis#115`

## Design Inputs

- `docs/designs/rcd-01-device-recording-headless-fixture.md`

## File Map

- `crates/synvelis-recording/tests/common/mod.rs` - 統合 harness 追加（StagingArea 構築、TapWriterEnds→StreamWriter 配線、drive loop、finalize/verify helper）。既存 fake port を再利用。
- `crates/synvelis-recording/tests/device_recording.rs` - 新規 fixture テスト（PCM fidelity / Punch・Loop 境界 / Arm・Monitor / fail-stop / writer 不作成）。
- `crates/synvelis-app/src/recording.rs` - `Project.recording` entity → 表示行の read-only 投影 module（新規）。
- `crates/synvelis-app/src/lib.rs` - module export 追加。
- `crates/synvelis-app/src/service.rs` - `syn.recording.info` op の additive 追加（必要最小限）。
- `crates/synvelis-app/tests/recording_projection.rs` - 投影 fixture テスト（新規）。
- `docs/impl/ir/` - 実装記録、各 `README.md` index。

## Implementation Sequence

- [ ] `tests/common/mod.rs` に統合 harness（staging 構築 + writer 配線 + drive/finalize helper）を追加
- [ ] `tests/device_recording.rs` に PCM fidelity（Mono/Stereo/MultiChannel/2Track）fixture を実装
- [ ] 同ファイルに Punch/Loop 境界 fixture を実装
- [ ] 同ファイルに Arm Off+Monitor On writer 不作成 fixture を実装
- [ ] 同ファイルに fail-stop fixture（Input Drop/Epoch/Rate/Device 切断/queue full）を実装
- [ ] `synvelis-app` に recording 投影 module + op + fixture を実装
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt` で検証
- [ ] impl 記録・index 更新・review-protocol レビュー

## Dependencies and Risks

- `TapWriterEnds.stream_index` ↔ `plan.targets`/`StagingStreamConfig` の対応付け（fixture 内で `stream_index == target index` を明示）。
- engine fault 時の writer finalize outcome（Complete になり得る）→ `capture-fault` marker が session レベルの Partial/Unknown を担う既存設計に従う。
- `synvelis-app` の `service.rs` は共有ファイル → additive 変更のみ、#112 領域に触れない。

## Task Breakdown

- Task 1: `synvelis-recording` 統合 fixture（harness + 全テストケース）
- Task 2: `synvelis-app` recording 状態投影 + fixture
- Task 3: 検証・文書・impl 記録

## Verification

- [ ] `mise run fmt-check` Pass
- [ ] `mise run check` Pass
- [ ] `mise run lint` Pass
- [ ] `mise run test` Pass（新規 fixture を含む）
- [ ] `mise run test-rt` Pass
- [ ] fake port であることがテスト・doc・PR で明示されている

## Open Questions / Gaps

- `synvelis-app` の recording command surface は本 slice では投影のみとし、command は #114 側の領域として deferred（spec の Out of Scope に記載済み）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution
- Independent work streams: none（逐次）
- User approval required before dispatch: yes（worker 契約により review-protocol の blocker 無しを承認として扱う）
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- reviewer は fixture が実 `StreamWriter`/staging を経由していること、fake port 明示、fail-stop の Verified Prefix/capture-fault 観測、`synvelis-app` 変更が additive で #112 領域に触れないことを確認する。
