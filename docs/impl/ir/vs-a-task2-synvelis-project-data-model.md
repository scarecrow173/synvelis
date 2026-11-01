---
id: "IMPL-034Qeedli77Yb2i69YgXSZ"
type: "impl"
status: "completed"
title: "VS-A task2 synvelis-project data model"
created: "2026-11-01"
updated: "2026-11-01"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/error.rs"
      - type: "file"
        path: "crates/synvelis-project/src/id.rs"
      - type: "file"
        path: "crates/synvelis-project/src/time.rs"
      - type: "file"
        path: "crates/synvelis-project/src/range.rs"
      - type: "file"
        path: "crates/synvelis-project/src/tempo.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/recording_data.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/id_time_range.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tempo.rs"
    modified:
      []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/vs-a-task2-synvelis-project-data-model.md"
  implemented-by: []
  depends-on: []
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
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---


# VS-A task2 synvelis-project data model

## Summary

Implemented the `synvelis-project` data model: 12 compile-time-distinct
stable ID kinds, checked time domains, half-open generic ranges, project POD
aggregate with recording registries and Source port, and the Synvelis-owned
tempo DTO layer over ruviform `TempoMap`.

## Context

Task2 of docs/plans/vs-a-project-recording-foundation.md; data contract per
the approved design doc (DESIGN-034QdO7AB6cp4DRgKeuuEn).

## Implementation

### Changes

- `id.rs`: macro-generated `u128`-backed ID newtypes
  (Project/Track/Lane/Clip/Source/Asset/Take/RecordingSession/RecordingPass/
  CaptureStream/CaptureSegment/ClockEpoch), tagged string serde
  (`<kind>-<32hex>`), `IdGenerator` trait + deterministic
  `CounterIdGenerator`, `compile_fail` doctests.
- `time.rs`: `SessionFrame`/`SourceFrame` (nonnegative `i64`, checked
  add/sub), `ProjectTime` (allows negative pre-roll), `MusicalTick`,
  `MediaTimeValue` — no ruviform `Tick` arithmetic used.
- `range.rs`: validated `Range<F>` `[start,end)` (inversion rejected,
  empty allowed), `contains`/`intersects`/`intersection`, plus
  `SessionRange`/`SourceRange`/`MusicalRange`/`MediaSpan` aliases.
- `tempo.rs`: `ProjectPpqn` (default 960), `TempoMapData` DTOs,
  `TempoMapRef`, rebuild into ruviform `TempoMap` via
  `TempoMapBuilder` (`TempoMap` fields are private / no serde).
- `project.rs`: `Project` aggregate (tracks/lanes/clips/sources/assets/
  tempo map + recording registries + `id_watermark`), `SourceState`
  strict one-stage transitions, `validate_references`.
- `recording_data.rs`: POD recording entities + `SessionState`
  (`Created->Open->Closed->Finalized`).

### Decisions During Implementation

- ID text parse uses `rsplit_once('-')` so multi-hyphen kinds such as
  `clock-epoch` and `recording-session` round-trip correctly (fixed a
  real PoC reload failure found by tests).
- `SourceState::next()` enforces strictly one-stage-forward progression
  (fixed an Imported->Saved skip found by tests).

## Related Experiments

### Adopted

None — approach was clear from the approved design; no experiment logs.

### Rejected

None.

## Validation

- `id_time_range.rs`: all 12 ID round-trips, checked-arithmetic rejections
  (negative/overflow/inverted), half-open adjacency semantics.
- `tempo.rs` tests: 120 BPM / 4/4 / 48 kHz four-bar loop = 384,000 frames
  <=> 15,360 ticks; PPQN default 960; TempoMapData serde round-trip.
- `compile_fail` doctests prove cross-kind type separation.

## Risks

- `Range<F>` is serialized as raw `start`/`end`; inversion is only
  rejected at construction — deserialization re-validates via
  `Project::validate_references` on load.

## Follow-ups

- Resolution-independent musical time (issue #19 / ruviform #189) will
  supersede the fixed-PPQN persistence context.
