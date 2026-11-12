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

## Summary

Issue #109 SF-XR-01 の headless slice を実装した。
`crates/synvelis-player/tests/sf_xr_01.rs`（commit
`43511f0`）に cross-gate oracle fixture を 2 test 追加し、
persisted strip/send 不変条件 → `InMemoryStore` save/load →
undo/redo → `lower_semantic` の `RoutingSourceMap`/`GraphSpec`
照合 → mute prune、および `CreateSend` の consent matrix と
commit 経路の atomicity を一気通貫で実測した。`src/` 変更
なしの fixture-only slice で、
`evidence/reports/sf-06-cross-gate-ledger.md` の SF-XR-01 行を
実測値で更新した。

## Context

Issue #109 の SF-06 cross-gate ledger で
`NotRun(#121/#122 依存)` のまま残っていた SF-XR-01 行の
headless 検証可能部分を実測する slice。CTL-01（#121・
closed）と CTL-02（#122・PR #243）の契約は実装済みで、
`crates/*/src/` を一切変更しない fixture-only slice
（`docs/specs/sfxr01-ctl-crossgate-oracle-fixture.md`）。
対象契約は 1 Track/Bus → 1 Strip cardinality・Host
`ParameterTargetId` が Send Gain の唯一正本・Send Gain /
PrePost / Mute の一貫性・Project Serialization / Undo /
Graph SourceMap の一致・異 Layout ChannelMap の明示 Consent。

## Implementation

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

## Related Experiments

### Adopted

- なし（`metadata.experiments.adopted` は空。`docs/impl/exp/`
  に本 slice 関連の experiment 記録なし）。

### Rejected

- なし。

## Validation

- `cargo test -p synvelis-player --test sf_xr_01` — 2 passed
  （補助証拠。canonical は `mise run test`）。
- `cargo clippy -p synvelis-player --tests -- -D warnings` —
  clean（`#[expect(clippy::too_many_lines)]` は既存 test 慣行）。
- `mise run fmt-check` / `check` / `lint` / `test` — merge
  `origin/main` 後に実行（結果は PR 本文に記録）。

## Risks

- **`lower_semantic` の内部仕様変更で fixture が脆くなる**:
  fixture は公開 API（`RoutingSourceMap`/`GraphSpec`/typed
  edge）のみを根拠にし、実装 helper には依存しない設計
  （`docs/plans/sfxr01-ctl-crossgate-oracle-fixture.md`
  Risks）。内部 lowering 変更時は fixture 側の追従が必要に
  なり得る。
- **ledger の lane 判定の過大評価**: headless 実測分のみを
  Pass とし、実 channel 変換 DSP・実 OS GUI・native plugin
  host・consumer lane・RT/probe 実測は `NotRun` を維持する
  運用（同 plan Risks・test-spec Out of scope）。
- **契約不整合を見つけた場合の扱い**: spec non-goal により
  `crates/*/src/` は変更しない — fixture が契約違反を観測
  した場合は変更せず報告する。本 slice ではそのような観測は
  なく 2 test 全 Pass。

## Follow-ups

実 channel 変換の DSP/PCM・実 OS GUI・native plugin host・
consumer lane・RT/probe 実測 — ledger の T1/T3/T4 および
実変換部分は `NotRun` のまま。
