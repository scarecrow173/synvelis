---
id: "IMPL-102UiCfInt006FfGgHhIiJ"
type: "impl"
status: "completed"
title: "UI-C5 製品 GUI 統合 — syn.control.relation / syn.control.edge / syn.recording.* op surface（headless slice）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
metadata:
  experiments:
    adopted: []
    rejected: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/uic5_app.rs"
      - type: "file"
        path: "docs/specs/ui-sf-control-integration.md"
      - type: "file"
        path: "docs/designs/ui-sf-control-integration-design.md"
      - type: "file"
        path: "docs/plans/ui-sf-control-integration.md"
      - type: "file"
        path: "docs/tasks/ui-sf-control-integration.md"
      - type: "file"
        path: "docs/test-specs/ui-sf-control-integration.md"
      - type: "file"
        path: "docs/impl/ir/ui-sf-control-integration.md"
      - type: "file"
        path: "docs/impl/exp/ui-sf-control-integration.md"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/relation_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/state.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/workspace.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/uic5.rs"
  implements:
    - "docs/plans/ui-sf-control-integration.md"
  implemented-by: []
  depends-on:
    - "docs/plans/ui-sf-control-integration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ui-sf-control-integration.md"
    - "docs/designs/ui-sf-control-integration-design.md"
    - "docs/test-specs/ui-sf-control-integration.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/ui-sf-control-integration.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# UI-C5 製品 GUI 統合 — Implementation Record

## 実装サマリ

先行 slice（PC-C5-01/02-03/04）が `synvelis-workspace` に実装した
Control Relation projection・edge gesture・recording status を、
`synvelis-app` の `syn.*` op surface へ型付きで接続した。
app 層は wire parse/serialize と op dispatch のみを担い、
domain semantics の複写は行っていない。

## 変更内容

### `synvelis-workspace`

- `relation_ui.rs`: `entity_label` の tagged id 生成を
  `control_entity_tag` として公開し、逆変換 `control_entity_parse`
  を追加（4 種の id parser を順に試し、prefix で一意に確定）。
  `ControlRelationInspectionFocus`（target=tagged 文字列、
  mode/follow_source 再利用、explore stacks は serde skip）と
  `ControlRelationViewState`（`deny_unknown_fields` で
  malformed → None 劣化）を新設。
- `state.rs`: `ViewState.control_relation: Option<...>` を
  `signal_flow` と同型の tolerant decode で追加。
- `workspace.rs`: `control_relation_state`/`inspect`/
  `set_inspection_mode`/`set_follow_source`/`explore_back`/
  `explore_forward` を追加。inspect は editing selection・
  `inspection_focus`・project・undo に触れない。Explore 離脱で
  履歴クリア、同一 anchor 再 inspect は stack 不変。
- `lib.rs`: 新規公開物を re-export。

### `synvelis-app`

- `ui_ops.rs`: 4 op を実装。
  - `op_control_relation`: action `view|inspect|set_mode|
    set_follow_source|back|forward`。`view`+`anchor` は
    stateless query（focus 不変）、他 action の応答 relation は
    常に現在 focus anchor。応答は `{ok, view, inspection,
    relation}` で coverage/project_revision を verbatim に保持。
  - `op_control_edge`: action `begin|preview|commit|cancel|state`。
    begin は `ModulationSource→Parameter` を app 境界で先に検証し、
    workspace の CAS/exclusivity に委譲。outcome は
    `applied{revision}|cancelled{reason}|stale` の typed 値。
  - `op_recording_status`: `recording_status(session)` を JSON 化。
    `Prepare` は caller-held payload が必要で構造上到達不能
    （module docs の契約どおり）。
  - `op_recording_action`: `inspect|retry|rebase|abort` を
    `control_record_rebase_preview`/`control_record_commit`/
    `control_record_approve_rebase`/`control_record_abort` に
    委譲。`RecordCommitError` は error detail に verbatim。
  - wire helper: `control_entity`/`control_entity_json`/
    `control_entity_param`（tagged id parse/serialize 一元化）、
    `control_relation_inspection_json`/`relation_view_json`/
    `control_edge_state_json`/`control_edge_gesture_json`/
    `control_edge_outcome_json`/`recording_status_json`/
    `recording_txn_json`/`recording_outcome_view_json`/
    `recording_provenance_json`/`rebase_preview_json`/
    `recording_outcome_json`/`record_disposition_json`/
    `recording_txn_id_param`/`record_commit_err`/`ui_kebab`/
    `polarity_str`。
- `service.rs`: `OpInfo` 4 件 + dispatch arm 4 件を登録。

### テスト

- `synvelis-workspace/tests/uic5.rs`: control_relation focus の
  契約テスト 2 件追加（inspect/explore/mode/undo 非接触、
  snapshot JSON 往復 + malformed 劣化）。
- `synvelis-app/tests/uic5_app.rs`: 新設 9 テスト。
  relation 双方向探索・back/forward・stateless anchor・
  inspect≠edit/undo 非接触・malformed fail-closed、
  edge begin/preview/commit/cancel/state・stale revision drop・
  routing gesture との分離、recording unknown/pending/conflict/
  applied・inspect/rebase/retry/abort・typed provenance。

## 設計上の判断

- **anchor の適用範囲**: `params.anchor` は `view` action のみで
  有効化。他 action の応答 relation は常に現在 focus を反映し、
  「inspect 後に別 anchor の relation が返る」混乱を防ぐ。
- **`Prepare` の非到達**: `recording_status` は session のみを
  見る read model で、caller-held `PreparedRecordingCommit` を
  観測できない。`Prepare` state の表示は `recording_status_with`
  経路の責務であり、本 op では構造上到達不能 — op doc に明記。
- **provenance の serialize**: `RecordDisposition` は
  `record_disposition_json` で `{kind: committed|aborted, id,
  revision}` に構造化。`Debug` 文字列（`"aborted { id: ... }"`）
  を wire に出さない。
- **cancel reason**: `CancelReason` の serde 表現
  （`"escape"` 等 lowercase）をそのまま返し、入力語彙と一致させる。
- **in-memory restore**: `capture`/`restore` は serde を通らない
  ため session-only explore stacks も保持される。永続化契約は
  JSON 境界（`to_json`/`from_json`）で規定 — `signal_flow` と同じ。

## 検証結果

- `mise run fmt-check`: pass
- `mise run check synvelis-app`: pass
- `mise run check synvelis-workspace`: pass
- `mise run lint synvelis-app`: pass
- `mise run lint synvelis-workspace`: pass
- `mise run test synvelis-app --test uic5_app`: 9/9 pass
- `mise run test synvelis-workspace --test uic5`: 14/14 pass
- `mise run test synvelis-app` / `mise run test synvelis-workspace`:
  （全量結果は PR 本文に記録）

## 未対応・Deferred

- 実 Window/GUI/plugin window/高 DPI/keyboard/IME/accessibility の
  実機検証 — product usage が activate するまで Deferred
  （UI-C5-05 の headless 分のみ実施）。
- link event → `ControlEntityRef` の retarget — 型写像がなく、
  Follow focus の安定性が契約（design doc 参照）。
- `synvelis-project` 本体の変更 — 不要だった（既存 read-only
  projection への接続のみで完結）。
