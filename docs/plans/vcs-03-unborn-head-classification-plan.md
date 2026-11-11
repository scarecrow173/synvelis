---
id: "PLAN-0zQ5ZmJ0ht86AMtqu4MSy"
type: "plan"
status: "approved"
title: "synvelis#335 unborn HEAD 誤分類修正 — rev-parse --quiet 除去 実装計画"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/335"
  changes:
    added:
      - type: "file"
        path: "docs/plans/vcs-03-unborn-head-classification-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-03-unborn-head-classification-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-03-unborn-head-classification.md"
      - type: "file"
        path: "docs/impl/ir/vcs-03-unborn-head-classification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/git/vcs.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/vcs03_git_provider.rs"
      - type: "file"
        path: "docs/designs/vcs-03-git-read-provider-design.md"
      - type: "file"
        path: "docs/test-specs/vcs-03-git-provider-fixtures.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
  implemented-by:
    - "docs/tasks/vcs-03-unborn-head-classification-task.md"
    - "docs/impl/ir/vcs-03-unborn-head-classification.md"
  depends-on:
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-03-unborn-head-classification.md"
    - "docs/plans/vcs-03-git-read-provider.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-03-unborn-head-classification.md"
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
  derived-by:
    - "docs/tasks/vcs-03-unborn-head-classification-task.md"
    - "docs/impl/ir/vcs-03-unborn-head-classification.md"
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-03-unborn-head-classification.md"
  references:
    - "crates/synvelis-library/src/git/vcs.rs"
    - "crates/synvelis-library/tests/vcs03_git_provider.rs"
  defers: []
  deferred-by: []
---

# synvelis#335 unborn HEAD 誤分類修正 — rev-parse --quiet 除去 Implementation Plan

## Goal

`GitVcsProvider` の `rev-parse --verify` 呼出から `--quiet` を除去
し、unborn HEAD・解決不能 revision を fatal stderr → `classify_kind`
→ `NotFound` 経由で `RevisionGone`/`Ok(None)` へ正しく写像する。
共有 classifier・runner・mirror provider は変更しない。

## Scope

- In: `crates/synvelis-library/src/git/vcs.rs`（3 箇所の `--quiet`
  除去 + コメント整合）、`crates/synvelis-library/tests/
  vcs03_git_provider.rs`（回帰 test 1 件）、本タスクの文書
  チェーン、`docs/designs/vcs-03-git-read-provider-design.md`・
  `docs/test-specs/vcs-03-git-provider-fixtures.md` の整合更新。
- Out: `objects.rs`（`classify_kind`/`GitRunner`）、`mirror.rs`、
  `synvelis-project` 他 crate、`tests/common/*_fixture.rs`、
  workflow、lockfile、docs index。

## Upstream Documents

- `docs/specs/vcs-03-unborn-head-classification.md`

## Design Inputs

- `docs/designs/vcs-03-unborn-head-classification-design.md`（approved）

## File Map

- `crates/synvelis-library/src/git/vcs.rs` —
  `resolve_oid`/`head_oid`/`probe_workspace` の rev-parse argv から
  `"--quiet"` を除去し、doc comment を新契約に合わせる。
- `crates/synvelis-library/tests/vcs03_git_provider.rs` —
  `unborn_head_reports_typed_absence_not_unavailable` を末尾に
  追加（0-commit repo で detect/status/resolve/history/staged を
  観測）。
- `docs/designs/vcs-03-git-read-provider-design.md` — resolve 節の
  `rev-parse --verify --quiet` 記述を `--verify`（stderr 駆動
  NotFound 分類）へ整合。
- `docs/test-specs/vcs-03-git-provider-fixtures.md` — test 数
  19→20 と unborn HEAD feature 行を追加。
- `docs/specs|designs|plans|tasks|test-specs|impl/ir/vcs-03-
  unborn-head-classification*` — 本タスクの文書チェーン。

## Implementation Sequence

- [x] 診断: scratch repo で `rev-parse --verify` の `--quiet`
  有無を実測（unborn・未知 oid・範囲外 peel で exit 128 +
  `fatal: Needed a single revision` on stderr を確認）。
- [x] 回帰 test を先に追加し、修正前コードで
  `Unavailable { detail: "TransientIo: git command failed" }`
  の失敗を確認（pre-fix fail 証拠）。
- [x] `vcs.rs` 3 箇所の `--quiet` を除去し doc comment を更新。
- [x] `cargo fmt --all` で整形、`cargo test -p synvelis-library
  --test vcs03_git_provider` で 20/20 pass を確認。
- [x] canonical 検証: `mise run fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`check-app-native`/`lint-app-native`/`test-msrv`/
  `build`。

## Dependencies and Risks

- リスク: `--quiet` 除去で失敗時に stderr 診断が出るが、
  `sanitize_detail` 済み detail のみが `VcsError` に載り、
  secret 漏洩経路は増えない。
- リスク: git の将来の診断文変更で `NotFound` パターンを外れる
  可能性は残るが、回帰 test が unborn/未知 ref/HEAD alias の
  3 系を固定する。
- 共有 classifier を触らないため ALIB-04 mirror 経路の blast
  radius はゼロ（`tests/git.rs` は無変更で全件 pass することを
  `mise run test` で実測）。

## Task Breakdown

- `docs/tasks/vcs-03-unborn-head-classification-task.md` — 唯一の
  task。依存なし、並列分解なし。

## Verification

- [x] `cargo test -p synvelis-library --test vcs03_git_provider`
  （補助証拠、canonical = mise）20/20 pass。
- [x] `mise run fmt-check` — exit 0。
- [x] `mise run check` — exit 0。
- [x] `mise run lint` — exit 0。
- [x] `mise run test` — exit 0（`git.rs` ALIB-04 系を含む全
  workspace pass）。
- [x] `mise run test-rt` — exit 0。
- [x] `mise run check-app-native` — exit 0。
- [x] `mise run lint-app-native` — exit 0。
- [x] `mise run test-msrv` — exit 0。
- [x] `mise run build` — exit 0。
- [ ] PR CI の verify lane 実測を観測（マージ判断は Coordinator）。

## Open Questions / Gaps

- なし（境界判断は design に記録済み。共有 classifier 変更が
  必要と判明した場合のみ再検討）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
  consistent.

## Implementation Handoff

- Recommended mode: inline execution（vcs.rs 3 箇所 + test 1 件の
  小修正）。
- Independent work streams: none。
- User approval required before dispatch: no（Issue #335 の
  受入条件確定済みの是正タスク）。
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- `--quiet` 除去が「意味確定を git+classifier に委ねる」正しい
  責務境界であること（call-site で exit code を再解釈しない）。
- `probe_workspace` の `--quiet` 除去が semantic 不変
  （`.ok()` が全失敗を `None` に畳む informational 経路）である
  こと。
- 回帰 test が観測面（detect/status/resolve/history/staged）を
  網羅し、pre-fix fail / post-fix pass を証拠化していること。
