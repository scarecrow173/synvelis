---
id: "SPEC-alib06-webdav-provider"
type: "spec"
status: "approved"
title: "ALIB-06 — WebDAV File-Server Provider（strong ETag/If-Match pin・conditional read・auth・cancel）（synvelis-library slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/90"
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/85"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/87"
    - "https://github.com/scarecrow173/synvelis/issues/88"
    - "https://github.com/scarecrow173/synvelis/issues/91"
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
  implements: []
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

# ALIB-06 — WebDAV File-Server Provider（synvelis-library slice）

## What Should Be Built

`synvelis-library` に、ALIB-01 の `AssetProvider` 契約を実装する
**Git 以外の最初の Remote Provider** `WebDavProvider` を追加する。
WebDAV File Server の collection を Metadata-first（PROPFIND）で
browse/stat し、**strong ETag + `If-Match` conditional read** を
pin/fetch の証拠として使う。pin→`open_pinned`→ALIB-02
`import_verified` で Verified Import へ供給する。

対象は library 契約と実装のみ。検証は **Local test WebDAV/HTTP
fixture**（`std::net` の in-process server、実装経路の証拠に
なる）で行い、実 WebDAV Server・実 network・実 OS credential
store・TLS・GUI は本 slice では実装しない — capability として
正直に宣言し、NotRun/Unsupported を Pass にしない。

SMB/NAS・SVN・P4・S3 は将来独立 Adapter であり、本 Issue は
WebDAV/File Server adapter のみを約束する。

## Why Now

ALIB-01（#85）provider 境界、ALIB-02（#86）verified import、
ALIB-03（#87）catalog/preview cache、ALIB-04（#88）Git Remote
Provider が main に揃った。Issue #90 は **Git 以外の Provider
能力差を実証する** adapter であり、#91 の failure matrix の
WebDAV/ETag/If-Match/権限 consumer でもある。Git（immutable
object）とは異なり WebDAV resource は mutable であり、
strong validator による conditional pin が唯一の強い固定手段
である — この差異こそが本 slice の検証対象。

## Users and Value

- **後続 slice の実装者**（#89 GUI Import・#91 Fault Matrix）:
  `AssetProvider` 越しに WebDAV asset を pin→import できる。
  `WebDavProviderInfo` で scheme/TLS/認証設定の capability
  証拠を取得できる。
- **Project 利用者**: NAS/ファイルサーバ上の素材を browse し、
  strong ETag を持つ resource のみ安全に pin→Verified Import
  できる。weak ETag/Last-Modified のみの resource は fail-closed
  で Unsupported となり、可変 object を黙って import しない。
- **セキュリティレビュア**: credential が URL/locator/
  provenance/log/error detail に残らないこと、cross-origin
  redirect で `Authorization` を転送しないこと、path
  confinement を test で確認できる。

## Scope

### In Scope

- `WebDavProvider`（`AssetProvider` 実装、
  `ProviderKind::WebDav`、`AccessMode::ReadOnly`）。
- `WebDavConfig`/`WebDavUrl`: base URL 検証（scheme allowlist
  `http`/`https`、userinfo・query・fragment 拒否、host/port
  grammar、IDN 非対応は拒否）、`root_path` confinement
  （config の root subtree 外の locator を拒否）、
  `credential_key`（`CredentialResolver` 経由の短命解決、
  `OpaqueCredential.context()` を `Authorization` header
  value としてのみ解釈 — provider は secret を parse/log
  しない）。
- HTTP/1.1 transport（`std::net::TcpStream`、blocking・
  non-RT）: origin-form request、`Content-Length` /
  `Transfer-Encoding: chunked` / close-delimited body、
  connect/read/write timeout、socket stall 中の
  cancel/deadline poll、redirect 上限と policy。
  `https` は TLS stack 非保有のため typed
  `UnsupportedCapability`（黙って http へ落とさない）。
- PROPFIND（Depth:0/1、`text/xml` body）による
  Metadata-first browse/stat。`multistatus` の最小 XML
  pull parser（entity/CDATA/comment/DOCTYPE/namespace
  prefix 対応、未知要素は読み飛ばし）。
- browse: Depth:1 collection 列挙。WebDAV に真の server
  paging はないため、`max_entries`・PROPFIND body byte
  cap・`max_depth` で **bounded listing** とし、打ち切り時は
  `Coverage::Partial` + 収集済み window 内の offset cursor
  のみ発行する。全件列挙したように見せない — 保証範囲は
  「server 応答順の先頭最大 `max_entries` 件を決定的 sort
  した窓」であり、それ以降は scope を絞る必要があることを
  docs/coverage で明示する。
- stat: Depth:0 PROPFIND → `EntryFacts`（`byte_len`=
  `getcontentlength`、`modified`=`getlastmodified` の
  HTTP-date parse、`kind`=`resourcetype` の collection
  判定、`checksum_hint` は ETag を digest 扱いしないため
  `Unknown`）。
- pin: Depth:0 PROPFIND → strong ETag（`W/` prefix なし、
  ASCII entity-tag grammar に適合）のみ
  `PinProof::ConditionalStrong{StrongRevisionToken::
  StrongValidator{validator, precondition: IfMatch}}` で
  pin。`expected_size`=`getcontentlength`、`digest_hint`=
  None（ETag は content digest ではない）。weak ETag・
  ETag なし・Last-Modified のみ・container・予算超過は
  fail-closed（`UnsupportedCapability`/`InvalidRequest`/
  `BudgetExceeded`）— 正式 Import を通さない。
- open_pinned: `GET` + `If-Match: <validator>` の conditional
  read。412→`PreconditionFailed`、409→`SourceChanged`、
  200 応答の `ETag` が pin validator と不一致→
  `SourceChanged`（fail-closed）、body は
  `VerifyingReader`（`expected_size` で byte 数照合、
  measured SHA-256 を receipt に記録、cancel/deadline/
  `max_bytes` を chunk 毎に確認）。`VerificationPolicy` は
  `IdentityBound{scheme:"webdav-if-match"}` — byte 結合は
  server-side precondition に由来し、strong digest は
  import 層が測定する。
- open_preview: `Range` header 付き GET。206→そのまま
  stream、200（server が Range 無視）→ `offset` までの
  bounded discard（`max_bytes` 計上）後 `len` まで、
  Range 要求時に `offset+len > max_bytes` →
  `BudgetExceeded`。明示的に `Unverified`。
- search: provider-side index 非保有のため default
  `UnsupportedCapability{RequiresIndex}`（偽の空結果を
  返さない）。
- `WebDavProviderInfo`: scheme・authority・root_path・
  TLS 可否・credential 設定有無の capability 証拠。
- WebDAV/HTTP fixture（`tests/common/webdav_fixture.rs`）:
  `TcpListener` の in-process server が in-memory tree で
  PROPFIND/GET/HEAD/redirect/auth/chunked/stall/disconnect/
  weak-etag/無 paging 大 collection を再現し、受信 request
  を記録して `If-Match`/`Range`/`Authorization` の wire
  証拠を検査する。実 server・実 network は使わない。
- docs チェーン（spec/design/plan/task/test-spec）。

### Out of Scope

- `synvelis-app`/`synvelis-workspace`/`synvelis-project`/
  `synvelis-recording`/`synvelis-player`/`synvelis-render`/
  `synvelis-gateway`/`synvelis-theme`/`recording-poc` への
  配線。Project Commit/Asset 正本との統合は #86 の公開
  契約（`import_verified`）で行い、本 slice は provider+pin
  の供給側のみ。
- 実 WebDAV Server・実 network・TLS（https）・実 OS
  credential store・Digest/Negotiate 等の auth scheme
  （`Authorization` header value は opaque に転送する
  のみ）・GUI。
- SMB/NAS・SVN・P4・S3 adapter、WebDAV locking/
  versioning（RFC 3253/4918 lock・`DAV:` version props）、
  `SEARCH` method、CalDAV/CardDAV。
- Ruviform Generic HTTP/File/FS/Secret Adapter の重複
  実装（汎用化が必要なら両 Consumer 互換評価後に基盤
  Issue へ還元 — 本 slice では `synvelis-library` 内に
  WebDAV 固有の最小 transport のみ実装）。
- RT callback からの HTTP/Network 呼出（provider は
  blocking worker 契約のまま — RT 層へは到達しない）。
- 実 OS・実デバイス・GUI・AudioLens/RT/Golden の変更
  （非回帰は既存 test で確認）。

## Requirements

- strong ETag を持つ Server からの選択 Download→Verified
  Import→Offline Project Playback が fixture で通る。
  条件不足 Server（weak ETag/ETag なし/Last-Modified のみ）
  は安全に Unsupported となり、可変 Object を勝手に
  import しない。
- `PinnedAssetRef` は immutable version id または strong
  ETag+取得時条件（`If-Match`）を持つ。weak validator・
  mtime・size 単独は pin 証拠にしない（ALIB-01 契約踏襲）。
  Source changed は 409/412・応答 ETag 不一致として意味
  付き typed error で返す。
- browse は Metadata-first（PROPFIND）で page/count 予算を
  持ち、paging 非対応 server では bounded listing/
  Partial/Coverage の保証範囲を偽らない。search は typed
  Unsupported。
- Range/Preview は provider 対応時のみ。Range 不可の全取得は
  事前 byte budget（`max_bytes`/`ProviderLimits`）で制限し、
  Transfer-Encoding/Content-Length/Compression/Redirect を
  検証する（`Content-Encoding` 非対応は明示拒否 — gzip 等を
  黙って decode しない）。URL/Path・TLS・SSRF・cross-domain
  redirect credential・認証秘密を制限する。
- 取得終了後に Local SHA-256（import 層測定）と実 Media
  decode を検証し、ETag を Digest 扱いしない。Project には
  Provider+固定条件+Byte Digest の証拠を残し、Credential
  は残さない。
- Connection/Auth Principal は Catalog/Preview Cache と
  連動（cursor は principal fingerprint に束縛）。認証失効・
  権限変更・offline・HTTP error を NotFound に隠さない
  （401→`AuthExpired`、403→`PermissionDenied`、5xx/
  切断→`TransientIo`/`Offline`）。
- 既存 ALIB-01/02/03/04 API・挙動を破壊しない（全て
  additive）。`Cargo.toml`/`Cargo.lock`/`mise.toml` は
  変更しない（依存追加なし — HTTP transport は `std::net`
  の最小実装、fixture も `std::net`）。

## Acceptance Criteria

- [ ] Local test WebDAV/HTTP fixture で strong/weak ETag、
  If-Match 412、Range 対応/非対応、ページングなし・大きな
  Collection、redirect、auth revoke、timeout、mid-transfer
  disconnect を再現する。
- [ ] 強い固定条件を満たす Server から選択 Download→
  Verified Import→Offline Project Playback。条件不足
  Server は安全に Unsupported となり、勝手な可変 Object
  Import はしない。
- [ ] 受入 Byte Budget/Cache 隔離・Credential redaction・
  URI/Path traversal・Cross OS 差・Cancellable nonRT での
  再生/録音非阻害を記録する。
- [ ] Ruviform 汎用 HTTP/File 関数を新設する必要が本当に
  ある場合のみ、両 Consumer 互換評価後に基盤 Issue を作る
  （本 slice では新設しない — WebDAV 固有の最小 transport
  で完結）。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`build`（+ 存在する app-native 系）が全て
  pass。既存 test の非退行。実 WebDAV Server/network・
  実 OS・GUI は NotRun/Unsupported として記録。

## Deferred Design Concerns

- 実 WebDAV Server（SabreDAV/Nextcloud/Windows IIS 等）・
  TLS・実 network 障害 | 本 slice は in-process fixture のみ
  実測。scheme/TLS/auth scheme は capability で区別し、
  未検証を Pass としない | 実 network を持つ検証 slice
  （#91 consumer 別判定へ伝達）| network 依存を fixture に
  持ち込まない。
- Digest/Negotiate/NTLM/Kerberos auth | `Authorization`
  header value を opaque に転送する契約のみ。OS credential
  store 連携は app 層の `CredentialResolver` 実装の責務 |
  実 auth が必要になった時点 | secret の parse を provider
  に持ち込まない。
- `Content-Encoding: gzip` 等 | 非対応を明示拒否（decode
  せず typed error）。必要なら Ruviform 圧縮基盤との
  Consumer 評価後 | 実 server が圧縮を返す事例が確認
  された時点 | YAGNI。
- WebDAV versioning/locking・`SEARCH` | Issue 対象外。
  props は displayname/resourcetype/getcontentlength/
  getlastmodified/getetag の最小集合 | 要件化された時点 |
  immutable version id は将来の pin 強化候補。
- 大 collection の真の paging | server 非対応のため
  bounded listing で代替。`max_entries` を超える collection
  は scope 絞り込みが必要 — 実測で問題化した時点に再検討 |
  YAGNI。

## Sources and Relations

- Issue #90（本 spec の直接の根拠）、#9（Asset Library 親・
  ALIB 詳細契約）、#85/ALIB-01、#86/ALIB-02、#87/ALIB-03、
  #88/ALIB-04、#91（Fault Matrix consumer）。
- `docs/specs/alib-01-asset-provider-contract.md`、
  `docs/specs/alib-02-streaming-verified-import.md`、
  `docs/specs/alib-03-catalog-preview-cache.md`、
  `docs/specs/alib-04-git-remote-provider.md`、対応する
  designs。
- RFC 4918（WebDAV）PROPFIND/multistatus/Depth、
  RFC 9110（HTTP semantics）ETag/If-Match/Range/Status、
  RFC 9112（HTTP/1.1）message format/chunked。
