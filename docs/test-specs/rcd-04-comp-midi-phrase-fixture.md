---
id: "TSPEC-RCD04Fx9mB3vC7xZ2qW5eR"
type: "test-spec"
status: "proposed"
title: "RCD-04 Comp Transition / MIDI Phrase fixture matrix"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 Comp Transition / MIDI Phrase fixture matrix

## Purpose

Issue #118 slice の crate 層契約を決定的に検証する
test-spec。fake FS・Mock PCM・plan 層 oracle 上の契約検証で
あり、実 Device/実 GUI/実 OS/AudioLens の Pass を意味
しない（NotRun — 実機 gate / consumer gate）。

## Feature/Rule/Example

### F1: Group Comp CAS / all-or-nothing（project + recording）

- Rule: `ProjectController::apply` は expected-revision
  一致時のみ clone→validate→commit し、失敗時は revision・
  comp registry・ID watermark・undo stack を進めない。
- Rule: `CompOp` の不正 op（重複 region・非隣接
  transition・範囲外/他 lane take・非 MIDI source 等）は
  validate で拒否され部分適用が残らない。
- Example: `rcd_04_comp_cas.rs` で stale revision・不正
  region/transition・undo/redo ID 一致・save/load 一致・
  serialized diff 無差分を assert。`rcd_04_take_comp_pcm.rs`
  で `SynvelisOp::Comp` 経由の同規則を実 recording 経路で
  検証。

### F2: Take/Comp/Recovery の raw 不変・Stable ID

- Rule: comp 操作・undo/redo・save/reload・recovery review
  の前後で、staging media/journal bytes・`Take`/`Source`/
  `Asset` field・全 stable ID が不変。
- Example: media file の byte hash を comp 前後で比較、
  `project_comp` projection の `PartialEq` 再現性、
  `InMemoryStore` save→load 後の ID 集合一致、serialized
  snapshot の semantic diff が comp/MIDI 関連で差分なし。

### F3: RT↔Offline PCM 差記録（recording）

- Rule: capture 済み PCM（staging `*.pcm` f32le）を RT 側
  実測値とし、`project_comp` の EffectiveComp を決定的
  offline renderer で評価した PCM との差を feature 別に
  記録する。identity 域は bit-exact、transition 域は
  player `FadeWindow::gain_at` と同一 curve の解析値、
  `Loop`/非対応 mapping は lowering の skip 規則と整合。
- Example: Source Handle（ExistingSource handle/play_from
  範囲）・Fade（overlap+FadeShape）・相関素材（2 take
  seed 差）・Transient（境界急変）・Beat Anchor/Tempo
  （tempo map 下の musical→media 写像）・Loop（Loop
  mapping region）の各 scenario で差分を assert。

### F4: Recovery partial + cancel + retry（recording）

- Rule: unverified tail を持つ stream は finalize しても
  `Complete` にならず、`Hold`（cancel/defer）は file 不変、
  `Adopt`（retry）は verified prefix のみ確定し冪等。
  Malformed/Corrupt/参照不明は fail-closed。
- Example: `rcd_04_recovery_partial.rs` で partial stream
  を `scan`→`Hold`→`Adopt` し、media/journal byte・
  project revision・take 数の不変と `finalized.json`/
  review 記録の durable 性を検証。journal 破損 byte で
  `Corrupted`/`Unreadable`→`review_stream` 拒否、
  `Discard` の probe 規則（未指定/Referenced/Unknown 拒否、
  Clear のみ削除・commit manifest 存在時拒否）を検証。

### F5: MIDI Phrase 境界安全性 + Stable ID（project）

- Rule: `compile_midi_comp` は phrase onset 内 note のみ
  採用し、phrase/comp 終端を越える note に virtual off
  を emit（hanging なし）。region 境界では off→on 順・
  同一 `(source_id,note_id)` 重複発音なし。各 region 開始
  に controller chase snapshot（kind 和集合・未宣言は
  default）を emit し comp 末尾 `AllNotesOff`（leak なし）。
  native tick/division は保存し project tick への写像は
  正確有理数のみ（剰余 != 0 は error・暗黙 quantize なし）。
- Rule: Undo/Redo・Save/Reload・serialized diff で
  SourceId/MidiSource.id/NoteId/ClipId・selection region
  ID が維持される。
- Example: `rcd_04_midi_phrase.rs` で境界 note 切断・
  境界順序・chase/leak・native tick provenance・不正
  selection 拒否・stable ID round-trip を assert。

## NotRun / Unsupported

- 実 OS/Device/FS durability・実 GUI Window・2Window stale
  gesture・history view — NotRun（#114・別 dispatch）。
- AudioLens native-f64/raw-recording 非回帰 — NotRun
  （Ruviform #294 consumer gate）。
- 実 RT engine（ruviform graph）での comp 再生 PCM 一致 —
  NotRun（player/render は本 slice 範囲外）。
- MIDI comp selection の project 永続化・legato 継続・
  fractional tick 写像・CC/Program 以外の event —
  Unsupported（本 slice の plan 層範囲外・deferred）。
