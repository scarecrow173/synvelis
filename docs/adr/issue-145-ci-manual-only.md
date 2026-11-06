---
id: "ADR-qs0t3aMSfTG19Vm5F4r2fp"
type: "adr"
status: "accepted"
title: "Issue #145 ci.yml manual-only 化 — pull_request/push trigger を撤去し workflow_dispatch 専用にする"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/145"
    - "https://github.com/scarecrow173/synvelis/issues/166"
    - "https://github.com/scarecrow173/synvelis/issues/167"
    - "docs/specs/issue-145-ci-manual-only.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "tools/ci-issue-tracker-reconcile.test.mjs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
  implements:
    - "docs/specs/issue-145-ci-manual-only.md"
  implemented-by:
    - "docs/designs/issue-145-ci-manual-only.md"
  depends-on:
    - "docs/specs/issue-145-ci-manual-only.md"
    - "docs/specs/issue-166-full-ci-ci-summary.md"
    - "https://github.com/scarecrow173/synvelis/issues/145"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/issue-167-tracker-reconcile-entry.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-145-ci-manual-only.md"
  derived-by:
    - "docs/designs/issue-145-ci-manual-only.md"
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "tools/ci-issue-tracker-reconcile.test.mjs"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# Issue #145 ci.yml manual-only 化 — pull_request/push trigger を撤去し workflow_dispatch 専用にする

## Context and Problem Statement

`ci.yml` は #166 stage 1 移行で「全 lane を全 trigger で毎回実行」に
整理されたが、`pull_request`/`push` trigger は `main-checkstatus`
ruleset の required context(`evidence`)を満たすため温存されていた
(#365 決定6 — trigger だけ先に止めると required が死に merge が
恒久的にブロックされるデッドロック禁止)。

その前提条件は全て解消した:

1. **Required 解除(先行条件)**: owner が `main-checkstatus` を
   disabled に変更 — 2026-11-06 の rulesets API 実測で SV/RF/AL の
   3 repo とも `required_status_checks` ルールが実効不在・
   `main-protect` は active(PR 必須・reviews 0・non-ff/deletion
   禁止・bypass なし)。classic branch protection なし。
2. **後続の自動検証系が main で稼働**: daily-ci(schedule+dispatch)、
   ci-issue-tracker(`workflow_run` event 入口)、
   ci-issue-tracker-reconcile(schedule+manual pull 入口)が main
   `e84a440` で実受入済み — run 42183098205 が mode reconcile・
   pin eed590e・最小権限・fail-closed noop を実測で確認。
3. **兄弟 consumer は適用完了**: RF PR #381・AL PR #164 が MERGED
   済みで PR/main push の CI 起動 0 を実 API 確認済み。SV が最後。

残る問題は「SV `ci.yml` の PR/push trigger が残っている」ことだけで、
PR open・synchronize・main push のたびに Full CI(12 leg)が自動
起動して runner コストと Actions 履歴を消費し、移行の目的(Daily
管理 + Issue 集約)に反する。

## Decision Drivers

- #145 の順序契約: Required 解除を実 API で確認してから trigger を
  止める — 逆順は merge デッドロックを生む。両条件を本 task で
  再検証した上で実施する。
- `ci.yml` の lane 構造・fail-closed 契約・`expected_sha` dispatch
  契約は daily-ci が依存する正本 — 変更しない。
- 正規入口の集約: 検証の entry point は `workflow_dispatch`
  (daily-ci・手動)に一本化し、PR 起動の重複をやめる。#145 運用
  方針どおり PR 時の判断はローカルテスト記録+レビューへ移行。
- `workflow_run` event 入口(ci-issue-tracker.yml)は `ci` の
  完了 run を消費する — trigger 撤去後も dispatch 由来の run
  完了で発火し続けるため契約不変。
- ruleset 書き込みは owner 権限 — worker は実 API 読み取り確認と
  workflow 変更に留める。

## Considered Options

- **Option A(採用)**: `pull_request` + `push` を両方除去し
  `workflow_dispatch` のみにする。#145 受入条件そのもので、
  RF(#381)/AL(#164)と同じ適用。
- **Option B**: `push: main` のみ残す(main 証跡の連続性確保)。
  **却下** — daily-ci が unverified SHA を検出して dispatch する
  設計が既に稼働しており、push ごとの自動 Full CI は runner 消費を
  二重化するだけ。reconcile 入口が dispatch 由来 run の抜けも
  拾うため検知 gap は戻らない。
- **Option C**: `pull_request` のみ除去し `push` は残す。
  **却下** — Option B と同じ重複。main 直 push は main-protect の
  PR 必須で事実上 merge commit のみだが、それも daily の対象外を
  生成しない(daily は main HEAD を無条件検査する設計ではなく
  未検証 SHA の検出 — push run が先に走ると二重起動)。
- **Option D**: trigger 撤去と同時に `evidence` job を廃止し
  ci-summary に畳む。**却下** — #145 は trigger 撤去の scope であり、
  evidence/pr-mode 集約は ci-summary の needs に組み込まれた
  fail-closed 層として残す(無関係の構造変更を混ぜない)。

## Decision Outcome

Chosen option: **Option A**。

- `on:` を `workflow_dispatch`(input `expected_sha`)のみにする。
- 全 lane・`evidence`・`ci-summary`・checkout pin・verify-sha
  契約・concurrency は無変更。
- 陳腐化コメントのみ更新: trigger 維持前提(#365 決定6 の条件部は
  達成済みとして記録)、`evidence` の required-context 位置づけ、
  concurrency コメントの trigger 名。
- 契約 test の ci.yml assertion を「dispatch 専用 + 陰性」へ転換。
- `CI_BASE_SHA` 式(`github.event.pull_request.base.sha`)は
  dispatch で空に解決して report context に記録されるのみ —
  fail-closed 判定に使わないため無変更。

### Consequences

- Good, because PR open/synchronize/main push で 12 leg の自動 CI が
  起動しなくなり、検証入口が daily/手動 dispatch に一本化される。
  Tracker(event)+ reconcile(pull)の 2 入口が Issue 集約を継続する
  ため失敗検知 gap は生じない。required checks は既に 0 で PR
  merge を塞ぐものがない。
- Bad, because PR 上で CI verdict が自動生成されない — 実装者の
  ローカル検証記録 + レビューが判断材料になる(#145 運用方針で
  受入済み)。誤って PR CI 証跡を期待する運用文書があれば追従が
  必要(既知の運用文書は #145/#166 に閉じる)。
- Neutral: `evidence` job は required context ではなくなったが、
  pr-mode の macOS 非 required 集約として ci-summary 手前の
  fail-closed 層に留まる。ruleset 側の `main-checkstatus` は
  disabled のまま owner が管理する。

## Implementation Plan

- Affected paths: `.github/workflows/ci.yml`(`on:` + コメント)、
  `tools/ci-issue-tracker-reconcile.test.mjs`(ci.yml trigger
  assertion の転換)、本 doc chain + 生成 index。
- Patterns to follow: RF #381 / AL #164 の consumer 適用
  (同じ trigger 除去パターン)、issue-167 chain の doc 構造。
- Patterns to avoid: ruleset 書き込み、`evidence`/`ci-summary` の
  構造変更、daily/tracker/reconcile への trigger 追加・削除、
  `CI_BASE_SHA` 式の意味論変更、製品コードへの波及。

## Verification

- [ ] `mise run test-ci-tracker-entry` → pass(dispatch 専用・
      陰性・不変性)。
- [ ] `mise run test-ci-summary` → 36/36 pass。
- [ ] `mise run lint-actions` → pass。
- [ ] `mise exec -- prek run --all-files` → pass。
- [ ] Ruleset API で 3 repo の required 0・main-protect active を
      実測(本 ADR の前提証拠を再掲)。
- [ ] 新 PR の opened/synchronize で Actions API に `ci` run 起動
      なし — head SHA/作成時刻で実測。
- [ ] 実 cron・実 Issue 更新 — 本工程範囲外(NotRun 維持)。
