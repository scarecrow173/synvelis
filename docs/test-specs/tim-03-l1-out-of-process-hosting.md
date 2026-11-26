---
id: "TSPEC-c7Hn4Qw9Lm3Xv6RtYsK2"
type: "test-spec"
status: "approved"
title: "tim-03-l1-out-of-process-hosting"
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
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-l1-out-of-process-hosting.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-l1-out-of-process-hosting.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-03-l1-out-of-process-hosting.md"
  verified-by: []
  references:
    - "docs/designs/tim-03-l1-out-of-process-hosting.md"
  defers: []
  deferred-by: []
---

# Test Spec — TIM-03 L1 out-of-process hosting production wiring

Feature: ruviform#630 `test_gain` VST3/CLAP を Synvelis production composition seam
経由の L1 out-of-process route で起動し、限定的な independent PCM oracle を実測する。

Fixture: `crates/synvelis-app/tests/tim03_l1_process_oracle.rs`、
`crates/synvelis-plugin-test-l1-runner`、mise task `test-tim03-l1`。

## Rules

- **L1-01 runner provisioning**: dedicated task は先に
  `synvelis-plugin-test-l1-runner` を build する。test は runner path を env override
  `SYNVELIS_TIM03_L1_VST3_RUNNER` / `SYNVELIS_TIM03_L1_CLAP_RUNNER` または target
  profile root から解決する。未解決は failure。
- **L1-02 artifact provisioning**: test は resolved fixture cdylib を runner の sibling
  un-hashed artifact として stage する。既存 artifact の hash mismatch は failure。
  runner argv は使わない（`ruviform-plugin-export` は argv を許さない）。
- **L1-03 route registration**: route は `ProductOutOfProcessPlugin` /
  `OutOfProcessSpec` で登録し、in-process `PluginFactory` へ偽装しない。
- **L1-04 scan/lifecycle**: direct host scan に `test_gain` descriptor が現れ、
  Instantiate → Initialize → Activate(48 kHz, 1..=512) → StartProcessing が成功する。
- **L1-05 bus report**: L1 `host.buses(handle)` は `BusReport::Unavailable`。
  これを unverified と記録し、`Reported([])` として扱わない。
- **L1-06 endpoint attach/detach**: `prepare_endpoint(512 frames, 2 channels)` が
  成功し、latency は `2 * 512 = 1024` frames。endpoint drop 後に Stop/Deactivate/
  Destroy が成功する。
- **L1-07 lookahead**: 最初の 2 process call は定義済み silence。以後、
  `passthrough_submits` delta が 0 の block のみ matched delivery とする。
- **L1-08 PCM membership**: normalized 0.25 → plain gain 0.5 を event で送り、
  deterministic input 履歴に対して `0.5 * input` の membership を bit-exact に確認
  する。24 call 中少なくとも 6 matched block を要求する。
- **L1-09 production seam**: `compose_routing_engine_with_out_of_process` 経由で
  project mutation から routing build を行い、receipt が `Active`、generation 1の
  state handle が存在する。
- **L1-10 production generation retirement**: 同じ production session で
  `RemoveProcessor` を commit し、replacement generation 2 が RT activation へ到達
  する。activation 後、retired generation 1 の `plugin_state_handles` は空でなければ
  ならない。これは generation retirement と lease teardown の独立観測であり、
  endpoint object detach receipt の代用証拠とはしない。
- **L1-11 evidence separation**: production activation/retirement evidence と direct endpoint
  process oracle を別 JSON object に記録する。L0 negative boundary test の結果を流用
  しない。
- **L1-12 honesty**: tempo/loop/automation sample accuracy、MIDI precision、state
  quality、crash/hang/fault isolation、GUI、Windows EXT-L3 は `NotRun`。Passと書かない。

## 正規実行

```sh
mise run test-tim03-l1
```

期待結果: VST3/CLAP 2 tests passed。stdout の `tim03-l1-process-oracle` JSON を
`evidence/sv127/l1-process-oracle.jsonl` の材料とする。
