---
id: "PLAN-alib04-lfs-remote-fetch"
type: "plan"
status: "approved"
title: "ALIB-04 LFS Remote Fetch — 実装計画"
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
    - "docs/specs/alib-04-lfs-remote-fetch.md"
    - "docs/designs/alib-04-lfs-remote-fetch-design.md"
  implemented-by:
    - "docs/tasks/alib-04-lfs-remote-fetch-task.md"
    - "docs/impl/ir/alib-04-lfs-remote-fetch.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/alib-04-git-remote-provider.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ALIB-04 LFS Remote Fetch — 実装計画

## Scope split

単一 slice。`LfsBackend` への `RemoteHttp` 追加・共有 HTTP 層の
内部一般化・headless fixture test・doc chain で完結する。

## File map

| file | 変更 |
| --- | --- |
| `crates/synvelis-library/src/webdav/http.rs` | `HttpEndpoint` 新設、`send_endpoint` 追加、`HttpRequest.query` 追加、`send`/`resolve`/`write_request` を endpoint ベースに内部 refactor |
| `crates/synvelis-library/src/webdav/mod.rs` | `http`/`locator`/`url` を `pub(crate)` 化、`HttpRequest` 構築に `query: None` |
| `crates/synvelis-library/src/git/lfs.rs` | `RemoteHttp` backend、endpoint 解決、batch parse、download GET、型付きエラー写像、credential 境界。local 経路は `open_local` として不変で温存 |
| `crates/synvelis-library/src/git/mod.rs` | `for_remote` に credential_key 受渡、`open_identity` の戻り値を `Box<dyn Read+Send>` に適合 |
| `crates/synvelis-library/tests/lfs.rs` | 新規: MockServer fixture + seeded mirror による e2e/typed error/regression |

## Dependency graph

`http.rs` 一般化 → `lfs.rs` 実装 → `git/mod.rs` 結線 →
`tests/lfs.rs` → doc chain → canonical verification。

## Risk register

| risk | 対応 |
| --- | --- |
| 署名 URL・credential の漏洩 | origin 一致時のみ auth 付与、action headers 優先、error detail に URL/token を出さない |
| https の silent downgrade | `UrlTarget` 段階で scheme 保持、`request` が socket 前に `UnsupportedCapability` |
| query の再 encode による token 破壊 | `HttpRequest.query` は verbatim 送出 |
| 巨大/敵対的 batch body | 1 MiB cap + typed error |
| `file://` 回帰 | `open_local` を無変更で分離、lfs.rs に regression test |

## Verification matrix

`mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
`check-app-native` / `lint-app-native` / `test-msrv` / `build` を
全て実走する。外部 gate（実 remote・HTTPS・SSH）は NotRun。

## Rollback

`LfsBackend::RemoteHttp` を除去し `for_remote` を従来の
local/unsupported 選択に戻せば完全に退避可能。`webdav` 側は
追加的変更のみで既存挙動を保持する。
