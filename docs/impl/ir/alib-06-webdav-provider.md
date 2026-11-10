---
id: "IMPL-ALIB06WebDavProvider"
type: "impl"
status: "completed"
title: "ALIB-06 WebDAV File-Server Provider 実装記録"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/90"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-library/src/webdav/mod.rs"
      - type: "file"
        path: "crates/synvelis-library/src/webdav/url.rs"
      - type: "file"
        path: "crates/synvelis-library/src/webdav/locator.rs"
      - type: "file"
        path: "crates/synvelis-library/src/webdav/http.rs"
      - type: "file"
        path: "crates/synvelis-library/src/webdav/xml.rs"
      - type: "file"
        path: "crates/synvelis-library/src/webdav/dav.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/webdav.rs"
      - type: "file"
        path: "docs/specs/alib-06-webdav-provider.md"
      - type: "file"
        path: "docs/designs/alib-06-webdav-provider-design.md"
      - type: "file"
        path: "docs/plans/alib-06-webdav-provider.md"
      - type: "file"
        path: "docs/tasks/alib-06-webdav-provider.md"
      - type: "file"
        path: "docs/test-specs/alib-06-webdav-provider.md"
      - type: "file"
        path: "docs/impl/ir/alib-06-webdav-provider.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-06-webdav-provider.md"
    - "docs/designs/alib-06-webdav-provider-design.md"
    - "docs/specs/alib-06-webdav-provider.md"
    - "docs/tasks/alib-06-webdav-provider.md"
    - "docs/test-specs/alib-06-webdav-provider.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# ALIB-06 WebDAV File-Server Provider 実装記録

## Summary

Issue #90「ALIB-06 WebDAV File-Server Provider」の実装。
`synvelis-library` に read-only の WebDAV backed `AssetProvider` を
追加した。metadata-first の `PROPFIND` (browse は Depth:1、stat/pin は
Depth:0) で listing・fact 収集を行い、pin は strong ETag のみを
`PinProof::ConditionalStrong` + `StrongValidator{IfMatch}` で結び、
`open_pinned` は `If-Match` 条件付き GET で revision を強制する。
transport は新規依存を追加せず `std::net::TcpStream` 上の最小
blocking HTTP/1.1 client として `webdav::http` に実装し、`http` のみ
対応(`https` は parse 可能だが TLS stack 非保有のため connect 時に
typed `UnsupportedCapability`、silent downgrade なし)。browse の真の
paging は WebDAV に存在しないため entry/byte budget で bounded
listing とし、打ち切りは `Coverage::Partial` として正直に報告する。
search は server-side index 非保有のため `RequiresIndex` で宣言的に
unsupported。

## Context

- Issue: scarecrow173/synvelis#90(親・設計オーナー: #9。関連:
  #17/#21/#7/#23 Provider 契約、#85 ALIB-01 baseline、#86 ALIB-02
  import、#87 ALIB-03 cache、#88 ALIB-04 git provider、#91 fault
  matrix)
- 前提: `91a2c3a9` 以降の `origin/main`(ALIB-02〜04 merge 済み)から
  `feature/90-alib06-webdav` を作成。作業中に `origin/main` を merge
  し VCS-02(#229)を取り込み、競合なし。
- 全検証は in-process の `TcpListener` 製 local HTTP/WebDAV fixture
  で実施。実 network・実 WebDAV server・OS credential store・GUI は
  NotRun/Unsupported。

## Implementation

### Changes

- `src/webdav/url.rs`: `WebDavUrl`/`WebDavScheme`。`http`/`https` +
  authority のみ受理し userinfo・query・fragment・path(``/`のみ許容)・
  malformed port・empty/non-ASCII host を拒否。default port は
  canonicalize、IPv6 literal(`[::1]`)対応、host は lowercase 正規化
  (RFC 準拠の case-insensitive 比較)。`same_origin`/`authority_matches`
  で credential を origin に閉じ込める。
- `src/webdav/locator.rs`: locator = decode 済み absolute server
  path。`normalize_path` が leading slash 1 本・末尾 slash なし
  (`/`のみ例外)・`.`/`..`/空 component/`\`/制御文字/NUL/過長
  path/component を拒否し `root_path` confinement を強制
  (`/music` と `/musical` を混同しない境界一致)。`href_to_path` は
  absolute/absolute-URI/scheme-relative/relative href を解決し、
  percent-decode は厳密に 1 回、dot-segment は構造的に除去。
  `merge_relative` が `/` 直下の dir merge で `//` を生成しないよう
  集約。
- `src/webdav/http.rs`: 依存なし最小 blocking HTTP/1.1 client。
  request head/body の byte 上限、header 数上限、response head の
  bounded read(先読み分は `pending` として body に引き継ぎ)、
  `Content-Length` と chunked の両対応、stall 検出 + deadline/cancel
  polling、same-origin redirect のみ follow(上限あり・cross-origin
  は拒否し credential を漏らさない)。`https` scheme は connect 前に
  `UnsupportedCapability`。
- `src/webdav/xml.rs`: multistatus subset 用 streaming pull parser。
  entity/CDATA/comment/PI/namespace-local 名対応、strict UTF-8、
  depth/byte/text 上限、malformed input は fail-closed。
- `src/webdav/dav.rs`: `PROPFIND` body 生成(`resourcetype`/
  `getcontentlength`/`getlastmodified`/`getetag`/`displayname`)と
  multistatus parser。`200` propstat のみ採用、`resourcetype` の
  `collection` で container 判定、missing/foreign/unaddressable
  href は skip して partial coverage に計上、entry 数・body byte
  上限で打ち切り。
- `src/webdav/mod.rs`: `WebDavProvider`(`AssetProvider` 実装)。
  browse/stat/pin/open_pinned/open_preview、capabilities
  (search=`RequiresIndex`、write 系は非宣言)、cursor は provider/
  scope/query/principal/membership fingerprint 束縛で changed
  collection を `CursorInvalid` で拒否。HTTP status →
  `ProviderErrorKind` 分類(401/403→auth/permission、404→
  NotFound/RevisionGone、412→PreconditionFailed、429→RateLimited、
  5xx→TransientIo 等)。pin は strong ETag のみ受理し weak(`W/`)・
  missing は `UnsupportedCapability` で fail-closed。
  `open_pinned` は `If-Match` 送信 + 応答 ETag 照合(不一致→
  `SourceChanged`)。preview は `Range` 要求で `206`+`Content-Range`
  検証、server が range を無視した `200` は bounded client-side
  skip(サーバ側 range 対応を偽装しない)、`416` は empty read。
  credential は call 毎に `ProviderContext.credentials` から解決し
  `Authorization` header のみ送出、secret は URL/locator/error/
  log に出さない。read は `ProviderLimits.max_bytes` で bounded、
  oversize は verifying reader/completion slot 経由で typed failure。
- `src/lib.rs`: `mod webdav` + additive re-export
  (`WebDavProvider`/`WebDavConfig`/`WebDavScheme`/`WebDavUrl`)のみ。
  `Cargo.toml`/`Cargo.lock`/`mise.toml` 不変(依存追加なし)。
- `tests/webdav.rs`: 依存なし in-process fixture(`TcpListener` で
  request を capture し scripted response を返す)。34 test —
  URL/authority 受理拒否、root confinement、browse 完全性/
  pagination/partial coverage/truncation/foreign href/file scope
  拒否、stat facts、strong/weak/missing ETag pin、If-Match proof・
  conditional read、応答 ETag 不一致/412/404 分類、range 206/
  ignored-range 200/416、max_bytes、cancel、credential 解決・
  secret 非漏洩、same-origin redirect・cross-origin 拒否、
  非 WebDAV server 拒否、foreign proof 拒否、https typed
  unsupported。fixture/test 双方に unit test(url/locator/xml/dav)。

### Decisions During Implementation

- pin/conditional read の revision 強制は server 側 `If-Match` に
  委譲し、client は応答 ETag の独立照合で `SourceChanged` を検出する
  二重化。weak ETag は RFC 上 `If-Match` に使えないため受理しない。
- `BoundedReader` は cap ちょうどの body を EOF と誤認しないよう
  cap+1 byte を probe し、超過は `BudgetExceeded` に分類する。
- response head 読み取りで先読みした body 先頭バイトを失わないよう
  `ResponseHead.pending` で body reader に引き継ぐ。
- WebDAV の listing に真の cursor paging はないため、browse は
  entry/byte budget 打ち切り + `Coverage::Partial` の正直な報告とし、
  cursor 継続は membership fingerprint 不一致で `CursorInvalid` を返す。
- preview で server が `Range` を無視して `200` を返した場合、
  client-side で bounded skip するが range 対応済みとは主張しない。
- host の case 正規化は `same_origin`/credential confinement の
  一貫性のため parse 時に lowercase 化(RFC 3986)。

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `build` / `test-msrv`: 全て pass(workspace 全 target、locked
  dependency graph、MSRV rust 1.89.0)。
- `mise run check-app-native` / `lint-app-native`: pass。
  `build-app-native` の結果は PR 記載の最終検証値を参照。
- `cargo test -p synvelis-library --test webdav`: 34/34 pass
  (in-process local fixture)。
- NotRun/Unsupported: 実 network・実 WebDAV server・`https`(TLS
  stack 非保有)・実 OS credential store・GUI・実 server 障害 —
  設計上の対象外。`synvelis-app`/workspace/project/gateway/player/
  recording/render/theme/poc・`.github/`・root manifest は未変更。
