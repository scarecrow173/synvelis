---
id: "SPEC-034oAncdBMNxHcIjF4PLx6"
type: "spec"
status: "approved"
title: "TIM-04 Plugin State Snapshot — real ABI consumer-run"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/128"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/tim-04-real-abi-state.md"
  depends-on:
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "https://github.com/scarecrow173/ruviform/issues/630"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/128"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-04-real-abi-state.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/630"
  defers: []
  deferred-by: []
---

# TIM-04 Plugin State Snapshot — real ABI consumer-run

## What Should Be Built

Issue #128 のうち、headless/mock slice に残っていた **実 ABI consumer-run**
を実装する。ruviform#630 の正規 ABI Test Plugin 実バイナリを VST3/CLAP
の両形式でロードし、mock/fake plugin を使わずに `PluginHost` 上で
capture/save/restore/state mutation を検証する。本 slice は Issue #128
全体の一部であり、Offline render 実組込み・L1 crash/timeout isolation・
製品 registry/discovery・AudioLens cross-gate は完了を主張しない。

- **dev-dependency edge と artifact 解決**: `synvelis-player` の
  dev-dependency として `ruviform-plugin-vst3`/`ruviform-plugin-clap`
  （`host` feature）と `ruviform-plugin-test-vst3`/
  `ruviform-plugin-test-clap` を追加する。test 側は cargo が生成する
  cdylib artifact を explicit path override → un-hashed artifact →
  profile-root artifact → hashed dep-artifact glob の順で fail-closed
  に解決し、path・SHA-256・plugin identity を記録する。artifact が
  見つからない場合は skip せず失敗または理由付き `NotRun` とする。
- **production host cell 上の consumer-run**:
  `Vst3Library::load`/`ClapLibrary::load` → `factories()` →
  `PluginHostProcessors::product` → 同じ shared `PluginHost` cell で
  `Scan`/`Instantiate`/`Initialize`/`Activate`/`StartProcessing`/
  `StopProcessing` を実行する。`PluginStateHost::bind` には同じ host
  cell が返した handle を使い、`PluginStateSession` と
  `DirectoryPluginStateStore` で save/restore する。private API や
  test-only backdoor は使わない。
- **typed runtime capability の実測**: production `ProcessorConnection`/
  typed graph が要求する endpoint は `PluginHost::prepare_endpoint` で
  測定する。現行 ruviform rev では VST3/CLAP host factory が
  `create_sendable() = None`（thread-affine object）なので、in-process
  real ABI instance の endpoint attach は `HostError::EndpointUnavailable`
  となる。この capability gap は `Unsupported` として evidence に残し、
  PCM oracle は Coordinator 決裁どおり同じ production `PluginHost` cell の
  非 RT `process` 経路で駆動する。typed runtime 経路が通ったかのように
  偽らない。
- **capability matrix**: VST3/CLAP × `test_stateful`/`test_fault`、および
  VST3 `test_stateful_single` について、Processing/Stopped ×
  `CaptureMode::{Coordinated,Quiescent,BestEffort}` を実測する。
  Processing 中は `BestEffort`、stable stop boundary では mode に応じて
  `Coordinated`/`Quiescent`、stateless `test_fault` は `Unsupported`
  を記録する。`BestEffort` から BitExact/exact を主張しない。
- **RFST v2 独立 parser と state hash**: test 内の独立 parser が
  magic・version・length・FNV-1a-32 checksum・plain/normalized gain を
  検査する。対象は VST3 distributed `test_stateful` の framed blob
  `[u32 comp_len][component RFST][u32 ctrl_len][controller RFST]`、
  VST3 `test_stateful_single` の empty-controller frame、CLAP
  `test_stateful` の raw 20-byte stream。blob SHA-256 を receipt の
  `blob_digest` と照合する。
- **deterministic PCM oracle**: `test_stateful` の DSP law
  `out = in * gain` に対し、固定入力 left `0.5`・right `-0.25` と
  normalized `0.375`（plain `0.75`）の parameter event を使い、
  mutation 前後の PCM を測定する。save → mutate → restore initial blob
  → save → process で、blob bytes/hash と PCM が初期状態へ戻ることを
  VST3/CLAP それぞれで確認する。
- **integrity failure**: real ABI capture 由来 blob を
  `DirectoryPluginStateStore` に保存した後、1 byte 改竄と削除を行い、
  restore がそれぞれ typed `Corrupt`/`Missing` となり、store の digest
  precheck 前に plugin へ黙って restore しないことを確認する。
- **docs/evidence**: `evidence/sv128/` に case ごとの machine-readable
  JSONL と日本語 summary を残し、command・exit code・verdict 集計・
  capability matrix・non-claims・残件を記録する。

## Why Now

既存 TIM-04 slice は quality model・content-addressed store・reconcile・
restore・Offline Recipe gate を実装済みだが、consumer 証拠は mock plugin
だった。POLICIES.md の EXT-L3 仮想優先方針では、正規 ABI を実装した実
バイナリ Test Plugin を host/process 結合検証の oracle に使える。本 slice
はその実バイナリで state snapshot の consumer-run を測定し、mock の成功を
実 ABI の成功へ無断で昇格しない状態を解消する。

## Acceptance Criteria

- `crates/synvelis-player/tests/tim04_real_abi_state.rs` が mock/fake
  plugin を使わず、VST3/CLAP の `test_stateful`・`test_fault` と VST3
  `test_stateful_single` を解決・ロード・実行する。
- artifact 解決が silent skip せず、artifact path/hash と scan された
  plugin id/name/vendor/version を evidence に記録する。
- capability matrix が `Coordinated`/`Quiescent`/`BestEffort`/
  `Unsupported` を別状態として記録する。
- RFST v2 独立 parser・blob digest 照合・save→mutate→restore→save・
  deterministic PCM restore が VST3 distributed/single・CLAP で Pass と
  なるか、差異があれば Fail/Unsupported として記録される。
- corrupt/missing blob restore が typed failure になる。
- `crates/synvelis-player/src/` など非占有領域を変更しない。
- evidence と PR が本 slice の Pass と残件を区別する。

## Out of Scope

- `test_fault` の `abort`/`exit`/`hang` fault band。これらは in-process
  test runner を殺す L1-only fault であり、本 slice では arm しない。
  `test_fault` は state capability の `Unsupported` 測定対象に限定する。
- L1 out-of-process crash/hang/timeout isolation・専用 runner・process
  supervisor。
- Offline render への実 plugin 組込み、`synvelis-render` 変更、VCS
  snapshot への manifest 統合。
- 製品 composition の VST3/CLAP library discovery/load registry 配線。
- GUI/Native Editor/実 Window、AU、市販 plugin、物理機材、AudioLens
  cross-gate、3OS 横断。
- ruviform pin bump、ruviform 側修正、Synvelis `src/` adapter/engine/
  project/render の変更。
- docs index（`docs/**/README.md`）の編集・再生成。

## Non-claims

- 本 slice の Pass は Windows x86_64・in-process/L0・ruviform pinned rev
  `20dc41f71c47674b3e51d1eb0921124b94f8bfaf` の Test Plugin に対する
  consumer-run であり、市販 plugin や全 OS/全 host mode の受入ではない。
- typed runtime/PCM endpoint は現行 rev で `Unsupported` と実測された。
  本 slice の PCM oracle は production `PluginHost::process` によるもので、
  typed graph attach の成功を意味しない。
- `BestEffort` capture を exact/BitExact とみなさない。Strict save に使える
  のは `Coordinated`/`Quiescent` capture のみという既存 quality policy を
  維持する。
- real ABI fixture の deterministic law に対する oracle であり、外部 asset・
  非決定 state・市販 plugin の bit-exact 再現を保証しない。
