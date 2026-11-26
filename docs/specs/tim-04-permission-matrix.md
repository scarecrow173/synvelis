---
id: "SPEC-Tim04PermissionMatrix01"
type: "spec"
status: "approved"
title: "TIM-04 plugin state permission fault matrix"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
---

# TIM-04 plugin state permission fault matrix

## What Should Be Built

Issue #128 の TIM-04 sliceとして、Plugin State Snapshot の Permission failure を次の2類に分離して記録する。

1. **plugin-originated state hook failure**: plugin または ABI state hook が capture/restore を拒否する。
2. **host/store OS/sandbox denial**: Synvelis host/project store が blob、manifest、HEAD、readback、restore 入力の読取りで OS/sandbox denial を受ける。

両者を同じ Pass/Fail evidence に混ぜない。host/store-side denial は `PluginStateStoreError::PermissionDenied { operation, effect, detail }` と `PluginStateStoreOperation` で構造的に識別し、表示文字列の grep を oracle にしない。plugin-port 側の拒否は `PluginStateRestoreOutcome::Failed`、store 側の record 読取り拒否は `StoreFailed` に分離する。

## Matrix scope

保存経路は blob publish、manifest publish、HEAD update の denial を独立 case とする。再読込経路は HEAD read、manifest read、blob read、適用経路は plugin port restore denial を区別する。Strict/Standard の差、旧 HEAD/manifest/blob 保護、indeterminate publish と permission-denied readback の fail-closed を含める。

保存入力は mock bytes だけでなく、real ABI L1 `test_stateful` から capture した actual RFST bytes を使う。VST3/CLAP それぞれで length、SHA-256、RFST parse 結果を evidence に残し、同じ bytes を fault-injecting store matrix へ入力する。

## Out of scope / non-claims

- 実ファイルシステム ACL/locking probe を Required Pass にしない。OS差や root 権限で不安定なため、deterministic store seam を OS-L1 contract evidence とする。
- plugin-originated PermissionDenied を mock で成功扱いしない。現行 ruviform pin `87a7ba8ff3fbe6d4502bea27d152d7f74c105899` の fixture fault mode と `HostError` に再現 capability がなければ verdict は `Unsupported`。
- Offline render、typed runtime process oracle、registry/discovery、GUI/Native Editor、AU/市販 plugin、AudioLens cross-gate、3OS final acceptance は本 slice の完了を主張しない。

## Acceptance mapping

本 spec は Issue #128 の「Plugin Crash/Timeout/Permission/Blob Corruption を検証」「インデターミネートPublishはHEAD/BlobのreadbackでReconcile」の Permission 部分を前進させる。`TIM04-RCP-01` end-to-end と Windows 最終受入は残件である。
