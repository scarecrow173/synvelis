---
id: "SPEC-x8Qn3Lm7Vw2Jt9AzPqR4"
type: "spec"
status: "approved"
title: "TIM-03 L1 out-of-process hosting production wiring"
created: "2026-11-26"
updated: "2026-11-26"
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
    - "docs/designs/tim-03-l1-out-of-process-hosting.md"
  depends-on:
    - "docs/specs/tim-03-real-abi-process-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-03-l1-out-of-process-hosting.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 L1 out-of-process hosting production wiring

## 目的

Synvelis production composition と `PluginHostProcessors` に、in-process
`Box<dyn PluginFactory>` 登録と混同しない typed な L1 out-of-process route 登録
seam を設ける。ruviform pin `20dc41f71c47674b3e51d1eb0921124b94f8bfaf` の
`PluginHost::register_out_of_process(OutOfProcessSpec)` を用い、rf#630
`test_gain` VST3/CLAP を production seam 経由で Scan → Instantiate → Initialize →
Activate → StartProcessing → attach まで通し、限定的な L1 process oracle を初回
再評価する。

## スコープ

- `compose_routing_engine_with_out_of_process` と
  `PluginHostProcessors::product_with_out_of_process` を追加する。
- Synvelis 所有の test-only runner crate `synvelis-plugin-test-l1-runner` と
  dedicated mise task `test-tim03-l1` を追加する。
- `crates/synvelis-app/tests/tim03_l1_process_oracle.rs` で VST3/CLAP の
  production activation と direct endpoint process oracle を実測する。
- L1 の `BusReport::Unavailable` は unverified と記録し、in-process の
  `Reported([])` と同一視しない。
- lookahead、substituted output、pass-through output を precision exact と誤認
  しない。

## 非スコープ

- tempo/loop/automation sample accuracy の全 matrix。
- `test_instrument`、`test_fault`、`test_stateful` の oracle。
- crash/hang/timeout/fault isolation、RT/Crash の最終判定。
- GUI/Native Dock、実 Window、Windows EXT-L3 最終受入。
- ruviform pin 更新、ruviform/audio-lens 側変更、Issue #127 close。

## Acceptance Criteria（この slice）

1. production composition seam が `OutOfProcessSpec` の descriptor、program、args、
   isolation、lookahead、timeout を upstream API の意味のまま host 登録へ渡す。
2. VST3/CLAP `test_gain` が production seam 経由で routing activation `Active` に
   到達し、activation generation に対応する state handle が観測できる。同じ
   production session の `RemoveProcessor` rebuild で replacement generation が
   RT activation へ到達し、retired generation の state handle 空化によって
   generation retirement を観測できる。
3. direct L1 endpoint attach が成功し、endpoint drop 後に Stop/Deactivate/Destroy が
   完了する。production endpoint object detach 自体は upstream API に detach receipt
   がない ownership `Drop` なので、same-instance re-attach oracle とはしない。
4. deterministic input に対し、少なくとも defined-output membership を複数 matched
   block で観測する。substituted/pass-through block は delivery evidence に数えない。
5. process oracle は「初回再評価済み・限定的」と記録し、精度 matrix、crash/hang
   isolation、state quality を Pass と書かない。
6. artifact path/hash、descriptor、runner artifact 解決方法、host route、lifecycle
   到達点、独立判定材料、未達項目を `evidence/sv127/` に記録する。
