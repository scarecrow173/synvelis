# Acceptance Ledger — synvelis#280 CB-03 MIDI Clip/Source headless スライス

Date: 2026-11-14. Slice: CB-03 `VariantScope::MidiClip` 実
ドメイン Adapter（`MidiClipAdapter`）+ MIDI note/source op の
scope confinement + headless fixture/回帰テスト。
残ドメイン Adapter（CB-04〜08 = #281〜#285）・Compare
Orchestrator（#285）・GPUI（#286）・Action/Gateway 統合
（#287）・Cross-Gate/実機受入（#288）は DEFER。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/280
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | scarecrow173/sv280-cb03 HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 / Intel Core i9-9900K |
| Toolchain | mise 管理 Rust（`mise run` 経由） |
| Project schema | v14（schema 非変更 — 永続 model への entity 追加なし） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-project/tests/
  cb03_midi_clip.rs` が `mise run test synvelis-project` で
  green。
- **NotRun**: 実 OS/実 Plugin/実 RT/実 Render/実 MIDI
  device/実デバイス入力 pipeline 依存の判定 — 本スライスでは
  実行していない。
- **Deferred**: 後続 Issue の範囲として明示的に送った部分。
- **Unsupported**: project 表現または engine surface が存在
  せず、型付き拒否が正しい verdict のもの。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| CB-MIDI-RCP-01 | Pass (headless) | `cb_midi_04_rcp_01_context_pin_scoped_invalidation` — scoped candidate 編集は当該 candidate の `BranchRenderPin.scope_digest` のみを移動し、adopted(live mirror) branch と `shared_context_digest` は不変 → 依存 Render は編集候補のみ失効。scope 外 live 編集は全 branch の shared-context digest を移動 → 旧 context 比較は Stale。native event fidelity（division/tick/SMF provenance）は candidate・render 解決 view の双方で不変、adopt は 1 CAS/Undo（`cb_midi_05`）。実 Render 再利用評価・invalidation 実行本体は RCP-01/`synvelis-render` 側・Deferred。 |
| CB-MIDI-01 | Pass (headless) | `cb_midi_01_candidate_contents_isolated_across_lifecycle` — 同一 MIDI source 由来の 3 scoped content（fork origin→B′ adopt→A′ freeze 編集）を pairwise 異なる scope digest で実演し、B の note/velocity 編集が live・adopted mirror・frozen candidate・raw `midi_sources`/`smf_documents`（serialize 比較で byte 一致）を変更しないことを確認。`cb_midi_01_sibling_midi_scope_isolated` — 別 MidiClip scope の sibling candidate/adopted も相互不変。literal な同時 3 branch set は engine の branch 追加 op 未実装のため Deferred（データモデルは `BTreeMap` で対応済み・E2 参照）。 |
| CB-MIDI-02 | Pass (headless) | `cb_midi_02_note_diff_matches_stable_lineage` — velocity/move/add が stable `NoteId` で `Modified`(field evidence: `velocity`/`start`)・`Added` に分類され exactly 3 changes。`cb_midi_02_delete_add_never_merges_into_modified` — delete+add pair は `Removed`+`Added` に分離され `Modified` に誤 merge されない。`cb_midi_02_ambiguity_is_typed_not_merged` — owner 変更は `OwnerConflict`、raw id 衝突は `CrossKindId` の `AmbiguousIdentity` issue で型付き Unknown（Equal/誤差分に丸めない）。branch-local 新規 `NoteId` は `ReservingIdGenerator` で live-space 予約され live と衝突しない。 |
| CB-MIDI-03 | Pass (headless) | `cb_midi_03_smf_native_fidelity_survives_save_reload_and_fork` — SMF Format0（division 480・奇数 tick 37/101・1001 系・CC64・pitch bend・sysex・tempo/time-sig meta）と Format1（division 96・3 track・tempo map 2 件）の import→fork→save→reload で `smf_documents`・`midi_sources.division`・投影 note の start/len が完全一致。project PPQN(960)・他 source division への requantize/quantize/丸めなし。実 MIDI device capture・ハードウェア往復は NotRun/Deferred。 |
| CB-MIDI-04 | Pass (headless・identity 層) | `cb_midi_04_rcp_01_context_pin_scoped_invalidation` — 同一 shared-context（instrument/FX/tempo は scope 外 live object）で 2 branch の pin が一致し、scoped 編集は candidate scope_digest のみ移動、scope 外編集は shared_context_digest を移動して比較を Stale にする型付き semantics を証明。実 Instrument/FX render の音響差分確定性は NotRun/Deferred（比較 orchestration 本体は CB-08 #285、実 Plugin は #114・後続 Gate）。Missing Instrument での MIDI 編集可否は op surface 上自明（note op は instrument 非依存）だが実音確認は Deferred。 |
| CB-MIDI-05 | Pass (headless) | `cb_midi_05_adopt_is_single_cas_with_undo_redo` — Adopt B が live revision +1 の 1 CAS で scoped entity のみ materialize（velocity 変更 + note 追加・clip source_id 不変）。undo で origin live 復元・variant set は 2 branch 維持・candidate content ref 不変、redo で B′ 復帰。対象 clip 以外の MIDI source/SMF document/他 clip は不変（`midi_state_json` serialize 比較）。 |
| CB-MIDI-06 | Pass (headless) | `cb_midi_06_capabilities_match_enforcement` — `MidiClipCapabilities` が note_add/move/velocity=Supported・他 10 項目=Unsupported を報告（display = enforcement）。`cb_midi_06_unsupported_ops_reject_typed_and_preserve_state` — RemoveMidiNote/AddMidiSource/RemoveMidiSource/DetachClipSource/Loop・SimpleStretch mapping/存在しない source 宛 op が全て型付き拒否で candidate content ref・live state 不変。`cb_midi_06_smf_backed_source_rejects_candidate_note_ops` — SMF-backed source の 4 種 note op が op 契約の typed `Project` 拒否、candidate 内 detach は `Unsupported`。CC/PitchBend/CC64・key/len・MIDI2/未知 event の編集 op は `ProjectOp` surface に存在せず `Unsupported` が正直な verdict — native stream は `SmfDocument` provenance に保持・削除/無断変換なし。 |

## 補足: scope validation 回帰

`cb_midi_scope_validation_mirror_and_shared_source_reject` —
media clip への `MidiClip` scope が `InvalidScope`/`Project`
で拒否（`AudioClipAdapter` 誘導拒否の鏡像）、存在しない
clip が拒否、2 clip 共有 MIDI source が ambiguous ownership
で拒否（CB-01 契約を本 adapter の `validate_scope` →
`scope_objects` 経路で再確認）。

## NotRun / Deferred 一覧

- 実 OS provider・実 Plugin host・実 RT・実 Render
  pipeline・実 MIDI device/SMF capture・実デバイス入力
  pipeline・workspace/GPUI projection: NotRun（headless
  fixture のみ。Fake/Domain Oracle の PASS を実機 PASS に
  昇格しない）。
- CB-MIDI-04 の実 Instrument/FX render を伴う音響差分
  確定性・Missing Instrument での保存済み Render Stale の
  実音確認: Deferred — 本スライスは pin identity/staleness
  semantics まで。実 Plugin は #114・後続 Gate の責務。
- literal な同時 N>2 branch VariantSet: Deferred — engine の
  set 内 branch 追加 op が必要（データモデルは対応済み）。
- CC/PitchBend/CC64・note key/len・MIDI2/未知 event の候補
  編集: Unsupported — `ProjectOp` surface 未実装。native
  stream は `SmfDocument` provenance に保持。
- 残ドメイン Adapter（CB-04〜08 = Issue #281〜#285）・
  Compare Orchestrator（#285）・GPUI（#286）・Action/Gateway
  統合（#287）・Cross-Gate/実機受入（#288）: Deferred。
- 実測なしの性能保証・Semantic Merge/Blend・全 Branch 常時
  RT 駆動: 対象外（Issue 本文の「対象外」どおり）。
