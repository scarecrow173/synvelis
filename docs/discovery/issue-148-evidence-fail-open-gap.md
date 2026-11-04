---
id: "DISC-034SVCIOPT02mQ3rTzAb"
type: "discovery"
status: "resolved"
title: "Issue #148 evidence skipped 許容の Fail-Open 経路と変更影響分類欠如"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/148"
  changes:
    added: []
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/issue-148-impact-classification.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/issue-151-ui-ci-lane-gap.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-148-impact-classification.md"
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
  defers: []
  deferred-by: []
---

# Issue #148 evidence skipped 許容の Fail-Open 経路と変更影響分類欠如

## Problem Statement

synvelis CI(Issue #83 系)は required 10 contexts と集約 `evidence` で
fail-closed を意図しているが、2 つの構造的 gap がある。

1. **`skipped` の黙認(Fail-Open)**: `evidence` の
   "Enforce not-run-is-not-pass" step は `needs.*.result` の `skipped` を
   成功にも失敗にも数えない(`skipped) ;;`)。GitHub required checks は
   skipped check を成功扱いするため、将来 lane job に `if:` 条件を入れた
   瞬間、全 lane が skip されても `evidence` だけが緑になり merge が
   通る経路が成立する。現在 job に `if:` は無いが、CI-OPT 系で条件実行を
   入れること自体が本 Issue の目的であり、Gate 側が先に fail-closed で
   なければならない。
2. **変更影響分類の欠如**: 変更と無関係な検証(例: docs 変更での
   GW-10 3OS、ui 未変更での ui-verify)が毎回走り、wall time・Runner
   時間を消費する。分類→必須 lane の Mapping が無く、削減の安全性を
   評価する基盤も無い。

## Current Behavior

- `evidence` は `needs.*.result` の aggregate のみを見る。matrix leg 単位の
  skipped/cancelled の区別、期待検証集合の宣言、NotApplicable の根拠付けは
  存在しない。
- `verify`(3OS)・`rt-verify`・`gw10-verify`(3OS)・`msrv-verify`・
  `ui-verify`(3OS)・`dep-guard` が全 trigger で無条件実行。
- NotRun lanes は証跡表に静的記載のみで、能力別(Browser/headless/実
  WebView)の分離や `Unsupported` の区別が無い。

## Expected Behavior

- 期待検証集合が分類 report として宣言され、実ジョブ結果(leg 単位の
  conclusion)との照合が `evidence` で行われる。
- `skipped`/`cancelled`/`failure`/未実行が required leg で即 fail。
  `NotApplicable` は分類根拠付きでのみ許容される。
- 分類不能・共有依存/設定変更は Full CI fallback で保守側に倒れる。
- PR は Shadow Mode で分類精度(偽陰性)を計測でき、skip 有効化判断の
  証跡になる。

## Constraints and Evidence

- ruviform #333 は汎用 Impact 契約の Owner だが、2026-11-04 時点で
  ruviform main に契約文書・共有実装は未マージ(open PR も無し)。
  Consumer である Synvelis は Issue #148 本文の契約仕様で自前実装し、
  後から整合確認する運用が Issue で明示されている。
- required 10 contexts は Ruleset `main-protect`(active)で必須化済み。
  追加で `branding` context が必須登録されているが対応 check run は
  存在しない(残留登録と思われる。本件スコープ外、記録のみ)。
- lane job への `if:` 追加は本変更では行わない(skip 無効が前提条件)。

## Outcome

`docs/specs/issue-148-impact-classification.md` に分類・Mapping・
Gate 契約を定義し、`tools/ci-impact.mjs` に authoritative 実装を集約する
方針で解決。`evidence` の skipped 黙認を除去し leg 単位照合へ改修する。
