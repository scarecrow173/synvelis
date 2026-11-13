---
id: "SPEC-034X5xBtwHRKs4lePhNM05"
type: "spec"
status: "approved"
title: "Issue #84 DirectoryBundleStore Snapshot/HEAD Ruviform StagedFile 移行"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/82"
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
  related:
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
    - "docs/specs/syn-io-02-head-cas.md"
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
    - "docs/specs/syn-io-02-head-cas.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv84/case-ledger.md"
    - "https://github.com/scarecrow173/ruviform/issues/295"
    - "https://github.com/scarecrow173/ruviform/issues/296"
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/306"
    - "https://github.com/scarecrow173/ruviform/issues/288"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/25"
    - "https://github.com/scarecrow173/audio-lens/issues/143"
  defers:
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/306"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/25"
    - "https://github.com/scarecrow173/audio-lens/issues/143"
    - "evidence/sv84/case-ledger.md"
  deferred-by: []
---

# Issue #84 DirectoryBundleStore Snapshot/HEAD Ruviform StagedFile 移行

## What Should Be Built

`crates/synvelis-project/src/store.rs` の `DirectoryBundleStore` が持つ
独自 `.tmp` → `File::sync_all` → `fs::rename` パイプライン
（`write_atomic`）を、Ruviform `ruviform-io-file` の公開
`StagedFile`/`SealedFile`/`PublishFailure` 契約へ移管し、
Snapshot（immutable name `r<N>.json`）と HEAD（atomic Replace）
の保存・CAS・Readback・Recovery を製品の Project 永続化契約
どおりに維持する。

Issue #84 全体の canonical spec として、既に main へ統合済みの
実装 slice（`syn-io-02` 系: store 移行 PR #178・HEAD CAS
PR #342）と、本 Issue の残受入（OS-L1 証拠・Windows 実利用
FS 検証・責務境界・両 Consumer Golden 照合）を一つの契約に
束ねる。詳細な振る舞い仕様の正本は関連 spec
`docs/specs/syn-io-02-snapshot-head-staged-publish.md` と
`docs/specs/syn-io-02-head-cas.md` であり、本 spec は Issue
#84 の範囲・受入・証拠区分を固定する上位契約とする。

- **Snapshot**: immutable name の `PublishMode::CreateNew`。
  同名衝突は `snapshot_collision` が `Satisfied`/`Refresh`/
  `Conflict` に分類し、受理経路は常に勝者 byte の完全
  publish 再実行（`Replace` + `FileAndDirectorySync`）で
  snapshot 自身の directory-entry 耐久性を再確立してから
  HEAD へ進む。
- **HEAD**: snapshot publish 成功後にのみ `Replace` で差し替え。
  単一 File Replace を Project CAS と誤認しない —
  ExpectedRevision/Lock/Actor/CAS/Recovery の判断は
  Synvelis（`ProjectController`・`HEAD.lock` critical
  section・fencing）に残す。
- **失敗の正直さ**: `PublishFailure` の effect
  （`NotPublished`/`Published`/`Indeterminate`）・phase・
  達成 durability・operation id・temp path を `Error::Store`
  へ転写する。DirSync 失敗や Indeterminate で HEAD を勝手に
  進めず、`head()` 再読込/Recovery へ接続する。
- **不変条件**: `SnapshotEnvelope` の SchemaVersion・serialize
  順序・`BTreeMap` canonical byte、Saved/Live Revision、
  AutoSave/Undo、Recovery/外部変更の既存契約を維持する。
  Ruviform に ProjectId/Revision/Undo 等の DAW 概念を追加しない。

## Why Now

- Ruviform IO-IMP-01/02（ruviform #295/#296）で
  `StagedFile`/OS publish/Readback 契約が利用可能になり、
  Synvelis 独自の `.tmp`+rename は Directory Sync・
  published-but-unsynced・indeterminate の分類を持たず、
  crash/再 Open 時の状態を正直に報告できない。
- `fs::rename` ベースの旧実装は immutable snapshot 名への
  CreateNew 保証を持たず、同一 revision の異 byte 上書きを
  拒否できなかった。
- 横断依存（RF #305 撤去候補・AL #143・RF #294/#306・SV #28/
  #294）との結合検証を Issue 単位で追跡する受入単位として
  残っている — 実装は統合済みだが、OS/FS/Consumer 別の
  Required/Deferred/NotRun 証拠区分が未確定だった。

## Users and Value

- Synvelis の Project 保存経路（`ProjectController::save`/
  `save_cas`、journal restore、将来の Autosave/Backup）:
  publish 失敗時に「何が起きたか」を分類済みで受け取り、
  旧 Saved HEAD を維持したまま再読込/Recovery へ進める。
- ユーザー: crash/電源断・外部変更・古い writer・同時保存
  の後の再 Open で、store が虚偽の成功を報告せず、
  immutable snapshot が上書き破壊されず、HEAD が
  勝手に進まない。
- Ruviform/AudioLens を含む 3 リポジトリ運用: 共通基盤の
  File 契約と Consumer 固有の Project 意味論の境界を
  実測証拠で分離して追跡できる。

## Scope

### In Scope

- `DirectoryBundleStore` の staged publish 移管（snapshot
  `CreateNew`・HEAD `Replace`・`FileAndDirectorySync`・
  `RequireAtomic`）— **main 統合済み**（PR #178）。
- `ProjectStore::save_cas` expected-head CAS・`HEAD.lock`
  協調 mutex・stale-break・fencing・`HeadCasError` —
  **main 統合済み**（PR #342、Issue #330）。
- Snapshot/HEAD/CAS/ExpectedRevision/Concurrent Save/
  Crash Simulation/Recovery の Headless Fixture
  （`tests/store_publish.rs`・`tests/head_cas.rs`・
  `tests/controller_store.rs`）による書戻し・二重
  Publish・旧 Writer 上書き・Orphan 禁止の検証 —
  **main 統合済み**。
- OS-L1 証拠（Windows/macOS/Linux 既存 CI の Native
  Build/Link・対象テストの SHA/Run URL 記録）と、Windows
  で実際に利用する Project 保存先/通常 FS の Snapshot/
  HEAD CAS/Recovery の実 FS 検証 — **本 doc chain と
  `evidence/sv84/` で実施**。
- StagedFile Publish と Project Commit の責務境界、
  Ruviform #288/#306・SV #28/#294 の両 Consumer 独立
  Golden の確認記録。

### Out of Scope

- Ruviform Generic File/FS/StagedFile の再実装、DAW
  ProjectId/Revision/Undo の共通 File API への流入、
  dir-sync Indeterminate を成功/HEAD 進行に換算すること。
- #86 の schema 変換・Verified Import 統合（#86 が Owner、
  移行アルゴリズムの二重実装禁止）。
- 他 Consumer の File/IO 本体、`Cargo.toml`/`Cargo.lock`/
  `mise.toml`/CI の変更（本 Issue の doc chain では
  依存変更を行わない — 移行時の依存追加は PR #178 で
  完了済み）。
- Network/Cloud/特異 FS・物理電断の実環境 Durability、
  実 GUI/実 Device/実 Plugin（POLICIES.md §2 の
  Required/Deferred/NotApplicable 区分に従う）。
- WAV Export（Synvelis #25 が Owner）、他 crate の
  `.tmp`/rename パイプライン。

## Requirements

- Snapshot は immutable name `r<N>.json` を `CreateNew` で
  公開し、内容差異のある同名再公開を拒否する。byte 完全
  一致・`id_watermark` 単調増の同一論理 snapshot のみ
  受理し、受理は勝者 byte の `Replace`+
  `FileAndDirectorySync` 再公開で耐久性を再確立する。
- HEAD は snapshot publish 成功後にのみ `Replace` で
  差し替える。HEAD publish が `NotPublished` で失敗した
  場合 snapshot は orphan として残り旧 HEAD を保持する。
  `Published`/`Indeterminate` は HEAD が既に進んだ可能性を
  正直に分類して報告し、`head()` 再読込が実態を決める。
- `save_cas` は `HEAD.lock` critical section 内で
  expected-head を照合し、不一致は typed
  `HeadCasError::Conflict`（publish 前判定・orphan 非生成）。
  lock 喪失時は `Ok` を返さず indeterminate として報告する。
- publish 失敗は `Error::Store` へ effect/phase/durability/
  operation id/temp path 付きで転写する。`Indeterminate`
  の残留 temp は自動削除しない。
- `SnapshotEnvelope`・canonical byte・`ProjectStore` port・
  `head()`/`read_snapshot()` の公開契約を変更しない。
- 検証は `mise run check`/`lint`/`test`（対象 package 指定）
  を正規入口とし、cargo 直接実行は補助証拠と明記する。

## Acceptance Criteria

Issue #84 の受入条件を本 spec の完了条件として転記する
（2026-11-13 POLICIES.md 整合版が正本）:

- [x] DirectoryBundleStore の旧 atomic write/HEAD/Snapshot
  の変更点と Ruviform StagedFile への公開 API 移行を
  文書・コードで確定する — syn-io-02 doc chain + 本 chain
  + `store.rs` 実装で確定。
- [x] Snapshot/HEAD/CAS/ExpectedRevision/Concurrent Save/
  Crash Simulation/Recovery の Headless Fixture で書戻し・
  二重 Publish・旧 Writer 上書き・Orphan の禁止を検証する
  — `store_publish.rs`/`head_cas.rs`/`controller_store.rs`
  の契約 fixture（OS-L1 CI + Windows 実機実走で記録）。
- [ ] Windows/macOS/Linux 既存 CI の Native Build/Link、
  Project/Recording/WAV/RT/Feature の対象テストと SHA/
  実行 URL を記録し、Linux/macOS で実走した通常 FS
  テストの結果だけを Pass とする — `evidence/sv84/`
  台帳で記録（対象 SHA の CI run が未実施の場合は
  nearest-SHA 証拠と NotRun を分離して記録）。
- [x] StagedFile Publish と Project Commit の責務境界、
  Ruviform #288/#306・SV #28/#294 の両 Consumer 独立
  Golden を確認する — design/test-spec/IR に境界を記録、
  Consumer 関係 Issue は `defers` で統合・受入へ接続。
- [ ] Windows L2/L3（利用中の機能のみ）: Windows で実際に
  利用する Project 保存先/通常 FS の Snapshot/HEAD CAS/
  Recovery は、実 Windows FS の状態遷移・Readback/再 Open・
  衝突/失敗復旧を対象機能に応じて検証する — 本 dispatch
  で Windows 実機の通常 FS 実走を `evidence/sv84/` に記録。
- [ ] macOS/Linux L1 のみ: 既存 CI の Native Build/Link、
  Headless/RT/必要 Feature と製品 Oracle を対象 SHA で
  検証し、L2/L3 は利用時まで Deferred — CI 証拠を台帳化。
- [ ] 非回帰/証拠: 各 OS/Owner/CI/実機証拠を分離し、
  NotRun/Fail を Pass に読み替えない — 台帳の分類規則。

## Risks

- **受入の partial 完了**: macOS/Linux の対象 SHA CI run が
  未実施の場合、nearest-SHA 証拠と `NotRun` を分離して
  記録し、統合受入は merge 後の対象 SHA CI 証拠に委ねる
  （POLICIES.md §5: ローカル検証の成功は統合受入ではない）。
- **Crash Simulation の限界**: process kill を伴う真の
  crash 注入は headless fixture の決定的 fault seam の範囲
  外 — crash window（orphan snapshot・stale `HEAD.lock`・
  temp 残滓）の模擬で契約を pin し、物理電断は Deferred
  として分類する。
- **責務境界の誤読**: `PublishMode::Replace` の成功を
  Project Commit 成功と読み違えない — HEAD repoint が
  commit 点であり、`Indeterminate`/DirSync 未達では HEAD
  を進めない契約を test-spec/fixture で pin 済み。
