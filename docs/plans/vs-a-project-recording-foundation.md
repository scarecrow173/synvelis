---
id: "PLAN-034QdZoc84FQ9sKQSfLWtn"
type: "plan"
status: "approved"
title: "VS-A Project/Recording Foundation 実装計画"
created: "2026-11-01"
updated: "2026-11-01"
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
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vs-a-f0-gate-stable-id-command-save-reload-recording.md"
  references: []
  defers: []
  deferred-by: []
---

# VS-A Project/Recording Foundation 実装計画 Implementation Plan

## Goal

Issue #13 の F0 Gate: Cargo Workspace 新設・Stable ID・時間型・原子的
Command/Undo・Save/Reload・Recording 参照モデルを headless 実装する。

## Scope

単一実装ストリーム（1 plan / 1 worktree）。3 クレート分割のみで、
複数 plan への分割は不要。

## Upstream Documents

- `docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md`（approved）
- `docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md`（accepted）

## Design Inputs

- `docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md`（approved）

## File Map

- `Cargo.toml` - workspace 正本（members/workspace.package/workspace.lints/profiles）
- `Cargo.lock` - 生成物・追跡（Ruviform git resolved revision を記録）
- `mise.toml` - `rust = "1.99.0"` と fmt/fmt-check/check/lint/test/build/test-msrv タスク
- `crates/synvelis-project/Cargo.toml`・`src/lib.rs` - 公開 API 集約
- `crates/synvelis-project/src/id.rs` - typed ID・IdGenerator・compile_fail doctest
- `crates/synvelis-project/src/time.rs` - SessionFrame/SourceFrame/MusicalTick/ProjectTime/MediaTimeValue
- `crates/synvelis-project/src/range.rs` - Range<F>/SessionRange/SourceRange/MusicalRange/MediaSpan
- `crates/synvelis-project/src/tempo.rs` - TempoMapData/TempoMapRef/TempoEventDto/SignatureDto/ProjectPpqn/TickTimeMapping
- `crates/synvelis-project/src/project.rs` - Project/Track/Lane/Clip/Source/Asset/ClockEpoch/RecordingRegistry POD/SessionState/TakeState
- `crates/synvelis-project/src/recording_data.rs` - RecordingSession/Pass/Stream/Segment/Take POD
- `crates/synvelis-project/src/controller.rs` - Operation trait/Command<O>/ProjectController<O>/ProjectRevision/SavedState
- `crates/synvelis-project/src/ops.rs` - ProjectOp variants + validate/apply
- `crates/synvelis-project/src/store.rs` - ProjectStore/SnapshotEnvelope/DirectoryBundleStore/InMemoryStore
- `crates/synvelis-recording/Cargo.toml`・`src/lib.rs` - RecordingOp/SynvelisOp/遷移表/validate
- `crates/recording-poc/Cargo.toml`・`src/main.rs` - 決定的シナリオ bin
- `crates/synvelis-project/tests/*.rs`・`crates/synvelis-recording/tests/*.rs` - AC 網羅テスト

## Implementation Sequence

- [ ] task1: workspace/mise/dependency 骨格（Cargo.toml、mise.toml、3 クレート stub、cargo fetch で lockfile 生成）
- [ ] task2: `synvelis-project` の id/time/range/tempo/project POD + serde（compile_fail doctest 含む）
- [ ] task3: `synvelis-project` の controller/ops/store（Operation/Undo/Save/Load/Envelope strict）
- [ ] task4: `synvelis-recording` の RecordingOp/SynvelisOp/状態遷移/参照検証
- [ ] task5: `recording-poc` 決定的シナリオ + AC 網羅テスト追加
- [ ] task6: 検証・merge origin/main・再生成・impl record

## Dependencies and Risks

- Ruviform git 依存の初回 fetch にネットワーク必須（既定 branch 追従、rev 固定なし）。
- `ruviform-media-core` に serde impl が無い → Synvelis DTO で対応済（design）。
- MSRV 1.89 で `edition2024` OK（rust 1.85 以降で stabilized）。

## Task Breakdown

- task1 → task2 → task3 → task4 → task5 → task6（直列、task4 は task3 の
  Operation 境界に依存、task5 は task4 に依存）

## Verification

- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`build` 全緑
- [ ] `cargo test --workspace --locked`（mise test 経由）が lockfile 再現
- [ ] `recording-poc` を 2 回実行し同一 stdout
- [ ] 全 spec AC を網羅するテスト（test-spec の AC→test 対応表に一致）

## Open Questions / Gaps

- なし（レビューで確定済の設計を実施するのみ）

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（subagent 分割不要・単一 worktree 直列）
- Independent work streams: none
- User approval required before dispatch: no（workspace 契約で review-protocol
  が承認 gate を代替、design approved 済）
- If approved: task-doc 単位で順次実装、各 task 完了時に関連テスト実行。

## Review Handoff

- 重点: spec AC の全項目が task5 のテスト群に落ちているか、Command/O seam
  （generic Operation）が spec の「Single Writer・二重 Store 禁止」を
  守っているか。
