---
id: "IMPL-5kKD8UEketY1kHcAnYUEao"
type: "impl"
status: "completed"
title: "Context Inspector 設計定義 — spec/design/test-spec 文書チェーン + UX Fixture + 被覆台帳（synvelis#5 docs slice）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
metadata:
  experiments:
    adopted: []
    rejected: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/5"
  changes:
    added:
      - type: "file"
        path: "docs/specs/context-inspector.md"
      - type: "file"
        path: "docs/designs/context-inspector-design.md"
      - type: "file"
        path: "docs/plans/context-inspector-plan.md"
      - type: "file"
        path: "docs/tasks/context-inspector-task.md"
      - type: "file"
        path: "docs/test-specs/context-inspector.md"
      - type: "file"
        path: "docs/impl/ir/context-inspector.md"
      - type: "file"
        path: "docs/impl/exp/context-inspector.md"
      - type: "file"
        path: "evidence/sv5/coverage.md"
    modified: []
  implements:
    - "docs/plans/context-inspector-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/context-inspector-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/context-inspector.md"
    - "docs/designs/context-inspector-design.md"
    - "docs/test-specs/context-inspector.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/context-inspector.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Context Inspector 設計定義 — Implementation Record

## 実装サマリ

synvelis#5（設計 Issue）の docs slice として、Context Inspector の
UX・状態・編集・Probe 契約を canonical 文書チェーン
（spec→design→plan→task→test-spec→ir/exp）と受入条件被覆台帳
（`evidence/sv5/coverage.md`）として固定した。**製品コードの変更は
一切なく**、`crates/`・`Cargo.toml`・`mise.toml`・CI 設定は
未変更。設計のみの項目は `NotRun` として台帳に記録し、headless
既存証拠と製品契約を分離した。

## 変更内容

- `docs/specs/context-inspector.md`: What/Why、CI-01〜13（Issue #5
  §9 の 13 受入条件を 1:1 対応）、Non-goals（#112/#108/#107/#114
  の Owner 境界と PoC 後決定事項）。
- `docs/designs/context-inspector-design.md`: Owner 境界表、
  Issue #5 概念→既存実装（`state.rs`/`link.rs`/`selection.rs`/
  `signal_flow_ui.rs`/`relation_ui.rs`/`probe.rs`/`probe_trace.rs`/
  gesture fixation）の対応表、`ContextInspectionTarget` tagged
  union、Follow/Explore/Pinned/Unresolved 状態機械、適応 Section
  情報階層、all-or-nothing 一括編集、Session 所有 Probe と観測
  分類、View Instance 単位永続化、Decision Log D1〜D19、Normal/
  Failure Scenario と Expected Oracle。
- `docs/plans/context-inspector-plan.md`: docs slice のスコープ・
  ファイルマップ・リスク・検証マトリクス・後続実装 Owner への
  引き継ぎ表・ロールバック。
- `docs/tasks/context-inspector.md`: 10 タスクの作業分解。
- `docs/test-specs/context-inspector.md`: CI-01〜13 に対応する
  UX Fixture の Rule R1〜R7・Examples・Guarantees・Non-goals。
- `evidence/sv5/coverage.md`: 全受入条件の既存 Fixture/実装被覆を
  `Pass (headless)`/`Partial`/`NotRun`/`Deferred` で台帳化。
  ギャップ一覧と検証実行記録を含む。
- `docs/impl/ir|exp/context-inspector.md`: 本記録と実験メモ。
- `docs/<type>/README.md`: doc-maintenance `index apply` による
  生成物（手編集なし）。

## 設計上の判断（design D1〜D19 の要約）

- 既存 `InspectionMode`/`FollowSource`/explore stack・per-view
  `Option` state + tolerant decode の確立済み契約を再利用し、
  新しい並行機構を作らない。
- `ContextInspectionTarget` は既存 entity 参照
  （`SemanticTarget`/`SignalFlowEntityRef`/`ControlEntityRef`/
  `SemanticProbePoint`）の tagged union であり、新 domain id を
  発明しない。`SemanticTarget` に写像不能な kind は fail-closed で
  Explore/明示 inspect 導線に限定。
- Explore→Follow 復帰は `link.rs` の共有 semantics を変更せず
  Inspector 側のイベント消費規則として定義（design D5）。
- 一括編集は絶対値・発行時全対象 snapshot・単一 controller
  preflight・all-or-nothing。相対変更・部分適用は後続。
- Probe は `ProbeSubscriptionKey`/`ProbePrincipal` lease を再利用
  し、Off 既定・View 閉鎖で session hold を維持。
- 固定 5 タブ・自動 Monitor・Explore 履歴永続化・DAW 語彙の
  Ruviform 持ち込みは不採用/保留として Decision Log に記録。

## 検証結果

- `mise run fmt-check`: **Pass**（exit 0、`cargo fmt --all --
  --check`。docs-only slice で Rust 変更なし）。
- docs index: `doc_maintenance.js index plan`/`index apply` で
  specs/designs/plans/tasks/test-specs/impl(ir) の README を
  再生成し `index plan` で全 dir `in-sync` を確認。repair
  `plan`/`apply` で designs/test-specs の rebuild-index を適用
  （新規 finding なし。残存 finding は全て他文書の既存
  manual/migration 案件で本 slice 起因ではない）。
- `review-protocol`: 仕様整合（Reviewer A 相当）・反例
  （Reviewer B 相当）の 2 pass を実施し、コード実体との照合で
  CONFIRMED な指摘を修正した（詳細は exp E6）:
  - design TargetKind 表の link event 粒度誤記 → Track 粒度へ
    修正（`map_to_track`/`deliver`/`signal_flow_entity_from_
    semantic` で実証）。
  - plan/task のファイル名誤記 → 修正。
  - evidence の存在しないテスト名参照 → 実在テストへ修正。
  - design タイトル「情報階級」→「情報階層」へ修正。
  修正後の再検証で新たな CONFIRMED P0/P1 は残っていない。
- 製品 AC（CI-01〜13）の実行結果: `evidence/sv5/coverage.md`
  の台帳どおり（Pass (headless) 3 件・Partial 7 件・NotRun
  相当を含む）。統合受入は merge 後対象 SHA の CI 証拠で確認。

## 未対応・Deferred

- Context Inspector View/Provider/状態機械/一括編集/Probe UI の
  実装 — #112/#108/#107 の Owner（本 slice 対象外）。
- 実 Window/IME/DPI/Plugin 実機 — #114/#176・Ruviform #313/#314
  の Gate（Deferred）。
- MIDI Note/Event Provider — #10/#12 model 確定待ち。
- Issue #5 本体の Close — 設計レビュー承認後も下流の未実装を
  隠さないため、Close 判断は Coordinator/人間に委ねる。
