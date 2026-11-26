---
id: "DESIGN-Tim04PermissionMatrix01"
type: "design"
status: "approved"
title: "TIM-04 permission fault matrix design"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
---

# TIM-04 permission fault matrix design

## 責務境界

`PluginStatePort` は plugin-originated capture/restore outcome を返す。`PluginStateStore` は content-addressed blob、immutable manifest、mutable HEAD の persistence を担う。Permission denial の分類は store/session 境界に集約し、player adapter や evidence writer が文字列を解釈しない。

`PluginStateStoreOperation` は `BlobPublish`、`ManifestPublish`、`HeadUpdate`、`HeadRead`、`ManifestRead`、`BlobRead` を区別する。`PluginStateStoreError::PermissionDenied` は operation と publish effect (`NotPublished`/`Published`/`Indeterminate`/`NotApplicable`) を保持する。ruviform の publish kind は permission を第一級区別しないため、`DirectoryPluginStateStore` は staged/write/read error chain 内の `std::io::ErrorKind::PermissionDenied` を境界で分類する。

## Fault injection seam

Required evidence は test-only store seam で deterministic に注入する。直接 denial は denied write が効果を持つ前に拒否される admission-control fault である。partial effect は別途 `Indeterminate` injection が制御し、denial injectionと混ぜない。これにより Windows ACL、POSIX mode bit、root 実行、locking 実装差に Required verdict を依存させない。

## Save ordering and protection

session は capture 全件 → manifest 構築 → Strict quality gate → blob publish → manifest publish/HEAD update の順で進む。Strict は degraded/unsupported/failed を含む入力を store へ到達させない。Standard は quality receipt に残したうえで store denial へ到達する。store failure は quality failure に変更しない。

denial 後の旧 state 保護は、HEAD が旧 manifest を指し続けること、manifest set と manifest bytes が変わらないこと、既存 content-addressed blob bytes/SHA-256 が変わらないことで判定する。denied 後段の前に publish 済みだった blob は、HEAD から参照されない garbage になり得るが、旧 blob を上書きせず valid state とみなさない。空 blob、partial manifest、failure record を成功 state とみなさない。

## Reconcile / fail-closed

`Indeterminate` publish は成功とも確定失敗とも扱わない。blob publish では blob readback と digest 照合、manifest/HEAD publish では HEAD と manifest bytes の readbackで reconcile する。readback 自体が permission denied の場合は `PluginStateError::ReadbackFailed { publish_error, readback_error }` にし、2つの原因を独立に残す。`NotFound` は negative evidence なので readback failure と混ぜない。

restore は HEAD read denial と manifest read denial を session error、blob read denial を per-record `StoreFailed`、digest 検証後の plugin port refusal を `Failed` に分ける。blob read denial で plugin portへ空 bytes を渡さない。

## Deferred concerns

plugin-originated PermissionDenied capability は ruviform fixture/HostError 側の追加が必要である。実FS permission probeは supplemental evidence に限る。production host/routing への波及、RT callback からの FS IO、offline render recipe 組込みは本設計に含めない。
