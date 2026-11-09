---
id: "DSN-alib06-webdav-provider"
type: "design"
status: "approved"
title: "ALIB-06 — WebDAV File-Server Provider 設計（synvelis-library）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/alib-06-webdav-provider.md"
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
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-03-catalog-preview-cache.md"
    - "docs/specs/alib-04-git-remote-provider.md"
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
---

# ALIB-06 — WebDAV File-Server Provider 設計（synvelis-library）

## 1. 位置づけ

ALIB-01 の `AssetProvider` 契約に対する **Remote Provider
adapter（Git 以外）**。WebDAV resource は mutable であり、
Git の immutable object と違って pin は **server-side
validator（strong ETag）を fetch 時条件（`If-Match`）で
強制する conditional pin** のみが可能である。provider は
**read-only** であり、project aggregate・working tree・
index・HEAD に一切触れない。

権威の分担:

- remote の正本 = WebDAV server 上の resource。client 側に
  mirror/cache は持たない（catalog/preview cache は ALIB-03
  の責務であり provider は呼出毎に live 観測する）。
- asset identity = provider 層の `RemoteAssetRef`/locator/
  `ObjectIdentity`。project 側 ID とは型で分離（ALIB-01
  踏襲）。
- byte 検証 = 2 段。provider 層は **conditional precondition**
  （strong ETag + `If-Match`）で pin 時点の revision を固定し、
  import 層が staging bytes の **SHA-256** を測定して
  `ContentDigest` とする。ETag は validator であり content
  digest ではない — `digest_hint`/`checksum_hint` に載せない。

## 2. モジュール構成

```
src/webdav/mod.rs    WebDavProvider（AssetProvider impl）+
                     公開 config/info 型の re-export
src/webdav/url.rs    WebDavUrl（scheme/authority grammar・
                     userinfo/query/fragment 拒否・
                     origin への正規化）
src/webdav/locator.rs  絶対 path locator 文法・root_path
                     confinement・percent encode/decode・
                     child locator 構成
src/webdav/http.rs   最小 HTTP/1.1 transport（TcpStream・
                     request/response・chunked・redirect・
                     auth header・stall poll・typed error）
src/webdav/xml.rs    XML pull parser（entity/CDATA/comment/
                     DOCTYPE/namespace local-name）
src/webdav/dav.rs    PROPFIND body 組立・multistatus parse・
                     prop 抽出・HTTP-date parse・ETag 分類
```

`src/lib.rs` に `mod webdav;` + 公開 re-export を追加する
のみで、既存 module・公開契約は変更しない（additive）。

依存追加なし: HTTP transport は `std::net::TcpStream` の
最小 blocking 実装、XML は `src/webdav/xml.rs` の限定
parser、fixture は `std::net::TcpListener`。Ruviform Generic
HTTP/File/FS/Secret の重複実装はしない（汎用化が必要なら
両 Consumer 互換評価後に基盤 Issue へ還元）。

## 3. 公開型

```rust
pub struct WebDavConfig {
    pub base: WebDavUrl,               // 検証済み origin（scheme+authority）
    pub root_path: String,             // browse 既定 root・locator confinement
    pub credential_key: Option<CredentialKey>,
    pub connect_timeout: Duration,     // default 10s
    pub io_timeout: Duration,          // socket stall 上限 default 30s
    pub max_redirects: u32,            // default 5
}

pub struct WebDavProviderInfo {        // capability 証拠
    pub provider: ProviderId,
    pub scheme: WebDavScheme,          // Http | Https
    pub authority: String,             // host:port（credential-free）
    pub root_path: String,
    pub tls_available: bool,           // 常に false（本 slice）
    pub credential_configured: bool,
}
```

`WebDavProvider::new(id, config)` は接続を作らない（lazy:
初回 provider call で TCP connect）。`WebDavUrl::parse` は
`http://`/`https://` の origin のみ受理し、userinfo・
query・fragment・非 ASCII host・空 host を拒否する。
`https` は config 上は受理するが transport が typed
`UnsupportedCapability` を返す（TLS stack 非保有 —
http への silent downgrade はしない）。

## 4. Locator 文法と confinement（locator.rs）

- locator = server root からの **絶対 path**（`/` 開始、
  `/` 区切り）。`RemoteAssetRef.locator` の text がそのまま
  path であり、provider が各 call で再検証する。
- grammar: 成分は空・`.`・`..`・NUL・制御文字・`\` を拒否、
  成分長 ≤255、全体 ≤2048、先頭 `/` 必須、末尾 `/` は
  collection を示唆するが normalize で除去（root `/` のみ
  例外）。percent-encode 済み入力は一度 decode してから
  検証する（`%2e%2e` 等の encoded traversal を拒否）。
- confinement: normalize 後の locator path が `root_path`
  と一致するか `root_path + "/"` prefix を持つこと。
  `root_path="/"` は server 全体を許可。root 外 →
  `InvalidRequest`（全入口 — browse scope・stat・pin・
  open_pinned・open_preview — で再検証。foreign に
  細工された `PinnedAssetRef` もここで止まる）。
- request path: locator を segment 毎に percent-encode
  （unreserved 以外を `%XX`）して origin-form に使う。
- child locator: PROPFIND href を percent-decode→normalize
  →同 grammar で検証して `OpaqueLocator` 化。root_path 外の
  href（server が symlink 的に外を指す等）は entry を
  skip + `truncated`（`Coverage::Partial`）。

## 5. HTTP transport（http.rs）

`std::net::TcpStream` のみ。connection は request 毎に
新規確立（keep-alive pool なし — YAGNI）。

- `connect`: `ToSocketAddrs` 解決後 `connect_timeout` で
  `TcpStream::connect_timeout`。`set_read_timeout`/
  `set_write_timeout`/`set_nodelay(true)`。
- stall poll: blocking socket + 短い read timeout slice
  （既定 50ms）で `read` を retry し、各 retry で
  `cx.cancel`/`deadline`/累積 `io_timeout` を確認する。
  socket stall 中でも cancel/deadline が ~50ms で効く
  （Cancelled Fetch の中核）。timeout 超過 →
  `TransientIo`、cancel → `Cancelled`、deadline →
  `BudgetExceeded`。
- request: `METHOD <encoded-path> HTTP/1.1` + `Host` +
  `Connection: close` + 呼出側 headers + body
  （PROPFIND のみ。Content-Length 付き）。header 値は
  検証済み構成要素のみ（path は encode 済み、validator は
  §7 の grammar 通過値、Authorization は opaque context）。
- response: status line + headers（小文字化 key の
  multi-map）+ body。body は `Content-Length`（完全一致を
  検証 — 不足/過剰は typed error）・`Transfer-Encoding:
  chunked`（dechunk stream）・close-delimited の 3 系統。
  `Content-Encoding` が `identity` 以外 →
  `UnsupportedCapability`（gzip 等を黙って decode しない）。
- GET body は `BodyReader`（`Read`）として返し、
  `VerifyingReader` が外側で digest/size/cancel を担う。
  chunked の dechunk・content-length countdown・close 検出を
  reader 内で行い、protocol 違反・mid-transfer disconnect は
  `io::Error` に typed `ProviderError` を埋め込んで返す
  （`io_provider_error` の downcast 経路で `TransientIo`/
  `IntegrityMismatch` 系に写像）。
- redirect: 301/302/303/307/308 + `Location` を
  request URL に対し resolve。scheme は `http` のみ
  （`https` 行先は `UnsupportedCapability` — TLS 非保有、
  http への downgrade も拒否）、`max_redirects` 上限、
  ループ検出（同一 URL の繰返し）。**cross-origin
  （scheme/host/port いずれか相違）では `Authorization`
  を付けない** — same-origin のみ転送。PROPFIND/GET とも
  redirect follow（303 は GET 化 — 本 provider は GET/HEAD/
  PROPFIND のみで body を持つのは PROPFIND;303 で
  PROPFIND→GET 化は semantics が変わるため 303 は
  follow せず typed error）。

## 6. 認証（auth）

- `WebDavConfig.credential_key` → 各 provider call で
  `cx.credentials.resolve(key)` を短命解決
  （resolver 未設定で key あり → `AuthExpired`）。
  `OpaqueCredential.context()` を **`Authorization` header
  の field-value としてそのまま**使う（`"Basic <b64>"`、
  `"Bearer <t>"` 等 — provider は scheme を parse しない）。
- resolve 結果は call 内でのみ保持し、戻り値・error・
  locator・`WebDavProviderInfo`・provenance に書かない
  （`credential_configured: bool` のみ公開）。
- 401→`AuthExpired`(`AfterAuth`)、403→`PermissionDenied`、
  407→`AuthExpired`。WWW-Authenticate の challenge 種別で
  再試行ロジックは持たない（opaque 転送契約 — Digest 等の
  handshake は app 層/resolver の責務）。

## 7. ETag / pin（dav.rs + mod.rs）

- `getetag` 値の分類: `W/` prefix → weak（pin 不可）。
  それ以外で `"..."` quoted-string かつ内部文字が
  visible ASCII（0x21,0x23-0x7E — space・`"`・CTL・
  obs-text を含まない）→ strong validator 候補。
  それ以外 → unusable（pin 不可）。validator は header
  injection 対策としてこの grammar で fail-closed 検証し、
  `If-Match` にそのまま載せる。
- `pin`: locator→path→PROPFIND Depth:0→先頭 response の
  prop を採用（href を decode→normalize して要求 path と
  一致検証 — 不一致は `Internal`）。`resourcetype` に
  collection → `InvalidRequest`（container は pin 不可）。
  strong ETag あり → `PinProof::ConditionalStrong{
  StrongValidator{validator, IfMatch}}`、
  `object_identity="webdav-resource:<path>"`、
  `expected_size`=getcontentlength（あれば）、
  `digest_hint=None`。weak/なし → `UnsupportedCapability`
  （fail-closed — mutable object の正式 Import を通さない）。
  `getcontentlength` > `max_bytes` → `BudgetExceeded`。
- `open_pinned`: `ConditionalStrong{StrongValidator{
  precondition: IfMatch}}` のみ受理（他 proof/他
  precondition/foreign provider → `InvalidRequest`）。
  locator→path を再検証（confinement）→ `GET` +
  `If-Match: <validator>`。status 処理:
  - 200: 応答 `ETag` header が pin validator と完全一致
    することを確認（trim OWS のみ正規化）— 不一致・
    欠落は `SourceChanged` で fail-closed（close body、
    byte を deliver しない）。一致なら body を
    `VerifyingReader{expected_digest: None,
    expected_size: pin.expected_size, max_bytes, cancel,
    deadline}` で包み `VerificationPolicy::
    IdentityBound{scheme:"webdav-if-match"}`。
  - 412→`PreconditionFailed`、409→`SourceChanged`、
    404→`RevisionGone`（pin 対象が消えた）、401/403→
    auth 系、その他は §9 の status 表。
- `open_preview`: asset→path→`GET`（Range あれば
  `Range: bytes=<offset>-<offset+len-1>`）。206→range
  stream、200+Range 要求→server が Range を無視した
  ため `offset` まで bounded discard（読んだ byte も
  `max_bytes` に計上、超過で `BudgetExceeded`）後
  `range.len` まで `take`。Range なし→先頭から
  `max_bytes+1` まで。`VerificationPolicy::Unverified`、
  `expected_size=None`（preview は未検証観測）。

## 8. browse/stat（dav.rs + mod.rs）

- `browse`: scope→path（Root=`root_path`、Locator=検証済み
  path）→ PROPFIND Depth:1 → multistatus を incremental
  parse。各 `<response>` から href・prop（最初の 2xx
  propstat）を取り、collection 判定・facts を構成。
  - 収集上限: `max_entries` response または PROPFIND body
    byte cap（`MAX_PROPFIND_BYTES`=256MiB、const）または
    `</multistatus>` で打ち切り。打ち切り時は
    `truncated=true` → `Coverage::Partial`。
  - href→locator 変換失敗（grammar 外・root 外・非 UTF-8）
    は entry skip + `unaddressable` 計数 → `Partial`。
  - 自分自身の response（href == 要求 collection）は
    entry に含めない。要求 path が collection でない
    （resourcetype に collection なし）→ `InvalidRequest`。
  - sort は `sort_entries`（local/git と同一規則）、
    filter は収集後に適用（server-side 検索なし）。
  - paging: 収集集合に対し offset cursor
    （ALIB-01 `CursorPayload`、epoch は browse 毎に新規
    — server snapshot がないため epoch 概念を持たず
    `epoch=0` 固定で scope/query/principal 束縛のみ有効。
    同一 query の連続 page は同じ PROPFIND を再発行する
    — server 側変更で window が変わり得ることを
    `Coverage::Partial` と docs で明示）。
  - `next_cursor`: 収集集合内に未 emit がある時のみ。
    truncated で収集 window を使い切った後は cursor を
    出さない（Partial のまま — 全件列挙を偽装しない）。
- `stat`: path→PROPFIND Depth:0→先頭 response。
  `byte_len`=getcontentlength（collection/欠落→
  `Unknown{NotApplicable|NotProvided}`）、`modified`=
  getlastmodified の HTTP-date parse（失敗→
  `Unknown{Other}`）、`kind`=collection→Directory else
  File、`checksum_hint`=`Unknown{NotProvided}`（ETag は
  digest ではない）、media=`all_unknown`。
  404→`NotFound`（auth 系は潰さない）。
- `search`: default `UnsupportedCapability{RequiresIndex}`。

## 9. status → ProviderErrorKind 対応表

| status | kind | retry |
| --- | --- | --- |
| 200/206/207 | ok | — |
| 301/302/307/308 | redirect（policy 付き follow） | — |
| 303 | `InvalidRequest`（PROPFIND semantics 変更を拒否） | Never |
| 400 | `InvalidRequest` | Never |
| 401/407 | `AuthExpired` | AfterAuth |
| 403 | `PermissionDenied` | Never |
| 404 | `NotFound` | Never |
| 405 | `UnsupportedCapability` | Never |
| 409 | `SourceChanged` | AfterUserRePin |
| 412 | `PreconditionFailed` | AfterUserRePin |
| 416 | `InvalidRequest` | Never |
| 429 | `RateLimited` | After(1s) |
| 500/502/504 | `TransientIo` | After |
| 503 | `Offline` | After |
| その他 | `TransientIo` | After |

## 10. 検証計画（fixture）

`tests/common/webdav_fixture.rs`: `TcpListener` を
`127.0.0.1:0` に bind し、accept loop thread が request を
parse（request line・headers・Content-Length body）して
in-memory tree（`BTreeMap<path, Node>`）へ dispatch する。
`Connection: close` 前提の 1 request/connection。

- node: `File{bytes, etag: ETagKind, mtime}` /
  `Dir{children}`。ETagKind = `Strong(String)` /
  `Weak(String)` / `None`（server 毎に切替可）。
- メソッド: `PROPFIND`（Depth 0/1 → multistatus XML 生成、
  prop 絞り込みは要求 prop のみ返す）、`GET`（If-Match
  評価→412、Range→206/200、ETag/Content-Length/chunked
  応答 mode）、`HEAD`。
- 変種（per-path または server flag で有効化）:
  redirect（同一/別 origin fixture server への 302、
  `Location` 絶対/相対）、auth（`Authorization` 必須化・
  revoke flag・401+WWW-Authenticate）、chunked 応答、
  stall（response を遅延/停止 — client `io_timeout` で
  `TransientIo`）、mid-transfer disconnect（headers 後に
  socket close / 部分 body 後 close）、no-etag resource、
  weak-etag resource、Last-Modified のみ resource、大
  collection（`max_entries` 超の flat dir — 真の paging
  なし）、etag 変更（file bytes 差替→新 strong etag）。
- request 記録: method/path/headers/body を `Mutex<Vec>`
  に保持し、`If-Match`・`Range`・`Authorization` の wire
  証拠・cross-origin での auth 非転送を assert する。
- 2 台目 fixture（redirect 先）で cross-origin auth strip
  を検証。TLS/実 network は fixture に含めない
  （NotRun/Unsupported）。

`tests/webdav.rs`: ALIB-01 contract と同じ
browse→stat→pin→open_pinned→bytes の列 + fixture 固有
failure 分類・redaction・import_verified 接続・cancel/
timeout/disconnect・coverage/budget を検証する。

## 11. 非対象の明示

- https/TLS・実 WebDAV server・実 network・OS credential
  store・GUI・auth handshake（Digest 等）: capability/info
  で区別し NotRun/Unsupported。
- `synvelis-app` 配線・catalog/preview cache 連携の app 側
  組立: ALIB-03 の cache が principal 単位で既に分離済み。
  本 slice は provider のみ。
- Ruviform Generic HTTP/File/FS/Secret: 重複実装しない。
  本 transport は WebDAV 固有の最小実装であり、他
  Consumer（VCS-03 等）が HTTP を必要とした時点で基盤
  Issue として再評価する。
