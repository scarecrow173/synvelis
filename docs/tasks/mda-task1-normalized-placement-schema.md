---
id: "TASK-A12kQItkHfVbth42pCr8ga"
type: "task"
status: "done"
title: "MD-A task1 正規化 Placement/Playback 型・Clip 移行・consumer 適合"
created: "2026-11-03"
updated: "2026-11-03"
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
    - "docs/plans/mda-comp-model-poc.md"
  implemented-by: []
  depends-on:
    - "docs/plans/mda-comp-model-poc.md"
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
    - "docs/test-specs/mda-model-poc-acceptance.md"
  references: []
  defers: []
  deferred-by: []
---

# MD-A task1 正規化 Placement/Playback 型・Clip 移行・consumer 適合

## Work

- `placement.rs` (新): `TimeAnchor`(点)、`ClipExtent`、
  `MusicalDuration`/`MediaDuration`、`ClipPlacement`、
  `ClipPlayback`、`PlaybackMapping`、`StretchRatio`、
  `resolve`/`ResolvedClip`。
- `project.rs`: 旧 `TimeAnchor`→`ProjectRange` rename、`Clip` を
  `placement`+`playback` へ移行。
- `ops.rs`: `AddClip`/`MoveClip`(anchor 指定)・`SetClipExtent`・
  `SetClipPlayback`。
- `store.rs`: `SCHEMA_VERSION = 2`。
- consumer 適合: `commit.rs` (clip 生成/検査)、`lowering.rs`
  (resolve 消費 + `SkippedClip::UnsupportedMapping`)、各テスト。

## Done

- `cargo check -p synvelis-project -p synvelis-recording -p synvelis-player`
  が通り、既存テストの Clip 構築が新スキーマに適合。
