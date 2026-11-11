---
id: "TSPEC-kFpNWfpWnZfgKYzwIi7JoE"
type: "test-spec"
status: "approved"
title: "Issue #245 webdav MockServer TransientIo flake — 接続ライフサイクル回帰 test 仕様"
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
        path: "docs/test-specs/sv-ci-webdav-transient-io-flake.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/webdav.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/common/webdav_fixture.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/alib-06-webdav-provider.md"
    - "docs/test-specs/alib-07-fault-matrix.md"
    - "docs/plans/sv-ci-webdav-transient-io-flake-plan.md"
    - "docs/tasks/sv-ci-webdav-transient-io-flake.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
  verified-by:
    - "crates/synvelis-library/tests/webdav.rs"
    - "crates/synvelis-library/tests/common/webdav_fixture.rs"
  references:
    - "crates/synvelis-library/tests/webdav.rs"
    - "crates/synvelis-library/tests/common/webdav_fixture.rs"
  defers: []
  deferred-by: []
---

# Issue #245 webdav MockServer TransientIo flake — 接続ライフサイクル回帰 test 仕様

本 slice が保証する対象は provider の振る舞いではなく
**fixture の接続ライフサイクル契約** — accept 済みソケットが
`handle_conn` の前提（blocking + timeout）を満たし、client の
request 到着タイミングに依存して接続が落ちないこと。
synvelis#245 の flake（`TransientIo` os error 10054/10053）は、
その前提が Winsock の nonblocking 継承で破られていたことが原因。

## 実行環境（Baseline）

- Synvelis branch: `fix/245-webdav-mock-transient-io`
- Toolchain: rustc 1.99.0（MSRV 1.89）
- OS: Windows（本 bug が顕在化する唯一の lane。POSIX では
  accepted socket が常時 blocking のため no-op）
- Fixture: `webdav_fixture::MockServer`（nonblocking listener +
  sequential accept、`Connection: close`、1 request/conn）
- Status 凡例は `alib-07-fault-matrix` に同じ。

## Feature: FIXTURE-CONN — accept 済み接続の socket mode 契約

- Rule: `handle_conn` は blocking + timeout 付き socket を要求する。
  accept 済み socket は Winsock では listener の nonblocking 属性を
  継承するため、fixture の責務として `handle_conn` 冒頭で
  `set_nonblocking(false)` に正規化する。`read`/`write` は
  「client bytes の未到着」で `WouldBlock` を返さず、request 未
  消費の conn drop による RST（client 側 os error 10054/10053）
  を生まない。
- Rule: accept loop は per-connection 一過性エラー（dead pending
  connection が Winsock で `ConnectionReset`/`ConnectionAborted`
  等として返るケース）で server thread を終了しない。stop flag
  と listener-level エラーのみが終了条件。
- Evidence: `tests/webdav.rs::fixture_serves_request_written_after_accept`

## 証拠 matrix（Case ID 別 Expected/Observed/Status）

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| FIXTURE-CONN-01 request 遅延 client | client が accept poll 間隔（2 ms）を大きく超える 150 ms 待機後に request を書いても、server は conn を保持し scripted 応答 `HTTP/1.1 204` を返す | `webdav.rs::fixture_serves_request_written_after_accept`（Pass。`set_nonblocking(false)` 一時 revert では Windows で `Os { code: 10053, kind: ConnectionAborted }` に決定的に失敗 — pre-fix fail / post-fix pass を確認） | Pass |
| FIXTURE-CONN-02 既存 42 test 非回帰 | scripted HTTP 応答・`Connection: close` 契約の全 test が green | `cargo test -p synvelis-library --test webdav`（補助証拠、canonical = `mise run test`）43/43 pass、並列 3 worker × 40 iter で 0 fail（修正前は同条件で 4 件の 10053/10054 TransientIo） | Pass |

## NotRun / Unsupported 記録

- 実 WebDAV server・TLS・OS credential store・実ネットワーク:
  fixture 設計どおり NotRun/Unsupported（`alib-06`・`alib-07`
  test-spec の範囲を変更しない）。
- accept loop の dead pending connection 経路（`ConnectionReset`
  continue 分岐）は OS の backlog 注入に依存するため headless
  deterministic には直接観測不能 — kind 分類は code review で
  担保し、回帰 test の保証対象外とする。

## 集計

| Feature | Cases | Pass | Fail | NotRun | Unsupported |
| --- | --- | --- | --- | --- | --- |
| FIXTURE-CONN | 2 | 2 | 0 | 0 | 0 |
