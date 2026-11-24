# synvelis#633 — WebDAV 実 provider 検証環境（docker/compose）検証記録

対象 SHA（worktree HEAD）: `72316ee2`（branch `feature/633-webdav-docker-provider`、base `72316ee2b4901bea77c0ea8a6b3736afde8085d1` — 検証時点の origin/main と同一）

OS: Windows 11（`x86_64-pc-windows-msvc`、Git Bash + Windows 版 curl/schannel）、Docker Desktop 4.98.0（Compose v5.6.1）、記録日: 2026-11-25

## 構成の要約

- `docker-compose.webdav.yml` + `docker/webdav/` — Apache httpd `2.4.71-alpine` + `mod_dav`/`mod_ssl` を `synvelis-webdav-httpd:local` として build。`certgen`（`alpine/openssl:3.5.9`）が初回 up 時に私有 CA・サーバ証明書（SAN: localhost/127.0.0.1/::1）・`htpasswd`・foreign CA（negative check 用）を `docker/webdav/certs/`（gitignored）へ生成する。
- `docker/webdav-probe/` — root workspace から `[workspace]` で独立させた path 依存 crate。実 `synvelis_library::webdav::WebDavProvider` を `WebDavUrl::parse` + `WebDavConfig{root_path:/library}` + `TlsTrust::from_pem` + `CredentialResolver`（env 由来の `Authorization: Basic …`）で組み立てて実接続する。
- `docker/verify.sh` — `config -q` → `up -d --build` → curl contract checks（補助証拠）→ probe → `down` を一括で走らせる補助 lane（`mise.toml` は触れないため mise task 化していない）。

## Verification lanes（実測。transcript: `verify-transcript.txt`）

| lane | コマンド | exit code | 結果 |
| --- | --- | --- | --- |
| compose 構文 | `docker compose -f docker-compose.webdav.yml --env-file docker/.env config -q` | 0 | Pass |
| 起動 | `docker compose … up -d --build`（certgen → webdav） | 0 | Pass（certgen idempotent、httpd Listen 8443） |
| 未認証 PROPFIND | curl `-X PROPFIND`（credential なし） | — | Pass（`401`、Basic auth 強制を確認） |
| PROPFIND Depth:1 | curl `-X PROPFIND` on `/library` | — | Pass（`207` multistatus、全 response に `getetag`） |
| GET strong ETag | curl `GET /library/sample.txt` | — | Pass（`ETag: "d6-66108c0e124a2"` — weak `W/` なし） |
| If-Match（bogus） | curl `-H 'If-Match: "sv633-bogus-validator"'` | — | Pass（`412`） |
| If-Match（正しい validator） | curl `-H "If-Match: $ETAG"` | — | Pass（`200`） |
| Range | curl `-H "Range: bytes=0-4"` | — | Pass（`206` + `Content-Range`） |
| probe（実 provider） | `cargo run --manifest-path docker/webdav-probe/Cargo.toml` | 0 | Pass（下表） |
| 停止 | `docker compose … down` | 0 | Pass |
| verify.sh 全体 | `bash docker/verify.sh` | 0 | Pass（`verdict=PASS`） |

### probe（実 `WebDavProvider`）観測 — `probe-output.txt`

| 操作 | 観測 | 結果 |
| --- | --- | --- |
| `capabilities` | browse/stat/pin/range_read/preview=`Supported`、access=`ReadOnly` | Pass |
| `browse`（Root、`/library`） | 2 entries（`nested/` Container + `sample.txt` Asset）、`coverage=Complete` | Pass |
| `stat` `/library/sample.txt` | `availability=Available`、`byte_len=214`（origin=Provider） | Pass |
| `pin` | `ConditionalStrong`/`StrongValidator`、validator=`"d6-66108c0e124a2"`、precondition=`IfMatch`、`expected_size=214` | Pass |
| `open_pinned` | 214 bytes 読了、`policy=IdentityBound{webdav-etag-if-match}` | Pass |
| `open_preview` `Range 0..16` | 16 bytes | Pass |
| negative: 誤 credential | `browse` が `Err(kind=AuthExpired)` — server 401 を provider が型付きエラー化 | Pass |
| negative: foreign CA anchor | `browse` が `Err(kind=TlsVerification)` — `TlsTrust::from_pem` の pin が実効 | Pass |

## 補助 lane（canonical = mise ではない）

| lane | コマンド | exit code | 結果 |
| --- | --- | --- | --- |
| probe fmt | `cargo fmt --manifest-path docker/webdav-probe/Cargo.toml` | 0 | Pass |
| probe lockfile | probe 側 `Cargo.lock` の ruviform rev を root lock と同一 `cd4213a1` に `cargo update --precise` で揃えて追跡対象化 | — | 再現性のため lockfile をコミット |

## 発見した不具合（環境側で workaround、provider 修正は範囲外）

実 Apache への接続で、`browse` が `TransientIo: too many redirects` で失敗する現象を確認した。原因は provider 側の redirect 解決にある:

- provider は collection を canonical path `/library`（trailing slash なし）で PROPFIND する。
- Apache `mod_dir` は canonical 形 `/library/` への `301` を返す（DAV server の標準的な挙動）。
- provider の `resolve_redirect`（`crates/synvelis-library/src/webdav/mod.rs:306-362`）は redirect 先を `normalize_path` に通すが、同関数は trailing slash を除去する（`locator.rs:46-49`）。結果 `/library/` → `/library` に戻り、同一 301 が `MAX_REDIRECTS`(4) まで繰り返される。

locator の canonical 形と wire 上の request path を同じ正規化で扱っていることが原因であり、scripted mock では再現しなかった実サーバ特有の経路である。本 slice は `crates/` を編集範囲に含まないため、`webdav.conf` で `DirectorySlash Off` を指定して slashless PROPFIND へ `207` を直接返させる workaround を採った（コメントに理由を記録）。provider 側の修正は `scarecrow173/synvelis#634` として起票した。

## 外部 gate（Pass 偽装しない）

| 項目 | 状態 | 後続 |
| --- | --- | --- |
| fault matrix（`docs/test-specs/alib-07-fault-matrix.md`）「実 WebDAV server は NotRun」行の実走・verdict 更新 | NotRun — 本 slice は環境と手順（`docs/specs/alib-07-webdav-real-run.md`）の提供まで | 後続 gate slice |
| slash-redirect を返す stock 設定（`DirectorySlash On`）の server での動作 | Fail 確定（上記 provider 不具合）— 修正後に再検証 | `scarecrow173/synvelis#634` |
| Linux/macOS での lane 実走 | NotRun — 当環境は Windows のみ | 必要になった時点の実測 |
| CI 組込み | NotRun — ローカル/opt-in lane のみ | 別判断 |
