---
id: "PLAN-RCD04Fx4kD9sG2hJ6mN8pQ1"
type: "plan"
status: "approved"
title: "RCD-04 Comp Transition / MIDI Phrase crate fixture slice — implementation plan"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 Comp Transition / MIDI Phrase crate fixture slice — plan

## Scope Split

単一 task（crate 層 fixture slice・依存は直列）。docs chain
→ project additive module → project fixtures → recording
fixtures → verification → impl record の順。

## File Map / Dependency Graph

```
docs chain (spec→design→plan→test-spec→task)
        │
        ▼
synvelis-project/src/id.rs         (+MidiCompRegionId)
synvelis-project/src/midi_comp.rs  (new)
synvelis-project/src/lib.rs        (+mod/export)
        │
        ├──▶ tests/rcd_04_midi_phrase.rs      (oracle + stable ID)
        └──▶ tests/rcd_04_comp_cas.rs         (CAS/atomicity/save-reload)
        │
        ▼
synvelis-recording/tests/rcd_04_take_comp_pcm.rs   (record→commit→comp→PCM)
synvelis-recording/tests/rcd_04_recovery_partial.rs (partial/cancel/retry)
        │
        ▼
mise verification → impl record → PR
```

## Task Breakdown

### Task 1: docs chain + project additive module + fixtures

1. `docs/` chain 5 file + 各 README index 更新。
2. `id.rs` に `MidiCompRegionId` 追加、`midi_comp.rs` 新規
   （型 + `validate` + `compile_midi_comp`）、`lib.rs` export。
3. `tests/rcd_04_midi_phrase.rs`: oracle 境界行列 + stable ID。
4. `tests/rcd_04_comp_cas.rs`: CAS/atomicity/undo/save-reload/
   serialized diff/provenance/immutability。

### Task 2: recording fixtures

5. `tests/rcd_04_take_comp_pcm.rs`: vs_d helper 踏襲で
   record→commit→comp 構築、CAS、PCM offline renderer、
   RT↔Offline 差記録、media byte 不変。
6. `tests/rcd_04_recovery_partial.rs`: partial→Hold→Adopt
   retry・冪等・fail-closed・Discard probe 規則。

### Task 3: verification + records

7. `git fetch origin main && git merge origin/main`。
8. `mise run fmt-check` / `check` / `lint` / `test` /
   `test-rt` / `build` を実行し結果を記録。
9. `docs/impl/ir/` に implementation record、task status
   更新、index 再生成（必要なら）。
10. 日本語 PR 作成（Refs #118・Close しない）。

## Risk Register

- `MidiRange`/`MusicalRange` の `Range<F>` は `Copy`・
  `start()/end()` のみ — 長さは `end-start` を typed value
  から計算（`MusicalTick::ticks_since`/`value()`）。
- `MidiTick` は非負 frame 型（`define_frame!`）— phrase
  start/end は `MidiTick::new(i64)` で構築、負値は error。
- `project.tempo_map.ppqn` は `u32` — `compile_midi_comp`
  は `project.tempo_map.ppqn` を分母側ではなく project
  PPQN として使用（`Ppqn(u32)`）。
- recording test の PCM renderer は `EffectiveComp` の
  `project_range`（`ProjectRange`）を `TickTimeMapping`
  経由で media frame に落とす — `Musical`/`Media` 両対応
  （lowering.rs `range_span` と同規則）。
- `Loop` mapping の offline 側再生は `SourceBinding.window`
  の union interval 内 wrap — lowering は skip するため
  「RT=無音・offline=loop 音」の差分として記録。

## Verification Matrix

| step | command | gate |
| --- | --- | --- |
| format | `mise run fmt-check` | exit 0 |
| type | `mise run check` | exit 0 |
| lint | `mise run lint` | exit 0 |
| test | `mise run test` | exit 0（新規 test 含む） |
| rt | `mise run test-rt` | exit 0（存在すれば） |
| build | `mise run build` | exit 0 |

## Rollback

- 全変更は新規 file + `id.rs`/`lib.rs` の additive 行のみ。
  revert は当該 commit の revert で完結し永続データ・
  既存 test に影響しない。
