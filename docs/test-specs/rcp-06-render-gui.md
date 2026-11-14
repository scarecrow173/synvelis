---
id: "TSPEC-Rcp06RenderGuiTestX"
type: "test-spec"
status: "approved"
title: "RCP-06 Missing Plugin/Render GUI — presentation model・ViewSpec・action 契約 fixture tests"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/295"
  verifies:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/adr/rcp-00-render-recipe.md"
  supersedes: []
  superseded_by: []
---

# RCP-06 render GUI fixture tests

## Purpose

synvelis#295 RCP-06 headless スライスの fixture が保証すべき
振る舞いを固定する。5 軸 presentation model（Editability/
Executability/RenderValidity/PlaybackQuality/RuntimeState）の
分離、Missing Plugin での edit-possible/blocked 併存、preview の
non-exact 保証、Branch Difference Explorer/Policy/Request/Job の
投影、共有 Reason code・snapshot pin・action id 語彙、grant
fail-closed、冪等 job control、view-local state の永続化寛容性を
`crates/synvelis-workspace/tests/rcp06_render_ui.rs`（17 case）と
`crates/synvelis-app/tests/rcp06_render.rs`（11 case）で検証する。
実 GPUI/実 OS・実 Plugin・実 VCS provider・CLI/Workflow/Script
実 surface・PCM Golden・AudioLens Consumer・cross-gate RCP-07 は
本 spec の対象外（`evidence/sv295/` で NotRun/Deferred 記録）。

## Feature

- `synvelis-workspace::render_ui` が RCP-01..RCP-04 authority 型を
  display read-model に投影し、`RenderReason` code・snapshot pin・
  `ACTION_RENDER_*` id を UI/CLI 共有語彙として提供する。
- `synvelis-app::render` が read-model を driver-neutral
  `ViewSpec` + binding decode に投影し、executable candidate のみ
  button 化・未知 intent fail-closed を保証する。
- `synvelis-theme::StatusTone::of_label` が 5 軸 + job state label
  を tone に写像する（色は副次 cue）。

## Rules and Examples

### RCP-UX-01 — 軸分離と非色意味付け

| Case | Fixture | Expected |
| --- | --- | --- |
| WS-UX01-01 | `PluginVerdict::Missing` node | `edit-possible` + `blocked(PluginMissing)` + `render-missing` + `quality-unavailable` |
| WS-UX01-02 | `AutomationIdentityIncomplete` | `edit-blocked(PluginAutomationIncomplete)` |
| WS-UX01-03 | `Live` + `EligibleExact` + `BitExactCertified` | `live` + `render-valid` + `quality-exact-artifact` + `runtime-active` |
| WS-UX01-04 | preview-class substitution | `quality-preview`（Exact を偽らない）+ substitution overlay `preview_class` |
| WS-UX01-05 | `last_good_mismatch` | `runtime-last-good-mismatch` |
| WS-UX01-06 | `LookupOutcome` 全 variant | `EligibleExact→Valid`/`EligiblePreview→Preview`/他は各対応軸 |
| APP-UX01-01 | missing node の status spec | 軸 label・`capability.plugin-missing` code が spec text に出る |
| TH-UX01-01 | `StatusTone::of_label` | 全軸 label が一意 tone に解決・未知は `Unknown` |

### RCP-UX-02 — 共有 Reason code / pin / Branch diff

| Case | Fixture | Expected |
| --- | --- | --- |
| WS-UX02-01 | `NonEquivalent` selection | verdict `non-equivalent-preview`（`exact` でない）+ pin 世代 + unresolved reason |
| WS-UX02-02 | `Blocked` selection | verdict `blocked`・`non_equivalent=false` |
| WS-UX02-03 | `project_branch_diff` | pin text・boundary coverage・changed params・missing plugins・semantic diff・shared reason code |
| APP-UX02-01 | branch diff spec | pin・coverage・`render.artifact-unknown`・recipe key prefix が spec text に出る |

### RCP-UX-03 — Policy 投影と trigger-only 不変性

| Case | Fixture | Expected |
| --- | --- | --- |
| WS-UX03-01 | `CommitCapability::Unsupported` | OnCommit provider=`CommitUnsupported`（code `policy.commit-unsupported`）、disabled 行は `TriggerDisabled`、Manual unconditional、`recipe_invariant=true`、provenance 非空 |
| WS-UX03-02 | draft trigger edit | `PolicyPatch` にのみ lowering、Manual off 拒否、ClearDraft で `None` |
| APP-UX03-01 | policy 入り status spec | trigger/provider/commit code・`recipe-invariant`・verdict/pin が spec text に出る |

### RCP-UX-04 — 共有 action 語彙と grant fail-closed

| Case | Fixture | Expected |
| --- | --- | --- |
| WS-UX04-01 | unresolved demand の request preview | coverage `0/1`、request=`SelectionBlocked`、publish=`GrantRequired`、plan/inspect executable |
| WS-UX04-02 | `render_request_report` | Demand event 無 grant→`GrantRequired`、publish 無 grant→`GrantRequired`、Manual+request は `Ok`、GUI/CLI 同一 triple で action/grant 一致・entry のみ差 |
| APP-UX04-01 | request spec | executable request のみ button、publish は button なし・disabled reason text のみ |
| APP-UX04-02 | decode | `job-control`/`toggle-trigger`/`set-*`/`request` が typed `RenderCommand` に decode、未知・malformed は `None` |

### RCP-UX-05 — 冪等 job control

| Case | Fixture | Expected |
| --- | --- | --- |
| WS-UX05-01 | `JobRecord` 各 state | Rendering→cancel のみ、Failed(retryable)→retry のみ、Failed(AuthDenied)→verify のみ、cancel_requested→cancel なし |
| WS-UX05-02 | `Partial` + manifests + receipts | `project_render_job` が state/entry/key summary/completeness gap/receipt tail を投影、attach executable |
| WS-UX05-03 | `render_job_command` | 同一 key+op で同一 command（coalesce 保証） |

### view-local state

| Case | Fixture | Expected |
| --- | --- | --- |
| WS-VS-01 | `RenderViewState` serialize→未知 field 付き deserialize | round-trip 成功・tolerant |
| APP-VS-01 | `apply_render_view_command` | `ViewLocal` のみ state 更新、domain command は no-op |

## NotRun / Deferred（Case ID）

| Case ID | 内容 | 状態 | 理由 |
| --- | --- | --- | --- |
| RCP-UX-06-GPUI | 実 GPUI/実 OS での Dock/Focus/Keyboard/Window Close/長時間 Job | NotRun | 専用 real GUI 自動レーンが現行にない。headless PASS を実機 PASS に昇格しない |
| RCP-UX-04-CLI | 実 CLI/Workflow/Luau/Rhai/Fake AI surface | Deferred | sv#273 他 lease。共有 action id/`TriggerReport` lowering の contract test のみ実施 |
| RCP-UX-01-RT | 実 Plugin/実 VCS/PCM Golden/AudioLens Consumer | NotRun | 別 Gate |
| RCP-UX-07 | cross-gate | Deferred | RCP-07 scope |
