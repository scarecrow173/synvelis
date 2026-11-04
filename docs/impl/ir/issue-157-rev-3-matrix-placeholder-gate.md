---
id: "IMPL-034RUA08INQhlEiCYTUuqn"
type: "impl"
status: "completed"
title: "Issue #157 rev.3 matrix placeholder gate 修正"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "docs/designs/issue-157-matrix-placeholder-gate.md"
    - "docs/adr/issue-157-rev-3-skip-placeholder-gate-vs-workflow.md"
  changes:
    added:
      - "tools/fixtures/run-41894049161-jobs.json"
    modified:
      - "tools/ci-impact.mjs"
      - "tools/ci-impact.test.mjs"
      - ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-157-t1-matrix-placeholder-gate.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Issue #157 rev.3 matrix placeholder gate 修正

## Summary

evidence gate を skip 収縮 matrix placeholder 対応に修正した。
`tools/ci-impact.mjs` は `<lane> (${{ ... }})` 形の placeholder を
「同 lane の全 leg が report 上 scheduled===false・verdict=skipped・
同 attempt で leg に置き換えられていない」の 3 条件でのみ正当と判定し、
それ以外は fail-closed で violation にする。jobs 収集の expected 件数は
新 `expected-jobs` サブコマンドが schedule 状態から正確に算出する
(13/14/11・形式不正 report は最大 universe へ fail-safe)。

## Context

`docs/tasks/issue-157-t1-matrix-placeholder-gate.md`(
plan: `docs/plans/issue-157-matrix-placeholder-gate.md`、
design: `docs/designs/issue-157-matrix-placeholder-gate.md`、
ADR: `docs/adr/issue-157-rev-3-skip-placeholder-gate-vs-workflow.md`、
spec: `docs/specs/issue-157-matrix-placeholder-gate.md`)に従う。
PR #161 / run 41894049161 で再現した「required 全緑・macOS queued でも
evidence fail」のブロッカー修正。

## Implementation

### Changes

- `tools/ci-impact.mjs`
  - `PLACEHOLDER_NAME_RE` で `<prefix> (${{ <任意式> }})` を認識し、
    `evaluateGate` の unclassified 検査に placeholder 分岐を追加。
    - prefix 非 lane → 従来どおり unclassified violation
    - 同 lane の leg entry が placeholder より新しい run_attempt
      → stale placeholder(過去 attempt の記録)として無視
    - 全 leg scheduled===false かつ verdict=skipped
      → rows に `skipped-placeholder` 記録
    - それ以外 → required-lane-skipped / conclusion 異常の violation
  - `expectedJobs()` を新設し `expected-jobs` サブコマンドを追加。
    `lanes` が LANE_LEGS 全 name を覆わない report は形式不正として
    最大 universe(14)へ fallback。
  - usage 文字列に `expected-jobs` を追記。
- `.github/workflows/ci.yml` evidence job
  - `expected` を `expected-jobs` 呼出しに置換。
  - jobs fetch を `--paginate --jq '.jobs[]' | jq -s '.'` に変更し、
    `filter=all` の 100 件超過時に古い attempt 側で打ち切られる
    fail-open 余白を除去(同一行の修正で収まるため併せて対応)。
- `tools/ci-impact.test.mjs` — fixture 再生・placeholder 受理/拒否・
  stale supersede・conclusion 照合・非文字列名・expected-jobs
  13/14/11/fail-safe を pin(47 tests)。
- `tools/fixtures/run-41894049161-jobs.json` — run 41894049161 の
  実 jobs API payload(13 entries)。

### Decisions During Implementation

- 判定の正本は `expectation` ではなく `scheduled` フラグ: job-level
  `if:` は `lanes` 出力(= scheduled 集合)を消費するため(design 通り)。
- review-protocol(reviewer A/B)で確認された指摘を全て修正:
  - A-1(P2): re-run-all 後の stale placeholder で全緑 run が永久 fail
    → attempt スコープ supersede 判定を追加。
  - A-2/B-5(P3): 非文字列 job name で `name.match` が TypeError
    → typeof ガードで従来の unclassified violation へ復帰。
  - A-3/B-1(P3): `{lanes:[]}` 等の部分 report で expected が under-count
    → universe 全網羅を wellFormed 条件に。
  - B-4(P3): placeholder conclusion 未照合 → skipped のみ受理へ厳格化
    (design 文書も更新)。
  - B-3(P3): `per_page=100` 非 paginate の取りこぼし → `--paginate`+slurp。
  - A-4/B-6(P3): usage 文字列の subcommand 欠落を修正。
- B-2(attempt-masking: 旧 attempt の pass が新 attempt の結果を隠しうる)
  は rev.3 以前からの既存ギャップであり本変更非因果のため、
  「より深い構造問題は別途報告」の規約どおり未修正で記録のみ。

## Related Experiments

### Adopted

- 実 run 41894049161 の jobs API payload を fixture として取り込み、
  実測形態(`ui-verify (${{ matrix.os }})`・conclusion=skipped・13 件)
  そのままの gate 再生を採用。

### Rejected

- placeholder の式内容照合(`matrix.os` 等)は spec/design の Deferred
  どおり不採用 — 名 prefix + lane 状態で十分 fail-closed。

## Validation

- `mise run test-ci-impact` — 47 tests pass(新規 pin 含む)。
- `mise run lint-actions`(actionlint)— pass。
- `prek run --all-files` — pass。
- `mise run check` / `mise run fmt-check` / `mise run test` — pass。
- `node tools/ci-impact.mjs gate` を fixture で再生 — verdict pass、
  `ui-verify (${{ matrix.os }})` が skipped-placeholder 行として記録。
- `expected-jobs` 実測: 全 schedule report → 14、missing report → exit 1
  (gate が fail-closed)。

## Risks

- GitHub が将来 placeholder の名形式・conclusion 意味論を変える可能性
  — 未知形式は unclassified/conclusion violation で fail-closed。
- B-2 の attempt-masking(re-run 後に旧 attempt の pass が残るケース)は
  pickJob 由来の既存制約として残置 — 後続 issue 候補。

## Follow-ups

- `docs/impl/ir/README.md` は hand-curated で自動 index 更新を skip
  されたため手動更新が必要なら別途。
- B-2 attempt-masking 根本対応(attempt 単位の expected 収束)は
  別 issue として起票検討。
