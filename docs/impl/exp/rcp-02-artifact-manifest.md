---
id: "EXP-rcp02-artifact-manifest"
title: "RCP-02 Immutable Artifact Manifest / Verified Publish / CAS GC / shared verification 設計実験メモ (synvelis#291)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: RCP-02 artifact manifest headless スライス（synvelis#291）

## E1 artifact identity を canonical manifest bytes の hash にする方式

**仮説**: RCP-STORE-03 は「同 Recipe 異 PCM は provenance
付き複数 artifact」「同 ContentHash blob dedup でも Recipe
同一性を推測しない」を要求する。artifact の identity を
recipe key や blob digest から導くと、これらを区別できない。

- 案 A: `RenderArtifactId = hash(recipe_key, blob_digest)`
  → 同 recipe・同 blob の別 provenance/別 codec class が
  衝突し、Recipe→Artifact 1:多を表現できない。
- 案 B（採用）: manifest が recipe key・ports・codec・
  content digest・renderer・reproducibility・receipt・
  provenance・trust を全て内包し、その canonical bytes の
  SHA-256（domain separator 付き）を artifact ID とする。
  blob は別途 `ContentDigest` CAS で dedup し、複数 manifest
  が同一 blob digest を参照できる。manifest bytes ↔ ID の
  一致は `verify_manifest` が fail-closed に検証する。

**観察**: `rcp_store_03_same_recipe_distinct_pcm_distinct_artifacts`
で同 recipe の異 PCM が別 artifact ID になり、
`rcp_store_03_blob_dedup_does_not_merge_recipe_identity` で
同一 blob digest を共有する 2 artifact が manifest ID で
区別され index に両方残ることを確認。

## E2 index を expected-state CAS にする方式

**仮説**: publish の atomicity は「staged bytes + manifest +
blob + index」の all-or-nothing で担保する必要がある。index
更新を無条件 write にすると 2-writer 競合で片方の artifact
が index から落ちる。

- `commit_publish` は `PublishCommit.expected_index`
  （commit 直前に読んだ index entries）を precondition とし、
  store 内 index が expected と一致しない場合は
  `StoreError::Conflict` で reject（何も mutate しない）。
  index は recipe key → artifact ID の決定的順序付き列で、
  latest-wins を持たない。

**観察**: `rcp_store_02_partial_write_and_cas_conflict_rejected`
で、先に publish して index を進めた後、古い expected_index
を持つ commit が `Conflict` で reject され、先行 artifact が
保持されることを確認。

## E3 readback 破壊を one-shot fault として注入する方式

**仮説**: RCP-STORE-02 の Corrupt 系は「staged bytes の
readback が write 時と異なる」場合に digest mismatch で
fail-closed することを要求する。実 file I/O なしでこの経路を
決定的に検証するには fault 注入が必要。

- `InMemoryArtifactStore::corrupt_next_readback` は次の
  `stage_read` のみ 1 byte 反転させる one-shot fault。
  `arm_fault(FaultPoint::…, StoreError)` は stage/seal/
  readback/commit blob・manifest・index/reads/references/
  external/deletion の各 point に任意 `StoreError` を
  注入する。

**観察**: `rcp_store_02_publish_success_then_failures_preserve_prior_artifact`
で corrupt readback → `PublishStage::Readback` の
`HashMismatch`、旧 verified artifact の index/blob/manifest
が不変であることを確認。
`rcp_store_02_fault_injection_every_stage_fails_closed` で
全 `FaultPoint` が型付き error で失敗し partial state を
残さないことを確認。

## E4 remote 検証を local publish と同一 commit 経路にする方式

**仮説**: shared manifest/blob は untrusted であり、
manifest が主張する receipt を信用してはいけない。remote
登録に独自の commit 経路を持つと local/remote で atomicity
契約がずれる。

- `register_remote` は manifest canonical 検証 → trust/ACL/
  expiry/principal → blob presence → blob digest/length →
  **独立 decode 再実行**（receipt の decode 記述と実測の
  矛盾を `BlobInvalid` で検出）→ idempotence
  （`AlreadyPresent`）→ staging + `PublishCommit` の atomic
  commit、の固定順序。transport claim（`expires_at` 等）と
  manifest の矛盾は `ClaimMismatch`。

**観察**: `rcp_store_06_remote_fail_closed_blob_and_manifest`
（BlobMissing/BlobMismatch/ManifestInvalid）と
`rcp_store_06_remote_fail_closed_trust_claim_and_receipt`
（share_verified=false/revoked/acl=None/expired →
AccessDenied、trust 無し → AccessDenied、claim 矛盾 →
ClaimMismatch、receipt 嘘 → BlobInvalid）、
`rcp_store_06_remote_registration_verified_and_idempotent`
（Registered → AlreadyPresent 冪等）を確認。

## E5 GC を conservative mark/sweep + grace + tombstone にする方式

**仮説**: RCP-STORE-05 は「branch discard 後も他 branch/
歴史 commit/share/lease から参照される artifact を消さない」
「最終参照消失 + grace 満了後のみ安全削除」を要求する。
reachability が不明なものを消すとデータ損失になる。

- `sweep` は `ArtifactReference`（Active/Pending/Released）
  と external manifest attachment を reachability root と
  して mark。Pending は remote reachability 未解決として
  保守的に live 扱い。Released は `grace_ticks` 経過後のみ
  保護解除。unreachable は `delete_artifact`（tombstone
  に tick 記録）経由。blob は残存 manifest の digest 参照が
  無い場合のみ解放（CAS refcount）。store/read 失敗時は
  sweep を中断し unsafe delete しない。

**観察**: `rcp_store_05_gc_grace_and_tombstone`（never-
referenced が grace 経過で collected + tombstone）、
`rcp_store_05_gc_reference_lifecycle`（Active→Released→
grace→sweep）、`rcp_store_05_gc_pending_and_external_roots_conservative`
（Pending・external root で keep、detach 後 grace で sweep）、
`rcp_store_05_gc_shared_blob_refcount_and_quarantine`
（shared blob は他 manifest 参照中は残り、quarantine は
serve/attach 不可）を確認。

## E6 lookup の fitness gate を Exact 昇格禁止にする方式

**仮説**: RCP-STORE-04 は Exact/Preview/Unknown/Corrupt/
Denied を変換せず返し、lossy/probe/format mismatch/欠落
port/範囲不足を Exact として選択しないことを要求する。
Media 検証（receipt 完備）だけでは Exact に昇格しない
（Input 完全性は RCP-01 の `RecipeResolution` が判定）。

- `lookup` は現在の `RecipeResolution` が authoritative。
  `Unknown`/`Unsupported` resolution は `Unknown` outcome。
  candidate は manifest integrity → quarantine → trust/ACL/
  expiry/revocation/access → span/ports/compatibility
  profile → codec class → reproducibility の順で評価し、
  `EligibleExact` は lossless codec +
  certified/verified/replay reproducibility のみ。
  Preview は codec/repro を緩和するが **Unknown
  reproducibility は Preview でも拒否**。candidate は
  eligibility rank → reproducibility quality → lowest
  artifact ID で deterministic に選択し、`CandidateReport`
  に `FitRejection` evidence を残す。

**観察**: `rcp_store_04_fitness_gates_never_promote_to_exact`
（lossy→Preview、PreviewOnly→Preview、Unknown repro→
Unknown、incomplete receipt→Unverified、denied ACL/local
trust無し under Shared→Denied、Exact が deterministic に
勝つ）、`rcp_store_04_request_fit_gates_never_select_exact`
（span/port/profile 不足で `EligibleExact` にしない +
`FitRejection` evidence）、`rcp_store_04_lookup_states_roundtrip`
（Missing/Corrupt/Stale/Unsupported の写像）を確認。

## E7 external manifest attach を VCS 非依存 registry にする方式

**仮説**: RCP-STORE-06 は「commit 後の render manifest
attach で commit SHA 不変」を要求する。Git commit を
書き換えるのではなく、provider 非依存の external registry
としてモデル化する。

- `attach_external_manifest` は `ExternalManifestRef`
  （locator + snapshot digest + artifact ID）を
  `ExternalManifestEntry`（+ store logical tick）に変換して
  registry に登録。`valid_locator` で traversal・区切り・
  制御文字・空白を reject（長さ ≤512）。quarantine ID は
  attach 不可。detach で root を外し GC grace に戻す。

**観察**: `rcp_store_06_external_manifest_attach_commit_sha_invariant`
で commit SHA/manifest bytes が attach 前後で不変、unsafe
locator（`../`、`/`、空白、長大、control 文字、空）が
reject、重複 attach が冪等、detach で GC 対象に戻ることを
確認。

## E8 media fact の検証を全境界で再実施する方式（review 是正）

**背景**: review-protocol の adversarial pass で、codec
class↔codec identity 一貫性・port channel tiling・per-port
PCM digest の検証が `publish_artifact` にのみ存在し、
`commit_publish`/`register_remote`/`lookup` が publish を
経由しない manifest を受理しうることが判明した。
`Lossless` class + `"mp3"` codec のような自己矛盾 manifest
が receipt の自己申告だけで `EligibleExact` に昇格しうる
（RCP-STORE-04 違反）。

**検討**:

- 案 A: `commit_publish` で manifest を再 decode して全
  検証 → store trait が decode 依存を持ち、headless
  contract を壊す。
- 案 B（採用）: invariant を manifest domain の共有判定
  （`codec_class_consistent`・`ports_tile_channels`・
  `port_digest`）として `manifest.rs` に集約し、各境界で
  適切な強度で再検査する。
  - `register_remote`: `remote_media_consistent` で
    receipt flag・decode receipt・top-level codec/frame/
    temporal 宣言・codec class 一貫性・port tiling・
    per-port PCM digest・chunk digest を blob 実測と
    全照合（untrusted 境界なので最大強度）。
  - `lookup`: codec class 一貫性 + port tiling を
    manifest 自己申告レベルで再検査し、矛盾は `Corrupt`
    （blob 再 decode は lookup の責務ではない — media
    fact の blob 実測照合は publish/remote 境界で済）。
  - `publish`: 従来どおり draft 検証で同じ共有判定を使用。

**観察**: `rcp_store_04_semantic_contradiction_is_corrupt`
（Lossless+mp3・Lossy+pcm・非 tiling ports が `Corrupt`）、
`rcp_store_06_remote_media_contradiction_rejected`
（codec class 矛盾・frame count 矛盾・port 非 tiling・
forged port digest・forged chunk digest・receipt flag 偽が
全て `BlobInvalid` で reject・store 無変更）を確認。
既存 18 fixture は全て green を維持。
