---
id: "TSPEC-102UiCfInt005EeFfGgHhI"
type: "test-spec"
status: "approved"
title: "UI-C5 製品 GUI 統合テスト仕様 — syn.control.relation / syn.control.edge / syn.recording.*（headless）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ui-sf-control-integration.md"
    - "docs/designs/ui-sf-control-integration-design.md"
    - "docs/tasks/ui-sf-control-integration.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/ui-sf-control-integration.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# UI-C5 製品 GUI 統合テスト仕様 — syn.control.relation / syn.control.edge / syn.recording.*（headless）

## Purpose

UI-C5-01〜04 の wire surface 契約を headless で固定する。
synvelis#102 の domain semantics（relation index・gesture CAS・
recording transaction）は先行 slice で検証済みであり、本 spec は
それらを `synvelis-app` op として公開した際の adapter 契約
（parse/serialize・focus state・outcome 語彙・委譲の正しさ）を
対象とする。実 Window/GUI/plugin/高 DPI/IME/a11y は本 spec の
対象外であり、headless の成功をもって GUI Pass を主張しない。

## Feature

`synvelis-app` の `syn.control.relation`・`syn.control.edge`・
`syn.recording.status`・`syn.recording.action` op が、
Workspace の Control Relation inspection focus・Control Layer
edge gesture・recording status read model を型付き JSON として
製品 GUI へ提供する。

## Rules

- `syn.control.relation` の `inspect` は editing selection・
  `inspection_focus`・project revision・undo depth を変えない。
- `view` + `anchor` は focus を動かさない stateless query であり、
  その他の action の応答 relation は常に現在の focus anchor を
  反映する。
- Explore back/forward は履歴を往復し、Explore を離れると履歴は
  クリアされる。Pinned は anchor を保持する。
- malformed entity tag / 未知 kind / 未知 action / 未知 mode は
  `ErrorBody` で fail closed — 推測した entity へ退化しない。
- `syn.control.edge` の `begin` は `ModulationSource → Parameter`
  のみ受理し、実行中 gesture の二重 begin・project 変更後の
  commit はそれぞれ拒否 / `stale` outcome になる。
- `preview`/`cancel` は project revision と undo depth を変えない。
  `commit` はちょうど 1 `ProjectOp`（`AddModulationAssignment`）
  と 1 undo 境界を生む。
- `syn.control.edge` の gesture は `syn.signal_flow.routing` の
  gesture と別 state slot に存在し、相互に見えない。
- `syn.recording.status` は `conflict|prepare|pending|applied|
  unknown` を返し、各 state の合法 action のみを `actions` に
  載せる。Conflict は Retry を含まない。
- `syn.recording.action` の `inspect|retry|rebase|abort` は
  `ProjectSession::control_record_*` へ委譲され、失敗は typed
  `RecordCommitError` として error detail に現れる。
- provenance は `kind` tag + typed scalar を持ち、`Debug` 文字列は
  複合 domain payload の `detail` に限定される。

## Examples

- Given a seeded control neighborhood（param/lane/source/
  assignment）、When `inspect(param)` → `inspect(assignment)` →
  `back`、Then focus は param へ戻り、`can_forward` が true になる。
- Given 実行中の control edge gesture、When `preview(depth=0.5,
  polarity=unipolar)` → `commit`、Then `applied{revision=rev+1}`、
  assignment が preview 値で 1 件だけ存在し undo で除去できる。
- Given begin 後に foreign `SetProjectName`、When `commit`、
  Then `stale` が返り assignment は作られない。
- Given open recording transaction + foreign revision drift、
  When status を読む、Then `conflict`・`revision_stale=true`・
  sealed=`project-change`・actions に retry が無い。
- When `inspect` → `rebase` → `retry`、Then `pending` → `applied`
  へ遷移し receipt provenance が現れる。

## Guarantees

- relation/gesture/recording の domain semantics は workspace/
  project に一元化され、app は wire 境界のみを担う。
- gesture outcome（applied/stale/cancelled）は error ではなく
  typed `outcome` 値として返る。
- `ViewState.control_relation` は snapshot JSON 往復で
  target/mode/follow_source を保持し、explore 履歴を復活させず、
  malformed payload を `None` に劣化させる。
- headless テストは `GpuiSimulated`/headless lane として記録され、
  実 GUI acceptance を装わない。

## Non-goals

- 実 Window・dock・focus の GUI 実機検証（Deferred）。
- `synvelis-project` の変更。
- link event から control entity への retarget（型写像が存在せず、
  Follow focus の安定性が契約）。
- `Prepare` state の op 経由到達（caller-held payload が必要で
  session は観測不能 — `recording_status_with` 経路は将来課題）。

## Risk

- wire shape の enum 語彙が他 op とずれる → kebab-case と
  tagged id の規約を `sf_entity`/`signal_flow` 系に揃えて固定。
- `Debug` 文字列の混入で provenance が型を失う → serializer で
  kind tag + scalar fields に限定し、テストで `kind` 存在を検査。
