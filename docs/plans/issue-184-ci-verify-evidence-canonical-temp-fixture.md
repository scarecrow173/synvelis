---
id: "PLAN-034U2vlyQzakAB9i25oAuM"
type: "plan"
status: "approved"
title: "Issue #184 CI verify/evidence 失敗 — canonical temp fixture 実装計画"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/184"
    - "https://github.com/scarecrow173/synvelis/actions/runs/42431461645"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  implemented-by:
    - "docs/tasks/canonical-temp-fixture-synvelis-library-symlink.md"
  depends-on:
    - "docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-184-ci-verify-symlink-fixture.md"
    - "docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md"
  derived-by:
    - "docs/test-specs/issue-184-canonical-temp-fixture.md"
    - "docs/tasks/canonical-temp-fixture-synvelis-library-symlink.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-184-canonical-temp-fixture.md"
  references:
    - "crates/synvelis-library/tests/common/mod.rs"
    - "crates/synvelis-library/tests/local.rs"
  defers: []
  deferred-by: []
---

# Issue #184 CI verify/evidence 失敗 — canonical temp fixture 実装計画 Implementation Plan

## Goal

`synvelis-library` テスト fixture `TempTree` が canonical spelling の
ルートを返すようにし、非 canonical な temp 綴り環境（macOS `/var`、
Windows 8.3 名・junction）でも symlink 契約テストが成立するようにする。

## Scope

単一の実装ストリーム。変更はテスト helper 1 ファイルに限定し、
プロダクトコード・workflow・他クレートには触れない。

## Upstream Documents

- `docs/specs/issue-184-ci-verify-symlink-fixture.md`

## Design Inputs

- `docs/designs/issue-184-ci-verify-evidence-canonical-temp-fixture.md`（approved）

## File Map

- `crates/synvelis-library/tests/common/mod.rs` - `TempTree::new` で
  作成後に `fs::canonicalize` し canonical spelling を保持。根拠を
  示すコメントを追加。

## Implementation Sequence

- [ ] `TempTree::new` で `create_dir_all` 後に `fs::canonicalize` した
  パスを `Self { root }` に保持するよう変更し、ruviform canonical
  component 契約に関するコメントを付ける。
- [ ] 非 canonical temp 綴りの再現環境（junction 経由 `TEMP`/`TMP`）で
  `cargo test -p synvelis-library --test local` を実行し全件 pass を
  確認する。
- [ ] 通常環境で `mise run fmt` / `fmt-check` / `check` / `lint` /
  `test` / `build` を実行する。
- [ ] `git fetch origin main && git merge origin/main` で最新 main を
  取り込み、必要なら lockfile・index を再生成する。
- [ ] Conventional Commits でコミットし、dispatch されたブランチ
  （`scarecrow173/sv184-ci-verify`）へ push、PR を作成する。

## Dependencies and Risks

- 依存: なし（新規クレート依存・ツール追加なし）。
- リスク: Windows の `\\?\` verbatim 形がテスト内の他 API で問題に
  なる可能性 → design の実測済み範囲では問題なし。PR CI が最終確認。
- リスク: canonical 化で否定系テスト（escape/unavailable 系）の
  検証経路が変わる → 意図どおり「真の逸脱」判定になり、むしろ
  検証として強くなる（design 参照）。

## Task Breakdown

- `docs/tasks/canonical-temp-fixture-synvelis-library-symlink.md` —
  唯一のタスク。fixture 修正と検証を一体で行う。

## Verification

- [ ] `TEMP`/`TMP` を junction（非 canonical 綴り）に向けた
  `cargo test -p synvelis-library --test local` が全件 pass
  （修正前は同一 4 件 FAILED を再現済み — 対照実験として記録）。
- [ ] `mise run test` 全件 pass（通常環境）。
- [ ] `mise run fmt-check` / `check` / `lint` / `build` pass。
- [ ] PR CI で `verify-macos`・`verify (windows-latest)`・`evidence`・
  `ci-summary` の結果を確認（CI 限定 lane は PR 実測）。

## Open Questions / Gaps

- GitHub-hosted `windows-latest` の TEMP が具体的にどの非 canonical
  綴り（8.3 名か別要因か）かは runner イメージ依存で未確定。ただし
  junction による再現で同一失敗を確認しており、原因クラスは確定。
  PR CI の結果で最終確認する。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一ファイルの小修正）
- Independent work streams: none
- User approval required before dispatch: yes
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- reviewer は「fixture が canonical spelling を返す」一点と、
  否定系テストの検証経路改善を確認する。ruviform 側契約の変更を
  期待しないこと（out-of-contract は意図的）。
