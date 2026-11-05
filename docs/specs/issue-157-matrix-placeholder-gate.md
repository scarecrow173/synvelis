---
id: "SPEC-034RUjwAo6z9Q2Hpkp99SH"
type: "spec"
status: "superseded"
title: "Issue #157 rev.3 matrix lane skip placeholder — evidence gate 修正"
created: "2026-11-05"
updated: "2026-11-05"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/157"
    - "https://github.com/scarecrow173/synvelis/pull/161"
    - "https://github.com/scarecrow173/synvelis/actions/runs/41894049161"
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
  implements: []
  implemented-by:
    - "docs/designs/issue-157-matrix-placeholder-gate.md"
    - "docs/plans/issue-157-matrix-placeholder-gate.md"
  depends-on:
    - "docs/specs/issue-157-selective-execution.md"
  blocks: []
  supersedes: []
  superseded-by:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  related:
    - "docs/specs/issue-148-impact-classification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-157-selective-execution.md"
  derived-by:
    - "docs/designs/issue-157-matrix-placeholder-gate.md"
    - "docs/plans/issue-157-matrix-placeholder-gate.md"
    - "docs/adr/issue-157-rev-3-skip-placeholder-gate-vs-workflow.md"
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

# Issue #157 rev.3 matrix lane skip placeholder — evidence gate 修正

> **Superseded (2026-11-05)**: lane `if:` skip が撤去されたため skip
> collapsed placeholder は発生しなくなった。`ci-summary` gate は
> placeholder 形の名前を unclassified violation として拒否する
> (fail-closed)。現行仕様は `docs/specs/issue-166-full-ci-ci-summary.md`
> を参照。本書は実装経緯の履歴として保持する。

## What Should Be Built

#157 で導入した selective execution(lane job の `if:` skip)と #148 の
fail-closed evidence gate の組合せ欠陥を修正する。matrix lane が
job-level `if:` で skip されると GitHub は leg を展開せず、Jobs API に
`<job名> (${{ matrix.os }})` という **skip 収縮 placeholder job** を
1 件だけ記録する(実測: run 41894049161 の
`ui-verify (${{ matrix.os }})`・job id 126798807824・
conclusion=skipped)。gate の unclassified 検査はこの名を未知 job として
violation にし、収集カウント `expected` も skip 収縮を考慮しないため、
全 required leg 緑の run でも evidence が fail する。

修正は 2 点:

1. `evaluateGate` が placeholder 名を「skip 収縮 placeholder」として分類
   し、当該 lane の全 leg が impact report で `scheduled: false` なら
   正当(skip 済み lane の記録)として受理、required/scheduled leg が 1
   つでもあれば「required lane skipped」violation とする。prefix が
   どの lane にも一致しない名前は従来どおり unclassified violation。
2. `ci.yml` evidence job の収集カウントを、skip 収縮を反映した
   `expected-jobs` サブコマンド出力に置き換える。

## Why Now

- PR #161(run 41894049161)で再現済み: 全 required leg 緑・macOS
  queued にも関わらず `evidence` が fail。selective execution 有効化後
  最初の被害 PR で、skip が発生するすべての後続 PR を塞ぐブロッカー。
- selective mode で lane skip は正常経路となった以上、placeholder は
  恒久的に出現しうる jobs API 入力であり、gate が受理できないのは
  gate 側の欠陥。

## Users and Value

- PR author / maintainer: 正当な skip を含む run が evidence gate を
  通り、真の fail-closed 判定(required lane skip・未知 job)のみが
  fail になる。
- gate 証跡の読み手: placeholder が rows に「skipped-placeholder」系の
  記録として残り、leg 欠落(absent)と区別できる。

## Scope

### In Scope

- `tools/ci-impact.mjs`: `evaluateGate` の placeholder 分類・判定、
  `expected-jobs` サブコマンド追加。
- `tools/ci-impact.test.mjs`: 上記契約の pin + run 41894049161 の
  jobs API fixture 再生。
- `.github/workflows/ci.yml` evidence job: `expected` カウント式の
  置換とコメントの実態整合。

### Out of Scope

- ruviform 側 `ci-gate`(`needs` コンテキスト評価であり本欠陥は
  synvelis の jobs API 名照合に固有)。
- lane 分割設計の変更(per-OS job 化等)、ruleset context の変更。
- selective execution の有効/無効切替・kill switch 意味の変更。

## Requirements

- placeholder 名の認識は `^<prefix> (${{ <式> }})$` パターン(実測
  ` (${{ matrix.os }})`、スペース・式の揺らぎを許容する正規表現)とする。
- `<prefix>` が `LANE_LEGS` の `lane` に一致する場合、当該 lane の全
  leg が impact report で `scheduled: false` のときのみ正当。placeholder
  は violation にせず、rows/observed に skipped-placeholder 系の記録を
  残す(leg 自体は absent + not-required で既に無 violation)。
- 当該 lane に required/scheduled leg が 1 つでもあれば violation —
  leg-level `skipped` と同じ「required lane skipped」契約。
- `<prefix>` がどの lane にも一致しない placeholder 風の名、および
  placeholder パターンに合わない未知名は従来どおり unclassified
  violation(fail-closed 維持)。
- `expected-jobs` は `scheduled leg 数 + 非 scheduled lane 数(各 1
  entry)+ OBSERVED_LEGS 数 + 1(evidence 自身)` を返す。impact
  report を読めない場合は最大 universe へ fail-safe 側に倒す
  (over-expect は bounded retry 経費のみ、under-expect は eventual
  consistency 下で job 欠落を見逃す方向のため)。
- fail-closed は維持: 未知 job・required leg の fail/cancelled/absent/
  skipped の violation 意味を弱めない。

## Acceptance Criteria

- [ ] `node --test tools/ci-impact.test.mjs` pass:
  - (a) `ui-verify (${{ matrix.os }})` placeholder + 全 leg
    not-applicable → violation なし
  - (b) 同 placeholder + required leg 存在 → violation
  - (c) 未知名 `foo (${{ matrix.os }})` → unclassified violation
  - (d) `expected-jobs` が skip 時 13・非 skip 時 14 を返す
  - (e) 既存テスト全件非回帰
- [ ] run 41894049161 の jobs API 出力を fixture として再生し、
  evaluateGate が violation なしで pass する(補助検証)。
- [ ] `mise run lint-actions` pass。
- [ ] `mise run test-ci-impact` pass。
- [ ] PR で evidence が placeholder 経由の run で緑になる
  (本 PR 自体は tools/ci 変更のため fallback 全 lane 実行;
  placeholder 受理は unit test + fixture で立証)。

## Deferred Design Concerns

- placeholder の matrix 式内容の照合(式が matrix.os であることまで
  検査するか) | 実害がなく語彙が細分化するだけ | GitHub の placeholder
  名仕様が変わったとき | 式を緩く受理しすぎて偽 placeholder を合法化
  する(現行は prefix 一致 + scheduled 判定で実害を抑止)。

## Sources and Relations

- 欠陥 Issue: https://github.com/scarecrow173/synvelis/issues/157
- 初の被害 PR: https://github.com/scarecrow173/synvelis/pull/161
- 再現 run: https://github.com/scarecrow173/synvelis/actions/runs/41894049161
  (jobs API 実測: `ui-verify (${{ matrix.os }})` skipped・全 13 job)
- 前提仕様: `docs/specs/issue-157-selective-execution.md`、
  `docs/specs/issue-148-impact-classification.md`
