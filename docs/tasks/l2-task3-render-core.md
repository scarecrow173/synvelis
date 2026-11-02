---
id: "TASK-034QmkzixMrECeHtSzDAOq"
type: "task"
status: "done"
title: "L2 task3 render core processors"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/l2-master-sequential-offline-wav-export.md"
  implemented-by: []
  depends-on:
    - "docs/plans/l2-master-sequential-offline-wav-export.md"
    - "docs/tasks/l2-task2-snapshot-preflight.md"
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
    - "docs/test-specs/l2-master-sequential-offline-wav-export.md"
  references: []
  defers: []
  deferred-by: []
---

# L2 task3 render core processors

## Work

Implement the render core: `prepare.rs` (single-fd streaming digest +
demux + `SourceReader` open, rendered-span decode-to-memory),
`processor.rs` (`ClipRenderer` window server + deterministic
`GainProc`/`OnePoleProc`/`DelayProc` graph processors), and
`synvelis-player/src/lowering.rs` `lower_with_inserts` (master-bus
insert chain reuse of L1 lowering).

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/l2-master-sequential-offline-wav-export.md
- docs/designs/l2-master-sequential-offline-wav-export-design.md

## Dependencies

- docs/tasks/l2-task2-snapshot-preflight.md

## Affected Files

- `crates/synvelis-render/src/prepare.rs`
- `crates/synvelis-render/src/processor.rs`
- `crates/synvelis-player/src/lowering.rs`
- `crates/synvelis-player/src/import.rs` (`content_digest_stream`)
- `crates/synvelis-player/src/error.rs` / `resolver.rs`
      (`UnsupportedCodec` source_id)

## Done When

- [x] Sources open on one fd: streaming digest + canonical decode
- [x] Decode window limited to the rendered span ∩ clip window
- [x] `Delay{frames:0}` is identity; per-channel DSP state
- [x] Insert chain lowers between clips and master sink

## Verification

- [x] `l2_03_stateful_inserts_block_invariant` + `l2_03_delay_zero_is_identity` — pass
- [x] `cargo test --workspace --locked` — 121 pass / 0 fail
