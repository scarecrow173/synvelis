---
id: "DESIGN-0886aYIscVs3Pjh9Ep5J8a"
type: "design"
status: "approved"
title: "Issue #514 design — selection 非消費の責務境界と将来の配線条件"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/514"
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/discovery/issue-514-session-engine-selection.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-514-session-engine-selection.md"
  implemented-by:
    - "docs/plans/issue-514-session-engine-selection.md"
  depends-on:
    - "docs/specs/issue-514-session-engine-selection.md"
    - "docs/adr/issue-514-session-engine-selection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv293-semantic-selection-integration.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
    - "evidence/sv514/decision-ledger.md"
  refines:
    - "docs/designs/sv293-semantic-selection-integration.md"
  refined-by: []
  derives-from:
    - "docs/specs/issue-514-session-engine-selection.md"
  derived-by:
    - "docs/plans/issue-514-session-engine-selection.md"
    - "docs/test-specs/issue-514-session-engine-selection.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-app/src/engine.rs"
    - "crates/synvelis-workspace/src/rcp_selection.rs"
    - "crates/synvelis-player/src/routing_engine.rs"
    - "crates/synvelis-player/src/rcp/verify.rs"
    - "crates/synvelis-player/src/rcp/model.rs"
  defers: []
  deferred-by: []
---

# Issue #514 design — selection 非消費の責務境界

## 責務分解（現行契約）

| 層 | 責務 | selection との関係 |
| --- | --- | --- |
| `synvelis-workspace::rcp_selection` | semantic model → RCP snapshot → `ExecutionSelection` + `ReconciliationReceipt` の構築、stale pin の pre-gate、engine への plan 駆動 | selection の producer / driver（caller 駆動） |
| `synvelis-player::RoutingEngine` | prepare/publish・checked_key（revision・fingerprint・generation・epoch・runtime instance）・compile budget・host connect・PluginUnavailable・last-good/live-plan 保持・activation receipt | selection を消費しない。engine build の唯一の安全性 oracle |
| `synvelis-app::SessionRoutingEngine` | `EnginePort` 通知の受領、`RoutingDeps`/`RoutingCompileRequest` 組立、`prepare`/`publish` 転送、control surface | plan 実行 adapter。selection 入力を持たず、selection を仲介しない |
| `synvelis-render` | render intent（OfflineRender/Export）側の lowering・artifact lookup | 別 intent 経路。SessionRoutingEngine を経由しない |

## なぜ app 層で selection を仲介しないか（root-cause）

### 1. `EnginePort` は selection の入力を運ばない

`on_project_revision(&Project, ProjectRevision)` のみ。intent・
plugin 証拠・environment facts・render candidates は通知に含まれず、
`SessionEngineConfig`/composition 経路にも供給 port が存在しない。
adapter が selection を「消費」するには、まず供給経路を新設する
必要があるが、その供給元自体が未着地（後述）。

### 2. `drive_selection_plan` の engine build は rebuild と同一

両経路とも `RoutingEngine::prepare(request, project, revision,
deps)` → `publish` を駆動する。違いは (a) PlanReconciler receipt、
(b) `selection_pin_fault` による stale pin の pre-publish 拒否、
(c) `Equivalent` verdict で publish を skip する caller 判断のみ。
**published plan の内容は変わらない** — `lower_semantic` は常に
`project.routing` 全体を lowering し、`ExecutionSelection.
substitutions` は decision 証拠として receipt に残るだけで plan
shaping に使われない。

つまり現契約で selection を配線しても得られるのは「receipt が増える」
ことだけで、missing-plugin substitution の実再生・artifact への
置換は実現できない。substitution を plan に反映するには engine 側の
lowering/plan 契約変更が必要であり、`synvelis-player` は本 Issue の
lease 外（Issue 本文指定: 変更せず別 Issue 起票）。

### 3. `SelectionInputs` の production 供給経路が存在しない

- `plugin_evidence`: `ProcessorBinding` は descriptor/version/
  capability_revision のみ。`PluginIdentity`（format/vendor/uid/
  version/binary_digest）・`PluginStateDecl`（TIM-04 capture）・
  automation completeness の producer がない。
- `environments`: `PluginHost` は Scan/Instantiate のみ公開。
  `PluginEnvironment`（installed/access/format_supported/
  resolved_version/resolved_binary_digest/advertised_ports/
  state_restore/uid_hash）の供給 API がない。
- `render_candidates`: `SemanticRenderCandidate{coverage,
  CandidateReport}`。`CandidateReport` は synvelis-render の lookup
  経路由来、`SemanticRenderCoverage`（artifact→semantic subgraph
  対応）の producer は RCP-01/02/05 領域で未着地。

app が candidate を組み立てるには synvelis-render dep が必要になる
（app は render 非依存の設計）。workspace 側供給 port で回避可能だが、
供給元がない以上 port を置いても中身がない。

### 4. verdict を hard gate にすると失敗モードが増えるだけ

- 空 evidence → `MissingPluginEvidence` fail-closed → 全 plugin
  project が build 不能（回帰）。
- 捏造/仮 evidence → honest-input 契約違反・テスト用 pin の本番
  混入。
- 部分的な実 evidence → `ProcessorHost::connect`（実 instantiate
  = ground truth）より弱い caller 申告情報で gate すると、engine が
  build 可能な plan を false-block し得る。能力は増えず、
  evidence/host 乖離という新しい失敗モードを持ち込む。

### 5. 安全性 pin は既に閉じている

Issue が「実装する」分岐で要求する pin:

- stale selection が publish されない → `selection_pin_fault`
  pre-gate（`drive_selection_plan` 内、engine 呼出前）+
  `rcp_ex05_stale_selection_never_publishes`。
- generation+epoch 不一致で旧 plan が Active にならない →
  `checked_key`（project_revision・routing_fingerprint・
  port_capability_generation・engine config generation・source
  format generation・lowering policy・build mode・device clock
  epoch・runtime instance）+ publish 再検証、player `rcp_04` oracle、
  `ex05_*` 系。
- QueueFull/RetireOverflow で last-good 保持 →
  `ex05_queue_full_and_retire_failure_retain_last_good` + engine の
  receipt 契約。
- adapter レベル → `sf03_session_engine.rs` が publish→Active→
  last-good→equivalence→stale を pin 済み。

adapter はこれらを engine から継承しており、selection を介しても
同じ不変条件が同じ層で守られる。二重に gate しても保証は増えない。

## 将来 app 層配線が必要になる条件（trigger）

以下が全て揃った時点で、per-intent selection service または
adapter 経路の配線を再検討する:

1. `SelectionInputs` の production 供給経路が存在する
   （plugin env scan API・TIM-04 `PluginStateDecl` 供給・
   artifact coverage mapping）。
2. `ExecutionSelection` の consumer が確定する（sv#295 の live
   selection 表示面、または substitution を実再生に反映する
   plan shaping — 後者は `synvelis-player` 契約変更を伴う可能性が
   高く別 Issue 領域）。
3. 対象 intent が確定する（MainPlayback は rebuild 経路、
   EditingPreview/OfflineRender/Export はそれぞれの経路 — 単一の
   rebuild hot path で全 intent を賄う設計ではない）。

現時点では 1・2 が未着地のため、配線は「供給のない consumer」を
作るだけになる。

## stateless `PluginStateDecl`（AC4）の扱い

`PluginStateDecl.digest: Option<String>` は「stateless（state を
持たない plugin）」と「state を capture していない」を型上区別
できない。`verify_plugin` は `StateRestoreSupport::Verified` の
Live 条件に `digest.is_some()` を要求し、`digest: None` は
`StateUnknown` へ fail-closed する
（`crates/synvelis-player/src/rcp/verify.rs`）。
`StateCaptureQuality::Unsupported`（plugin が state
save/restore を実装しない）の語彙は型に存在するが、
`StateRestoreSupport` に「適用外」を表す variant がなく、
workspace 側で verdict を変える lever がない — 判定ロジックは
全て player 内。player 契約変更が必要なため本 Issue では実装せず、
別 Issue を起票して帰属を記録する（`evidence/sv514/
decision-ledger.md` 参照）。
