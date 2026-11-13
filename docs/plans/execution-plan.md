---
id: "PLAN-034X841J2QRrgpxCPlRiKb"
type: "plan"
status: "in-progress"
title: "Execution Plan Plan — docs chain + sv23 evidence ledger"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/execution-plan.md"
    - "docs/designs/execution-plan-design.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/execution-plan.md"
    - "docs/designs/execution-plan-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/execution-plan.md"
    - "docs/designs/execution-plan-design.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/project-model.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/transport-integration.md"
    - "docs/specs/musical-time.md"
    - "docs/specs/recording-take-comping.md"
    - "docs/specs/automation-modulation.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/specs/svt-product-e2e.md"
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
    - "docs/specs/ci-os-l1-native-gpui.md"
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/recording-take-comping-design.md"
    - "docs/designs/automation-modulation-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/execution-plan-design.md"
  derived-by:
    - "docs/tasks/execution-plan-task.md"
  verifies: []
  verified-by:
    - "docs/test-specs/execution-plan.md"
  references:
    - "docs/test-specs/execution-plan.md"
    - "evidence/sv23/execution-plan-gate-ledger.md"
    - "evidence/sv23/execution-plan-entry-gate-ledger.md"
  defers: []
  deferred-by: []
---

# Execution Plan Plan — docs chain + sv23 evidence ledger Implementation Plan

## Goal

Issue #23 の横断実行計画（RCP-00〜07/F01・F0/E2E-L/R/X・
Signal Flow/Automation/GUI の Start/Integration/Acceptance
単一 DAG・Owner/Gate/Acceptance Matrix・OS-L1/GUI-L2/
EXT-L3 検証 matrix）を canonical 文書鎖
（spec→design→plan→task→test-spec→impl ir/exp +
evidence/sv23）として確立する。実装・CI 変更は行わない。

## Scope

単一 docs/ 設計契約ストリーム。コード・`Cargo.toml`/
`Cargo.lock`/`mise.toml`/`.github/workflows/` の変更なし。
実装は各 workstream の個別 Issue（#104〜118・#125〜128・
#175/#176/#177・#289〜297・GW/VCS/ALIB/CB/AUTX 各 Issue）
と Ruviform 各 Issue が別途担う。sv#21/sv#22 並行 Worker
の文書は参照のみ。

## Upstream Documents

- `docs/specs/execution-plan.md`（SPEC-034X83uiIsH0n6OuQGcU1T）
- `docs/designs/execution-plan-design.md`（DESIGN-034X83GOEQ84LyUmDRBepu）
- `docs/designs/synvelis-driven-ruviform-target-architecture-v07.md`
- `docs/designs/project-asset-persistence-architecture.md`
- `docs/designs/playback-mixer-generic-execution-graph-architecture.md`
- `docs/designs/configurable-ppqn-musical-time-architecture.md`
- `docs/designs/time-transport-midi-automation-plugin-integration-architecture.md`
- `docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md`
- `docs/designs/recording-take-comping-design.md`
- `docs/designs/automation-modulation-design.md`
- `docs/test-specs/execution-plan.md`
- `POLICIES.md`（OS-L1/GUI-L2/EXT-L3・Required/Deferred/
  NotApplicable・Pass/Fail/NotRun/Unsupported の規範）

## Design Inputs

- design の DAG Model・Workstream Lane・edge 表・Owner/Gate/
  Acceptance Matrix・OS Level 規則・Evidence Schema・
  正規入口対応・候補 SHA tuple Registry・Failure
  Taxonomy を正本とする。
- Issue #23 本文の Stage 3 分離・正規化 v1.1・GUI lane
  ordered DAG・RCP/CB/AUTX 追加 DAG・判定台帳を写像元と
  する。
- 個別 workstream docs chain（sv#10/#11/#12/#17〜#20・
  GUI 系・VCS/ALIB/OBS 系）を depends-on に固定し、
  sv#21/#22 は参照のみ。

## File Map

| File | 内容 |
| --- | --- |
| `docs/specs/execution-plan.md` | EP-R-01〜12 要件・EP-AC-01〜09 受入 |
| `docs/designs/execution-plan-design.md` | DAG Model・lane・edge 表・Owner/Gate Matrix・OS Level・Evidence Schema・入口対応・tuple Registry |
| `docs/plans/execution-plan.md` | 本書 — 実装 Owner 写像・証拠計画 |
| `docs/tasks/execution-plan-task.md` | タスク分解・done when |
| `docs/test-specs/execution-plan.md` | DAG/matrix/OS level の不変条件と検証規則 |
| `docs/impl/ir/execution-plan.md` | 実施記録 |
| `docs/impl/exp/execution-plan.md` | 試行・契約照合 log |
| `evidence/sv23/execution-plan-gate-ledger.md` | workstream node × Issue × Owner × Start/Integration/Acceptance × OS Level × 状態台帳 |
| `evidence/sv23/execution-plan-entry-gate-ledger.md` | 正規入口 × OS × Gate/Feature × SHA × 結果状態台帳 |
| `evidence/sv23/execution-plan-verification.md` | 検証実行記録 |

## Implementation Streams（実装 Owner への写像）

本 slice は docs のみ。実装 Owner への写像は design の
lane 表が正本。要約:

| Stream | 実装 Owner | 本計画が供給するもの |
| --- | --- | --- |
| F0/E2E-L/R/X | synvelis #13/#24/#25/#14〜16/#26 | Gate-A への acceptance edge と L1 Oracle 対応 |
| Signal Flow | synvelis #104〜109 + ruviform graph/probe | Start/Integration/Acceptance 分離と SF-INTEGRATION-GATE |
| Control/Automation | synvelis #98〜103・#121/#122・#131〜134 | #103 Oracle PASS は #133 製品受入条件（start ではない） |
| Recording | synvelis #12・#115〜120 + ruviform#187/#251 | EXT-L3-WIN の提供範囲限定規則 |
| Time/MIDI/Plugin | synvelis #125〜130 + ruviform#188/#253/#307 | EXT-L3-PLUGIN Gate |
| GUI Native | synvelis #110〜114・#175〜177 + ruviform#385〜390/#464/#313/#243/#262/#419/#532 | GUI-L2-WIN Gate・#114/#313/#243 紐付け |
| Gateway/AI | synvelis #44〜46・GW-01〜10 | AI-Gate 独立軸 |
| VCS/ALIB/OBS | synvelis #85〜97・#81 + ruviform OBS | 各 Cross Gate |
| CB/AUTX | synvelis #259〜267・#277〜288 + ruviform#562 | #264→#265 start・#261→mutating CLI acceptance |
| RCP | synvelis #289〜297 + ruviform#196/#198/#562 | RCP-07 Cross Gate・F01 後続 |
| CI/証拠 | synvelis #28/#83 + ruviform#219/#294 + audio-lens#127/#142/#166 | 候補 SHA tuple・Daily/手動 Full のみ |

## Dependency Graph

```
spec → design → test-spec → plan → task → impl(ir/exp) → evidence
                                   ↑ verified-by
実装は design の edge 表を各 Issue が参照。docs slice は
実装に依存しない（計画は実装前に固定する）。
```

## Risk Register

| Risk | 対応 |
| --- | --- |
| DAG 循環・Owner 重複の混入 | test-spec の不変条件（非巡回・edge 型・Owner 一意）で検証。違反は design を修正 |
| headless/Fake Pass の実機誤昇格 | evidence 台帳で capability/OS Level 別に分離。`gpui-test`/`webview`/Fake Driver を GUI-L2/EXT-L3 Pass にしない |
| `NotRun` の `Pass`/`Unsupported` 誤記 | 要求状態と実行結果を別軸で記録（POLICIES.md §3） |
| Issue 状態の陳腐化 | 台帳に写像時点の Issue state を記録し、変化は台帳更新で追従 |
| index/生成物のみの競合 | merge → `index apply` 再生成で解消（検証流用可） |
| graph router の既存 blocker | baseline finding として evidence に記録、範囲外は修復しない |
| sv#21/#22 並行 Worker との path/内容競合 | 本 slice の file map は `execution-plan*` と `evidence/sv23/` に限定。競合時は Coordinator へ確認 |

## Verification Plan

| 段 | 検証 | 手段 |
| --- | --- | --- |
| docs 契約 | lint/index/impl audit | `doc_status.js lint`・`doc_maintenance.js index apply`・`audit_impl_record.js` |
| 体裁 | fmt | `mise run fmt-check` |
| graph | route | `route_graph.js --focus execution-plan --json` |
| DAG 不変条件 | 非巡回・edge 型・Owner 一意・語彙分離 | `docs/test-specs/execution-plan.md` の規則を evidence 台帳へ適用 |
| review | blocker なし | `review-protocol` |
| 実装/Gate | 各 workstream の OS-L1/GUI-L2/EXT-L3 | 個別 Issue・Daily/手動 Full CI（本 slice 対象外・NotRun） |

## Rollback

- docs のみの追加。revert で全削除可能。index は再生成で
  復元。既存文書・コードへの変更なし。

## Task Breakdown

`docs/tasks/execution-plan-task.md` を参照。docs 生成→
evidence 台帳→index 再生成→lint/audit→review→
commit/push/PR の順。

## Open Questions / Gaps

- 実 Window/実 Device/実 Plugin/実 Provider の L2/L3 実測は
  本 slice では NotRun。Windows で実際に提供する機能の
  確定範囲は #176・#115・#117・#126/#127 と Ruviform
  capability Gate に委譲。
- macOS/Linux の GUI-L2/EXT-L3 は利用要件発生まで Deferred。
- AudioLens f64/長尺の非回帰実測は Ruviform#294 candidate
  SHA Gate（別 Gate）。
- RCP-F01・AUT-F・RCD-F・CTL-F・TIM-F 系は初期 Gate 非必須
  の後続として Open のまま管理。

## Self-Review

- spec EP-R-01〜12 ↔ design DAG/Matrix/Schema ↔ test-spec
  不変条件 ↔ evidence 台帳の対応を確認。
- Start/Integration/Acceptance の 3 相分離と、headless/
  Fake Pass を実機受入にしない規則を全層で一貫させた。
- Owner 分離（Synvelis 製品 Domain vs Ruviform 共通基盤
  vs AudioLens Consumer vs CI）を lane/Matrix で固定。
- 新 CI Workflow を増設しないことを plan/verification に
  明記。

## Implementation Handoff

- 実装 Owner は design の lane/edge 表と `evidence/sv23/`
  台帳を参照し、自分の node の Start/Integration/
  Acceptance と OS Level を確認する。
- 共通基盤の不足は Synvelis 側で恒久重複実装せず
  Ruviform Issue へ起票する。
- `NotRun`/`Fail`/`Unsupported` が残る node を `Pass`
  扱いしない。merge 後 main SHA の必須 CI 結果が統合
  検証の証拠。

## Review Handoff

- reviewer は spec/design/test-spec/evidence の整合・
  DAG 非巡回性・Owner 分離・OS Level 規則・evidence
  誠実さを確認。`review-protocol` で blocker なしを確認。
