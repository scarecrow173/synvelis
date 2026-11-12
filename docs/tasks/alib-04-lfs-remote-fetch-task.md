---
id: "TASK-alib04-lfs-remote-fetch"
type: "task"
status: "completed"
title: "ALIB-04 LFS Remote Fetch — タスク"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/331"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-04-lfs-remote-fetch-plan.md"
  implemented-by:
    - "docs/impl/ir/alib-04-lfs-remote-fetch.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/alib-04-git-remote-provider.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/alib-04-lfs-remote-fetch.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ALIB-04 LFS Remote Fetch — タスク

- [x] T1: `webdav/http.rs` に `HttpEndpoint`/`send_endpoint`/
  `HttpRequest.query` を追加し `send` を薄い wrapper 化
- [x] T2: `webdav/mod.rs` の module 可視性を `pub(crate)` 化し
  `HttpRequest` 構築を追従
- [x] T3: `git/lfs.rs` に `RemoteHttp` backend・endpoint 解決・
  batch API・download GET・型付きエラー・credential 境界を実装
- [x] T4: `git/mod.rs` で credential_key 受渡と
  `Box<dyn Read+Send>` 適合
- [x] T5: `tests/lfs.rs` — e2e download+検証、404/401/403、
  size/digest 不一致、malformed、https unsupported、
  credential 非漏洩、file:// 回帰
- [x] T6: doc chain（spec/design/plan/task/test-spec/impl-ir）
- [x] T7: canonical verification 全実走
