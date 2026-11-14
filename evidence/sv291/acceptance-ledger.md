# Acceptance Ledger — synvelis#291 RCP-02 headless スライス

Date: 2026-11-14. Slice: RCP-02 Immutable Artifact Manifest・
Verified Publish・CAS 参照/GC・共有検証の
`synvelis-render::artifact` への headless 実装。実
OS/filesystem store・実 Plugin・RT lanes・VCS integration
（RCP-05）・cross-gate（RCP-07）は DEFER。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/291
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | feat/291-rcp02-artifact-manifest HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 |
| Toolchain | mise 管理 Rust（lint 出力 rustc 1.99.1、`mise run` 経由） |
| Project schema | v14（変更なし — read adapter は既存 schema を消費） |
| Recipe schema | `RECIPE_SCHEMA_VERSION` = 1（RCP-01 由来・変更なし） |
| Artifact manifest schema | `ARTIFACT_MANIFEST_VERSION` = 1（新設） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-render/tests/
  rcp02_artifact.rs` が `mise run test synvelis-render` で
  green。
- **NotRun**: 実 OS/実 filesystem store・実 Plugin・実 VCS
  provider・実 codec coverage・実 RT — 本スライスでは
  実行していない。
- **Deferred**: 後続 Issue の範囲として明示的に送った部分。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| RCP-STORE-01 | Pass (headless) | `rcp_store_01_manifest_roundtrip_preserves_contract_fields` — publish → `read_manifest` → `verify_manifest` で ordered port・frame count・latency/tail・format・recipe key・content digest・provenance・verification receipt（whole/chunk/port digest・decode 詳細）が保持され、canonical bytes ↔ `RenderArtifactId` の一致・index/blob 到達を確認。schema version 不一致は `verify_manifest` が `UnsupportedVersion` で reject（`manifest.rs` gate + lookup の `Unsupported` 写像を `rcp_store_04_lookup_states_roundtrip` で確認）。 |
| RCP-STORE-02 | Pass (headless) | `rcp_store_02_publish_success_then_failures_preserve_prior_artifact`（corrupt readback→`PublishStage::Readback` hash mismatch、seal 不一致・commit 失敗で旧 artifact の index/blob/manifest 不変）、`rcp_store_02_fault_injection_every_stage_fails_closed`（全 `FaultPoint` で型付き error・staging 残留なし）、`rcp_store_02_partial_write_and_cas_conflict_rejected`（seal 前部分 write は commit 不可・古い expected_index は `Conflict`）、`rcp_store_02_cancellation_at_stage_boundaries`（cancel→abort+phase 報告）。partial/intermediate を Completed/Exact にしないことは publish が manifest を全検証 pass 後にのみ構築する構造で担保。 |
| RCP-STORE-03 | Pass (headless) | `rcp_store_03_same_recipe_distinct_pcm_distinct_artifacts`（同 recipe の異 PCM blob が別 `RenderArtifactId` で provenance 付き共存）、`rcp_store_03_blob_dedup_does_not_merge_recipe_identity`（同一 `ContentDigest` blob を共有する 2 manifest が index で区別・blob は CAS refcount で 1 つ）。artifact identity は canonical manifest bytes の SHA-256 で recipe 同一性を推測しない。 |
| RCP-STORE-04 | Pass (headless) | `rcp_store_04_lookup_states_roundtrip`（Missing/Corrupt/Stale/Unsupported/Denied の `LookupOutcome` 写像・変換なし）、`rcp_store_04_fitness_gates_never_promote_to_exact`（lossy→Preview、PreviewOnly→Preview、Unknown repro→Unknown、incomplete receipt→Unverified、denied ACL/local trust 無し under Shared→Denied、deterministic rank で Exact が勝つ）、`rcp_store_04_request_fit_gates_never_select_exact`（span/port/profile 不足で `EligibleExact` にせず `FitRejection` evidence）、`rcp_store_04_semantic_contradiction_is_corrupt`（codec class↔codec 矛盾・port 非 tiling の manifest を `Corrupt` — review 是正で追加）。Unknown reproducibility は Preview でも拒否。 |
| RCP-STORE-05 | Pass (headless) | `rcp_store_05_gc_grace_and_tombstone`（never-referenced が publish tick + grace 経過で collected・tombstone 記録）、`rcp_store_05_gc_reference_lifecycle`（Active→Released→grace→sweep）、`rcp_store_05_gc_pending_and_external_roots_conservative`（Pending=remote reachability 未解決で保守的 keep・external manifest root で keep・detach 後 grace で sweep）、`rcp_store_05_gc_shared_blob_refcount_and_quarantine`（shared blob は残存 manifest 参照中は残り・quarantine は serve/attach 不可）。Raw Source/Take は本 module の対象外（derived media のみ）。 |
| RCP-STORE-06 | Pass (headless) | `rcp_store_06_remote_registration_verified_and_idempotent`（remote 検証→Registered・bytes/manifest/index 着地・再送→AlreadyPresent）、`rcp_store_06_remote_fail_closed_blob_and_manifest`（BlobMissing/BlobMismatch/ManifestInvalid）、`rcp_store_06_remote_fail_closed_trust_claim_and_receipt`（share_verified=false/revoked/acl=None/expired→AccessDenied・trust 無し→AccessDenied・transport claim 矛盾→ClaimMismatch・receipt 嘘→BlobInvalid）、`rcp_store_06_remote_media_contradiction_rejected`（codec class 矛盾・frame count 矛盾・port 非 tiling・forged port/chunk digest・receipt flag 偽を `BlobInvalid` で reject — review 是正で追加）、`rcp_store_06_external_manifest_attach_commit_sha_invariant`（attach で commit SHA/manifest bytes 不変・unsafe locator reject・重複冪等・detach で GC 対象に戻る）。 |

## NotRun / Deferred 一覧

- 実 OS/filesystem `ArtifactStore`（atomic rename・fsync・
  ACL・disk-full・sync uncertainty の実 semantics）: NotRun。
  `InMemoryArtifactStore` は contract 参照実装であり、
  headless PASS を実 storage Gate の PASS に昇格しない。
- 実 Plugin・実 codec coverage（symphonia demuxer が decode
  可能な codec に限る）・実 media fixture: NotRun。
- 実 VCS provider 結合・external manifest locator の実在性
  検証: NotRun（RCP-05 の責務）。
- playback reader/active job/retention pin 等の実 lease 供給
  と GC root 接続: Deferred（呼び出し側責務）。
- Render Job/Policy（RCP-03）からの publish 配線・実行経路
  選択（RCP-04）からの lookup 消費配線: Deferred。
- cross-gate（RCP-07）・`synvelis-workspace`/Gateway
  projection: Deferred（別 Worker の lease）。
- `synvelis-library` Verified Import/`synvelis-player`
  `verified_import` との artifact reuse 統合: read-only
  消費に留め、統合は後続 slice。
