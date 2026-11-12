---
id: "IMPL-ALIB04LfsRemoteFetch"
type: "impl"
status: "completed"
title: "ALIB-04 LFS Remote Fetch — Git LFS batch API / HTTP download 実装記録"
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
  implements:
    - "docs/plans/alib-04-lfs-remote-fetch-plan.md"
    - "docs/designs/alib-04-lfs-remote-fetch-design.md"
    - "docs/specs/alib-04-lfs-remote-fetch.md"
    - "docs/tasks/alib-04-lfs-remote-fetch-task.md"
    - "docs/test-specs/alib-04-lfs-remote-fetch.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/alib-04-git-remote-provider.md"
    - "docs/impl/ir/alib-06-webdav-provider.md"
    - "docs/impl/ir/alib-07-fault-matrix.md"
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

# ALIB-04 LFS Remote Fetch — 実装記録

## Summary

Issue #331「Git remote LFS fetch — LFS batch API / HTTP download
endpoint による実Byte取得」の実装。`LfsBackend` に
`RemoteHttp` variant を追加し、`http://` Git remote の LFS pointer
を Git LFS batch API(`POST <endpoint>/objects/batch`)と download
action の `GET` 経由で実バイト列として解決する。取得 stream は
従来どおり SHA-256 `VerifyingReader`（oid = content の SHA-256）
と pointer の declared size を通過した場合のみ asset bytes に
なる。`file://` の local `lfs/objects` 経路は `open_local` として
無変更で温存し、`https://`（TLS stack 非保有）・SSH・`git://` は
socket を開かず typed `UnsupportedCapability` を返す。新規外部
crate 依存は追加せず、ALIB-06 で導入した `webdav::http` の最小
blocking HTTP/1.1 transport を `pub(crate)` の共有層として再利用
した（query 対応と `HttpEndpoint` 分離のみの内部一般化。WebDAV
公開 API・wire 挙動は不変）。

## Context

- Issue: scarecrow173/synvelis#331（親 consumer: #91 ALIB-07
  fault matrix lane LIB-PIN-02 / ALIB-REAL-03。前提実装: #88
  ALIB-04 local `file://` LFS、#90 ALIB-06 WebDAV transport、
  #94 VCS-03 transport 再利用方針）。#91 の fault matrix 判定行・
  test-spec は本 slice では変更しない（別 Issue 所有）。
- 前提: `scarecrow173/sv331-lfs-fetch` branch（`origin/main`
  最新から）。並行 worker（sv330/sv106/sv125/sv112）が稼働中の
  ため占有範囲外のファイルは未変更。
- 全検証は in-process `TcpListener` 製 `MockServer`
  （`tests/common/webdav_fixture.rs`）と local `git clone
  --mirror` seed mirror で実施。git network 操作・実 remote・
  HTTPS・SSH・TLS・実 OS credential store・GUI は NotRun/
  Unsupported — 外部 gate であり Pass を主張しない。

## Implementation

### Changes

- `src/webdav/http.rs`: 共有 transport の内部一般化のみ。
  `HttpRequest` に `query: Option<String>` を追加（server の
  spelling を verbatim 送出 — 再 encode は署名 token を壊すため
  request line 直前で control/`#` を拒否する fail-closed 検査を
  追加）。`HttpEndpoint{host,port,ipv6}` を新設し、
  `send_endpoint(&HttpEndpoint, …)` を `send` から分離 —
  WebDAVUrl 以外の HTTP 呼出側（Git LFS）が同じ wire 挙動
  （1 request/connection、`Connection: close`、cancel/deadline/
  stall guard）を再利用できる。`send` は `HttpEndpoint::from(url)`
  経由で従来挙動を完全保持。`https` は endpoint 構築時点で
  排除される設計（transport に TLS がない）。
- `src/webdav/mod.rs`: `http`/`locator`/`url` を `pub(crate)` 化
  （crate 内共有層としての公開。外部公開 API は不変）。
  `HttpRequest` 構築箇所に `query: None` を追加。
- `src/git/lfs.rs`: `LfsBackend` に `RemoteHttp { remote,
  credential_key }` variant と `LfsBackendKind::RemoteHttp` を追加。
  - backend 選択: `remote.lfs_objects_root()`（`file://`）は従来
    どおり `Local`、`GitRemoteScheme::Http` は `RemoteHttp`、
    それ以外（`https`/`ssh`/`git`/`Other`）は `Unsupported`。
  - endpoint 解決: git-lfs server discovery の clone-URL 規則 —
    remote path が `.git` 終端なら `<path>/info/lfs`、それ以外は
    `<path>.git/info/lfs` を endpoint とし `/objects/batch` を
    POST 先とする。`lfs.url`/`remote.*.lfsurl`/`.lfsconfig`
    override は provider に repository-config 供給源がないため
    参照しない（設計書に根拠記録）。
  - batch request: `POST`、`Accept`/`Content-Type` とも
    `application/vnd.git-lfs+json`、body は
    `{"operation":"download","transfers":["basic"],"objects":[
    {"oid","size"}]}`。response body は 1 MiB cap で bounded read、
    serde で `hash_algo`/`objects[].{oid,size,actions.download.
    {href,headers},error.{code}}` を parse（`message` は server
    free text が署名 URL 等を含み得るため model 化せず error
    detail にも出さない）。`hash_algo` が `sha256` 以外なら
    `UnsupportedCapability`。
  - object 解決: 応答 `objects` から要求 `oid` を検索し、
    per-object `error.code` を size/action 整合チェックより先に
    authoritative として処理（HTTP status と同じ番号体系で
    `status_error` に写像）。`size` 不一致・`actions.download`
    欠落は `InvalidRequest`。`expires_in`/`expires_at` は助言的
    metadata として扱い href をそのまま使う（期限切れは GET の
    typed HTTP error として顕在化、refresh/rewrite しない）。
  - download href 解決: `http(s)://` 絶対 URL・protocol-relative
    `//host/…`・絶対 path・relative reference を受理。`https`
    と非 HTTP scheme は `UnsupportedCapability`（socket を開か
    ない）。relative は remote origin root `/` 基準で解決。
    fragment は送信しない。path は厳密 percent-decode、
    query は raw のまま保持。
  - download GET: action `headers` を string value のみ verbatim
    付与（wire 境界で header name/value を再検証し不正なものは
    drop）。redirect は最大 `MAX_REDIRECTS` まで follow、
    303 および POST への 301/302 は GET に rewrite、307/308 は
    method/body 保持。redirect chain の各 hop で https を
    `UnsupportedCapability` として拒否。`Content-Length` が
    declared size と不一致なら streaming 前に
    `IntegrityMismatch`（宣言長がある場合のみの early check —
    EOF の SHA-256 `VerifyingReader` が authoritative）。
  - credential 境界: `credential_key` は fetch 毎に
    `ProviderContext.credentials` から解決し `Authorization`
    header 値としてのみ使用。付与は target origin が git remote
    origin と一致する場合のみ（署名付き cross-origin CDN href
    には絶対に付かない）。action-supplied `authorization` は
    上書きしない。URL・error detail・batch body に secret を
    出さない。
  - typed errors: batch/download の HTTP status は
    `status_error` で写像 — 401→`AuthExpired`、403→
    `PermissionDenied`、404→`NotFound`、405→
    `UnsupportedCapability`、409/5xx→`TransientIo`、
    412/416→`PreconditionFailed`、429→`RateLimited`、その他→
    `InvalidRequest`。transport 失敗は `webdav::http` 既存の
    `Offline`/`TransientIo`/`Cancelled`/`BudgetExceeded` 分類を
    そのまま継承。oid 形式不正・batch JSON 不正・size/action
    不整合は `InvalidRequest`。pointer が宣言しない
    `expected_size=None` の remote fetch は `InvalidRequest`
    （batch API の `(oid,size)` pair が必須のため）。
- `src/git/mod.rs`: `LfsBackend::for_remote` に
  `credential_key` を受渡。`open_identity` の LFS 分岐は戻り値を
  `Box<dyn Read + Send>` に適合（`File` → stream 一般化、
  `StreamVerified` policy と digest 派生は不変）。
- `tests/lfs.rs`: 新規 11 test。`common::webdav_fixture::
  MockServer`（in-process `TcpListener`）が batch POST と
  download GET を scripted response で返し、handler は `Host`
  header から署名付き href を組み立てる。git objects は
  `git clone --mirror` で seed した local mirror から供給
  （`reference="main"` で `ls-remote` を回避し git network
  操作を排除）。
  - `remote_lfs_batch_download_fetches_and_verifies`: happy
    path。request trace で `POST /repo.git/info/lfs/objects/batch`
    → `GET /objects/<oid>?sig=test-token`（署名 query verbatim）
    を確認し、返却 bytes が fixture content と一致、
    `receipt.verified==true`・`bytes_read==size`。
  - `remote_lfs_batch_404_is_not_found` / `…_object_error_404…`:
    batch endpoint 404 と batch 200+per-object error.code=404 の
    両経路で `NotFound`。
  - `remote_lfs_batch_401_is_auth_expired` /
    `remote_lfs_download_403_is_permission_denied`: auth 系 typed
    error。
  - `remote_lfs_download_size_mismatch_is_integrity`: Content-
    Length 短い body で streaming 前 `IntegrityMismatch`。
  - `remote_lfs_corrupt_bytes_fail_sha256_at_eof`: 同 length の
    corrupt bytes が EOF で SHA-256 不一致、completion slot に
    `IntegrityMismatch`。
  - `remote_lfs_https_remote_is_unsupported`: `https://` remote
    で `LfsBackendKind::Unsupported` + open が
    `UnsupportedCapability`（socket 非接続 — port 9 の dead
    address でも応答は typed error のみ）。
  - `remote_lfs_malformed_batch_is_typed_not_panic`: 非 JSON
    batch body が `InvalidRequest`。
  - `remote_lfs_credentials_stay_on_remote_origin`: credential
    が batch endpoint origin にのみ付き、別 origin の署名付き
    href には付かないことを 2 listener で検証。
  - `file_remote_lfs_still_reads_local_objects`: `file://`
    regression — `LocalObjects` backend で local `lfs/objects`
    から fetch・検証成功。

### Decisions During Implementation

- batch response の object 照合は request 順ではなく `oid` 一致
  検索（spec は順序を保証しない）。複数 object batch は本 slice
  では単一 object request のみ発行するが、parse は複数 object
  応答を許容する。
- `transfers:["basic"]` を request に含め、custom transfer
  （`lfs-standalone-file` 等）の negotiation を行わない —
  `actions.download` が返らない server は `InvalidRequest` で
  fail-closed。
- relative download href は batch endpoint path 基準ではなく
  remote origin root `/` 基準で解決。batch API path は API
  endpoint であって resource namespace ではなく、server が
  resource 相対 href を発行する場合の解釈を一意にするため
  （設計書の代替案比較参照）。
- action-supplied headers は「server が要求する request header」
  として verbatim 送出するが、wire 境界で header 形式を再検証
  し CR/LF・空白・非 ASCII を含むものは drop — request
  smuggling 経路を塞ぐ。
- redirect follow は WebDAV の same-origin 制限を適用せず、
  origin を跨いだ follow を許可する（署名付き CDN redirect は
  LFS の正常系）が、credential 付与は常に remote origin 一致時
  のみという不変条件で漏洩を防ぐ。`https` redirect target は
  `UnsupportedCapability`。
- `Content-Length` early check は declared size との一致のみ
  検査し、`content-length` 欠落・chunked は streaming verifier
  に委譲（early check を「唯一の size 保証」にしない）。

## Related Experiments

### Adopted

なし（本 slice は実験を伴わない provider 実装作業。
`metadata.experiments` は adopted/rejected とも空で、
`docs/impl/exp/` に関連する実験ログは存在しない）。

### Rejected

なし。

## Validation

- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `check-app-native` / `lint-app-native` / `test-msrv` / `build`:
  全て実走 pass。`test`・`test-msrv` は workspace 全 target を
  実行し、`tests/lfs.rs` 11/11・`tests/git.rs` 28/28（file://
  LFS regression 含む）を含め全 green。`test-msrv` は rust
  1.89.0 toolchain で全 suite を実走。
- `cargo test -p synvelis-library --test lfs`: 11/11 pass
  （in-process local fixture、実 network 不使用）。
- NotRun/Unsupported: 実 remote（GitHub LFS 等）・`https://`
  （TLS stack 非保有 — `UnsupportedCapability` が仕様）・SSH・
  `git://`（LFS HTTP transport/auth 経路なし）・`.lfsconfig`/
  `lfs.url` override 解決・custom transfer negotiation・実 OS
  credential store・GUI。これらは外部 gate であり本 slice の
  Pass 対象外 — Issue #331 および #91 fault matrix の該当行は
  別途所有される。

## Risks

- **`transfers:["basic"]` 固定の fail-closed**: custom transfer
  （`lfs-standalone-file` 等）の negotiation を行わないため、
  `actions.download` を返さない server は `InvalidRequest` で
  fail-closed になる。basic 以外を要求する LFS server では
  remote fetch が機能しない。
- **relative href の解釈は origin root `/` 基準**: batch API
  path は API endpoint であって resource namespace ではない
  という解釈に基づく。server が別基準の relative href を発行
  する場合の解決結果は設計上の前提に依存する。
- **`expires_in`/`expires_at` は助言的 metadata**: href の期限
  切れ refresh/rewrite は行わず、期限切れは GET の typed HTTP
  error として顕在化する。長時間 pin した asset の再 open で
  署名 URL 期限切れが `TransientIo`/typed error として現れ得る。
- **credential は remote origin 一致時のみ付与**: 署名付き
  cross-origin CDN href には絶対に credential が付かない設計。
  CDN 側が独自 auth を要求する構成では download が失敗し得る
  （action-supplied `authorization` は上書きしない）。
- **action-supplied headers は wire 境界で再検証・drop**:
  CR/LF・空白・非 ASCII を含む header は drop されるため、
  それらを必須とする exotic server では request が成立しない
  可能性がある（request smuggling 経路を塞ぐための意図的な
  fail-closed）。

## Follow-ups

- 実 remote（GitHub LFS 等）・`https://`（TLS stack 非保有）・
  SSH・`git://`: NotRun/Unsupported。Issue #331 および #91
  fault matrix の該当行が別途所有する。
- `.lfsconfig`/`lfs.url`/`remote.*.lfsurl` override 解決・
  custom transfer negotiation: provider に repository-config
  供給源がないため本 slice 対象外（設計書に根拠記録）。
- 実 OS credential store・GUI: 外部 gate。
- LIB-PIN-02 remote LFS 行の fault matrix 判定は #91 側で
  更新される（`docs/impl/ir/alib-07-fault-matrix.md` の
  NotRun 記述どおり capability 宣言上 `Unsupported`/`NotRun`
  のまま）。
