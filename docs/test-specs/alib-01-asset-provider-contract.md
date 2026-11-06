---
id: "TSPEC-DM8VnIP07x0mq4VnOt6PJD"
type: "test-spec"
status: "approved"
title: "ALIB-01 検証仕様 — Asset Provider 契約・Local/Project 基準 Provider"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/85"
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/alib-01-asset-provider-contract-design.md"
  derived-by: []
  verifies:
    - "docs/specs/alib-01-asset-provider-contract.md"
---

# ALIB-01 Test Spec

検証本体: `crates/synvelis-library/tests/{contract,local,project}.rs`
と src 内 unit tests。全て headless・実 FS tempdir・実
`Mutex<ProjectController>`（test-only `Operation` 適用）で検証。
mock は `Clock`/`ProjectCatalogSnapshot` のみ（決定性のための注入
port であり、検証対象の代替ではない）。

## Feature: Identity/契約型 (AC1, AC3)

- `ProviderId`/`OpaqueLocator`/`RemoteAssetRef`/`PinnedAssetRef`/
  `PinProof`/`ContentDigest` は `synvelis_project` の
  `AssetId`/`SourceId`/`ImportId` と別型。serde tagged round-trip。
- `OpaqueLocator` は空・NUL・長さ超過を拒否。
- `ContentDigest` は `sha256:<64hex>` 形式のみ受理（legacy crc16hex
  は `ObjectIdentity` 側で別型のまま）。
- `ProviderCapabilities` の `Unsupported{reason}` は型付き理由を
  保持。`AccessMode::ReadOnly` が両 baseline provider で宣言。
- `ProviderError` は kind と `RetryAdvice` を保持。

## Feature: Local browse/stat (AC1, AC2)

- tempdir に `a/b/kick.wav`、`a/snare.wav`、empty dir、symlink
  （root内 dir へ・root 外へ・loop 自己参照）を配置。
- `browse` page_limit=2 で Complete まで cursor paging。0件 dir は
  entries 空 + `Coverage::Complete`。budget 打切りは `Partial`。
- scope=Root / scope=Locator("a") で同一次元の entries。
- `..`・絶対・NUL・`\\` 混じり locator は拒否。root 外 target の
  symlink file は entry として列挙されるが stat/pin/open は
  `PermissionDenied`。
- stat は byte_len/modified/kind を `Observed`、media は `Unknown`。
- rename/差替え/削除後に古い cursor は `CursorInvalid` でなく
  epoch 整合のうちは動くが、pin→open で byte 差異を検出する
  （後述）。

## Feature: Local pin/open_pinned (AC1)

- pin は `ConditionalStrong{ContentHash{sha256}}` + object_identity
  ="sha256:<hex>" + expected_size。
- open_pinned で全 byte を read → EOF で receipt `verified=true` +
  digest 一致。
- pin 後に file を上書き（size 同）→ open_pinned の stream は
  `IntegrityMismatch`（completion に typed error）。
- pin 後に file を size 変更 → open_pinned は先に `SourceChanged`
  （expected_size precondition）。
- pin 後に rename → open_pinned は `NotFound`。
- dir/container の pin は typed error。

## Feature: Browse cursor 失効 (AC2)

- cursor を異なる sort/filter/scope/principal/provider/epoch で
  再利用 → `CursorInvalid`。
- decode 失敗・未知 `v`・他 provider の cursor → `CursorInvalid`。
- Project は `apply` で `ProjectRevision` が進んだ後の cursor 再利用
  → `CursorInvalid`。

## Feature: cancel/limits (AC2)

- `CancellationToken::cancel` 後の browse/read は `Cancelled`。
- `deadline` 超過は `BudgetExceeded`。
- `max_entries`/`max_depth` 打切りは `Partial` coverage。

## Feature: Local search/preview (AC2, AC3)

- `search` "kick" は name 部分一致のみ列挙。0件 + `Complete`。
- `open_preview(range)` は指定 byte のみを返す。claims は
  `VerificationPolicy::Unverified`。

## Feature: ProjectAssetsProvider (AC1, AC2, AC3)

- `Mutex<ProjectController>` に test-only `Operation` で
  Asset+Source+AppliedImport を登録。`LocalBlobDir` tempdir に
  digest 名 blob。
- browse は `AssetId` sort・revision epoch cursor・`Complete`。
- stat は source states/import provenance を `Observed`、blob
  byte_len を `Observed`。blob missing は `Availability::Missing`
  + `Unknown` byte_len。
- pin は `ImmutableObject{revision:"blob:<digest>"}` +
  digest_hint=sha256。open_pinned は EOF sha256 照合で verified。
  blob bytes 改竄（digest 名そのまま中身差替え）は
  `IntegrityMismatch`。
- search は `UnsupportedCapability`。0件 catalog は Complete 空。

## Feature: Registry (AC3)

- `ProviderRegistry` は dup `ProviderId` を拒否し、登録順で
  descriptor を返す。未登録 id は `NotFound`。

## Non-RT / 公開 API 境界 (AC3)

- `AssetProvider` object は `Send+Sync` に収まることを static
  assert。公開 API に tokio/HTTP/GUI/RT 型が現れない
  （`mise run check` + 目視レビュー）。

## 受入マトリクス

| AC | 検証 |
| --- | --- |
| AC1 | local.rs / project.rs の pin/open/mutation/traversal/identity tests |
| AC2 | local.rs の browse/cursor/cancel/search tests、project.rs cursor |
| AC3 | contract.rs + typed Unsupported + Send+Sync assert + API 非露出 |
| AC4 | `mise run fmt-check/check/lint/test` 全 workspace |
