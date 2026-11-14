# Acceptance Ledger — synvelis#278 CB-01 headless スライス

Date: 2026-11-14. Slice: CB-01 VariantSet/Scope/Lineage +
ProjectController Fork/Edit/Adopt/Undo + 永続 schema v14 +
Fake Domain Adapter fixture（model + schema + Fake Adapter の
headless 部分のみ）。実ドメイン Adapter（#279〜284）・
Comparison Orchestrator（#285）・GPUI（#286）・Action/Gateway
（#287）・`synvelis-workspace` session glue は DEFER。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/278
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | feat/278-variant-set HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 / Intel Core i9-9900K |
| Toolchain | mise 管理 Rust（`mise run` 経由） |
| Project schema | v14（v13→v14 pass-through migration 追加） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-project/tests/
  cb01_variant.rs` または `src/variant/engine.rs` unit test が
  `mise run test synvelis-project` で green。
- **NotRun**: 実 OS/実 Plugin/実 RT/実 Render/実機依存の判定 —
  本スライスでは実行していない。
- **Deferred**: 後続 Issue の範囲として明示的に送った部分。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| CB-MODEL-RCP-01（fixture 部分） | Pass (headless) | `cb_model_rcp_01_scope_digest_is_render_reuse_identity`（同一内容の scope digest 一致 = render 再利用 identity）、`cb_model_rcp_01_missing_plugin_safe_adopt_and_reject`（Missing Plugin の安全 adopt + 不正 binding reject）。実 Render 再利用評価は Deferred（#289〜296 系）。Undo/Redo/Reload 参照維持は `cb_model_03_*`・`cb_model_01_variant_set_save_load_roundtrip`。 |
| CB-MODEL-01 | Pass (headless) | `cb_model_01_scope_typed_serde_roundtrip`（7 scope の typed serde 往復 + kind tag 改竄拒否）、`cb_model_01_variant_set_save_load_roundtrip`（stable ID/asset/HEAD 保持）、`cb_model_01_v13_snapshot_migrates_to_v14`（schema migration）。 |
| CB-MODEL-02 | Pass (headless) | `cb_model_02_candidate_edit_leaves_live_untouched`（Fake Adapter fixture — 採用 A の Live 対象不変）。 |
| CB-MODEL-03 | Pass (headless) | `cb_model_03_adopted_edit_single_cas_and_roundtrip`（採用 branch 通常 edit が live+候補を同一 CAS で更新、Undo/Redo/Save/Reload で非分岐）。 |
| CB-MODEL-04 | Pass (headless) | `cb_model_04_stale_adopt_rejected_without_partial_apply`（stale project/branch/scope CAS + 未承認を fail-closed、live 完全一致を確認）。 |
| CB-MODEL-05 | Pass (headless) | `cb_model_05_multi_object_adopt_single_commit_single_undo`（複数 object adopt = 1 commit + 1 undo、redo で ID 安定）。 |
| CB-MODEL-06 | Pass (headless) | `cb_model_06_branch_local_ids_no_collision_and_lineage`（live-space 予約で衝突なし・同一 ID adopt）、`cb_model_06_candidate_payload_digest_is_verified`、unit `ambiguous_lineage_is_rejected`。 |
| CB-MODEL-07 | Pass (headless) | `cb_model_07_overlapping_scope_rejected`、`cb_model_07_invalid_scope_and_unsupported_op_rejected`、`cb_model_07_shared_midi_source_scope_rejected`（typed reject・無断置換なし）。unknown Plugin/Asset・legacy version の実機系判定は NotRun/Deferred。 |
| CB-MODEL-08 | Pass (headless) | `cb_model_08_runtime_failure_keeps_project_committed`、`cb_model_08_pending_publish_is_not_reported_as_current`（project commit ≠ runtime publish の別 receipt、失敗時 pending/last-good 分離）。実 runtime publish は Deferred（#285/#287）。 |

## NotRun / Deferred 一覧

- 実 OS provider・実 Plugin host・実 RT・実 Render pipeline:
  NotRun（headless fixture のみ。Fake/Domain Oracle の PASS を
  実機 PASS に昇格しない）。
- 実ドメイン Adapter（CB-02〜07 = Issue #279〜284）:
  Deferred — 本スライスは `DomainAdapter` 境界と Fake Adapter のみ。
- Comparison Orchestrator（#285）、GPUI（#286）、Action/Gateway
  統合（#287）: Deferred。
- `crates/synvelis-workspace/` session glue 配線: Deferred
  （並列 worker との衝突回避。PR 本文「未対応の作業」に記録）。
- scope entity 削除を伴う edit: Unsupported（CB-01 では typed
  reject — `adapter.rs` の scope-removal 検査）。
- Legacy v5/旧 schema の variant 固有対応: pass-through migration
  のみ（固有 backfill なし — authored state は derive しない）。
