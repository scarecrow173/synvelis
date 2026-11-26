---
id: "TASK-pcC4L1RealAbi01"
type: "task"
status: "approved"
title: "PC-C4 L1 real ABI binding tasks"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-c4-l1-real-abi-binding.md"
  implemented-by:
    - "docs/impl/pc-c4-l1-real-abi-binding.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c4-l1-real-abi-binding.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv101/pc-c4-l1-real-abi-binding.jsonl"
  defers: []
  deferred-by: []
---

# PC-C4 L1 real ABI binding tasks

## 完了

- [x] `crates/synvelis-app/tests/pc_c4_l1_real_abi_binding.rs` を追加。
- [x] VST3/CLAP `test_gain` artifact resolution/staging/provenance を実装。
- [x] production composition seam での activation/binding/frame-base verification を実装。
- [x] fast control/automation control ledger oracle を実装。
- [x] normalized/plain PCM oracle と L1 lookahead/pass-through caveat を実装。
- [x] generation replacement/retirement/stale-generation fail-closed を実装。
- [x] `mise.toml` に `test-pc-c4-l1` を追加。
- [x] `evidence/sv101/pc-c4-l1-real-abi-binding.jsonl` を追加。
- [x] 最新 main `b59333d4` を fast-forward で取り込み（docs/tools のみの upstream 差分）。

## 完了処理

- [ ] 全必須 verification lane を記録する。
- [ ] commit、push、PR 作成。

## 後続 slice / gate

- [ ] PLUG-C4-01 Frame/Value Receipt RT+Offline parity。
- [ ] PLUG-C4-02 typed reject matrix。
- [ ] PLUG-C4-03 複合制御、PLUG-C4-04 全 generation matrix、PLUG-C4-05 cross-consumer。
- [ ] GUI/Native Dock/EXT-L3、fault/crash/hang、他 fixture。
- [ ] docs index 整備（Coordinator 統合時）。
