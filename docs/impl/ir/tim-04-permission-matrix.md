---
id: "IMPL-Tim04PermissionMatrix01"
type: "impl"
status: "approved"
title: "TIM-04 permission fault matrix implementation record"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
---

# TIM-04 permission fault matrix implementation record

## 実装概要

`synvelis-project` の plugin-state store contract を operation-aware にし、permission denial を構造化した。`PluginStateStore::save_manifest` は immutable manifest publish と HEAD repoint を合成するが、failure operation は `ManifestPublish` と `HeadUpdate` を区別する。`DirectoryPluginStateStore` は ruviform staged publish/write/read の error chain から `std::io::ErrorKind::PermissionDenied` を境界で分類し、呼び出し側の文字列 grep を排除した。

session は publish effect が indeterminate で readback も失敗した場合を `ReadbackFailed` として表す。blob `NotFound` は negative evidence として扱い、readback denialと混同しない。restore は store read denialを `StoreFailed`、digest 検証後の plugin-port refusalを `Failed` に分ける。

## Test / evidence

`crates/synvelis-project/tests/plugin_state.rs` は deterministic `MatrixStore` で publish/read/restore denial、Strict/Standard、旧 HEAD/manifest/blob 保護、indeterminate+denied readback を検証する。`crates/synvelis-player/tests/tim04_l1_abi_state.rs` は real ABI `test_stateful` から capture した actual RFST bytes を VST3/CLAP matrix へ入力し、JSONL evidence を出力する。

plugin-originated PermissionDenied は、現行 ruviform pinの `test_fault` mode (`none/abort/exit/hang/error/rt_alloc/bad_time/invalid`) に permission band がなく、`HostError` に PermissionDenied variant がなく、`test_fault` が state capabilityを持たないため `Unsupported` とした。mock の PermissionDenied は type-boundary testにのみ使い、real ABI Passへ転用していない。

## 残件 / non-claims

Offline render、typed runtime process oracle、製品 registry/discovery、GUI/Native Editor、AU/市販 plugin、AudioLens、3OS final acceptance は未実施である。実FS permission/locking probeは Required evidenceにしていない。docs index は Coordinator が統合時に整備する。
