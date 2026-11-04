---
id: "PLAN-034SVCIOPT02jH8vNfRd"
type: "plan"
status: "approved"
title: "Issue #148 変更影響分類・Fail-Closed evidence Gate 実装計画"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/148"
    - "docs/designs/issue-148-impact-classification.md"
  changes:
    added:
      - type: "file"
        path: "tools/ci-impact.mjs"
      - type: "file"
        path: "tools/ci-impact.test.mjs"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/issue-148-impact-classification.md"
    - "docs/specs/issue-148-impact-classification.md"
  implemented-by:
    - "docs/tasks/issue-148-t1-impact-gate.md"
  depends-on:
    - "docs/designs/issue-148-impact-classification.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/ci-synvelis-pr-push-regression-gate.md"
    - "docs/plans/issue-151-ui-ci-lane.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-148-impact-classification.md"
    - "docs/designs/issue-148-impact-classification.md"
  derived-by:
    - "docs/tasks/issue-148-t1-impact-gate.md"
    - "docs/test-specs/issue-148-impact-classification.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-148-impact-classification.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/ci-impact.mjs"
    - "tools/ci-impact.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #148 変更影響分類・Fail-Closed evidence Gate 実装計画

## Goal

Synvelis 固有の impact 分類(class・依存伝播・期待 lane Mapping)と、
期待検証集合に対する fail-closed `evidence` gate を実装する。PR は
Shadow Mode(分類出力のみ・lane は全実行)で偽陰性監査証跡を残し、
required 10 contexts・main push/dispatch の網羅性を維持する。

## Scope

単一の実装 stream。変更は `tools/ci-impact.mjs`・`tools/ci-impact.test.mjs`
(新規)と `.github/workflows/ci.yml`・`mise.toml`(変更)の 4 ファイルに
閉じる。lane 実装・required contexts・lane `if:` 条件実行は含まない。

## Upstream Documents

- `docs/specs/issue-148-impact-classification.md`
- `docs/discovery/issue-148-evidence-fail-open-gap.md`

## Design Inputs

- `docs/designs/issue-148-impact-classification.md`(approved)

## File Map

- `tools/ci-impact.mjs` — LANE_LEGS(lane universe)・PATH_RULES・
  CRATE_DEPS/CRATE_CLASSES/FILE_CLASS_RULES・RT_OWNERS/GW_OWNERS・
  DECLARED_NOT_RUN と `classify`/`gate`/`lanes` subcommand。
- `tools/ci-impact.test.mjs` — node:test。path 規則・crate 伝播・
  trigger/fallback・mode・gate verdict 全判定経路。
- `mise.toml` — `ci-impact` / `ci-evidence-gate` / `test-ci-impact` task。
- `.github/workflows/ci.yml` — `classify` job 追加(fetch-depth 0・
  node のみ・自戒テスト+分類+output)、`evidence` job の改修
  (checkout/mise・report 永続化・jobs API 収集・gate 実行、job 単位
  `permissions: contents:read + actions:read`)、冒頭コメントに
  impact 分類の存在を追記。

## Implementation Sequence

- [x] `tools/ci-impact.mjs` 実装(分類→期待集合 JSON、gate 照合、
      markdown 証跡、`GITHUB_OUTPUT` slim 連携)。
- [x] `tools/ci-impact.test.mjs` 実装・全件 pass。
- [x] `mise.toml` に 3 task 追加。
- [x] `ci.yml`: `classify` job 追加、`evidence` を fail-closed gate に
      改修(needs に classify 追加・権限・3 step 構成)。
- [x] worktree 検証(node:test・actionlint・classify/gate の dry-run)。
- [ ] docs チェーン(discovery/spec/design/plan/test-spec/task +
      各 README index)整備。
- [ ] `git merge origin/main`・コミット・push・PR 作成・
      `gh pr checks --watch` で CI 緑を確認。

## Dependencies and Risks

- `gh api .../jobs` は `GITHUB_TOKEN` の `actions: read` が必要 →
  evidence job に job 単位権限を追加(他 lane 不変)。
- classify の `git diff base...head` は fetch-depth 0 が前提。
  base 未到達・push の zero `before` は tool 側 fallback で full CI。
- `GITHUB_OUTPUT` report は files 配列を count 化して縮約(slim)。
- job 名の完全一致前提: matrix leg 名を LANE_LEGS に固定し、未知 job は
  violation(fail-closed で drift 検出)。
- Windows/macOS leg の `skipped` 検出経路は jobs API 依存 — API 障害は
  leg 欠落扱いで fail(緑化しない)。

## Before / After・Rollback

### Before

- `evidence` は `needs.*.result` aggregate のみを見て `skipped` を無視
  (Fail-Open 経路あり)。期待検証集合・leg 単位結果・NotApplicable
  根拠・分類は存在しない。
- 変更内容に関係なく全 lane 実行(docs-only PR でも ~全 Runner 消費)。

### After(本変更 — Shadow 期間)

- 全 lane 従来どおり実行(skip 無効)+ `classify` が期待集合を宣言。
- `evidence` が leg 単位の Pass/Fail/NotRun/Skipped/Cancelled を照合し
  violation を fail。Shadow 照合表・Declared NotRun/Unsupported・
  head/base/ruviform SHA・lane 総 Runner 時間を証跡へ。
- 想定 Runner 時間差分: classify +〜1 分、evidence +〜40 秒のみ。
  lane 節減は selective 有効化後の別変更で計測(p50 20% は目標値)。

### Rollback

- 本変更の revert で即時完全復旧(lane `if:` 未導入のため実行形態は
  Before と同一)。`classify`/`evidence` の改修部分のみが消える。
- 恒常 kill switch 不要(条件実行未配線。導入時は lane `if:` 除去または
  repo var で切る想定を spec §移行に記録)。

## Task Breakdown

- `docs/tasks/issue-148-t1-impact-gate.md` — 上記 4 ファイル + docs
  チェーン + 検証/PR までを一括で担う単一 task(分類・gate・workflow
  が不可分のため分割しない)。

## Verification

- [x] `node --test tools/ci-impact.test.mjs` pass(28 件)。
- [x] `mise exec -- actionlint .github/workflows/ci.yml` pass。
- [x] `classify`/`gate` のローカル dry-run(files-file・合成 jobs JSON で
      required/not-applicable・skipped 検出・偽陰性記録を目視確認)。
- [ ] `mise exec -- prek run --all-files` pass。
- [ ] PR で `classify`・`evidence` が起動し緑、証跡に leg 表・Shadow
      照合・Declared NotRun・revision が出る。
- [ ] 既存 required 10 contexts の job 名が不変。

## Open Questions / Gaps

- ruviform #333 の汎用契約文書が未提供。入手時に本 spec の class 語彙・
  verdict 語彙・fallback 意味論との差異を確認し PR/追記で記録する
  (現状は Issue 本文仕様に完全一致する独自実装)。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution(4 ファイル・単一 task・不可分)。
- Independent work streams: なし。
- User approval required before dispatch: yes
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

## Review Handoff

- レビュー観点: LANE_LEGS が ci.yml job 名と完全一致するか、RT/GW
  OWNERS と dep graph の方向性(dependents 伝播)が crate Cargo.toml と
  一致するか、gate が全 violation 経路で exit 1 するか、evidence job の
  権限追加が最小化されているか。
