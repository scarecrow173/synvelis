# RCD-02 Blob SHA-256 Promotion — Evidence Ledger (synvelis#116)

Headless fixture evidence for the `synvelis-player` slice of
Issue #116 (RCD-02 録音Durability・Recovery Review・Asset
SHA-256整合). Every row records an independently observed
Expected/Observed pair from
`crates/synvelis-player/tests/rcd02_blob_digest.rs`.
`Pass` here means *the stated headless/integration-test lane
only* — it never implies real OS power-loss, real device
capture, real GUI, other-OS filesystem behavior, or product
acceptance. Those lanes are `NotRun`/`Deferred` with reasons,
per `POLICIES.md`.

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis branch | `scarecrow173/sv116-blobsha` |
| OS | Windows (win32) |
| Toolchain | rustc 1.99.1 (58cf8f9c3 2026-10-13) / cargo 1.99.1 (mise pin 系) |
| Fixture | temp dir の `StagingArea`（manifest + finalized Complete ref）+ deterministic f32le PCM media。blob publish は production `StagedBlobPublisher`、fault 注入は `publish_recorded_with` の `BlobPublisher` double |
| Lane | headless integration test のみ（`cargo test -p synvelis-player --test rcd02_blob_digest`、補助証拠; canonical = `mise run test synvelis-player`） |

Status vocabulary: `Pass` = measured pass in the stated lane;
`Fail` = measured failure; `NotRun` = lane exists but was not
measured here; `Unsupported` = capability genuinely unavailable;
`Deferred` = intentionally out of scope for this slice.

## RCD02-BS-01 — sha256 blob publish + receipt

Fixture: `publish_writes_sha256_blob_and_receipt`.

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| blob path | `<media_root>/blobs/sha256/<64hex>` に実在、flat `blobs/<digest>` には無い | 一致 | Pass |
| receipt digest | `"sha256:<64hex lowercase>"`、canonical WAV bytes の SHA-256 と一致 | 一致 | Pass |
| receipt fields | `raw_digest` provenance・`byte_len`・`source_frames`・`sample_rate`・`channels` 保持 | 一致 | Pass |
| report | `published` に 1 件、`digest`/`raw_digest`/`blob`/`byte_len`/`source_frames` が receipt と一致 | 一致 | Pass |

## RCD02-BS-02 — idempotent republish

Fixture: `republish_is_idempotent_and_reuses_verified_blob`.

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| 2 回目 publish | `reused` に 1 件・`published` は空、digest 不変 | 一致 | Pass |
| blob | 既存 blob を byte 再検証して reuse、新規 publish しない | 一致 | Pass |

## RCD02-BS-03 — missing blob republish

Fixture: `missing_sha256_blob_is_republished`.

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| blob 削除後の republish | receipt は書き換えず blob のみ再 publish、`reused` 分類 | 一致 | Pass |

## RCD02-BS-04 — tampered blob fail-closed

Fixture: `tampered_sha256_blob_fails_closed`.

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| blob bytes 改竄 | `open_blob_verified` の hash mismatch → `PublishError::CorruptMedia`、silent 上書き・trust なし | 一致 | Pass |

## RCD02-BS-05 — legacy CRC32 receipt の dual-scheme 維持

Fixture: `legacy_crc32_receipt_keeps_flat_blob`・
`opaque_receipt_digest_fails_closed`.

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| legacy hex16 receipt | flat `blobs/<digest>` に blob を確保・receipt を sha256 へ書き換えない・`reused` 分類 | 一致 | Pass |
| opaque digest receipt | content-addressed でない digest は fail-closed（`CorruptMedia`） | 一致 | Pass |

## RCD02-BS-06 — publish 失敗の observation-reconcile

Fixture: `indeterminate_publish_is_reconciled_by_observation`・
`published_effect_failure_is_reconciled_by_observation`・
`unlanded_publish_retries_once_then_fails_closed`.

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| Indeterminate + landed blob | destination の verified 観測で採用（失敗を `Ok` にも一律 `Err` にもしない） | 一致 | Pass |
| Published effect 失敗 + verified blob 実在 | 観測で採用 | 一致 | Pass |
| publish 未着地（destination 不在） | 1 回だけ retry、再失敗後に観測して fail-closed | 一致 | Pass |

## RCD02-BS-07 — provenance

Fixture: `report_carries_provenance_fields`.

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| `PublishedStream` | `raw_digest`（raw fold provenance）と `digest`（canonical sha256）を別 field で保持 | 一致 | Pass |

## Lane summary

| Lane | Status | Reason |
| --- | --- | --- |
| headless publish/recover/reconcile（本 file） | Pass | 実測（10/10 tests） |
| 実 OS power-loss / kill 相当 | NotRun | 実機・電源断 fixture は本 slice 対象外（#116 外部 gate） |
| Linux/macOS real-FS matrix | NotRun | Windows のみ実測 |
| 実 device 録音 | Deferred | synvelis#115 |
| 製品 GUI E2E | Deferred | synvelis#118 |
| consumer 照合（#86 verified blob 実接続） | NotRun | 本 slice は recording publish 側のみ |
| `synvelis-render` `x_e2e.rs` blob layout assertion | NotRun | 編集範囲外・coordinator 判断待ち |
