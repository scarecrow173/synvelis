---
id: "TSPEC-034V2ixodcql8RLLJfoYqH"
type: "test-spec"
status: "proposed"
title: "RCD-01 device-recording headless fixture test spec"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source: []
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
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/rcd-01-device-recording-headless-fixture.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 device-recording headless fixture test spec

## Purpose

`InputAdapter`（fake port）の `InputBlock`/PCM 出力が既存 Raw Capture Writer・Journal・Recovery の契約を破らないことを、決定的 fixture で固定する。本 spec は fake port 契約証拠であり、実 Device 受入を意味しない。実 Device slice が同一検証面を再利用するまで維持する。

## Feature

Feature: RCD-01 device-recording headless fixture
fake Ruviform port から供給される入力 PCM が、armed tap ごとの実 `StreamWriter` 経由で staging media/journal に正しく記録され、Punch/Loop 境界・fail-stop・writer 不作成が Expected/Observed で検証される。

## Rules

- Rule: PCM Fidelity — Mono/Stereo/MultiChannel/2Track の各 channel map で、media file の interleaved f32 PCM が `MockPcm` と channel map から導出した Expected PCM と bit 一致する。
- Rule: Journal Integrity — journal replay が fault 前の全 chunk を verified prefix として検証し、`pcm_crc32`/`media_offset`/`media_len` が media 実バイトと一致する。
- Rule: Punch Boundary — Punch `[in,out)` の block 途中境界が Sample 単位で一意に確定し、span が `[punch_in_frame, min(punch_out_frame, session_end))` を隙間・重複なく tile する。
- Rule: Loop Pass — Loop Pass ごとに `pass_index` が単調増加し、各 pass の session/project 範囲が loop template に従い、`source` 範囲が pass をまたいで連続する。
- Rule: Arm/Monitor Independence — Arm Off + Monitor On で `TapWriterEnds` が空、staging media/journal が空のまま。Monitor 状態は raw PCM を変更しない。
- Rule: Fail-stop — Input Drop / Clock Epoch 変更 / Rate 変更 / Device 切断 / RT queue full / filled queue full で fail-stop し、fault latch の対応 bit が立ち、`capture-fault` marker が記録され、journal verified prefix が保持される。
- Rule: Partial Visibility — fail-stop 後の finalize/recovery が Partial/Unknown（recoverable 分類または非 Complete outcome）を観測可能にする。
- Rule: Determinism — 全 fixture が seed・block size 固定で再現可能。

## Examples

- Example: Stereo Normal 録音 — Given stereo device に 2ch tap、When `stream.deliver(256)` を session_end まで反復、Then media PCM が `MockPcm` の ch0/ch1 interleave と bit 一致し、journal 全 chunk が verified。
- Example: Punch block 内境界 — Given punch_in/out が block 途中の frame に一致しない場合でも、When 録音、Then span の session_start/end が Sample 単位で正確に `[in,out)` に一致する。
- Example: Loop 3 pass — Given loop 長 1000f・punch [250,750)、When session_end=2500f、Then pass_index 0/1/2 の span が各 pass の punch 区間のみを tile し、最終 pass は session_end で clamp される。
- Example: Input Drop — Given 録音中に `stream.jump_to` で frame を飛ばす、When pump、Then `FAULT_INPUT_DISCONTINUITY` が latch され、fault 前 chunk のみ verified prefix として残る。
- Example: Arm Off Monitor On — Given 全 tap が `arm=Off, monitor=On`、When open、Then `writer_ends` が空で staging file が 0 byte のまま。

## Guarantees

- fixture が実 `StreamWriter`/`StagingArea`/journal/recovery を経由していること（in-memory inspection のみで完結しない）。
- PCM 検証が media 実バイトと journal CRC の双方で行われること。
- fail-stop の各 fault 種別で Verified Prefix 保持と `capture-fault` marker の存在が assert されること。
- `synvelis-app` 投影 fixture が永続 recording entity を正しく投影すること。

## Non-goals

- 実 OS device・driver・実機の clock domain/latency。
- 録音 command・UI 操作・#114/#116/#117/#119 の領域。
- AudioLens f64 非回帰（#294）。
- Mock/fake 結果の実 Device Pass への昇格。

## Risk

- fixture が in-memory writer のみを使うと実 staging/journal の回帰を見逃す → 実 `StreamWriter` 必須を Rule/Garantee で固定。
- fail-stop を fault bit のみで判定すると journal/marker の durable 記録漏れを見逃す → Verified Prefix + marker を必須観測にする。
- Punch/Loop の境界を block 単位で検証すると Sample 単位のずれを見逃す → span の session/source/project を Sample 単位で assert する。
