---
id: "EXP-signal-flow-design"
title: "Signal Flow 設計正本 — 語彙照合・契約確認・文書規約の探索記録"
created: "2026-11-14"
updated: "2026-11-14"
---

# Signal Flow 設計正本: 探索・照合記録(2026-11-14)

- Issue: https://github.com/scarecrow173/synvelis/issues/4
- Task: `docs/tasks/signal-flow-design-task.md`
- Plan: `docs/plans/signal-flow-design-plan.md`
- Design: `docs/designs/signal-flow-design-design.md`

## 目的

設計正本の記述が既存実装語彙・SF 系正本 doc と一致することを、
文書作成前に確認する探索的検証。コード変更は伴わない。

## 観測した事実

### 1. Issue #4 本文の契約構造

Issue 本文（385 行）を取得し、以下を確認:

- 受入条件 6 項目（Local/Expanded 探索・Send/Sidechain 作成・
  接続検証・編集境界・Semantic/Visual Group 独立・Probe 表示
  一貫性）。
- 「操作は Intent/Project CAS/Compile/Apply Receipt に分け、
  UI クリック時点を DSP 適用済みとみなさない」— design §C.2・
  §E.1 の世代証拠分離の根拠。
- 「Doc には SF-03〜06 の ID/ポート意味/失敗・Rollback/Case と
  依存 #104〜109 を固定する」— scenario ID 対応表（§E.4）と
  depends-on/references relations の根拠。
- 「単なる `serde(default)` 空 Map では完了不可」— legacy
  migration/source map 受入の要件として spec SFD-R5 に反映。

### 2. 実装語彙の照合（読み取りのみ）

| 照合対象 | ファイル | 結果 |
| --- | --- | --- |
| `ProbeMode`/`ProbeResolveState`/`ProbeStopReason` | `crates/synvelis-project/src/probe.rs` | design §D.2/D.3 の語彙と一致。Issue 本文の概念名（`Unbound`/`Partial`）は実装 enum（`PartiallyRebound`/`StaleGeneration`/`Stopped`）と異なるため、design で実装語彙を正本と明記 |
| `RoutingEditError` variant 群 | `crates/synvelis-project/src/routing_tx.rs:332` | design §B.3 の検証分類表と一致 |
| `SignalFlow*` UI 型群 | `crates/synvelis-workspace/src/signal_flow_ui.rs` | `SignalFlowNodeRuntime`/`EdgeRole`/`Scope`/`Layer`/`VisualGroup` が design §A/§E.3 の記述と一致 |
| `audible_strips`・`ProjectOp` | `crates/synvelis-project/src/routing.rs`/`ops.rs` | design §C.3 の Mixer 整合記述と一致 |

### 3. 文書チェーン規約の確認

- 先行例 `docs/specs|designs|plans|tasks|test-specs|impl/
  {ir,exp}/issue-365-*` を読み、front matter `type`/`status`/
  `relations`・index 再生成・IR audit の規約を確認。
- generator script の gate を確認: plan は approved design 必須、
  task は approved/in-progress plan 必須、test-spec は解決可能な
  `verifies` 対象必須。このため作成順序を
  spec → design(approved) → test-spec → plan(approved) → task
  とした。
- `docs/impl/exp/` には `.md`（叙述的な探索記録）と `.jsonl`
  （CLI 管理の event stream）の 2 形式が存在。本タスクは
  dispatch が指定する `signal-flow-design.md` を叙述形式で作成
  した（JSONL は編集・生成対象外）。

### 4. graph router の状態

`node .apm/skills/doc-driven-dev-graph/scripts/route_graph.js`
を focus `signal-flow-design` で実行したところ `status: blocked`
（current/next: probe、condition: blocked）。blocker は repository
全体の既存 broken relation（`docs/specs/sf-03-map-semantic-
runtime-contract.md` の `defers` が非ファイル target を指す等）
と `focus-invalid` であり、本タスク起因ではない。文書作成は
skill の手順に従い直接実施した。

## 結論

- design の語彙は実装 enum と一致することを確認（差異は Issue
  本文の概念名のみで、design で対応を注記済み）。
- 文書チェーンの依存順序と gate 要件を確認し、規約通りに作成。
- graph router の blocked は外部要因として記録し、修復は
  タスク範囲外として PR 本文に記載する。
