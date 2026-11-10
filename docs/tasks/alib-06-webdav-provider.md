---
id: "TASK-alib06-t1"
type: "task"
status: "in-progress"
title: "ALIB-06 Task 1 — webdav provider modules・fixture・tests"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/alib-06-webdav-provider.md"
  depends-on:
    - "docs/designs/alib-06-webdav-provider-design.md"
  verifies:
    - "docs/test-specs/alib-06-webdav-provider.md"
---

# ALIB-06 Tasks

## Task 1 — transport 基盤 module

- [ ] `src/webdav/url.rs`: `WebDavUrl`/`WebDavScheme` —
      scheme allowlist・userinfo/query/fragment/非 ASCII
      host 拒否・port 既定値。
- [ ] `src/webdav/locator.rs`: 絶対 path 文法・percent
      decode/encode・`root_path` confinement・child
      locator。
- [ ] `src/webdav/xml.rs`: XML pull parser（entity/CDATA/
      comment/DOCTYPE/local-name・bounded）。
- [ ] `src/webdav/http.rs`: HTTP/1.1 transport（request/
      response/chunked/redirect/auth/stall poll/
      `BodyReader`）。
- [ ] `src/webdav/dav.rs`: PROPFIND 組立・multistatus
      parse・prop 抽出・HTTP-date・ETag 分類。

## Task 2 — provider + exports

- [ ] `src/webdav/mod.rs`: `WebDavProvider`（browse/stat/
      pin/open_pinned/open_preview/search/capabilities）、
      `WebDavConfig`/`WebDavProviderInfo`。
- [ ] `src/lib.rs`: `mod webdav;` + re-export（additive）。

## Task 3 — fixture + tests

- [ ] `tests/common/webdav_fixture.rs`: in-process
      WebDAV/HTTP server + request 記録。
- [ ] `tests/webdav.rs`: contract sequence・ETag/If-Match・
      Range・bounded listing・redirect/auth・timeout/
      disconnect/cancel・redaction・traversal・import
      接続・capabilities。

## Task 4 — 検証・docs 仕上げ

- [ ] mise 正規検証一式 + merge origin/main + impl record
      + index 再生成。

## Notes

- HTTP transport は `std::net::TcpStream` の最小 blocking
  実装 — 依存追加なし。TLS/https は typed Unsupported。
- ETag は validator であり digest ではない —
  `digest_hint`/`checksum_hint` に載せない。
- weak ETag/ETag なしは pin fail-closed
  （`UnsupportedCapability`）。
- paging 非対応のため browse は bounded listing +
  `Coverage::Partial` の保証範囲を偽らない。
- 実 WebDAV server/network・TLS・実 OS・GUI は
  NotRun/Unsupported。
