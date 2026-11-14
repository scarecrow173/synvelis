---
id: "EXP-cb03-midi-clip-adapter"
title: "CB-03 MIDI Clip/Source Adapter 設計実験メモ (synvelis#280)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: CB-03 MIDI Clip/Source headless スライス（synvelis#280）

## E1 `SmfDocument` を scope object に含めるか

**仮説**: `MidiClip` scope が `MidiSource`（投影 notes）を所有
するなら、provenance の `SmfDocument` も scope 所有に含めて
adopt で一緒に materialize すべきかもしれない。

- 案 A（scope 所有に追加）: `ObjectKind::SmfDocument` は diff/
  adopt surface に定義済みだが、`smf_documents` は
  `SourceId` key の shared provenance であり、同一 SMF source
  を読む複数 clip・re-import 関係で所有が一意に定まらない。
  scope object に含めると ambiguous ownership の解釈が
  増え、adopt が import stream を書き換える経路が開ける —
  native event fidelity（event order/meta/sysex 保持）の
  契約に反する。却下。
- 案 B（scope 外・shared provenance のまま）: candidate は
  full project clone なので `smf_documents` は candidate に
  も保持される。adopt は scoped `MidiSource`/`MidiNote` のみ
  transplant し、document は live のものを参照し続ける —
  import stream は一切書き換わらない。SMF-backed source の
  note 不変契約とも整合。

**観察**: 案 B を採用。`cb_midi_03` で candidate・save/reload
双方の `smf_documents` 完全一致と division/奇数 tick の
非量子化を確認。`SmfDocument` を scope object に加える必要が
生じた場合（例: candidate が独自 SMF stream を持つ編集
surface）は `ObjectKind::SmfDocument` の adopt 経路整備と共に
別 slice で扱う。

## E2 「同一MIDI Sourceから3候補をFork」の解釈

**仮説**: CB-MIDI-01 の「3候補」は VariantSet に 3 branch を
同時に持つことを要求するのか。

- 調査: `fork_variant_set` は set + adopted(live mirror) +
  candidate の 3 entity を 1 CAS で生成し、set 内への追加
  branch を作る engine op は存在しない（`VariantOp` は
  Fork/EditAdopted/EditCandidate/Adopt/Discard のみ）。
  `branches` は `BTreeMap` でデータモデル上は N branch を
  許容するが、branch 追加 surface は未実装。
- 案 A（engine に branch 追加 op を実装）: `VariantOp` 新
  variant + public fn + CAS/lineage 設計が必要で、全
  ドメイン adapter（CB-04〜08）共通の engine slice として
  扱うべき変更 — 本タスクの leased 範囲（adapter +
  confinement + fixture）の責務を超える。却下。
- 案 B（lifecycle 実演 + sibling scope）: fork → B 編集 →
  adopt → A freeze → A 編集 で origin/B′/A′ の 3 distinct
  scoped content（pairwise 異なる scope digest・相互不変）
  を実演し、別 MidiClip scope の sibling candidate も同時に
  存在させて cross-candidate 分離を証明。

**観察**: 案 B を採用。`cb_midi_01_candidate_contents_isolated_
across_lifecycle` と `cb_midi_01_sibling_midi_scope_isolated`
で「B の編集が A/C（他候補内容）と raw native source を
変更しない」を headless で検証。literal な同時 3 branch は
engine slice の follow-up として ledger/PR に記録。

## E3 SMF-backed source の candidate note op 拒否の置き場所

**仮説**: `reject_smf_backed`（SMF provenance source の note
不変契約）を candidate 編集経路のどこで enforce するか。

- 背景: 共有 `edit_scoped_project_op` は `Operation::apply`
  を validate 無しで呼び、post-apply `validate_references`
  のみを実行する — live の controller 経路（validate→apply）
  とは非対称。SMF-backed 拒否は `Operation::validate` 内に
  あるため、このままでは candidate 内で SMF document の
  投影 notes が書き換え可能になってしまう。
- 案 A（confinement helper に SMF 判定を追加）: helper は
  scope confinement の責務であり、op 契約（domain 不変条件）
  とは別層 — ここに domain ルールを混ぜると drift 源になる。
  却下。
- 案 B（adapter の `edit_candidate` で note op のみ
  `Operation::validate` を再実行）: adapter が「この op
  surface は op 自身の契約を持つ」ことを明示し、
  `reject_smf_backed`・velocity/key/len domain・参照解決を
  live と同一の判定で enforce。他 op（clip-local・
  routing 系）は post-validate_references + confinement で
  十分（CB-02 と同じ境界）。

**観察**: 案 B を採用。`cb_midi_06_smf_backed_source_rejects_
candidate_note_ops` で 4 種の note op が typed
`VariantError::Project` で拒否され、detach が `Unsupported`
であることを確認。live 側 `DetachClipSource` が唯一の
editable-copy 経路である契約を維持。

## E4 `check_created_within_scope` の引数増加への対処

**仮説**: MIDI source/note の新規 entity 検査追加で helper
が 8 引数になり clippy `too_many_arguments` に抵触。

- 案 A（`#[allow]`）: 禁止事項（lint 隠蔽）に抵触。却下。
- 案 B（引数を struct に集約）: 6 つの同型 `BTreeSet` を
  `PreEditIds` struct にまとめ、`capture(project)` で
  取得。関数は `(candidate, after_objects, before)` の
  3 引数に。責務・判定は不変で、entity kind 追加時の
  拡張点も明確になる。

**観察**: 案 B を採用。lint Pass・全 test green。
