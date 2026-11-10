---
id: "SPEC-034VpjuNHFlkvgLqAJQGHJ"
type: "spec"
status: "approved"
title: "RCD-02 StagedFile Publish Migration — synvelis#116 slice"
created: "2026-11-10"
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
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  defers: []
  deferred-by: []
---

# RCD-02 StagedFile Publish Migration — synvelis#116 slice

## What Should Be Built

`crates/synvelis-recording` の staging publish 経路を、手書きの
`tmp→sync→rename` 実装から ruviform `ruviform_io_file::StagedFile` /
`SealedFile::publish(PublishOptions)` ベースへ移行する。

- `publish_atomic` / `publish_atomic_faultable` の公開 signature
  （`(&Path, &[u8]) -> io::Result<()>` / `(&Path, &[u8], &mut
  PublishFaults) -> io::Result<()>`）は維持し、内部を StagedFile
  pipeline へ差し替える薄い wrapper として残す。`synvelis-player`
  （`import.rs`/`publish.rs`）が利用中のため signature 変更は禁止。
- `PublishInject`/`PublishFaults` seam（`Denied`/`SyncFail`/
  `Indeterminate` の決定的注入）は port seam 上に保全し、注入位置を
  StagedFile pipeline の対応 stage（pre-publish / file-sync 相当 /
  publish 適用後応答喪失相当）へ写像する。
- manifest publish の `exists()`→publish の TOCTOU を
  `PublishMode::CreateNew` の atomic no-replace で閉じ、競合する
  `StagingArea::create` は `AlreadyExists` として正直に失敗する。
- `PublishEffect::Indeterminate`/`PublishFailureKind`/`Durability`
  の正直な分類を維持する（効果不明を `Ok`/`NotPublished` に偽装
  しない）。`io::Result` 境界では `PublishFailure` を error source
  として保持し、分類（phase/kind/effect/achieved durability）を
  message に残す。
- 既存 fault matrix（kill/power-loss 相当・media/journal short
  write・indeterminate publish 等）を新 publish 経路上で回帰
  検証し、OS/FS 別 durability の実測行（Windows/NTFS）を evidence
  ledger へ追記する。

## Why Now

Issue #116 の残 headless 件は staging publish 経路の ruviform
`StagedFile` 移行のみである。#84（Snapshot/HEAD の StagedFile
Publish・CAS・Recovery 整合）は実装済み・PR #178 merged で、
`ruviform_io_file` は workspace dependency として pin 済み
（Cargo.lock rev `b02942c2f9e6e195d0d752803b077866468fe58e`）。
synvelis-project 側は既に `StagedFile`/`PublishMode::CreateNew`/
`FileAndDirectorySync` で snapshot/HEAD を publish しており、
recording 側 staging publish だけが手書き rename に残っている。
統合 touchpoint「#84 StagedFile/HEAD CAS」の recording 側接続を
本スライスで実現する。

## Users and Value

- 将来の統合層・検証者: staging publish が project 側と同一の
  `ruviform_io_file` primitive 上に乗り、publish 効果分類
  （NotPublished/Published/Indeterminate）と recovery handle の
  契約を共有できる。
- 保守者: temp sibling 命名・排他作成・no-replace primitive・
  dir sync pipeline を upstream（ruviform）へ一元化し、synvelis
  側の重複実装と platform 差分リスクを減らす。

## Scope

### In Scope

- `crates/synvelis-recording/` 内の publish 実装差し替え:
  `src/staging.rs`（`publish_atomic`/`publish_atomic_faultable`
  内部 + manifest publish の CreateNew 化）、`src/faultinj.rs`
  （注入 seam の doc/写像維持）、`src/writer.rs`・`src/recovery.rs`・
  `src/commit.rs`・`src/lib.rs`（呼出し整合・export）、
  `Cargo.toml`（`ruviform-io-file.workspace = true` 追加）。
- `crates/synvelis-recording/tests/` の回帰確認と、任意で
  unsynced-write 喪失 fixture（crash 後に media/journal を最終
  sync frontier へ truncate する test）1-2 件。
- `docs/` チェーン（spec/design/plan/task/test-spec 更新/impl
  record）と `evidence/reports/` ledger の #84 接続行・Windows/NTFS
  実測行更新。

### Out of Scope

- `synvelis-project` 側の HEAD CAS/migration 進化 — 残件として
  明記（#84 側 gate）。
- #86 verified-blob 実接続（lease 競合 + dep edge 規約判断が必要）
  — PR 本文「未対応の作業」に明記。
- `synvelis-player`/`synvelis-project`/`synvelis-app`/
  `synvelis-workspace`/`synvelis-gateway`/`synvelis-library`/`ui/`/
  `.github/workflows/`/`mise.toml` の変更 — player は `publish_atomic`
  signature 維持で無変更のまま動くこと。
- 録音 Raw 追記 Writer（media/journal append）の atomic write 化 —
  issue 契約どおり対象外。
- 実 OS/FS の power-loss 実測・実 device・製品 GUI（#118）—
  NotRun（実機 gate）。

## Requirements

- `publish_atomic`/`publish_atomic_faultable` の signature・戻り値
  型・公開 export を維持する（player の `import.rs`/`publish.rs`
  が無変更で compile/link できる）。
- publish は `StagedFile::create` → `write_all` → `seal` →
  `SealedFile::publish(PublishOptions)` の pipeline で行い、
  `PublishMode`/`Durability`/`VisibilityRequirement` を明示する。
  現行の保証水準（file sync + atomic visibility、dir sync は
  capability 検出済み caller 側の `sync_dir`/`sync_parent_dir`）を
  維持し、保証を過剰主張しない。
- `PublishInject::Denied` は publish 前段の拒否、`SyncFail` は
  pre-publish sync stage の失敗、`Indeterminate` は publish 適用後の
  応答喪失として、旧実装と同じ観測可能な結果（destination 非公開/
  公開済み+Err）を再現する。
- manifest publish は `PublishMode::CreateNew` で atomic no-replace
  とし、既存 manifest との競合を `AlreadyExists` として分類する。
  review record 等の冪等再公開が必要な経路は `Replace` を維持する。
- `PublishFailure` の `effect`/`phase`/`kind`/`durability` を
  `io::Error` の source/message に保持し、Indeterminate の leftover
  を自動削除しない（ruviform 契約）。
- 既存 `recovery_review`/`vs_c`/`vs_d`/`project_adapter` tests が
  無変更で Pass すること（回帰不変条件）。

## Acceptance Criteria

- [ ] Kill/Power-loss 相当・Media short write・journal short write・
      sync failure・DirSync unsupported・Permission・DiskFull・
      indeterminate publish を Fault Injection で再現する既存
      fault matrix が、新 StagedFile publish 経路上でも全て Pass
      する（回帰として再検証）。
- [ ] `publish_atomic`/`publish_atomic_faultable` の signature が
      維持され、`synvelis-player` が無変更で build/test できる。
- [ ] manifest publish が `PublishMode::CreateNew` の atomic
      no-replace で行われ、`exists()`→publish の TOCTOU が閉じる。
- [ ] `PublishInject`/`PublishFaults` seam が Denied/SyncFail/
      Indeterminate の決定的注入を保全し、注入ごとの観測結果が
      旧実装と一致する。
- [ ] OS/FS 別 Durability と Recovery 可能範囲を実測 — Windows/NTFS
      実測行（`detect_dir_sync` 等）を ledger に追記する。他 OS・
      実 power-loss は NotRun 継続で Pass 扱いしない。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
      `test-msrv` が全て Pass する。

## Deferred Design Concerns

- `synvelis-project` 側 HEAD CAS/migration の進化 | #84 側 gate・
  dep edge/lease 規約の判断が必要 | project 側 slice 実施時 |
  recording publish と HEAD CAS の結合検証が残る
- #86 verified-blob 実接続 | lease 競合 + dep edge 規約判断 |
  #86 slice 実施時 | `verified_sha256` が provenance 記録どまり
- 実 OS/FS durability 実測（他 OS・実 power-loss）| 実機 gate |
  実機環境整備後 | fake/injected fault を実 durability と誤認する
  リスク（文書で明記して抑止）

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/116>（RCD-02）
- 前提 spec: `docs/specs/rcd-02-durability-evidence-recovery-review.md`
  （task1/task2 で implemented 済みの durability/review/receipt
  契約を維持する）
- 関連 Issue: #84（StagedFile/HEAD CAS — 実装済み PR #178、
  synvelis-project 側の先例）、#86（SHA-256/Blob — 未接続）
- upstream API: `ruviform_io_file::{StagedFile, SealedFile,
  PublishOptions, PublishMode, Durability, PublishEffect,
  PublishFailure, PublishFailureKind}`（Cargo.lock rev
  `b02942c2f9e6e195d0d752803b077866468fe58e`）
