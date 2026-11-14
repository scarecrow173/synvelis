---
id: "DESIGN-7xa4ndX9WpWkTxHtHv2HZs"
type: "design"
status: "approved"
title: "RCD-01 Virtual L1 Oracle Design"
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
  implements:
    - "docs/specs/rcd-01-virtual-l1-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-01-device-recording-headless-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md"
    - "docs/designs/rcd-02-blob-sha256-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-01-virtual-l1-oracle.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  defers: []
  deferred-by: []
---

# RCD-01 Virtual L1 Oracle Design

## Overview

`crates/synvelis-recording/tests/rcd01_oracle.rs`（新規 test file）に、
`MockPcm` と構造・定数を共有しない独立 deterministic oracle を inline
実装し、scripted oracle `Device`/`InputStream`（test-local）を
`RealDeviceRig::open` の `&dyn Device` 経路で駆動して、実
`StreamWriter`→staging media→journal→`commit_session`→save→reload まで
production 経路変更なしで E2E fidelity を実測する。production
`src/`・既存 test file・`Cargo.toml` は変更しない。

## Key decisions

### D1: 独立 oracle generator（`oracle_sample`）

- `MockPcm::sample`（`src/mock.rs`）は 3 項 `wrapping_mul`/XOR +
  `(v^(v>>29))*0xBF58476D1CE4E5B9` + `>>48` → u16 → `/65536` の構造。
  oracle は**異なる mixer**: `(frame, channel, seed)` を splitmix64 連鎖
  相当の乗算 hash で攪拌し、2 回の Murmur3-fmix64-style avalanche round
  （`x ^= x>>33; x*=0xff51afd7ed558ccd; x ^= x>>33; x*=0xc4ceb9fe1a85ec53;
  x ^= x>>33`）を適用し、上位 bit を `f64` 経由で `[-1, 1)` に射影して
  `f32` 化する。定数・ビット窓・射影経路のいずれも `MockPcm` と共有しない。
- 補助として同 file 内に golden `const` 配列 `ORACLE_GOLDEN`（
  `(frame, channel, seed, f32 bits)`）を inline 埋め込みし、
  `oracle_sample_golden_consts` test で出力 bit pattern を pin する。
  追加 file は lease 外のため作らない。
- **emission-log 検証**: oracle device は callback buffer に実際に書き
  出した `f32` bit pattern を `OracleStreamShared.emitted` に記録する。
  検証側は staging media の f32le bytes を decode してこの emission log
  と bitwise 照合する — generator を再計算して期待値を作るのではなく、
  「device が実際に出した bit 列」を正とする。これにより test が自分の
  generator の自己撞着にならず、callback→adapter→writer→media の
  各段の変換・順序・channel map が独立に検証される。
  未供給 frame の sentinel は `u32::MAX`（oracle 出力は有限値 `[-1,1)`
  なので衝突しない）。

### D2: oracle device / stream（test-local `Device`/`InputStream`）

- `OracleInputDevice`/`OracleInputStream` を `rcd01_oracle.rs` に実装。
  `FakeInputDevice`/`FakeStreamShared` を構造的に踏襲するが、
  `pcm: MockPcm` ではなく `oracle_sample` で slab を埋め、seed・
  device_channels・clock domain を spec 駆動にする。
- scripted handle は `Arc<Mutex<OracleStreamShared>>` で test が保持し、
  `deliver(frames)`/`jump_to`（discontinuity 注入）を駆動する。
- rig 化は `RealDeviceRig::open(&[(&dyn Device, &InputStreamSpec)])` を
  使用 — `DeviceRig` は `&FakeInputDevice` 専用で oracle device を受け
  られないため、`&dyn Device` を取る `RealDeviceRig` を選択。
  `deliver`→`pump_all`→`finish` の駆動は `DeviceRig` と同型。

### D3: oracle path は `MockPcm` 系を期待値構成に呼ばない

- oracle 経路の期待値は `oracle_sample`（emission log 経由）と
  `sha2` 再計算のみ。`MockPcm`・`verify_capture_pcm`・`verify_tap_log`・
  `verify_log`・`MockInput`/`block_pcm`・`synvelis_recording::mock::*`
  は import/call graph 上も期待値構成に登場しない。
  `tests/common/mod.rs` の read-only journal/media 解析 helper
  （`plan_for`/`plan_gated`/`tap`/`mono_spec`/`project_with_lanes`/
  `input_config`/`capture_of`/`all_journal_spans`/`assert_*`/`span_*`）は
  生成コードではないため再利用可。

### D4: E2E chain と byte resolve

- `oracle_record` helper が `RealDeviceRig::open` →
  `InputSession::open`/`add` → `StreamWriter::create` → `StagingArea::create`
  → drive（`deliver`/`pump_all`/`jump_to`）→ `finish_all` を束ね、
  `OracleRun`（rig・session・area・finalized refs・emitted handles）を返す。
- `oracle_commit_reload` が `build_payload` → `commit_session` →
  `DirectoryBundleStore` save → 別 `ProjectController::load` を行い、
  `Source.content_digest`/`Asset.digest` の整合・digest が指す bytes
  （staging media verified prefix）を再読して emission log と照合、
  `CommitStatus::AlreadyApplied` の idempotency・`applied_commits` を pin。
- receipt 無し commit では `Source.content_digest` = finalized ref の raw
  fold digest（対象 bytes = staging media verified prefix）。
  `verified_sha256`（`src/writer.rs` 由来、journaled prefix bytes の
  SHA-256）は test 内で `sha2` により独立再計算して照合する。
- journal `ChunkRecord.media_offset`/`spans[].media_offset` で media byte
  を index し、f32le decode → oracle 比較。

### D5: multi-track 構成

- (i) `oracle_stereo_device_two_mono_taps_commit_reload`: 1 stereo oracle
  stream が 2 つの mono tap（別 `ChannelMap`）で別 stream・別 staging
  media に分離され、各々 oracle 値で記録される。
- (ii) `oracle_two_devices_same_clock_commit_reload`: same-clock 2 oracle
  device が `InputSession::add` で束ねられ、別 stream に別 seed の oracle
  PCM が記録される。
- `oracle_stereo_identity_commit_reload`/`oracle_multichannel_identity_commit_reload`:
  stereo/multichannel の identity channel map での fidelity。

### D6: fault case（`oracle_input_discontinuity_verified_prefix`）

- oracle stream の `jump_to` で position gap を注入 →
  `FAULT_INPUT_DISCONTINUITY` → fail-stop。
- precedent（`device_recording.rs` の
  `fake_input_drop_fails_stop_and_preserves_verified_prefix` /
  `assert_fail_stop_prefix`）に倣い、writer の finalized outcome は
  durable frontier として `Complete` を期待し、session-scoped
  capture-fault marker が commit 層の recoverable 分類を制御する。
- journal verified prefix bytes が oracle prefix と bit 一致
  （`verified_sha256` 再計算照合含む）かつ、faulted stream が `Source`
  へ昇格せず honest に recoverable 分類されることを commit→reload 越しに
  検証。健全 stream が混在する session では健全側の commit→reload oracle
  fidelity も通す（`oracle_missing_finalized_mixed_session`）。

### D7: canonical blob publish 脚の扱い

- `publish_recorded`/`open_blob_verified`/`blob_path` は
  `synvelis-player` にあり `synvelis-recording` の下流 — test から import
  不能（`Cargo.toml` 変更 forbidden）。
- 本 slice では canonical blob publish 脚を**実装しない**。代わりに
  digest adoption 契約（`src/commit.rs` `adopt_canonical_digest`）の範囲で
  `Source.content_digest` = raw fold digest → staging media verified
  prefix bytes を解決し、`verified_sha256` を `sha2` で独立再計算して
  pin する。canonical blob publish 脚は既存証拠
  （`crates/synvelis-player/tests/rcd02_blob_digest.rs`・
  `evidence/sv116/rcd-02-blob-sha256-ledger.md`）を matrix に参照として
  明記し、本 slice で Pass 転用しない。

## Evidence / Case Matrix

`evidence/sv115/` に ledger + case matrix を作成。Required case を
synthetic-L1=実測 Pass / rf#628-fixture=NotRun / rf#629 仮想 endpoint
EXT-L3=NotRun(NOT_PROVISIONED) / physical=NotRequired / real-input
opt-in lane=実行して SKIP/NotRun を正直記録 に分類する。

## Verification

`mise run fmt-check` / `check`/`lint`/`test synvelis-recording` /
`test-rt` / `check-ruviform-rev` / `test-recording-real-input` /
`prek run --all-files`。補助証拠として `cargo test -p
synvelis-recording --test rcd01_oracle`（canonical = mise）。
