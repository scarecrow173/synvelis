---
id: "TASK-ktcgo0ekChv0F6hDG8O3rr"
type: "task"
status: "done"
title: "X task1 canonical receipt schema + commit digest 接続 (synvelis-recording)"
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

# X task1 canonical receipt + commit digest 接続

## Work

`synvelis-recording`: `staging.rs` に `CanonicalSourceRef` schema
(`deny_unknown_fields`)・`CANONICAL_SCHEMA_VERSION`・`canonical_rel`/
`canonical_path`・`load_canonical` (best-effort、error 記録) を追加。
`commit.rs`: `StreamEntry.raw_digest: Option<String>` (`serde(default)`)、
`build_payload` で canonical receipt 整合時 `digest=canonical.digest` +
`raw_digest=Some(fold)`、不整合時 `CommitError::Manifest`、不在時従来。
`COMMIT_SCHEMA_VERSION` bump。整合/不在/不整合の unit test。
