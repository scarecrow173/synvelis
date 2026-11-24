# synvelis#634 — WebDAV redirect trailing slash ping-pong 修正の検証記録

対象 SHA（worktree HEAD）: `9480f07`（branch `scarecrow173/sv634-redirect-fix`。
構成: `f1a84a8` = failing test first、`9480f07` = fix。base は検証時点の
`origin/main` `3b614b64` と同一 — rebase/merge 不要を `git merge-base
--is-ancestor origin/main HEAD` で確認）

OS: Windows 11（`x86_64-pc-windows-msvc`、Git Bash + Windows 版 curl/schannel）、
Docker Engine 29.9.2（docker compose v2）、記録日: 2026-11-25

## 変更の要約

- `crates/synvelis-library/src/webdav/mod.rs` の `resolve_redirect` で、redirect
  `Location` の decoded path が `/` で終わる（かつ normalized 形が `/` 自体で
  ない）場合、confinement 判定は `normalize_path` の canonical 形に対して行い、
  返す wire path には trailing slash を復元する。RFC 4918 §5.2 の collection
  trailing-slash convention を wire 上で維持し、locator canonical grammar は
  変更しない。
- 同一 `(origin, path)` 再訪検出（design doc が謳う「ループ検出」）は未実装の
  まま — `MAX_REDIRECTS`(4) の hop 上限による fail-closed が既に維持されて
  おり、本 Issue の最小修正では追加しない（判断を記録）。
- `docs/test-specs/alib-06-webdav-provider.md` に「Feature: Redirect
  follow」を追加し、新規 test 名を Example 列に記載。
- `docs/specs/alib-07-webdav-real-run.md` の「制約と既知事項」を
  `DirectorySlash On` 再検証済みの事実に更新（tracked conf 自体は不変更）。

## 正規検証 lane（canonical = mise）

| lane | コマンド | exit code | 結果 |
| --- | --- | --- | --- |
| fmt | `mise run fmt-check` | 0 | Pass |
| check | `mise run check synvelis-library` | 0 | Pass |
| lint | `mise run lint synvelis-library` | 0 | Pass |
| test | `mise run test synvelis-library` | 0 | Pass（`tests/webdav.rs` 55/55 — 新規 3 件を含む。同 package の他 suite 全て pass、same-origin 限定・cross-origin 拒否・root confinement・credentials/ETag/Range 契約は非回帰） |

## Regression test（failing test first — 補助証拠）

| 状態 | コマンド | 観測 | 結果 |
| --- | --- | --- | --- |
| 修正前（`f1a84a8` 時点の本番コード） | `cargo test --locked -p synvelis-library --test webdav collection_slash` | 3 件とも `ProviderError { kind: TransientIo, retry: Never, detail: "too many redirects" }` で失敗 | Fail（期待どおり ping-pong を再現） |
| 修正後（`9480f07`） | 同上 | 3 件とも pass | Pass |

追加 test（`tests/webdav.rs` Redirects セクション、request path を wire 記録する oracle）:

- `collection_slash_redirect_is_followed` — `/lib` に `301 Location: /lib/`、`/lib/` に `207` を返す script で、request path が `["/lib", "/lib/"]` かつ `browse` 成功。
- `collection_slash_redirect_merges_relative_location` — `/col` → `301 /col/` → `301 Location: inner/`（相対）→ `/col/inner/` で `207`。request path `["/col", "/col/", "/col/inner/"]` — slash 付き base が相対 merge の正しい基準になることを pin。
- `collection_slash_redirect_judges_decoded_spelling` — `Location: /enc%2F`（encode された trailing slash）で decode 1 回後の綴り `/enc/` として follow。request path `["/enc", "/enc/"]`。

## docker 実サーバ再検証（stock mod_dir — `DirectorySlash Off` workaround なし）

`docker/` 配下の tracked ファイルは一切変更せず、`evidence/sv634/` の
`webdav-slash-on.conf`（`docker/webdav/conf/webdav.conf` の
`DirectorySlash Off` を `On` に戻した複製）を
`docker-compose.webdav.override.yml` で
`/usr/local/apache2/conf/extra/webdav.conf` へ bind mount して実走した。
probe は worktree の `crates/synvelis-library` を path 依存で build するため、
検証対象コードは本ブランチの実装そのもの。

| lane | コマンド / 観測 | 結果 |
| --- | --- | --- |
| compose 構文 | `docker compose -f docker-compose.webdav.yml -f evidence/sv634/docker-compose.webdav.override.yml --env-file docker/.env config -q` | exit 0 — Pass |
| 起動 | 同上 `up -d --build`（certgen → webdav） | exit 0 — Pass |
| redirect oracle | `curl -X PROPFIND https://localhost:8443/library`（認証付き） | `HTTP/1.1 301 Moved Permanently` + `Location: https://localhost:8443/library/` — stock mod_dir が slash 付与 redirect を返すことを実測 |
| slash 付き direct | `curl -X PROPFIND https://localhost:8443/library/` | `207` |
| probe（修正前コード） | `cargo run --manifest-path docker/webdav-probe/Cargo.toml`（`9480f07` の mod.rs を一時 stash して build） | `PROBE FAIL: TransientIo: too many redirects` — 実サーバで旧挙動を再現 |
| probe（修正後） | 同上 | `PROBE PASS` — `browse` が `/library` → `301` → `/library/` → `207` を follow し 2 entries（`nested/` Container + `sample.txt` Asset）`coverage=Complete`。stat/pin/open_pinned/open_preview・誤 credential `AuthExpired`・foreign CA `TlsVerification` も全て pass |
| 停止 | 同上 `down` | exit 0 — Pass |

## 受入条件との対応

| 受入条件 | 結果 |
| --- | --- |
| `resolve_redirect` が `Location` の trailing slash を `normalize_path` で落とさず collection slash 付与 redirect を follow できる（regression test で ping-pong 不発を確認） | Pass — 上記 3 test + docker 実走 |
| locator canonical 形と wire path の責務分離を維持し、same-origin 限定・cross-origin 拒否・`root_path` confinement・percent decode 1 回の既存契約を非回帰 | Pass — `within_root` は normalized 形に対してのみ適用（`mod.rs`）、`mise run test` で既存 test 全 pass |
| `mise run check/lint/test synvelis-library` が pass（OS-L1・Windows） | Pass（exit 0） |
| docker 実環境で `DirectorySlash Off` workaround なしの `browse` 成功を再検証 | Pass — override による stock `mod_dir`（`DirectorySlash On`）で `PROBE PASS`。`docker/` tracked ファイルは不変更のため恒久解除判断は残置（下表） |

## 残置事項（Pass 偽装しない）

| 項目 | 状態 | 後続 |
| --- | --- | --- |
| `docker/webdav/conf/webdav.conf` の `DirectorySlash Off` workaround の恒久的解除 | 未実施 — `docker/` は lease 外。本 Issue の検証は evidence 側 override で代替 | Coordinator / 別 Issue で判断 |
| `docs/test-specs/alib-07-fault-matrix.md`「実 WebDAV server は NotRun」行の verdict 更新 | NotRun — 後続 gate slice の責務（本ファイルはその材料） | 後続 gate slice |
| 同一 `(origin, path)` 再訪検出（design doc の「ループ検出」記述と実装の差） | 未実装 — hop 上限の fail-closed を維持し本修正では追加しない判断 | 必要になれば別 Issue |
| Linux/macOS での docker lane 実走 | NotRun — 当環境は Windows のみ | 必要になった時点の実測 |
| ローカル検証≠統合検証 | — | merge 後の main SHA に対する必須 CI 結果が統合検証の証拠（Coordinator が確認） |
