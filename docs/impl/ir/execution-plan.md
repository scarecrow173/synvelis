---
id: "IMPL-034X84u7PqVqLhNVOe8UZB"
type: "impl"
status: "completed"
title: "Execution Plan — Issue #23 横断 DAG 文書化 + sv23 evidence 台帳"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/23"
  changes:
    added:
      - type: "file"
        path: "docs/specs/execution-plan.md"
      - type: "file"
        path: "docs/designs/execution-plan-design.md"
      - type: "file"
        path: "docs/plans/execution-plan.md"
      - type: "file"
        path: "docs/tasks/execution-plan-task.md"
      - type: "file"
        path: "docs/test-specs/execution-plan.md"
      - type: "file"
        path: "docs/impl/ir/execution-plan.md"
      - type: "file"
        path: "docs/impl/exp/execution-plan.md"
      - type: "file"
        path: "evidence/sv23/execution-plan-gate-ledger.md"
      - type: "file"
        path: "evidence/sv23/execution-plan-entry-gate-ledger.md"
      - type: "file"
        path: "evidence/sv23/execution-plan-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/execution-plan-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/execution-plan-task.md"
    - "docs/plans/execution-plan.md"
    - "docs/specs/execution-plan.md"
    - "docs/designs/execution-plan-design.md"
    - "docs/test-specs/execution-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/execution-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/execution-plan-task.md"
    - "docs/plans/execution-plan.md"
    - "docs/specs/execution-plan.md"
    - "docs/designs/execution-plan-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/execution-plan.md"
  verified-by: []
  references:
    - "evidence/sv23/execution-plan-gate-ledger.md"
    - "evidence/sv23/execution-plan-entry-gate-ledger.md"
    - "evidence/sv23/execution-plan-verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/execution-plan.md"
    rejected: []
---

# Execution Plan — Issue #23 横断 DAG 文書化 + sv23 evidence 台帳

## Summary

Issue #23 の横断実行計画を canonical 文書鎖
（spec→design→plan→task→test-spec→impl ir/exp）+
`evidence/sv23/` 台帳 3 件として確立した。RCP-00〜07/
F01・F0/E2E-L/R/X・Signal Flow・Automation/Control・
GUI・Recording・Time/MIDI/Plugin・Gateway・VCS/ALIB/
OBS/CB/AUTX・CI の全 workstream を、Start/
Integration/Acceptance の 3 相 edge 型を持つ循環のない
単一 DAG として `docs/designs/execution-plan-design.md`
と `evidence/sv23/execution-plan-gate-ledger.md` に固定
し、共通/Consumer Owner・OS-L1/GUI-L2/EXT-L3・要求
状態/実行結果の語彙分離・正規入口対応・候補 SHA tuple
追跡を台帳化した。実装コード・CI・Cargo/mise の変更は
行っていない。

## Context

`docs/tasks/execution-plan-task.md`（TASK-034X84fclPRNdTgHrtrDka）
と `docs/plans/execution-plan.md`（PLAN-034X841J2QRrgpxCPlRiKb）
に従い、`docs/specs/execution-plan.md`（SPEC-034X83uiIsH0n6OuQGcU1T）
と `docs/designs/execution-plan-design.md`
（DESIGN-034X83GOEQ84LyUmDRBepu）の設計契約を文書化
した。Issue #23 本文の Stage 3 Start/Integration/
Acceptance 分離・横断依存正規化 v1.1・GUI lane ordered
DAG・RCP/CB/AUTX 追加 DAG・判定台帳を写像元とし、
`POLICIES.md` の OS-L1/GUI-L2/EXT-L3・Required/Deferred/
NotApplicable・Pass/Fail/NotRun/Unsupported 規範へ写像
した。

## Implementation

### Changes

- `docs/specs/execution-plan.md`: EP-R-01〜12 要件と
  EP-AC-01〜09 受入を定義。単一 DAG・3 相分離・Owner
  分離・OS 検証レベル・判定語彙・Evidence schema・
  候補 SHA tuple・正規入口・CI 増設禁止・並行可能
  Slice・文書鎖正本性・fail-closed 判定を規定。
- `docs/designs/execution-plan-design.md`: DAG Model
  （node/edge 型・判定語彙）・Workstream Lane 定義・
  Start/Integration/Acceptance edge 表（全 lane）・
  Owner/Gate/Acceptance Matrix・OS Level 規則・
  Evidence Schema・正規入口対応・候補 SHA tuple
  Registry・Failure Taxonomy を記述。
- `docs/plans/execution-plan.md`: Goal/Scope/Upstream/
  Design Inputs/File Map/Implementation Streams 写像/
  Dependency Graph/Risk Register/Verification Plan/
  Rollback/Task Breakdown/Open Questions/Self-Review/
  Implementation Handoff/Review Handoff を記述。
- `docs/tasks/execution-plan-task.md`: Work 11 項目・
  Classification・Upstream・Dependencies・Affected
  Files・Done When・Verification を記述。
- `docs/test-specs/execution-plan.md`: EP-TS-R01〜R07
  の不変条件（非巡回・edge 型・Owner 一意・OS Level・
  Evidence Schema・tuple 追跡・fail-closed・文書鎖整合）
  と Examples/Guarantees を定義。
- `docs/impl/exp/execution-plan.md`: E1〜E4 の試行記録
  （edge 型写像・OS level/語彙分離・正規入口/tuple
  台帳化・文書鎖生成）と採用/棄却した判断を記録。
- `evidence/sv23/execution-plan-gate-ledger.md`:
  lane 別 node 台帳（Issue・Owner・Start/Integration/
  Acceptance・OS Level・写像時点状態）・Gate node 定義・
  候補 SHA tuple Registry・fail-closed 判定を記録。
- `evidence/sv23/execution-plan-entry-gate-ledger.md`:
  正規入口 × OS Level × Gate/Feature × 要求状態 ×
  結果状態 × 対象 SHA × 証拠の対応、capability 分離の
  注意、OS 別要求状態、候補 SHA tuple 追跡欄、CI lane
  を記録。
- `evidence/sv23/execution-plan-verification.md`:
  本 slice の検証実行記録（実行したコマンド・OS・
  結果・NotRun/Unsupported の理由）。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/README.md`: `doc_maintenance.js index
  apply` による再生成（手編集なし）。

### Decisions During Implementation

- Issue #23 本文の DAG 記述を一つの非巡回 DAG として
  統合し、node は lane 単位・Issue 番号併記で粒度を
  抑えた（全 Issue 個別 node 化は可読性を損なうため
  棄却）。
- 「Pass(部分)」の中間状態を導入せず、要求状態と実行
  結果の 2 軸 + 計画参照 `NotRequired` のみを採用
  （POLICIES.md 語彙との一致を優先）。
- sv#21/sv#22 並行 Worker の文書は編集・作成せず参照
  のみとし、path/内容競合を回避した。
- 実装/Gate の実行は本 slice の対象外とし、台帳の結果
  状態は `NotRun`/`Deferred` を誠実に記録した。文書
  承認を実装完了・製品受入と混同しない規則を
  test-spec Guarantees に明記した。
- 実験 log は `docs/impl/exp/execution-plan.md` の
  Markdown 形式（既存 exp `.md` 慣行に合わせる）とし、
  IR `metadata.experiments.adopted` からリンクした。

## Related Experiments

### Adopted

- `docs/impl/exp/execution-plan.md`: Issue #23 本文の
  DAG 記述の edge 型写像・OS level/判定語彙分離・
  正規入口/tuple 台帳化・文書鎖生成の試行記録。
  採用理由: 本 slice の設計判断（edge 型・語彙・粒度・
  並行 Worker 参照のみ）の根拠を追跡可能にするため。

### Rejected

- なし（棄却した案は exp の「棄却した案」節に記録:
  全 Issue 個別 node 化・新規 CI Workflow 増設・
  headless/Fake Pass の実機誤昇格）。

## Validation

実行した検証の詳細は `evidence/sv23/execution-plan-
verification.md` を参照。実行したもの:

- `node .agents/skills/doc-status/scripts/doc_status.js
  lint <対象 7 文書>`: 結果は verification.md 参照。
- `node .agents/skills/doc-maintenance/scripts/
  doc_maintenance.js index apply`: index 再生成。
- `node .agents/skills/impl-doc/scripts/
  audit_impl_record.js --json`: 結果は verification.md
  参照。
- `node .agents/skills/doc-status/scripts/audit_docs.js`:
  結果は verification.md 参照。
- `node .agents/skills/doc-driven-dev-graph/scripts/
  route_graph.js --focus execution-plan --json`: 結果は
  verification.md 参照。
- `mise run fmt-check`: 結果は verification.md 参照。
- `review-protocol`: 結果は verification.md 参照。

未実施（`NotRun`）: 実装・CI・Cargo/mise 変更を伴う
検証、実機/実 Plugin/実 GUI の L2/L3 受入、MSRV 系
タスク（ユーザー指定により本 slice では実行しない）、
Daily/手動 Full CI の対象 SHA 実行（merge 後 main SHA
の CI が統合検証の証拠）。

## Risks

- DAG の肥大化: lane node + Issue 番号併記で粒度を抑
  えたが、Issue 状態の変化は台帳更新で追従する必要が
  ある。
- Issue 状態の陳腐化: 写像時点の Issue state を台帳に
  記録し、変化は台帳更新で追従する（design 本文は
  書き換えない）。
- 「実際に提供する機能」の未確定: Windows L2/L3 の
  Required 範囲は製品提供範囲（#176・#115・#117・
  #126/#127 と Ruviform capability）に依存し、確定前
  に全組合せを Required にしない。
- 並行 Worker との index 競合: docs index は生成物。
  競合時は merge → `index apply` 再生成で解消（検証
  流用可）。
- graph router の既存 baseline blocker: 範囲外の既存
  broken-relation findings は修復せず、evidence に記録
  した。

## Follow-ups

- 各 workstream の実装 Owner が design の lane/edge 表と
  `evidence/sv23/` 台帳を参照し、自分の node の
  Start/Integration/Acceptance と OS Level を確認する。
- 共通基盤の不足は Synvelis 側で恒久重複実装せず
  Ruviform Issue へ起票する。
- `NotRun`/`Fail`/`Unsupported` が残る node を `Pass`
  扱いしない。merge 後 main SHA の必須 CI 結果が統合
  検証の証拠。
- 実 Window/実 Device/実 Plugin/実 Provider の L2/L3
  実測は synvelis#114/#176・ruviform#313/#243/#251/
  #253/#388/#389/#532 が担う（本 slice 対象外）。
- macOS/Linux の GUI-L2/EXT-L3 は利用要件発生まで
  Deferred。AudioLens f64/長尺の非回帰実測は
  Ruviform#294 candidate SHA Gate。
