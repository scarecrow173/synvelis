# AUTX-07 GUI views — Case 台帳 (Issue #266 headless slice)

作成: 2026-11-16 / branch: `scarecrow173/sv266-autx7`
対象: `crates/synvelis-workspace/src/automation_ui.rs`・
`crates/synvelis-app/src/automation.rs`・
`crates/synvelis-theme/src/status.rs` と対応 test

語彙は POLICIES.md §2/§3 に従う。**headless Pass は実機 Pass ではない**。
実機未検証項目は Case ID 付きで NotRun/Deferred として記録する。
本スライスは `crates/synvelis-gateway`・`crates/synvelis-automation`・
`assets/themes/` を変更しない（別 Worker リース / 範囲外）。

検証環境: Windows 11 (x86_64)、rustc は `mise` 管理 toolchain、
`cargo --locked` による補助証跡。canonical 検証は `mise run` レーン
（PR 本文に記録）。

## 1. 受入条件 → Case → test → 結果

| Issue #266 受入行 | Case | 検証内容 | test / 証跡 | Expected | Observed | Result |
| --- | --- | --- | --- | --- | --- | --- |
| Action Palette が Capability Version/Availability/Schema/Effect/Explicit Target/Grant state を同期表示し、未対応/認証不足/Unknown を実行可能に見せない | SV266-AC-01 | catalog projection の fail-closed 行列 + spec の button 非生成 | `synvelis-workspace/tests/autx07_automation_ui.rs`: `catalog_projects_every_fixture_row_with_display_fields`・`fail_closed_matrix_never_renders_blocked_rows_executable`・`blocked_rows_stay_visible_in_catalog`・`catalog_query_filters_by_id_and_domain_case_insensitively`。`synvelis-app/tests/autx07_automation.rs`: decode/select 系 | Disabled/Planned/Retired・grant Missing/Unknown は `executable=false` で表示残存、executable 行のみ Preview/Submit button | 29 tests pass（workspace）+ 7 tests pass（app）。blocked 行は理由文字列付きで catalog に残り、spec は executable 行にのみ action button を生成 | **Pass（headless）** |
| Explicit Selection → Stable Target Capture（確定後の Focus/Selection 変更で Target をすり替えない） | SV266-AC-02 | `TargetContract` shape/kind 厳密照合 + `CapturedTarget` の capability id+version+project 固定 | `capture_target_accepts_matching_resource_contract`・`capture_target_rejects_missing_wrong_kind_and_wrong_shape`・`capture_target_none_contract_rejects_any_explicit_target`・`capture_target_create_contract_requires_matching_kinds`・`submit_rejects_unknown_capability_missing_target_and_unavailable` | mismatch/missing/None 契約への explicit target は `TargetMismatch`、capture は凍結値を保持 | 全 case pass。request lowering も同一 `capture_target` を通る | **Pass（headless）** |
| Macro Preview/Apply が 1 Project の Atomic request 列を生成し、Project/Grant/Job の第二正本を GUI に作らない | SV266-AC-03 | preset→`WireRequest` 列の lowering（順序・同一 project・caller consistency・idempotency suffix・step 毎 fail-closed） | `macro_apply_lowers_ordered_steps_with_suffixed_idempotency`・`macro_apply_rejects_unknown_preset_and_non_executable_step`・`macro_preset_projection_carries_version_inputs_effects_undo`・`view_local_actions_mint_no_requests` | step 順・`<key>/<index>` suffix・Availability 非 Available step は error・view-local action は空 Vec | 全 case pass。request は authority を持たず Gateway bind/invoke が再検査する契約 | **Pass（headless）**。live adapter 接続は SV266-NR-04 Deferred |
| Workflow Editor/Monitor が Validation/Approval/CancelRequested/Confirmed/Failed/Indeterminate を正確に表示し、Job Pending を Completed としない | SV266-AC-04 | `project_job_status` の phase×cancel-link 全組合せ + run row の authority field 保持 | `pending_is_never_projected_as_completed`・`completed_requires_provider_confirmation`・`cancel_requested_confirmed_unconfirmed_stay_distinct`・`run_monitor_rows_preserve_authority_fields_and_cancel_axis`・`workflow_plan_projection_preserves_validation_and_approval_display` | Pending→Pending、Unconfirmed→Indeterminate、Requested/Confirmed/Unconfirmed cancel は別 status、approval は表示のみ | 全 case pass。Workflow Editor spec は button を生成しない（approval mint なし） | **Pass（headless）** |
| ViewAction→Capability Binding が GPUI/OS handle を公開型に漏らさず、共通 Component/Runner を重複実装しない | SV266-AC-05 | `synvelis-app::automation` が `ruviform_ui::view` + `synvelis_workspace` のみを使用、mount helper が `ViewHost`/`AttachEpochAllocator` の既存契約に従う | `synvelis-app/tests/autx07_automation.rs`: `decode_*`・`empty_states_render_empty_labels_without_panicking`・`mount_helpers_produce_valid_view_hosts`。`cargo check` で `synvelis-app` の依存木に `synvelis-gateway`/`gpui` が無いことを確認 | GPUI/OS/gateway 型が公開 signature に出ない、ViewHost mount が valid | 7 tests pass、clippy warning 0。新規 component/runner 実装なし（`Button`/`Table`/`Label`/`Panel` のみ使用） | **Pass（headless）** |
| ViewState/Selection が Project 正本を変更しない・2Window で view-local state が独立 | SV266-AC-06 | `AutomationViewState` が view-local field のみ保持 + `ViewState` tolerant deserialize | `automation_view_state_roundtrips_through_serde`・`stale_epoch_helpers_reject_older_catalogs_and_monitors`・`view_state_tolerates_malformed_automation_payload`・`new_view_types_are_registered` | automation state は serde roundtrip 可能・malformed→None・epoch stale 判定・project/grant/job field を持たない | 全 case pass。`ViewType` 4 variant は既存 `ViewRegistry` API で spawn/adopt 可能 | **Pass（headless）**。実 2Window WindowClose/Detach は SV266-NR-01 NotRun |
| Action/Workflow Editor と Script Editor #274 が同じ Capability/Schema/Availability/Target を示す | SV266-AC-07 | 契約レベル: 両 surface が `synvelis_gateway::CapabilityDescriptor` を唯一の source として参照（`project_action_row`/`resolve_capability` が descriptor を直接消費） | `catalog_projects_every_fixture_row_with_display_fields`（descriptor→row の全 field 写像）+ 本台帳 §3 の Deferred 記録 | Editor 同一 source の契約保証 + #274 実照合は Deferred | 契約レベルは Pass。#274 Editor 実装は別 Issue のため照合自体は **NotRun** | **Pass（契約）/ NotRun（#274 照合）** |
| GUI受入: 実GPUI Windows/macOS/Linux で Pointer/Keyboard/IME/Focus/DPI/2Window/WindowClose・Alert/Undo/Save/Reload | SV266-NR-01 | 実 OS lane — 本スライス範囲外 | — | — | — | **NotRun**（#114/#267 別 Gate） |

## 2. Case 詳細（headless 実施分）

| Case | crate::test | 状態 | 備考 |
| --- | --- | --- | --- |
| SV266-T-01 | `synvelis-workspace::autx07_automation_ui` 全 29 test | Pass | projection・capture・request lowering・job status・serde |
| SV266-T-02 | `synvelis-app::autx07_automation` 全 7 test | Pass | decode matrix・empty spec・mount smoke |
| SV266-T-03 | `synvelis-theme` status module 3 test + 既存 contract 14 test | Pass | `StatusTone` token path vocabulary・label 写像 fail-closed |
| SV266-T-04 | `cargo clippy --locked --all-targets`（3 crate） | Pass | warning 0（pedantic 含む） |
| SV266-T-05 | `cargo fmt --all --check` | Pass | fmt 適用済み |

## 3. NotRun / Deferred 対応表

| Case | 内容 | 状態 | Required/Deferred | 後続 gate / 必要ファイル |
| --- | --- | --- | --- | --- |
| SV266-NR-01 | 実 GPUI Windows/macOS/Linux Pointer/Keyboard/IME/Focus/DPI/2Window/WindowClose・Alert/Undo/Save/Reload・Window 断継続 Job | **NotRun** | Required（GUI-L2）だが本スライスは headless-first | synvelis#114/#267。`crates/synvelis-app` native feature・`ui/` driver 統合時に Case ID を引き継ぐ |
| SV266-NR-02 | Script Editor (#274) との同一 Capability/Schema/Availability/Target source 実照合・Analyzer PASS ≠ Gateway 認可 PASS の UI 文言検証 | **NotRun** | Deferred（#274 Owner） | synvelis#274。`crates/synvelis-app` Script Editor surface 実装後に本 slice の `ActionRow`/`resolve_capability` を共有 source として照合 |
| SV266-NR-03 | `assets/themes/synvelis-{dark,light}.dtcg.json` への `color.status-*` token 値追加 | **NotRun** | Deferred（編集範囲外 — `assets/` は repo root、`crates/synvelis-theme` src+tests のみ占有） | `synvelis_theme::status::StatusTone::token_path` の 11 path に対応する `$value` を dark/light product document に追加する slice。`crates/synvelis-theme/tests` の `shipped_assets_*` test が値の存在を検証する拡張が必要 |
| SV266-NR-04 | `synvelis-automation` crate（#264）統合後の live adapter 接続 — `WorkflowPlanView`/`RunMonitorView`/`MacroBrowserView` を実 authority から投影 | **NotRun** | Deferred（#264 別 Worker リース、main 未統合） | `docs/automation/design-workflow-run.md` の Plan/Run 型が main に入った後、`project_workflow_plan`/`project_run_monitor` の呼出側を live adapter に接続 |
| SV266-NR-05 | composition root（`SynvelisApp`/`NativeShell`）への 4 surface mount 配線と shell action dispatch | **NotRun** | Deferred（本 slice は ViewSpec/projection 層まで。mount helper は提供済み） | `crates/synvelis-app/src/{app,service,shell,composition}.rs` — いずれも本タスクの編集範囲外。`mount_*_view` + `decode_automation_command` + `ViewState.automation` を呼ぶ配線 slice |
| SV266-NR-06 | `gpui-test` feature の GPUI Semantic scenario（`scenario_gpui.rs` 系）への 4 surface 追加 | **NotRun** | Deferred（任意 lane — spec builder/mount は headless test で検証済み） | `mise run check-app-gpui`/`lint-app-gpui`/`test-app-gpui` が必要になる変更を含む後続 slice |

## 4. 補足

- `synvelis-app` は `synvelis-gateway` に依存しないため、app 層 test は
  decode/spec/mount のみを対象とし、projection/request lowering の
  oracle は `synvelis-workspace` 側 test が `synvelis_gateway::fixture`
  を使って担保する（依存方向の維持）。
- Macro Preview は「project mutation を伴わない表示」であり、現行
  `automation_requests` では view-local（request を mint しない）に
  分類される。Preview の dry-run request が必要になった場合は
  `AutomationViewAction` への variant 追加として扱う。
