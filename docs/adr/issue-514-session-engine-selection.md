---
id: "ADR-0e3CjgAC6VMxn4HycdTGhk"
type: "adr"
status: "accepted"
title: "Issue #514 — SessionRoutingEngine は RCP selection を消費しない（責務は RoutingEngine レベルで閉じる）"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/514"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "docs/discovery/issue-514-session-engine-selection.md"
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/designs/issue-514-session-engine-selection.md"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-514-session-engine-selection.md"
      - type: "file"
        path: "docs/specs/issue-514-session-engine-selection.md"
      - type: "file"
        path: "docs/designs/issue-514-session-engine-selection.md"
      - type: "file"
        path: "docs/plans/issue-514-session-engine-selection.md"
      - type: "file"
        path: "docs/tasks/issue-514-session-engine-selection.md"
      - type: "file"
        path: "docs/test-specs/issue-514-session-engine-selection.md"
      - type: "file"
        path: "docs/impl/ir/issue-514-session-engine-selection.md"
      - type: "file"
        path: "docs/impl/exp/issue-514-session-engine-selection.md"
      - type: "file"
        path: "docs/impl/exp/issue-514-session-engine-selection-exp.jsonl"
      - type: "dir"
        path: "evidence/sv514"
      - type: "file"
        path: "crates/synvelis-app/src/engine.rs"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-514-session-engine-selection.md"
  implemented-by:
    - "docs/designs/issue-514-session-engine-selection.md"
  depends-on:
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/designs/issue-514-session-engine-selection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv293-semantic-selection-integration.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
    - "evidence/sv293/acceptance-ledger.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-514-session-engine-selection.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv514/decision-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/530"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/295"
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# Issue #514 — SessionRoutingEngine は RCP selection を消費しない

## Context and Problem Statement

Synvelis#293 / Synvelis#106 の residual: RCP-04 semantic→selection
統合は `synvelis-workspace::rcp_selection` +
`synvelis-player::rcp` で実装済みだが production caller は 0 で、
`SessionRoutingEngine`（synvelis-app）が selection を消費するかは
「Deferred」のまま未確定だった。Issue #514 はこの帰属を確定し、
「実装する」「RoutingEngine レベルで責務が閉じる（実装不要）」の
いずれかを root-cause で記録することを要求する。

## Decision

**`SessionRoutingEngine` は RCP selection を消費しない。
rebuild 経路は現行の直接 `RoutingEngine::prepare`/`publish` 駆動を
維持し、selection 非消費は「意図的」とする。** 責務は
`RoutingEngine` レベルで閉じており、app 層への配線は現契約では
不要かつ有害になり得る。

## 根拠（root-cause）

1. **入力の不在**: `EnginePort::on_project_revision` は
   `(&Project, ProjectRevision)` のみを受け取る。`SelectionInputs` が
   要求する plugin_evidence・environments・render_candidates の
   production 供給経路は存在しない（`ProcessorBinding` は
   descriptor/version/capability_revision のみ、`PluginHost` に
   environment facts API なし、`SemanticRenderCoverage` の producer
   なし）。
2. **出力の不変**: `drive_selection_plan` が駆動する engine build
   は rebuild と同一入力・同一呼出であり、published plan を変えない。
   substitution は decision 証拠に留まり plan shaping に使われない
   ため、配線しても実再生上の能力は増えない。
3. **gate 化の弊害**: `ProcessorHost::connect` が executability の
   ground truth であるのに対し、selection evidence は caller 申告の
   弱い二次情報。verdict を hard gate にすると evidence/host 乖離で
   build 可能な plan を false-block する失敗モードを新設するだけ。
4. **pin の既存性**: stale 非 publish・generation/epoch 不一致・
   QueueFull/RetireOverflow last-good は `drive_selection_plan` の
   pre-gate と `RoutingEngine` 契約で既に pin 済み
   （workspace `rcp_selection_integration.rs`・player `rcp_04`
   oracle・`sf03_session_engine.rs`）。adapter はこれらを継承する。
5. **consumer の不在**: live `ExecutionSelection` の consumer
   （sv#295 表示面）は producer を待つ構成であり、自然な producer は
   per-intent selection service であって intent 非依存の rebuild
   hot path ではない。

## Consequences

- `SessionRoutingEngine` の rebuild 経路・`EnginePort` 契約・
  `SessionEngineConfig`/composition 経路は変更しない。
- `synvelis-app` への dep 追加は発生しない（synvelis-render 非依存を
  維持）。
- `crates/synvelis-app/src/engine.rs` の module doc に「selection
  非消費は意図的」である旨を明記し、将来の誤配線を防ぐ。
- stateless `PluginStateDecl` の型表現問題は player 契約変更が必要な
  ため本 Issue では実装せず、別 Issue に帰属する
  （`evidence/sv514/decision-ledger.md`）。
- 将来の配線 trigger: `SelectionInputs` の production 供給経路・
  `ExecutionSelection` の consumer・対象 intent の確定（design 文書
  参照）。

## Alternatives Considered

- **rebuild を `drive_selection_plan` に置換**: SelectionInputs を
  組み立てる供給経路がなく、空 evidence は fail-closed で全 plugin
  project が build 不能になる。捏造 evidence は honest-input 契約
  違反。verdict gate は false-block を持ち込み、substitution は
  plan に反映されないため能力も増えない。
- **selection を「観測のみ」で rebuild に併走**: receipt を得るため
  だけに毎 revision で selection を走らせても consumer がなく、
  証拠供給がないため verdict は常に fail-closed 系になる。
  意味のないコストと複雑性のみ増える。
- **engine 側に selection 消費を移す**: `RoutingEngine` は
  `synvelis-workspace` に依存できない（依存方向:
  workspace → player）。また engine は selection の意味論を持たず、
  plan 実行に専念する設計が SF-03 slice5 で確定済み。
