# synvelis#91 — ALIB-07 headless residual slice の検証記録

対象 SHA（worktree HEAD・最終検証時点）: `084a01a4`（branch
`scarecrow173/sv91-alib07-headless-slice`。構成: base `359fb293` =
PR #644 merge で分岐 → `df2b41f` = 本 slice の変更 → `084a01a4` =
`origin/main` `c6701363`（PR #645）の merge。merge 時点で
`origin/main` と同一ツリーに同期済み。upstream 差分は
synvelis-app/synvelis-project と `evidence/sv643/` のみで、本
slice の変更範囲と Cargo.lock に差分は出なかった）。

下記 lane は merge 後の対象 SHA（`084a01a4`）で再実走した結果
（merge 前 `359fb293` でも同一結果を確認済み）。

Ruviform resolved SHA（`Cargo.lock`）:
`20dc41f71c47674b3e51d1eb0921124b94f8bfaf`

OS: Windows（`x86_64-pc-windows-msvc`、Git Bash）、toolchain:
rustc 1.99.1（MSRV 1.89）、記録日: 2026-11-25

## 変更の要約

- `docs/test-specs/alib-07-fault-matrix.md` を現行 main へ同期:
  baseline に「Slice 3 追記」を append（旧 baseline 行は履歴保持）、
  front matter `verified-by` に `tests/lfs.rs`・
  `tests/vcs06_git_matrix.rs`・`tests/common/vcs_git_fixture.rs` を
  追加し、LIB-PIN-02/03/05・LIB-IMPORT-04・LIB-CACHE-05・
  LIB-SEC-01/02/03/04 の Observed を merge 済みの headless test 名で
  更新した。
- LIB-SEC-04 の監査で確定した gap — remote LFS batch（http/https）・
  signed download href・WebDAV TLS 経路で credential/secret が typed
  error の `Display`/`detail()`/`{err:?}` に漏れないことの assert
  が無かった点 — を headless test 3 件で閉塞した:
  - `tests/lfs.rs::remote_lfs_credentials_never_echo_in_typed_errors`
    （http/https batch-401。sentinel `Bearer SECRET-LFS-7f3a` が
    wire の `Authorization` header に届いたことを recorded header で
    確認し、error surface に非含有を assert）
  - `tests/lfs.rs::remote_lfs_signed_url_tokens_never_echo_in_errors`
    （signed href `?sig=SIGNED-TOKEN-q1w2` + 403→`PermissionDenied`、
    token が error surface と `{pin:?}` に非含有）
  - `tests/webdav.rs::https_credentials_never_echo_in_typed_errors`
    （`MockTlsServer` 経由 TLS 401→`AuthExpired`、sentinel
    `Bearer SECRET-WEBDAV-5k9m` 非含有）
- `docs/specs/alib-07-headless-residual.md` に本 slice の scope spec
  を追加した。

## 監査結果（対象行ごとの verdict）

| Case | verdict | 根拠 |
| --- | --- | --- |
| LIB-PIN-02 | Pass（同期のみ・test 追加なし） | `tests/lfs.rs` の remote batch/TLS 一式が merge 済み。「LFS batch API/HTTP download endpoint は Unsupported/NotRun」の旧注記は stale — in-process `MockServer`/`MockTlsServer` で実走。実 LFS server は引き続き NotRun |
| LIB-PIN-03 | Pass（同期のみ） | `webdav.rs::https_pin_and_open_pinned_keep_if_match_contract`・`https_dead_port_fails_typed_not_unsupported`・`import_verified_consumes_webdav_pin` を Observed に追加 |
| LIB-PIN-04 | Pass（監査: 既存証拠で十分・test 追加なし） | `refresh_makes_remote_updates_visible`・`force_push_keeps_pinned_objects_fetchable`・`remote_rename_keeps_pin_and_fixed_commit_identity`。mutable branch 更新は git object identity の問題で、LFS remote/TLS backend の追加で不変条件は変わらない |
| LIB-PIN-05 | Pass（同期のみ） | `remote_lfs_(https_)batch_401_is_auth_expired`・`remote_lfs_(https_)download_403_is_permission_denied` を Observed に追加 |
| LIB-IMPORT-04 | Pass（同期のみ） | `webdav.rs::cancel_observed_inside_stalled_body` を Observed に追加 |
| LIB-CACHE-01〜05 | Pass（監査: 網羅済み・test 追加なし） | `tests/cache.rs` が principal/credential 隔離・TTL/LRU/lease・revocation & clear・full-fetch warning・entry/depth/byte budget を網羅。cache 層は provider transport 非依存のため、webdav TLS/git LFS https 経路の追加 coverage は不要。LIB-CACHE-05 には transport deadline/stall budget 証拠（`silent_response_head_fails_budget_typed`・`mid_body_stall_fails_budget_typed`・`https_silent_response_fails_budget_typed`・`https_deadline_fails_during_tls_handshake`・`remote_lfs_https_deadline_fails_during_tls_handshake`）を Observed に追加。FD/memory/disk の headless 計測不能軸は NotRun 注記を維持 |
| LIB-SEC-01 | Pass（同期のみ） | `collection_slash_redirect_*`（3 件）・`https_same_origin_redirect_is_followed_over_tls`・`https_downgrade_redirect_never_reaches_plaintext` を Observed に追加 |
| LIB-SEC-02 | Pass（一部 NotRun・同期のみ） | LFS credential forwarding 一式（`remote_lfs_credentials_stay_on_remote_origin`・`remote_lfs_https_credentials_stay_on_remote_origin`・`remote_lfs_redirect_strips_sensitive_action_headers_cross_origin`・`remote_lfs_https_redirect_is_followed_over_tls`・`remote_lfs_https_protocol_relative_redirect_stays_tls`）と `https_cross_origin_redirect_never_reaches_other_origin` を Observed に追加。実 SSRF 到達試行は NotRun 維持 |
| LIB-SEC-03 | Pass（同期のみ） | `remote_lfs_malformed_batch_is_typed_not_panic` を Observed に追加 |
| LIB-SEC-04 | Pass（一部 NotRun・test 追加で閉塞） | 新 transport 3 経路の credential 非漏洩 test を追加（変更の要約参照）。provider src の error detail は固定句で構築され secret 混入の欠陥は観測されなかったが、契約を pin する assert が存在しなかったため補強。`vcs06_git_matrix.rs::remote_credentials_never_surface_in_provider_output` は #97 lane の相互参照として記録。app 層の実 log pipeline は consumer 範囲で NotRun 維持 |
| LIB-QOS-01〜03 | Pass（再実走確認・test 追加なし） | `mise run test-rt` で対象 SHA の rt_ 契約を再確認（lane 結果は下表） |
| LIB-QOS-04 | NotRun（維持） | 実 device/OS/GUI 実走は headless 実現不能（#114/#89 系 gate）。行の要件「実機/headless 証拠の区別記録」は matrix の区別記録で継続して満たす |
| LIB-IMPORT-07 | NotRun（維持） | multiple windows は #89/#114 consumer slice。headless library slice では実現不能 |
| VCS cross | 相互参照のみ | `tests/vcs06_git_matrix.rs` は synvelis#97 lane。matrix の verified-by・LIB-SEC-04 Observed に相互参照として記録し、重複実装しない。verdict は #97 側 ledger に従う |

## 正規検証 lane（canonical = mise）

記録対象の全 lane を実走した。exit code と結果は下表のとおり。

| lane | コマンド | exit code | 結果 |
| --- | --- | --- | --- |
| fmt | `mise run fmt-check` | 1 | NotRun（環境不能 — 既知制約 `os error 206`、Windows コマンド長制限。補助証拠で代替・下表参照） |
| check | `mise run check synvelis-library` | 0 | Pass |
| lint | `mise run lint synvelis-library` | 0 | Pass |
| test | `mise run test synvelis-library` | 0 | Pass（`tests/lfs.rs` 24/24・`tests/webdav.rs` 60/60 — 新規 3 件を含む。同 package の他 suite も全て green） |
| build | `mise run build synvelis-library` | 0 | Pass |
| test-rt | `mise run test-rt` | 0 | Pass（workspace 全体の rt_ 契約 — LIB-QOS-01〜03 の headless 証拠を対象 SHA で再確認） |
| test-msrv | `mise run test-msrv` | 0 | Pass（MSRV 1.89 toolchain の workspace test） |
| headless-deps | `mise run check-headless-deps` | 0 | Pass（headless crate に GUI 依存なし） |
| prek | `mise exec -- prek run --all-files` | 0 | Pass |

## 補助証拠（canonical ではない）

新規 test の先行確認と fmt 制約の代替証拠。

| コマンド | exit code | 結果 |
| --- | --- | --- |
| `cargo test -p synvelis-library --test lfs` | 0 | Pass — 新規 2 件を含む全件 green |
| `cargo test -p synvelis-library --test webdav` | 0 | Pass — 新規 1 件を含む全件 green |
| `mise exec -- cargo fmt --package synvelis-library -- --check` | 0 | Pass — `mise run fmt-check` の `os error 206` 制約に対する代替（対象 SHA `084a01a4` で実施・変更した package の fmt は clean） |

## ローカル検証と統合検証の区別

- 本記録の lane 結果はローカル検証（変更範囲の検証 — Windows
  headless 実測）であり、統合受入の成功ではない。統合検証は merge
  後の対象 SHA に対する必須 CI 証拠（Coordinator が確認）。
- 他 lane の引用: 実 WebDAV docker 実走の先例は `evidence/sv633/`・
  `evidence/sv634/` を参照（本 slice では再実走していない — 引用
  のみ）。`tests/vcs06_git_matrix.rs` の verdict は synvelis#97 側
  ledger に従う。
- 本 slice で扱わない行（Issue 受入のうち cross-OS・実 provider/
  実 GUI・consumer 互換）は上表どおり `NotRun` 維持。後続 gate
  slice（実 provider E2E・cross-OS leg・GUI gate）への提案として
  記録する。
