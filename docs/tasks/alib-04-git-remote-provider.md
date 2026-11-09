---
id: "TASK-alib04-t1"
type: "task"
status: "done"
title: "ALIB-04 Task 1 — git provider modules・fixture・tests"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/alib-04-git-remote-provider.md"
  depends-on:
    - "docs/designs/alib-04-git-remote-provider-design.md"
  verifies:
    - "docs/test-specs/alib-04-git-remote-provider.md"
---

# ALIB-04 Tasks

## Task 1 — git 基盤 module

- [x] `src/git/url.rs`: `GitRemoteUrl`/`GitRemoteScheme` —
      scheme allowlist（file/http/https/ssh/git）・userinfo
      拒否・`ext::`/`fd::` 拒否・local path→`file://` 正規化・
      `lfs_objects_root()`。
- [x] `src/git/locator.rs`: `<rev>:<path>` 文法 parse/validate/
      child locator/scope 分解。
- [x] `src/git/objects.rs`: `ls-tree -z -l` parser・mode/type
      分類・Git LFS Pointer parse・`run_git`（固定 argv/env/
      credential env 注入/interrupt）・stderr→error 分類。
- [x] `src/git/mirror.rs`: `ensure_mirror`・filter 検出・
      `refresh`・`resolve_commit`・`list_tree`/`stat_entry`・
      blob stream・epoch→sha map・`GitProviderInfo`。
- [x] `src/git/lfs.rs`: `LfsBackend` local objects open。

## Task 2 — provider + exports

- [x] `src/git/mod.rs`: `GitRemoteProvider`（browse/stat/pin/
      open_pinned/open_preview/search/capabilities）、
      `GitRemoteConfig`/`GitProviderInfo`/
      `UnsupportedFilterPolicy`/`GitObjectFormat`/`MirrorMode`/
      `LfsBackendKind`。
- [x] `src/lib.rs`: `mod git;` + re-export（additive）。

## Task 3 — fixture + tests

- [x] `tests/git_fixture.rs`: work/bare/`file://`/LFS/
      submodule/secret fixture builder（git 非存在時は
      `Option` で明示 skip）。
- [x] `tests/git.rs`: contract 相当 sequence・LFS・selective・
      pin 固定・import 接続・failure 分類・redaction・cursor。

## Notes

- git 呼出は `std::process::Command` 固定 argv — 依存追加なし。
- Git OID ≠ `ContentDigest`。LFS oid = SHA-256 のみ一致。
- mirror は derived cache — 全喪失は再 clone で回復。
- 実 network/git server・実 OS・GUI は NotRun/Unsupported。
