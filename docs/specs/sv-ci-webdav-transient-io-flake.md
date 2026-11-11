---
id: "SPEC-Zu5lKLwb3CUPWZGbvKMdCI"
type: "spec"
status: "proposed"
title: "Issue #245 verify (windows-latest) webdav MockServer TransientIo flake — accept 済みソケットの blocking 正規化"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/245"
    - "https://github.com/scarecrow173/synvelis/actions/runs/42938180676"
    - "https://github.com/scarecrow173/synvelis/actions/runs/42750871026"
  changes:
    added:
      - type: "file"
        path: "docs/specs/sv-ci-webdav-transient-io-flake.md"
      - type: "file"
        path: "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
      - type: "file"
        path: "docs/plans/sv-ci-webdav-transient-io-flake-plan.md"
      - type: "file"
        path: "docs/tasks/sv-ci-webdav-transient-io-flake.md"
      - type: "file"
        path: "docs/test-specs/sv-ci-webdav-transient-io-flake.md"
      - type: "file"
        path: "docs/impl/ir/sv-ci-webdav-transient-io-flake-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/common/webdav_fixture.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/webdav.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ci-245-macos-alloc-flake.md"
    - "docs/test-specs/alib-06-webdav-provider.md"
    - "docs/test-specs/alib-07-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
    - "docs/plans/sv-ci-webdav-transient-io-flake-plan.md"
    - "docs/tasks/sv-ci-webdav-transient-io-flake.md"
    - "docs/test-specs/sv-ci-webdav-transient-io-flake.md"
    - "docs/impl/ir/sv-ci-webdav-transient-io-flake-slice.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-library/tests/common/webdav_fixture.rs"
    - "crates/synvelis-library/tests/webdav.rs"
    - "crates/synvelis-library/src/webdav/http.rs"
  defers: []
  deferred-by: []
---

# Issue #245 verify (windows-latest) webdav MockServer TransientIo flake — accept 済みソケットの blocking 正規化

## What Should Be Built

`crates/synvelis-library/tests/common/webdav_fixture.rs` の
`MockServer` が、`accept()` で得た `TcpStream` を `handle_conn` 冒頭で
blocking モードへ正規化する。listener は stop 監視のため
nonblocking で動かしており、Winsock では accept 済みソケットが
listener の nonblocking 属性を継承する（POSIX では継承しない）。
`handle_conn` / `read_request` / `write_response` は「timeout 付き
blocking ソケット」前提で書かれており、nonblocking のままだと
client の先頭バイト到着より先に `read` が `WouldBlock` で失敗して
接続を drop し、request 未消費のまま close されるため RST が
client へ返り、`read head` / `write` で os error 10054 / 10053 の
`ProviderErrorKind::TransientIo` になる。

あわせて、accept loop が per-connection な一過性エラー（accept 前に
死んだ queued 接続が Winsock で報告される
`ConnectionReset`/`ConnectionAborted` 等）で server thread を黙って
終了しないよう、一過性 kind は sleep して継続する。

## Why Now

main Full CI run 42938180676（`17fc3dec` 系）で `verify (windows-latest)`
の `mise run test` が失敗し、`tests/webdav.rs` の
`auth_revoked_between_pin_and_conditional_get`・
`browse_partial_on_foreign_and_unaddressable_hrefs`・
`browse_lists_children_complete` の 3 件が `TransientIo`
（os error 10054/10053）で落ちた。3 件とも test 内 **最初の provider
呼出** で `read head`/`write` 失敗しており、run 開始 9 ms 以内に
一括発生している。同一 SHA の ubuntu/macos lane は pass — 本 bug は
Winsock 依存のため POSIX では顕在化しない。Worker ローカルでも
並列負荷を掛けると同じ `TransientIo`（write: 10053）を再現できた。
`ci-summary` が fail-closed のため、main の検証信号が信用できない。

## Users and Value

- Synvelis 開発者・CI 運用者: `verify (windows-latest)` の合否が
  実際の回帰を反映する状態に戻り、main CI の signal が復元される。
- `synvelis-library` の WebDAV テスト: mock の接続ライフサイクル
  契約（`Connection: close`・1 request/conn・scripted 応答）が
  OS 非依存に保たれる。

## Scope

### In Scope

- `handle_conn` 冒頭で accept 済み `TcpStream` を
  `set_nonblocking(false)` に正規化（socket mode の所有境界を
  fixture が担保する）。
- accept loop の per-connection 一過性エラー
  （`WouldBlock`/`ConnectionReset`/`ConnectionAborted`/`TimedOut`/
  `Interrupted`）を sleep + continue とし、listener-level の
  エラーのみで終了する分岐。
- Windows 上で deterministic に race window を強制する回帰 test の
  `tests/webdav.rs` への追加（client が accept poll 間隔を超えて
  request を遅延させる）。

### Out of Scope

- provider 側の接続管理・エラー分類の変更。`io_provider_error` が
  transport 失敗を `TransientIo`（`retry: After(None)`）へ写像する
  分類は正当であり、本修正では「正常 close 後の transient」と
  「真の接続破壊」の区別を緩めない。
- テスト成功条件の緩和・retry/再試行の追加による隠蔽。
- `crates/synvelis-library/src/` のプロダクトコード変更（診断の
  結果、発生源は fixture の socket mode であり provider 側の
  不変条件は破られていない）。
- `.github/workflows/`、他クレート、`Cargo.toml`/`Cargo.lock`。

## Requirements

- accept 済みソケットは `handle_conn` の責務境界内で必ず
  blocking へ正規化されること（`set_nonblocking(false)`）。POSIX
  では no-op 相当であり振る舞いを変えない。
- `read_request`/`write_response`/`handler` の契約と mock の
  wire 契約（1 request/conn、`Connection: close`、scripted 応答）
  を変更しない。
- accept loop は stop flag と listener-level エラーでのみ終了し、
  dead pending connection の一過性報告では生き続けること。
- 回帰 test は成功条件を弱めず、fixture の接続ライフサイクルを
  観測可能な形で固定すること。
- 新規の外部依存を追加しないこと。

## Acceptance Criteria

- [ ] `tests/webdav.rs` の `TransientIo`（10054/10053）発生源が
  fixture の accept 済みソケット mode と特定され、原因層で修正
  されている（retry 追加・条件緩和なし）。
- [ ] 回帰 test（request 遅延 client）が修正前 Windows で決定的に
  失敗し（os error 10053）、修正後に pass することが証拠化される。
- [ ] `cargo test -p synvelis-library --test webdav` の連続実行が
  修正後に安定 pass することを複数回の実走で証拠化（canonical は
  `mise run test`、cargo 直接実行は補助証拠と明記）。
- [ ] 既存の 42 test（scripted HTTP 応答・`Connection: close`
  契約）を回帰しない。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv` が pass する。
- [ ] 直近 2 run の CI 失敗との対応（同一失敗パターン・修正が
  その源に効く根拠）が PR 本文に記録される。

## Deferred Design Concerns

- `mise run test` 以外の Windows lane（`gw10-verify`・`ui-verify`）
  に同種の in-process mock listener が存在するかの棚卸しは別
  slice 候補（本 slice の調査では `tests/common/` 内の他 fixture
  に nonblocking listener は見つからなかった）。
