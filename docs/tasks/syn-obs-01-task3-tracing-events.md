---
id: "TASK-034RBFW6PKUMmU8bg04whw"
type: "task"
status: "done"
title: "SYN-OBS-01 task3 service/app 非RT tracing イベント"
created: "2026-11-04"
updated: "2026-11-04"
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
    - "docs/plans/syn-obs-01-host-logging-source-catalog.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-obs-01-host-logging-source-catalog.md"
    - "docs/tasks/syn-obs-01-task1-deps-logging-module.md"
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
    - "docs/test-specs/syn-obs-01-logging-source-catalog.md"
  references: []
  defers: []
  deferred-by: []
---

## Work

- `src/service.rs`: op dispatch 受領時 `debug!(op = name)`、
  `CallOutcome` が error 系のとき `warn!`(op 名のみ、params/body は
  field に載せない)。
- `src/app.rs`: `project_new`/`project_open`/`project_save` 成功時
  `info!`、失敗時 `warn!`(`AppError` の kind のみ、パスや内部値を
  field に載せない)。`request_quit` で `info!`。
- `record_intent`/`record_intent_error` のカウンタ発火点では
  イベントを emit しない(頻度が高いため `debug!` は op 層に留める)。
- 機密分類規約: event field にパス・秘密情報・生データを書かない。
  doc-comment にも明記。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/syn-obs-01-host-logging-source-catalog.md`

## Dependencies

- `docs/tasks/syn-obs-01-task1-deps-logging-module.md`

## Affected Files

- `crates/synvelis-app/src/service.rs`、`crates/synvelis-app/src/app.rs`

## Done When

- [ ] op 受領/失敗/プロジェクト操作で `tracing` event が発行される。
- [ ] emit field にパス・秘密情報を含まない。

## Verification

- [ ] `mise run lint` pass、`cargo test -p synvelis-app` pass
