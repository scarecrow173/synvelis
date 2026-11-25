---
id: "TSPEC-G0yUY6l6HxgOBguhDUFTI"
type: "test-spec"
status: "approved"
title: "tim-03-real-abi-process-oracle"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-real-abi-process-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-real-abi-process-oracle.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-03-real-abi-process-oracle.md"
  verified-by: []
  references:
    - "docs/designs/tim-03-real-abi-process-oracle.md"
  defers: []
  deferred-by: []
---

# Test Spec — TIM-03 real ABI process oracle boundary slice

Feature: ruviform#630 製 real VST3/CLAP test plugin binary を artifact discovery
と lifecycle までは実走し、production typed-runtime process endpoint へ attach
できない境界で fail-closed になることを independent consumer evidence として固定
する。

Fixture: `crates/synvelis-app/tests/tim03_real_abi_process_oracle.rs`。test file
内で artifact resolution、fixture scan、host lifecycle、processor-host connect、
composition-root activation measurement を完結させる。共通 helper/module は作らない。

## Rules

- **RA-01 artifact resolution**: `SYNVELIS_TIM03_REAL_ABI_VST3` /
  `SYNVELIS_TIM03_REAL_ABI_CLAP` override、または dev-dependency build output から
  real fixture cdylib を解決する。未解決は skip/Unsupported ではなく failure。
  resolved path は file として存在し、SHA-256 は 64 hex chars で記録される。
- **RA-02 ABI discovery**: VST3 descriptor scan は 7 件、CLAP は 6 件。
  `test_gain` descriptor id は VST3 で Windows `218a4f6d7c3b059ef1a24c68b9d37e51` /
  非 Windows `6d4f8a213b7c9e05f1a24c68b9d37e51`（`vst3` crate `uid()` の
  プラットフォーム依存 byte order による正当な差異）、CLAP
  `org.ruviform-test.clap-test-gain` と一致する。scan 一覧には VST3/CLAP とも
  `test_instrument` が現れるが、本 slice では駆動しない。
- **RA-03 lifecycle**: `test_gain` factory は Scan → Instantiate → Initialize →
  Activate(48 kHz, 1..=256 frames) → StartProcessing まで成功する。
- **RA-04 endpoint boundary**: lifecycle に成功した同じ in-process instance で
  `prepare_endpoint(256 frames, 2 channels)` が `HostError::EndpointUnavailable`
  を返す。成功は architecture 変化または誤経路なので failure。
- **RA-05 bus-report ordering**: in-process host bus report は pinned revision で
  `Reported([])`。`PluginHostProcessors::product(...).connect(...)` は
  stereo input/output declaration との cross-check に失敗し、
  `ProcessorConnectError::Incompatible` を返す。`Unavailable` と期待しては
  いけない — endpoint error は RA-04 で直接測る。
- **RA-06 production composition fail-closed**: `compose_routing_engine` 経路で
  chain processor を持つ project を build すると、activation receipt は
  `Failed` または `Blocked` で、fault は deterministic mapping どおり
  `RoutingFault::UnsupportedFormatOrTimeMap`。Published/Active へ進んではいけない。
- **RA-07 evidence JSON**: 各 format が `tim03-real-abi-oracle` prefix 付き JSON を
  出力し、format、artifact、sha256、descriptor count/list、`test_gain` id、
  factory class id evidence、scan/lifecycle verdict、bus report、
  `create_sendable` capability、endpoint error、adapter connect error、production
  activation fault、process oracle NotRun rationale を含む。
- **RA-08 isolation**: session media root は process id + atomic sequence で一意。
  lifecycle teardown は StopProcessing/Deactivate/Destroy を best-effort で行う。

## Test matrix

| Case | Format | Coverage | Expected verdict |
| --- | --- | --- | --- |
| `tim03_real_abi_vst3_discovery_lifecycle_and_fail_closed_boundary` | VST3 | RA-01..RA-08 | Pass（process oracle 以外） |
| `tim03_real_abi_clap_discovery_lifecycle_and_fail_closed_boundary` | CLAP | RA-01..RA-08 | Pass（process oracle 以外） |

## Examples

- VST3 `test_gain`: descriptor/factory class id は Windows
  `218a4f6d7c3b059ef1a24c68b9d37e51` / 非 Windows `6d4f8a213b7c9e05f1a24c68b9d37e51`
  （raw component TUID lowercase hex。byte order はプラットフォーム依存）。
- CLAP `test_gain`: descriptor/factory instantiation id は
  `org.ruviform-test.clap-test-gain`。
- endpoint measurement: `prepare_endpoint` → `EndpointUnavailable`。
- adapter measurement: `connect` → `Incompatible`。
- production activation measurement: `UnsupportedFormatOrTimeMap` で fail-closed。

## NotRun

次は本 slice では実行せず、evidence でも Pass にしない。解除条件は「L1
out-of-process plugin hosting が Synvelis production composition path に配線され、
real ABI instance が typed endpoint へ attach できるようになった後に再評価」である。

- deterministic PCM/crc oracle、normalized/plain mapping、sample-accurate parameter
  automation、same-offset last-wins、zero-frame flush、frame offset exactness。
- loop/seek/tempo revision 跨ぎの real ABI process boundary 照合。
- real ABI 経路での illegal process length・誤配送・stale generation の観測。
- `test_instrument` MIDI/transport click oracle。
- `test_fault`/`test_stateful`/State/Unload、crash/hang/timeout isolation、
  editor/GUI/Native Dock、実 Window、OS matrix、Windows EXT-L3、3OS CI、
  AudioLens f64/native PCM cross-consumer oracle。
