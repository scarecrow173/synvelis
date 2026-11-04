---
id: "TSPEC-034RUl6XqtnGnSMl6PCvgL"
type: "test-spec"
status: "approved"
title: "Issue #157 rev.3 matrix placeholder gate 検証仕様"
created: "2026-11-05"
updated: "2026-11-05"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/157"
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: "tools/ci-impact.test.mjs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-148-impact-classification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-157-matrix-placeholder-gate.md"
    - "docs/plans/issue-157-matrix-placeholder-gate.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
    - "docs/designs/issue-157-matrix-placeholder-gate.md"
  verified-by: []
  references:
    - "tools/ci-impact.mjs"
    - "tools/ci-impact.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #157 rev.3 matrix placeholder gate 検証仕様

## Purpose

skip 済み matrix lane の jobs API placeholder に対する gate 契約
(正当 placeholder は受理・required lane placeholder は拒否・未知
prefix は unclassified)と、収集カウント `expected-jobs` の正確性を
pin する。GitHub の placeholder 名形式が変わっても、fail-closed
方向にだけ壊れることを保証する。

## Feature

`tools/ci-impact.mjs evaluateGate` の unclassified 検査に対する
placeholder 分岐と、`expectedJobs`/`expected-jobs` サブコマンドによる
期待 job 件数計算。

## Rules

- Rule 1: `^<prefix> (${{ <式> }})$` 形式の名は placeholder として
  分類される。`<prefix>` が `LANE_LEGS` の `lane` に一致し、当該 lane
  の全 leg が impact report で `scheduled: false` なら violation に
  せず `skipped-placeholder` を rows に記録する。
- Rule 2: 同形式で lane に required/scheduled leg が 1 つでもあれば
  violation(required lane skipped)。report に当該 lane が無い場合も
  violation(fail-closed)。
- Rule 3: `<prefix>` がどの lane にも一致しない placeholder 風の名は
  従来どおり unclassified violation。
- Rule 4: `expectedJobs` = scheduled leg 数 + 非 scheduled lane 数
  (各 1 entry)+ OBSERVED_LEGS 数 + 1。report 非読取は最大
  universe(14)に fail-safe。
- Rule 5: 本変更は既存の violation 意味(required leg の fail/
  cancelled/absent/skipped、未知 job)を弱めない — 既存テスト全件が
  非回帰で pass する。

## Examples

- run 41894049161 再生(PR #161・ui-verify のみ非 schedule):
  jobs に `ui-verify (${{ matrix.os }})`(skipped)を含む 13 entry →
  evaluateGate ok=true、placeholder は `skipped-placeholder` 記録。
- 同 placeholder で selective report の ui-verify が required の場合 →
  violation(required lane skipped)。
- `foo (${{ matrix.os }})` → unclassified violation。
- expected-jobs: ui-verify 非 schedule → 13、全 schedule → 14、
  docs-only(classify のみ schedule)→ 11。
