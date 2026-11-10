---
id: "TASK-034UwY7u8viaAzE6tUxLHa"
type: "task"
status: "done"
title: "TIM-02 Task1 SMF Format0/1 project+recording headless slice"
created: "2026-11-09"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/tim-02-smf-format01-core.md"
  implemented-by: []
  depends-on:
    - "docs/plans/tim-02-smf-format01-core.md"
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
    - "docs/test-specs/tim-02-smf-format01-fixtures.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-02 Task1 SMF Format0/1 project+recording headless slice

## Work

spec `tim-02-smf-format01-import-export` /
design `tim-02-smf-format01-core` /
plan `tim-02-smf-format01-core` /
test-spec `tim-02-smf-format01-fixtures` に従い、
`synvelis-project` の SMF Format0/1 core と
`synvelis-recording` の Take/Phrase 消費 fixture を実装する。

1. `synvelis-project/src/smf.rs`（新規）: `SmfDocument`/
   `SmfTrack`/`SmfEvent`/`SmfEventKind`/`SmfFormat`/
   `SmfError`/`SmfExportOptions`/`SmfFile`/
   `SmfConversionReport`/`SmfRational` と
   `parse_smf`/`export_smf`/`paired_notes`/
   `controller_events`/`validate`。
2. `synvelis-project/src/lib.rs`: `smf` module 公開 +
   re-export。
3. `synvelis-project/src/project.rs`:
   `Project.smf_documents` field・`Project::new` 初期化・
   `validate_references` の SMF 節。
4. `synvelis-project/src/ops.rs`:
   `ProjectOp::ImportMidiSmf`・note 編集 op の immutable
   guard・`RemoveMidiSource` の document 連動削除。
5. `synvelis-project/src/midi_comp.rs`:
   `MidiControllerEvent.channel`・`(channel,kind)` chase・
   `MidiPlanAction::ControllerChase.channel`・region report
   chase 表現。
6. `synvelis-project/src/store.rs`: `SCHEMA_VERSION` 8→9。
7. `synvelis-project/src/migration.rs`: v8→v9 step。
8. `synvelis-project/tests/tim_02_smf.rs`（新規）: test-spec
   の Rule/Example を網羅する fixture。
9. `synvelis-project/tests/rcd_04_midi_phrase.rs`: 既存
   `MidiControllerEvent` literal に `channel` を追加。
10. `synvelis-recording/tests/tim_02_midi_phrase.rs`（新規）:
    SMF import → `compile_midi_comp` → 発音残り 0。
11. `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
    `test-msrv`/`build` で検証し `docs/impl/ir/` に
    implementation record を残す。

## Done When

- [x] test-spec `tim-02-smf-format01-fixtures` の Rule/Example
  が全て実測 Pass（`tim_02_smf` 28 test・`tim_02_midi_phrase`
  2 test 全て Pass）
- [x] `synvelis-player`/`synvelis-app`/`synvelis-workspace`/
  `synvelis-library`/Ruviform 側に diff がない
  （`synvelis-gateway/src/prepare.rs` は `ProjectOp` の
  exhaustive `op_name` match 追従のみ — impl record 記載の
  機械的変更）
- [x] impl record
  `docs/impl/ir/tim-02-smf-format01-import-export.md` に
  pass/fail/NotRun/Unsupported と実行した検証コマンド・
  exit status を正確に記録

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/tim-02-smf-format01-core.md`
- `docs/specs/tim-02-smf-format01-import-export.md`
- `docs/designs/tim-02-smf-format01-core.md`
- `docs/test-specs/tim-02-smf-format01-fixtures.md`

## Dependencies

- `origin/main` `9798be5`（PR #219 `compile_midi_comp` 済み）
  以降。最終検証前に `git fetch origin main && git merge
  origin/main`。

## Affected Files

- `crates/synvelis-project/src/{smf.rs,lib.rs,project.rs,
  ops.rs,midi_comp.rs,store.rs,migration.rs}`
- `crates/synvelis-project/tests/{tim_02_smf.rs,
  rcd_04_midi_phrase.rs}`
- `crates/synvelis-recording/tests/tim_02_midi_phrase.rs`
- `docs/` chain（spec/design/plan/test-spec/task/impl）

## Verification

- [x] `mise run fmt-check` — Pass
- [x] `mise run check` — Pass
- [x] `mise run lint` — Pass（warning 0）
- [x] `mise run test` — Pass（workspace 全 test、exit 0。
  2026-11-10 に `origin/main` `5690844` + docs-only 差分で
  再検証。初回・2 回目は `synvelis-library` webdav
  `same_origin_redirect_is_followed` が Windows socket
  error 10053 で flake（TIM-02 と無関係、単独実行は Pass）。
  3 回目で全 test binary exit 0）
- [x] `mise run test-rt` — Pass
- [x] `mise run test-msrv` — Pass（rust 1.89.0）
- [x] `mise run build` — Pass
- [ ] 実 OS/Device/GUI・実 RT engine・SMPTE/MIDI2 — NotRun /
  Unsupported（test-spec 宣言どおり後続 slice）
