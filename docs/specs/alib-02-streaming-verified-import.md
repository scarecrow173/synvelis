---
id: "SPEC-034UviRHTzZZY5QqaoRYZV"
type: "spec"
status: "implemented"
title: "ALIB-02 — Streaming Verified Import・SHA-256 識別子（synvelis-library slice）"
created: "2026-11-09"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/85"
    - "https://github.com/scarecrow173/synvelis/issues/24"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-library/src"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/src/project.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/alib-02-streaming-verified-import-design.md"
  depends-on:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
  deferred-by: []
---

# ALIB-02 — Streaming Verified Import・SHA-256 識別子（synvelis-library slice）

## What Should Be Built

`synvelis-library` に、ALIB-01 の `AssetProvider`/`PinnedAssetRef` 契約の上に載る
**bounded-memory streaming verified import** を実装する。Provider から pin 済み
asset を chunked stream で staging へ複写しながら SHA-256 と byte 長を実測し、
staged bytes の readback 再検証を経て、SHA-256 名前空間
（`blobs/sha256/<64hex>`、object identity `"sha256:<64hex>"`）へ atomic publish、
最後に `expected_project_revision` 固定の単一 `Operation` で Project live commit
するまでの型・手順・障害分類・復旧経路を library 層で完結させる。

対象は library 契約と実装のみ。`synvelis-player`/`synvelis-app`/`synvelis-project`/
`synvelis-recording` の配線・呼び出し側は本 slice では実装しない。

## Why Now

ALIB-01（#85, PR #180 merged）で provider 境界・pin・streaming digest 検証が
揃った。次の垂直スライス（Issue #86）は「任意 Byte サイズを bounded memory で
verified import し、Crash/fault に対して正直な状態機械を持つ」ことであり、
現行 `synvelis-player::import_local` は `fs::read` で全件メモリ化し、
CRC32 系 digest・`live_revision()` 無条件 commit・dir-sync 非対応を
 durability 証拠として扱わない等の差分がある。本 slice は library 層に
その契約を先に確立し、後続の app/player 配線がその上に載れるようにする。

## Users and Value

- **後続 slice の実装者**（player/app 層）: provider 非依存の verified import
  手順・receipt・復旧 API を再利用できる。
- **Project 利用者**: 巨大・remote asset をメモリ枯渇なく import でき、
  Crash/競合時も asset/source/clip identity と provenance が保全される。
- **レビュア**: `PublishedIndeterminate`・DirSync 非対応・Orphan を
  「未公開」「保存済み」と誤報しない証拠を型とテストで得られる。

## Scope

### In Scope

- `ImportIntent`（`ImportId` 冪等キー・`expected_project_revision`・
  `PinnedAssetRef`・optional `ImportTarget`）と `ImportStore`
  （staging root + media root の path 規約）。
- Staging → Verify → Published → LiveCommit の stage machine と
  crash-safe な `ImportManifest`（atomic write、予約 ID、pin、測定 digest）。
- `BlobPublisher` port + `StagedBlobPublisher` 実装
  （`ruviform_io_file` の `StagedFile`/`SealedFile` publish pipeline を利用、
  独自の atomic rename/durability 再実装はしない）。
- `CommitVerifiedImport` `Operation`（asset+source+applied-import+optional clip
  を単一 atomic apply、reserved ID の reseed/衝突検査）。
- `PublishedIndeterminate`・DirSync 非対応の正直な分類、Orphan 保護、
  明示 `gc_orphans`、`inspect_imports`/`rebase_import` 復旧経路。
- `LocalBlobDir` への `sha256:<hex>` → `sha256/<hex>` object identity 拡張
  （additive、既存 flat digest 解決は不変）。
- 上記の crate tests と docs チェーン。

### Out of Scope

- `synvelis-player`/`synvelis-app`/`synvelis-workspace` への配線、
  `import_local` の置き換え、UI/RT 結合。
- `synvelis-project`/`synvelis-recording` の変更（`ProjectStore` の Save/HEAD
  fault は test 側 double でのみ検証）。
- v5 snapshot 互換移行（正常/不正/欠落/旧 CRC 一致でも Bytes 差）— 後続 slice。
- media decode/format 検証（SourceState を `Verified`/`Committed` へ進める
  consumer 側責務。本 slice は `Imported` で commit する）。
- Linux/macOS 実機 FS 差・DirSync 可否 — **NotRun**（Windows 実 FS は本環境で検証）。
- AudioLens baseline・Ruviform #294 Consumer Gate — 別 Gate。
- Local WAV Golden/Record/Playback/E2E の app 層結合 — 後続 slice。
- Project CAS/Undo への実配線 — library 契約のみ。

## Requirements

- 任意 Byte サイズを bounded memory で処理する（chunked copy + streaming
  SHA-256、全件 `Vec`/`fs::read` 化しない）。
- chunk/cancel/deadline/byte-budget を各 read chunk で協調チェックし、
  clean な typed 終端（`Cancelled`/`BudgetExceeded`/`IntegrityMismatch`）を返す。
- SHA-256 digest と byte 長の両方を pin 期待値・測定値・既存 blob で照合し、
  不一致を verified content として配送しない。
- `ImportId` replay は冪等（同一 content → 同一 receipt、`reused=true`）。
  同一 `ImportId` + 異 content は `IntegrityConflict`。
- `expected_project_revision` を import start 時に manifest へ永続化し、
  commit はその値を使う。stale は `StaleRevision` typed error とし、
  `live_revision()` への無条件差替えをしない。解決は明示 `rebase_import`。
- staging manifest は各 stage で atomic に永続化し、crash 直前直後で
  resume 可能。予約 ID（import/asset/source/clip）は manifest に記録し、
  reload 後の再発行を reservation-floor 前進と commit 時衝突検査で防ぐ。
- publish は `PublishEffect` の `NotPublished`/`Published`/`Indeterminate`
  を保持し、Indeterminate を「未公開」と誤報しない（destination を stat で
  reconcile してから判定する）。
- DirSync 非対応は `DirectorySync::Unsupported`/`Failed` として記録し、
  publish 自体の成否と混同しない。Mock/double FS の結果を実 durability
  証拠として報告しない。
- Orphan（publish 後 commit 失敗/crash）は blob を保持し、明示
  `gc_orphans` のみが reachability（Project 参照 + active manifest）に基づき
  削除する。staging dir は `Committed` かつ applied 確認時のみ削除候補。
- `CommittedLive`（live revision）と `SavedRevision`/`HEAD` を分離し、
  import receipt が save 完了を主張しない。
- RT 非阻合: 本 crate は blocking worker 前提のまま、RT thread/callback で
  I/O しない。`test-rt` の既存計測を regress させない。
- 既存 ALIB-01 API・挙動を破壊しない（`object_rel` 拡張は additive）。

## Acceptance Criteria

- [ ] fixture で bounded-memory streaming を実証（小 chunk provider・
  大 byte 数で全件メモリ化しないことを構造的に確認）。
- [ ] 同一 `ImportId` replay・同一 id 異 content conflict・重複登録なし。
- [ ] crash 直前直後（staging 途中失敗・publish 失敗後 resume）で
  stage machine が収束する。
- [ ] `StaleRevision` で安全に拒否され、`rebase_import` 後に冪等収束。
- [ ] 対象 lane/track 消失・他 track lane 指定を commit で拒否
  （clip の誤配置を起こさない）。
- [ ] hash mismatch・size mismatch・staged drift・既存 blob 不一致を
  `IntegrityMismatch`/`Corrupt` として拒否。
- [ ] `PublishedIndeterminate` が stat reconcile で正直に分類され、
  「未公開」誤報も「無条件成功」もない。
- [ ] DirSync `Unsupported`/`Failed` が receipt/manifest に記録され、
  publish 成否と分離される。double FS は fault 注入証拠であり
  durability 証拠ではない。
- [ ] Orphan blob/staging が保護され、`gc_orphans` が reachability に
  基づき明示削除する。
- [ ] SHA-256 identity（`sha256:<64hex>` / `blobs/sha256/<64hex>`）が
  旧 CRC16 flat blob namespace と衝突しないことを型・test で証明。
- [ ] opaque bytes（非メディア byte 列）と provenance（pin/provider
  asset/expected revision）が manifest・receipt・project に維持される。
- [ ] `CommittedLive` 後に `SavedRevision`/`HEAD` が進まないこと、
  save 失敗が import receipt を毀さないことを test で示す。
- [ ] 既存 `synvelis-library` tests が全て pass。

## Deferred Design Concerns

- v5 snapshot 互換移行（旧 CRC digest asset の読み込み・移行）|
  本 slice は新規 import のみ | project schema 所有者の slice 起票時 |
  旧 asset が `LocalBlobDir` flat 解決を必要とし続ける。
- `SourceState` の `Verified`/`Committed` 前進（media decode 検証）|
  library は byte integrity のみ検証し decode は consumer 責務 |
  player/app 層の verified-media slice |
  decode 未検証 bytes が Verified 扱いされる誤解。
- Linux/macOS 実機 DirSync/atomicity の実測 | 本環境は Windows のみ |
  対象 OS を持つ環境での検証 slice | Unsupported を Supported と誤記。
- Ruviform `BlobPublisher` 相当の汎用 publish port 昇格 | 現状は
  synvelis-library 内 port に留める | 第二 consumer が出た時点 |
  DAW 固有契約の Ruviform 混入。

## Sources and Relations

- Issue #86（本 spec の直接の根拠）、#9（Asset Library 親）、
  #85/ALIB-01（provider 契約）、#24（blob publication と project commit の
  receipt 分離）、PAP architecture（Orphan 保護・Crash matrix・Retention）。
- `docs/specs/alib-01-asset-provider-contract.md`、
  `docs/designs/alib-01-asset-provider-contract-design.md`、
  `docs/designs/project-asset-persistence-architecture.md`。
- `ruviform-io-file` `StagedFile`/`SealedFile`/`PublishEffect`/
  `DurabilityEvidence`（publish pipeline・honest classification）。
