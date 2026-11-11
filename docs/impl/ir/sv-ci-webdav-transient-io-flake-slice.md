---
id: "IMPL-p1XVD7cITxEvasi3u5lh9F"
type: "impl"
status: "completed"
title: "Issue #245 webdav MockServer TransientIo flake — accept 済みソケット blocking 正規化 slice"
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
        path: "docs/impl/ir/sv-ci-webdav-transient-io-flake-slice.md"
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
    - "docs/tasks/sv-ci-webdav-transient-io-flake.md"
    - "docs/plans/sv-ci-webdav-transient-io-flake-plan.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/sv-ci-webdav-transient-io-flake.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
    - "docs/plans/sv-ci-webdav-transient-io-flake-plan.md"
    - "docs/test-specs/sv-ci-webdav-transient-io-flake.md"
    - "docs/impl/ir/ci-245-macos-alloc-flake-slice.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/sv-ci-webdav-transient-io-flake.md"
    - "crates/synvelis-library/tests/common/webdav_fixture.rs"
    - "crates/synvelis-library/tests/webdav.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Issue #245 webdav MockServer TransientIo flake — accept 済みソケット blocking 正規化 slice

## Summary

`MockServer` の `handle_conn` 冒頭で accept 済み `TcpStream` を
`set_nonblocking(false)` に正規化した。Winsock では accepted
socket が listener の nonblocking 属性を継承し（POSIX は継承
しない）、`read_request` の `read` が client の先頭バイト到着に
競ち勝つと `WouldBlock` で conn を drop → request 未消費のまま
close → RST → client が `read head`/`write` で os error
10054/10053 の `TransientIo` になる因果鎖を、fixture の socket
mode 境界で断った。あわせて accept loop を per-connection 一過性
エラーで生き続けるよう分類し、deterministic な回帰 test を
`tests/webdav.rs` に追加した。provider 側（`src/`）は不変 —
`TransientIo` 写像は正当な transient 分類として維持。

## Context

`docs/tasks/sv-ci-webdav-transient-io-flake.md` /
`docs/plans/sv-ci-webdav-transient-io-flake-plan.md` /
`docs/specs/sv-ci-webdav-transient-io-flake.md` /
`docs/designs/sv-ci-webdav-transient-io-flake-design.md` に従う。
main Full CI run 42938180676 の `verify (windows-latest)` で
`tests/webdav.rs` 3 件（`auth_revoked_between_pin_and_conditional_get`・
`browse_partial_on_foreign_and_unaddressable_hrefs`・
`browse_lists_children_complete`）が test 内最初の provider 呼出で
`TransientIo`（`read head: os error 10054` / `read head: os error
10053`）に flake した件の是正。Issue #245 は tracker issue であり、
同 run の verify-macos 失敗（rt_cache alloc 計測）は
`ci-245-macos-alloc-flake-*` チェーンが対応する。

## Diagnosis（因果の特定経過）

1. `pin`→`stat_remote`・`browse` はいずれも 1 PROPFIND request で、
   provider は TransientIo を retry しない。すなわち **test 内
   最初の唯一の connection が `read head`/`write` で reset** されて
   いる。CI log では 3 件とも run 開始 9 ms 以内に一括失敗。
2. fixture に env-gated logging を一時追加し、並列 3 worker で
   webdav.exe を回して再現（約 3 %/run）。失敗した全ての接続で
   server 側が `read_request` の `Os { code: 10035, kind:
   WouldBlock }` を返して conn を drop していた — accept 済み
   socket が nonblocking のまま `handle_conn` に渡っていた。
3. Winsock の accepted-socket nonblocking 継承が原因と特定。
   `handle_conn`/`read_request`/`write_response` は「timeout 付き
   blocking socket」前提で書かれており、`WouldBlock` を fatal に
   扱う設計自体は正しい — 破られた不変条件は socket mode であり、
   所有層は fixture の accept 経路。request 未消費のまま close する
   ため kernel が RST を返し、client の `read head`/`write` で
   10054/10053 になる。loopback では通常 client の write が先に
   届くため大半は発動せず、高負荷並列で顕在化する flake だった。

## Implementation

### Changes

- `crates/synvelis-library/tests/common/webdav_fixture.rs`
  - `handle_conn` 冒頭に `let _ = stream.set_nonblocking(false);`
    を追加（`set_read_timeout`/`set_write_timeout` 設定より前）。
    Winsock 継承を正規化し、POSIX では実質 no-op。
  - accept loop の `Err(_) => break` を、`transient_accept_error`
    が true の kind（`WouldBlock`・`ConnectionReset`・
    `ConnectionAborted`・`TimedOut`・`Interrupted`）は sleep +
    continue、それ以外は break の分岐に変更。dead pending
    connection が Winsock（および POSIX の ECONNABORTED）で
    accept() に返るケースで server thread が黙って死ぬ第二の
    flake 源を除去。
- `crates/synvelis-library/tests/webdav.rs`
  - 回帰 test `fixture_serves_request_written_after_accept` を
    末尾に追加。client が accept poll 間隔（2 ms）を大きく超える
    150 ms 待ってから request を書き、`HTTP/1.1 204` head を
    検証する。修正前 Windows では `Os { code: 10053, kind:
    ConnectionAborted }` で決定的に失敗することを一時 revert で
    確認済み。client read は `set_read_timeout(10s)` で wedge を
    hang ではなく fast fail に変換。
- 文書チェーン: `docs/specs|designs|plans|tasks|test-specs|impl/ir/
  sv-ci-webdav-transient-io-flake*`。

### Decisions During Implementation

- socket mode 正規化は `handle_conn` 冒頭に置いた。socket の
  前提を要求するのは `read_request`/`write_response` であり、
  fixture の責務として接続処理境界で担保するのが正しい層。
  `read_request`/`write_response` への `WouldBlock` retry は、
  正規化後では到達不能な補正を信頼済み内部に撒くことになるため
  採用しない。
- `set_nonblocking(false)` は `let _ =`（best-effort）。失敗しても
  従来挙動に落ちるだけで新規の害を生まず、file 内の timeout
  設定と同じ流儀。`.expect()` にすると server thread panic で
  downstream が分かりにくくなる。
- accept loop の継続 kind は「dead pending connection の一過性
  報告」のみに限定し、listener-level kind（例: `PermissionDenied`
  など fd/権限系）は break を維持して fail-fast を残した。

## Related Experiments

### Adopted

- fixture への env-gated 診断 logging（`WEBDAV_FIXTURE_DEBUG`）—
  原因特定に寄与。証拠取得後に revert し、本 slice には残さない
  （再 flake 時は同様に再付与して調査）。

### Rejected

- `read_request`/`write_response` 側への `WouldBlock` retry 追加 —
  socket mode を正規化した後では到達不能な defensive check であり、
  症状側の補正であって原因側の修正でない。
- provider (`src/webdav/http.rs`) への retry/分類緩和 — transport
  失敗の `TransientIo` 写像は正当であり、retry で隠すのは本
  slice の禁止事項に該当。

## Validation

ホスト: Windows（ローカル、rustc 1.99.0 / MSRV 1.89.0）。cargo 系は
`mise run` 経由、`cargo test -p synvelis-library --test webdav` の
直接実行は補助証拠（canonical = mise）。

- `mise run fmt-check` — exit 0。
- `mise run check` — exit 0。
- `mise run lint` — exit 0（clippy -D warnings）。
- `mise run test` — exit 0（workspace 全 pass、`webdav` 43/43 で
  `fixture_serves_request_written_after_accept` 含む）。
- `mise run test-rt` — exit 0。
- `mise run test-msrv` — exit 0（rust 1.89.0 で workspace test）。
- 補助証拠（並列ストレス）: webdav.exe を 3 worker × 40 iter で
  並列実行 — **0 fail**（修正前の同条件では 4 件の
  10053/10054 `TransientIo`、約 3 %/run）。
- 補助証拠（pre-fix fail 証明）: `set_nonblocking(false)` を一時
  revert すると回帰 test が `read: Os { code: 10053, kind:
  ConnectionAborted }` で決定的に失敗、復元後 pass — CI と同一の
  os error signature。

未実行: `verify (windows-latest)` の CI 実測（PR CI で観測する。
単一 green は flake 除去の証明にならない点を PR 本文に明記）。

## Risks

- accept loop の一過性継続は、persistent な listener-level 障害で
  2 ms sleep を挟み回り続ける。mock は test 寿命内限定のため実害
  は小さいが、listener-level kind のみ break を残す形で
  fail-fast の判断を一箇所に留めた。
- CI 環境で本修正が効かない別源（OS ephemeral port 枯渇等）の
  可能性は残る。ただし観測された signature（test 内最初の conn が
  `WouldBlock` drop で RST）は本原因で完全に説明される。
- `offline_server_fails_typed_on_every_op` が drop 直後の connect で
  ephemeral port を再利用中の他 listener に当たる理論上の窓は
  残るが、本 flake の観測 signature を説明する原因ではないため
  本 slice では扱わない。

## Follow-ups

- 他 lane（`gw10-verify`・`ui-verify`）の in-process mock
  listener に同種の nonblocking inheritance リスクがあるかの
  棚卸し（本 slice の編集範囲外。spec の Deferred Design
  Concerns に同記録）。
- PR CI の `verify (windows-latest)` 実測を観測し、flake 再発の
  有無を確認（マージ判断は Coordinator）。
