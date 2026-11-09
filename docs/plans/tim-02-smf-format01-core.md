---
id: "PLAN-034UwXgmnKOCYgkwlGLFkf"
type: "plan"
status: "approved"
title: "TIM-02 SMF Format0/1 core implementation plan"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
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
    - "docs/specs/tim-02-smf-format01-import-export.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/rcd-04-comp-midi-phrase-fixture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/designs/tim-02-smf-format01-core.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-02 SMF Format0/1 core implementation plan

## Goal

`synvelis-project` に SMF Format 0/1 の deterministic
parse/serialize・provenance 永続化・typed error・export +
ConversionReport を実装し、`compile_midi_comp` を channel-aware
chase に拡張して、Import/Save/Export/Reimport と phrase 再生の
fixture を `synvelis-project` / `synvelis-recording` tests で
固定する。

## Scope

単一 implementation stream。file map は `synvelis-project`
中心 + recording test 1 本。`synvelis-player`・app/workspace/
library・gateway・Ruviform は対象外（spec の Out of Scope）。

## Upstream Documents

- `docs/specs/tim-02-smf-format01-import-export.md`
- Issue #126（`https://github.com/scarecrow173/synvelis/issues/126`）

## Design Inputs

- `docs/designs/tim-02-smf-format01-core.md`（approved）
- `docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md`
  （`compile_midi_comp` oracle）

## File Map

- `crates/synvelis-project/src/smf.rs` — **新規**。SMF
  document model・parser・serializer・typed error・export +
  `ConversionReport`・`paired_notes`/`controller_events`/
  `validate`。
- `crates/synvelis-project/src/lib.rs` — `smf` module 公開と
  re-export。
- `crates/synvelis-project/src/project.rs` —
  `Project.smf_documents` field、`Project::new` 初期化、
  `validate_references` の SMF 節。
- `crates/synvelis-project/src/ops.rs` —
  `ProjectOp::ImportMidiSmf`、note 編集 op の immutable guard、
  `RemoveMidiSource` の document 連動削除。
- `crates/synvelis-project/src/midi_comp.rs` —
  `MidiControllerEvent.channel`、`(channel, kind)` chase、
  `MidiPlanAction::ControllerChase.channel`、region report
  chase 表現。
- `crates/synvelis-project/src/store.rs` —
  `SCHEMA_VERSION` 8→9。
- `crates/synvelis-project/src/migration.rs` — v8→v9 step
  （identity）。
- `crates/synvelis-project/tests/tim_02_smf.rs` — **新規**。
  Format0/1 parse、複数 track/channel、PPQN mismatch 変換、
  tempo/CC64/event order、NoteOff→On、malformed/overflow/
  unsupported、Export→Reimport golden、chase/panic、
  immutable guard、schema v9 roundtrip。
- `crates/synvelis-project/tests/rcd_04_midi_phrase.rs` —
  `MidiControllerEvent` literal に `channel` を追加。
- `crates/synvelis-recording/tests/tim_02_midi_phrase.rs` —
  **新規**。SMF import → phrase plan → 発音残り 0。

## Implementation Sequence

- [ ] T1: `smf.rs` document model + parser（Format0/1、
  typed error、running status 解決、meta/sysex 保存）。
- [ ] T2: `paired_notes` / `controller_events` / `validate` /
  serializer + `export_smf` + `ConversionReport`。
- [ ] T3: `Project.smf_documents` + `validate_references` +
  schema v9 + migration step。
- [ ] T4: `ProjectOp::ImportMidiSmf` + immutable guard +
  `RemoveMidiSource` 連動削除。
- [ ] T5: `midi_comp.rs` channel-aware chase + 既存 test 更新。
- [ ] T6: `tests/tim_02_smf.rs` fixture 群（project）。
- [ ] T7: `tests/tim_02_midi_phrase.rs` fixture（recording）。
- [ ] T8: `mise run fmt-check` → `check` → `lint` → `test` →
  `test-rt` → `test-msrv` → `build`、review-protocol、impl
  record、PR。

## Dependencies and Risks

- `origin/main` `9798be5` 以降を前提（PR #219 の
  `compile_midi_comp` 済み）。最終検証前に
  `git fetch origin main && git merge origin/main`。
- `SmfDocument` の serde 永続化は `deny_unknown_fields`
  規約に従う — 将来 field 追加時は schema 前進が必要。
- `paired_notes` の unclosed note 厳格 error は実 file で
  reject されうる → typed error で明示（design 記載済み）。
- doc router blocked（pre-existing broken relation）→
  chain 文書で進行、impl record に記録（RCD-04 先例）。

## Task Breakdown

- TSK 単一 task（T1–T8 を 1 task doc に集約）。

## Verification

- [ ] `cargo test -p synvelis-project tim_02` 全緑
- [ ] `cargo test -p synvelis-recording tim_02` 全緑
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `build`（存在するもの全て）
- [ ] Export→Reimport golden・PPQN mismatch report・
  malformed/unsupported typed error を fixture で確認
- [ ] review-protocol で blocker なし

## Open Questions / Gaps

- なし（design で確定済み）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（本 worker が直接実装）。
- Independent work streams: none。
- User approval required before dispatch: yes（worker 契約により
  review-protocol を承認として扱う）。
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute task-by-task
  with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- `SmfDocument` の serde shape・schema v9 migration・immutable
  guard の範囲・`(channel,kind)` chase の後方互換（plan-level
  型のみ変更、外部 consumer なし）を reviewer が確認。
