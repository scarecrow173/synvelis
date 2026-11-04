---
id: "DESIGN-034SVCIOPT02kX3pLwQs"
type: "design"
status: "approved"
title: "Issue #148 変更影響分類・Fail-Closed evidence Gate 設計"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/148"
    - "docs/specs/issue-148-impact-classification.md"
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
    - "docs/specs/issue-148-impact-classification.md"
  implemented-by:
    - "docs/plans/issue-148-impact-classification.md"
  depends-on:
    - "docs/specs/issue-148-impact-classification.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ci-synvelis-pr-push-regression-gate.md"
    - "docs/designs/issue-151-ui-ci-lane.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-148-impact-classification.md"
  derived-by:
    - "docs/plans/issue-148-impact-classification.md"
    - "docs/test-specs/issue-148-impact-classification.md"
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/ci-impact.mjs"
    - "tools/ci-impact.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #148 変更影響分類・Fail-Closed evidence Gate 設計

## Context

`docs/specs/issue-148-impact-classification.md` が定める分類契約・
Mapping・Gate 意味論を、検証ロジックを mise task + tools/*.mjs に集約する
既存境界(Issue #83・#151 の構成)に整合させて実装する。lane 実装
(verify/rt/gw10/msrv/ui/dep-guard)と required contexts は変更しない。

## Scope

### In Scope

- `tools/ci-impact.mjs`(新規): lane universe・path→class 規則・crate
  dep graph・Declared NotRun registry を単一ファイルに所有し、
  `classify` / `gate` / `lanes` の 3 subcommand を提供する。
- `tools/ci-impact.test.mjs`(新規): node:test による規則・伝播・
  fallback・gate verdict の単体テスト。
- `ci.yml`: `classify` job 追加、`evidence` job を fail-closed gate に
  改修、evidence job のみ `actions: read` 権限を追加。
- `mise.toml`: `ci-impact` / `ci-evidence-gate` / `test-ci-impact` task。

### Out of Scope

- lane への `if:` 条件実行・Ruleset 変更・Runner 最適化の追加施策。
- ruviform 側の変更・#333 共有部品の取込(入手次第で整合確認)。

## Components and Boundaries

- `tools/ci-impact.mjs` — authoritative 実装。分類規則・依存 graph・
  lane universe・Declared NotRun を一元管理し、spec・workflow・test が
  同じ知識を複製しない(DRY)。外部依存ゼロ(node 標準 lib のみ)。
  - `classify`: 変更ファイル集合 → classes・affected crates・期待 lane
    集合(expectation + reason)の JSON report。入力は
    `--files-file` または `--base/--head`(内部で `git diff --name-only
    base...head`、失敗時は two-dot → それでも駄目なら full CI fallback)。
  - `gate`: impact report + Actions jobs API の job 配列を照合し、
    violation があれば `::error::` + exit 1。証跡 markdown を
    `$GITHUB_STEP_SUMMARY` へ出力。
  - `lanes`: lane universe の列挙(監査・デバッグ用)。
- `classify` job(ci.yml) — orchestration のみ。checkout(fetch-depth 0)
  → mise(node のみ有効化)→ `test-ci-impact` 自戒テスト → event 別に
  mode/base/head を解決して `mise run ci-impact`。report JSON は
  job output(`report`, class の files 配列を count に縮約)で
  `evidence` へ渡し、人間向け表は step summary へ。
- `evidence` job(ci.yml) — `needs` に `classify` を追加。checkout +
  mise(node)で tools を実行可能にし、①classify output を
  `impact-report.json` へ永続化、②`gh api .../jobs?filter=all` でこの
  run の leg 単位結果を `run-jobs.json` へ収集、③`mise run
  ci-evidence-gate` で fail-closed 照合。job 単位の
  `permissions: contents:read + actions:read` を宣言(他 lane の
  既定 `contents: read` は不変)。
- mise task — 薄い委譲(`node tools/ci-impact.mjs ...`)。引数は
  workflow 側で env(`CI_IMPACT_*`)渡しにし、mise task の引数伝播へ
  依存しない。

## Data and Control Flow

```
pull_request (shadow) / push, dispatch (full)
        │
        ▼
  classify job ── impact report(JSON, lanes[].expectation) ──▶ job output
        │                                                        │
        ▼ (全 lane 従来どおり実行 — skip 無効)                      │
  verify×3 / rt-verify / gw10-verify×3 / msrv-verify /            │
  ui-verify×3 / dep-guard                                         │
        │                                                        │
        ▼                                                        ▼
  evidence job ── gh api jobs?filter=all ──▶ gate(impact report ×
        実 job conclusion の leg 単位照合)──▶ verdict + summary + exit
```

- shadow mode: 分類の `not-applicable` 集合は would-skip として shadow
  照合表に載り、実結果が fail/cancelled なら「偽陰性」として violation
  も兼ねる(実害は無いが分類の誤りを merge 前に可視化)。
- gate の violation 条件: required leg の `skipped`/`fail`/`cancelled`/
  `not-run`/leg 欠落、selective での無根拠 skip、NotApplicable leg の実
  失敗(偽陰性)、lane universe 外の未知 job、impact report 欠落。
- `pickJob`: 同名 job の複数 attempt から `run_attempt` 以下の最大を選択
  (rerun 対応)。

## Non-Functional Constraints

- fail-closed: Gate の判定に使う情報(report・jobs API)が取れない場合は
  全て violation(`impact report missing or malformed`、leg 欠落)。
  `continue-on-error`・`if:` による gate 回避は導入しない。
- required contexts 不変: job 名 `verify`/`rt-verify`/`gw10-verify`/
  `msrv-verify`/`ui-verify`/`dep-guard`/`evidence` を変更・削除しない。
  `classify` は追加 job(evidence `needs` 経由で必須化と等価)。
- 権限最小化: `actions: read` は evidence job のみ。classify は権限追加
  なし(git diff のみ)。
- 再現性: node は `mise install --locked node` で pin。tool は
  stdlib のみで追加依存を持たない。
- Runner 時間: classify は checkout+node install+分類で ~1 分未満を想定。
  evidence は +checkout+node install+API 1 回で従来 +30〜40 秒程度。

## Risks and Trade-offs

- Actions jobs API 依存: API 障害時は leg 欠落扱いで fail(復旧は rerun)。
  per-leg 精度を `needs.*.result` aggregate で代替しないのは、skipped の
  leg 特定と Runner 時間集計が本 Issue の要件だから。
- `GITHUB_OUTPUT` 経由の report 受渡はサイズ上限対策として class 毎の
  files を count に縮約する(slim 化)。完全版は classify の step
  summary と job log で監査可能。
- Shadow 照合で偽陰性が観測されても lane 実結果が fail なら run は既に
  赤であり、分類の誤りが黙って通ることはない(記録の二重化)。
- matrix leg 名のハードコード: `verify (ubuntu-latest)` 等を LANE_LEGS
  に固定。未知 job を violation にすることで drift を fail-closed に検出。

## Deferred Design Concerns

- selective mode の有効化(lane `if:` + Ruleset 移行): spec §移行 の手順で
  別変更。機構(gate の selective 判定・NotApplicable 認可 skip)は本
  実装に含め検証済みだが配線はしない。
- jobs API の pagination(>100 legs)・check-runs API 併用: 現行 14 job
  規模では不要。lane 増加時に per_page 上限監視を再検討。

## Verification Notes

- [ ] `mise run test-ci-impact` が pass(28 ケース)。
- [ ] `mise run lint-actions` が ci.yml 変更後も pass。
- [ ] `mise exec -- node tools/ci-impact.mjs classify --files-file <f>`
      で代表パターンの期待集合を手検証。
- [ ] PR で `classify` 緑・`evidence` 証跡に leg 表 + Shadow 照合 +
      Declared NotRun + revision が出る。
- [ ] required 10 contexts の構成を弱めていない(job 名不変)。
