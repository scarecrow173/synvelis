---
id: "TSPEC-j6Hm3Qw8Ln4Xv7RtYsK9"
type: "test-spec"
status: "approved"
title: "tim-03-transport-application"
created: "2026-11-27"
updated: "2026-11-28"
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
    - "docs/designs/tim-03-transport-application.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-03-transport-application.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-03-transport-application.md"
  verified-by: []
  references:
    - "docs/designs/tim-03-transport-application.md"
  defers: []
  deferred-by: []
---

# Test Spec — TIM-03 transport application minimal vertical

Fixture: `crates/synvelis-app/tests/tim03_l1_transport_application.rs`、
`crates/synvelis-app/tests/common/tim03_l1.rs`、mise task `test-tim03-transport`。

## Rules

- **TA-01 production wiring**: VST3/CLAP `test_gain` は
  `compose_routing_engine_with_out_of_process_and_boundaries` と L1 runner 経由で起動する。
  fake plugin や in-process 偽装で Pass にしない。
- **TA-02 conversion once**: loop start/end、seek、automation instant は project beat で入力し、
  seam の conversion evidence が tempo map 由来の media frame と一致する。caller-side
  二重変換や seconds 由来の推測を証拠に使わない。
- **TA-03 boundary oracle**: transport command は `boundary_stage_window` 経由で staged order
  に記録され、second window の loop wrap `BoundaryPoint` は `boundary_loop_wraps` 由来の
  offset 128 を示す。policy は Exact。
- **TA-04 control ledger projection**: automation write は `Dispatched` stage を持ち、
  `call_start + frame_offset` が transport anchor で補正した tempo-converted physical frame
  544 を再構成する。position は media frame 545、anchor は -1。
- **TA-05 span oracle**: seek/loop 後の `last_observed_span` は Playing、position 513、
  loop `[385,641)`、occurrence >= 1、transport generation >= 1 を示す。
- **TA-06 PCM oracle**: input `(+0.25,-0.25)` に対し automation normalized 0.375 は
  plain gain 0.75 として `input*0.75` block を lookahead 後に観測する。silence、
  lookahead/pass-through unity、transition `other` block を fresh automation delivery に数えない。
  normalized-as-plain 相当の `input*0.25` block は 0 件。
- **TA-07 negative evidence**: published だが inactive な generation への transport application は
  typed `InactiveGeneration { surface, active }` で失敗する。
- **TA-08 determinism/ordering**: runner build → integration test は dedicated opt-in lane
  `mise run test-tim03-transport` に束ね、test は `--ignored` と `--test-threads=1` で実行する。
  この artifact-dependent test は generic workspace-wide / MSRV / RT lane の対象外とする。

## Evidence

両 format の JSONL record に artifact path/hash、runner path/hash、ruviform revision、
tempo map、conversion、command、loop wrap/span、ledger projection、PCM classification、
negative evidence、NotRun を記録する。
