---
id: "SPEC-034UwMIulZ7DXWykQFy704"
type: "spec"
status: "proposed"
title: "TIM-02 SMF Format0/1 Import/Export — project+recording headless slice (synvelis#126)"
created: "2026-11-09"
updated: "2026-11-09"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/designs/midi-source-note-editing-design.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/52"
    - "https://github.com/scarecrow173/synvelis/issues/118"
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers:
    - "synvelis-player の MIDI RT/Offline 結合・Loop/Seek/Stop・RT callback（後続 slice）"
    - "実 MIDI Backend OS 別・実 Device/Plugin 検証（Ruviform #253 の責務）"
    - "SMPTE Division・MIDI 2.0/MPE（#129/#130 future・明示 Unsupported）"
    - "AudioLens Native f64 非回帰（#294 別 Gate）"
  deferred-by: []
---

# TIM-02 SMF Format0/1 Import/Export — project+recording headless slice (synvelis#126)

## What Should Be Built

Issue #126（TIM-02 MIDI 1.0 Standard MIDI File Import/Export）のうち、
`synvelis-project` / `synvelis-recording` 両 crate 内で完結する
headless slice を実装する。

- MIDI 1.0 SMF **Format 0 / Format 1** の deterministic な parse /
  serialize。PPQN division のみを対象とし、SMPTE time base・
  Format 2・MIDI 2.0/MPE は typed `Unsupported` で fail-closed
  する（成功扱いしない）。
- Parse 結果を project に取り込む `ProjectOp::ImportMidiSmf`。
  取り込みは source-native tick / PPQN / track・channel・event
  provenance を `SmfDocument` として永続化し、note 射影
  （`MidiSource.notes`）は document からの決定的な派生とする。
- `MidiSource` から SMF bytes への Export。Export 先 division
  （PPQN）を選択でき、native tick → target tick の変換について
  `ConversionReport { exact_count, rounded_count, max_error,
  unsupported_events }` を返す。native source の provenance
  document をユーザー承認なしに上書きしない。
- Event scope: Note On/Off（velocity 0 Note On = Note Off）、
  CC、Pitch Bend、CC64 sustain、その他 channel event
  （Program Change / Aftertouch）、Meta event（Tempo /
  Time Signature / End of Track / その他は raw bytes 保存）、
  System Exclusive。未知 event は executable MIDI に化けない
  typed な provenance 保存とする。
- `compile_midi_comp`（RCD-04, PR #219 merged）との整合:
  controller lane に channel provenance を持たせ、region 境界の
  chase を `(channel, kind)` 単位で行い、同時刻 NoteOff→On・
  Panic（comp 末尾 AllNotesOff）・発音残り 0 を保証する。
- `synvelis-recording` 側は既存 Take/Phrase 接続を消費する
  fixture のみ追加し、recording crate の構造は変更しない。

## Why Now

RCD-04（PR #219）は `compile_midi_comp` で phrase 境界安全性・
controller chase・厳密 tick 写像・stable ID を確立したが、
「persisted MIDI comp state / event model は #126 の
native-tick/chase event model 確定後」と明示的に deferred して
いた。本 slice がその event model を確定し、SMF という実際の
外部 MIDI データ源から native tick・tempo・note order・CC64 を
失わずに project へ取り込み、往復（Export→Reimport）で
provenance を保存する境界を headless に固定する。

`synvelis-player` の MIDI RT/Offline 結合は sv101 が並行作業中の
ため本 slice では触れず、file-headless の成功を実機 MIDI timing
成功と混同しない。

## Users and Value

- DAW 利用者: SMF ファイルを project に取り込み、編集・保存・
  書き出し・再取り込みしても source-native のタイミングと
  provenance が失われない。
- 開発者: typed error により malformed / overflow / unsupported
  入力を成功と区別でき、fixture で決定的に検証できる。

## Scope

### In Scope

- `crates/synvelis-project/src/` への SMF Format 0/1 parser /
  serializer / document model / import op / export API の追加。
- `Project` への SMF provenance document の永続化
  （schema version 前進 + migration）。
- `MidiControllerEvent` / chase への channel provenance 追加と
  `compile_midi_comp` の `(channel, kind)` chase。
- Import 由来 source の note 編集 op に対する immutable 化
  （provenance 破壊防止）。Detach は provenance を引き継がない
  独立コピーとして編集可能にする。
- `crates/synvelis-project/tests/` の deterministic fixture /
  golden test（Format0/1、複数 track/channel、PPQN mismatch、
  tempo、CC64、event order、NoteOff→On、malformed/overflow/
  unsupported、Export→Reimport、chase/panic）。
- `crates/synvelis-recording/tests/` の Take/Phrase 消費 fixture
  （発音残り 0）。recording 実装は変更しない。
- `docs/` チェーン（spec/design/plan/test-spec/task/impl）。

### Out of Scope

- `synvelis-player` の MIDI RT/Offline・Loop/Seek/Stop・RT
  callback 結合（後続 slice。sv101 との競合回避）。
- `synvelis-app` / `synvelis-workspace` / `synvelis-library`、
  gateway/render/theme/poc、`.github/`、mise.toml、Ruviform 側。
- 実 MIDI Backend OS 別・実 Device/Plugin 検証（NotRun。
  Ruviform #253 の責務）。
- SMPTE Division・MIDI 2.0/MPE（#129/#130。明示 Unsupported）。
- AudioLens Native f64 非回帰（#294 の別 Gate）。
- GUI 経路。file-headless API のみ。

## Requirements

- SMF Format 0 / 1 を parse し、Format 2・SMPTE division・
  その他未対応を typed `Unsupported` で拒否する。malformed
  （truncated varlen、length 不整合、負 tick 等）・overflow
  （delta 蓄積・tick 変換）は typed error とする。
- Import は native PPQN division・raw tick・track/channel・
  event 順序・tempo/time-sig meta・sysex を provenance
  document として保存し、note 射影を決定的に生成する。
- Project PPQN 変更・save/load・export・再 import を通じて
  native tick・note order・tempo・CC64 が失われない。
- Export は PPQN 変換を明示的に行い、`exact_count` /
  `rounded_count` / `max_error` / `unsupported_events` を
  `ConversionReport` として返す。暗黙の quantization をしない。
- 同一 tick の event 順序は NoteOff→NoteOn を保証する決定的
  ordering とする。
- SMF 由来 source の note 編集は拒否し、provenance document を
  保持する。DetachClipSource は provenance を持たない独立
  コピーを生成する。
- `compile_midi_comp` の phrase 再生で、region 境界 chase が
  channel を区別し、comp 末尾の AllNotesOff により発音残り 0
  を保証する。
- Export→Reimport の golden roundtrip で document 意味が保存
  されることを fixture で確認する。

## Acceptance Criteria

- [ ] PPQN 異種の Import / Project Change / Save / Export /
  再 Import で Native Tick・Note order・Tempo・CC64 が保持され、
  丸め数と最大誤差が `ConversionReport` で計測できる。
- [ ] Format 0/1・複数 Track/Channel・同時刻 NoteOff→On・
  Panic/Chase・MIDI Take Phrase で発音残り 0 が fixture で
  確認できる。
- [ ] SMF 異常 / Overflow / Malformed / Unsupported の typed
  error があり、SMPTE / MIDI 2.0 を成功扱いしない。
- [ ] Export→Reimport の golden roundtrip が deterministic に
  成立する。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `build`（存在するもの全て）が
  通る。実行不能な検証は NotRun / Unsupported と記録する。

## Deferred Design Concerns

- `synvelis-player` の MIDI RT/Offline 結合・Loop/Seek/Stop・
  RT callback — sv101 slice の責務 | 後続 slice 着手時 |
  放置すると file 層と RT 層の契約齟齬が残る | 中
- 実 MIDI Backend・Device/Plugin | Ruviform #253 未完 |
  Ruviform 側完了後 | 実機 timing 未検証のまま | 中
- SMPTE Division / MIDI 2.0 / MPE | TIM-F01/02 future issue
  | #129/#130 起票時 | 現状は明示 Unsupported で安全 | 低
- AudioLens Native f64 非回帰 | #294 別 Gate | #294 対応時 |
  float 精度差の見逃し | 低

## Sources and Relations

- Issue #126（TIM-02）: SMF Format0/1 Import/Export・native
  timestamp/chase fidelity の契約。
- TIM-01 (#125) native tick/PPQN 契約、#19/#20 musical time
  契約、Ruviform #188 timestamped MIDI event/Note safety 契約。
- Recording #12 MIDI Take、Ruviform #253 OS MIDI/VST3
  translation（integration dependency、開始前提ではない）。
- RCD-04 spec/design（`docs/specs/rcd-04-*.md`、
  `docs/designs/rcd-04-*.md`）: `compile_midi_comp` の境界
  oracle と deferred event model の受け皿。
- `docs/designs/midi-source-note-editing-design.md`、
  `docs/designs/configurable-ppqn-musical-time-architecture.md`。
