---
id: "TSPEC-Tim04PermissionMatrix01"
type: "test-spec"
status: "approved"
title: "TIM-04 permission fault matrix test spec"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
---

# TIM-04 permission fault matrix test spec

## Purpose

Permission failure を plugin-originated state hook failure と host/store OS/sandbox denial に分離し、host/store-side matrix を deterministic fault injection で固定する。

## Rules

- R1: save denial は `BlobPublish`、`ManifestPublish`、`HeadUpdate` を独立 case とする。verdict は typed `PermissionDenied`、operation、effect、旧 HEAD、manifest set、manifest bytes、既存 blob bytes/SHA-256 の before/after で判定する。
- R2: read/restore denial は `HeadRead`、`ManifestRead`、`BlobRead` を独立 case とする。HEAD/manifest read denial は session error、blob read denial は per-record `StoreFailed` であり、黙った成功や空 state 適用は Fail。
- R3: plugin port restore refusal は `Failed`、store blob read denial は `StoreFailed` とする。同じ failure kind に潰れた場合は Fail。
- R4: exact-eligible capture では Strict/Standard の両方が store denial へ到達する。degraded/unsupported/failed を含む入力では Strict が store 未接触で `QualityRejected`、Standard が quality receipt を保持する経路で store denial へ到達する。
- R5: indeterminate blob publish + blob readback denial、indeterminate HEAD update + HEAD/manifest readback denial は `ReadbackFailed` でなければならない。成功または確定失敗への格下げは Fail。
- R6: real ABI matrix は VST3/CLAP `test_stateful` から capture した actual RFST bytes を使う。evidence に len、SHA-256、RFST gain/checksum、format、artifact/runner SHA-256、ruviform resolved rev、store backend、fault point、policy、capture quality を記録する。
- R7: plugin-originated PermissionDenied capability が fixture/HostError にない場合は verdict `Unsupported`。mock の PermissionDenied を real ABI Pass に転用しない。
- R8: 実 FS permission/locking probe は supplemental evidence に限り、Required Pass/Fail を決めない。

## Evidence schema

`evidence/sv128/tim04_l1_abi_state.jsonl` は JSONL とし、各行は `case`、`driver`、`verdict`、`detail`、`observed`、`environment` を持つ。`environment` は OS/arch、`environment_kind: test-plugin`、`isolation: out-of-process/l1`、format、fixture artifact path/sha256、runner path/sha256、ruviform resolved rev、descriptor、store backend、fault point、policy、capture quality を持つ。denial row は typed operation/effect、HEAD/manifest/blob before-after、reconcile result、capability verdict を `observed` に持つ。

## Cases

project unit tests は mock `MatrixStore` で R1–R5 を網羅する。L1 real ABI test は actual RFST bytes を入力に R1/R2/R4/R5/R6/R7 を記録する。全状態操作は non-RT control path に限定する。

## 実行契約

L1 real ABI matrix を実行する `tim04_l1_abi_state.rs` の `l1_permission_fault_matrix_real_rfst` は、test-only runner と VST3/CLAP fixture cdylib を必要とする artifact-dependent test である。generic workspace-wide / MSRV lane の対象外とし、正規実行は dedicated opt-in lane `mise run test-tim04-l1` とする。coverage は削除せず、同 laneが `--ignored` で実行する。
