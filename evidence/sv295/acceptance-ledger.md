# Acceptance Ledger — synvelis#295 RCP-06 headless/driver-neutral 投影スライス

Date: 2026-11-15. Slice: RCP-06 Missing Plugin/Render
Availability・Policy UI・Action/CLI/Workflow 契約の
presentation model（`synvelis-workspace::render_ui`）+
driver-neutral ViewSpec/binding（`synvelis-app::render`）+
`StatusTone` 語彙拡張の headless 実装。実 GPUI/実 OS・
実 Plugin・実 VCS provider・CLI/Workflow/Luau/Rhai 実
surface（sv#273）・PCM Golden・AudioLens Consumer・
cross-gate（RCP-07）は DEFER/NotRun。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/295
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | feat/295-rcp06-render-gui HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 |
| Toolchain | mise 管理 Rust（`mise run` 経由） |
| Project schema | 既存 schema を read adapter で消費（変更なし） |
| Recipe/artifact/policy schema | RCP-01..RCP-03 既存契約を消費（変更なし） |
| doc graph route | `doc-driven-dev-graph` route は既存の unrelated broken relations（`artifact-graph` 等）で `blocked`/`focus-required` を返したため、RCP-03/AUTX-07 と同じ標準 doc chain（impl/ir・impl/exp・test-specs）で進行。relation 修復は本 lease 範囲外 |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-workspace/tests/
  rcp06_render_ui.rs`（17 case）と `crates/synvelis-app/tests/
  rcp06_render.rs`（11 case）が `mise run test` で green、
  および theme unit test。
- **NotRun**: 実 GPUI/実 OS/IME/DPI/2Window/pixel/RT・実
  Plugin・実 VCS provider・PCM Golden・AudioLens Consumer・
  cross-gate RCP-07 — 本スライスでは実行していない。
- **Deferred**: 後続 Issue/lease の範囲として明示的に送った
  部分（実 CLI/Workflow/Luau/Rhai/Fake AI surface = sv#273、
  RCP-UX-06 real GUI gate）。
- **Unsupported**: typed `Unsupported`/`GrantRequired` 返却が
  契約の経路は fixture で検証済み。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| RCP-UX-01 | Pass (headless) / NotRun (real GPUI-OS) | `missing_plugin_keeps_edit_possible_but_blocks_execution`（edit-possible + blocked(PluginMissing) + render-missing + quality-unavailable の 5 軸併存）、`automation_identity_incomplete_blocks_editing_too`、`live_plugin_with_exact_artifact_is_fully_available`、`preview_class_substitution_is_never_exact`、`last_good_mismatch_is_distinct_from_current_state`、`lookup_outcomes_map_to_distinct_validity_labels`（EligiblePreview→Preview で Exact を偽らない）、`status_spec_surfaces_axes_reasons_pins_and_policy_without_color`（軸 label + reason code が spec text に出る = 色非依存）、`rcp06_axis_labels_map_to_distinct_tones`（theme）。real OS gate は Case ID `RCP-UX-06-GPUI` で NotRun。 |
| RCP-UX-02 | Pass (headless) | `selection_projection_carries_pins_and_never_calls_preview_exact`（non-equivalent-preview verdict + pin 世代 + unresolved reason）、`blocked_selection_reports_blocked_verdict_and_reasons`、`branch_diff_shows_pins_boundaries_missing_plugins_and_semantics`（pin text/coverage/changed params/missing plugins/semantic diff/shared reason code）、`branch_diff_spec_shows_pins_boundaries_and_missing_plugins`。`SnapshotPinView::pin_text`/`RenderReason::code`/`ACTION_RENDER_*` が UI/CLI 共有の単一 source。 |
| RCP-UX-03 | Pass (headless) | `policy_projection_shows_commit_unsupported_with_shared_reason`（OnCommit provider=`policy.commit-unsupported`、disabled=`policy.trigger-disabled`、Manual unconditional、`recipe_invariant=true`、provenance）、`draft_trigger_edit_lowers_to_policy_patch_only`（draft→`PolicyPatch` のみ・Manual off 拒否・ClearDraft）、`status_spec_shows_policy_provenance_commit_and_trigger_block`。trigger-only 変更で RecipeKey 不変は RCP-03 `pol02_*` fixture が既に保証し、本投影は `recipe_invariant` flag として表示。 |
| RCP-UX-04 | Pass (headless contract) / Deferred (実 surface) | `render_request_report_is_the_single_lowering_and_fails_closed`（Demand event 無 grant→`GrantRequired`、publish 無 grant→`GrantRequired`、GUI/CLI 同一 triple で action/grant 一致・entry のみ差 = 同一 job-key material）、`request_preview_gates_request_until_all_demands_resolve`（request=`SelectionBlocked`、publish=`GrantRequired`）、`request_spec_gates_buttons_to_executable_candidates_only`（executable のみ button）。実 CLI/Workflow/Luau/Rhai/Fake AI surface は sv#273 lease で Deferred（Case ID `RCP-UX-04-CLI`）。 |
| RCP-UX-05 | Pass (headless) | `job_candidates_gate_cancel_retry_attach_by_state`（Rendering→cancel、Failed(retryable)→retry、Failed(AuthDenied)→verify のみ、cancel_requested→cancel なし = `RenderJobKey` 冪等 gate）、`job_projection_carries_receipts_key_summary_and_entry`（receipt tail は render-job 専用 stream・entry provenance 表示）、`job_control_command_is_idempotent_on_the_key`（同一 key+op→同一 command）。Project Undo/Live/Recording Writer への副作用は orchestrator 側契約で、本投影は read-only + typed op のみ。 |
| RCP-UX-06 | NotRun | 実 GPUI/実 OS Gate — 専用 real GUI 自動レーンが現行にない（Case ID `RCP-UX-06-GPUI`）。headless/Fake PASS を実機 PASS に昇格しない。 |

## NotRun / Deferred 一覧

- 実 GPUI/実 OS/IME/DPI/2Window/pixel/RT lane（RCP-UX-06）:
  NotRun。headless/Fake Driver の PASS は semantic/headless
  証拠であり、実機 Gate の代替ではない。
- 実 CLI/Workflow/Luau/Rhai/Fake AI の実 surface:
  Deferred（sv#273 他 lease）。本スライスは共有 action id
  語彙・`TriggerReport` lowering・reason code の contract
  test まで（Case ID `RCP-UX-04-CLI`）。
- 実 Plugin・実 VCS provider・PCM Golden・AudioLens Consumer:
  NotRun（別 Gate、Case ID `RCP-UX-01-RT`）。
- cross-gate RCP-07: Deferred。
- `doc-driven-dev-graph` route: 既存 unrelated broken relations
  で `blocked`（`artifact-graph`/`focus-required`）— RCP-03/
  AUTX-07 と同じ標準 doc chain で代替。relation 修復は範囲外。
