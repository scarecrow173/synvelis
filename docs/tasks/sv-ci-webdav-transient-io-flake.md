---
id: "TASK-G5fq0gTQhuqohY3rd5he3w"
type: "task"
status: "done"
title: "webdav MockServer accept 済みソケットの blocking 正規化 + 回帰 test"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/245"
  changes:
    added:
      - type: "file"
        path: "docs/tasks/sv-ci-webdav-transient-io-flake.md"
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
    - "docs/plans/sv-ci-webdav-transient-io-flake-plan.md"
  implemented-by:
    - "docs/impl/ir/sv-ci-webdav-transient-io-flake-slice.md"
  depends-on:
    - "docs/plans/sv-ci-webdav-transient-io-flake-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
    - "docs/test-specs/sv-ci-webdav-transient-io-flake.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
    - "docs/plans/sv-ci-webdav-transient-io-flake-plan.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-library/tests/common/webdav_fixture.rs"
    - "crates/synvelis-library/tests/webdav.rs"
  defers: []
  deferred-by: []
---

# webdav MockServer accept 済みソケットの blocking 正規化 + 回帰 test

## Work

`webdav_fixture.rs` の `handle_conn` 冒頭で accept 済み `TcpStream`
を `set_nonblocking(false)` に正規化し、Winsock の nonblocking
継承による `read` の `WouldBlock` → conn drop → RST → client
`TransientIo`（10054/10053）を原因層で断つ。あわせて accept
loop を per-connection 一過性エラーで生き続けるよう分類し、
`tests/webdav.rs` に deterministic 回帰 test を追加する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sv-ci-webdav-transient-io-flake-plan.md`
- `docs/specs/sv-ci-webdav-transient-io-flake.md`
- `docs/designs/sv-ci-webdav-transient-io-flake-design.md`

## Dependencies

- なし（ブロッカーなし）

## Affected Files

- `crates/synvelis-library/tests/common/webdav_fixture.rs` —
  `handle_conn` の socket mode 正規化、accept エラー分類
  `transient_accept_error`
- `crates/synvelis-library/tests/webdav.rs` — 回帰 test
  `fixture_serves_request_written_after_accept` + import 追加
- `docs/specs|designs|plans|tasks|test-specs|impl/ir/sv-ci-webdav-*` —
  本タスクの文書チェーン

## Done When

- [x] `handle_conn` 冒頭で `stream.set_nonblocking(false)` され、
  blocking + timeout 前提の不変条件が fixture で担保される
- [x] accept loop が per-connection 一過性 kind（`WouldBlock`・
  `ConnectionReset`・`ConnectionAborted`・`TimedOut`・
  `Interrupted`）で sleep + continue し、残りで break する
- [x] 回帰 test が client の request を accept poll 間隔超
  （150 ms）遅延させ、修正前 Windows で決定的に
  `os error 10053` で失敗・修正後に pass する
- [x] mock の wire 契約（1 request/conn・`Connection: close`・
  scripted 応答）と既存 42 test の振る舞いが不変

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check` — exit 0
- [x] `mise run lint` — exit 0
- [x] `mise run test` — exit 0（`webdav` 43 test 含む全 pass）
- [x] `mise run test-rt` — exit 0
- [x] `mise run test-msrv` — exit 0
- [x] `cargo test -p synvelis-library --test webdav` 並列
  ストレス（3 worker × 40 iter）0 fail — 補助証拠
- [ ] PR CI の `verify (windows-latest)` 実測を観測（マージ判断は
  Coordinator）
