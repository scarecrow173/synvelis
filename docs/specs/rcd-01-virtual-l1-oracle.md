---
id: "SPEC-82BJ1E0D6FN2fJQokzfPoY"
type: "spec"
status: "approved"
title: "RCD-01 Virtual L1 Oracle — synvelis#115 slice (Option A synthetic-L1)"
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
    - "docs/specs/rcd-01-device-recording-headless-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-01-real-input-coverage.md"
    - "docs/specs/rcd-02-blob-sha256.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  defers: []
  deferred-by: []
---

# RCD-01 Virtual L1 Oracle — synvelis#115 slice

## Purpose

Issue `scarecrow173/synvelis#115`（RCD-01 実 Audio Device 録音統合）の
closeout/oracle slice（Option A — synthetic-L1 最終 Oracle の確立）。
現行 `verify_capture_pcm` は期待 PCM を production `MockPcm` で再計算する
自己参照検証であり、「Callback→Raw Recorder→Journal→Project/Reload の
独立 sample/byte Oracle」を満たさない。本 slice は独立 oracle・E2E fidelity・
Evidence/Case Matrix・doc chain を確立し、Issue を open 継続のまま
一部分だけ閉じる。

## Scope（本 slice で実測するもの）

- `crates/synvelis-recording/tests/rcd01_oracle.rs`（新規）に、
  `MockPcm`・`verify_capture_pcm`・`verify_tap_log`・`verify_log`・
  `MockInput`/`block_pcm`・`synvelis_recording::mock::*` を期待値構成に
  一切呼ばない独立 deterministic oracle を実装する。
- scripted oracle `Device`/`InputStream`（test-local）→
  `RealDeviceRig::open`（`&dyn Device` 経路）→ `InputSession`/`InputAdapter`
  → 実 `StreamWriter` → staging media + journal replay →
  `commit_session` → `DirectoryBundleStore` save →
  `ProjectController::load`（別 controller）までを production 経路
  変更なしで駆動し、mono/stereo/multichannel で sample/byte fidelity を
  実測する。
- multi-track: (i) 1 stereo stream の 2 mono tap が別 stream・別 staging
  media に oracle 値で分離、(ii) same-clock 2 device が別 stream に別 seed
  の oracle PCM を記録 — いずれも commit→reload まで検証。
- fault: input discontinuity（position gap）注入 → fail-stop →
  journal verified prefix が oracle prefix と bit 一致
  （`verified_sha256` 再計算照合含む）かつ commit 層で faulted stream が
  honest に recoverable 分類されることを commit→reload 越しに検証。
- `verified_sha256`（journaled verified prefix bytes の SHA-256）は
  test 内で `sha2` crate により独立再計算して照合する。
- `evidence/sv115/` に ledger + case matrix を作成し、Issue の Required
  case を synthetic-L1 Pass / rf#628-fixture NotRun / rf#629 仮想
  endpoint EXT-L3 NotRun(NOT_PROVISIONED) / physical NotRequired に分類。

## Non-scope（本 slice でやらないもの）

- `crates/synvelis-recording/src/`・`crates/synvelis-project/`・
  `crates/synvelis-app/` の変更一切（oracle は既存 production 経路を
  変更なしで再利用）。
- `publish_recorded`/`open_blob_verified`/`blob_path` の canonical blob
  publish 脚の再実装・再実走（`synvelis-player` 管轄。既存証拠
  `crates/synvelis-player/tests/rcd02_blob_digest.rs`・
  `evidence/sv116/rcd-02-blob-sha256-ledger.md` を参照・相互リンク）。
- rf#628 fixture variant・rf#629 仮想 endpoint EXT-L3・物理 HW・
  GUI（Option B）の実走。これらは NotRun/NotRequired として matrix に
  正直記録するのが本 slice の仕事。
- `*-msrv` 系検証（Daily CI 委譲）、`mise.toml`/`Cargo.toml`/
  `Cargo.lock`/`.github/workflows/`/`ui/`/`POLICIES.md`/`AGENTS.md` の変更。
- Issue #115 の auto-close（`Refs #115` のみ。merge と Close は
  Coordinator が行う）。

## Acceptance Criteria（本 slice）

1. 独立 oracle: 期待 PCM を `MockPcm` と生成コードを共有しない
   independently-authored deterministic generator で検算。同 file 内の
   小さな golden `const` 配列を補助に使ってよい。oracle 経路は
   `MockPcm`・`verify_capture_pcm`・`verify_tap_log`・`verify_log`・
   `MockInput`/`block_pcm`・`synvelis_recording::mock::*` を期待値構成に
   一切呼ばない。入力側 device が供給する PCM も oracle generator 由来とし、
   「入力生成」と「期待値検算」が同じ関数を経由しない構造にする。
2. E2E byte/sample fidelity: scripted oracle device →
   `InputSession`/`InputAdapter` → 実 `StreamWriter` → staging media +
   journal replay → `commit_session` → `DirectoryBundleStore` save →
   `ProjectController::load`（別 controller）→
   `Source.content_digest`/`Asset.digest` の整合 → digest が指す bytes
   を再読 → oracle sample と bit/sample 一致、を mono/stereo/multichannel
   で実測 Pass。journal `ChunkRecord` の `media_offset`/`spans` で media
   byte を index し、f32le decode → oracle 比較。`verified_sha256` を
   `sha2` 再計算で pin。
3. Multi-track: 上記 (i)(ii) を commit→reload まで fidelity 実測。
4. Fault case: input discontinuity → fail-stop → verified prefix bytes
   が oracle prefix と bit 一致（`verified_sha256` 再計算照合含む）かつ
   commit 層で faulted stream が honest に recoverable 分類される
   （`Source` へ昇格しない）ことを commit→reload 越しに検証。
5. Evidence/case matrix: `evidence/sv115/` に ledger + case-matrix doc。
   synthetic-L1=実測 Pass / rf#628-fixture=NotRun / rf#629 仮想 endpoint
   EXT-L3=NotRun(NOT_PROVISIONED) / physical=NotRequired /
   `mise run test-recording-real-input`=実行して SKIP/NotRun を正直記録 /
   canonical blob publish 脚=既存証拠参照で Pass 転用しない。
6. Doc chain: slug `rcd-01-virtual-l1-oracle` の
   specs→designs→plans→tasks→test-specs→impl/ir→impl/exp を作成し、
   `docs/*/README.md` 管理 index を `doc-maintenance` Skill の手順で
   再生成して同じコミットに含める。
7. Honesty: `review-protocol` 1 cycle 実施・blocker 0。evidence file が
   実在し、実行したものだけを Pass と記録する。Issue #115 は open 継続、
   PR/コミットは `Refs #115`。

## Verification（本 slice で実行した正規検証）

- `mise run fmt-check` / `mise run check synvelis-recording` /
  `mise run lint synvelis-recording` / `mise run test synvelis-recording` /
  `mise run test-rt` / `mise run check-ruviform-rev` /
  `mise run test-recording-real-input`（opt-in HW lane — device 不在なら
  test 内 loud SKIP = 正直な NotRun）/ `mise exec -- prek run --all-files`。
- 補助証拠: `cargo test -p synvelis-recording --test rcd01_oracle`
  （canonical = mise）。
