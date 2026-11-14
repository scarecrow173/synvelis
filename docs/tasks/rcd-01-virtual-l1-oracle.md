---
id: "TASK-AUD8AWTXpU1EXRd0Jbf7ts"
type: "task"
status: "done"
title: "RCD-01 Virtual L1 Oracle — synvelis#115 Option A slice"
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
    - "docs/plans/rcd-01-virtual-l1-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-01-virtual-l1-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-01-virtual-l1-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-01 Virtual L1 Oracle — synvelis#115 Option A slice

## Tasks

- [x] T1: `rcd01_oracle.rs` に独立 oracle generator（`oracle_sample`）+
  golden const + scripted `OracleInputDevice`/`OracleInputStream`
  （`Device`/`InputStream` 実装、emission-log 記録、`jump_to` fault 注入）
  を実装。
- [x] T2: `oracle_record`/`oracle_commit_reload` helper で
  `RealDeviceRig::open` → `InputSession`/`InputAdapter` → 実
  `StreamWriter` → staging media + journal replay → `commit_session` →
  `DirectoryBundleStore` save → 別 `ProjectController::load` を束ねる。
- [x] T3: E2E fidelity test — stereo identity / multichannel identity /
  stereo device→2 mono taps / same-clock 2 device の各 case で
  commit→reload→digest resolve→emission-log bitwise 照合 +
  `verified_sha256` `sha2` 再計算 pin。
- [x] T4: fault case — input discontinuity → fail-stop → verified prefix
  oracle bit 一致 + commit 層 recoverable 分類（`Source` 非昇格）を
  commit→reload 越しに検証。mixed session（健全+faulted）も網羅。
- [x] T5: `evidence/sv115/` ledger + case matrix 作成（synthetic-L1 Pass /
  rf#628 NotRun / rf#629 NotRun(NOT_PROVISIONED) / physical NotRequired /
  real-input opt-in SKIP/NotRun / canonical blob 既存証拠参照）。
- [x] T6: doc chain（spec→design→plan→task→test-spec→impl/ir→impl/exp）
  作成 + `docs/*/README.md` index を `doc-maintenance` で再生成。
- [x] T7: 正規検証（fmt-check/check/lint/test synvelis-recording/test-rt/
  check-ruviform-rev/test-recording-real-input/prek）+ review-protocol
  1 cycle + commit/push/PR（`Refs #115`、auto-close なし）。

## Notes

- `tests/common/mod.rs` は変更せず、`RealDeviceRig::open` の
  `&dyn Device` 経路と read-only journal/media helper のみ再利用した。
- canonical blob publish 脚は `synvelis-player` 管轄のため本 slice では
  実装せず、digest adoption 契約の範囲で raw fold digest → staging media
  verified prefix bytes を解決する方式を採用（design D7）。
