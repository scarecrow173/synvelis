---
id: "SPEC-497QYMVthAP9s510aXzuVO"
type: "spec"
status: "approved"
title: "SYN-IO-02 Windows NTFS 実機 Case lane — synvelis#84 残件 slice"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
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
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
    - "docs/specs/syn-io-02-head-cas.md"
    - "docs/specs/syn-io-02-head-cas-e2e.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-02-unsynced-loss-real-fs-probe.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-project/tests/real_fs_probe.rs"
    - "crates/synvelis-project/src/store.rs"
    - "crates/synvelis-project/tests/head_cas.rs"
    - "evidence/reports/syn-io-02-os-l1-evidence.md"
    - "POLICIES.md"
  defers: []
  deferred-by: []
---

# SYN-IO-02 Windows NTFS 実機 Case lane — synvelis#84 残件 slice

## What Should Be Built

Issue #84「まず Windows 実機」行の残件 —
`evidence/reports/syn-io-02-os-l1-evidence.md` 残ゲート表の
「Windows NTFS 実機 Case（Snapshot CreateNew→HEAD CAS/
Crash→Recovery、Failed/Indeterminate で HEAD 誤進行なし）」
= Required・NotRun の行を完結させる opt-in real-FS lane。

1. `crates/synvelis-project/tests/real_fs_probe.rs`（新規・
   全 test `#[ignore]`）: test binary self re-exec +
   env guard + go-file barrier で子 process を実 OS
   process として spawn し、`HEAD.lock` 出現 /
   `*.rvf-tmp-*` 出現 / snapshot rename 済みの trigger を
   親が spin-poll して `Child::kill`（TerminateProcess
   相当）で publish 途中に殺す。reopen して不変条件を
   assert し、`CASE=SV84-REAL-*` 行を stdout に出す。
2. `mise.toml` に `[tasks.test-project-real-fs]` を 1 件
   追加（`test-recording-real-fs` と同型・opt-in・
   CI 対象外。`--nocapture` で CASE 行を証拠転記可能に）。
3. `evidence/reports/syn-io-02-windows-real-fs.md`（新規）
   に Case ID × Requirement×Result・対象 SHA・
   Expected/Observed・実行環境を実測どおり記録し、
   `syn-io-02-os-l1-evidence.md` 残ゲート表の該当行を
   実測結果で更新する。
4. docs chain（本 spec を含む 6 file）と管理 index の
   再生成。

## Why Now

SYN-IO-02 実装本体（DirectoryBundleStore の StagedFile
publish・HEAD CAS・Recovery 移行）は main 統合済み
（PR #178・#342）だが、OS-L1 証拠は CI leg の headless
fixture に留まり、「CI Windows success を実機 Case に
読み替えない」原則により Windows NTFS 実機 Case は
NotRun のまま残っていた。本機は Windows 11/NTFS のため
opt-in lane で実測できる。RCD-02 `test-recording-real-fs`
と `head_cas.rs`/`import.rs` の self re-exec 先例が
揃っており、実装コストは test + 証拠のみ（`src/` 差分
ゼロが前提 — 契約は実装済み）。

## Users and Value

- 検証者・レビュア: Issue #84「まず Windows 実機」条件の
  Snapshot CreateNew→HEAD CAS/Crash→Recovery・Failed で
  HEAD 誤進行なしが、独立 Case ID・実測値付きで確認
  できる。
- 保守者: process kill が dirty page を失わない実測と
  合わせて、真の power-loss・Indeterminate を `NotRun`
  に留める根拠が証拠として残る。

## Scope

### In Scope

- `crates/synvelis-project/tests/real_fs_probe.rs` —
  `SV84-REAL-01`（kill-mid-publish → reopen 不変条件・
  5 trigger window）、`SV84-REAL-02`（crash 後
  `save_cas` retry 回復・stale `HEAD.lock` 実 break）、
  `SV84-REAL-03`（Failed publish・CAS `Conflict` で
  HEAD 非進行 + retry 回復）の `#[ignore]` test。
- `mise.toml` — `[tasks.test-project-real-fs]` の追加
  のみ（既存 task・記述は不変）。
- `evidence/reports/syn-io-02-windows-real-fs.md`（新規）
  と `evidence/reports/syn-io-02-os-l1-evidence.md`
  残ゲート表の該当行更新のみ。
- docs chain 6 file + 管理 index 再生成。

### Out of Scope

- `crates/*/src/**` 一切（契約は実装済み。変更が必要に
  見えた場合は実装側の問題として報告し、直さない）。
- Import↔Save 結合 Case（Issue 行の `Recovery→Import↔
  Save` 部分は sv#86/#91 責務）。
- 真の power-loss・`PublishEffect::Indeterminate` の
  実 FS 誘発（非破壊的に自動化不能・`NotRun` 維持）。
- Network/Cloud/特異 FS・macOS/Linux 実機・実
  GUI/Device/Plugin（Deferred/他 Owner）。
- GitHub-hosted runner への実機 Case 移行・新 Workflow
  増設・`*-msrv` 系タスク。
- Issue #84 の他残ゲート行（対象 SHA CI run・upstream/
  consumer gate 等）— 本 slice は Windows NTFS 実機
  Case 行のみを閉じ、Issue は tracker として open 継続。

## Requirements

- Case ID は `SV84-REAL-*` で独立に pin し、kill window
  の観測値（trigger・reopen HEAD・orphan/temp/lock 残滓）
  を `CASE=` 行として stdout に出して証拠へ転記する。
- kill-mid-publish の不変条件は「旧 HEAD 保持 or 完結
  した新 HEAD + orphan 保存」— torn HEAD・HEAD が部分
  snapshot を指す状態・orphan の byte 不完全を許さない。
  directory 全体の atomicity は主張しない。
- `save_cas` retry は crash/失敗後に一貫 HEAD へ回復
  しなければならず、stale `HEAD.lock` は実 stale path
  （手動削除ではなく `HEAD_LOCK_STALE_AFTER` の break）
  で処理される。
- Failed（publish 失敗・CAS `Conflict`）は HEAD を
  誤進行させず、foreign byte を破壊せず、clean retry が
  回復する。
- process kill を power-loss・Indeterminate の代替と
  主張しない。観測値を偽装せず、環境依存の挙動は
  print に留め契約不変条件のみ assert する。
- lane は `#[ignore]` + 専用 manual task で opt-in とし、
  既定 `cargo test`/CI に混入しない。child は bounded
  wait で wedged を fail にし、missed kill window は
  fresh bundle で retry して pass に数えない。

## Acceptance Criteria

- [ ] `real_fs_probe.rs` が `SV84-REAL-01/02/03` を
      独立 Case ID で pin し、`mise run
      test-project-real-fs` が `#[ignore]` test のみを
      実行する。
- [ ] 実 Windows NTFS での実測値が
      `syn-io-02-windows-real-fs.md` に Case ID・
      Requirement×Result・SHA・Expected/Observed・環境
      付きで記録され、power-loss/Indeterminate は
      `NotRun` のまま残る。
- [ ] `syn-io-02-os-l1-evidence.md` 残ゲート表の Windows
      NTFS 実機 Case 行が実測結果で更新される。
- [ ] `src/` 差分ゼロ・既存 test の回帰なし
      （`mise run fmt-check`/`check`/`lint`/`test`
      synvelis-project Pass）。

## Deferred Design Concerns

- 真の power-loss 実測 | 非破壊的自動化が不能 |
  外部 power-cut 実験環境 | process kill 実測を
  power-loss の代替として Pass 主張しない
- `Indeterminate` の実 FS 誘発 | ambiguous OS error が
  必要で非破壊的に作れない | ruviform-io-file 側
  fault-injection 拡張時に再検討 | consumer の分類転写は
  fixture で pin 済み
- Import↔Save 結合 Case | sv#86/#91 の責務 | 各 Issue
  gate | 本 slice の範囲外として明記
- macOS/Linux 実機 real-FS | 各 OS 実機環境 | 同一 lane
  の他 OS 実走 | Windows/NTFS の観測値を他 OS に
  一般化しない

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/84>
  （SYN-IO-02・tracker）
- 前提 spec: `docs/specs/syn-io-02-snapshot-head-staged-
  publish.md`（StagedFile publish/HEAD ordering）、
  `docs/specs/syn-io-02-head-cas.md`（CAS/`HEAD.lock`）、
  `docs/specs/syn-io-02-head-cas-e2e.md`（self re-exec
  先例）
- 先例 lane: `docs/specs/rcd-02-unsynced-loss-real-fs-
  probe.md`（opt-in real-FS probe + `NotRun` 正直管理）
- 実装先例: `crates/synvelis-project/tests/head_cas.rs`
  （e2e self re-exec）、`crates/synvelis-library/tests/
  import.rs`（LIB-IMPORT-08 crash child・読み取りのみ）
