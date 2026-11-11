---
id: "PLAN-0DE1gOLgt5QsNGBzENzb8a"
type: "plan"
status: "approved"
title: "Issue #245 webdav MockServer TransientIo flake — blocking 正規化 実装計画"
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
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
  implemented-by:
    - "docs/tasks/sv-ci-webdav-transient-io-flake.md"
    - "docs/impl/ir/sv-ci-webdav-transient-io-flake-slice.md"
  depends-on:
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
    - "docs/plans/ci-245-macos-alloc-flake-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-ci-webdav-transient-io-flake.md"
    - "docs/designs/sv-ci-webdav-transient-io-flake-design.md"
  derived-by:
    - "docs/tasks/sv-ci-webdav-transient-io-flake.md"
    - "docs/impl/ir/sv-ci-webdav-transient-io-flake-slice.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv-ci-webdav-transient-io-flake.md"
  references:
    - "crates/synvelis-library/tests/common/webdav_fixture.rs"
    - "crates/synvelis-library/tests/webdav.rs"
  defers: []
  deferred-by: []
---

# Issue #245 webdav MockServer TransientIo flake — blocking 正規化 Implementation Plan

## Goal

`MockServer` の accept 済みソケットを `handle_conn` 冒頭で blocking
に正規化し、Winsock の nonblocking 継承による `WouldBlock` →
conn drop → RST → client `TransientIo`（10054/10053）の因果鎖を
原因層で断つ。accept loop の per-connection 一過性エラー耐性と、
deterministic な回帰 test をあわせて入れる。

## Scope

- In: `crates/synvelis-library/tests/common/webdav_fixture.rs`、
  `crates/synvelis-library/tests/webdav.rs`、本タスクの文書チェーン。
- Out: `src/` プロダクトコード、他クレート、workflow、lockfile。

## Upstream Documents

- `docs/specs/sv-ci-webdav-transient-io-flake.md`

## Design Inputs

- `docs/designs/sv-ci-webdav-transient-io-flake-design.md`（approved）

## File Map

- `crates/synvelis-library/tests/common/webdav_fixture.rs` —
  `handle_conn` 冒頭に `set_nonblocking(false)`、accept loop に
  一過性エラー分類 `transient_accept_error`。
- `crates/synvelis-library/tests/webdav.rs` — 回帰 test
  `fixture_serves_request_written_after_accept`（import に
  `std::io::Write`・`std::net::TcpStream`・`std::time::Duration`
  を追加）。
- `docs/specs|designs|plans|tasks|test-specs|impl/ir/sv-ci-webdav-*` —
  本タスクの文書チェーン。

## Implementation Sequence

- [x] 診断: fixture に env-gated logging を入れて並列負荷下で再現
  → 失敗 conn は全て `read_request` の `WouldBlock` (10035) で
  drop されることを観測（証拠は IR に記録、logging は revert）。
- [x] `handle_conn` 冒頭で `stream.set_nonblocking(false)` を
  追加し、Winsock 継承を正規化。POSIX では no-op 相当。
- [x] accept loop の `Err(_) => break` を、per-connection 一過性
  kind は sleep + continue、残りは break の分岐に変更。
- [x] `webdav.rs` 末尾に回帰 test を追加（150 ms 遅延 request）。
- [x] `set_nonblocking(false)` を一時 revert して回帰 test が
  Windows で決定的に `os error 10053` で失敗することを確認
  （pre-fix fail / post-fix pass の証明）→ restore。
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` で検証。

## Dependencies and Risks

- リスク: 修正は 1 ファイルの socket mode 正規化であり、mock の
  wire 契約と 42 既存 test の振る舞いに影響しない。
- リスク: 一過性 accept エラー継続は listener-level 障害時に
  2 ms sleep で回り続ける。test 寿命内限定のため実害は小さく、
  break 条件は listener-level kind に限定して保持する。
- 再現不能時の fallback は不要 — Windows ローカルで決定的再現
  （回帰 test の pre-fix fail）を得ている。

## Task Breakdown

- `docs/tasks/sv-ci-webdav-transient-io-flake.md` — 唯一の task。
  依存なし、並列分解なし。

## Verification

- [x] `cargo test -p synvelis-library --test webdav`（補助証拠、
  canonical = mise）: 43/43 pass、並列 3 worker × 40 iter の
  ストレスで 0 fail（修正前は同条件で 4 件の 10053/10054）。
- [x] `mise run fmt-check` — exit 0。
- [x] `mise run check` — exit 0。
- [x] `mise run lint` — exit 0。
- [x] `mise run test` — exit 0。
- [x] `mise run test-rt` — exit 0。
- [x] `mise run test-msrv` — exit 0。
- [ ] PR CI の `verify (windows-latest)` 実測を観測（マージ判断は
  Coordinator）。単一 green は flake 除去の証明とみなさない。

## Open Questions / Gaps

- 他 lane の in-process mock listener に同種の nonblocking
  inheritance リスクがあるかは未棚卸（spec Deferred Design
  Concerns 記録どおり別 slice 候補）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（fixture 1 箇所 + test 1 件の
  小修正）。
- Independent work streams: none。
- User approval required before dispatch: no（受入条件確定済みの
  是正タスク。代わりに review-protocol レビューで blocker なしを
  確認する）。
- If declined or unavailable: execute inline using the same dependency
  graph and verification matrix.

## Review Handoff

- `handle_conn` 冒頭の `set_nonblocking(false)` が socket mode の
  正しい所有境界であること（`read_request`/`write_response` が
  blocking + timeout を前提とする不変条件を fixture が担保）。
- `transient_accept_error` の kind 分類が「dead pending connection
  の一過性報告 → continue」「listener-level → break」を正しく
  分けていること。
- 回帰 test が provider ではなく fixture の接続ライフサイクルを
  deterministic に固定していること。
