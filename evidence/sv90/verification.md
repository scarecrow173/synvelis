# synvelis#90 — ALIB-06 WebDAV https/TLS opt-in 検証記録

対象 SHA（worktree HEAD）: `d151716e66d69e89cebbcd93aedb06f53c424906`（branch `feat/90-webdav-tls-provider`、origin/main と同一 — 最新 main の取り込みは差分なし）

OS: Windows（`x86_64-pc-windows-msvc`）、記録日: 2026-11-24

## 変更内容

- `WebDavProvider::request` が `url.scheme()` 毎に endpoint を選択 — `https` は `tls::session_for(&url, cx.tls.as_ref())` で `cx.tls` の trust root（未設定時は platform native store）から `TlsSession` を組み立て、`HttpEndpoint::new_tls` 経由で `http::send_endpoint` に送る。redirect hop 毎に再構築するため same-origin https redirect も同一 trust で処理する。http への silent downgrade はしない。
- `http.rs` の旧 `send`（https を `UnsupportedCapability` で拒否）を削除し、`send_endpoint` を全 caller の単一 entry point とした。
- `WebDavUrl::same_origin`（scheme+host+port 比較）により、https→http downgrade・http→https・別 port https は全て cross-origin として `InvalidRequest` で拒否される（既存契約のまま変更なし）。

## 追加 test 一覧（`crates/synvelis-library/tests/webdav.rs`）

- `https_dead_port_fails_typed_not_unsupported`（旧 `https_is_typed_unsupported` を新契約に更新 — dead port は `Offline` であり `UnsupportedCapability` ではない）
- `https_browse_and_stat_follow_plaintext_contract`
- `https_pin_and_open_pinned_keep_if_match_contract`（pin→`If-Match` conditional GET が TLS 上でも同一契約）
- `https_untrusted_ca_is_tls_verification`（foreign CA → `TlsVerification` + `RetryAdvice::Never`、error detail に秘密・URL を含まない）
- `https_native_roots_reject_self_signed`（`cx.tls` 未設定 → native roots が fixture CA を拒否）
- `https_same_origin_redirect_is_followed_over_tls`
- `https_downgrade_redirect_never_reaches_plaintext`（https→http redirect 拒否、credential 非転送）
- `https_cross_origin_redirect_never_reaches_other_origin`（別 port https redirect 拒否、credential 非転送）
- `https_deadline_fails_during_tls_handshake`（silent listener、handshake 中 deadline → `BudgetExceeded`）
- `import_verified_consumes_webdav_pin`（webdav pin → `import_verified` → SHA-256 blob 照合 → offline 再読）

## Verification lanes

| lane | コマンド | exit code | 結果 |
| --- | --- | --- | --- |
| fmt | `mise run fmt` | 0 | Pass |
| fmt-check | `mise run fmt-check` | 0 | Pass |
| check | `mise run check synvelis-library` | 0 | Pass |
| lint | `mise run lint synvelis-library` | 0 | Pass |
| test | `mise run test synvelis-library` | 0 | Pass（webdav.rs 52 tests 全 pass） |
| test-rt | `mise run test-rt` | 0 | Pass（rt_ 系 test 全 pass） |
| test-msrv | `mise run test-msrv` | 0 | Pass（workspace 全 test、rust 1.89.0） |
| build | `mise run build synvelis-library` | 0 | Pass |
| prek | `mise exec -- prek run --all-files` | 0 | Pass（9 hooks 全 Passed/Skipped） |

## 外部 gate（Pass 偽装しない）

| 項目 | 状態 | 後続 |
| --- | --- | --- |
| 実 WebDAV Server（SabreDAV/Nextcloud/IIS 等）・実 network・Cross OS 差・実 TLS capability matrix | NotRun — in-process fixture（`MockTlsServer`、rcgen fixture CA、IP SAN `127.0.0.1`）のみ検証 | synvelis#91（ALIB-REAL-04 fault matrix Owner）|
| 実 OS credential store（`CredentialResolver` 本番実装）| NotRun — app 層の責務。test は `TestResolver` stub | 別 gate（専任 Issue 未起票）|
| 実 GUI・Offline Project Playback E2E | NotRun | synvelis#89/#114 系 |
