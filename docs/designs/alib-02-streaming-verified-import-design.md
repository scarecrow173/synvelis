---
id: "DSN-alib02-streaming-import"
type: "design"
status: "approved"
title: "ALIB-02 — Streaming Verified Import 設計（synvelis-library）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/alib-02-streaming-verified-import.md"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-library/src"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/project.rs"
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-02-streaming-verified-import.md"
  implemented-by: []
  depends-on:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
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

# ALIB-02 — Streaming Verified Import 設計（synvelis-library）

## 1. 位置づけ

ALIB-01 の provider 境界（`AssetProvider::open_pinned` → `PinnedRead`）を入力とし、
「staged bytes の測定 → readback 再検証 → atomic publish → expected-revision
live commit」までを library 内で完結させる。Project の真実は
`ProjectController` 一つのまま — 本設計は第二 registry を作らない。
`ImportId` 冪等性の照合権威は `project.applied_imports`、job 状態の権威は
staging manifest、blob の権威は filesystem 上の immutable object。

## 2. モジュール構成

`src/import.rs`（公開 API + pipeline + `CommitVerifiedImport` op）に集約。
新規 port は `BlobPublisher` のみ。`LocalBlobDir::object_rel` に
`sha256:<64hex>` → `sha256/<hex>` の additive 解釈を追加し、read 側
（`ProjectAssetsProvider`）が新 namespace をそのまま解決できるようにする。

```
import.rs
  ImportIntent / ImportTarget / ImportStore
  ImportManifest / ImportStage          (serde, atomic write)
  BlobPublisher / StagedBlobPublisher   (ruviform StagedFile 利用)
  import_verified()                     pipeline driver
  rebase_import() / inspect_imports() / gc_orphans()
  CommitVerifiedImport                   Operation impl
  ImportError / 各 receipt 型
```

## 3. データフローと stage machine

```
ImportIntent{import_id, expected_project_revision, source: PinnedAssetRef,
             target: Option<{track_id,lane_id,placement,playback}>}
  │ manifest 無し → 予約 ID draw（import/asset/source[/clip]）
  │   ※ draw 前に staging_root 全 manifest の最大 reserved raw を走査し、
  │     それを超えるまで draw して進める（reload 後の ID 再利用防止）
  ▼
Started ──stream provider→source.bin (64KiB chunk, SHA-256+len 実測,
  │        cancel/deadline/max_bytes を chunk 毎に検査,
  │        pin.digest_hint / proof ContentHash / expected_size と照合)──▶ Staged
  │ manifest{content_digest, byte_len} atomic write
  ▼
Staged ──source.bin を再 stream hash (stream_digest 再利用)──▶ Verified
  │ digest+len 不一致 → Corrupt（staging drift）
  ▼
Verified ──blob publish──▶ Published
  │ dest = <media>/blobs/sha256/<hex>
  │ 既存 → open+stream_digest で照合 → Reused（不一致 → IntegrityMismatch）
  │ 無し → BlobPublisher.publish(FileAndDirectorySync)
  │   Ok → Created + durability evidence
  │   Published-effect 失敗 → dest を verify → Published 扱い（dir_sync=Failed 記録）
  │   Indeterminate → manifest flag + dest stat reconcile
  │     （在り→verify→Published / 無し→再 publish。二度目も Indeterminate →
  │      ImportError::PublishIndeterminate で終端、manifest は Verified+flag）
  │   NotPublished+Unsupported(dir sync) → FileSync で再試行、dir_sync=Unsupported
  ▼
Published ──CommitVerifiedImport apply──▶ Committed
  │ Command.expected_revision = manifest.expected_project_revision
  │ StaleRevision → ImportError::StaleRevision（rebase_import が唯一の解決）
  │ その他 reject → ImportError::CommitRejected（blob は orphan 保持）
  ▼
applied_imports に {import_id → content_hash, asset_id, source_id} を記録
```

各 manifest 遷移は `StagedFile`+`PublishMode::Replace`+`FileSync` の atomic
write。`source.bin` は working file（`File::create`+`sync_all`）で、
publication ではないため publish pipeline は通さない。

## 4. 主要な型

- `ImportIntent` — `import_id`（冪等キー）、`expected_project_revision`
  （start 時確定・manifest 永続化）、`source: PinnedAssetRef`、
  `target: Option<ImportTarget>`。
- `ImportTarget { track_id, lane_id, placement, playback }` — commit で
  lane 存在・`lane.track_id == track_id`・clip contract を atomic に検証。
- `ImportManifest` — `format_version`、予約 ID 群、pin、`content_digest`、
  `byte_len`、`stage`、`publish_indeterminate`。
- `ImportStage` — `Started/Staged/Verified/Published/Committed`。
- `BlobPublisher` port —
  `publish(staged_source: &Path, expected: &ContentDigest, expected_len,
  destination: &Path, durability: PublishDurability, cx)`。
  `staged_source` を自ら open して hash 照合しながら `StagedFile` へ copy、
  seal → `open_readback` で temp 内容を再 hash → `publish(CreateNew, …)`。
  戻り値は `BlobPublishReceipt{durability: DurabilityEvidence,
  directory_sync}` / `BlobPublishError::{Publish(BlobPublishFailure),
  Interrupted, Io, IntegrityMismatch}` — ruviform の
  `PublishEffect` trichotomy を独自型で保持（`PublishFailure` は
  crate 外で構築不可のため写し取る）。
- `CommitVerifiedImport` — `Operation` impl。validate: applied_imports
  照合（same→already applied / diff→integrity conflict）、reserved ID
  衝突（assets/sources/clips）、target の lane/track 所有と clip contract
  （`validate_window`/`extent.is_empty`/`frames_at` の public API で再現;
  `validate_references` が最終権威）。apply: `ids.peek()` より大きい
  reserved max+1 への `reseed`（reload で generator が巻き戻った場合のみ）、
  Asset/Source(`Imported`)/applied_imports/optional Clip を挿入。
- `ImportOutcome` — `{import_id, content_digest, byte_len, blob receipt,
  commit receipt{revision, asset_id, source_id, clip_id}, reused,
  publish_indeterminate, source_state: Imported}`。

## 5. SHA-256 名前空間

- object identity / `Asset.digest` / `Source.content_digest` /
  `AppliedImport.content_hash` = `ContentDigest::to_string()` =
  `"sha256:<64hex>"`。
- blob path = `<media_root>/blobs/sha256/<64hex>`。
- `LocalBlobDir`（root = `<media_root>/blobs`）の `object_rel` を拡張し、
  `sha256:` prefix + 64 hex を `sha256/<hex>` の 2-component
  `RelativePath` に写す。`:`・`..`・device 名・非 hex は従来通り拒否。
  flat な既存 digest 名（CRC16 hex 等）の解決は不変 → 旧 namespace と
  衝突しない（file 名空間・identity 文字列の両方で分離）。

## 6. 冪等・予約・復旧

- replay 権威は `project.applied_imports`。manifest 存在時は entry で
  fast-path replay（`reused=true`）。manifest 喪失後の replay は staging を
  やり直して測定 digest を `applied.content_hash` と照合 — same→
  Committed へ reconcile、diff→`IntegrityConflict`。
- 予約 ID: manifest 作成時に `draw_raw_id` で import/asset/source[/clip] を
  確保。作成前に staging_root の全 manifest を走査し max reserved raw を
  計算、generator がそこを超えるまで draw を進める（watermark が古い
  save から復元されても import 予約領域を再発行しない）。apply 時にも
  `reseed(max_reserved+1)` を保険として行う。最終的な衝突は op validate の
  `contains_key` 検査が fail-safe に拒否する。
- `undo`/旧 save からの復帰で applied_imports が消えた場合、
  `stage==Committed` のまま apply を再実行すれば同じ予約 ID で収束する
  （validate は entry 不在を確認して通る）。
- 復旧 API: `inspect_imports`（job 一覧 + applied 照合）、
  `rebase_import`（`StaleRevision` の明示解決）、`gc_orphans`
  （reachability = Project asset digest ∪ 全 manifest digest の blob を
  保護、`Committed`+applied の staging dir のみ削除）。

## 7. Durability / fault の正直さ

- publish の durability 証拠は `PublishReceipt.durability()`
  （`DurabilityEvidence`）と `DirectorySync` のみ — 要求値をそのまま
  証拠にしない。`FileAndDirectorySync` 要求が `Unsupported` なら
  `FileSync` へ capability fallback し `directory_sync=Unsupported` を記録
  （silent downgrade ではなく receipt への明示）。publish 後 dir sync
  失敗（effect=Published）は `directory_sync=Failed` で Published 扱い。
- `PublishEffect::Indeterminate` は temp/dest を区別した上で destination
  を stat reconcile する。「未公開」断定も「公開済み」断定もしない。
- Mock/double（`BlobPublisher` test impl、`ProjectStore` test impl）の
  結果は fault 注入・分類の証拠であり、実 durability PASS の根拠にしない。
  実 durability は `StagedBlobPublisher` + 実 FS の receipt のみ。
- Save/HEAD は `ProjectController::save`/`ProjectStore` の責務 — import は
  live commit までで止まり、receipt は saved を主張しない。

## 8. 境界・非目標

- media decode/format 検証は行わない → commit する `Source.state` は
  `Imported`（byte integrity 検証済みだが decode 未検証を Verified と
  偽らない）。前進は consumer の `SetSourceState` 責務。
- RT I/O なし・Tokio/HTTP/Git SDK を public API に出さない（ALIB-01 踏襲）。
- `synvelis-project`/`synvelis-recording`/`synvelis-player`/app 層は変更
  しない。呼び出し側の配線は後続 slice。
