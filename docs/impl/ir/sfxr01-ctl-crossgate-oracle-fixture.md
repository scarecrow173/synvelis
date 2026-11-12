---
id: "IMPL-xRTJTdZHW0ii0jrP4pCFkK"
type: "impl"
status: "completed"
title: "SF-XR-01 CTL-01/CTL-02 cross-gate oracle fixture — implementation record"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/sf_xr_01.rs"
    modified:
      - type: "file"
        path: "evidence/reports/sf-06-cross-gate-ledger.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/tasks/sfxr01-ctl-crossgate-oracle-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sfxr01-ctl-crossgate-oracle-fixture.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sfxr01-ctl-crossgate-oracle-fixture.md"
  verified-by: []
  references:
    - "docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/designs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/plans/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/tasks/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "docs/test-specs/sfxr01-ctl-crossgate-oracle-fixture.md"
    - "evidence/reports/sf-06-cross-gate-ledger.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-XR-01 CTL-01/CTL-02 cross-gate oracle fixture — implementation record

## What was built

`crates/synvelis-player/tests/sf_xr_01.rs`（commit `43511f0`）—
SF-XR-01 の headless 検証可能部分を 2 test で実測。

- `sfxr01_01_strip_send_authority_survives_serialization_undo_and_lowering`:
  persisted strip/send 不変条件 → `InMemoryStore` save/load →
  undo/redo → `lower_semantic` の `RoutingSourceMap`/`GraphSpec`
  照合 → mute prune、を 1 committed project で一気通貫。
- `sfxr01_02_create_send_channel_map_consent_and_atomic_rejection`:
  `CreateSend` の consent matrix（`routing_preflight`）と commit
  経路の atomicity（revision/project bytes/undo stack 不変）。

`src/` 変更なし。`synvelis-library`/`app`/`workspace`/`render`/
`recording` 非接触。

## What was tried / learned

- 既存 coverage（`ctl_01.rs` 21 test・`ctl_02.rs` project 9 +
  player 3・`sf_02.rs` consent/atomicity・`sf06_oracle.rs`
  invariant oracle・`sf03_l1.rs` lowering helper）と重複しない
  よう、cross-gate（persisted→serialization/undo→lowering の
  同一 project 照合 + `CreateSend` consent）のみを fixture 化。
- `RoutingEditIntent` の `approved_conversion` は map が equal
  layout の permutation でも必須 — fixture は swap map
  `[1,0]` を Approved 付きで commit し persisted→spec edge の
  `(src,dst)` 変換を実測。
- send の `channel_map` は gain→destination edge に載る
  （`lower_send_edge`）— `source_map.connections[send_conn]` の
  `Resolved` id で spec edge を引き `channel_map()` を照合。

## Verification（実測）

- `cargo test -p synvelis-player --test sf_xr_01` — 2 passed
  （補助証拠。canonical は `mise run test`）。
- `cargo clippy -p synvelis-player --tests -- -D warnings` —
  clean（`#[expect(clippy::too_many_lines)]` は既存 test 慣行）。
- `mise run fmt-check` / `check` / `lint` / `test` — merge
  `origin/main` 後に実行（結果は PR 本文に記録）。

## NotRun（正直な残件）

実 channel 変換の DSP/PCM・実 OS GUI・native plugin host・
consumer lane・RT/probe 実測 — ledger の T1/T3/T4 および
実変換部分は `NotRun` のまま。
