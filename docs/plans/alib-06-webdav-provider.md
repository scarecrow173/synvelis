---
id: "PLAN-alib06-webdav-provider"
type: "plan"
status: "in-progress"
title: "ALIB-06 実装計画 — WebDAV File-Server Provider（synvelis-library slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/90"
  changes:
    added:
      - type: "dir"
        path: "crates/synvelis-library/src/webdav"
      - type: "file"
        path: "crates/synvelis-library/tests/webdav.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/common/webdav_fixture.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-06-webdav-provider.md"
  implemented-by: []
  depends-on:
    - "docs/designs/alib-06-webdav-provider-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-03-catalog-preview-cache.md"
    - "docs/specs/alib-04-git-remote-provider.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/alib-06-webdav-provider.md"
---

# ALIB-06 Plan

## Task decomposition

### Task 1 — transport 基盤 module（url/locator/http/xml/dav）

- [ ] `src/webdav/url.rs`: `WebDavUrl`/`WebDavScheme` —
      `http`/`https` origin parse・userinfo/query/fragment/
      非 ASCII host 拒否・port 既定値・authority 文字列。
- [ ] `src/webdav/locator.rs`: 絶対 path locator 文法
      （percent decode→component 検証→normalize）・
      `root_path` confinement・request path encode・
      child locator 構成。
- [ ] `src/webdav/xml.rs`: XML pull parser — start/end/
      empty/text event・entity/CDATA/comment/DOCTYPE/
      namespace local-name・bounded input。
- [ ] `src/webdav/http.rs`: request 構築（origin-form・
      Host・Connection: close・headers・body）・response
      parse（status/headers/body 3 系統）・chunked
      dechunk・redirect policy（scheme/host 制約・auth
      strip・上限・loop 検出）・stall poll（cancel/
      deadline/io_timeout）・`BodyReader`（typed error
      埋め込み）。
- [ ] `src/webdav/dav.rs`: PROPFIND body 組立・
      multistatus incremental parse・prop 抽出
      （displayname/resourcetype/getcontentlength/
      getlastmodified/getetag）・HTTP-date parse・
      ETag strong/weak/unusable 分類。

### Task 2 — provider + exports

- [ ] `src/webdav/mod.rs`: `WebDavProvider`
      （`AssetProvider` 実装: browse/stat/pin/
      open_pinned/open_preview/search/capabilities）、
      `WebDavConfig`/`WebDavProviderInfo`/
      `WebDavScheme` re-export、status→error 対応表、
      credential 解決・auth header 注入・redirect 越境
      auth strip。
- [ ] `src/lib.rs`: `mod webdav;` + re-export（additive
      のみ）。

### Task 3 — fixture + tests

- [ ] `tests/common/webdav_fixture.rs`: in-process
      WebDAV/HTTP server（in-memory tree・PROPFIND/GET/
      HEAD・auth・redirect・chunked・stall・disconnect・
      weak/no etag・大 collection・request 記録）。
- [ ] `tests/webdav.rs`: contract 相当 sequence・strong/
      weak ETag pin 分岐・If-Match 412・Range 206/200・
      bounded listing Partial・redirect・auth revoke・
      timeout・mid-transfer disconnect・cancel・
      redaction・traversal/confinement・import_verified
      接続・capabilities/info。

### Task 4 — 検証・docs 仕上げ

- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
      `test-msrv`/`build`（+ 存在すれば app-native 系）。
- [ ] managed index 再生成・impl record・`git fetch origin
      main && git merge origin/main` 後の再検証。

## File map

| path | 役割 |
| --- | --- |
| `crates/synvelis-library/src/webdav/mod.rs` | provider + 公開 config/info |
| `crates/synvelis-library/src/webdav/url.rs` | base URL 検証 |
| `crates/synvelis-library/src/webdav/locator.rs` | locator 文法・confinement |
| `crates/synvelis-library/src/webdav/http.rs` | HTTP/1.1 transport |
| `crates/synvelis-library/src/webdav/xml.rs` | XML pull parser |
| `crates/synvelis-library/src/webdav/dav.rs` | PROPFIND/multistatus/ETag/date |
| `crates/synvelis-library/src/lib.rs` | module+re-export（additive） |
| `crates/synvelis-library/tests/common/webdav_fixture.rs` | fixture server |
| `crates/synvelis-library/tests/webdav.rs` | acceptance tests |

依存追加なし（`Cargo.toml`/`Cargo.lock`/`mise.toml` は
変更しない — HTTP は `std::net::TcpStream`、XML は自前
pull parser、fixture は `std::net::TcpListener`。dev-dep
追加も不要）。

## Risk register

| risk | 対応 |
| --- | --- |
| TLS 非保有で https server に繋げない | `WebDavScheme::Https` を config で受理し transport が typed `UnsupportedCapability`。http への silent downgrade・cross-scheme redirect follow を拒否 |
| WebDAV paging 非対応の大 collection | `max_entries`/body byte cap で bounded listing、`Coverage::Partial` + 収集 window 内 cursor のみ。全件列挙を偽装しないことを docs/test で明示 |
| ETag header injection（server 制御値） | validator を ASCII entity-tag grammar で fail-closed 検証してから `If-Match` に載せる |
| cross-origin redirect で credential 漏洩 | origin（scheme+host+port）一致時のみ `Authorization` 転送。303 は follow せず typed error |
| socket stall 中の cancel/deadline 不応 | 短い read timeout slice の retry loop で ~50ms 毎に cancel/deadline/io_timeout を確認 |
| chunked/content-length/close の 3 系統 body | `BodyReader` で系統別に実装し、protocol 違反・mid-transfer disconnect を typed `ProviderError` 埋め込み io error で返す |
| multistatus XML の複雑さ | 限定 pull parser（entity/CDATA/comment/DOCTYPE/local-name）+ 未知要素読み飛ばし。fixture で実 parse 経路を検証 |
| 非 UTF-8/grammar 外 href | entry skip + `unaddressable` → `Coverage::Partial`（git provider と同じ fail-visible 方針） |
| `getlastmodified` の非標準 date | IMF-fixdate/RFC850/asctime の 3 形式 parse、失敗は `Unknown{Other}`（捏造しない） |

## Verification matrix

| 検証 | 手段 |
| --- | --- |
| fmt/lint/check/test | `mise run fmt-check check lint test build test-rt test-msrv` |
| strong/weak ETag pin | fixture の ETagKind 切替で pin 成功/UnsupportedCapability を検証 |
| If-Match conditional read | pin→GET wire に `If-Match` が乗ること・412→PreconditionFailed・応答 ETag 不一致→SourceChanged を検証 |
| Range 対応/非対応 | 206/200 両経路・bounded discard・byte budget を検証 |
| paging なし大 collection | `max_entries` 超 flat dir で Partial + window cursor の保証範囲を検証 |
| redirect/auth | same/cross-origin・auth strip・revoke→AuthExpired を検証 |
| timeout/disconnect/cancel | stall・mid-transfer close・CancellationToken で typed error を検証 |
| import 接続 | pin→import_verified→SHA-256 blob→offline 再読を検証 |
| redaction/traversal | credential 非含有・`..`/encoded traversal/confinement escape 拒否を検証 |
| 非回帰 | 既存 `tests/*.rs` 全 pass（ALIB-01〜04） |
| NotRun/Unsupported | 実 WebDAV server/network・TLS・実 OS・GUI・auth handshake を明記 |

## Rollback

全変更は `crates/synvelis-library/` 内の additive module +
`tests/` + `docs/` のみ。revert commit で完全に戻せる。
`Cargo.toml`/`Cargo.lock`/`mise.toml`/`.github/` は不変。
