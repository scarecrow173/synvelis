# Acceptance Ledger — synvelis#286 CB-09 Difference Explorer headless slice

Date: 2026-11-15. Slice: CB-09 Difference Explorer — headless
view-model + driver-neutral ViewSpec（mock/Fake driver 並行開始
slice）。Issue #286 の headless 受入行を `cb09_diff_explorer.rs`
の 33 test と doc chain に写像した監査台帳。

**実 GPUI mount・実 plugin dock・PCM/RT signal 測定・OS/IME・
CB-08 orchestrator 経由 e2e は一切実行していない。** headless
の Pass を実 GUI/plugin/RT/OS の Pass に昇格しない。それらは
synvelis#288 系の別 acceptance gate が Owner。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/286
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | `48f1e1e5d807579d2af67ad97771b38eae34f6f0`（origin/main base）+ 本 change set |
| Ruviform resolved SHA | `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（`Cargo.lock` git dep・変更なし） |
| OS / CPU / FS | Windows（win32）/ NTFS |
| Toolchain | rustc 1.99.1（`58cf8f9c3`、mise 管理。CI pin は `mise.ci.toml` の 1.99.0） |
| Device + Plugin | なし（headless — audio device・native plugin host 未接続。signal/perceptual は mock seam） |

## 判定語彙

- **Pass-measured (headless)**: `mise run test
  synvelis-workspace` が当該 change set で green だった
  `cb09_*` test の存在を確認したことのみを意味する。
- **NotRun**: 実 OS/実 GUI/実 plugin/実 device/実 RT/
  orchestrator e2e — 本 slice で実行していない部分。
- **Unsupported**: Arrangement domain の adapter 未 merge
  （CB-07/#284）— `EvidenceState::Unsupported` として型で
  表現される設計上の非対応。捏造していない。

## 受入行 → 証拠写像

| 受入行 | 要件 | 証拠（test / doc） | 判定 |
| --- | --- | --- | --- |
| CB-UX-01 | 明示的 pinned target の action binding | `cb09_compare_and_adopt_require_explicit_pins`, `cb09_implicit_malformed_and_stale_targets_fail_closed` | Pass-measured (headless) |
| CB-UX-02 | Adopted/Auditioning/Editing/RT label の独立性 | `cb09_branch_labels_are_independent_axes` | Pass-measured (headless) |
| CB-UX-03 | per-domain diff projection・unsupported の正直な扱い | `cb09_domain_labels_cover_every_scope`, `cb09_arrangement_domain_never_fabricates_rows`, `cb09_supported_domain_projects_empty_diff_as_equal` | Pass-measured (headless) |
| CB-UX-04 | adopt/adopt-preview が pinned branch + scope digest を運ぶ | `cb09_compare_and_adopt_require_explicit_pins`（intent の `scope_digest == authority.scope_digest` を assert） | Pass-measured (headless) |
| CB-CMP-01 | stale/rejected context の明示と action 封鎖 | `cb09_stale_and_rejected_contexts_are_honest`, `cb09_blocked_actions_never_become_buttons`, `cb09_apply_revalidates_context_against_apply_time_authority`（projection→apply 間の authority drift でも intent を mint しない） | Pass-measured (headless) |
| CB-CMP-02 | Unknown/Unavailable を Equal/zero-diff に昇格しない | `cb09_unavailable_and_unknown_layers_are_not_zero_diff` | Pass-measured (headless) |
| CB-CMP-03 | Semantic/Signal/Perceptual layer の分離 + mock provenance | `cb09_mock_evidence_is_labelled_not_measured`, `cb09_unavailable_and_unknown_layers_are_not_zero_diff` | Pass-measured (headless) |
| CB-UI-RCP-01 | editability と exact audition の capability 分離 | `cb09_missing_plugin_splits_editability_from_audition` | Pass-measured (headless) |
| — | Semantic evidence は merged `compare_snapshots` の実出力 | `cb09_real_changeset_projects_domain_rows`（real variant-set fixture: fork→edit_branch→decode_candidate→compare_snapshots→投影。`clip/modified` row・`variants_bookkeeping >= 1`・bookkeeping 行の content 非表示を assert） | Pass-measured (headless) |
| — | driver-neutral ViewSpec の determinism・stable key・executable-only control | `cb09_viewspec_is_deterministic_with_stable_keys`, `cb09_blocked_actions_never_become_buttons`, `cb09_spec_keys_unique_with_two_non_adopted_sides`, `cb09_spec_keys_unique_with_many_issues_and_reasons`, `cb09_every_button_params_roundtrips_decode` | Pass-measured (headless) |
| — | 投影 row/issue の上限と省略の正直な表示 | `cb09_rows_issues_and_aux_rows_are_capped_honestly`（256/128/256 cap・omitted count・spec の "showing X of Y" label） | Pass-measured (headless) |
| — | pinned context identity（variant set/project revision/scope digest）の spec 表示 | `cb09_pinned_context_identity_is_visible_in_spec` | Pass-measured (headless) |
| — | view-local state が authority を持たず serde tolerant | `cb09_view_local_actions_never_touch_authority`, `cb09_viewstate_roundtrip_and_tolerant_decode`, `cb09_selected_row_projects_field_detail`（selection が spec に観測可能な field detail を出す） | Pass-measured (headless) |
| — | ViewType/論理 target の登録 | `cb09_view_type_registers_as_variant_diff`（`"variant-diff"`・`cb.variant-diff`） | Pass-measured (headless) |
| — | 実 GPUI mount / Fake・Mock driver 実消費 | — | NotRun（#288 系 gate） |
| — | 実 plugin state・PCM/RT signal 測定・perceptual 評価 | — | NotRun（#288 系 gate） |
| — | CB-08 orchestrator からの end-to-end 駆動・adopt 実行 | — | NotRun（#285/#288 系） |
| — | OS window/IME/accessibility 実機 | — | NotRun（#288 系 gate） |

## fixture 補完判定

新規 fixture/test として `tests/cb09_diff_explorer.rs` を追加
（33 test）。semantic evidence は mock ではなく merged
`synvelis_project::diff::compare_snapshots` を real variant-set
fixture（`fork_variant_set` → `edit_branch` → `decode_candidate`）
で駆動する — headless 成功が real 測定を装わないよう、signal/
perceptual は `EvidenceOrigin::Mock` として provenance label を
spec に露出する設計を test で固定した。

## Doc chain

- spec: `docs/specs/creative-branch.md`（既存）
- design: `docs/designs/creative-branch-design.md`（既存）
- test-spec: `docs/test-specs/cb-09-diff-explorer.md`（新規）
- impl record: `docs/impl/ir/cb-09-diff-explorer.md`（新規）
- experiment log: `docs/impl/exp/cb-09-diff-explorer.md`（新規）
- index: `docs/impl/ir/README.md`・`docs/test-specs/README.md`
  を再生成済み（`docs/impl/exp/README.md` は jsonl index のみ
  管理で .md exp は対象外 — cb-01〜06 と同じ扱い）
