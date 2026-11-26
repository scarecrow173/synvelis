---
id: "SPEC-pcC4L1RealAbiBind01"
type: "spec"
status: "approved"
title: "PC-C4 L1 real ABI binding — test_gain production evidence"
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
    - "docs/designs/pc-c4-l1-real-abi-binding.md"
  depends-on:
    - "docs/specs/tim-03-l1-out-of-process-hosting.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-03-l1-out-of-process-hosting.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c4-l1-real-abi-binding.md"
  references:
    - "evidence/sv101/pc-c4-l1-real-abi-binding.jsonl"
  defers: []
  deferred-by: []
---

# PC-C4 L1 real ABI binding — test_gain production evidence

## 目的

rf#630 `test_gain` の VST3/CLAP 実 ABI を、fake plugin ではなく
`compose_routing_engine_with_out_of_process` による production composition から
L1 out-of-process plugin として起動し、PC-C4 parameter binding/control の最小限の
実 ABI evidence を headless に作る。

## Scope

- deterministic 48kHz stereo fixture project、stereo clip、track/strip、processor
  plugin、1 つの `ProcessorParam` target。
- caller-vended capability は `ParamId(0)`、name `Gain`、plain range `[0,2]`、
  default `1`、automatable。
- routing activation `Active`、binding report `bound`、stable plugin instance id、
  processor node、authored `PluginRef`、graph/capability generation、verified
  `AttachClockContext.frame_base`。
- fast control normalized `0.25` と 1 点 automation normalized `0.375` の control
  ledger observation。fast write は explicit in-block frame offset を持つ。
- normalized `0.25` が plain gain `0.5` に解決され、PCM が `input*1.0` から
  `input*0.5` へ遷移する非 identity mapping oracle。
- clip 追加による graph generation replacement、旧 generation handle retirement、
  stale-generation observe fail-closed。
- artifact path/size/SHA-256、runner provenance、ruviform resolved revision。

## Out of Scope

- tempo map、loop、pause/resume、multi-node、全 automation interpolation matrix。
- GUI/Native Dock/EXT-L3、crash/hang/fault isolation、`test_stateful`/`test_fault`/
  `test_instrument`。
- plugin output report、Frame/Value Receipt RT+Offline parity、typed reject matrix 全量。
- endpoint detach receipt や same-instance re-attach の証明。

この slice は Issue #101 の進捗 evidence であり Issue close ではない。PLUG-C4-01/02
の real ABI 全 matrix、PLUG-C4-03 複合制御、PLUG-C4-04 の loop/seek/GUI を含む全
generation matrix、PLUG-C4-05 cross-consumer/AudioLens/f64/upstream 互換検証は残件。

## Acceptance Criteria（この slice）

1. 両 format で production wiring が `Active` に達し、target が `bound`。
2. binding pin と frame base 48000/1 が source map/control frame base/capability
   generation と一致し、foreign frame base は reject。
3. fast/automation write が `Dispatched` observation と frame offset projection を持つ。
4. `input*0.25`（normalized を plain と誤解した出力）が現れず、`input*1.0` の後に
   `input*0.5` を観測する。
5. replacement generation で新 binding pin、retired generation handles empty、stale
   observe fail-closed。
6. evidence JSONL と verification note を `evidence/sv101/` に残す。
