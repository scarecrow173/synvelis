---
id: "SPEC-034Uri8JTbpWmKASolvM4A"
type: "spec"
status: "proposed"
title: "RCD-02 Recording Durability Evidence / Recovery Review — synvelis#116 slice"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-01-recording-input-adapter-channel-binding-arm-monitor-fake-port-fail-stop-s.md"
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
    - "https://github.com/scarecrow173/synvelis/issues/118"
  defers: []
  deferred-by: []
---

# RCD-02 Recording Durability Evidence / Recovery Review — synvelis#116 slice

## What Should Be Built

`crates/synvelis-recording` の VS-C/VS-D 基盤を拡張し、Issue #116 のうち
同 crate 内で完結する durability/recovery slice を実装する。

- checkpoint ごとの `DurabilityEvidence`
  （`media_synced`/`journal_synced`/`dir_sync_state`/`frontier`/
  `fs_capability`）を writer が記録し、診断・finalized ref・Recovery
  へ提供する。
- Recovery Review: `scan()` の review-pending 分類に対し、明示的な
  `Adopt`/`Hold`/`Discard` 判定 API を追加する。`Discard` は参照
  チェック（port）を必須とし、参照中 media の GC 的削除を行わない。
  review なしの自動 finalize を行わない。
- commit/save/publish-indeterminate を区別する receipt と、回復時の
  冪等再照合（commit ID・file 再読込による）を追加する。
- 冪等 re-reconciliation: 別 Start 後に同じ Session/Stream を二重
  Commit せず、duplicate take を生成しない。
- CRC32（journal/PCM 整合・commit 冪等 hash）と SHA-256 系 content
  identity（#86 領域）の用途分離を型・命名で明確化する。
- fault injection 拡張（permission denied・indeterminate publish 等）
  と fault-matrix test を追加する。

## Why Now

Issue #116 は録音 Durability・Recovery Review・Asset SHA-256 整合を
扱う。本スライスはそのうち `synvelis-recording` 内で完結する部分を
切り出したもので、#12/#15/#17 の既存契約（Raw PCM Writer/Journal/
Recovery/Project Save）を拡張・実 FS 検証する。synvelis-project /
synvelis-library への統合 adapter は別 dispatch が担当するため本
スライスでは境界（port trait）設計の文書化に留める。

## Users and Value

- 将来の統合層（synvelis-project/library adapter、#118 製品 GUI）:
  review 判定・receipt 分類・durability evidence を再利用可能な
  契約として得る。
- 検証者: kill/short write/sync fail/DirSync unsupported/DiskFull/
  permission/indeterminate publish の fault matrix で Verified
  Prefix 以外が Complete 化しないことを実測できる。

## Scope

### In Scope

- `DurabilityEvidence` 型と checkpoint 単位の記録・公開
  （`StreamWriter`、`FinalizedSourceRef`、`StreamRecovery`）。
- Recovery Review 判定 API（`Adopt`/`Hold`/`Discard`）と判定記録の
  durable 化、`Discard` の参照チェック port。
- `scan()` の review-pending 分類（`StreamRecovery` への review
  状態追加）。
- commit receipt の区別（manifest 確保 / project commit / marker
  publish / journal finalize record）と冪等 reconcile。
- `IntegrityDigest` 等の型・命名による CRC32 fold と content
  identity の分離。`StreamEntry.digest` への scheme タグ。
- `faultinj` 拡張（permission denied、indeterminate/denied publish）
  と `tests/recovery_review.rs` fault-matrix。
- `docs/` チェーン（spec/design/plan/task/test-spec/impl record）。

### Out of Scope

- `synvelis-project`/`synvelis-library` への統合 adapter
  （Recovery Review→Project Single Writer/Save/Reload/Undo の実接続）
  — 別 dispatch（sv100 系）が担当。
- #84 StagedFile/HEAD CAS・#86 Verified SHA-256/Blob Identity/
  Migration との実接続 — NotRun（各 Issue gate）。本スライスは型・
  命名分離のみ。
- Legacy v5/Asset/Project Schema migration・旧 Snapshot/HEAD 保護
  — NotRun（#86 領域）。
- 実 OS/FS の durability 実測（Windows/macOS/Linux 実機
  Input Recorder/Filesystem Capability/LowDisk/Disconnect/Crash）、
  #118 Comp/Recovery 製品 GUI — NotRun（実 OS gate）。
- RT callback 上の noalloc/noblock の実オーディオスレッド計測 —
  `test-rt` の既存検証に留める。
- `src/input.rs`（RCD-01 成果物）の変更。
- `recording-poc`・`synvelis-render`・`synvelis-player` 等の他 crate
  の変更（workspace build を壊さない後方互換を維持する）。

## Requirements

- Journal の未検証 Tail を Complete として Commit/Finalize しない。
  `recover_finalize` 系でも `FinalizeOutcome::Complete` は
  「intact prefix が CleanEnd・validated/unverified tail が共に 0」
  の場合に限る。
- Verified Prefix 以外を Complete としない。別 Start 後に同じ
  Session/Stream を二重 Commit しない（`applied_commits` registry +
  commit-manifest 再利用の既存契約を維持・検証する）。
- `Discard` 判定は参照チェック port が `Clear` を返した場合のみ
  実行し、`Referenced`/`Unknown`/port 不在は fail-closed で拒否する。
  参照中 media を削除しない。
- review 判定は durable に記録し、再 scan/再起動で冪等に再評価
  できる。`Adopt` の再実行は既存 finalized ref を返し二重生成
  しない。
- receipt は commit（registry 適用）・marker publish
  （`commit.json`）・journal finalize record・save 状態を区別し、
  publish indeterminate（`publish_atomic` 成功・応答喪失相当）を
  独立分類して冪等に再照合できる。
- CRC32/fold digest は journal 整合・commit 冪等 hash 専用と明記し、
  content identity（#86 SHA-256 系）と型・命名で混同しない。
- RT callback 経路から File IO/Project Commit を行わない
  （既存 `test-rt` 契約を維持）。
- fake/mock FS の成功を実 FS Pass に昇格しない。実測不能な OS/FS
  組合せは NotRun/Unsupported として記録する。

## Acceptance Criteria

- [ ] Kill/Power-loss 相当・Media short write・journal short write・
      sync failure・DirSync unsupported・Permission・DiskFull・
      indeterminate publish を Fault Injection で再現する
      fault-matrix test がある。
- [ ] Verified Prefix 以外を Complete としない。unverified tail を
      持つ stream は finalize/recover/commit のいずれでも
      Complete 化しない。
- [ ] 別 Start（再読込した project + 再 open した staging）後に
      同じ Session/Stream を二重 Commit せず、duplicate take を
      生成しない。
- [ ] Recovery Review の `Adopt`/`Hold`/`Discard` が明示 API で
      動作し、`Discard` は参照チェック必須・参照中 media を削除
      しない。review なしの自動 finalize を行わない。
- [ ] commit/save/publish-indeterminate が区別された receipt として
      観測でき、indeterminate は冪等に再照合される。
- [ ] CRC32（integrity fold）と content identity（#86 SHA-256 系）
      の用途が型・命名・文書で区別されている。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
      `build` が全て Pass する。

## Deferred Design Concerns

- synvelis-project/library への統合 adapter（Review→Single
  Writer/Save/Reload/Undo）| 別 dispatch が synvelis-project を編集中
  でありスコープ外 | project 側 dispatch 完了後 | review 判定と
  project commit の接続が未検証のまま残る
- #86 SHA-256 Blob identity/migration との実接続 | 各 Issue gate・
  本スライスは型分離のみ | #86 実装時 | fold digest が暫定
  content_digest として残る期間の取り扱い
- #84 StagedFile/HEAD CAS との接続 | 同上 | #84 実装時 | staging
  領域の file 管理が現行 layout のまま
- 実 OS/FS durability 実測・製品 GUI | 実機 gate・#118 | 実機環境
  整備後 | fake FS 検証を実 durability と誤認するリスク（文書で
  明記して抑止）

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/116>（RCD-02）
- 前提契約: VS-C spec/design（Journal/Checkpoint/DurableFrontier/
  recovery）、VS-D spec/design（commit manifest・applied_commits・
  commit.json marker）、RCD-01（input adapter、本スライスと並行）
- 関連 Issue: #84（StagedFile/HEAD）、#86（SHA-256/Blob）、#118
  （Comp/Recovery 製品 E2E）— いずれも統合・受入 gate であり本
  スライスの着手条件ではない
