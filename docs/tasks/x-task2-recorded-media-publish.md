---
id: "TASK-lon8rNqrFA4DGM8WvurTtM"
type: "task"
status: "done"
title: "X task2 recorded media canonical publish (synvelis-player)"
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
    - "docs/plans/x-recorded-verified-source-playback-export.md"
  implemented-by: []
  depends-on:
    - "docs/plans/x-recorded-verified-source-playback-export.md"
    - "docs/tasks/x-task1-canonical-ref-manifest.md"
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
    - "docs/test-specs/x-e2e-recorded-source-playback-export.md"
  references: []
  defers: []
  deferred-by: []
---

# X task2 recorded media canonical publish

## Work

`synvelis-player` に `ruviform-io-wav` dep + `publish.rs`:
`publish_recorded(area, media_root) -> Result<PublishReport, PublishError>`。
Complete ref のみ: identity 整合 (staging manifest 照合) → frontier
`media_byte_len` bytes のみ読取・frame 一致検証 → deinterleave →
`WavEncoder(F32)`+`WavMuxer` → `content_digest` → `blob_path` 原子公開 →
`canonical.json` 原子公開。冪等 (receipt 整合 → reused、blob 欠損 → blob
のみ再公開、不整合 → Conflict)。layout: 1ch Mono/2ch Stereo、他
`UnsupportedChannels` (capability gap)。`PublishReport{published,reused,
skipped}`。unit test で publish→receipt→blob 検証 + partial/不整合 skip。
