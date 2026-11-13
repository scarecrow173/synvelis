---
id: "EXP-0YsJ319KB8hmBTYdl0WICg"
title: "Context Inspector 設計定義 実験メモ (synvelis#5)"
created: "2026-11-14"
updated: "2026-11-14"
---

# Context Inspector 設計定義 — experiment log

## E1. doc-driven-dev-graph router の blocked 状態

`doc-driven-dev-graph` の route 実行は `artifact-graph` および
`docs/` 全体の既存 broken relations（defer/related が artifact
でない文字列を指す）により `blocked` を返した。本 slice 起因では
なく repository 全体の既存状態（先行 slice `ui-sf-control-
integration` も同じ状態で着地、同 exp E6 に記録あり）。
文書チェーンは先行 slice と同じ規約で spec→design→plan→task→
test-spec→ir→exp を作成し、無関係な docs の修復は行わなかった。

## E2. `ContextInspectionTarget` の語彙設計

Issue #5 の TargetKind（Track/Clip/Plugin/Parameter/Connection/
Probe）は既存の 3 系統の entity 語彙
（`SemanticTarget`・`SignalFlowEntityRef`・`ControlEntityRef`・
`SemanticProbePoint`）にまたがる。当初「単一正規化 ID への写像」
も検討したが、Provider が必要とする id 型が kind ごとに異なり、
写像不能な組合せが推測変換を誘発するため、**既存参照の tagged
union（同格な同居）** を採用した（design D3/D4）。link event が
運べるのは `SemanticTarget` のみなので、Processor/Parameter/
Connection/Probe は fail-closed で Explore/明示 inspect に限定し、
「link で全 kind に到達できる」とは設計しない。

## E3. Explore→Follow 復帰の配置

Issue #5 §3.3 は「Explore 中に FollowSource 一致の新規選択 event
で Follow へ戻す」を要求するが、既存 `LinkGroupManager::deliver`
は Follow mode のみ retarget し Explore/Pinned を決して動かさない
（`link.rs`、`uic5_03_*`/`signal_flow_link_delivery_*` で固定済み）。
共有 semantics を変更すると WS-A/SF-UX/UI-C5 の契約を壊すため、
**Inspector 側のイベント消費規則**として定義し、実装経路
（subscribe/derive or additive delivery hook）は #112/#108 slice
に委ねた（design D5）。この「契約は固定・実装経路は後続」の
分離を plan のリスク表と evidence のギャップ一覧にも記録した。

## E4. artifact ID の採番

`doc_maintenance`/`migrate_ids` の ID 契約は `<PREFIX>-<22-char
Base62>`。7 文書分を UUIDv7 相当の乱数 128bit → Base62 で生成し、
`SPEC-`/`DESIGN-`/`PLAN-`/`TASK-`/`TSPEC-`/`IMPL-`/`EXP-` prefix を
付与した。`docs/impl/exp` は先行 slice と同じく `EXP-` prefix・
最小 front matter（id/title/created/updated）を踏襲した。

## E5. evidence 台帳の判定語彙

`evidence/sv107/coverage.md` の ✅/◐/⛔ 凡例を、本 slice では
POLICIES.md §3 の `Pass`/`Fail`/`NotRun`/`Unsupported` に揃え、
headless 機構の証明と製品 UI の未実装を **`Pass (headless)` と
`NotRun`/`Partial` の併記**で分離した。CI-08（Probe 共有）・
CI-09（観測分類）・CI-13（非変更不変条件）は既存 headless 証拠が
強く、CI-01/02/06 は設計のみとして正直に `NotRun` とした。

## E6. review-protocol で検出した link event 粒度の誤記修正

初版の TargetKind 表は「Clip/Lane/Source・Bus も link event で
到達可能」と読める記述だった。`link.rs` の実装を再検証した結果、
`LinkEvent.target` は `map_to_track` 適用後の
`SemanticTarget::Track` のみ（Clip/Lane→owning Track、Source→
`None` で event 不発）であり、Bus/Strip/Processor/Parameter/
Connection/Send/Port/Probe は `EditingSelection`/`SemanticTarget`
の対象外と判明。design の TargetKind 表・D4・F8・対応表、
test-spec R3、spec ナビゲーション節を「Follow retarget は Track
粒度、他 kind は Explore/明示 inspect 経路」へ修正した。
あわせて plan/task のファイル名誤記（`context-inspector.md`→
`-plan.md`/`-task.md`）と、存在しないテスト名
`sf04_probe_off_is_true_observation_silence` への言及
（正: `sf04_s2_probe_trace_bridge.rs::
sf04_s2_mode_off_maps_to_disabled_and_unrouted_record_returns_false`）
を修正した。
