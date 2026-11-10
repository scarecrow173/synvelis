---
id: "SPEC-034V2ghdXlh1m9UFtt5ks1"
type: "spec"
status: "proposed"
title: "RCD-01 実Audio Device録音統合 headless slice — Mock Input→Raw Writer/Journal→Arm/Punch/Loop・Fail-stop Fixture（synvelis#115）"
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
  depends-on:
    - "SPEC-034UoScGX6dIi25hMurqCI"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "TASK-LRVMzVtzazw5gBnDcKYhLk"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 実Audio Device録音統合 headless slice — Mock Input→Raw Writer/Journal→Arm/Punch/Loop・Fail-stop Fixture（synvelis#115）

## What Should Be Built

`synvelis#115` の headless slice として、RCD-01 Input Adapter（fake port）の `InputBlock` 供給から既存の Raw Capture Writer / Journal / Recovery までを結合する決定的 Fixture と Expected/Observed 検証を実装する。

具体的には `crates/synvelis-recording/tests/` に、fake Ruviform port（`tests/common/mod.rs` の `FakeDeviceEnumerator`/`FakeDevice`/`FakeInputStream`/`MockPcm`）→ `InputAdapter` → `TapWriterEnds` → 実 `StreamWriter`（`StagingArea` 上の media/journal）→ `finalize`/`recovery` までを一貫して駆動する harness を追加し、以下を Expected/Observed で検証する。

- Mono / Stereo / MultiChannel / 2Track（1 stream 2 tap）の各 channel map で、media PCM が `MockPcm` から導出した Expected interleaved f32 PCM と bit 一致すること。
- Punch `[in,out)` と Loop Pass の block 途中境界が Sample 単位で一意に確定し、journal の span が session 区間を隙間・重複なく tile すること。
- Arm Off + Monitor On で writer が作成されず media/journal が空のままであること。
- Input Drop / Clock Epoch 変更 / Rate 変更 / Device 切断 / RT queue full / filled queue full で fail-stop し、Journal Verified Prefix を保持したまま `capture-fault` marker 経由で Partial/Unknown が可視化されること。

`synvelis-app` 側は、録音 Session/Pass/Stream/Take の永続状態を投影する recording 状態投影（projection）と、その fixture を追加する。実 Device open・OS 別ドライバ・実 GUI は本 slice の対象外。

## Why Now

Issue `synvelis#115`（RCD-01）の受入条件は実 Device での Mono/Stereo/MultiChannel/2Track 照合と fail-stop を要求するが、Ruviform 側の実 Input Stream 契約は RF#251 で未提供であり、OS 別ドライバ実機検証も本環境では実行不能である。本 slice は Issue 本文が明示する「headless で成立する部分」— Mock Input/Clock Epoch・Raw Writer/Journal/PCM・Arm/Punch [in,out)・Loop Pass 境界・Fail-stop/QueueFull・Writer 不作成条件の Fixture と Expected/Observed — を先行して固定し、後続の実 Device slice が同一の検証面を再利用できるようにする。

既存の RCD-01 task（`TASK-LRVMzVtzazw5gBnDcKYhLk`）は fake port 契約層（binding/arm/monitor/callback 変換/fail-stop 判定）のみを対象とし、`InputAdapter` の出力を実 `StreamWriter` に接続する統合 fixture は未整備である。本 spec はそのギャップを埋める後続 slice として位置づける。

## Users and Value

- Synvelis 録音機能の実装者・レビュアー: 実装済み adapter が Raw Writer/Journal/Recovery の既存契約を破らないことを、再現可能な決定的 fixture で確認できる。
- 後続の実 Device slice（RF#251 解消後）: 同一の Expected/Observed 検証面を実デバイスへ移植する際の baseline となる。
- `synvelis-app` の録音 UI（#114/SV-GUI-05）: recording 状態投影の fixture が製品側の表示契約の土台になる。

## Scope

### In Scope

- `crates/synvelis-recording/tests/` への統合 fixture/harness 追加（`tests/common/mod.rs` の拡張を含む）。既存の `InputAdapter`/`StreamWriter`/`StagingArea`/`recovery` API のみを使用し、src への変更は原則行わない。
- Mono/Stereo/MultiChannel/2Track の PCM fidelity、Punch/Loop 境界、Arm/Monitor 独立性、fail-stop、writer 不作成の Expected/Observed 検証。
- `crates/synvelis-app/` への録音状態投影（`Project.recording` の session/pass/stream/segment/take + recorder slot の投影）と fixture。
- 本 slice 用の spec/design/plan/test-spec/task/impl 文書チェーンと index 更新。

### Out of Scope

- 実 OS オーディオデバイスの open/capture、OS 別ドライバ、実機検証（RF#251 依存、別 Gate）。
- `synvelis-app` の録音 command 実装・画面 UI（#114/SV-GUI-05 の領域）。本 slice では投影と fixture のみ。
- #116 Recovery・#117 Monitor 領域の変更、#119 cross-clock multi-device。
- Ruviform 側契約の変更・pin 更新。
- Mock/fake port の結果を実 Device Pass として扱うこと（昇格禁止）。

## Requirements

- fixture は `StagingArea` 上の実 `StreamWriter` を経由し、in-memory inspection writer のみで完結させない。
- PCM 検証は journal `ChunkRecord` の `pcm_crc32`/`media_offset`/`media_len` と media file 実バイトの双方で行い、span の session/source/project 範囲が Expected と一致することを確認する。
- fail-stop 検証は fault latch の bit、`capture-fault` marker、journal verified prefix（fault 前 chunk の完全検証）、finalize/recovery の Partial/Unknown 分類を観測する。
- Arm Off + Monitor On では `TapWriterEnds` が空であることに加え、staging の media/journal が空のまま（writer 未駆動）であることを確認する。
- 全検証は `mise run` タスク経由で実行し、fake port であることをテスト名・doc・PR で明示する。

## Acceptance Criteria

- [ ] `crates/synvelis-recording/tests/` に fake port → `InputAdapter` → 実 `StreamWriter` → staging media/journal → finalize/recovery を駆動する fixture が存在し、Mono/Stereo/MultiChannel/2Track の PCM が Expected と bit 一致する。
- [ ] Punch `[in,out)`（block 途中境界を含む）と Loop Pass で span が session 区間を一意に tile し、pass ごとの `pass_index`・`project_in/out`・連続 `source` 範囲が Expected と一致する。
- [ ] Arm Off + Monitor On で writer が作成されず、media/journal が空のままである。
- [ ] Input Drop / Clock Epoch 変更 / Rate 変更 / Device 切断 / RT queue full / filled queue full の各ケースで fail-stop し、Verified Prefix が保持され `capture-fault` marker 経由で Partial/Unknown が観測できる。
- [ ] `crates/synvelis-app` に recording 状態投影とその fixture が存在し、永続 recording entity を正しく投影する。
- [ ] `mise run fmt-check` `check` `lint` `test` `test-rt` がすべて Pass する。

## Deferred Design Concerns

- 実 Device open/capture・OS 別ドライバ | RF#251 の実 Input Stream 契約と実機環境が前提 | RF#251 解消・実機検証環境の整備 | fake fixture のみでは実デバイスの clock domain/latency 問題を検出できない。
- `synvelis-app` の録音 command/画面 | #114/SV-GUI-05 の UI 受入が前提 | #114 の GUI slice 着手時 | command 未実装のまま UI だけ進むと操作不能な画面になる。
- AudioLens native f64 非回帰 | #294 の領域 | #294 着手時 | 本 slice の変更は AudioLens に触れないが、回帰確認は別 Gate。

## Sources and Relations

- Issue: `synvelis#115`（RCD-01 実Audio Device録音統合）。受入条件は Issue 本文から転記。
- 先行 slice: `TASK-LRVMzVtzazw5gBnDcKYhLk` / spec `SPEC-034UoScGX6dIi25hMurqCI`（fake port 契約層）。
- 既存基盤: VS-A–D（capture engine, SPSC handoff, raw writer, journal, staging, recovery, commit）。
- 関連 Issue: RF#251（実 Input 契約）、synvelis#116（Recovery）、#117（Monitor）、#114（GUI）、#119（cross-clock）、#83（受入証拠）。
