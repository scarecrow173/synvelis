---
id: "TSPEC-034SYN166CiGateX"
type: "test-spec"
status: "proposed"
title: "Issue #166 ci-summary fail-closed gate 検証仕様"
created: "2026-11-05"
updated: "2026-11-05"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/166"
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  changes:
    added:
      - type: "file"
        path: "tools/ci-summary.test.mjs"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-148-impact-classification.md"
    - "docs/test-specs/issue-157-matrix-placeholder-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-166-full-ci-ci-summary.md"
    - "docs/plans/issue-166-full-ci-ci-summary.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
    - "docs/designs/issue-166-full-ci-ci-summary.md"
  verified-by: []
  references:
    - "tools/ci-summary.mjs"
    - "tools/ci-summary.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #166 ci-summary fail-closed gate 検証仕様

## Purpose

`tools/ci-summary.mjs` の fail-closed 契約を pin する。分類・期待集合を
持たない Full CI 集約で、未実行/skipped/failure/cancelled/timed_out/
unknown を成功としないこと、`pr`/`full` mode の required 昇格境界が
設計どおりであることを検証する。

## Cases(node:test、tools/ci-summary.test.mjs)

### pass 条件

1. 全 12 leg success + evidence(in_progress)で `pr`/`full` とも
   ok=true(full では evidence 完了済み success も許容)。
2. `full` mode で全 leg + `evidence` job success → ok=true。
3. attempt 指定時、旧 attempt の失敗 job が後続 attempt の success で
   上書き評価される。

### fail-closed(violation)条件 — mode 共通

4. required leg の `failure` → violation。
5. required leg の `cancelled` → violation。
6. required leg の `timed_out`/`action_required`/`neutral`/未知
   conclusion → violation(fail 判定)。
7. required leg の `skipped` → violation(skipped is not Pass)。
8. required leg の status=`in_progress`/`queued` → not-run →
   violation。
9. required leg が jobs API に absent → violation。
10. run 内の未知 job 名 → unclassified violation。
11. `<lane> (${{ matrix.os }})` 形 placeholder 名 → unclassified
    violation(lane `if:` 廃止後は受理しない)。

### mode 昇格境界

12. `pr` mode: `verify-macos` failure は violation ではなく observed
    記録(rev.2 ブロッキング境界維持)。
13. `full` mode: `verify-macos` failure/cancelled/absent は
    violation(Full CI 失敗)。
14. `full` mode: `evidence` job が failure/absent → violation
    (gate infra 障害を成功にしない)。
15. `pr` mode: `ci-summary` job は universe 既知名として受理されるが
    required でも observed でもない(その時点で未作成でも違反なし)。

### expected-jobs

16. `--mode pr` → 13(12 leg + evidence 自身)。
17. `--mode full` → 14(12 leg + evidence + ci-summary 自身)。

## Non-goals

- classify/分類規則の検証 — 撤去対象のため対象外。
- GitHub Actions 実 run の E2E — GitHub 書き込み禁止のため、jobs API
  応答を模した合成 JSON での再生検証に留める。
- `evidence`/`ci-summary` job 自体の summary 体裁の E2E — Job Summary
  出力は markdown 生成の unit 確認に留める。
