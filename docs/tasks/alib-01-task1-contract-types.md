---
id: "TASK-kMjCKWyai7zEXTluv8aW1T"
type: "task"
status: "done"
title: "ALIB-01 task1 — synvelis-library crate + 契約型/registry/digest"
created: "2026-11-07"
updated: "2026-11-07"
owners: []
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
  implements:
    - "docs/plans/alib-01-asset-provider-baseline.md"
  implemented-by: []
  depends-on:
    - "docs/plans/alib-01-asset-provider-baseline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
---

# ALIB-01 task1 — crate skeleton + 契約型

## 内容

- ルート `Cargo.toml` members に `crates/synvelis-library` を追加。
- `Cargo.toml`: workspace package/lints 継承、deps =
  `synvelis-project`/`serde`/`serde_json`/`sha2`。
- `id.rs`: `ProviderId`(u128, "provider-<32hex>" tagged),
  `OpaqueLocator`(validated), `PrincipalId`, `CredentialKey`,
  `OpaqueCredential`。
- `refs.rs`: `RemoteAssetRef`, `RevisionKey`, `ObjectIdentity`,
  `ContentDigest`, `PreconditionKind`, `StrongRevisionToken`,
  `PinProof`, `PinnedAssetRef`。
- `capability.rs`: `Capability`, `UnsupportedReason`, `PinCapability`,
  `AccessMode`, `ProviderCapabilities`。
- `observation.rs`: `ObservedAt`, `ObservationOrigin`,
  `InferenceMethod`, `UnavailableReason`, `StaleReason`,
  `AssetObservation<T>`, `AssetKind`, `Availability`, `EntryFacts`,
  `AssetStat`。
- `browse.rs`: `BrowseScope`, `SortSpec`, `FilterSpec`,
  `BrowseRequest`, `BrowseEntry`, `EntryKind`, `Coverage`,
  `BrowsePage`, `SearchQuery`, `OpaquePageCursor`（hex(JSON) 符号化
  + fingerprint/epoch 検証）。
- `context.rs`: `ProviderContext`, `CancellationToken`,
  `ProviderLimits`, `Clock`/`SystemClock`, `CredentialResolver`。
- `error.rs`: `ProviderErrorKind`, `RetryAdvice`, `ProviderError`。
- `provider.rs`: `AssetProvider` trait, `ByteRange`, `FetchClaims`,
  `VerificationPolicy`, `PinnedRead`, `FetchCompletion`,
  `FetchReceipt`。default `search`/`open_preview` は typed
  `UnsupportedCapability`。
- `digest.rs`: streaming sha256 + EOF 照合 verify reader +
  cancel/deadline check。
- `registry.rs`: `ProviderDescriptor`, `ProviderKind`,
  `ProviderRegistry`。
- `lib.rs`: crate doc + re-export。
- unit tests: ID tagged round-trip、locator validation、cursor
  encode/decode・失効、error advice、digest verify reader、
  `Send+Sync` static assert。

## 検証

`cargo test -p synvelis-library` が全 unit test GREEN
（正規: `mise run test` の一部）。`mise run fmt-check`。
