---
id: "EXP-wfsUpQNkKIY5yKb2sHeCj5"
type: "experiment"
status: "completed"
title: "Issue #514 experiment log — 判定検討の試行記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/514"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/exp/issue-514-session-engine-selection-exp.jsonl"
  implements:
    - "docs/plans/issue-514-session-engine-selection.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-514-session-engine-selection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/issue-514-session-engine-selection.md"
    - "docs/impl/exp/issue-514-session-engine-selection-exp.jsonl"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #514 experiment log

判定分岐の検討過程を記録する。詳細な試行データは
`issue-514-session-engine-selection-exp.jsonl` を参照。

## EXP-1: rebuild を `drive_selection_plan` に置換する案の検討

- 仮説: selection を消費すれば missing-plugin substitution・typed
  verdict・reconciler receipt が app 層で得られる。
- 結果: **棄却**。`SelectionInputs` の production 供給経路がなく、
  空 evidence は `MissingPluginEvidence` fail-closed、部分 evidence
  の gate 化は `ProcessorHost::connect`（ground truth）より弱い
  情報で false-block を持ち込む。substitution は plan shaping に
  使われず published plan は変わらない。

## EXP-2: selection を「観測のみ」で rebuild に併走する案の検討

- 仮説: verdict で gate せず receipt だけ採用すれば安全。
- 結果: **棄却**。consumer がなく、evidence 供給がないため verdict
  は常に fail-closed 系。コストと複雑性のみ増える。

## EXP-3: engine 側に selection 消費を移す案の検討

- 仮説: `RoutingEngine` が selection を直接消費すれば adapter は
  不要。
- 結果: **棄却**。依存方向（workspace → player）に反し、engine は
  plan 実行に専念する設計が SF-03 slice5 で確定済み。

## EXP-4: stateless `PluginStateDecl` を workspace 側で表現確定する案

- 仮説: `StateCaptureQuality::Unsupported` + `digest: None` で
  stateless を表現できるのではないか。
- 結果: **棄却（player 契約変更が必要）**。`verify_plugin` の Live
  条件は `digest.is_some()` を無条件要求し、`StateRestoreSupport`
  に「適用外」variant がない。判定ロジックは player 内にあり、
  workspace 側に verdict を変える lever がない。別 Issue に帰属。
