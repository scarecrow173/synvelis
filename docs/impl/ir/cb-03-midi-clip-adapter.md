---
id: "IMPL-034XkoKBFx6VCR0iVHzGas"
type: "impl"
status: "completed"
title: "CB-03 MIDI Clip/Source 候補 Adapter — Native Event 保持・Note Diff・同一 Instrument 比較（synvelis#280）"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/280"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/variant/midi_clip.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb03_midi_clip.rs"
      - type: "file"
        path: "docs/impl/ir/cb-03-midi-clip-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-03-midi-clip-adapter.md"
      - type: "file"
        path: "docs/test-specs/cb-03-midi-clip-adapter.md"
      - type: "file"
        path: "evidence/sv280/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv280/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/variant/adapter.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/audio.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/creative-branch.md"
    - "docs/designs/creative-branch-design.md"
    - "docs/adr/cb-00-creative-branch.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/cb-02-audio-clip-adapter.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/cb-03-midi-clip-adapter.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/cb-03-midi-clip-adapter.md"
    rejected: []
---

# CB-03 MIDI Clip/Source 候補 Adapter — Native Event 保持・Note Diff・同一 Instrument 比較（synvelis#280）

## Summary

`VariantScope::MidiClip` の実ドメイン `DomainAdapter`
（`MidiClipAdapter` + `MidiClipCapabilities`）を
`crates/synvelis-project/src/variant/midi_clip.rs` に実装し、
共有 confinement helper（`check_op_targets` /
`check_created_within_scope`）へ MIDI note/source op の
target 解決と新規 entity 検査を追加した。native
`MidiSource.division`/`MidiTick`・`SmfDocument` provenance・
event stream を無変更で保持し、note add/move/velocity の
scope 閉じ込め編集・canonical NoteId lineage diff・
same-context render pin（scope/shared-context digest）による
staleness semantics・1 CAS + 1 Undo adopt・capability 誠実性を
headless fixture 12 件で検証した。

## Context

synvelis#280 CB-03 スライス。CB-00（#422）統合設計・CB-01
（#278, PR #425）VariantSet/Branch/Scope/engine・CB-02
（#279, PR #437）`AudioClipAdapter`/`TakeUsageAdapter`・
RCP-00/01/02（#421/#431/#436）の read adapter 契約を前提とする。
本スライスは headless の実ドメイン Adapter + fixture に限定
— Compare Orchestrator（#285）・GPUI（#286）・Action/Gateway
統合（#287）・実機受入（#288）は後続 Issue。

## Implementation

### Changes

- `src/variant/midi_clip.rs`（新規）: `MidiClipAdapter` —
  `validate_scope` は `validate_scope_structure` + clip 存在 +
  clip が MIDI source を読むことの確認 + `scope_objects`（MIDI
  source 排他性 = CB-01 ambiguous ownership 拒否を継承）。
  media clip への `MidiClip` scope は `InvalidScope` で鏡像拒否
  （`AudioClipAdapter` の誘導拒否の対称）。`fork_candidate` は
  `fork_candidate_project` 共用。`edit_candidate` は
  `BranchEditOp::Project` のみ受理し、MIDI note op 4 種
  （Add/Remove/SetVelocity/Move）に限り op 自身の
  `Operation::validate` を confinement 前に再実行 — 共有
  confined flow は apply 時に validate しないため、
  `reject_smf_backed`（SMF provenance source の note 不変
  契約）を candidate 内でも enforce する。`preflight_adopt`
  は lane recording-open gate（CB-02 先例）+ candidate clip
  の source_id が live と一致する source-identity gate。
- `MidiClipCapabilities`: note_add/move/velocity=Supported、
  note_remove/key/len/cc/pitch_bend/smf_backed_notes/
  detach_in_candidate/loop_playback/stretch/midi2_unknown_
  events=Unsupported の誠実な能力報告（display truth =
  enforcement truth、CB-MIDI-06）。
- `src/variant/adapter.rs`: `check_op_targets` に MIDI note
  op の target 解決を追加 — `AddMidiNote` は scope 所有
  `MidiSource`、`RemoveMidiNote`/`SetMidiNoteVelocity`/
  `MoveMidiNote` は `MidiSource`+`MidiNote` の双方を
  `require_scope` で検査。`AddMidiSource`/`RemoveMidiSource`/
  `ImportMidiSmf`/`DetachClipSource` は scope 外 entity 生成・
  source 再指向のため `Unsupported` のまま fail-closed。
  `check_created_within_scope` は新規 `MidiSource`/`MidiNote`
  id を scope owned set と照合 — 8 引数化を避けるため
  before 集合を `PreEditIds` struct に集約（clippy
  too_many_arguments 対応・責務は不変）。
- `src/variant/audio.rs`: `playback_mapping_capability`/
  `clip_edit_guard`/`lane_recording_open` を `pub(crate)` 化
  — clip-generic な playback guard・recording gate・
  capability 判定を MIDI adapter が複製せず再利用するため
  （confinement ルールの drift 防止、helper docstring 契約）。
- `src/variant/mod.rs`: `mod midi_clip` + `MidiClipAdapter`/
  `MidiClipCapabilities` re-export。
- `tests/cb03_midi_clip.rs`（新規）: 12 fixture —
  CB-MIDI-01×2・CB-MIDI-02×3・CB-MIDI-03・CB-MIDI-04/RCP-01・
  CB-MIDI-05・CB-MIDI-06×3・scope validation mirror。

### Decisions During Implementation

- **`SmfDocument` は scope object に含めない**: `midi_clip_
  objects` が所有するのは Clip・exclusive `MidiSource`・
  `MidiNote`・clip-local `Note`・control closure のみ。
  `smf_documents` は `SourceId` key の shared provenance で
  あり、adopt が import stream を書き換えない native-fidelity
  原則（CB-MIDI-03/06）に従い scope 外のまま。candidate
  payload は full project clone のため provenance は
  candidate にも保持される。
- **「3候補」の解釈**: engine の `VariantSet` は adopted +
  candidate の 2 branch を fork ごとに生成し、set 内への
  追加 branch op は未実装（データモデル上は可能だが
  engine surface 拡張は本スライス範囲外）。CB-MIDI-01 は
  fork → edit → adopt → freeze → edit の lifecycle で
  origin/B′/A′ の 3 つの distinct scoped content（pairwise
  異なる scope digest）を実演し、別 scope の sibling
  candidate との相互不変も検証した。literal な同時 3
  branch set は後続 engine slice の課題として exp/ledger
  に記録。
- **SMF-backed source の candidate 編集**: note op は op
  自身の `reject_smf_backed` 契約で typed `Project` 拒否
  （`MidiSource.notes` は `SmfDocument` の投影であり
  document が authoritative）。editable copy は live での
  `DetachClipSource`（fresh SourceId/NoteId・provenance
  除去）のみ — candidate 内 detach は scope 再指向となる
  ため `Unsupported`。
- **scope confinement の共有化**: MIDI op target 解決は
  adapter 固有実装へ複製せず `check_op_targets`/
  `check_created_within_scope` を拡張 — Fake/Audio/MIDI
  全 adapter が同一 confinement 規則を共有し drift しない
  （CB-02 が確立した helper docstring 契約どおり）。
- **same-instrument 比較の提供形**: 比較 orchestration 本体
  は CB-08（#285）。本スライスは `BranchRenderPin` の
  `scope_digest`（scope 内容 identity）+ `shared_context_
  digest`（scope 外 live semantic object = instrument/FX/
  tempo 等の共有 context）の pin semantics を headless で
  証明 — scoped 編集は candidate の scope_digest のみを動かし
  shared context を動かさず、scope 外編集は全 branch の
  shared_context_digest を動かして比較を Stale にする。

## Related Experiments

### Adopted

- `docs/impl/exp/cb-03-midi-clip-adapter.md` — `SmfDocument`
  の scope 外扱い・「3候補」lifecycle 解釈・SMF-backed
  pre-validate 配置の設計検討（いずれも採用）。

### Rejected

- なし（重大な代替案比較は exp E1〜E3 に記録）。

## Validation

Windows 11 / mise 管理 toolchain で実行（詳細は
`evidence/sv280/verification.md`）:

- `mise run fmt-check` — Pass
- `mise run check synvelis-project` — Pass（`--locked`）
- `mise run lint synvelis-project` — Pass
- `mise run test synvelis-project` — Pass（45 test binary・
  全 green、新規 12 fixture 含む、CB-01/02 回帰なし）
- `mise run check synvelis-workspace` — Pass
- `mise run check synvelis-render` — Pass

## Risks

- literal な同時 N>2 branch を持つ VariantSet は engine が
  未対応 — CB-MIDI-01 の「3候補」は lifecycle 実演で代替
  しており、set 内 branch 追加 op が必要になった場合は
  engine slice での対応が必要（データモデルは既に map）。
- CC/PitchBend/CC64・note key/len・MIDI2/未知 event の編集
  surface は `ProjectOp` に存在しない — `Unsupported` が
  正直な verdict であり、native stream は `SmfDocument`
  provenance に保持されるが候補編集はできない（CB-MIDI-06
  の契約どおり）。これらの編集 op 追加は別 Issue。
- 実 Instrument/FX render を伴う音響比較の確定性・実 MIDI
  device/SMF capture・RT lanes は NotRun/Deferred —
  headless PASS を実機 PASS に昇格しない。

## Follow-ups

- CB-04〜08（#281〜#285）残ドメイン Adapter・Compare
  Orchestrator — 本 adapter の pin semantics
  （scope/shared-context digest）を比較 context identity
  として消費する。
- engine の set 内 branch 追加 op（複数同時 candidate）—
  必要になった時点で別 slice。
- MIDI note key/len 編集・CC/automation 候補 surface —
  `ProjectOp` 拡張が前提の別 Issue。
