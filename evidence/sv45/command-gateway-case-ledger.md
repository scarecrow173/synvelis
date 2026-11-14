# Capability / Command Gateway — GW Case Ledger (synvelis#45)

Issue #45 の GT-001〜012・JM-01〜05・MCP-01〜06・UX-01〜06・
SEC-01〜14・DATA-01〜10・REC-01〜12・JOB-01〜10・SYS-01〜10・
AC-01〜10・E2E-1/2/3・G0〜G6・GTV-001〜008 の受入ケースに
ついて、fixture・oracle・Owner・mode/layer・status・証拠を
台帳化する。**本 slice は docs/ 設計契約 + evidence +
headless 回帰実測のみ**であり、実装・GUI・実 device/plugin/
OS/Provider/第三者 MCP Host 受入は対象外。

Fixture 表記は `path::test_fn`（Rust integration test の関数名）
または `path`（module/file 単位）。gw10 evidence の verdict
正本は `target/gw10-evidence/*.jsonl` を `gw10_report --
--strict` で集約した結果とする。

## Status 語彙

| 軸 | 値 | 意味 |
|---|---|---|
| 要求状態 | `Required` | 現行必須（本 slice の case は全て Required または条件付き Required） |
| 要求状態 | `Deferred` | 利用条件成立まで保留（実機/GUI/Provider lane） |
| 要求状態 | `NotApplicable` | 対象機能では不要 |
| 実行結果 | `Pass` | 実行した検証が成功（mode/layer を明記・headless/mock/PoC real を実環境と混同しない） |
| 実行結果 | `Fail` | 検証が失敗 |
| 実行結果 | `NotRun` | 未実装または未検証 |
| 実行結果 | `Unsupported` | harness/環境が技術的に非対応（根拠を伴う） |

headless/mock/PoC real transport の Pass を実機/GUI/Provider/
第三者 MCP Host 受入に繰り上げない。設計レビュー Pass と
実測 Pass を区別する。

## GW-R → Case Family 写像（spec 要件 → 受入ケース）

| GW-R | spec 要件 | 主な case family |
|---|---|---|
| GW-R-01 | Capability Registry & Typed Invocation | GT-001〜003・MCP-04・SYS-08 |
| GW-R-02 | Authorization | SEC-01〜14・GT-003・JM-02 |
| GW-R-03 | Context Projection & Disclosure | GT-004・DATA-01〜10 |
| GW-R-04 | Semantic Preparation & Effects | GT-005/006・AC-09 |
| GW-R-05 | Prepared ChangeSet & Approval | GT-007/008・SEC-09〜11 |
| GW-R-06 | Separated State Machines | GT-007・SYS-05/06・UX-05 |
| GW-R-07 | Typed Results & GW-06 Boundary | GT-010・GT-011・SYS-01 |
| GW-R-08 | Authorized Commit Gate & Recovery | GT-009・REC-01〜12・AC-01〜08 |
| GW-R-09 | Job/Media/Provenance Adapter | JM-01〜05・JOB-01〜10・E2E-3 |
| GW-R-10 | MCP Adapter & Trusted Binding | MCP-01〜06・E2E-2-*・SEC-02 |
| GW-R-11 | Permission/Approval UX Contract | UX-01〜06・SYS-02-ux/03-ux（NotRun） |
| GW-R-12 | Verification Harness & Evidence | GTV-001〜008・G0〜G6 |
| GW-R-13 | Boundary Non-duplication | SYS-01・GT-011（第二正本なし）・CONSUMER-* |
| GW-R-14 | RT Boundary & Consumer Regression | SYS-07-rt（Unsupported）・CONSUMER-*（NotRun） |
| GW-R-15 | Evidence Honesty | 本 ledger・`command-gateway-verification.md` の mode/layer/status 分離 |

## mode / layer 凡例

- `mode`: `mock`（in-process・controlled doubles）/ `real`
  （real store・real transport・real process）。
- `layer`: L1 Unit/Contract・L2 Component Integration・L3
  In-process E2E・L4 Process/Fault E2E・L5 System Regression。
- `required`: GW-10 MATRIX の required flag。`required=false`
  row は gate を保持しないが正直に報告される。
- `real_only`: `mode=real` の record のみが verdict を満たす
  row（mock Pass で代替不可）。

## Fixture 凡例（実在 file）

- `crates/synvelis-gateway/tests/`: `contract.rs`・
  `authorization.rs`・`context.rs`・`prepare.rs`・
  `prepare_regressions.rs`・`proposal.rs`・`proposal_
  regressions.rs`・`commit_gate.rs`・`job.rs`・`mcp_adapter.rs`・
  `schema_compat.rs`・`ctl_01.rs`・`fixtures.rs`・`gw10_contract.rs`・
  `gw10_authorization.rs`・`gw10_concurrency.rs`・
  `gw10_persistence.rs`・`gw10_project.rs`・`gw10_mcp.rs`・
  `gw10_job.rs`・`gw10_system.rs`・`gw10_harness.rs`・`rig/`・
  `common/`
- `crates/synvelis-gateway/src/`: `evidence.rs`（MATRIX/
  EvidenceRecord）・`oracle.rs`・`schedule.rs`・`trace.rs`・
  `fixture.rs`・`bin/gw10_report.rs`
- `crates/mcp-poc/tests/`: `gw10_transport.rs`・`stdio_e2e.rs`
- `crates/synvelis-workspace/src/gateway_ui.rs`（GW-09 headless）
- `crates/synvelis-project/src/controller.rs`（ProjectController）

## GT-001〜012（Contract 受入 — GTC-002/007/008）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| GT-001 | 明示 Typed Stable ID/範囲で対象指定・UI Selection 非依存 | `contract.rs::gt001_explicit_typed_target_reaches_executor`・`gt001_implicit_targeting_fields_rejected`・`gt001_missing_target_rejected`・`gw10_contract.rs::typed_ids_enforce_domain_and_kind_on_the_wire`・oracle | #53・GW10-A/G0 | mock/L1-L2 | **Pass** |
| GT-002 | 外部入力から Trusted Authority を自己申告不可 | `contract.rs::gt002_authority_fields_rejected_on_wire`・`gt002_caller_comes_from_adapter_not_wire`・`gt002_unknown_wire_fields_rejected`・`gw10_authorization.rs::sec02_wire_authority_fields_cannot_self_declare`・`mcp_adapter.rs::wire_authority_flows_from_the_permit_record`・`connect_binds_record_identity_not_client_claims` | #53/#59・GW10-A・B/G0-G1 | mock/L1-L2 | **Pass** |
| GT-003 | Capability 存在と利用権限を分離 | `contract.rs::gt003_discovery_returns_metadata_without_granting`・`gt003_new_capability_does_not_widen_existing_grants`・`gt003_permit_is_stage_scoped`・`gw10_authorization.rs::data09_tool_discovery_does_not_extend_the_grant` | #53/#54・GW10-A・B/G0-G1 | mock/L1-L2 | **Pass** |
| GT-004 | 内部 Context Read と外部 Disclosure を分離 | `gw10_authorization.rs::gt004_context_query_projects_pinned_snapshot`・`context.rs::metadata_only_grant_reads_metadata_never_content`・`media_group_is_descriptor_never_bytes`・`derived_group_reports_unavailable_honestly`・`external_envelope_strips_diagnostics` | #55・GW10-B/G1 | mock/L2 | **Pass** |
| GT-005 | Proposal 作成権限と Apply 権限を分離 | `gw10_project.rs::gt005_prepare_leaves_the_pinned_base_untouched`・`proposal.rs::apply_denies_without_the_capability_in_grant`・`suggest_mode_never_applies`・`approve_to_apply_flows_through_request_grant_consume` | #56/#57・GW10-C/G2 | mock/L2-L3 | **Pass** |
| GT-006 | 実際の Effect Set を Synvelis 側が導出 | `gw10_project.rs::gt006_effects_derive_from_real_before_after_attributes`・`prepare.rs::prepare_derives_effects_and_mutates_nothing`・`derived_effects_flow_through_grant_scope`・`unverifiable_effects_fail_closed_for_every_caller` | #56・GW10-C/G2 | mock/L2 | **Pass** |
| GT-007 | ChangeSet は不変・Digest は意味的変更+Revision を識別 | `gw10_project.rs::gt007_proposal_states_do_not_conflate`・`proposal.rs::identical_prepares_digest_identically`・`semantic_drift_changes_the_digest`・`changeset_verify_detects_tampering`・`changeset_exposes_its_schema_identity` | #57・GW10-C/G2 | mock/L2 | **Pass** |
| GT-008 | Approval は Principal/Project/ChangeSet/Revision に束縛 | `gw10_project.rs::gt008_accepted_apply_carries_an_operation_identity`・`proposal.rs::receipt_binding_rejects_wrong_grant`・`receipt_binding_rejects_wrong_task`・`receipt_for_another_proposal_denies`・`lookup_requires_proposal_and_digest` | #57・GW10-C/G2 | mock/L2 | **Pass** |
| GT-009 | Grant 失効後は未使用 Approval で Commit 不可 | `gw10_authorization.rs::sec12_revoked_grant_kills_the_unused_approval`・`gw10_concurrency.rs::rec01_revoke_first_denies_commit`・`commit_gate.rs::revoke_confirmed_before_commit_denies`・`expired_proposal_rejects_at_commit` | #58・GW10-D/G2-G3 | mock/L2 | **Pass** |
| GT-010 | GW-01〜05 で Semantic Project Revision を変更しない | `gw10_project.rs::gt010_prepare_approve_preview_leave_revision_untouched`・`contract.rs::gt010_bind_does_not_execute`・`proposal.rs::preview_*` | #53〜#57・GW10-D/G3 | mock/L2 | **Pass** |
| GT-011 | Project Mutation 確定は GW-06→ProjectController Single Writer のみ | `gw10_system.rs`(SYS-01)・`gw10_project.rs::gw10c_commit_advances_revision_and_undo_rewinds`・`commit_gate.rs::concurrent_commits_single_writer_conflict`・`manual_commit_is_human_only_and_journaled`・`synvelis-project/src/controller.rs` | #58・GW10-H/G6 | mock/L3 | **Pass** |
| GT-012 | MCP/AI Disabled でも手動 UI 編集・通常 DAW 機能が成立 | `gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers`(E2E-1)・`sys08_unsupported_versions_refuse_typed` | #62・GW10-H/G6 | mock/L3 | **Pass**（headless）・実 GUI NotRun（#114） |

## JM-01〜05（Job/Media/Provenance — GIA-001〜003）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| JM-01 | Disconnect/Window Close 非依存 Job | `gw10_job.rs::job02_disconnect_leaves_the_job_book_intact`・`job.rs::ids_stay_distinct_and_jobs_survive_disconnect`・`pending_cancel_upgrades_to_confirmed_on_poll` | #21 接続・GW10-G/G5 | mock/L2-L3 | **Pass**（headless）・実 Window Close NotRun（#114） |
| JM-02 | Grant 失効後の追加保護操作拒否 | `gw10_job.rs::job03_revocation_denies_calls_and_requests_cancel`・`job.rs::revocation_denies_new_calls_but_only_requests_cancel`・`expired_grant_admits_no_new_job`・`gw10_authorization.rs::data04_revoked_grant_discloses_nothing_new` | #54/#62・GW10-B/G・G1/G5 | mock/L2 | **Pass** |
| JM-03 | Job Completed と Media Verified 分離 | `gw10_job.rs::job05_unverified_output_never_registers`・`gtv002_observed_states_stay_distinct_across_the_lifecycle`(gw10_system)・`sys05_sys06_state_nonconflation_is_observable` | #21 接続・GW10-E/H/GTV/G5-G6 | mock/L2-L3 | **Pass** |
| JM-04 | Verified Media≠Project Commit | `gw10_job.rs::job10_verified_output_then_drifted_base_conflicts`・`gtv004_e2e3_verified_media_commits_with_provenance`・`job.rs::verified_media_commits_through_gate`・`verified_media_lowers_to_register_source_with_gate_digest`・`gw10_persistence.rs::rec11_crash_after_apply_leaves_verified_source_and_no_clip` | #21 接続・GW10-G/G5 | mock/L2-L3 | **Pass** |
| JM-05 | Provenance の Model/Rights Unknown | `gw10_job.rs::job07_absent_claims_stay_unknown_in_provenance`・`job.rs::provenance_levels_and_secret_bound` | #21 接続・GW10-G/G5 | mock/L2 | **Pass**（headless）・実 Provider NotRun |

## MCP-01〜06（MCP Adapter — GIA-004）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| MCP-01 | 信頼できる Principal Binding | `mcp-poc/tests/gw10_transport.rs::gw10f_session_binds_permit_record_never_client_claims`・`gw10_mcp.rs::gw10f_session_binds_permit_record_never_client_claims`・`mcp_adapter.rs::connect_binds_record_identity_not_client_claims`・`issue_permit_on_control_mints_a_working_credential`・`connect_rejects_invalid_secrets_with_one_blind_verdict`・`tampered_secret_hash_fails_closed` | #59・GW10-F/G4 | real(PoC)/L4 | **Pass**（PoC transport）・第三者 Host NotRun |
| MCP-02 | Tool/Invocation 権限分離 | `mcp_adapter.rs::rejects_capability_outside_the_tool_allowlist`・`catalog_is_channel_scoped`・`discovery_requires_a_bound_session`・`control_methods_are_blind_on_the_client_channel`・`gw10_authorization.rs::data09_tool_discovery_does_not_extend_the_grant` | #59・GW10-B/F/G1/G4 | mock+real/L2-L4 | **Pass** |
| MCP-03 | 3 Mode E2E | `mcp-poc/tests/gw10_transport.rs::gw10f_three_modes_over_real_transports`・`gw10f_permit_cli_lifecycle_real`・`gw10f_delegated_apply_commits_and_survives_restart_real`・`gw10_mcp.rs::gw10f_suggest_mode_denies_apply`・`gw10f_delegated_apply_commits_through_the_host`・`mcp_adapter.rs::suggest_mode_prepares_but_denies_apply`・`delegated_apply_commits_through_the_seam`・`approve_to_apply_requires_and_consumes_the_receipt` | #59・GW10-F/G4 | real(PoC)/L4 | **Pass**（PoC transport）・第三者 Host NotRun |
| MCP-04 | 異種 Stable ID/未知 Schema 拒否 | `gw10_contract.rs::typed_ids_enforce_domain_and_kind_on_the_wire`・`unknown_capability_and_version_drift_are_typed_rejections`・`schema_drift_and_malformed_wire_are_classified_errors`・`mcp_adapter.rs::rejects_wrong_kinds_and_malformed_typed_ids`・`rejects_unknown_missing_and_malformed_fields`・`unsupported_protocol_version_is_rejected`・`contract.rs::unknown_*`/`unsupported_*`/`malformed_tagged_id_rejected` | #53/#59・GW10-A/F/G0/G4 | mock+real/L1-L4 | **Pass** |
| MCP-05 | Disconnect/Retry で二重 Commit 防止 | `mcp-poc/tests/gw10_transport.rs::gw10f_delegated_apply_commits_and_survives_restart_real`・`gw10_mcp.rs`(E2E-2-restart)・`mcp_adapter.rs::settle_op_reports_committed_after_restart_scoped_reconcile`・`commit_failure_reports_the_reconciled_verdict_not_success`・`gw10_concurrency.rs::rec05_rec06_keyed_retry_replays_and_conflicts`・`commit_gate.rs::keyed_commit_retry_replays_the_receipt`・`wire_level_keyed_conflict_rejects_different_payload` | #58/#59・GW10-D/F/G3-G4 | real(PoC)+mock/L2-L4 | **Pass** |
| MCP-06 | 大容量 Media Read/Disclosure 制約 | `gw10_authorization.rs::data08_bounded_budgets_reject_oversized_requests`(real/L4)・`data10_external_transfer_needs_its_class`・`mcp_adapter.rs::rejects_oversized_ranges_pages_and_payloads`・`job.rs::result_disclosure_requires_the_disclosure_class`・`context.rs::page_*`/`range_*`/`total_items_are_bounded`/`selector_id_count_is_bounded` | #55/#59・GW10-B/F/G1/G4 | mock+real/L2-L4 | **Pass**（bounded 契約）・実大容量 media NotRun |

## UX-01〜06（Permission/Approval UX — GIA-005・#114）

| Case | 要求 | Fixture/Oracle | Owner/Gate | mode/layer | Status |
|---|---|---|---|---|---|
| UX-01 | AI Runtime vs External MCP 分離 | `synvelis-workspace/src/gateway_ui.rs`（headless 契約）・`gw10_system.rs::sys02_revoking_one_grant_never_touches_another`・`sys08_unsupported_versions_refuse_typed` | #114・GW10-H/G6 | mock/L2 | **Pass(部分)**（headless）・実 UI NotRun |
| UX-02 | 固定/動的 Grant Scope 表示 | `gateway_ui.rs`・`gw10_authorization.rs::sec06_dynamic_scope_escape_denies`・`sec07_selection_changes_do_not_retarget_the_grant`・`context.rs::dynamic_scope_filters_by_kind` | #114・GW10-B/H/G1/G6 | mock/L2 | **Pass(部分)**（headless）・実 UI NotRun |
| UX-03 | 特定 ChangeSet/Revision/Effects の承認表示 | `gateway_ui.rs`・`gw10_system.rs::sys03_approval_binds_proposal_not_ambient_selection`・`proposal.rs::preview_reports_*`・`lookup_requires_proposal_and_digest` | #114・GW10-C/H/G2/G6 | mock/L2-L3 | **Pass(部分)**（headless）・実 UI NotRun |
| UX-04 | Grant Revoke と Job Cancel 区別 | `gw10_system.rs`・`gw10_job.rs::job03_revocation_denies_calls_and_requests_cancel`・`job04_unreachable_provider_reports_unconfirmed_never_stopped`・`job09_cancel_verdicts_reflect_actual_ordering`・`cancel_verdicts_are_honest` | #62・GW10-G/H/G5-G6 | mock/L2 | **Pass**（状態区別）・実 UI NotRun |
| UX-05 | Proposal/Committed/Applied/Saved 表示 | `gw10_system.rs::sys05_sys06_state_nonconflation_is_observable`・`gtv002_observed_states_stay_distinct_across_the_lifecycle`・`gateway_ui.rs` | #114・GW10-E/H/GTV/G5-G6 | mock/L2-L3 | **Pass(部分)**（headless）・実 UI NotRun |
| UX-06 | AI 無効時の通常 DAW 維持 | `gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers`(E2E-1) | #62・GW10-H/G6 | mock/L3 | **Pass**（headless）・実 GUI NotRun |

## SEC-01〜14（Security — GW10-B/G1）

| Case | 要求 | Fixture | mode/layer | Status |
|---|---|---|---|---|
| SEC-01 | 未認証 request deny | `gw10_authorization.rs::sec01_unauthenticated_request_denies` | mock/L2 | **Pass** |
| SEC-02 | wire authority 自己申告拒否 | `sec02_wire_authority_fields_cannot_self_declare`・`contract.rs::gt002_*` | mock/L1-L2 | **Pass** |
| SEC-03 | 他 Principal の Grant 使用拒否 | `sec03_foreign_principal_cannot_spend_the_grant` | mock/L2 | **Pass** |
| SEC-04 | 他 Project 横断拒否（存在秘匿） | `sec04_foreign_project_boundary_denies`・`context.rs::foreign_project_snapshot_is_existence_blind`・`foreign_caller_cursor_is_indistinguishable_from_unknown` | mock/L2 | **Pass** |
| SEC-05 | 共有 Source 書込は Source scope 要求 | `sec05_shared_source_write_needs_source_scope`・`prepare.rs::linked_source_write_needs_source_scope_not_just_clip`・`context.rs::clip_scope_never_implies_source_scope`・`source_scope_never_implies_clip_scope` | mock/L2 | **Pass** |
| SEC-06 | Dynamic Scope 脱出拒否 | `sec06_dynamic_scope_escape_denies` | mock/L2 | **Pass** |
| SEC-07 | Selection 変更で Grant 対象 retarget しない | `sec07_selection_changes_do_not_retarget_the_grant`・`sys03_approval_binds_proposal_not_ambient_selection`(gw10_system) | mock/L2 | **Pass** |
| SEC-08 | Suggest mode Apply 拒否・Revision 不変 | `sec08_suggest_mode_apply_denies_without_revision_change`(L3) | mock/L3 | **Pass** |
| SEC-09 | ChangeSet tamper を Digest check で拒否 | `sec09_changeset_tamper_denies_at_digest_check`・`proposal.rs::changeset_verify_detects_tampering` | mock/L2 | **Pass** |
| SEC-10 | 他人 Approval Receipt 拒否 | `sec10_foreign_approval_receipt_denies`・`proposal.rs::receipt_for_another_proposal_denies`・`foreign_and_unknown_operations_deny_identically`(job) | mock/L2 | **Pass** |
| SEC-11 | Approval 後 Revision drift は Conflict | `sec11_revision_drift_after_approval_conflicts`・`proposal.rs::revision_conflict_rejects_apply`・`expected_revision_mismatch_is_a_conflict` | mock/L2 | **Pass** |
| SEC-12 | Grant revoke→未使用 Approval Commit 拒否 | `sec12_revoked_grant_kills_the_unused_approval`・`commit_gate.rs::revoke_confirmed_before_commit_denies` | mock/L2 | **Pass** |
| SEC-13 | Grant 外 Capability 拒否 | `sec13_capability_outside_the_grant_denies`・`mcp_adapter.rs::rejects_capability_outside_the_tool_allowlist`・`prepare.rs::inner_capability_outside_grant_denies`・`batch_capability_itself_must_be_granted` | mock/L2 | **Pass** |
| SEC-14 | 未知 Effect は自動許可しない | `sec14_unknown_effect_is_never_auto_permitted`・`prepare.rs::unverifiable_effects_fail_closed_for_every_caller` | mock/L2 | **Pass** |

## DATA-01〜10（Data Disclosure — GW10-B/G1）

| Case | 要求 | Fixture | mode/layer | Status |
|---|---|---|---|---|
| DATA-01 | metadata Grant は structured content を読めない | `gw10_authorization.rs::data01_metadata_grant_cannot_read_structured_content`・`context.rs::metadata_only_grant_reads_metadata_never_content` | mock/L2 | **Pass** |
| DATA-02 | derived feature disclosure は class 要求 | `data02_derived_feature_disclosure_needs_its_class`・`context.rs::derived_group_reports_unavailable_honestly` | mock/L2 | **Pass** |
| DATA-03 | pagination は revision を混在させない | `data03_pagination_never_mixes_revisions`・`context.rs::pagination_chain_is_revision_stable_across_project_change`・`evicted_snapshot_reports_expired_never_rebased` | mock/L2 | **Pass** |
| DATA-04 | revoked Grant は新規 disclosure しない | `data04_revoked_grant_discloses_nothing_new`・`context.rs::revoked_grant_denies_the_next_page`・`expired_grant_denies_the_next_page` | mock/L2 | **Pass** |
| DATA-05 | foreign Operation ID は結果を取得不可 | `data05_foreign_operation_id_cannot_fetch_results`・`job.rs::foreign_and_unknown_operations_deny_identically` | mock/L2 | **Pass** |
| DATA-06 | metadata 内 prompt injection は権限を変えない | `data06_prompt_injection_in_metadata_changes_no_authority`・`gw10_job.rs::gw10g_prompt_input_never_discloses_into_results` | mock/L2 | **Pass** |
| DATA-07 | arbitrary URI は resource を漏らさない | `data07_arbitrary_uri_never_leaks_a_resource`・`context.rs::explicit_selector_denies_pin_absent_blind`・`explicit_selector_denies_uncovered_and_absent_identically` | mock/L2 | **Pass** |
| DATA-08 | bounded budget は oversized を拒否 | `data08_bounded_budgets_reject_oversized_requests`(real/L4)・`context.rs::page_count_is_bounded`/`page_limit_is_bounded`/`total_items_are_bounded`/`range_window_is_bounded`・`mcp_adapter.rs::rejects_oversized_ranges_pages_and_payloads` | real+mock/L2-L4 | **Pass** |
| DATA-09 | Tool discovery は Grant を拡張しない | `data09_tool_discovery_does_not_extend_the_grant`・`mcp_adapter.rs::discovery_requires_a_bound_session`・`catalog_is_channel_scoped` | real+mock/L2-L4 | **Pass** |
| DATA-10 | External Transfer は class 要求 | `data10_external_transfer_needs_its_class`・`job.rs::submit_denies_out_of_scope_inputs_and_missing_transfer_class`・`result_disclosure_requires_the_disclosure_class` | mock/L2 | **Pass** |

## REC-01〜12（Revoke/Commit/Retry/Crash/Recovery — GW10-D/E・G3）

| Case | 要求 | Fixture | mode/layer | Status |
|---|---|---|---|---|
| REC-01 | Revoke 先→Commit 拒否 | `gw10_concurrency.rs::rec01_revoke_first_denies_commit`・`gtv005_revoke_before_receipt_consumption_denies`・`commit_gate.rs::revoke_confirmed_before_commit_denies` | mock/L2 | **Pass** |
| REC-02 | Commit 先→Mutation 有効 | `rec02_commit_first_stands`・`commit_confirmed_before_revoke_stands`・`acg003_commit_before_revoke_stands`(authorization) | mock/L2 | **Pass** |
| REC-03 | 2 Commit・同一 base→1 つ conflict | `rec03_two_commits_one_base_one_conflicts`・`commit_gate.rs::concurrent_commits_single_writer_conflict` | mock/L2 | **Pass** |
| REC-04 | （matrix 上は独立 case — 実装は `commit_gate.rs`/`gw10_concurrency.rs` の CAS/ordering で被覆） | `commit_gate.rs::commit_records_the_policy_position_it_observed`・`saves_publish_head_in_commit_order` | mock/L2 | **Pass** |
| REC-05/06 | keyed retry は receipt replay・Payload 相違は conflict | `rec05_rec06_keyed_retry_replays_and_conflicts`・`commit_gate.rs::keyed_commit_retry_replays_the_receipt`・`wire_level_keyed_conflict_rejects_different_payload`・`contract.rs::idempotency_*` | mock/L2 | **Pass** |
| REC-07 | crash after authorize で spend 整合 | `gw10_persistence.rs::rec07_crash_after_authorize_preserves_spend_consistency`・`commit_gate.rs::crash_after_authorize_leaves_nothing_committed` | mock/L2 | **Pass** |
| REC-08 | confirmed revoke は torn tail を越えて残る | `rec08_confirmed_revoke_survives_torn_tail_does_not`・`commit_gate.rs::torn_revoke_tail_never_confirms_a_revocation`・`durable_revoke_survives_restart`・`file_policy_store_replays_lifecycle`・`durable_policy_admin_never_double_counts_consumption` | mock/L2 | **Pass** |
| REC-09 | crash points で committed または indeterminate・torn なし | `rec09_crash_points_recover_committed_or_indeterminate_never_torn`・`commit_gate.rs::crash_after_apply_reports_indeterminate_and_no_partial_state`・`crash_after_durable_record_recovers_committed`・`restart_recovers_committed_state_and_verdict` | mock/L2 | **Pass** |
| REC-10 | save HEAD を commit 順で publish | `rec10_saves_publish_head_in_commit_order`・`commit_gate.rs::saves_publish_head_in_commit_order` | mock/L2 | **Pass** |
| REC-11 | crash after apply で verified source 残存・clip なし | `rec11_crash_after_apply_leaves_verified_source_and_no_clip` | mock/L2 | **Pass** |
| REC-12 | restart 後 stale proposal は commit しない | `rec12_stale_proposal_never_commits_after_restart`・`commit_gate.rs::stale_proposal_can_never_commit_after_restart`・`expired_proposal_rejects_at_commit`・`gw10_persistence.rs::sys04_expired_proposal_rejects_not_applies` | mock/L2 | **Pass** |

## JOB-01〜10（Job/Media/Provenance — GW10-G/G5）

| Case | 要求 | Fixture | mode/layer | Status |
|---|---|---|---|---|
| JOB-01 | Job Admission・scope・budget | `gw10_job.rs`・`job.rs::submit_denies_out_of_scope_inputs_and_missing_transfer_class`・`unknown_provider_denies_admission`・`job_budget_bounds_admissions`・`job_book_spend_is_atomic_and_refundable`・`job_book_refuses_foreign_saturation`・`batch_media_register_uses_the_gate` | mock/L2 | **Pass** |
| JOB-02 | Disconnect で JobBook 不変 | `job02_disconnect_leaves_the_job_book_intact`・`ids_stay_distinct_and_jobs_survive_disconnect` | mock/L2 | **Pass** |
| JOB-03 | Revoke→新規 call 拒否+Cancel 要求 | `job03_revocation_denies_calls_and_requests_cancel`・`revocation_denies_new_calls_but_only_requests_cancel` | mock/L2 | **Pass** |
| JOB-04 | unreachable provider は unconfirmed・stopped と偽らない | `job04_unreachable_provider_reports_unconfirmed_never_stopped`・`cancel_verdicts_are_honest`・`job09_cancel_verdicts_reflect_actual_ordering` | mock/L2 | **Pass** |
| JOB-05 | unverified output は register しない | `job05_unverified_output_never_registers`・`failed_verification_never_registers`・`register_fails_closed_without_a_gate` | mock/L2 | **Pass** |
| JOB-06 | input snapshot は committed revision を pin | `job.rs::input_snapshot_pins_committed_revision` | mock/L3 | **Pass** |
| JOB-07 | absent claims は provenance で unknown | `job07_absent_claims_stay_unknown_in_provenance`・`provenance_levels_and_secret_bound` | mock/L2 | **Pass** |
| JOB-08 | （real record — provider truth poll） | `job.rs::progress_polls_report_provider_truth` | real/L4 | **Pass** |
| JOB-09 | cancel verdict は実順序を反映 | `job09_cancel_verdicts_reflect_actual_ordering`・`pending_cancel_upgrades_to_confirmed_on_poll`・`cancel_verdicts_are_honest` | mock/L2 | **Pass** |
| JOB-10 | verified output 後 drifted base は conflict | `job10_verified_output_then_drifted_base_conflicts`・`verified_media_commits_through_gate`・`repeated_get_materializes_once` | mock/L2 | **Pass** |

## SYS-01〜10（System Regression — GW10-H/G6）

| Case | 要求 | Fixture | mode/layer | Status |
|---|---|---|---|---|
| E2E-1 | AI-off human pipeline（commit/undo/recover） | `gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers` | mock/L3 | **Pass** |
| SYS-01 | Project Mutation は Single Writer のみ（GT-011） | `gw10_project.rs::gw10c_*`・`commit_gate.rs::*`・`sys01`(gw10_system) | mock/L3 | **Pass** |
| SYS-02 | grant 独立性（1 grant revoke が他を触らない） | `sys02_revoking_one_grant_never_touches_another`・`authorization.rs::ahg007_*` | mock/L2 | **Pass** |
| SYS-03 | approval は ambient selection でなく proposal に束縛 | `sys03_approval_binds_proposal_not_ambient_selection`・`sec07_*` | mock/L2 | **Pass** |
| SYS-04 | expired proposal は reject（apply しない） | `gw10_persistence.rs::sys04_expired_proposal_rejects_not_applies`・`proposal.rs::expired_proposal_is_unavailable`・`expired_proposal_reopens_as_fresh_request` | mock/L2 | **Pass** |
| SYS-05 | state 非混同（Proposal/Committed/Applied/Saved） | `sys05_sys06_state_nonconflation_is_observable`・`gtv002_*` | mock/L2-L3 | **Pass** |
| SYS-06 | （state 非混同の別側面 — SYS-05 と同 fixture 群） | `sys05_sys06_state_nonconflation_is_observable` | mock/L2 | **Pass** |
| SYS-07 | RT boundary は記録・budget 未実測 | `sys07_rt_boundary_is_recorded_not_claimed` | mock/L2 | **Unsupported**（budget 未実測・required=false） |
| SYS-08 | unsupported version は typed reject | `sys08_unsupported_versions_refuse_typed`・`contract.rs::unsupported_*`・`schema_compat.rs::*` | mock/L1 | **Pass** |
| SYS-09 | process restart（real record） | `gw10_persistence.rs`/`commit_gate.rs::restart_*`・`mcp_adapter.rs::settle_op_reports_committed_after_restart_scoped_reconcile`・`file_store_survives_restart_and_never_leaks_secrets` | real/L4 | **Pass** |
| SYS-10 | （matrix 上の残 case — gw10_system/gtv 系で被覆） | `gw10_system.rs`・`gtv004_*` | mock/L2-L3 | **Pass** |
| SYS-02-ux | 実 GUI での grant 独立表示 | （未実装 — #114 GPUI） | — | **NotRun**（required=false） |
| SYS-03-ux | 実 GUI での approval binding 表示 | （未実装 — #114 GPUI） | — | **NotRun**（required=false） |
| SYS-05-powerloss | OS/電源断 durability | （harness 非対応 — `fault::POWER_LOSS` は常に Unsupported） | — | **NotRun/Unsupported**（required=false） |
| SYS-07-rt | RT 性能 budget 実測 | （budget 未実測 — 記録のみ） | — | **Unsupported**（required=false） |
| CONSUMER-RUVIFORM | Ruviform Consumer 回帰 | （本 slice で Ruviform 変更なし・rf#294/#553 条件付き） | — | **NotRun**（required=false・条件未成立） |
| CONSUMER-AUDIOLENS | AudioLens Consumer 回帰 | （同上） | — | **NotRun**（required=false・条件未成立） |

## AC-01〜10（GW-06 受入候補 — ACG-005・G3）

| Case | 要求 | Fixture | mode/layer | Status |
|---|---|---|---|---|
| AC-01 | 有効 Grant による MIDI 変更と Undo | `gw10_concurrency.rs`/`commit_gate.rs`・`proposal.rs::prepared_op_commits_through_controller_and_undoes`(prepare)・`gw10c_commit_advances_revision_and_undo_rewinds` | mock/L2 | **Pass** |
| AC-02 | Revoke-before-Commit 拒否 | `rec01_revoke_first_denies_commit`・`revoke_confirmed_before_commit_denies` | mock/L2 | **Pass** |
| AC-03 | Commit-before-Revoke 維持 | `rec02_commit_first_stands`・`commit_confirmed_before_revoke_stands` | mock/L2 | **Pass** |
| AC-04 | 手動 UI と AI の Revision 競合 | `rec03_two_commits_one_base_one_conflicts`・`gw10c_conflict_after_revision_drift`・`revision_conflict_rejects_apply` | mock/L2 | **Pass** |
| AC-05 | 承認後改竄拒否 | `sec09_changeset_tamper_denies_at_digest_check`・`ac05_commit_stage_evaluation_denies_out_of_scope_writes`・`changeset_verify_detects_tampering` | mock/L2 | **Pass** |
| AC-06 | 同一要求二重 Apply 防止 | `rec05_rec06_keyed_retry_replays_and_conflicts`・`keyed_commit_retry_replays_the_receipt`・`double_consumption_can_never_succeed` | mock/L2 | **Pass** |
| AC-07 | 応答前 Disconnect 後の結果照合 | `job02_disconnect_leaves_the_job_book_intact`・`settle_op_reports_committed_after_restart_scoped_reconcile`・`rec09_*` | mock/L2-L4 | **Pass** |
| AC-08 | Commit/Revoke 途中 Crash+Recovery | `rec07`〜`rec12`・`commit_gate.rs::crash_*`/`torn_*`/`durable_*`/`restart_*` | mock/L2 | **Pass** |
| AC-09 | Linked MIDI Source への Effect 認可 | `sec05_shared_source_write_needs_source_scope`・`prepare.rs::linked_midi_write_names_source_and_linked_clips`・`linked_source_write_needs_source_scope_not_just_clip`・`derived_effects_flow_through_grant_scope` | mock/L2 | **Pass** |
| AC-10 | AI Off で通常 UI 操作継続 | `e2e1_ai_off_human_pipeline_commits_undoes_and_recovers` | mock/L3 | **Pass** |

## E2E-1/2/3（GTV-004 3 経路）

| Case | 要求 | Fixture | mode/layer | Status |
|---|---|---|---|---|
| E2E-1 | Manual: AI/MCP 無効→編集/Undo/再生/Save/Reload | `gw10_system.rs::e2e1_ai_off_human_pipeline_commits_undoes_and_recovers` | mock/L3 | **Pass**（headless）・実 GUI NotRun |
| E2E-2-suggest | Initial MCP MIDI・Suggest mode | `mcp-poc/tests/gw10_transport.rs`(gw10f_*_real)・`gw10_mcp.rs::gw10f_suggest_mode_denies_apply` | real(PoC)/L4 | **Pass**（PoC transport）・第三者 Host NotRun |
| E2E-2-approve | Initial MCP MIDI・Approve-to-Apply | `gw10f_*_real`・`approve_to_apply_requires_and_consumes_the_receipt` | real(PoC)/L4 | **Pass**（PoC transport） |
| E2E-2-delegated | Initial MCP MIDI・Delegated Apply | `gw10f_delegated_apply_commits_and_survives_restart_real`・`delegated_apply_commits_through_the_seam` | real(PoC)/L4 | **Pass**（PoC transport） |
| E2E-2-restart | restart を越えた結果照合・二重 Commit 防止 | `gw10f_delegated_apply_commits_and_survives_restart_real`・`settle_op_reports_committed_after_restart_scoped_reconcile`・`rec05_rec06_*` | real(PoC)/L4 | **Pass**（PoC transport） |
| E2E-3 | Generated Media: Job→Staging/Verification/Provenance→Proposal→Commit | `gw10_job.rs::gtv004_e2e3_verified_media_commits_with_provenance` | mock/L3 | **Pass**（headless）・実 Provider NotRun |

## GTV-001〜008（Harness Criteria — GTV）

| Case | 要求 | Fixture | mode/layer | Status |
|---|---|---|---|---|
| GTV-001 | 5 検証責務と Owner 維持 | `evidence.rs` MATRIX・`gw10_harness.rs::matrix_covers_every_failure_family_and_gate`・`oracle_*` | mock/L1 | **Pass** |
| GTV-002 | 8 状態正本と Completion 分離 | `gw10_system.rs::gtv002_observed_states_stay_distinct_across_the_lifecycle`・`sys05_sys06_*` | mock/L2 | **Pass** |
| GTV-003 | 5 層 Test と独立 Oracle | `gw10_harness.rs::oracle_is_closed_world_until_facts_are_planted`・`oracle_linearization_names_the_first_confirmed_winner`・`oracle_policy_modes_gate_the_apply_stage`・`causal_trace_preserves_observed_order`・`crash_hook_arms_exactly_one_named_stage`・`scheduled_fence_parks_labeled_sections_until_released` | mock/L1 | **Pass** |
| GTV-004 | E2E 3 経路と Failure Matrix | `gw10_system.rs::e2e1_*`・`gw10_mcp.rs`/`gw10_transport.rs`(E2E-2)・`gw10_job.rs::gtv004_*`(E2E-3) | mock+real/L3-L4 | **Pass** |
| GTV-005 | Revoke/Commit 競合の決定論的検証 | `gw10_concurrency.rs::gtv005_both_orderings_enumerated_deterministically`・`gtv005_revoke_before_receipt_consumption_denies`・`scheduled_fence_*`・`crash_hook_*` | mock/L1-L2 | **Pass** |
| GTV-006 | MCP/Prompt Injection/Disclosure 防御 | `gw10_authorization.rs::data06_*`・`gw10_job.rs::gw10g_*`・`mcp_adapter.rs::*` | real+mock/L2-L4 | **Pass** |
| GTV-007 | RT 性能と Consumer 回帰 | `sys07_rt_boundary_is_recorded_not_claimed`（記録のみ）・CONSUMER-*（条件付き NotRun） | mock/L2 | **Pass**(記録)・**Unsupported**(budget)・**NotRun**(consumer) |
| GTV-008 | Evidence と Gate 判定 | `evidence.rs` EvidenceRecord/MATRIX・`gw10_harness.rs::evidence_records_roundtrip_through_jsonl_scan`・`bin/gw10_report.rs` | mock/L1 | **Pass** |

## G0〜G6 Gate（`gw10_report -- --strict` 集約）

| Gate | 内容 | Verdict | 備考 |
|---|---|---|---|
| G0 | Contract（GW10-A） | **PASS** | GT-001〜003・typed id/schema/error |
| G1 | Scope+Disclosure（GW10-B） | **PASS** | SEC-01〜14・DATA-01〜10・GT-004 |
| G2 | Proposal+Effect（GW10-C） | **PASS** | GT-005〜009・changeset/digest/approval |
| G3 | 3 Mode+Commit/Revocation/Recovery（GW10-D/E） | **PASS** | AC-01〜10・REC-01〜12・GT-010 |
| G4 | Real MCP transport E2E（GW10-F） | **PASS** | E2E-2-*（PoC real transport・第三者 Host は対象外） |
| G5 | Job/Media/Provenance（GW10-G） | **PASS** | JOB-01〜10・E2E-3 |
| G6 | System Regression（GW10-H） | **PASS** | E2E-1・SYS-01〜10（required case） |
| GTV | Harness self-verification | **PASS** | GTV-001〜008 |

strict report が報告する非 required の未被覆:

| Case | Status | 理由/後続 |
|---|---|---|
| SYS-02-ux | NotRun | 実 GPUI 未実装 — #114 |
| SYS-03-ux | NotRun | 実 GPUI 未実装 — #114 |
| SYS-05-powerloss | NotRun/Unsupported | OS/電源断は harness 非対応 — 実機 lane・#83 |
| SYS-07-rt | Unsupported | RT budget 未実測（記録のみ） — 実機 lane |
| CONSUMER-RUVIFORM | NotRun | Ruviform 変更なし・rf#294/#553 条件付き |
| CONSUMER-AUDIOLENS | NotRun | 同上 |

## 集計

- GT-001〜012: 12/12 Pass（GT-011/012 は SYS-01/E2E-1 経由・
  GT-012 は headless のみで実 GUI NotRun）
- JM-01〜05: 5/5 Pass（headless/mock・実 Provider/Window Close
  は NotRun）
- MCP-01〜06: 6/6 Pass（PoC real transport/L4・第三者 MCP Host
  は NotRun）
- UX-01〜06: headless Pass(部分)・実 GPUI は全件 NotRun（#114）
- SEC-01〜14: 14/14 Pass（mock/L1-L3）
- DATA-01〜10: 10/10 Pass（mock+real/L2-L4）
- REC-01〜12: 12/12 Pass（mock/L2）
- JOB-01〜10: 10/10 Pass（mock+real/L2-L4）
- SYS-01〜10: required case Pass・SYS-07-rt Unsupported・
  SYS-02-ux/03-ux/05-powerloss/CONSUMER-* NotRun/Unsupported
  （全て required=false）
- AC-01〜10: 10/10 Pass（mock/L2）
- E2E-1/2/3: Pass（E2E-1 mock/L3・E2E-2 PoC real/L4・E2E-3
  mock/L3）
- GTV-001〜008: Pass（GTV-007 は budget Unsupported + consumer
  NotRun を含む）
- G0〜G6・GTV: 全 PASS（strict report・required case）
