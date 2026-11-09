---
id: "IMPL-TIM02SmfFormat01Core0001"
type: "impl"
status: "completed"
title: "TIM-02 SMF Format0/1 Import/Export — project+recording headless slice 実装記録"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/smf.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_02_smf.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/tim_02_midi_phrase.rs"
      - type: "file"
        path: "docs/specs/tim-02-smf-format01-import-export.md"
      - type: "file"
        path: "docs/designs/tim-02-smf-format01-core.md"
      - type: "file"
        path: "docs/plans/tim-02-smf-format01-core.md"
      - type: "file"
        path: "docs/tasks/tim-02-task1-smf-core.md"
      - type: "file"
        path: "docs/test-specs/tim-02-smf-format01-fixtures.md"
      - type: "file"
        path: "docs/impl/ir/tim-02-smf-format01-import-export.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/midi_comp.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/ctl_01.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/midi_source.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/rcd_04_midi_phrase.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/tim-02-task1-smf-core.md"
    - "docs/plans/tim-02-smf-format01-core.md"
    - "docs/specs/tim-02-smf-format01-import-export.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim-02-task1-smf-core.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-04-comp-transition-midi-phrase-crate-fixture-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/designs/tim-02-smf-format01-core.md"
    - "docs/plans/tim-02-smf-format01-core.md"
  derived-by: []
  verifies:
    - "docs/test-specs/tim-02-smf-format01-fixtures.md"
  verified-by:
    - "docs/test-specs/tim-02-smf-format01-fixtures.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-02 SMF Format0/1 Import/Export — project+recording headless slice 実装記録

## Summary

Issue #126（TIM-02 SMF Format0/1）のうち project SMF
Import/Export + recording Take/Phrase 接続の headless slice を
実装した。`synvelis-project` に SMF 境界 `smf.rs`
（`parse_smf`/`export_smf`・provenance document・typed error・
conversion report）を追加し、`Project.smf_documents` による
provenance 永続化・`ProjectOp::ImportMidiSmf`・SMF-backed source
の note 直接編集拒否・schema v9 identity migration・channel-aware
controller chase を接続した。`synvelis-recording` 側は既存の
Take/Phrase 消費経路に対する fixture で「発音残り 0」を検証した
（project 28 test・recording 2 test）。実 Device・OS backend・
plugin・GUI・RT player・Ruviform generic MIDI は対象外。

## Context

- spec `docs/specs/tim-02-smf-format01-import-export.md`
- design `docs/designs/tim-02-smf-format01-core.md`
- plan `docs/plans/tim-02-smf-format01-core.md`
- test-spec `docs/test-specs/tim-02-smf-format01-fixtures.md`
- task `docs/tasks/tim-02-task1-smf-core.md`

Issue #126 の crate 層受入条件（PPQN 異種の Import/Project
Change/Save/Export/再 Import で Native Tick/Note order/Tempo/
CC64 保持・丸め数と最大誤差確認・Format0/1・複数 Track/Channel・
同時刻 NoteOff→On・Panic/Chase・MIDI Take Phrase 発音残り 0・
SMF 異常/Overflow/Malformed/Unsupported typed error・SMPTE/
MIDI2 未対応を成功扱いしない）を headless fixture でカバーする。
`synvelis-player`/`synvelis-app`/`synvelis-workspace`/
`synvelis-library`/gateway/render/theme/poc（`op_name` の
exhaustive match 追従を除く）・`.github/`・mise.toml・
Ruviform は変更していない。

## Implementation

### Changes

- `crates/synvelis-project/src/smf.rs`（新規）: SMF Format0/1
  parser/exporter。`SmfDocument`/`SmfTrack`/`SmfEvent`/
  `SmfEventKind`（channel 付き channel event・Tempo・
  TimeSignature・TrackName・raw `MetaOther`・`Sysex`・
  `EndOfTrack`）・`SmfError`（`Malformed`/`Unsupported`/
  `Overflow`/`NotFound` の typed variant）・`SmfExportOptions`・
  `SmfRational`/`SmfConversionReport`（exact/rounded/max_error/
  unsupported_events）を定義。parser は running status・VLQ・
  velocity-0 Note On → NoteOff 正規化を処理し、SMPTE division・
  Format2・MIDI 2.0/MPE を `Unsupported` で拒否、非ゼロ長
  `0x2F` meta は payload を落とさず `MetaOther` として保持する。
  `SmfDocument::validate` は division `1..=32767`・track 数
  `1..=u16::MAX`・非減少 tick・wire domain を強制し、
  `paired_notes` は track ごとの `(channel,key)` FIFO pairing で
  `MidiSource.notes` への deterministic 投影を返す。exporter は
  provenance document を優先し、なければ `MidiSource.notes` から
  note-only SMF を合成。tick 変換は round-half-up の checked
  算術で exact/rounded と最大有理誤差を報告し、canonical emit
  （running status なし・同一 tick は meta→sysex→NoteOff→
  controller/channel→NoteOn→EOT の rank 順、Format0 merge は
  `(tick,rank,track,seq)` tiebreak）を行う。
- `crates/synvelis-project/src/project.rs`: `Project` に
  `smf_documents: BTreeMap<SourceId, SmfDocument>`
  （`#[serde(default)]`）を追加。reference validation で
  document の source 存在・division 一致・`validate` 合格・
  `paired_notes` 投影と `MidiSource.notes` の multiset 一致を
  強制し、divergence を load 時に拒否する。
- `crates/synvelis-project/src/ops.rs`: `ProjectOp::ImportMidiSmf`
  を追加（document validate → deterministic pairing → 新規
  `SourceId`/`NoteId` 採番 → native division の `MidiSource`
  生成 → `smf_documents` 登録）。`AddMidiNote`/
  `RemoveMidiNote`/`SetMidiNoteVelocity`/`MoveMidiNote` は
  SMF-backed source に対して `reject_smf_backed` で拒否。
  `RemoveMidiSource` は対応 document を除去し、
  `DetachClipSource` は provenance を持たない編集可能コピーを
  生成する。
- `crates/synvelis-project/src/midi_comp.rs`:
  `MidiControllerEvent` に `channel` を追加し、
  `MidiPlanAction::ControllerChase` も channel-aware 化。
  chase は `(channel, MidiControllerKind)` キーで state 漏洩を
  防ぎ、plan 末尾は `AllNotesOff` で終了する（RCD-04 の
  channel 非依存 chase を本 slice で是正）。
- `crates/synvelis-project/src/migration.rs` /
  `src/store.rs`: schema version を 8→9 に更新し、v8→v9 は
  `smf_documents` が `serde(default)` で空 map になる identity
  migration として登録。`SCHEMA_VERSION` 参照箇所と現行
  snapshot assertion（`ctl_01.rs`/`md_a.rs`/`midi_source.rs`）を
  9 に追従。v8 fixture は migration 検証用に v8 のまま保持。
- `crates/synvelis-project/src/lib.rs`: `smf` module 公開 +
  re-export。
- `crates/synvelis-gateway/src/prepare.rs`: `ProjectOp` の
  exhaustive `op_name` match に `ImportMidiSmf` →
  `"import-midi-smf"` arm を追加（workspace compile のための
  機械的追従）。
- `crates/synvelis-project/tests/tim_02_smf.rs`（新規・28 test）:
  Format0/1 parse（native tick・PPQN・tempo・time signature・
  channel・CC64・sysex・unknown meta・EOT provenance）・
  velocity-0 正規化・running status・typed error
  （malformed/overflow/unsupported: SMPTE・Format2・MIDI2
  header）・Import の deterministic NoteId・Save/Load での
  `smf_documents` 保持と schema v9・SMF-backed note 編集拒否・
  Detach の provenance-free copy・export の exact/rounded/
  max_error・同一 tick NoteOff→On・channel-aware chase・
  `AllNotesOff` 終端・Export→Reimport golden roundtrip・
  PPQN 変換での note order/tempo/CC64 保持。
- `crates/synvelis-recording/tests/tim_02_midi_phrase.rs`
  （新規・2 test）: SMF import → `SmfDocument::controller_events`
  → `MidiControllerLanes` → `compile_midi_comp` の既存
  Take/Phrase 経路で phrase plan を生成し、全 NoteOn が NoteOff
  で閉じ plan が `AllNotesOff` 終端すること（発音残り 0）と、
  export→reimport が project 経由で document 意味を保持する
  ことを検証。
- `docs/` チェーン 5 文書 + 各 README index 追加。

### Decisions During Implementation

- `MidiSource` への field 追加は採用しなかった。`mcp-poc`/
  `synvelis-gateway` など編集範囲外の crate に struct literal が
  存在するため、provenance は additive な
  `Project.smf_documents: BTreeMap<SourceId, SmfDocument>` に
  格納し schema v9 + identity migration で導入した。
- `MidiSource.notes` は SMF document の deterministic 投影とし、
  document を authoritative provenance とした。投影と source
  notes の multiset 一致を reference validation で強制し、
  乖離状態を load 時に fail-closed で拒否する。
- SMF-backed source の note 直接編集は全 op で拒否し、編集は
  `DetachClipSource` 経由の provenance-free copy に限定した
  （provenance と投影の divergence を構造的に防止）。
- `export_smf` は provenance document を信用する前に
  `doc.validate()` を実行し、手組み project の不正 field が
  wire byte を破壊しないよう fail-closed とした。合成経路も
  note の key/velocity domain を encode 前に検証する。
- `SmfDocument::validate` は SMF wire domain（division
  `1..=32767`・track 数 `1..=u16::MAX`）を強制し、非ゼロ長
  `0x2F` meta は `EndOfTrack` と誤認せず `MetaOther` として
  payload を保存する。
- controller chase は `(channel, MidiControllerKind)` キーに
  変更し、channel 間の controller state 漏洩を防止した
  （RCD-04 で deferred されていた channel 意識を本 slice で
  解消）。
- doc-driven-dev-graph router は main 上の既存 broken relation
  で blocked（RCD-04 と同じ既知の基盤問題）だったため、
  spec→design→plan→task→test-spec チェーンは手動で作成し、
  本記録にその旨を残した。

## Related Experiments

### Adopted

<!-- 実験 log なし（既知解の機械的実装 + fixture 作成）。 -->

### Rejected

<!-- なし -->

## Validation

canonical 検証（`mise.toml` タスク、`origin/main` `30033c3`
merge 後の最終状態で全て exit 0）:

- `mise run fmt` / `mise run fmt-check` — Pass
- `mise run check` — Pass（`cargo check --workspace
  --all-targets`）
- `mise run lint` — Pass（`cargo clippy --workspace
  --all-targets -- -D warnings`、warning 0）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  138 test binary 全て ok・failure 0。新規 `tim_02_smf` 28 test・
  `tim_02_midi_phrase` 2 test を含む）
- `mise run test-rt` — Pass（129 test result ok・failure 0）
- `mise run build` — Pass
- `mise run test-msrv` — Pass（rust 1.89.0、138 test result
  ok・failure 0）

NotRun / Unsupported（test-spec 宣言どおり）:

- 実 MIDI Device/OS backend・plugin・GUI・RT player 挙動 —
  NotRun（後続 slice / Ruviform #253 の責務）
- SMPTE division・Format2・MIDI 2.0/MPE — Unsupported
  （`SmfError::Unsupported` で明示拒否、成功扱いしない）
- AudioLens native-f64 非回帰 — NotRun（#294 の別 Gate）

## Risks

- `smf_documents` は `MidiSource` と別 map にあるため、
  project JSON では source と document が別 field に分かれる。
  reference validation で両者の整合を強制しているが、
  `MidiSource` 自体を直接 deserialize する経路（存在しない
  前提）があれば provenance を見落とす。現状は
  `Project` 単位の load のみ。
- export の Format1 track 保持は provenance document の track
  index に依存し、document が 65535 track を超える場合は
  `validate` で拒否される。極端に大きな SMF の roundtrip は
  fixture 範囲外。
- `unsupported_events` は現状常に空（全 parsed kind が
  round-trip 可能）。将来 drop が必要な kind が増えた場合に
  report 経路を使う設計であり、現時点では未使用の typed slot。

## Follow-ups

- Issue #126 残件: `synvelis-player` の MIDI RT/Offline 結合・
  Loop/Seek/Stop・RT callback（sv101 との競合回避のため別
  slice）、実 MIDI Backend OS 別・実 Device/Plugin
  （Ruviform #253）、AudioLens f64 非回帰（#294 consumer
  gate）。
- MIDI comp selection 自体の project 永続化は本 slice 対象外
  （RCD-04 からの deferred を維持）。chase の channel-aware 化は
  plan 層に適用済みだが、selection model の永続化・GUI 接続は
  後続 slice。
