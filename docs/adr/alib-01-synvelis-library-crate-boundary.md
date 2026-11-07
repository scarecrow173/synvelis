---
id: "ADR-qjrwwGin4E0PE4ksCSLysY"
type: "adr"
status: "accepted"
title: "ALIB-01 — synvelis-library crate 配置と Provider 契約境界"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/85"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-library"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/85"
  implemented-by:
    - "docs/designs/alib-01-asset-provider-contract-design.md"
  depends-on:
    - "docs/specs/alib-01-asset-provider-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/gw-01-gateway-contract-crate-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/9"
  derived-by: []
---

# ADR: ALIB-01 — synvelis-library crate 配置と契約境界

## Status

Accepted（Issue #85 実装判断。review-protocol blocking 指摘なし後に確定）

## Context

Issue #85 は Provider 契約の置き場として「`synvelis-library` 新
crate、または既存 workspace 内独立 module、ADR で決定」を要求。
依存禁止は UI・ProjectStore・Git SDK・RT graph。`AssetProvider`
は `Send+Sync` の blocking/nonRT worker 契約で、Tokio/HTTP SDK を
公開 API に露出しない。

## Considered Options

- **Option 1: `synvelis-library` 新 crate** — Provider 契約・
  registry・Local/Project 基準 provider を独立 crate に集約。
  依存は `synvelis-project`（値型/Controller 投影）+ `serde`/
  `serde_json`/`sha2` のみ。
- **Option 2: `synvelis-project` に同居** — persisted model の
  正本 crate に Provider 契約を混在させると、model の責務
  （永続値・single writer）と catalog/port の責務が曖昧になる。
  却下。
- **Option 3: `synvelis-player` に同居** — import/blob 規約の隣だが、
  player は media decode/prefetch 依存を持つため契約 crate として
  重く、blocking worker 境界を越えた依存を公開 API に混入させる
  危険がある。却下。
- **Option 4: 各 consumer に散在** — 型の authoritative な単一実装を
  失い、ALIB-02〜06 が依存する共通境界を提供できない。却下。

## Decision

Option 1 を採用し、以下の責務境界を確定する。

- **所有する契約**: `ProviderId`/`OpaqueLocator`/`PrincipalId`/
  `RemoteAssetRef`/`PinnedAssetRef`/`PinProof`/`RevisionKey`/
  `ObjectIdentity`/`ContentDigest`/`ProviderCapabilities`/
  `AssetObservation<T>`/`BrowseRequest`/`BrowsePage`/`Coverage`/
  `OpaquePageCursor`/`ProviderContext`/`ProviderError`/
  `AssetProvider` trait/`ProviderRegistry`/`LocalFolderProvider`/
  `ProjectAssetsProvider`/`ProjectCatalog`/`ProjectBlobStore`。
- **所有しない契約**: Project 正本・Revision 変更
  （`ProjectController` のみ）、ProjectStore/snapshot/HEAD、
  import/Provenance（SV86）、cache（SV87）、media decode
  （synvelis-player/ruviform）、UI（SV112）。
- **依存**: `synvelis-project`（`Project`/`AssetId`/`SourceId`/
  `ImportId`/`ProjectRevision`/`ProjectController` のみ。
  `ProjectStore` は使用しない）、`serde`/`serde_json`（cursor
  符号化）、`sha2`（pin 計測と stream 検証）。
- **注入 port**: `ProjectCatalog`（controller read-only 投影）と
  `ProjectBlobStore`（blob open/stat）は trait で注入し、
  `media_root/blobs` 規約を本 crate 内に複製しない。`LocalBlobDir`
  は filename-safe 検証付きの FS 実装。
- **公開 API 非露出**: Tokio/HTTP SDK・GUI framework・RT graph 型を
  公開 API に含めない。

## Consequences

- ALIB-02(#86) は `PinnedAssetRef`/`open_pinned`/`FetchReceipt` を
  Import 境界からそのまま利用できる。`ContentDigest` は
  `sha256:<hex>` の algorithm-tagged で、legacy crc16hex とは別型。
- 新 crate 追加で `Cargo.toml` members と `Cargo.lock` が変わる
  （共有ファイル。統合時に共存）。
- cursor/observation の serde 形状は crate 内部フォーマットであり
  Project snapshot schema には影響しない。
