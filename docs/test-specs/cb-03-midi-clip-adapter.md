---
id: "TSPEC-034XkQVtkDaUEeWhxjkiAf"
type: "test-spec"
status: "approved"
title: "CB-03 MIDI Clip/Source 候補 Adapter fixture tests"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/280"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/creative-branch-design.md"
  derived-by: []
  verifies:
    - "docs/specs/creative-branch.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# CB-03 MIDI Clip/Source 候補 Adapter tests

## Purpose

synvelis#280 CB-03 スライスの headless fixture が保証すべき
振る舞いを固定する。`VariantScope::MidiClip` の実ドメイン
`DomainAdapter`（`MidiClipAdapter`）が、native MIDI/SMF
データを保持したまま note 編集候補・canonical note diff・
same-context 比較 identity・adopt・capability 誠実性を行う
契約を検証する。実 OS/実 Plugin/実 RT/実 MIDI device/
実 Render・workspace/GPUI projection に依存する判定は本
spec の対象外（`evidence/sv280/` で NotRun/Deferred 記録）。
spec/design が CB-04 以降で再設計された場合でも、ここに
記す不変条件（native fidelity・scope 閉じ込め・fail-closed・
capability 誠実性・lineage diff）は維持されるべき保証であり、
本 test spec はその判定基準として残す。

## Feature

`MidiClipAdapter` による `VariantScope::MidiClip` の branch
candidate 編集と、`adopt_branch_adapted` 経由の scoped
entity materialize。candidate は live-space 予約 ID・CAS
revision・scope digest を CB-01 契約どおり継承し、
`MidiSource.division`/`MidiNote` の native tick・
`SmfDocument` provenance/event stream を無変更で保持する。

## Rules

- **Rule CB-MIDI-SCOPE**: `MidiClip` scope は clip が排他的に
  読む MIDI source のみを所有し、media clip・存在しない
  clip・共有 source（ambiguous ownership）は fork 時に型付き
  拒否。scope 外の clip/source/note を対象とする op は
  `ScopeViolation`/`Unsupported` で live・candidate の
  いずれも変更しない。
  Examples: `cb_midi_scope_validation_mirror_and_shared_source_reject`,
  `cb_midi_01_sibling_midi_scope_isolated`,
  `cb_midi_06_unsupported_ops_reject_typed_and_preserve_state`.
- **Rule CB-MIDI-NATIVE**: `MidiSource.division`・note の
  source-native `MidiTick` start/len・`SmfDocument` の
  format/division/ordered event stream（tempo map・meta・
  CC・sysex）は fork・candidate 編集・adopt・save/reload を
  通じて無変更。project PPQN への requantize・無断
  quantize/丸めは発生しない。
  Examples: `cb_midi_03_smf_native_fidelity_survives_save_reload_and_fork`.
- **Rule CB-MIDI-ISOLATION**: 同一 MIDI source 由来の複数
  scoped content（fork した候補・adopt 後の freeze 候補・
  別 scope の sibling 候補）の編集は相互に分離され、live
  および raw native source（`midi_sources`/`smf_documents`）
  を candidate 編集だけでは変更しない。
  Examples: `cb_midi_01_candidate_contents_isolated_across_lifecycle`,
  `cb_midi_01_sibling_midi_scope_isolated`.
- **Rule CB-MIDI-DIFF**: note の canonical diff は stable
  `NoteId` lineage に一致し、Move/Velocity 変更は `Modified`
  （field evidence）、Delete+Add は `Removed`+`Added` に
  分離され誤って merge されない。owner 変更は
  `OwnerConflict`、raw id 衝突は `CrossKindId` の型付き
  ambiguity であり Unknown を Equal/ゼロ差分に丸めない。
  Examples: `cb_midi_02_note_diff_matches_stable_lineage`,
  `cb_midi_02_delete_add_never_merges_into_modified`,
  `cb_midi_02_ambiguity_is_typed_not_merged`.
- **Rule CB-MIDI-CONTEXT**: 同一 instrument/FX/tempo 等の
  共有 context でのみ候補比較 identity（`BranchRenderPin`
  の scope/shared-context digest）が一致する。scoped 編集は
  当該 candidate の scope digest のみを動かし、scope 外の
  live 編集は全 branch の shared-context digest を動かして
  旧 context の比較を Stale にする — Exact 再表示しない。
  Examples: `cb_midi_04_rcp_01_context_pin_scoped_invalidation`.
- **Rule CB-MIDI-ADOPT**: Adopt は宣言された MidiClip scope
  のみを 1 CAS + 1 Undo で live に反映し、対象 clip 以外の
  MIDI source・SMF document・他 clip を変更しない。
  undo/redo で採用内容と両 branch（adopted/candidate）が
  維持される。
  Examples: `cb_midi_05_adopt_is_single_cas_with_undo_redo`.
- **Rule CB-MIDI-CAPABILITY**: `MidiClipCapabilities` の
  display verdict と edit enforcement が一致し、CC/
  PitchBend/CC64・note key/len・note remove・SMF-backed
  note 編集・candidate 内 detach・`Loop`/`SimpleStretch`
  mapping・MIDI2/未知 event は `Unsupported`/型付き拒否で
  保持され、削除・無断変換・成功の偽装をしない。
  Examples: `cb_midi_06_capabilities_match_enforcement`,
  `cb_midi_06_unsupported_ops_reject_typed_and_preserve_state`,
  `cb_midi_06_smf_backed_source_rejects_candidate_note_ops`.

## Examples

- `cb_midi_01_candidate_contents_isolated_across_lifecycle`:
  fork→B 編集（velocity/move/add）→ live・adopted mirror・
  raw source 不変を確認 → adopt → frozen A 編集 → live=B′
  不変・3 distinct scope digest（origin/B′/A′ pairwise 相異）。
- `cb_midi_01_sibling_midi_scope_isolated`: 別 MidiClip scope
  の sibling candidate/adopted が set 1 の編集で bitwise 不変。
- `cb_midi_02_note_diff_matches_stable_lineage`: velocity/move/
  add が `Modified`(field evidence)/`Added` に分類。
- `cb_midi_02_delete_add_never_merges_into_modified`: crafted
  delete+add が `Removed`+`Added` に分離。
- `cb_midi_02_ambiguity_is_typed_not_merged`: owner 変更=
  `OwnerConflict`、raw id 衝突=`CrossKindId`。
- `cb_midi_03_smf_native_fidelity_survives_save_reload_and_fork`:
  SMF0(480)/SMF1(96) の document・division・奇数 tick が
  fork/save/reload で完全一致。
- `cb_midi_04_rcp_01_context_pin_scoped_invalidation`: scoped
  編集は candidate scope_digest のみ移動、scope 外編集は
  shared_context_digest を移動。
- `cb_midi_05_adopt_is_single_cas_with_undo_redo`: adopt=rev+1
  の 1 CAS、undo/redo で A/B とも維持。
- `cb_midi_06_*`・`cb_midi_scope_validation_mirror_and_
  shared_source_reject`: capability= enforcement、全 unsupported
  op が型付き拒否で状態不変、scope validation 鏡像 + 共有
  source 拒否。

## Fixtures

`crates/synvelis-project/tests/cb03_midi_clip.rs`
（`Fx` harness + `midi_fixture`/`smf_format0_odd_ticks`/
`smf_format1_div96`）。adapter を
`fork_variant_set`/`edit_branch_op`/`adopt_branch_adapted` に
注入し、candidate は `decode_candidate`、render identity は
`resolve_render_project` の `BranchRenderPin`、canonical
diff は `compare_snapshots`（`snap_manual` projection）で
検証する（`cb02_audio_take.rs`/`vcs02_diff.rs` 先例）。

## Guarantees

- native MIDI/SMF データ（division・tick・event order・
  tempo map・meta・sysex provenance）は候補編集・adopt・
  save/reload を通じて不変。
- scope 閉じ込め・fail-closed 拒否・capability 誠実性は
  CB-01/CB-02 契約を継承し、MIDI op surface でも drift
  なく適用される。
- note diff は stable `NoteId` lineage の canonical 分類で
  あり、番号衝突・owner 競合は型付き ambiguity を返す。
- render-reuse identity（scope digest + shared-context
  digest）は scoped 編集の部分失効と context 変更の
  staleness を区別する。

## Non-goals

- Compare Orchestrator 本体（#285/CB-08）・GPUI（#286）・
  Action/Gateway 統合（#287）・Cross-Gate/実機受入（#288）。
- 実 Instrument/FX render を伴う音響差分の確定性・実 MIDI
  device/SMF capture・RT lanes・workspace projection —
  `evidence/sv280/` で NotRun/Deferred。
- literal な同時 N>2 branch VariantSet（engine の branch
  追加 op が未実装 — 「3候補」は lifecycle + sibling scope
  で実演）。
- CC/PitchBend/CC64・note key/len・MIDI2/未知 event の
  候補編集 surface（`ProjectOp` 未実装 — `Unsupported` が
  正直な verdict）。
- Semantic Merge/Blend・全 Branch 常時 RT 駆動・実測なしの
  性能保証（Issue の対象外範囲）。

## Risk

- これらの保証を落とすと、native event の破壊・scope 外
  汚染・note lineage の誤 merge・stale context の Exact
  誤表示・未サポート処理の成功偽装が起こり得る。特に
  native fidelity と fail-closed 拒否は CB-04〜08 の全
  adopt/比較経路と Cross-Gate 受入が依存する土台であり、
  退行は critical path 全体を危うくする。
