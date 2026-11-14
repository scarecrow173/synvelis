---
id: "DESIGN-102UiCfInt002BbCcDdEeF"
type: "design"
status: "approved"
title: "UI-C5 製品 GUI 統合 設計 — syn.control.relation / syn.control.edge / syn.recording.* op surface"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ui-sf-control-integration.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/ui-sf-control-integration.md"
    - "docs/tasks/ui-sf-control-integration.md"
    - "docs/test-specs/ui-sf-control-integration.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ui-sf-control-integration.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# UI-C5 製品 GUI 統合 設計

## 概要

`synvelis-app` は Workspace API の薄い型付き wire adapter とする。
relation traversal・gesture CAS・recording commit の semantics は
`synvelis-workspace`/`synvelis-project` の既存実装をそのまま呼び、
app 層では JSON 境界の parse/serialize と op dispatch のみを担う。

## レイヤー配置

```
GUI / test
   │  syn.control.relation / syn.control.edge / syn.recording.*
   ▼
synvelis-app  ui_ops.rs  (wire parse → Workspace call → JSON)
   ▼
synvelis-workspace  control_relation_* / control_edge_* / recording_status
   ▼
synvelis-project  RelationIndex / ProjectOp / ControlSession
```

依存方向は既存どおり app → workspace → project。app は domain rule を
持たない。

## Workspace 追加（`synvelis-workspace`）

### `ControlRelationViewState` / `ControlRelationInspectionFocus`

`relation_ui.rs` に新設。Signal Flow inspection focus
（`signal_flow_ui.rs`）と同じ Follow/Explore/Pinned semantics を
`ControlEntityRef` anchor に対して提供する。

- `target`: tagged id 文字列（`control_entity_tag`/`control_entity_parse`
  で round-trip。`ControlEntityRef` は serde を持たないため文字列で永続化）。
- `mode`/`follow_source`: `state.rs` の `InspectionMode`/`FollowSource`
  を再利用。
- `explore_back`/`explore_fwd`: `Vec<ControlEntityRef>`、serde skip
  （session-only。signal_flow / inspection_focus と同じ契約）。
- `#[serde(default, deny_unknown_fields)]`: `SignalFlowViewState` と
  同じ tolerant-decode 契約 — malformed payload は snapshot 全体を
  reject せず `None` へ安全劣化する。

### `ViewState.control_relation`

`Option<ControlRelationViewState>`、`signal_flow` と同型の
`deserialize_control_relation`（tolerant decode）付き。
`None` = その view が relation surface を使ったことがない。

### `WorkspaceManager::control_relation_*`

- `control_relation_state(view)` — read。
- `control_relation_inspect(view, entity)` — 現在 anchor を
  explore_back へ push し target を差し替え、mode=Explore。
  同一 anchor の再 inspect は stack を触らない。
- `control_relation_set_inspection_mode(view, mode)` — Explore 離脱で
  履歴クリア（`signal_flow_set_inspection_mode` と同規則）。
- `control_relation_set_follow_source(view, Option<FollowSource>)`。
- `control_relation_explore_back/forward(view)` — stack walk。
  "元画面へ戻る" は pop/push で path を失わない。

link delivery（`SemanticTarget` ベース）は `ControlEntityRef` へ
写像できないため Follow-mode control-relation focus の retarget は
no-op — multi-selection/dock/focus 変化下で対象がすり替わらない
（UI-C5-03）のが契約。これは「retarget 実装の欠落」ではなく、
型レベルで写像が存在しないことに由来する意図的な安定性。

## App op surface（`synvelis-app`）

### `syn.control.relation`

`params.view_id`（default: main view）、`params.action`:

| action | params | 効果 |
| --- | --- | --- |
| `view` | `anchor`? | read-only。focus + anchor（明示 or focus target）の RelationView |
| `inspect` | `entity` | Explore enter + push |
| `set_mode` | `mode` | follow/explore/pinned |
| `set_follow_source` | `follow_source` | null / `{view}` / `{group}` |
| `back` / `forward` | — | Explore history walk |

応答: `{ ok, view, inspection, relation }`。`inspection` は
`target`/`mode`/`follow_source`/`can_back`/`can_forward`。
`relation` は `RelationView` wire shape（`anchor`/`direction`/
`project_revision`/`coverage`/`rows[]`）— rows は
`source`/`target`/`kind`/`state`/`assignment` を verbatim に保持し、
coverage と revision が staleness の証拠として常に見える。

entity wire 形式は `sf_entity` と同型: `{"kind": "parameter"|
"automation-lane"|"modulation-source"|"assignment", "id": "<tagged>"}`
または bare tagged 文字列。`control_entity`/`control_entity_json`/
`control_entity_param` が parse/serialize を一元化する。

### `syn.control.edge`

`params.view_id` + `params.action`:

| action | params | 効果 |
| --- | --- | --- |
| `begin` | `source`,`target` | mod-source→param のみ受理。fixation 確定 |
| `preview` | `depth`?,`polarity`? | preview 値のみ更新（project 不変） |
| `commit` | — | fixated revision で 1 `AddModulationAssignment` |
| `cancel` | `reason`? | gesture 破棄（project 不変） |
| `state` | — | fixation + preview の read |

応答の `outcome`: `applied{revision}` / `cancelled{reason}` /
`stale`。workspace の `ControlEdgeOutcome` をそのまま wire 化する。
`CancelReason` は serde 表現（`"escape"` 等）をそのまま返す。

`begin` の endpoint 検証は 2 段: app 側で `ModulationSource→Parameter`
の型を先に確認し（早期の分かりやすい error）、workspace の
`control_edge_begin` が generic gesture/routing gesture/dock transfer
との排他と window epoch 固定を行う。`syn.signal_flow.routing` とは
別 op・別 `ViewState` slot（`control_edge_gesture` vs
`routing_gesture` vs `gesture`）で、相互に GestureActive を返す。

### `syn.recording.status`

`recording_status(session)` の `RecordingStatusView` を JSON 化。
`state` は kebab（`conflict|prepare|pending|applied|unknown`）、
`transaction`/`outcome`/`explanations[].provenance`/`actions` を
typed に serialize する。enum は kebab-case tag、
`Debug` 文字列は複合 domain payload の `detail` に限定し、
revision/id/count/flag は常に構造化フィールド。

`Prepare` は caller-held `PreparedRecordingCommit` が必要で
session は観測できないため、この op では構造上到達不能
（module docs どおり）。`recording_status_with` を呼ぶ経路が
将来必要になれば別 op/param で拡張する。

### `syn.recording.action`

`params.action`:

| action | 委譲先 | 応答 |
| --- | --- | --- |
| `inspect` | `control_record_rebase_preview` | `{preview: RebasePreview}` |
| `retry` | `control_record_commit` | `{outcome: RecordingOutcome}` |
| `rebase` | `control_record_approve_rebase(id)` | `{id}` — `params.id` 省略時は open txn |
| `abort` | `control_record_abort` | `{action}` |

`RecordCommitError` は `ErrorBody` の detail に Debug 表現で
verbatim に載せる（typed rejection を潰さない）。
成功系 op の後は `ui_pump()` で publish を回す。

## Wire shape の規約

- id は tagged 文字列（`param-…`/`automation-lane-…`/`mod-source-…`/
  `mod-assign-…`/`rec-tx-<n>`）。
- enum は kebab-case（`inspection.mode`、`relation.kind`/`state`/
  `coverage`/`direction`、recording `state`/`actions`/provenance `kind`）。
- `Option` は `null`、空 list は `[]`。
- 失敗は `ErrorBody`（invalid params / workspace refusal）。
  gesture outcome（applied/stale/cancelled）は error ではなく
  `outcome` field の typed 値 — stale drop は異常ではなく契約。

## 永続化・snapshot

`ViewState.control_relation` は `WorkspaceSnapshot` に乗る。
`target`/`mode`/`follow_source` は永続化、`explore_*` は skip。
restore は `deserialize_control_relation` の tolerant decode で
malformed → `None`。in-memory `capture`/`restore` は serde を
通らないため session-only field も保持される — 永続化契約は
JSON 境界（`to_json`/`from_json`）で規定する（`signal_flow` と同じ）。

## 検証戦略

- `synvelis-workspace/tests/uic5.rs` に control_relation focus の
  workspace 契約テストを追加（inspect/explore/mode/snapshot）。
- `synvelis-app/tests/uic5_app.rs` を新設し、op surface を
  `SynvelisApp` の op メソッド直接呼出で検証（`sf05_signal_flow.rs`
  と同じ headless seam。`UiRuntime::dispatch_op` と同一路）。
- 実 Window/GPUI driver 経路は本 slice では追加しない —
  view-layer 契約は workspace/app op の headless 証拠で担保し、
  GUI レーンは Deferred として正直に記録する。
