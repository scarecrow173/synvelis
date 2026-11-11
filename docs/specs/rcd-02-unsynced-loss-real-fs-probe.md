---
id: "SPEC-MtkyEBkLAdbCHkkziVNHIk"
type: "spec"
status: "approved"
title: "RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe — synvelis#116 slice"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
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
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  defers: []
  deferred-by: []
---

# RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe — synvelis#116 slice

## What Should Be Built

Issue #116 の残存 slice。2 つの成果物を作る。

1. **unsynced-write 喪失 fixture（headless）**: `checkpoint()` が
   完了した時点の media/journal 実 file 長を「最終 sync
   frontier」とみなし、checkpoint 後に `write_all` が成功した
   unsynced byte を `set_len` で切り捨てることで、OS が dirty
   page を捨てる power-loss 相当の状態を決定的に再現する
   test を `crates/synvelis-recording/tests/recovery_review.rs`
   に追加する。scan / review Adopt が verified prefix
   （Last Valid Checkpoint）以外を `Complete` にしないことを
   検証する。
2. **Windows/NTFS real-FS probe（opt-in lane）**:
   `recording-poc` に `realfs` サブコマンドを追加し、実 FS 上で
   rename 原子性・dir sync capability・unsynced tail の実挙動を
   観測して key=value で出力する。`crates/recording-poc/tests/
   real_fs_probe.rs` の `#[ignore]` test から呼び、`mise run
   test-recording-real-fs`（新規 manual task・CI 対象外）で
   実行する。実測値を `evidence/reports/
   rcd-02-durability-recovery-review-ledger.md` に実測行として
   追記する。

## Why Now

Issue #116 の受入「Kill/Power-loss 相当 … Fault Injection」と
「OS/FS 別 Durability と Recovery 可能範囲を実測し、
NotRun/Unsupported を Pass 扱いしない」について、task1-3 で
durability evidence・recovery review・fault matrix・
StagedFile publish を統合済みだが、次の 2 点が残っている。

- fault matrix は in-process の `FaultWriter`/byte 編集による
  注入であり、「`write_all` 成功後だが `sync_data` 未到達の
  byte が OS に捨てられる」喪失形（真の unsynced-write 喪失）を
  直接は再現していない。file 長を最終 sync frontier へ戻す
  truncation が、その喪失形を決定的に再現する最小モデル。
- ledger の OS/FS 実測行は `detect_dir_sync` の Windows 観測値
  （manifest 記録）に留まり、rename 原子性・publish 経路・
  unsynced tail の**実 FS 挙動の独立実測**が無い。本機は
  Windows/NTFS のため opt-in lane で実測できる。真の
  power-loss（電源断）は非破壊的に自動化できないため
  `NotRun` のまま正直に残し、process kill（exit/abort 相当）で
  代替可能な範囲だけ実測する。

## Users and Value

- 検証者・レビュア: unsynced-write 喪失が headless fixture
  として決定的に再現され、verified prefix 契約が喪失形の
  違い（対称・非対称）で破られないことを確認できる。
- 保守者: Windows/NTFS の rename 原子性・dir sync capability・
  process kill 後の unsynced byte 実挙動が実測値として ledger
  に残り、`Pass`/`NotRun`/`Unsupported` の主張が実測に裏付け
  られる。process kill が unsynced byte を失わないという実測は、
  「kill 相当 ≠ power-loss」を証拠付け、power-loss 行を
  `NotRun` に留める根拠になる。

## Scope

### In Scope

- `crates/synvelis-recording/tests/recovery_review.rs` への
  unsynced-loss fixture test 1-2 件（headless・temp FS 上で
  決定的）。
- `crates/recording-poc/src/main.rs` への `realfs`
  サブコマンド追加（`probe` / `write-unsynced`）と
  `crates/recording-poc/tests/real_fs_probe.rs` の `#[ignore]`
  opt-in test。
- `mise.toml` への `test-recording-real-fs` manual task 1 件
  （既存 `test-recording-real-input` と同型・opt-in・CI 対象外。
  real-FS lane 実現に必要なため本 spec が例外として許可する）。
- `evidence/reports/rcd-02-durability-recovery-review-ledger.md`
  への実測行追記（実測した行のみ `Pass`、未実施は理由付き
  `NotRun`/`Unsupported`）。
- docs チェーン（spec/design/plan/task/test-spec/impl-ir）と
  管理 index の再生成。

### Out of Scope

- 真の power-loss（電源断）の実測 — 非破壊的に自動化不能。
  `NotRun` 維持で `Pass` 扱いしない。
- verified blob / Asset 登録（#86）、project 側 HEAD CAS 進化
  （#84）、consumer 照合（#219/#294）— 別 Issue 帰属。
- `crates/synvelis-recording/src/` の実装変更 — 本 slice は
  fixture/probe のみで、writer/recovery/staging の契約は
  変更しない（契約違反が見つかった場合は実装ではなく報告する）。
- `synvelis-library`/`synvelis-app`/`synvelis-project`/
  `synvelis-player`/`.github/workflows/` の変更。
- 他 OS（macOS/Linux）の FS matrix — `NotRun`。
- 実 device 録音（#115）・製品 GUI（#118）。

## Requirements

- unsynced-loss fixture は「checkpoint 完了時の実 file 長 =
  最終 sync frontier」として truncate し、frontier 以後の
  byte/record が消失した状態を再現する。対称喪失（media/journal
  両方）と非対称喪失（片側のみ残留）を区別して検証する。
- Adopt/recover は verified prefix（Last Valid Checkpoint の
  `media_len`）のみを確定し、Finalize record を持たない
  crash 由来の finalize は `Partial` に留まる。unverified/
  journaled tail は byte として保存され、`Complete` に昇格
  しない。
- probe は実 FS 上の観測値をそのまま報告し、能力の有無を
  推測で偽装しない（`detect_dir_sync` と同じ正直さ）。
  process kill 後の観測は「kill が unsynced byte を失わせない」
  実測として記録し、power-loss の代替とは主張しない。
- real-FS lane は `#[ignore]` + 専用 manual task で opt-in とし、
  既定 `mise run test` / CI に混入しない。
- 既存の fault matrix・staged publish・headless 全 test が
  回帰しない。

## Acceptance Criteria

- [ ] checkpoint 後の unsynced write が sync frontier まで
      喪失した状態を再現する headless fixture が存在し、
      Adopt が verified prefix のみを `Partial` で確定し、
      喪失分を含めて `Complete` にしないことが検証される。
      非対称喪失（unverified tail 残留 / journaled 被覆超過）
      でも同契約が保たれる。
- [ ] Windows/NTFS（本機）で rename 原子性・dir sync
      capability・unsynced tail の実挙動を実測し、観測値が
      ledger に実測行として記録される（実測した行のみ
      `Pass`、未実施は理由付き `NotRun`、真の power-loss は
      `NotRun` 維持）。
- [ ] 実測/NotRun/Unsupported を `Pass` 扱いしない。
- [ ] 既存の recording fault matrix・staged publish・headless
      全 test が回帰しない（`mise run test` Pass）。

## Deferred Design Concerns

- 他 OS（macOS/Linux）の real-FS matrix | 実機 gate | 各 OS
  実機環境で `test-recording-real-fs` を実行 | Windows/NTFS の
  観測値を他 OS に一般化しない
- 真の power-loss 実測 | 非破壊的自動化が不能 | 外部
  power-cut 実験環境の整備 | process kill 実測を power-loss の
  代替として Pass 主張しない
- staging 層の resume append（crash 後に同一 stream へ追記
  継続）| 現契約では recovery が責務を持ち writer create は
  空 journal のみ許容 | 別 slice 判断 | unsynced 喪失と resume
  の組合せ検証が残る

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/116>（RCD-02）
- 前提 spec: `docs/specs/rcd-02-durability-evidence-recovery-review.md`
  （durability/review/receipt 契約）、
  `docs/specs/rcd-02-stagedfile-publish-migration.md`
  （StagedFile publish 経路）
- 関連 Issue: #84（StagedFile/HEAD CAS）、#86（SHA-256/Blob）
- 既存 fixture: `crates/synvelis-recording/tests/recovery_review.rs`、
  `crates/recording-poc/tests/vsc_process.rs`（process kill →
  recover 先例）
