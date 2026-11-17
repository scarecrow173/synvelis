# Git LFS remote fetch HTTPS/TLS transport — 検証・判定台帳 (synvelis#521)

Issue #521「Git LFS remote fetch の HTTPS/TLS transport」の実測記録。
ALIB-04 で `UnsupportedCapability` として残っていた `https://`
remote を、rustls（ring provider、TLS 1.2/1.3、検証必須・
silent downgrade なし）による検証済み TLS transport として実装。
trust root は `ProviderContext::tls: Option<TlsTrust>` seam
（`None` = native roots lazy、`Some` = `TlsTrust::from_pem` 明示
CA）。証明書検証失敗は新規 `ProviderErrorKind::TlsVerification`
（`RetryAdvice::Never`）に分類し、`library_ui.rs` に
`"tls-verification"` wire tag を追加（Coordinator 承認の
option A、enum は `#[non_exhaustive]` 維持）。

**設計上の要点（docs/impl/exp 記録）**: `rustls::Stream` の
`prepare_read` 内部 loop は cooperative `IoGuard` を bypass
し得るため不採用。`TlsConn` は `complete_io` 1 pass +
`reader().read` の手動駆動で `WouldBlock` を guard へ伝播。
handshake 中も `IO_POLL` 毎に cancel/deadline/stall を再確認。

## 環境・対象 Revision・実行コマンド

| 項目 | 値 |
|---|---|
| 対象 Repository | scarecrow173/synvelis |
| 対象 Revision | branch `feat/521-lfs-https-transport`、base `origin/main` `3e206c03`（rebase 済み — 旧 base `4b45304d` では sv513 merge 由来の gateway `op_name` 非網羅で workspace 系 check が blocked だったが、Coordinator 対処の PR #527 `61ac1d71` で修復済み）。検証は commit `aeec222a` で実施 — PR head SHA は PR 本文に記録 |
| OS / Arch | Windows 11 / x86_64（Git Bash 経由） |
| Rust toolchain | mise 管理（`mise.toml` / CI pin `mise.ci.toml` 1.99.0） |
| 実行日 | 2026-11-17 |
| 作業 worktree | `G:\orca\workspaces\synvelis\sv521-lfs-transport` |

## AC → case → test → result

| AC | case | test / 証拠 | result |
|---|---|---|---|
| `https://` remote が検証済み TLS で batch POST + download GET | trusted fixture CA | `tests/lfs.rs`: `remote_lfs_https_batch_download_fetches_and_verifies`（`MockTlsServer` + `TlsTrust::from_pem`） | Pass |
| 未信頼 CA は typed `TlsVerification`（Never retry） | foreign CA trust | `remote_lfs_https_untrusted_ca_is_tls_verification` | Pass |
| native roots は self-signed を reject（default 経路） | `ProviderContext::tls = None` | `remote_lfs_https_native_roots_reject_self_signed` | Pass |
| HTTPS batch 401 → `AuthExpired` | TLS 上の typed status | `remote_lfs_https_batch_401_is_auth_expired` | Pass |
| HTTPS download 403 → `PermissionDenied` | 同上 | `remote_lfs_https_download_403_is_permission_denied` | Pass |
| HTTPS download href / redirect が TLS で follow | `https://` href・redirect target | `remote_lfs_https_redirect_is_followed_over_tls` | Pass |
| protocol-relative href/redirect は remote/current の scheme を継承（silent downgrade なし） | `//host/…` href・`Location: //host/…` on https remote | `remote_lfs_https_protocol_relative_href_stays_tls` / `…_protocol_relative_redirect_stays_tls` | Pass |
| cross-origin signed URL に credential を送らない | https remote → 別 origin href | `remote_lfs_https_credentials_stay_on_remote_origin` | Pass |
| TLS handshake 中の deadline が `BudgetExceeded` | deadline < handshake | `remote_lfs_https_deadline_fails_during_tls_handshake` | Pass |
| plaintext HTTP / file:// / SSH・git:// 非回帰 | 既存 suite | `remote_lfs_batch_download_fetches_and_verifies` 他 + `file_remote_lfs_still_reads_local_objects` + `remote_lfs_https_remote_uses_tls_transport`（dead port → `Offline`） | Pass |
| WebDAV の `https` 拒否は不変 | `send()` 境界 | `tests/webdav.rs`: `https_is_typed_unsupported` | Pass |

## Canonical verification

| 検証 | コマンド | result |
|---|---|---|
| fmt | `mise run fmt-check` | Pass |
| check (library) | `mise run check synvelis-library` | Pass |
| check (workspace) | `mise run check synvelis-workspace` | Pass（rebase 後 `61ac1d71`+ で gateway 修復済み main 上で実走） |
| lint (library) | `mise run lint synvelis-library` | Pass（`Wire::Tls` の `large_enum_variant`・doc backtick・`unused_self` を修正済み） |
| lint (workspace) | `mise run lint synvelis-workspace` | Pass（同上） |
| test (library) | `mise run test synvelis-library` | Pass — 全 suite green（`tests/lfs.rs` 22/22、`tests/webdav.rs` 43/43 他） |
| test (workspace) | `mise run test synvelis-workspace` | Pass — 全 suite green（`library_ui` tests 含む） |
| headless deps | `mise run check-headless-deps` | Pass（12 headless crates に GUI 依存なし — rustls/rcgen 追加後も維持） |
| hooks | `mise exec -- prek run --all-files` | Pass |
| whitespace | `git diff --check` | Pass |

## Review 追補（worker_done 後の adversarial review 反映）

- action headers の cross-origin redirect 漏洩: batch action が
  発行した `authorization`/`proxy-authorization`/`cookie` を、
  redirect 先 origin が href 由来 origin と異なる hop では
  strip するよう `request()` を強化（Go `net/http`・browser と
  同じ sensitive-header 規則）。非 sensitive header は従来通り
  forward。回帰 test:
  `remote_lfs_redirect_strips_sensitive_action_headers_cross_origin`。
- 明示 `http://` redirect/href target の扱い: client は scheme
  を暗黙に変更しない（protocol-relative は現行 scheme を継承）。
  server が明示した `http://` target は upstream git-lfs と同じ
  parity で follow する — 当該 secret は同一 server chain が
  発行したものであり、silent downgrade ではない。`resolve_location`
  の doc に判断を記録。厳格化（https 文脈での http target 拒否）
  は Coordinator 判断に委ねる。
- `TlsConn::write` の `complete_io` error 握り潰し → 非 timeout
  error は即座に伝播するよう修正済み（error-kind fidelity 維持）。
- `reader().read` の WouldBlock 依存 → lock 済み rustls 0.23.46
  で `WouldBlock` 返却を確認済み（non-issue）。

## NotRun / Unsupported（正直記録）

- 実 HTTPS endpoint・実 remote（GitHub LFS 等）での動作確認 —
  外部 gate。fixture は loopback `MockTlsServer`（rcgen 生成
  CA/leaf）のみで、実 network・実 CA・実 remote は未検証。
- SSH / `git://` LFS transport — Unsupported 維持（socket を
  開かず typed error）。
- `.lfsconfig` / `lfs.url` / `remote.*.lfsurl` override、
  custom transfer adapter — provider に repository-config
  供給源がないため対象外（alib-04 設計書の非対象節を維持）。
- WebDAV-over-HTTPS — `send()` は引き続き `https` を
  `UnsupportedCapability` で拒否。TLS transport は
  `HttpEndpoint.tls` opt-in の caller（Git LFS）のみが使用。
  解放は #90 の責務。
- `*-full`/`test-rt`/`check-app-*`/`gw10` lane — Daily Full
  CI・別 lane 委譲。
- doc managed index（`docs/*/README.md`）の再生成 — 新規 doc
  追加に伴う index drift は Coordinator の統合時 regen に委譲
  （main では #525/#528 として別 commit で処理される先例）。
