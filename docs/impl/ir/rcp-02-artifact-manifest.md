---
id: "IMPL-rcp02-artifact-manifest"
type: "impl"
status: "completed"
title: "RCP-02 Immutable Artifact Manifest・Verified Publish・CAS参照/GC・共有検証 — Headless スライス"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/291"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-render/src/artifact/mod.rs"
      - type: "file"
        path: "crates/synvelis-render/src/artifact/manifest.rs"
      - type: "file"
        path: "crates/synvelis-render/src/artifact/store.rs"
      - type: "file"
        path: "crates/synvelis-render/src/artifact/publish.rs"
      - type: "file"
        path: "crates/synvelis-render/src/artifact/lookup.rs"
      - type: "file"
        path: "crates/synvelis-render/src/artifact/gc.rs"
      - type: "file"
        path: "crates/synvelis-render/src/artifact/share.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/rcp02_artifact.rs"
      - type: "file"
        path: "docs/impl/ir/rcp-02-artifact-manifest.md"
      - type: "file"
        path: "docs/impl/exp/rcp-02-artifact-manifest.md"
      - type: "file"
        path: "docs/test-specs/rcp-02-artifact-manifest.md"
      - type: "file"
        path: "evidence/sv291/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv291/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-render/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-render/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcp-02-artifact-manifest.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/adr/rcp-00-render-recipe.md"
  derived-by: []
  verifies:
    - "docs/specs/render-checkpoint.md"
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/rcp-02-artifact-manifest.md"
    rejected: []
---

# Implementation Record: RCP-02 Immutable Artifact Manifest Headless スライス（synvelis#291）

## Summary

Issue #291 の RCP-02 として、Render Recipe 出力を mutable
cache entry ではなく immutable・検証済み derived media として
扱う `synvelis-render::artifact` module を新設した。
canonical bytes から content-address される
`ArtifactManifest`/`RenderArtifactId`、固定順序の
stage→seal→readback/digest/independent decode→atomic
commit→index CAS の `publish_artifact`、決定的 eligibility
評価の `lookup`、typed root の mark/sweep + grace +
tombstone の `sweep`、untrusted remote の fail-closed 再検証
`register_remote`、および VCS commit 履歴を書き換えない
external manifest registry `attach_external_manifest` を実装。
filesystem/OS 非結合の injectable `ArtifactStore` trait と
参照実装 `InMemoryArtifactStore`（fault injection 付き）を
提供し、RCP-STORE-01..06 を網羅する
`tests/rcp02_artifact.rs`（20 fixture）を追加した。

## Context

- 対象 Issue: https://github.com/scarecrow173/synvelis/issues/291
  （親 #289、RCP-00 契約正本は PR #421、RCP-01 Canonical
  RenderRecipe は PR #431、CB-01 VariantSet CAS は PR #425、
  RCP-04 `ArtifactEligibility`/`ReproducibilityClass` は
  PR #426 で統合済み）。
- 本スライスは **headless 実装**に限定。実 OS/filesystem
  store・実 Plugin・RT lanes・VCS integration（RCP-05）・
  cross-gate（RCP-07）は後続 Issue/ Gate の範囲。
- `synvelis-player::rcp::model` の `ArtifactEligibility`
  （Missing/Unverified/Corrupt/Stale/ValidRecipe/
  VerifiedMedia/EligibleExact/EligiblePreview/Unsupported/
  Unknown）と `ReproducibilityClass` は read-only 消費。
  lookup はこれらへ写像しつつ Issue 語彙
  （`LookupOutcome`）を変換せず返す。
- `synvelis-project::snapshot::ContentDigest`（CB-01）を
  blob/port digest の canonical 表現として再利用。
- RCP-01 の `RecipeKey`/`RecipeResolution`/`RecipeTarget`/
  `TemporalDomain`/`RendererProfile`/`PcmFormat` を消費。
  `spec.rs` の `PluginStateRecipe` とは別物。
- Store は filesystem/OS 非結合の injectable trait とし、
  headless contract test が `InMemoryArtifactStore` で
  全 fault class を検証する。Source/Raw Capture は render
  cache を source of truth にしない（本 module は derived
  media のみを扱う）。

## Implementation

### Changes

- `synvelis-render/src/artifact/manifest.rs` 新設:
  - `RenderArtifactId` — `ARTIFACT_ID_DOMAIN`
    （`synvelis/render-artifact/v1\0`）+ `sha-256` over
    canonical manifest bytes の content-addressed ID。
  - `codec_class_consistent` — codec class↔codec identity
    の manifest-domain invariant（Lossless は lossless
    codec family のみ、Lossy は非 lossless、Probe は
    任意）。publish・remote registration・lookup の
    3 境界で共有する authoritative 判定。
  - `ArtifactManifest`（schema version・recipe key・target・
    snapshot・temporal domain・cut/latency frames・ordered
    output ports・codec descriptor/class・frame count・byte
    length・content digest・renderer profile・reproducibility
    class・verification receipt・provenance・optional trust）
    を `deny_unknown_fields` 付き serde で canonical
    直列化。
  - `verify_manifest`/`decode_manifest` — schema version
    gate（`UnsupportedVersion`）・ID↔bytes 一致検証。
  - `VerificationReceipt` — whole-file digest・byte length・
    chunk digests・independent decode 詳細（container/codec/
    frames/sample rate/channels/port digests）・per-port PCM
    digest・optional PCM expectation 比較の証跡。
  - resource 上限定数（`MAX_ARTIFACT_PORTS`=64、
    `MAX_ARTIFACT_FRAMES`、`MAX_MANIFEST_BYTES`=1MiB、
    `MAX_BLOB_BYTES`=16GiB、`STAGE_CHUNK_BYTES`=64KiB）。
- `synvelis-render/src/artifact/store.rs` 新設:
  - `ArtifactStore` object-safe trait — staging
    （`stage_begin`/`stage_write`/`stage_seal`/`stage_read`/
    `stage_abort`）、atomic `commit_publish(&PublishCommit)`、
    `read_blob`/`read_manifest`/`index_entries`/`artifacts`、
    reference（`attach_reference`/`release_reference`/
    `pend_reference`/`references`）、external manifest
    （`attach_external_manifest`/`detach_external_manifest`/
    `external_manifests`/`has_external_manifest`）、GC
    （`is_quarantined`/`delete_artifact`/`tombstone`/
    `now`/`advance_clock`）。
  - `StoreError` — Conflict/NotFound/Denied/Corrupt/
    Unsupported/Io/PartialWrite/DiskFull/SyncUnknown/
    IntegrityContradiction/InvalidInput 等の型付き
    fail-closed error。
  - `PublishCommit` — staging ID・blob digest・manifest ID/
    bytes・recipe key・expected index state・optional
    reference を束ね、commit は staged bytes 検証・manifest
    identity・blob digest・expected-index CAS precondition・
    optional reference を全て検証してから mutate（all-or-
    nothing）。
  - `InMemoryArtifactStore` — 参照実装。`arm_fault`
    （stage/seal/readback/commit blob・manifest・index/
    reads/references/external/deletion の各 `FaultPoint`）、
    `corrupt_next_readback`（one-shot readback 破壊）、
    `quarantine` を備える。blob CAS refcount により残存
    manifest が参照する digest の blob は解放しない。
    quarantine ID は reference/external attach 不可。
    `valid_locator` は traversal/区切り/制御文字/空白を
    reject し長さ上限付き。
- `synvelis-render/src/artifact/publish.rs` 新設:
  - `publish_artifact` — `ArtifactDraft` +
    `PublishExpectation` + optional PCM expectation +
    optional reference + optional `CancellationToken` を受け、
    RCP-00 正本の固定順序で検証:
    stage write → seal(expected len) → staged bytes の独立
    readback + whole/chunk digest → Ruviform
    `Demuxer`/`SourceReader`（symphonia backend）による独立
    decode（writer の view を信用しない）→ codec/container
    identity・frame count・sample rate・channel 数・
    lossless/lossy/probe codec class 一貫性・port channel
    tiling・optional PCM predicate・declared port facts vs
    measured PCM の照合 → 全 pass 後に manifest 構築 →
    `PublishCommit` + expected index state で atomic commit。
  - 失敗時は staging abort + 型付き `PublishError`/`PublishStage`
    返却。cancel は abort + phase 報告。partial/intermediate
    を `Completed`/`Exact` にしない。
- `synvelis-render/src/artifact/lookup.rs` 新設:
  - `CacheLookup{recipe_key, intended_use, requested_span,
    required_ports, trust_scope, compatibility_profile}` と
    `lookup` — 現在の `RecipeResolution` が authoritative。
    `Unknown`/`Unsupported` resolution は `Unknown` outcome
    （hit を主張しない）。
  - candidate manifest の identity/index 一貫性・schema
    version・**意味的一貫性（codec class↔codec identity・
    port channel tiling — publish/share が enforce する
    manifest-domain invariant を lookup 側でも再検査し、
    store bypass 経路の矛盾 manifest を `Corrupt` にする）**・
    trust/ACL/expiry/revocation・access state・
    required span/ports/compatibility profile・codec class・
    reproducibility を fail-closed 評価。`EligibleExact` は
    lossless codec + certified/verified/replay reproducibility
    のみ。Preview は codec/repro を緩めるが **Unknown
    reproducibility は Preview でも受理しない**。quarantine
    は valid として serve しない。
  - `LookupOutcome`（Issue 語彙: EligibleExact/EligiblePreview/
    Stale/Unverified/Corrupt/Missing/Denied/Unknown/
    Unsupported）を変換せず返し、`CandidateReport` に
    rejection evidence（`FitRejection`）を保持。
    deterministic 選択: eligibility rank → reproducibility
    quality → lowest artifact ID。
- `synvelis-render/src/artifact/gc.rs` 新設:
  - `GcPolicy{grace_ticks}` + `sweep` — Active/Pending
    reference・external manifest attachment を reachability
    root として mark。Released reference は grace period 経過
    後のみ保護解除。Pending は remote reachability 未解決の
    保守的 live 扱い。unreachable artifact は
    `delete_artifact`（tombstone 記録）経由で sweep。blob は
    残存 manifest 参照が無い場合のみ解放。store/read 失敗時は
    unsafe delete しない。
- `synvelis-render/src/artifact/share.rs` 新設:
  - `register_remote` — untrusted remote manifest/blob を
    固定順序で検証: canonical manifest + artifact ID →
    trust/ACL/expiry/principal → blob presence → blob digest/
    length → independent container/codec/frame decode →
    **media consistency 全再検証**（receipt flag・decode
    receipt・top-level codec/frame/temporal 宣言・codec
    class↔codec 一貫性・port tiling・per-port PCM digest
    再計算・chunk digest 再計算 — publish が実測した全
    media fact を receipt の自己申告ではなく blob 実測で
    照合）→ idempotence → staging + `PublishCommit` による
    atomic commit。transport claim と manifest の矛盾は
    reject。既存同一 artifact は `AlreadyPresent`（冪等）。
    `ShareRejection`（ManifestInvalid/BlobMissing/
    BlobMismatch/BlobInvalid/AccessDenied/ClaimMismatch）を
    fail-closed に返す。
  - `attach_external_manifest` — VCS provider 非依存の
    locator + snapshot digest 結合で commit-associated
    manifest を external registry に登録（commit SHA 不変、
    Git 履歴書き換えなし）。現在の store logical tick を
    stamp。unsafe locator は `valid_locator` で reject。
- `synvelis-render/src/lib.rs`: `pub mod artifact` 追加。
- `synvelis-render/Cargo.toml`: `sha2` 追加（manifest ID
  domain hash）。`Cargo.lock` は共有生成物として再生成。
- `synvelis-render/tests/rcp02_artifact.rs` 新設: 20 fixture
  （RCP-STORE-01..06 を網羅）。blob fixture は手組み 16-bit
  PCM WAV を実 symphonia/`SourceReader` decode 経路に通す。

### Decisions During Implementation

- **artifact identity = canonical manifest bytes の SHA-256**
  （exp E1）: manifest が recipe・ports・codec・digest・
  provenance・receipt を全て内包するため、content-addressed
  ID が recipe 同一性を「推測」する必要がない。blob dedup
  は `ContentDigest` CAS で行い、同一 blob を共有する複数
  artifact（異 recipe/異 provenance）は manifest ID で区別
  される（RCP-STORE-03）。
- **index は recipe key → artifact ID 列の CAS**（exp E2）:
  `commit_publish` の `expected_index` precondition で 2-writer
  競合を fail-closed。latest-wins を持たず、lookup は
  deterministic rank で選択。
- **readback corruption は one-shot fault として注入可能**
  （exp E3）: `corrupt_next_readback` で staged bytes の
  readback 時に 1 byte 反転させ、digest mismatch →
  `PublishStage::Readback` fail-closed を検証。実 file
  rename/read ではなく staged bytes 経路で検証（headless
  契約）。
- **remote 検証は local publish と同じ atomic commit 経路**
  （exp E4）: `register_remote` は独自 commit を持たず、
  staging + `PublishCommit` を再利用。manifest 検証済みの
  receipt を信用せず、decode を独立に再実行して receipt
  との矛盾（lying receipt）を `BlobInvalid` で検出。
- **GC は conservative**（exp E5）: Pending reference・
  reachability 不明・external attachment は全て keep。
  Released は grace_ticks 経過後のみ sweep 対象。
  quarantine は serve も GC 保護もしない（tombstone 済みは
  再 sweep 対象外）。
- **lookup の fitness gate は Exact 昇格を許さない**
  （exp E6）: lossy codec・PreviewOnly/Unknown
  reproducibility・不完全 receipt・Denied trust・span/port/
  profile 不足はいずれも `EligibleExact` にしない。
  Preview は codec/repro を緩和するが Unknown reproducibility
  は Preview でも拒否。
- **media fact の検証は publish だけでなく remote/lookup
  境界でも再実施する**（exp E8、review 指摘の是正）:
  codec class↔codec identity 一貫性・port channel tiling・
  per-port PCM digest は manifest の自己申告であり、
  `commit_publish`/`register_remote` は publish を経由
  しない manifest を受理しうる。当初は publish のみが
  これらを enforce しており、`Lossless`+`"mp3"` のような
  矛盾 manifest が `EligibleExact` に昇格しうる gap が
  あった。`codec_class_consistent`/`ports_tile_channels`/
  `port_digest` を manifest domain の共有判定として
  `manifest.rs` に集約し、`register_remote` の
  `remote_media_consistent` で receipt flag・decode receipt・
  top-level 宣言・port tiling・per-port PCM digest・chunk
  digest を blob 実測と全照合、`lookup` でも同一 invariant
  を `Corrupt` 判定として再検査する形に修正した。

## Related Experiments

- `docs/impl/exp/rcp-02-artifact-manifest.md` — identity
  モデル・index CAS・fault 注入・remote 再検証・GC 保守性・
  fitness gate の検討記録（adopted）。

## Validation

- `mise run fmt-check` — Pass。
- `mise run check synvelis-render` — Pass。
- `mise run lint synvelis-render` — Pass（clippy `-D warnings`）。
- `mise run test synvelis-render` — Pass（`rcp02_artifact.rs`
  20 件を含む全 suite green: l2_e2e 17, rcd_04 2,
  rcp01_recipe 11, rcp02_artifact 20, sf03 2, tim04 5,
  x_e2e 5）。
- `mise run check synvelis-project` — Pass。
- `mise run check synvelis-player` — Pass。
- 詳細は `evidence/sv291/verification.md`・
  `evidence/sv291/acceptance-ledger.md`。

## Risks

- `InMemoryArtifactStore` は contract の参照実装であり、
  実 filesystem/OS の atomicity・fsync・ACL・disk-full
  semantics は未検証（後続 Gate の責務）。headless PASS を
  実 OS/storage Gate の PASS に昇格しない。
- `publish_artifact` の independent decode は Ruviform
  `SourceReader`/symphonia 経路に依存。対応 codec は
  demuxer が decode 可能なものに限られ、未対応 codec は
  `PublishStage::Decode` fail-closed（deny-by-default で
  正しいが、codec coverage は実 media Gate で要確認）。
- `lookup` は `RecipeResolution` を authoritative として
  消費するが、caller が stale/incorrect resolution を渡した
  場合の防御は resolution 側（RCP-01 `RecipeRecord::verify`/
  `evaluate_validity`）の責務。本 module は resolution の
  `Unknown`/`Unsupported` を hit に昇格させないことのみ保証。
- GC の grace は store logical tick（`advance_clock`）基準。
  実時間・外部 lease（playback reader/active job）の tick
  同期は呼び出し側責務 — 未接続の lease を Pending として
  保守的に保持する設計だが、lease の実供給は後続 slice。
- external manifest registry は locator 文字列の安全性
  （traversal/制御文字/長さ）のみ検証し、locator の実在性・
  VCS provider との結合は RCP-05 の責務。

## Follow-ups

- 実 OS/filesystem `ArtifactStore` 実装（atomic rename・
  fsync・ACL・disk-full handling）— 後続 Gate。
- Render Job/Policy（RCP-03）からの publish 呼び出し配線、
  実行経路選択（RCP-04）からの lookup 消費配線。
- VCS integration・external manifest の実 provider 結合
  （RCP-05）、cross-gate（RCP-07）。
- playback reader/active job/retention pin 等の実 lease 供給と
  GC root 接続。
- `synvelis-library` Verified Import/`synvelis-player`
  `verified_import` との artifact reuse 検討（read-only
  消費に留めた）。
