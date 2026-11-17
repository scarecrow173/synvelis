---
id: "EXP-521a1b2c3d4e5f6g7h8i9j0k2"
type: "experiment"
status: "completed"
title: "issue-521-lfs-transport — rustls transport seam 実験記録"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/521"]
  derives-from: ["docs/impl/ir/issue-521-lfs-transport.md"]
---

# EXP: issue-521-lfs-transport

イベント列は `issue-521-lfs-transport-exp.jsonl` を正本とする。
実装中に行った技術的検証と採否:

- **`rustls::Stream` vs 手動 `TlsConn`**: `Stream::read` の
  `prepare_read` は `wants_read` が立つ限り内部 loop し、
  `WouldBlock` を呼出側へ返さない経路があり得るため、
  cooperative `IoGuard`（cancel/deadline/stall の 250ms poll）
  を bypass する危険がある。vendored rustls source を確認し、
  `complete_io` は buffered plaintext がある間 `wants_read` を
  落とす（plaintext 喪失なし）ことを確認した上で、1 op =
  `complete_io` 1 pass + `reader().read` の手動 `TlsConn` を採用。
- **trust root seam**: `ProviderContext.tls: Option<TlsTrust>` —
  `None` は process 内 1 回 lazy の native root store
  （`rustls-native-certs`）、`Some` は `TlsTrust::from_pem` の
  明示 CA。rustls 型は公開 API に出さず、検証強度は不変
  （trust anchor の選択のみ）。
- **error 分類**: `rustls::Error::InvalidCertificate` →
  `ProviderErrorKind::TlsVerification`（`RetryAdvice::Never`、
  Coordinator 承認の option A — enum を `#[non_exhaustive]` 化し
  `library_ui.rs` に `"tls-verification"` wire tag を追加）。
  その他の TLS/record 失敗は `TransientIo`。detail に cert
  bytes・peer data・credential・署名 URL を出さない。
- **WebDAV 境界**: `send()` は引き続き `https` を
  `UnsupportedCapability` で拒否。TLS transport は
  `HttpEndpoint.tls` opt-in の caller（Git LFS）のみが使用し、
  WebDAV-over-HTTPS の解放は #90 の責務として残す（棄却案として
  記録）。
- **fixture**: in-process `MockTlsServer`（rustls
  `StreamOwned` accept loop）+ rcgen 生成 CA/leaf。
  `TlsTrust::from_pem` で trust を注入し、実 network・実
  remote は使わない。
