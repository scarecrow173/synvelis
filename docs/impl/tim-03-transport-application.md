---
id: "IMPL-x4Qw7Lm2Vn9Kr5TyEbP8"
type: "impl"
status: "approved"
title: "TIM-03 transport application minimal vertical"
created: "2026-11-27"
updated: "2026-11-27"
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
  implements:
    - "docs/plans/tim-03-transport-application.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-transport-application.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv127/transport-application.jsonl"
    - "evidence/sv127/transport-application-verification.md"
  defers: []
  deferred-by: []
---

# TIM-03 transport application minimal vertical — Implementation Record

## 実装内容

- `crates/synvelis-app/src/engine.rs`
  - `TransportApplicationCommand`、`TransportConversionEvidence`、
    `TransportApplicationReport`、`TransportApplicationError` を追加。
  - `SessionRoutingEngine::control_apply_transport` を追加。tempo map compile、
    verified frame base conversion、node-cursor upcoming window derivation、
    loop-wrap derivation、boundary staging、automation tick supply を一つの seam に集約した。
- `crates/synvelis-app/src/composition.rs`
  - `compose_routing_engine_with_out_of_process_and_boundaries` を追加。
  - processor-keyed `BoundaryDeclaration` を `PluginHostProcessors` へ登録する。
  - 既存 `compose_routing_engine_with_out_of_process` は空 table で委譲する。
- `crates/synvelis-app/tests/common/tim03_l1.rs`
  - fixture/runner resolution、staging、paced run、ledger query、PCM classification などの
    shared helper を追加。
- `crates/synvelis-app/tests/tim03_l1_transport_application.rs`
  - VST3/CLAP `test_gain` deterministic scenario を追加。
- `mise.toml`
  - `test-tim03-transport` を追加。runner build と dedicated test を束ねる。
- `evidence/sv127/transport-application.jsonl`
  - format ごとの machine-readable evidence を追加。

## deterministic scenario

tempo map は初期 120 BPM、tick 1 から 125 BPM。loop `[1/60,1/36)` beat は media
`[385,641)` frame、seek `1/45` beat は 513、automation `17/720` beat は 545 に変換される。
seek apply head は frame 512 なので transport anchor は -1、automation physical frame は
544、ledger projection は `call_start 512 + frame_offset 32 = 544` を再構成する。
second window `[768,1024)` では position 513 が loop end 641 まで 128 frame で、
loop wrap boundary offset 128 を staging oracle へ供給する。

## 観測結果の要点

- VST3/CLAP とも production activation generation 1、policy Exact。
- span は Playing、position 513、loop `[385,641)`、occurrence 1、transport generation 4。
- PCM blocks 4..11 は `input*0.75`。blocks 0..1 は silence、block 2 は unity、
  block 3 は transition/other であり、これらを automation delivery に数えない。
- inactive generation negative は `InactiveGeneration { surface: 2, active: 1 }`。

詳細な command/hash/classification は JSONL を正本とする。
