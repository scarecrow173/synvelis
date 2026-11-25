---
id: "SPEC-alib07-headless-residual"
type: "spec"
status: "implemented"
title: "ALIB-07 slice 3 — headless residual（fault matrix の現行 main 同期と LIB-SEC-04 新 transport 経路の閉塞）"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/91"
  changes:
    added:
      - type: "file"
        path: "docs/specs/alib-07-headless-residual.md"
      - type: "file"
        path: "evidence/sv91/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/lfs.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/webdav.rs"
      - type: "file"
        path: "docs/test-specs/alib-07-fault-matrix.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/test-specs/alib-07-fault-matrix.md"
  derives-from:
    - "docs/specs/alib-07-fault-matrix.md"
    - "docs/specs/alib-07-import-cancel.md"
    - "docs/specs/alib-07-webdav-real-run.md"
---

# ALIB-07 slice 3 — headless residual

## 目的

`docs/test-specs/alib-07-fault-matrix.md` の baseline が slice 2 時点
（`d867f88`+`ae5ddc4a`）で stale のまま残っていたため、現行 main
（`359fb293` = PR #644 merge）までに merge 済みの headless 証拠を
matrix の Observed/verified-by/baseline に反映する。あわせて、監査で
確定した未走行 headless gap — LIB-SEC-04 の新 transport 経路
（remote LFS batch http/https・signed download href・WebDAV TLS）に
おける credential/secret 非漏洩 assert — を `synvelis-library` の
headless test 追加で閉塞する。

本 slice は Issue synvelis#91 を close しない。実 provider/実 OS/
実 GUI/cross-OS の行は従来どおり `NotRun` として分離記録を維持する。

## スコープ

- 対象: `crates/synvelis-library/` の headless test（scripted mock /
  in-process fixture で完結するもの）と、matrix・evidence の簿記。
- 追加した test:
  - `tests/lfs.rs::remote_lfs_credentials_never_echo_in_typed_errors` —
    http/https 両経路の batch-401 で、sentinel credential が wire の
    `Authorization` header に届いたことを確認した上で、typed error
    の `Display`・`detail()`・`{err:?}` に sentinel が出ないこと。
  - `tests/lfs.rs::remote_lfs_signed_url_tokens_never_echo_in_errors` —
    signed download href の `?sig=` token が 403→`PermissionDenied`
    の error surface と `{pin:?}` に出ないこと。
  - `tests/webdav.rs::https_credentials_never_echo_in_typed_errors` —
    TLS 401→`AuthExpired` で secret 非含有。
- matrix 更新: baseline へ「Slice 3 追記」を append、front matter
  `verified-by` に `tests/lfs.rs`・`tests/vcs06_git_matrix.rs`・
  `tests/common/vcs_git_fixture.rs` を追加、LIB-PIN-02/03/05・
  LIB-IMPORT-04・LIB-CACHE-05・LIB-SEC-01/02/03/04 の Observed を
  現行 main の test 名で同期。
- スコープ外（NotRun 維持・変更しない）: 実 Git remote/実 LFS
  server/実 WebDAV server/OS credential store/実 SSRF 到達/実 FS
  障害注入/実 device/実 GUI、macOS/Linux leg の新規実走、consumer
  互換（Ruviform #219/#294 candidate SHA tuple）、LIB-IMPORT-07
  multiple windows（#89/#114 consumer slice）。

## 監査結果（本 slice の判断）

- LIB-SEC-04: 既存証拠は git transport と `OpaqueCredential` の Debug
  redaction のみ。LFS remote batch・signed href・WebDAV TLS 経路の
  credential 非漏洩 assert が無く、上記 3 test で閉塞した。provider
  src 側で secret が error detail に混入する欠陥は観測されなかった
  （error detail は固定句で構築される）。
- LIB-CACHE-01〜05: `tests/cache.rs`（約 30 test）が
  principal/credential 隔離・TTL/LRU/lease・revocation & clear・
  full-fetch warning・entry/depth/byte budget を網羅済み。cache 層は
  provider transport 非依存であり、新 transport 経路の追加 coverage は
  不要と判断した。FD/memory/disk の headless 計測不能軸は従来どおり
  NotRun 注記。
- LIB-PIN-04: LFS remote/TLS 追加後も既存証拠
  （`refresh_makes_remote_updates_visible`・
  `force_push_keeps_pinned_objects_fetchable`・
  `remote_rename_keeps_pin_and_fixed_commit_identity`）で十分と判断。
  mutable branch 更新は git object identity の問題であり LFS backend
  の追加で不変条件は変わらない。
- LIB-QOS-01〜03: `rt_cache.rs` 系の既存証拠を対象 SHA で再実走して
  green を確認する（`mise run test-rt` lane）。新規 test 追加は不要。
- LIB-QOS-04: headless 実現不能（実 device/OS/GUI）。`NotRun` を維持
  し、「実機/headless 証拠の区別記録」という行の要件は matrix 側の
  区別記録で継続して満たす。
- VCS cross: `tests/vcs06_git_matrix.rs`（synvelis#97 lane）を
  verified-by と LIB-SEC-04 の相互参照に記録するのみで、重複実装は
  しない。verdict は #97 側 ledger に従う。

## 検証

正規検証は `mise.toml` の lane（`mise run fmt-check`・
`check/lint/test/build synvelis-library`・`test-rt`・`test-msrv`・
`check-headless-deps`・`prek run --all-files`）で実施し、コマンド・
OS・exit code を `evidence/sv91/verification.md` に記録する。
cargo 直接実行は補助証拠としてのみ扱う。
