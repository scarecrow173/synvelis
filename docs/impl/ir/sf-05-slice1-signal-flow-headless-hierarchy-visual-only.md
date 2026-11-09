---
id: "IMPL-034UvpX7fem2fghDtfNDJm"
type: "impl"
status: "completed"
title: "SF-05 slice1 Signal Flow headless — hierarchy 投影 + visual-only 不変 + 状態表示"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/signal_flow_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/sf_05.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/sf05_signal_flow.rs"
      - type: "file"
        path: "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
      - type: "file"
        path: "docs/adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md"
      - type: "file"
        path: "docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md"
      - type: "file"
        path: "docs/plans/sf-05-slice1-signal-flow-headless.md"
      - type: "file"
        path: "docs/tasks/sf-05-slice1-signal-flow-headless.md"
      - type: "file"
        path: "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
      - type: "file"
        path: "docs/impl/ir/sf-05-slice1-signal-flow-headless-hierarchy-visual-only.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/engine.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/service_ops.rs"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "index"
        path: "docs/adr/README.md"
      - type: "index"
        path: "docs/designs/README.md"
      - type: "index"
        path: "docs/plans/README.md"
      - type: "index"
        path: "docs/specs/README.md"
      - type: "index"
        path: "docs/tasks/README.md"
      - type: "index"
        path: "docs/test-specs/README.md"
      - type: "index"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/plans/sf-05-slice1-signal-flow-headless.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-05-slice1-signal-flow-headless.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md"
    - "docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md"
    - "docs/tasks/sf-05-slice1-signal-flow-headless.md"
    - "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-05-slice1-signal-flow-headless.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
  verified-by:
    - "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-05 slice1 Signal Flow headless — 実装記録

## Summary

Issue #108（SF-05 Signal Flow Editor / Semantic+Visual Hierarchy）の
headless 完結 slice として、`RoutingModel` を権威とする Signal Flow
read model（`synvelis-workspace`）と、app 層の visual-only op 面・
engine evidence 合成 status・read-only screen（`synvelis-app`）を
実装した。SF-UX-01/02/05 の headless 受入を fixture test で検証。

## Context

- 対象: synvelis#108 SF-05 slice 1（headless）。
- 文書チェーン: `docs/specs/sf-05-signal-flow-headless-hierarchy.md`
  → `docs/adr/sf-05-slice1-...md` → `docs/designs/sf-05-slice1-...md`
  → `docs/plans/sf-05-slice1-signal-flow-headless.md` →
  `docs/tasks/sf-05-slice1-signal-flow-headless.md` →
  `docs/test-specs/sf-05-slice1-signal-flow-headless.md`。
- 先行実装（main 済み）: SF-01 routing model、SF-03 lowering/
  SourceMap/activation receipt、SF-04 probe adapter、PC-C5
  `relation_ui` read model 先例。

## Implementation

### Changes

- `crates/synvelis-workspace/src/signal_flow_ui.rs`（新規）:
  - `signal_flow_graph`/`signal_flow_graph_with`: `ProjectSession`
    の live project から `SignalFlowView`（nodes/edges/status/
    coverage/scope）を投影。`RoutingConnection` を edge の唯一の権威
    とし 1:1 投影（Send connection + SendConfig の二重 edge なし）。
    processor-chain edge は Expanded scope で順序から合成し
    `SignalFlowEdgeId::Chain` で区別。scope は Overview/Expanded/
    Local（1-hop induced subgraph）/History（Unsupported）、
    role filter は edge のみ絞り node は保持、collapse は processor
    展開のみ fold し semantic node/edge を失わない。
  - `SignalFlowStatus`: live revision・engine_attached・activation
    （receipt verbatim: planned/published/observed/last-good
    generation・fault）・source-map revision/generation・stale・
    probe 一覧を合成。evidence なしでは `engine_attached=false`・
    activation なし・node runtime `Unobserved` を正直に返す。
  - node runtime は source-map の `MappingResolution` を
    Resolved/Unavailable/Unsupported/NotLowered/Unobserved/Stale に
    写像し、probe binding は Track/Bus/Strip/Processor entity を
    node に対応付ける（Send point は status.probes のみ）。
  - `SignalFlowVisualState`: view 毎の additive visual state
    （positions/groups/collapsed/zoom/pan/link_focus/scope/
    `visual_revision`）。`visual_*` free functions は全て pure で
    `ProjectSession`/engine/undo に触れない。`view_options()` が
    graph query へ fold する。
  - coverage: `expected_revision` 不一致 → `Stale`（空 graph +
    live revision status）、bounds → deterministic prefix +
    `Truncated`、History scope/Audio layer 欠落 → `Unsupported`、
    Local 未知 root → `Unavailable`。
- `crates/synvelis-workspace/src/lib.rs`: `pub mod signal_flow_ui;`
  + additive re-export のみ。
- `crates/synvelis-app/src/app.rs`: `routing_engine:
  Option<SessionRoutingEngine>` を保持し `attach_routing_engine`
  で clone を session と共有。`signal_flow_view(view)` が per-view
  visual state を options に畳み込み optional evidence を合成。
  `signal_flow_runtime_evidence()` が receipt/source-map/probe
  bindings を snapshot。`publish_project_length` で
  `MainScreenState.signal_flow` を refresh。project new/open で
  engine を clear。`capabilities()` の `signal_flow` を headless
  read model + visual ops available（canvas deferred）に更新。
- `crates/synvelis-app/src/engine.rs`: `probe_bindings()` accessor
  を追加（requested/RT-confirmed を区別したまま返す）。
- `crates/synvelis-app/src/screens.rs`: `SignalFlowScreenState`
  （最新 `SignalFlowView`）を `MainScreenState.signal_flow` に追加。
  main view に Signal Flow tab（status label + nodes/edges の
  read-only Table、`selection: None`）を追加。status label は
  revision/coverage/engine/activation/planned/published/observed/
  last-good generation/source-map revision・generation/probe 数/
  stale marker を表示し `Published`≠`Active` を区別。
- `crates/synvelis-app/src/ui_ops.rs`: `UiOpsState.signal_flow:
  BTreeMap<String, SignalFlowVisualState>` を追加し
  `syn.signal_flow.view`/`syn.signal_flow.visual` op を実装。
  visual op（move_node/set_group/fold/unfold/zoom/pan/link_focus/
  set_scope）は visual state のみ更新し `{ok, view,
  view_generation}` を返す。view op は visual state 由来の既定
  options に param override を畳み込み、`expected_revision` で
  stale query を受理。wire entity は `{"kind","id"}` tagged 形式。
- `crates/synvelis-app/src/service.rs`: 2 op を OPS/dispatch に登録。
- `crates/synvelis-app/tests/service_ops.rs`: capabilities テストを
  signal_flow available に追従。

### Decisions During Implementation

- read model は `relation_ui` と同じ「session を読み DTO を返す」
  パターンに揃え、第二 domain model を作らない（ADR 決定）。
- visual state は `WorkspaceSnapshot`/`ViewState`（schema v2,
  `deny_unknown_fields`）に入れず app 層 `UiOpsState` の in-memory
  BTreeMap に保持 — 永続化は SF-UX-07 の範囲。
- engine evidence は `Option<SignalFlowRuntimeEvidence>` snapshot
  合成とし、engine 未接続を「成功」に見せない。`Published`
  （RT 未観測）と `Active`（observed generation 一致）は receipt
  の generation field を分けて保持。
- processor-chain edge は persisted `RoutingConnection` に存在しない
  ため Expanded scope でのみ chain 順序から合成し、
  `SignalFlowEdgeId::Chain` で connection edge と区別 — Overview
  では出さず二重表示を避ける。
- `Local{Processor}` scope は対象 processor node のみ展開し
  chain-order edge を抑止（順序を暗示しない）。
- `StaticProcessorHost` + `GainProcessor` で app fixture の plugin
  解決を headless に成立させた（connect が instance を consume
  するため fixture は semantic build を 1 回に限定）。

## Related Experiments

### Adopted

なし。

### Rejected

なし。

## Validation

`origin/main`（RCD-03 merge 取り込み後）を merge し、doc index を
`doc-maintenance` で再生成した HEAD で実施。canonical = mise。

| コマンド | exit status |
| --- | --- |
| `mise run fmt-check` | 0 |
| `mise run check` | 0 |
| `mise run lint` | 0 |
| `mise run test` | 0（`cargo test --workspace --locked`、sf_05 13 tests + sf05_signal_flow 5 tests 含む全 pass） |
| `mise run check-app-native` | 0 |
| `mise run lint-app-native` | 0 |
| `mise run build-app-native` | 0 |

補助証拠: `cargo test -p synvelis-workspace --test sf_05`（13/13）・
`cargo test -p synvelis-app --test sf05_signal_flow`（5/5）直接実行も
pass（canonical は上記 mise）。doc-status audit で sf-05 文書の
link/relation error なし（`traceability-missing-upstream` warning は
sf-04/pc-c5 等 main 上の先例と同型で残置）。

## review-protocol 結果

- spec→adr→design→plan→task→test-spec の文書チェーンと実装 diff を
  review-protocol でレビュー（worker 契約により subagent 不使用、
  separated pass で実施）。CONFIRMED P0/P1 なし。
- レビューで確認した点: `RoutingConnection` 1:1 edge 投影による
  Send 二重表示の不在、visual op が `UiOpsState.signal_flow` のみを
  触り revision/project JSON/engine builds/receipt が不変であること
  （test で snapshot 比較）、`Published`→`engine.run` 後に `Active`
  へ遷移する receipt 駆動の status、engine 未接続時の
  `Unobserved`/`engine_attached=false`、stale expected_revision で
  空 graph + live revision status、`Local`/`Expanded`/collapse/
  role filter の node 保持 semantics。
- 対応した指摘: `signal_flow_graph_with` の `too_many_lines` を
  strip/expand/processor/edge/local-scope helper に分割、test/op
  helper の clippy 指摘（assert_is_empty/needless_pass_by_value/
  unnecessary_wraps/redundant_closure/too_many_lines）を解消、
  spec の doc chain link 誤記を修正、capabilities 変更に伴う
  service_ops テスト追従。

## NotRun / Deferred

- **SF-UX-03**（Port Drag 編集・consent/late receipt）: **NotRun** — 後続 slice。
- **SF-UX-04**（Follow/Explore/Pinned/2Window/Dock/Close/Focus 連携）: **NotRun** — 後続 slice。
- **SF-UX-06**（Probe 共有・Capture 重複なし）: **NotRun** — #107 後続依存。
- **SF-UX-07**（Workspace Schema v2/v3 永続化）: **NotRun** — visual state は in-memory のみ。
- **SF-UX-08**（実 OS/GPUI Native lifecycle/HighDPI/A11y/IME）: **NotRun** — #114 gate。
- **#109/#103/#114 との Case ID 結合**: **NotRun** — oracle/gate 段階。
- **実 OS・実 GPUI での描画証拠**: **NotRun** — headless slice のみ。`check-app-native`/`lint-app-native`/`build-app-native` は native feature 経路の確認として実行（結果は PR 本文）。
- `crates/synvelis-recording/`・`crates/synvelis-player/`・`crates/synvelis-project/` control ledger 本体・Ruviform: 変更なし。

## Risks

- `publish_project_length` 毎に `signal_flow_view` を再投影する
  （`relation_rows` refresh と同じコスト特性）。大規模 routing での
  query コスト最適化は後続 slice の検討事項。
- wire の node/edge id は `Debug` 文字列、entity は tagged id —
  frontend 側の安定 contract は SF-UX-04 以降で要検討。
- `signal_flow_graph` と `signal_flow_graph_with` は現状同一 signature
  （`relation_ui` の対称命名に合わせた将来拡張点）。

## Follow-ups

- SF-UX-03/04/06/07/08 は Issue #108 の残件として後続 slice へ。
- visual state の Workspace Schema v2 永続化（SF-UX-07）で
  `UiOpsState.signal_flow` を snapshot へ移行する判断。
