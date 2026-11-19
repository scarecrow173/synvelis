---
id: "DESIGN-alib04-lfs-remote-fetch"
type: "design"
status: "approved"
title: "ALIB-04 LFS Remote Fetch — 設計（endpoint 解決・batch API・型付きエラー・credential 境界）"
created: "2026-11-10"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/331"
    - "https://github.com/scarecrow173/synvelis/issues/521"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-04-lfs-remote-fetch.md"
  implemented-by:
    - "docs/impl/ir/alib-04-lfs-remote-fetch.md"
  depends-on:
    - "docs/specs/alib-04-git-remote-provider.md"
    - "docs/designs/alib-04-git-remote-provider-design.md"
    - "docs/designs/alib-06-webdav-provider-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/alib-04-lfs-remote-fetch.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/git-lfs/git-lfs/blob/main/docs/api/batch.md"
    - "https://github.com/git-lfs/git-lfs/blob/main/docs/api/discovery.md"
  defers: []
  deferred-by: []
---

# ALIB-04 LFS Remote Fetch — 設計

## Overview

`LfsBackend`（`src/git/lfs.rs`）に `RemoteHttp` variant を追加し、
`http://`/`https://` remote の LFS object を Git LFS Batch API
経由で取得する。HTTP transport は WebDAV provider の
`src/webdav/http.rs` を内部共有する最小一般化で賄う。TLS は
`scarecrow173/synvelis#521` slice で rustls（ring provider、
TLS 1.2/1.3、検証必須・silent downgrade なし）として追加され、
`https` endpoint は `HttpEndpoint.tls` の `TlsSession` 経由で
検証付き接続する。プロキシは導入しない。検証の権威は従来
どおり `digest.rs` の `VerifyingReader`（SHA-256=oid +
expected size を EOF で照合）にあり、本 slice は「正しい
オブジェクトのストリームを返す」ことだけを責務とする。

## Endpoint 解決規則

正本: git-lfs `endpointGitFinder.NewEndpointFromCloneURL`
（`lfsapi/endpoint_finder.go`）と `docs/api/discovery.md`。

1. remote URL は `GitRemoteUrl` が credential-free で検証済み。
   `http://`/`https://` が対象（`for_remote` で
   `GitRemoteScheme::Http | GitRemoteScheme::Https` のとき
   `RemoteHttp` を選択。#521 slice で https を追加）。
2. URL の path 部を percent-decode し、末尾 `/` を除去した上で:
   - path が `.git` で終わる → `<path>/info/lfs`
   - それ以外 → `<path>.git/info/lfs`
   - path が空/root のみ → `InvalidRequest`（endpoint 導出不能）
3. batch target = `<endpoint>/objects/batch`。
4. `lfs.url` / `remote.<name>.lfsurl` / `.lfsconfig` の override は
   **未対応として記録**: 本 provider は remote を明示設定で受け取り
   repository config の source を持たない。clone-URL 規則のみが
   正直な導出であり、override 対応は将来の config-source 導入時に
   行う（設計判断として明示）。

## Batch API

- `POST <endpoint>/objects/batch`
- headers: `Accept`/`Content-Type:
  application/vnd.git-lfs+json`、`User-Agent` は transport 既定。
- body: `{"operation":"download","transfers":["basic"],
  "objects":[{"oid":"<64hex>","size":<u64>}]}`
- `expected_size` は pointer の declared size。remote backend は
  batch object entry が `(oid,size)` の対であるため `None` を
  `InvalidRequest` で拒否する（local backend は従来どおり
  pre-check skip を許容）。
- response `hash_algo` が存在し `sha256` 以外 →
  `UnsupportedCapability`（本 provider は SHA-256 のみ検証可能）。
- `objects[]` から要求 oid に一致する entry を検索（順序非依存）。
  `size` 不一致・entry 欠落・`actions.download` 欠落・JSON 不正・
  body 1MiB 超過 → `InvalidRequest`（決定的なサーバ契約違反、
  retry 無意味）。
- per-object `error.code` は HTTP status と同じ写像で型付けし、
  `message` 本文は error detail に含めない（署名 URL 等の secret
  を含み得るため）。
- `expires_in`/`expires_at` は助言的メタデータとして無視する。
  href は供給されたまま即時使用し、期限切れは GET の型付き
  HTTP エラーとして現れる。refresh・書換・bypass はしない。

## Download GET

- `actions.download.href` を `UrlTarget` に解決:
  絶対 `http(s)://` URL、`//host/…`（protocol-relative は
  remote の scheme を継承 — https remote での silent
  downgrade を防ぐ）、絶対 path（remote origin
  相対）、相対参照（origin root 相対）を受理。fragment は送信
  しない。`https` href は #521 slice の TLS transport で
  検証付き接続される。非 HTTP scheme →
  `UnsupportedCapability`。redirect の `Location` も同規則で、
  protocol-relative は現在 target の scheme を継承する。
- `actions.download.headers`（文字列値のみ）は verbatim で付与
  し、wire 境界で header 妥当性を再検証する。
- redirect は `MAX_REDIRECTS`(=4) まで追従。`https` 遷移先も
  同一の TLS transport で follow する（scheme は target のものを
  使い、暗黙の downgrade/upgrade はしない）。303 と POST への
  301/302 は GET 化、307/308 は method/body を保持。
- `Content-Length` が declared size と不一致なら open 時に
  `IntegrityMismatch`（早期検出）。宣言なし・chunked・
  close-delimited でも EOF の `VerifyingReader` が権威的に検証
  するため取りこぼしはない。
- 返り値は `HttpResponse.body`（`Box<dyn Read + Send>`）を
  そのまま provider の `VerifyingReader` へ渡す。全量 buffer は
  行わない。

## Credential 境界

- `GitRemoteConfig.credential_key` を `LfsBackend::for_remote` に
  渡して保持し、fetch 毎に `cx.credentials` で解決
  （WebDAV `auth_value` と同一契約: key あり・resolver なし →
  `AuthExpired`、context が header 値として不正 → `AuthExpired`）。
- `Authorization` は **target origin == remote origin** のとき
  のみ付与。署名付き download href が別 origin（port 違いを含む）
  なら credential は送られない。action-supplied の
  `authorization` header が既にある場合は上書きしない。
- credential 値・署名 URL・query token はいかなる error detail
  にも含めない。

## HTTP transport の一般化

`webdav/http.rs` は provider 非依存の内部層とする:

- `HttpEndpoint { host, port, ipv6 }` を新設し、`send` は
  `WebDavUrl` → `HttpEndpoint` 変換の薄い wrapper に。
  `send_endpoint` が非 WebDAV caller（Git LFS）向けの公開
  （crate 内）入口。wire 挙動は完全に同一: 1 request/connection、
  `Connection: close`、cooperative cancel/deadline/stall guard、
  `Content-Length`/`chunked`/close-delimited body。
- `HttpRequest` に `query: Option<String>` を追加。既に
  percent-encode 済みの query を verbatim 送出する（署名 token
  を再 encode すると壊れる）。path は従来どおり decoded →
  `encode_path`。query に空白・制御文字・`#` があれば wire 直前で
  `Internal` 拒否。
- `webdav` の公開 API（`WebDavProvider`/`WebDavConfig`/
  `WebDavUrl` 等）は不変。`http`/`locator`/`url` module は
  `pub(crate)` 化のみ。
- TLS（#521 slice）: `HttpEndpoint` に `tls: Option<TlsSession>`
  を追加し、`tls: Some` の endpoint は rustls handshake を
  cooperative guard（`IO_POLL` 毎の cancel/deadline/stall 再確認）
  の下で行う `TlsConn` で接続する。trust root は
  `ProviderContext::tls: Option<TlsTrust>` が保持する — `None` は
  platform native root store（`rustls-native-certs`、process 内
  1 回 lazy 構築）、`Some` は `TlsTrust::from_pem` の明示 CA
  （private CA・fixture 用。検証強度は変えず trust anchor の
  選択のみ）。server name は IP literal → `ServerName::IpAddress`、
  その他 → DNS name で、rustls `ClientConnection::complete_io`
  を 1 record flight ずつ回す独自 `Read`/`Write` 実装により
  `WouldBlock` を guard へ伝播させる（`rustls::Stream` の
  `prepare_read` 内部 loop は guard を bypass し得るため不採用）。
  TLS handshake は application request 送信前に完了し、証明書
  検証失敗（`rustls::Error::InvalidCertificate`）は
  `ProviderErrorKind::TlsVerification`（`RetryAdvice::Never`）、
  その他の TLS/record エラーは `TransientIo` に分類する。
  WebDAV provider 自身の `send` は引き続き `https` を
  `UnsupportedCapability` で拒否する — TLS transport は
  endpoint opt-in の caller（Git LFS）のみが使い、WebDAV-over-
  HTTPS の解放は #90 の責務として残す。

## 型付きエラー写像

| 条件 | ProviderErrorKind |
| --- | --- |
| batch/download HTTP 401 | `AuthExpired` |
| 同 403 | `PermissionDenied` |
| 同 404 | `NotFound` |
| 同 405 | `UnsupportedCapability` |
| 同 409・5xx | `TransientIo` |
| 同 412・416 | `PreconditionFailed` |
| 同 429 | `RateLimited` |
| 同 その他非 2xx | `InvalidRequest` |
| per-object `error.code` | 同上の写像 |
| JSON 不正・必須 field 欠落・oid/size 不一致・download action 欠落・body 超過 | `InvalidRequest` |
| `hash_algo != sha256`・非 HTTP scheme | `UnsupportedCapability` |
| TLS 証明書検証失敗（#521 slice） | `TlsVerification` |
| その他の TLS handshake/record 失敗（#521 slice） | `TransientIo` |
| content-length ≠ declared size（早期）・EOF での size/digest 不一致 | `IntegrityMismatch`（EOF 側は `VerifyingReader`） |
| DNS/connect 失敗 | `Offline`（transport 既定） |
| 途中 stall・protocol 違反 | `TransientIo`（transport 既定） |
| cancel/deadline | `Cancelled`/`BudgetExceeded`（guard 既定） |

## 非対象（明示）

- SSH/`git://` LFS transport・SSH agent/credential 経路。
- WebDAV provider の `https` capability 解放（#90 の責務 —
  TLS transport は endpoint opt-in で LFS のみが使用）。
- 実 remote（GitHub LFS 等）・実 HTTPS endpoint での動作確認 —
  外部 gate、NotRun（#521 slice の fixture 検証は loopback の
  `MockTlsServer` のみ）。
- `.lfsconfig`/`lfs.url`/`remote.*.lfsurl` override、pure SSH
  protocol download、custom transfer adapter（`lfs-standalone-file`
  等）。`transfers` は `["basic"]` のみ申告し、download action の
  href GET が唯一の実装経路。
- #91 fault-matrix の判定行・test-spec — 別 Issue 所有、変更しない。
