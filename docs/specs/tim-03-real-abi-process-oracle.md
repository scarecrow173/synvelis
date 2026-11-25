---
id: "SPEC-dY1xgGhccy2D0xj8JhQC3"
type: "spec"
status: "approved"
title: "TIM-03 real ABI process oracle boundary slice"
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
  implemented-by:
    - "docs/designs/tim-03-real-abi-process-oracle.md"
  depends-on:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-03-real-abi-process-oracle.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 real ABI process oracle boundary slice

## What Should Be Built

`crates/synvelis-app/tests/tim03_real_abi_process_oracle.rs` に、ruviform
`20dc41f71c47674b3e51d1eb0921124b94f8bfaf` pin に含まれる ruviform#630
製の実 VST3/CLAP test plugin binary を使う self-contained consumer oracle
を追加する。本 slice が検証するのは、artifact resolution、ABI discovery、
`test_gain` の host lifecycle、および Synvelis production composition path が
real ABI instance を typed-runtime process endpoint へ attach できない境界で
typed fail-closed になることである。

PCM/Event/transport offset の process oracle そのものは本 slice の Pass 条件
ではない。upstream の runtime integration は L1 out-of-process endpoint path を
正規経路としており、in-process real plugin bridge instance は `!Send` で
`create_sendable` が `None` のため、in-process typed endpoint に attach できない
（ruviform pin 内 `crates/ruviform-plugin-test-vst3/tests/l1_real_test_plugin_e2e.rs:1318-1326`
と `crates/ruviform-plugin-core/src/plugin.rs:398-417`）。Synvelis はまだ L1
out-of-process hosting を production composition path に配線していない。したがって
本 slice はその architecture gap を実走 evidence として固定し、process oracle を
`NotRun` として記録する。

## Why

- Issue #127 の仮想優先の置換 DoD は、Fake Plugin の成功を本番 ABI Pass に
  昇格しないことを要求する。実 ABI binary がどこまで production path に乗り、
  どこで拒否されるかを実測しなければ、Fake/headless evidence と real ABI
  evidence の境界を誤認する。
- artifact discovery と lifecycle は in-process host surface でも成立する。
  一方、typed-runtime endpoint attach は `Send` capability を要求し、real ABI
  bridge instance は thread-affine state を持つため成立しない。この両者を
  分けて記録することが consumer oracle の責務である。
- production path の最初の defined rejection は endpoint failure だけではない。
  pinned ruviform revision の VST3/CLAP in-process adapter は `Plugin::buses`
  の default empty table を返し、composite host はこれを `Reported([])` として
  公開する。bridge `attach()` は consumer declaration（stereo input/output）と
  empty report を cross-check し、endpoint preparation へ進む前に bus mismatch
  で reject する。この ordering も実測どおりに記録しなければ、endpoint gap が
  production evidence から読めない。

## Acceptance Criteria (this slice)

1. test file は self-contained であり、共通 helper/module を新設しない。
   `SYNVELIS_TIM03_REAL_ABI_VST3` / `SYNVELIS_TIM03_REAL_ABI_CLAP` による
   explicit artifact override と、dev-dependency build output（hashed cdylib を含む）
   の解決を行う。対象 OS で artifact を解決できない場合は silent skip ではなく
   test failure とする。
2. resolved artifact path と SHA-256 を記録し、`Vst3Library::load` /
   `ClapLibrary::load` が成功する。descriptor scan は VST3 7 件、CLAP 6 件で、
   `test_gain` を含む。`test_gain` descriptor id は VST3
   `218a4f6d7c3b059ef1a24c68b9d37e51`、CLAP
   `org.ruviform-test.clap-test-gain` と一致する。VST3 descriptor id は raw
   component TUID の lowercase hex、CLAP descriptor id は factory instantiation id
   として evidence に記録する。
3. `test_gain` factory を `PluginHost` に登録し、Scan → Instantiate → Initialize →
   Activate（48 kHz, 1..=256 frames）→ StartProcessing まで成功する。同じ activated
   instance に対する `prepare_endpoint` は `HostError::EndpointUnavailable` で失敗し、
   factory の `create_sendable` capability が `None` であることと併せて直接の
   measured evidence とする。lifecycle teardown は StopProcessing →
   Deactivate → Destroy を best-effort で行う。
4. `PluginHostProcessors::product` + `ProcessorConnectRequest` による production
   processor-host connect は fail-closed であり、成功を claim しない。pinned revision
   では最初の defined rejection が `ProcessorConnectError::Incompatible`
   （empty bus report cross-check）であることを pin する。endpoint preparation 自体が
   `EndpointUnavailable` である事実は 3 の direct measurement と結び付けて記録する。
5. `compose_routing_engine` → `SessionRoutingEngine` の production composition path
   でも processor chain は active/published へ進まず、typed routing fault で fail-closed
   になる。pinned rejection mapping では `Incompatible` →
   `RoutingFault::UnsupportedFormatOrTimeMap` であり、これを `PluginUnavailable` に
   書き換えない。
6. process PCM oracle、sample-accurate automation、event/frame offset、loop/seek/tempo
   跨ぎ、illegal process length・誤配送・stale generation の real ABI 観測は
   `NotRun` と明記する。解除条件は「Synvelis production composition path に L1
   out-of-process plugin hosting が配線され、real ABI instance が typed endpoint に
   attach できるようになった後に再評価」である。
7. production source、`Cargo.toml`、dependency pin、既存 plugin dock/sv#128 領域は
   変更しない。docs index は編集しない。

## Out of Scope

- crash/hang/timeout isolation、L1 out-of-process oracle 本体、`test_fault`、
  `test_stateful`、State/Unload、editor/GUI/Native Dock、実 Window、OS matrix、
  Windows EXT-L3 の最終実走。
- `test_instrument` の MIDI/transport oracle。descriptor discovery に現れることは
  記録してよいが、process endpoint がないため駆動しない。
- PCM/Event/transport independent oracle、sample-accurate automation、loop/seek/tempo
  process 照合。これらは endpoint attach 後に再評価する。
- desktop product の実 plugin scan/registry owner、`compose_product_routing` の
  empty registry、L1 runner feature、ruviform 側の bus enumeration / `Send` bridge /
  out-of-process host 実装。
- 3OS CI、AudioLens f64/native PCM cross-consumer evidence、consumer 独立結果集約。
- Issue #127 の close。本 slice は `Refs #127` 相当の前進記録である。

## Evidence Classification

- Pass とよべるのは Windows local headless lane における artifact resolution、
  ABI discovery、lifecycle、fail-closed rejection の実測だけである。
- `mise run test synvelis-player` と `mise run test-rt` は既存 TIM-03 fake/headless
  wiring の非回帰証拠であり、real ABI process Pass に昇格しない。
- GUI-L2/EXT-L3、macOS/Linux、crash isolation、実 GUI、AudioLens cross-consumer は
  `NotRun`。未実 OS を Pass にしない。
