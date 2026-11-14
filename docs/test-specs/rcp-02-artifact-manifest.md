---
id: "TSPEC-Rcp02ArtifactManifestX"
type: "test-spec"
status: "approved"
title: "RCP-02 Immutable Artifact Manifest / Verified Publish / CAS GC / shared verification fixture tests"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/291"
  verifies:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/adr/rcp-00-render-recipe.md"
  supersedes: []
  superseded_by: []
---

# RCP-02 Immutable Artifact Manifest fixture tests

## Purpose

synvelis#291 RCP-02 スライスの headless fixture が保証すべき
振る舞いを固定する。immutable `ArtifactManifest`/
`RenderArtifactId`、verified publish の fail-closed 契約、
CAS 参照 + GC eligibility、shared manifest/blob の再検証、
external manifest attach の commit SHA 不変性を
`crates/synvelis-render/tests/rcp02_artifact.rs` で検証する。
実 OS/filesystem store・実 Plugin・実 VCS provider・実 codec
coverage・実 RT は本 spec の対象外（`evidence/sv291/` で
NotRun/Deferred 記録）。

## Feature

`synvelis-render::artifact` が Render Recipe 出力を
content-addressed immutable artifact として publish・
lookup・GC・share する。`ArtifactStore` は injectable trait
で、`InMemoryArtifactStore` が headless contract の参照実装。

## Rules

- **Rule RCP-MANIFEST-ROUNDTRIP**: Manifest は canonical
  bytes から `RenderArtifactId` を導き、ordered multi-out
  port・frame count・latency/tail・format・recipe・content
  hash・provenance・verification receipt を roundtrip で
  保持する。schema version 不一致・ID↔bytes 不一致は
  `verify_manifest` が reject する。
  Examples: `rcp_store_01_manifest_roundtrip_preserves_contract_fields`.
- **Rule RCP-PUBLISH-FAILCLOSED**: Stage→seal→independent
  readback/digest/decode→atomic commit のいずれの段階で
  Cancel/Crash 相当/DiskFull/Write/Sync/ACL/Corrupt/2-writer
  競合が起きても旧 verified artifact を保持し、partial/
  intermediate を `Completed`/`Exact` にしない。
  Examples: `rcp_store_02_publish_success_then_failures_preserve_prior_artifact`,
  `rcp_store_02_fault_injection_every_stage_fails_closed`,
  `rcp_store_02_partial_write_and_cas_conflict_rejected`,
  `rcp_store_02_cancellation_at_stage_boundaries`.
- **Rule RCP-MULTI-ARTIFACT**: 同 Recipe の異 PCM は
  provenance 付き複数 artifact であり、同 ContentHash の
  blob dedup でも Recipe/artifact の同一性を推測しない。
  Examples: `rcp_store_03_same_recipe_distinct_pcm_distinct_artifacts`,
  `rcp_store_03_blob_dedup_does_not_merge_recipe_identity`.
- **Rule RCP-LOOKUP-NO-PROMOTE**: Exact/Preview/Unknown/
  Corrupt/Denied を変換せず返し、lossy/probe/format
  mismatch/欠落 port/範囲不足/未知 reproducibility を Exact
  として選択しない。Unknown reproducibility は Preview でも
  受理しない。candidate 選択は deterministic。
  Examples: `rcp_store_04_lookup_states_roundtrip`,
  `rcp_store_04_fitness_gates_never_promote_to_exact`,
  `rcp_store_04_request_fit_gates_never_select_exact`.
- **Rule RCP-GC-CONSERVATIVE**: Branch discard 後も他
  branch/歴史 commit/share/lease から参照される artifact
  を GC で削除しない。最終参照消失 + grace 満了後のみ安全
  削除し、Pending reference・external manifest root・
  shared blob は保守的に保持する。Raw Source/Take は GC
  対象外（本 module は derived media のみ）。
  Examples: `rcp_store_05_gc_grace_and_tombstone`,
  `rcp_store_05_gc_reference_lifecycle`,
  `rcp_store_05_gc_pending_and_external_roots_conservative`,
  `rcp_store_05_gc_shared_blob_refcount_and_quarantine`.
- **Rule RCP-SHARE-FAILCLOSED**: Remote manifest/blob は
  untrusted として hash/length/format/codec/frame/identity/
  schema/trust/ACL/expiry/revocation を固定順序で再検証し、
  矛盾・欠落・期限切れ・未検証・decode 不能を reject する。
  既存同一 artifact は冪等 `AlreadyPresent`。commit 後の
  external manifest attach は commit SHA を変えない。
  Examples: `rcp_store_06_remote_registration_verified_and_idempotent`,
  `rcp_store_06_remote_fail_closed_blob_and_manifest`,
  `rcp_store_06_remote_fail_closed_trust_claim_and_receipt`,
  `rcp_store_06_external_manifest_attach_commit_sha_invariant`.

## Examples

- `rcp_store_01_manifest_roundtrip_preserves_contract_fields`:
  publish → `read_manifest` → `verify_manifest` で全 contract
  field（ports/frames/latency/codec/digest/provenance/
  receipt）が保持され、index/blob も到達可能。
- `rcp_store_02_publish_success_then_failures_preserve_prior_artifact`:
  corrupt readback・seal 不一致・commit 失敗の各注入で旧
  artifact が index/blob/manifest 全てで不変。
- `rcp_store_02_fault_injection_every_stage_fails_closed`:
  全 `FaultPoint` で型付き error、staging 残留なし。
- `rcp_store_02_partial_write_and_cas_conflict_rejected`:
  seal 前の部分 write は commit 不可、古い expected_index
  は `Conflict`。
- `rcp_store_02_cancellation_at_stage_boundaries`:
  cancel が abort + phase を報告し artifact を残さない。
- `rcp_store_03_same_recipe_distinct_pcm_distinct_artifacts`:
  同 recipe の 2 blob が別 `RenderArtifactId` で共存。
- `rcp_store_03_blob_dedup_does_not_merge_recipe_identity`:
  同一 blob digest を共有する 2 manifest が index で区別
  される。
- `rcp_store_04_lookup_states_roundtrip`:
  Missing/Corrupt/Stale/Unsupported/Denied の写像。
- `rcp_store_04_fitness_gates_never_promote_to_exact`:
  lossy/PreviewOnly/Unknown repro/incomplete receipt/
  denied trust/local-under-shared が Exact に昇格しない。
- `rcp_store_04_request_fit_gates_never_select_exact`:
  span/port/profile 不足で `EligibleExact` にならず
  `FitRejection` evidence が残る。
- `rcp_store_05_gc_grace_and_tombstone`: never-referenced
  artifact が grace 経過で collected + tombstone 記録。
- `rcp_store_05_gc_reference_lifecycle`: Active→Released→
  grace→sweep の遷移。
- `rcp_store_05_gc_pending_and_external_roots_conservative`:
  Pending・external root で keep、detach 後 grace で sweep。
- `rcp_store_05_gc_shared_blob_refcount_and_quarantine`:
  shared blob は他 manifest 参照中は残り、quarantine は
  serve/attach 不可。
- `rcp_store_06_remote_registration_verified_and_idempotent`:
  remote 検証→Registered、再送→AlreadyPresent。
- `rcp_store_06_remote_fail_closed_blob_and_manifest`:
  BlobMissing/BlobMismatch/ManifestInvalid。
- `rcp_store_06_remote_fail_closed_trust_claim_and_receipt`:
  trust mutation→AccessDenied、claim 矛盾→ClaimMismatch、
  receipt 嘘→BlobInvalid。
- `rcp_store_06_external_manifest_attach_commit_sha_invariant`:
  attach で commit SHA/manifest 不変、unsafe locator reject、
  detach で GC 対象に戻る。
