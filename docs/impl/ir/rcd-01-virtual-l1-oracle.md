---
id: "IMPL-CeXimflIeKN09e4F6PKXjr"
type: "impl"
status: "completed"
title: "RCD-01 Virtual L1 Oracle — synvelis#115 Option A slice"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/tests/rcd01_oracle.rs"
      - type: "file"
        path: "evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md"
      - type: "file"
        path: "evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md"
      - type: "file"
        path: "docs/specs/rcd-01-virtual-l1-oracle.md"
      - type: "file"
        path: "docs/designs/rcd-01-virtual-l1-oracle.md"
      - type: "file"
        path: "docs/plans/rcd-01-virtual-l1-oracle.md"
      - type: "file"
        path: "docs/tasks/rcd-01-virtual-l1-oracle.md"
      - type: "file"
        path: "docs/test-specs/rcd-01-virtual-l1-oracle.md"
      - type: "file"
        path: "docs/impl/ir/rcd-01-virtual-l1-oracle.md"
      - type: "file"
        path: "docs/impl/exp/rcd-01-virtual-l1-oracle.md"
    modified:
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
      - type: "file"
        path: "docs/impl/exp/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/rcd-01-virtual-l1-oracle.md"
    - "docs/specs/rcd-01-virtual-l1-oracle.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-01-virtual-l1-oracle.md"
    - "docs/plans/rcd-01-virtual-l1-oracle.md"
    - "docs/test-specs/rcd-01-virtual-l1-oracle.md"
    - "docs/impl/exp/rcd-01-virtual-l1-oracle.md"
    - "evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md"
    - "evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-01-virtual-l1-oracle.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-01-virtual-l1-oracle.md"
  verified-by:
    - "docs/test-specs/rcd-01-virtual-l1-oracle.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/115"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/rcd-01-virtual-l1-oracle.md"
    rejected: []
---

# RCD-01 Virtual L1 Oracle — Implementation Record

## Summary

`synvelis#115` の closeout/oracle slice（Option A — synthetic-L1 最終
Oracle の確立）。`crates/synvelis-recording/tests/rcd01_oracle.rs`（新規、
約 1040 行）に、`MockPcm` と構造・定数を共有しない独立 deterministic
oracle を実装し、scripted oracle `Device`/`InputStream` を
`RealDeviceRig::open` の `&dyn Device` 経路で駆動して、実
`StreamWriter`→staging media→journal→`commit_session`→save→reload まで
production 経路変更なしで E2E fidelity を実測した。`tests/common/mod.rs`
は変更せず、`src/`・既存 test file・`Cargo.toml` も非変更。

## What was implemented

- **独立 oracle generator**（`oracle_sample`）: `(frame, channel, seed)`
  を splitmix64 連鎖相当の乗算 hash で攪拌し、2 回 Murmur3-fmix64-style
  avalanche round を適用、上位 bit を `f64` 経由で `[-1,1)` に射影して
  `f32` 化。`MockPcm`（3 項 `wrapping_mul`/XOR + `>>48` → u16 → `/65536`）
  と mixer・定数・bit window・射影経路を共有しない。golden `const` 配列
  `ORACLE_GOLDEN` で出力 bit pattern を pin。
- **emission-log 検証**: `OracleStreamShared.emitted` が callback buffer に
  実際に書き出した `f32` bit pattern を記録。検証側は staging media の
  f32le bytes を decode して emission log と bitwise 照合 — generator
  再計算に依存しない（自己撞着防止）。sentinel `u32::MAX`。
- **scripted oracle device**: `OracleInputDevice`/`OracleInputStream` が
  `Device`/`InputStream` を実装。`Arc<Mutex<OracleStreamShared>>` handle で
  `deliver(frames)`/`jump_to`（discontinuity）を駆動。`RealDeviceRig::open`
  の `&dyn Device` 経路で rig 化。
- **E2E helper**: `oracle_record`（rig→session→adapter→writer→staging→
  drive→finish_all）と `oracle_commit_reload`（build_payload→
  commit_session→DirectoryBundleStore save→別 ProjectController::load→
  digest resolve→emission-log 照合→AlreadyApplied idempotency pin）。
- **7 test**: `oracle_sample_golden_consts` /
  `oracle_stereo_identity_commit_reload` /
  `oracle_multichannel_identity_commit_reload` /
  `oracle_stereo_device_two_mono_taps_commit_reload` /
  `oracle_two_devices_same_clock_commit_reload` /
  `oracle_input_discontinuity_verified_prefix` /
  `oracle_missing_finalized_mixed_session`。
- **fault case**: input discontinuity → `FAULT_INPUT_DISCONTINUITY` →
  fail-stop。writer finalized outcome は durable frontier として `Complete`
  を期待し、commit 層 recoverable 分類は session-scoped capture-fault
  marker が制御（`fake_input_drop_fails_stop_and_preserves_verified_prefix`
  先例に準拠）。verified prefix の oracle bit 一致 + `verified_sha256`
  `sha2` 再計算照合 + `Source` 非昇格を commit→reload 越しに検証。
- **digest 独立照合**: receipt 無し commit の `Source.content_digest` =
  finalized ref の raw fold digest → staging media verified prefix bytes
  を journal `ChunkRecord.media_offset`/`spans[].media_offset` で index
  して f32le decode → oracle 比較。`verified_sha256` を `sha2` で再計算。
- **doc chain + evidence**: slug `rcd-01-virtual-l1-oracle` の
  spec/design/plan/task/test-spec/impl-ir/impl-exp + `evidence/sv115/`
  ledger/case-matrix を作成。

## Decisions made during implementation

- `DeviceRig` は `&FakeInputDevice` 専用で oracle device を受けられない
  ため、`&dyn Device` を取る `RealDeviceRig::open` を選択（design D2）。
- fault case の finalized outcome を `Partial` と当初誤認したが、
  precedent を確認し durable frontier `Complete` + capture-fault marker
  による recoverable 分類が正しい契約と判断して修正（production 変更
  なし・test 側の期待値を precedent に合わせた）。
- canonical blob publish 脚は `synvelis-player` 管轄・`Cargo.toml` 変更
  forbidden のため実装せず、digest adoption 契約の範囲で raw fold digest
  → staging media verified prefix bytes を解決する方式を採用（design D7）。

## Verification performed

- `cargo test -p synvelis-recording --test rcd01_oracle` — 7 pass / 0 fail
  （補助証拠。canonical = mise）。
- `cargo clippy -p synvelis-recording --test rcd01_oracle -- -D warnings` —
  pass（補助証拠）。
- `rustfmt --edition 2024 --check` — clean。
- 正規 `mise run` 検証・`prek`・review-protocol は本 record 確定前に
  別途実行・記録（下記 Verification 欄または evidence ledger を参照）。

## Out of scope / deferred

- rf#628 fixture variant（`scarecrow173/ruviform#628` OPEN、pinned
  ruviform `fd1d4d5` に fixture 未収録）→ NotRun。
- rf#629 仮想 endpoint EXT-L3（`scarecrow173/ruviform#629` OPEN、
  環境未構築）→ NotRun(NOT_PROVISIONED)。
- physical HW・GUI（Option B）→ NotRequired/Deferred。
- canonical blob publish 脚 → `synvelis-player` 管轄既存証拠参照。
- Issue #115 は open 継続（`Refs #115` のみ、auto-close なし）。
