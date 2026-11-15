---
id: "TSPEC-GDVJyEaURIyTbCcZGMAkDR"
type: "test-spec"
status: "approved"
title: "RCD-01 Virtual L1 Oracle test-spec"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
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
    - "docs/specs/rcd-01-virtual-l1-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-01-device-recording-headless-fixture.md"
    - "docs/test-specs/rcd-02-blob-sha256-digest.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-01-virtual-l1-oracle.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-01-virtual-l1-oracle.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 Virtual L1 Oracle test-spec

## Purpose

`crates/synvelis-recording/tests/rcd01_oracle.rs` が保証すべきことを
定義する test-spec。fake/injected fault の成功を実 device 特性・実 OS・
GUI の証明にしない（synthetic-L1 headless の Pass を real-device/GUI/OS
lane の Pass に転用しない）。

## Feature

scripted oracle `Device`/`InputStream`（`MockPcm` 非依存の独立 generator）
→ `InputSession`/`InputAdapter` → 実 `StreamWriter` → staging media +
journal replay → `commit_session` → save → reload の E2E fidelity、
multi-track 分離、fault fail-stop verified prefix、digest 独立照合。

## Rules

- **R1 独立 oracle**: 期待 PCM は `MockPcm` と構造・定数を共有しない
  independently-authored deterministic generator（`oracle_sample`:
  splitmix64 連鎖相当 hash + 2 回 Murmur3-fmix64-style avalanche +
  `f64` 経由 `[-1,1)` f32 射影）で検算する。oracle 経路は `MockPcm`・
  `verify_capture_pcm`・`verify_tap_log`・`verify_log`・`MockInput`/
  `block_pcm`・`synvelis_recording::mock::*` を期待値構成に一切呼ばない。
  golden `const` 配列は同 file 内 inline 埋め込みのみ。
- **R2 emission-log 検証**: device が callback buffer に実際に書き出した
  `f32` bit pattern を記録し、staging media の f32le bytes を decode して
  bitwise 照合する。generator 再計算で期待値を作らない（自己撞着防止）。
  未供給 frame sentinel は `u32::MAX`（oracle 出力は有限 `[-1,1)` で衝突
  しない）。
- **R3 E2E fidelity**: mono/stereo/multichannel で scripted oracle device
  → commit→reload→`Source.content_digest`/`Asset.digest` 整合 → digest
  が指す bytes 再読 → oracle sample bit/sample 一致を実測 Pass。
  journal `ChunkRecord.media_offset`/`spans[].media_offset` で media byte
  を index し f32le decode → oracle 比較。`verified_sha256` は `sha2` で
  独立再計算して pin。
- **R4 multi-track**: (i) 1 stereo stream の 2 mono tap が別 stream・別
  staging media に oracle 値で分離、(ii) same-clock 2 device が別 stream
  に別 seed の oracle PCM を記録 — いずれも commit→reload まで fidelity
  実測。
- **R5 fault fail-stop**: input discontinuity（position gap）注入 →
  fail-stop → journal verified prefix bytes が oracle prefix と bit 一致
  （`verified_sha256` 再計算照合含む）かつ commit 層で faulted stream が
  `Source` へ昇格せず honest に recoverable 分類されることを
  commit→reload 越しに検証。writer finalized outcome は durable frontier
  として `Complete` を期待し、recoverable 分類は capture-fault marker が
  制御する（`fake_input_drop_fails_stop_and_preserves_verified_prefix`
  先例）。健全 stream 混在 session では健全側 fidelity も通す。
- **R6 honesty**: evidence file が実在し、実行したものだけを Pass と
  記録。synthetic-L1 Pass を rf#628/rf#629/physical/GUI lane の Pass に
  転用しない。canonical blob publish 脚は `synvelis-player` 管轄の既存
  証拠参照とし、本 slice で Pass 転用しない。

## Examples（test ↔ rule 対応）

| Test | Rule |
| --- | --- |
| `oracle_sample_golden_consts` | R1（generator 出力 bit pin） |
| `oracle_stereo_identity_commit_reload` | R3（stereo identity E2E） |
| `oracle_multichannel_identity_commit_reload` | R3（multichannel identity E2E） |
| `oracle_stereo_device_two_mono_taps_commit_reload` | R4(i)（1 stereo→2 mono tap 分離） |
| `oracle_two_devices_same_clock_commit_reload` | R4(ii)（same-clock 2 device） |
| `oracle_input_discontinuity_verified_prefix` | R5（discontinuity fail-stop verified prefix） |
| `oracle_missing_finalized_mixed_session` | R5（健全+faulted 混在 session） |
