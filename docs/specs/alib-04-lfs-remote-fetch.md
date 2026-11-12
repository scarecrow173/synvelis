---
id: "SPEC-alib04-lfs-remote-fetch"
type: "spec"
status: "approved"
title: "ALIB-04 LFS Remote Fetch — Git LFS batch API / HTTP download による remote LFS object 実Byte取得（synvelis-library slice）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/331"
    - "https://github.com/scarecrow173/synvelis/issues/91"
    - "https://github.com/scarecrow173/synvelis/issues/88"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-library/tests/lfs.rs"
      - type: "file"
        path: "docs/specs/alib-04-lfs-remote-fetch.md"
      - type: "file"
        path: "docs/designs/alib-04-lfs-remote-fetch-design.md"
      - type: "file"
        path: "docs/plans/alib-04-lfs-remote-fetch-plan.md"
      - type: "file"
        path: "docs/tasks/alib-04-lfs-remote-fetch-task.md"
      - type: "file"
        path: "docs/test-specs/alib-04-lfs-remote-fetch.md"
      - type: "file"
        path: "docs/impl/ir/alib-04-lfs-remote-fetch.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/git/lfs.rs"
      - type: "file"
        path: "crates/synvelis-library/src/git/mod.rs"
      - type: "file"
        path: "crates/synvelis-library/src/webdav/http.rs"
      - type: "file"
        path: "crates/synvelis-library/src/webdav/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/alib-04-git-remote-provider.md"
    - "docs/specs/alib-06-webdav-provider.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/alib-07-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/git-lfs/git-lfs/tree/main/docs/api/batch.md"
    - "https://github.com/git-lfs/git-lfs/blob/main/docs/api/discovery.md"
  defers: []
  deferred-by: []
---

# ALIB-04 LFS Remote Fetch — Git LFS batch API / HTTP download

## What Should Be Built

ALIB-04 Git Remote Provider の LFS backend を拡張し、`file://`
以外の remote で LFS pointer が指す**実バイト**を取得可能にする。
本 slice は **`http://` remote のみ**を対象とし、Git LFS Batch API
（`POST <endpoint>/objects/batch`）と download action の HTTP `GET`
でオブジェクトをストリーミング取得する。取得バイトは従来どおり
下流の SHA-256 `VerifyingReader` が oid・expected size と照合する。

`https://`（TLS スタック不在）、SSH、`git://` は本 slice でも
`UnsupportedCapability` のままとし、理由を設計・IR に明記する。
#91（ALIB-07 fault matrix）の判定行・test-spec は変更しない。

## Why

- #91 fault-matrix lane `LIB-PIN-02` / `ALIB-REAL-03` は「remote
  LFS object の実バイト取得」を要求するが、#88 実装は `file://`
  の `lfs/objects` 直接読みのみで、非 file transport は
  `Unsupported` だった。
- LFS pointer はメタデータであり、pointer テキストを asset bytes
  として返すことは契約違反。remote でも同一の不変条件
  （oid=SHA-256、declared size、検証済みストリームのみ fetchable）
  を維持しなければならない。

## Acceptance Criteria

- `http://` remote で `git-lfs:<oid>` identity の open が
  Batch API → `actions.download.href` GET → stream を返す。
- endpoint 解決は git-lfs server discovery の clone-URL 規則
  （remote path が `.git` 終端なら `+/info/lfs`、それ以外は
  `+.git/info/lfs`、batch は `+/objects/batch`）。
- `file://` の既存 local object 経路は不変（回帰 test あり）。
- `https://`/SSH/`git://` は `UnsupportedCapability`。
- 型付きエラー: 404→`NotFound`、401→`AuthExpired`、
  403→`PermissionDenied`、size/digest 不一致→`IntegrityMismatch`、
  非対応 hash_algo/transport→`UnsupportedCapability`。
- credential は remote origin 宛のみ。cross-origin の署名付き
  download href へは送らない。
- 実 remote・HTTPS・SSH の動作は外部 gate（NotRun 記録）。
