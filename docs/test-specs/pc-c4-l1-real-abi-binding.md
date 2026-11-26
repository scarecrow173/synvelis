---
id: "TSPEC-pcC4L1RealAbi01"
type: "test-spec"
status: "approved"
title: "PC-C4 L1 real ABI binding test spec"
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
  implements: []
  implemented-by:
    - "docs/impl/pc-c4-l1-real-abi-binding.md"
  depends-on:
    - "docs/specs/pc-c4-l1-real-abi-binding.md"
    - "docs/designs/pc-c4-l1-real-abi-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/pc-c4-l1-real-abi-binding.md"
  verified-by: []
  references:
    - "evidence/sv101/pc-c4-l1-real-abi-binding.jsonl"
  defers: []
  deferred-by: []
---

# PC-C4 L1 real ABI binding test spec

## Cases

- `pc_c4_l1_vst3_real_abi_binding`
- `pc_c4_l1_clap_real_abi_binding`

両者とも同一 helper を `--test-threads=1` で実行する。artifact 不在や runner 不在は
skip ではなく fail-closed であり、missing runner message は `mise run test-pc-c4-l1`
を示す。

## Inputs

- 48kHz、256 frames/block、stereo input L=0.25/R=-0.25、9600 frames の第 1 clip。
- target capability: param id 0、range [0,2]、default 1、automatable。
- fast write: normalized 0.25、media frame 900（in-block offset 132）。
- automation: normalized 0.375、media frame 4900（in-block offset 36）、1 target/1 value。
- playback は 20ms/block pacing の 2×16 block。

## Expectations / fail conditions

Pass 条件:

1. activation `Active`、binding report `bound_targets=1`、unbound/without_control empty。
2. binding が stable instance id、processor node、authored plugin ref、graph/capability
   generation、verified frame base を持つ。foreign base は reject。
3. fast/automation observation が `Dispatched`、media position と
   `frame_offset = position % 256`、receipt 由来 transport anchor 込み physical
   projection に一致。
4. PCM block classification で `input*0.25` が 0 件、最初の `input*1.0` が最初の
   `input*0.5` より前。
5. replacement graph generation で新 binding、旧 generation handles empty、stale
   observe `None`。

Fail 条件: 上記不一致、missing artifact/process start failure、normalized==plain を
仮定した実装が作る `input*0.25` の安定出力、fake/in-process route、unsupported format
を Pass へ混入すること。

NotRun とするもの: sample-exact PCM frame offset、plugin output report、RT+Offline
parity、typed reject matrix、tempo/loop matrix、GUI/EXT-L3、fault/crash/hang、
その他 fixture。
