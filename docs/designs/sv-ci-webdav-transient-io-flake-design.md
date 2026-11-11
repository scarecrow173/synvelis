---
id: "DESIGN-jmv4N50pKcC7sDGSX5TVEq"
type: "design"
status: "approved"
title: "Issue #245 webdav MockServer TransientIo flake — accept 済みソケット blocking 正規化設計"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/245"
    - "https://github.com/scarecrow173/synvelis/actions/runs/42938180676"
  changes:
    added:
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
  implements:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
  implemented-by:
    - "docs/plans/sv-ci-webdav-transient-io-flake-plan.md"
  depends-on:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ci-245-macos-alloc-flake-design.md"
    - "docs/test-specs/alib-06-webdav-provider.md"
    - "docs/test-specs/alib-07-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
  derived-by:
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

# Issue #245 webdav MockServer TransientIo flake — accept 済みソケット blocking 正規化設計

## Context

`MockServer`（`tests/common/webdav_fixture.rs`）は stop 監視のため
`TcpListener` を `set_nonblocking(true)` で動かし、accept loop が
`WouldBlock` を 2 ms sleep で回す。観測された失敗は、CI run
42938180676（windows-latest）で test 内最初の provider 呼出が
`read head` / `write` の `TransientIo`（os error 10054/10053）に
なること、およびローカル再現でも同一 signature（`read head`/
`write` で 10053/10054）が出ること。

fixture に診断用 logging を入れて再現させると、**失敗した全ての
接続で server 側は `read_request` が `Os { code: 10035, kind:
WouldBlock }` を返して conn を drop していた**。すなわち accept 済み
`TcpStream` が nonblocking のまま `handle_conn` に渡っていた。

原因は Winsock と POSIX の accept 済みソケット属性差:
Winsock では `accept()` が返すソケットは listener の nonblocking
属性を継承し、POSIX では継承しない（blocking で返る）。Rust std
の `TcpListener::accept` も Windows ではその継承を反映するため、
`handle_conn` / `read_request` / `write_response` が前提とする
「timeout 付き blocking ソケット」契約が Windows では破られる。

破られた場合の因果は deterministic: accept 直後の `read` が
client の先頭バイト到着に競ち勝つと `WouldBlock` → `read_request`
が `Err` を返す → `handle_conn` が応答せずに return → client の
request bytes が unread のまま socket close → kernel が RST を送出
→ client の `read head`/`write` で os error 10054/10053 →
`io_provider_error` が `TransientIo` に写像。loopback では通常
client の write が先に届くため大半は発動せず、CI runner のような
高負荷並列環境（accept thread と client thread の scheduling
順位が入れ替わる）でのみ顕在化する — 「retry すれば解消する
flake」として観測されていたものと整合する。

破られた不変条件: **`handle_conn` は blocking + timeout 付き
socket を要求する**（`read_request` が `?` で `WouldBlock` を
fatal に扱う設計自体は正しい）。この不変条件を所有するのは
fixture の accept 経路であり、`src/webdav/` provider 側の
接続管理（1 request/conn・`Connection: close`・retry なし）に
問題はない — provider の `TransientIo` 写像は正当な transient の
分類として維持する。

## Scope

### In Scope

- `handle_conn` 冒頭での `stream.set_nonblocking(false)` —
  socket mode の正規化を fixture の接続処理境界に置く。
- accept loop の per-connection 一過性エラー継続（dead pending
  connection が Winsock で `ConnectionReset`/`ConnectionAborted`
  として accept() に返り得るため、それらで server thread を
  終了しない）。
- `tests/webdav.rs` に deterministic な回帰 test を追加。

### Out of Scope

- provider (`src/`) 側の変更 — 不変条件の破れは fixture にあり、
  `TransientIo` 分類を緩める必要はない。
- `read_request`/`write_response` への WouldBlock retry — socket
  mode を正規化した後では到達不能な補正であり、defensive check
  を信頼済み内部に拡散しない。
- mock の wire 契約（1 request/conn・`Connection: close`・
  scripted 応答・`MockResponse::raw`）の変更。

## Components and Boundaries

- `webdav_fixture.rs` `MockServer::start` / accept loop:
  listener の nonblocking 運用と stop 監視は現行どおり。accept
  エラーを「per-connection 一過性（`WouldBlock`・
  `ConnectionReset`・`ConnectionAborted`・`TimedOut`・
  `Interrupted` → sleep + continue）」と「listener-level
  （上記以外 → break）」に分類する。dead pending connection が
  Winsock で `WSAECONNRESET` を accept() に返すのは既知の挙動で、
  現行 `Err(_) => break` はそれ 1 件で mock server 全体を黙って
  殺し、backlog の正常接続を RST する第二の flake 源だった。
- `webdav_fixture.rs` `handle_conn`: socket mode 正規化の責務
  境界。`set_nonblocking(false)` → `set_read_timeout(15s)` →
  `set_write_timeout(15s)` → `read_request` → `handler` →
  `write_response` の順は変えない。POSIX では accepted socket
  は元来 blocking であり実質 no-op。
- `webdav.rs` 回帰 test `fixture_serves_request_written_after_accept`:
  client が connect 後に accept poll 間隔（2 ms）を大きく超える
  150 ms 待ってから request を書く。修正前の Windows では
  server が `WouldBlock` で conn を drop するため client の
  read/write が os error 10053 で決定的に失敗し、修正後は
  blocking read が待って応答を返す — race window を deterministic
  に固定する。

## Data and Control Flow

修正後の接続ライフサイクル:

1. accept loop（nonblocking listener）が conn を dequeue。
2. `handle_conn` が `set_nonblocking(false)` で socket を blocking
   へ正規化し、read/write timeout(15s) を設定。
3. `read_request` が head(`\r\n\r\n`)まで + `Content-Length` 分の
   body を blocking read（`WouldBlock` は発生しない。timeout の
   `TimedOut` は従来どおり fatal → conn drop の正当経路）。
4. `handler` が scripted `MockResponse` を返す。
5. `write_response` が head + body を `write_all` し、stream が
   drop → FIN（server-initiated close、`Connection: close`
   契約どおり）。

## Non-Functional Constraints

- mock の公開 wire 契約と `Handler`/`MockResponse`/`MockRequest`
  API は不変。`handle_conn` のシグネチャは外部非公開のまま。
- 新規依存・`#[allow]`・成功条件の緩和・テスト retry を追加しない。
- `set_nonblocking(false)` は POSIX で no-op 相当であり、
  ubuntu/macos lane の振る舞いを変えない。

## Risks and Trade-offs

- `set_nonblocking(false)` に失敗した場合、従来どおりの挙動に
  落ちる（`let _ =` は `set_read_timeout` 等と同じ best-effort
  流儀）。失敗時に panic すると server thread が死んで別種の
  混乱を生むため、契約上の失敗経路は維持する。
- accept loop の一過性継続は、persistent な listener-level 障害
  （fd 枯渇等）では sleep 2 ms を挟みつつ回り続ける。mock は
  test 寿命内に限定されるため、無限 spin のリスクは実質小さい;
  listener-level kind のみ break を残すことで fail-fast の判断を
  一箇所に留める。
- 回帰 test の 150 ms 待機は suite に ~0.16 s を加えるのみ。

## Deferred Design Concerns

- 他 lane（`gw10-verify`・`ui-verify`）の in-process mock
  listener 棚卸しは spec の Deferred Design Concerns に同記録。
- `read_request` の head 1 MiB bound・15 s timeout 値の再設計は
  本 flake の原因ではないため変更しない。

## Verification Notes

- [x] fixture 診断 logging で、失敗接続の server 側が
  `read_request` の `WouldBlock` (os error 10035) で drop する
  ことを並列負荷下で観測（iter 9/10/21/39 で同一パターン）。
- [x] 回帰 test を `set_nonblocking(false)` 無効化の一時 revert で
  実行 → Windows で決定的に `Os { code: 10053, kind:
  ConnectionAborted }` で失敗（修正前の再現 proof）。
- [x] 修正後 `cargo test -p synvelis-library --test webdav`
  （補助証拠、canonical = `mise run test`）で 43/43 pass。
