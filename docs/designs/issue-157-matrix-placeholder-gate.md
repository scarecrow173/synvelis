---
id: "DESIGN-034RUkhVNYe0G4XOqwwqOa"
type: "design"
status: "approved"
title: "Issue #157 rev.3 matrix lane skip placeholder — evidence gate 設計"
created: "2026-11-05"
updated: "2026-11-05"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/157"
    - "https://github.com/scarecrow173/synvelis/pull/161"
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: "tools/ci-impact.mjs"
      - type: "file"
        path: "tools/ci-impact.test.mjs"
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
  implemented-by:
    - "docs/plans/issue-157-matrix-placeholder-gate.md"
  depends-on:
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
    - "docs/designs/issue-148-impact-classification.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-157-selective-execution.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-157-matrix-placeholder-gate.md"
    - "docs/adr/issue-157-rev-3-skip-placeholder-gate-vs-workflow.md"
  derived-by:
    - "docs/plans/issue-157-matrix-placeholder-gate.md"
    - "docs/test-specs/issue-157-matrix-placeholder-gate.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-157-matrix-placeholder-gate.md"
  references:
    - ".github/workflows/ci.yml"
    - "tools/ci-impact.mjs"
    - "tools/ci-impact.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #157 rev.3 matrix lane skip placeholder — evidence gate 設計

## Context

`tools/ci-impact.mjs` の `evaluateGate` は jobs API の job 名を
`LANE_LEGS`/`OBSERVED_LEGS`/`evidence` と名照合し、未知名を fail-closed
の `unclassified job` violation にする。#157 の job-level `if:` skip により、
matrix lane(verify・gw10-verify・ui-verify)が skip されると GitHub は
leg を展開せず、未展開の name template `'<lane> (${{ matrix.os }})'` を
持つ placeholder job を 1 件だけ記録する(実測値・conclusion=skipped)。
この名は universe に存在しないため violation となる。

同時に、evidence job の収集カウント `expected` は
`lanes | wc -l + 1`(全 leg 展開前提の 14)で、skip 収縮(placeholder
1 件 + 非 matrix lane は通常名 skipped entry 1 件)を反映せず、
実 job 数 13 に対し bounded retry 4 回すべて不足する。

前提となる GitHub の観測事実(run 41894049161・PR #161 で実測):

- job-level `if:` が false の matrix job → `<name template>` が
  `${{ matrix.* }}` 未展開のまま 1 entry、status=completed・
  conclusion=skipped。
- job-level `if:` が false の非 matrix job(`ui-verify-macos` 等)→
  通常名で 1 entry、conclusion=skipped。
- したがって skip の有無に関わらず **lane/job あたり 1 entry** が
  現れる。収縮するのは matrix leg 数のみ。

## Scope

### In Scope

- `evaluateGate` の placeholder 分類と判定(rows 記録・violation 条件)。
- `expected-jobs` サブコマンドと `expectedJobs()` カウント関数。
- `ci.yml` evidence job の `expected` 式置換。
- 契約 pin テスト + run 41894049161 fixture 再生。

### Out of Scope

- LANE_LEGS/OBSERVED_LEGS の構成変更、lane 分割設計の変更。
- ruviform 側 gate、ruleset、selective/kill switch 意味論。

## Components and Boundaries

- `PLACEHOLDER_NAME_RE` — skip 収縮 placeholder の名パターン
  `^(.+?) \(\$\{\{[\s\S]*?\}\}\)$` を単一の定数として所有
  (名照合境界に閉じた知識として tools/ci-impact.mjs 内に置く)。
- `evaluateGate` — unclassified 検査を placeholder 分岐つきに拡張。
  lane universe の照合責務は従来どおりここに集約し、ci.yml 側へ
  判定ロジックを分散しない。
- `expectedJobs(impact)` — 期待 job 件数の計算を impact report から
  導出する純粋関数として tool 内に置き、CLI `expected-jobs` と
  テストが共有する。
- `ci.yml` — `expected` を `expected-jobs` 呼出し 1 行に置換する
  薄い配線のみ(判定・カウント意味論は tool 側)。

## Data and Control Flow

### placeholder 判定

```
for name in jobs.byName:
  known(LANE_LEGS/OBSERVED_LEGS/evidence) → skip
  match PLACEHOLDER_NAME_RE(文字列名のみ) → prefix P
    P が LANE_LEGS の lane でない → unclassified violation
    P のいずれかの leg entry が placeholder より新しい run_attempt
      → stale placeholder(過去 attempt の記録)として無視
    impact.lanes の lane=P の leg が全て scheduled===false
      かつ placeholder の verdict が skipped
      → rows に verdict="skipped-placeholder" 記録(violation なし)
    それ以外(1 leg でも scheduled/required・report に lane 無し・
      placeholder conclusion が skipped 以外)
      → violation(leg-level skipped / 意味不明レコードと同契約)
  上記以外 → unclassified violation(従来どおり)
```

判定は `expectation` ではなく **`scheduled` フラグ**を見る:
job-level `if:` は `lanes` 出力(= scheduled lane id 集合)を消費する
ため、placeholder の正当性は「その lane が schedule されなかったか」で
決まる。selective で expectation=required なら scheduled=true であり
両者は整合するが、契約の正本は scheduled 側。report に当該 lane の
leg が無い( malformed report )場合は scheduled とみなせず violation
— fail-closed。

placeholder job の conclusion は skipped のみ受理する: placeholder は
「skip の記録」であり実行結果ではないため、skipped 以外の conclusion を
持つ同形 entry は GitHub 側意味論のずれとみなし fail-closed(leg が
in_progress/failure で violation になるのと同じ扱い)。また placeholder
判定は attempt スコープ: 過去 attempt の placeholder が残留しても、
同 lane の leg がより新しい attempt で展開されていれば stale として
無視する(re-run-all で selective が無効化されたロールバック経路を
誤 fail させない)。

### expected-jobs カウント

```
expected = scheduled leg 数(impact.lanes の scheduled===true)
         + 非 scheduled lane 数(scheduled===false の leg を持つ lane 数、各 1 entry)
         + OBSERVED_LEGS 数
         + 1 (evidence 自身)
```

- scheduled lane は leg 数分展開される。非 scheduled lane は
  matrix→placeholder 1 件 / 非 matrix→skipped entry 1 件で常に 1 entry。
- OBSERVED_LEGS(`*-macos`)は lane `if:` を共有するため skip されても
  非 matrix の skipped entry として 1 件残る — 常に 3 件。
- impact report 非読取/形式不正 → `LANE_LEGS + OBSERVED_LEGS + 1`
  (最大 universe = 14)に fail-safe。`lanes` が全 LANE_LEGS の name を
  覆わない(空・部分・非配列)場合も「形式不正」として同じ fallback
  — under-expect は件数不足の早期打切りで absent job を見逃しうるため
  信用しない。over-expect は retry 経費のみ。
- jobs API fetch は `--paginate` + `.jobs[]` 展開を slurp で単一配列に
  戻す: `filter=all` は全 attempt の entry を返すため、per_page=100
  超過時に古い attempt 側で打ち切られる fail-open 余白を塞ぐ。
- 検証値: PR #161 型(ui-verify のみ非 schedule)→ 8+1+3+1=13、
  全 schedule → 10+0+3+1=14、docs-only → 1+6+3+1=11。

## Non-Functional Constraints

- fail-closed 維持: 本変更で新たに violation にならない入力は
  「scheduled=false が report で裏付けされた lane の placeholder」のみ。
- gate の判定は jobs API の name/conclusion と impact report のみに依存;
  workflow 側へ新たな権限・API 呼出を追加しない。
- `expected-jobs` は retry の「十分性ヒント」であり verdict を変えない
  — 件数不足でも gate は absent leg を fail-closed で検出する。

## Risks and Trade-offs

- GitHub が placeholder 名形式を将来変えるリスク: prefix 一致しない
  新形式は unclassified violation → fail-closed で検出(緑化しない)。
  緩い式受理との緩和は §Deferred。
- `scheduled` フラグを正本にする選択: expectation との不整合がある
  壊れた report では placeholder を過剰受理しうるが、その場合 leg 側の
  required 判定(expectation)が先に violation を出すため実害はない。
- `expected` 計算が実際の job 数を過小に見ると収集が早く打ち切られる
  リスクは、カウントを「schedule 側だけ leg 展開」に正確化することで
  除去。反対方向(過大)は retry 経費のみ。

## Deferred Design Concerns

- 式内容の照合 | spec §Deferred と同じ。
- 非 matrix lane の placeholder 風名を lane 判定から除外するか
  (現行は lane 一致なら受理 — GitHub は生成しないが受理しても
  scheduled 判定で正当性は保たれる) | 観測なし | 実害報告時 |
  GitHub が将来非 matrix job に placeholder を出した場合に備えても
  現行で fail-closed は保たれる。

## Verification Notes

- [ ] unit test(a)〜(e)が spec の AC を pin する。
- [ ] run 41894049161 の実 jobs fixture で evaluateGate が ok=true。
- [ ] `expected-jobs` 出力を手動で実 run 件数と突合(13/14/11)。
