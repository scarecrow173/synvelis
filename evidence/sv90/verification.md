# synvelis#90 — ALIB-06 WebDAV provider 検証記録

Issue #90 の検証記録を slice 毎に時系列で積む。各節は当該 slice の対象 SHA・OS・結果を記す。

## 2026-11-25 — producer 側残行収束 slice（timeout named test・実 server 実走）

対象 SHA（worktree HEAD）: `f8600b96`（検証時点の origin/main HEAD と同一。検証は
同 SHA + 本 PR 差分 — fixture/test/spec/evidence — のワークツリー状態で実施した。
commit 後の HEAD SHA は PR を参照）

OS: Windows 11（`x86_64-pc-windows-msvc`、Git Bash + Windows 版 curl/schannel）、
Docker Engine 29.9.2（Compose v5.6.1）、記録日: 2026-11-25

### 変更の要約

- `tests/common/webdav_fixture.rs` — `MockResponse::hold_open` を追加。応答 bytes
  を書き込んだ後、accepted connection を開いたまま応答を止める（server の stop
  flag まで保持）stall/silent peer を再現できる。既存の scripted 応答・TLS
  fixture・request 記録の挙動は不変。
- `tests/webdav.rs` — deadline/stall/cancel が型付き結果（silent 成功・hang・
  panic ではない）で返ることを固定する named test を追加。
- `docs/specs/alib-06-webdav-provider.md` — 現行実態へ同期（TLS opt-in 済み・実
  server lane の存在と本 slice の実測・外部 gate は NotRun/Deferred 維持）。

### 追加 test 一覧（`crates/synvelis-library/tests/webdav.rs`）

- `silent_response_head_fails_budget_typed` — silent peer（`hold_open` +
  空応答）に deadline 付き `stat` を投げ、deadline 超過で
  `ProviderErrorKind::BudgetExceeded` が型付きで返ること。server 側に request
  が届いている（peer は受信済みで応答だけを保留）ことを記録で併せて確認。
- `mid_body_stall_fails_budget_typed` — head + 先頭 chunk 応答後に残り body を
  保留する peer へ deadline 付き `open_pinned` を投げ、stream 途中の stall が
  `BudgetExceeded` で返ること（completion receipt にも型付き結果が残る）。
- `cancel_observed_inside_stalled_body` — `open_pinned` の reader に対し、
  配信済み prefix を `read()` で消費 → cancel → 残り stall body への `read()`
  が `ErrorKind::Interrupted`（内部 kind = `Cancelled`）で返ること。
  `read_to_end` は `Interrupted` を retry して `Ok(0)` に見せるため、reader
  contract を直接検証する。
- `https_silent_response_fails_budget_typed` — `MockTlsServer` + `hold_open`。
  TLS handshake 成立後の silent response head が同様に `BudgetExceeded` で
  返ること（`https_deadline_fails_during_tls_handshake` の handshake 中
  deadline に対し、本 test は handshake 後の応答待ちを固定）。

### 正規検証 lane（canonical = mise）

| lane | コマンド | exit code | 結果 |
| --- | --- | --- | --- |
| fmt | `mise run fmt` | 0 | Pass |
| fmt-check | `mise run fmt-check` | 0 | Pass |
| check | `mise run check synvelis-library` | 0 | Pass |
| lint | `mise run lint synvelis-library` | 0 | Pass |
| test | `mise run test synvelis-library` | 0 | Pass（`tests/webdav.rs` 59/59 — 新規 4 件を含む。同 package の他 suite 全て pass） |
| test-rt | `mise run test-rt` | 0 | Pass（workspace `rt_` 系 test 全 pass） |
| test-msrv | `mise run test-msrv` | 0 | Pass（workspace 全 test、rust 1.89.0） |
| build | `mise run build synvelis-library` | 0 | Pass |
| prek | `mise exec -- prek run --all-files` | 0 | Pass（全 hook Passed/Skipped） |

### 実 server lane（補助 lane — canonical = mise ではない）

`bash docker/verify.sh`（localhost の Apache `mod_dav` + TLS + Basic 認証。外部
network 不要）を実走 — 全体 **exit 0・`verdict=PASS`**。probe は worktree の
`crates/synvelis-library` を path 依存で build するため、検証対象コードは本
ブランチの実装そのもの。transcript: `docker-verify-transcript.txt`。

| lane | 観測 | 結果 |
| --- | --- | --- |
| compose 構文 `config -q` | exit 0 | Pass |
| 起動 `up -d --build`（certgen → webdav） | httpd Listen 8443 | Pass |
| 未認証 PROPFIND | `401`（Basic auth 強制） | Pass |
| PROPFIND Depth:1 | `207` multistatus・全 response に `getetag` | Pass |
| GET strong ETag | `ETag: "d2-6610e079ff4bc"`（weak `W/` なし） | Pass |
| If-Match（bogus） | `412` | Pass |
| If-Match（正しい validator） | `200` | Pass |
| Range `bytes=0-4` | `206` | Pass |
| probe `capabilities` | browse/stat/pin=ConditionalStrong/range_read/preview=Supported・access=ReadOnly | Pass |
| probe `browse`（`/library`） | 2 entries（`nested/` Container + `sample.txt` Asset）`coverage=Complete` | Pass |
| probe `stat` `/library/sample.txt` | `availability=Available`・`byte_len=210`（origin=Provider） | Pass |
| probe `pin` | `ConditionalStrong`/`StrongValidator`・validator=`"d2-6610e079ff4bc"`・precondition=IfMatch・expected_size=210 | Pass |
| probe `open_pinned` | 210 bytes 読了・`policy=IdentityBound{webdav-etag-if-match}` | Pass |
| probe `open_preview` `Range 0..16` | 16 bytes | Pass |
| negative: 誤 credential | `browse` が `Err(kind=AuthExpired)` — 実 server 401 を型付き化 | Pass |
| negative: foreign CA anchor | `browse` が `Err(kind=TlsVerification)` — trust pin が実効 | Pass |
| 停止 `down` | exit 0 | Pass |

producer 側の契約違反は見つからなかった（synvelis#634 型の実欠陥なし —
修正対象の発生なし）。

### 外部 gate（Pass 偽装しない）

| 項目 | 状態 | 後続 |
| --- | --- | --- |
| 実 WebDAV Server（Apache `mod_dav` + TLS + Basic）との適合 | Pass — 本 slice で `docker/verify.sh` を実走（上表） | — |
| 他 server 実装（SabreDAV/Nextcloud/IIS 等）・実 network 障害・Cross OS・fault matrix verdict 更新 | NotRun — docker lane は localhost のみ | synvelis#91（fault matrix Owner） |
| 実 OS credential store（`CredentialResolver` 本番実装） | NotRun — app 層の責務。test は `TestResolver` stub | 別 gate（専任 Issue 未起票） |
| 実 GUI・Offline Project Playback E2E・実録音/再生の非阻害 | NotRun | synvelis#89/#114 系 |
| Linux/macOS での lane 実走 | NotRun — 当環境は Windows のみ | 必要になった時点の実測 |
| ローカル検証≠統合検証 | — | merge 後の main SHA に対する必須 CI 結果が統合検証の証拠（Coordinator が確認） |

## 2026-11-24 — https/TLS opt-in slice（PR #625）

対象 SHA（worktree HEAD）: `7f4fd33`（branch `feat/90-webdav-tls-provider`、base `d151716e66d69e89cebbcd93aedb06f53c424906` — 検証時点の最新 origin/main と同一で取り込み差分なし）

OS: Windows（`x86_64-pc-windows-msvc`）、記録日: 2026-11-24

### 変更内容

- `WebDavProvider::request` が `url.scheme()` 毎に endpoint を選択 — `https` は `tls::session_for(&url, cx.tls.as_ref())` で `cx.tls` の trust root（未設定時は platform native store）から `TlsSession` を組み立て、`HttpEndpoint::new_tls` 経由で `http::send_endpoint` に送る。redirect hop 毎に再構築するため same-origin https redirect も同一 trust で処理する。http への silent downgrade はしない。
- `http.rs` の旧 `send`（https を `UnsupportedCapability` で拒否）を削除し、`send_endpoint` を全 caller の単一 entry point とした。
- `WebDavUrl::same_origin`（scheme+host+port 比較）により、https→http downgrade・http→https・別 port https は全て cross-origin として `InvalidRequest` で拒否される（既存契約のまま変更なし）。

### 追加 test 一覧（`crates/synvelis-library/tests/webdav.rs`）

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

### Verification lanes

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

### 外部 gate（Pass 偽装しない）

| 項目 | 状態 | 後続 |
| --- | --- | --- |
| 実 WebDAV Server（SabreDAV/Nextcloud/IIS 等）・実 network・Cross OS 差・実 TLS capability matrix | NotRun — in-process fixture（`MockTlsServer`、rcgen fixture CA、IP SAN `127.0.0.1`）のみ検証 | synvelis#91（ALIB-REAL-04 fault matrix Owner）|
| 実 OS credential store（`CredentialResolver` 本番実装） | NotRun — app 層の責務。test は `TestResolver` stub | 別 gate（専任 Issue 未起票）|
| 実 GUI・Offline Project Playback E2E | NotRun | synvelis#89/#114 系 |
