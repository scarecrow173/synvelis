---
id: "SPEC-034SVCIOPT02bN7mQwErTy"
type: "spec"
status: "superseded"
title: "Issue #148 変更影響分類・Fail-Closed evidence Gate 仕様"
created: "2026-11-04"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/148"
    - "https://github.com/scarecrow173/ruviform/issues/333"
    - "docs/discovery/issue-148-evidence-fail-open-gap.md"
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
  implements: []
  implemented-by:
    - "docs/designs/issue-148-impact-classification.md"
    - "docs/plans/issue-148-impact-classification.md"
  depends-on:
    - "docs/discovery/issue-148-evidence-fail-open-gap.md"
  blocks: []
  supersedes: []
  superseded-by:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  related:
    - "docs/specs/ci-synvelis-pr-push-regression-gate.md"
    - "docs/specs/issue-151-ui-ci-lane.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-148-evidence-fail-open-gap.md"
  derived-by:
    - "docs/designs/issue-148-impact-classification.md"
    - "docs/plans/issue-148-impact-classification.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-148-impact-classification.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "https://github.com/scarecrow173/synvelis/blob/3f4f4450131b0258a1919b213de3ad394fbbb709/tools/ci-impact.mjs"
    - "https://github.com/scarecrow173/synvelis/blob/3f4f4450131b0258a1919b213de3ad394fbbb709/tools/ci-impact.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #148 変更影響分類・Fail-Closed evidence Gate 仕様

> **Superseded (2026-11-05)**: 変更影響分類・選択実行機構は Issue #166
> stage 1 で撤去され、fail-closed 集約は `tools/ci-summary.mjs` が
> 引き継いだ。現行仕様は `docs/specs/issue-166-full-ci-ci-summary.md`
> を参照。本書は実装経緯の履歴として保持する。

## What Should Be Built

Synvelis 製品固有の変更影響分類(Impact Classification)と、期待検証集合に
対する Fail-Closed 集約 `evidence` Gate を実装する。分類は変更ファイル
→ impact class → crate 依存伝播 → 期待 lane/OS 集合を決定し、PR では
Shadow Mode(分類結果の出力のみ、lane は全実行)で Full CI との照合
証跡を残す。`evidence` は期待集合と実ジョブ結果を leg 単位で照合し、
`skipped` を GitHub required checks の意味(成功扱い)に委ねない。

本仕様は ruviform [#333](https://github.com/scarecrow173/ruviform/issues/333)
の汎用 Impact 分類契約の Consumer 側実装である。#333 の契約文書は
本実装時点で未提供(ruviform main に契約文書・共有部品なし、
関連 PR 未作成)のため、Issue #148 本文の契約仕様に忠実に実装し、
#333 契約が入手でき次第の差異確認を PR 証跡として記録する。両者で
共有する語彙は `Pass / Fail / NotRun / Unsupported / NotApplicable`・
Shadow Mode・Fail-Closed Gate・fallback to Full CI。

## Why Now

現行 `evidence` は lane 結果の `skipped` を判定上スルーする
(`skipped) ;;`)ため、将来 lane に条件実行を入れた瞬間に job skip が
required check を緑にする Fail-Open 経路が成立する。また変更と無関係な
検証が毎回走り Runner 時間を消費している。分類・Gate・安全移行は
不可分の実装単位(#333 と同じ設計判断)。

## Users and Value

- synvelis 開発者: 変更に必要な検証集合が PR 上で監査可能になり、
  skip 導入時の安全性が shadow 証跡で裏付けられる。
- maintainer: `evidence` 証跡が leg 単位の Pass/Fail/NotRun/NotApplicable
  と根拠・対象 revision を示し、skipped の黙認が構造的に不可能になる。

## Scope

### In Scope

- Synvelis 固有 impact class 定義・file→class 規則・crate 依存伝播・
  class/crate→期待 lane/OS/feature/fixture Mapping(本書 §Mapping)。
- `tools/ci-impact.mjs`: `classify`(分類→JSON report)と `gate`
  (期待集合 vs 実結果の fail-closed 照合)を一ファイルに所有する
  authoritative 実装 + node:test スイート。
- `ci.yml`: `classify` job 追加、`evidence` job の gate 化
  (Actions jobs API による leg 単位結果収集、`actions: read` 権限)。
- Shadow Mode 照合レポート(classifier would-skip 集合 vs 実結果の
  偽陰性監査)を evidence 証跡へ常設。
- Declared NotRun/Unsupported 表の能力別分離(WebView 行の追加・
  陳腐化した ruviform issue 参照の更新)。

### Out of Scope

- lane job への `if:` 条件実行配線(selective execution)。偽陰性なしが
  Shadow で確認されるまで skip は無効のまま(本変更では lane を一切
  skip しない)。
- Ruleset 必須 context の変更(移行計画は §移行 に記録し、本変更では
  required 10 contexts の名称・構成を維持する)。
- ruviform 側の変更・共有部品導入・SolidJS 依存の ruviform への持込。
- ruviform candidate SHA の両 Consumer 互換検証(ruviform #294 の責務)。

## Requirements

- R1 分類は保守側に倒す: 不明・分類不能・共有依存/設定の変更は
  Full CI へ fallback し、検証を過小にしない。
- R2 lane skip は導入しない: 本変更後も全 lane が従来どおり実行される。
  条件実行の有効化は Shadow 証跡の偽陰性評価後に別変更で行う。
- R3 Gate は fail-closed: Pass 以外(Fail/Cancelled/NotRun/Skipped/
  未知 conclusion)は violation。`NotApplicable` は分類 report が
  根拠付きで宣言した lane のみ許す。
- R4 lane universe は ci.yml に対して total: run 内に未知の job が現れたら
  violation(lane 追加時に mapping 更新を強制する)。
- R5 `classify`/`evidence` 自体も検証対象: classify の失敗・report 欠落は
  gate fail とし、分類不能 run を緑にしない。
- R6 対象 revision を集約: head/base SHA・ruviform resolved SHA・
  run/attempt・分類 mode を証跡に含める。
- R7 RT/Gateway/MSRV/GUI の保護を弱めない: rt_ strict prefix ガード・
  GW-10 harness・MSRV lane・ui-verify を Mapping 上で対応 class に
  必須化し、既存 lane 実装は変更しない。
- R8 検証ロジックは mise task 経由(`ci-impact`/`ci-evidence-gate`/
  `test-ci-impact`)。workflow 側は薄いシェルのみ。

## Impact Classification

### Classes

Synvelis Owner の class 集合(Issue 指定 + `docs`):

`project` `recording` `recovery` `playback` `control` `automation`
`plugin` `graph` `rt` `gateway` `ipc` `webview` `solidjs-gui` `ci`
`cargo-lock` `toolchain` `docs`

### Path → class 規則(順序付き・先勝ち)

| path pattern | class | trigger |
|---|---|---|
| `Cargo.lock`, `Cargo.toml`, `crates/*/Cargo.toml` | `cargo-lock` | Full CI |
| `mise.toml`, `mise.lock`, `.mise/*`, `rust-toolchain*` | `toolchain` | Full CI |
| `.github/*`, `tools/*`, `.pre-commit-config.yaml` | `ci` | Full CI |
| `ui/*` | `solidjs-gui` | — |
| `crates/<name>/*` | crate 既定 class(下表)+ file 細則 | — |
| `docs/*`, `*.md`, `LICENSE*`, `assets/*`, git/agent 設定系 | `docs` | — |
| 上記以外の全 path | — | Full CI (fallback) |

### Crate → class・依存伝播

| crate | classes | rt owner | gw10 owner |
|---|---|---|---|
| synvelis-project | project, control, automation | ✓(pc_b_rt) | — |
| synvelis-recording | recording, recovery(+file 細則で rt) | ✓(vs_b_rt_alloc) | — |
| recording-poc | recording, recovery | — | — |
| synvelis-player | playback, graph, plugin | — | — |
| synvelis-render | graph, playback | — | — |
| synvelis-gateway | gateway, ipc | — | ✓ |
| mcp-poc | gateway, ipc | — | ✓(gw10_transport) |
| synvelis-workspace | control, ipc | — | — |
| synvelis-app | webview, ipc | — | — |

file 細則: recording の `journal/recovery/faultinj/commit.rs`・`vs_c/vs_d`
test → `recovery`。`gate/capture/clock/engine/handoff/bounded/mock/plan/ops`・
`vs_b*` test → `rt`。project の `pc_b_rt.rs` → `rt`。
app の `service/ui_ops/host/app.rs` → `ipc`、`assets/host.rs`・`build.rs` →
`webview`。

伝播: 変更 crate の内部 path-dep 依存**被依存側**(dependents)を
推移的に affected とする(変更は上流から下流へ影響する)。
`synvelis-project` 変更 → 全 crate affected。`synvelis-recording` 変更 →
recording-poc/player/render。`synvelis-gateway` 変更 → mcp-poc/workspace/app。

### Class/Crate → 期待 lane/OS/feature/fixture Mapping

| 条件 | 必須 lane(全 OS leg) |
|---|---|
| 常時 | `classify`, `dep-guard`(docs-only 変更では NotApplicable) |
| 任意 crate が changed または affected | `verify` ×3 OS, `msrv-verify` |
| affected ∩ RT owners ≠ ∅ | `rt-verify`(strict prefix・zero-test ガードは lane 内) |
| affected ∩ GW owners ≠ ∅ | `gw10-verify` ×3 OS(GW-10 harness) |
| `solidjs-gui` class | `ui-verify` ×3 OS |
| trigger class または fallback または full mode | 全 lane |

feature 軸: `webview` class の実窓 host 検証は capability 分離により
Declared `Unsupported`(headless 面は verify が担保。`check-app-webview`
等はローカル task)。fixture 軸: GW-10 rig fixture は `gw10-verify` lane が
所有し分類外。Browser/headless/実 WebView の lane 導入は
Declared NotRun 表の各行が追う。

### Mode

- `pull_request` → `shadow`: 分類・期待集合・would-skip 照合を出力。
  lane は全実行し skip は無効。
- `push: main` / `workflow_dispatch` → `full`: 全 lane 必須(従来維持)。
- `selective`: 将来の有効化 mode。分類の `required` のみ実行し、
  `not-applicable` lane は根拠付きで skip を許す。本変更では未使用。

### Gate verdict 語彙

| leg の実結果(job conclusion) | verdict | required 時 | selective NotApplicable 時 |
|---|---|---|---|
| completed/success | `Pass` | ok | shadow 照合対象 |
| completed/failure・timed_out・action_required・startup_failure | `Fail` | violation | violation(偽陰性) |
| completed/cancelled | `Cancelled` | violation | violation(偽陰性) |
| completed/skipped | `Skipped` | violation(必要 skip 拒否) | 許容(根拠付き skip) |
| 未到達/未完了/未知 conclusion/leg 欠落 | `NotRun`/`Fail` | violation | 許容(未実行として) |
| job が lane universe 外 | — | violation(mapping 不完全) | 同左 |

## Acceptance Criteria

- [ ] `node --test tools/ci-impact.test.mjs` が pass(分類規則・伝播・
      fallback・gate verdict を pin)。
- [ ] PR の `classify` job が期待 lane 集合 JSON + summary 表を出力し、
      `evidence` が leg 単位 verdict・Shadow 照合・Declared NotRun・
      revision を証跡に含める。
- [ ] 任意 leg の skipped/cancelled/failure で `evidence` が fail する
      (Fail-Open でない)。
- [ ] required 10 contexts の job 名を変更・削除していない。`classify`・
      `evidence` の追加・改修で保護構成を弱めない。
- [ ] main push/dispatch は全 lane 実行(full mode)を維持。
- [ ] RT 変更需要条件: rt owner affected、GW-10: gw owner affected、
      MSRV: 任意 crate changed/affected、UI: solidjs-gui — Mapping 通り。

## Deferred Design Concerns

- selective execution の有効化: Shadow で複数 PR の偽陰性ゼロ確認後、
  lane `if:` 配線と Ruleset required contexts の原子移行を別変更で行う
  (§移行)。本変更では機構のみ導入する。
- impact report の artifact 化・Runner 時間の継続計測ダッシュボード化
  (現状は job summary の集計行で代替)。

## 安全移行(Shadow → selective → Ruleset)

現行 required contexts(`verify`/`gw10-verify` 各 3OS・`rt-verify`・
`msrv-verify`・`dep-guard`・`evidence` + ruleset 残留 `branding`)は、
job skip を「成功」として扱う GitHub セマンティクスの上に立つ。無保護
期間なしの移行手順として以下を定める(本変更では実施しない):

1. Shadow 運用で偽陰性ゼロを確認(複数 PR・各 class 網羅)。
2. `evidence` gate が selective 判定に対応済みであること(本変更で導入済み)。
3. lane job に `if:` 条件を追加する変更と、Ruleset から leg 個別
   required context を外し `evidence`+`classify` を必須とする更新を
   同一期間に行う。`evidence` は常時必須のため無保護期間は生じない。
4. `branding` は本件スコープ外の残留 context として記録のみ行う
   (発生源不明・別途 owner 判断)。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/148
- 汎用契約 Owner: https://github.com/scarecrow173/ruviform/issues/333
- Discovery: `docs/discovery/issue-148-evidence-fail-open-gap.md`
- 上流: `docs/specs/ci-synvelis-pr-push-regression-gate.md`(Issue #83 系)
- 整合対象: ruviform CI-OPT-00 #327・Ruleset #145(synvelis)
