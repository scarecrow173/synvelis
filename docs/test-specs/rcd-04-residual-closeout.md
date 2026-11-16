---
id: "TSPEC-034YpVitlyGxkRAEe75uDo"
type: "test-spec"
status: "approved"
title: "RCD-04 residual closeout — AC 照合・canonical lane・gate 棚卸し検証仕様"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-04-residual-closeout.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-04-comp-midi-phrase-fixture.md"
    - "docs/test-specs/rcd-04-midi-undo-stableid.md"
    - "docs/test-specs/rcd-04-product-e2e.md"
    - "docs/test-specs/rcd-04-rt-engine-comp-pcm.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-residual-closeout.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-04-residual-closeout.md"
  verified-by: []
  references:
    - "docs/discovery/rcd-04-residual-closeout.md"
    - "evidence/sv118/rcd-04-closeout-verification.md"
  defers: []
  deferred-by: []
---

# RCD-04 residual closeout — AC 照合・canonical lane・gate 棚卸し検証仕様

## Purpose

RCD-04 residual closeout slice（docs+evidence only）の簿記が
誠実であることを保証する test-spec。新規 test/fixture を
追加せず、既存 test 名・merge 済み PR 記録・canonical lane
実測への 1:1 対応付けと、未実測 lane の NotRun/Deferred 記録
を検証対象とする。実装済み機能そのものの検証は既存
test-spec（crate fixture・product E2E・RT engine comp PCM・
MIDI boundary/stable-ID）が担う。

## Feature

Issue #118 受入条件 4 行の closeout 照合、canonical lane 実測
記録、外部 gate 棚卸し、doc status 簿記の整合。

## Rules

- **R1 1:1 対応付け**: Issue 本文の各受入条件行は、実在する
  test 名（`crates/*/tests/rcd_04_*.rs`）・merge 済み PR
  記録（#219/#300/#319/#374）・実測 lane 結果のいずれかに
  1:1 で対応付けられなければならない。推測で証拠を補わ
  ない。
- **R2 実測のみ Pass**: `Pass` は実際に実行して成功した
  lane のみを指す。未実行・未測定・自動化不能の lane は
  `NotRun`/`Unsupported`/`Deferred` とし、Pass と書かない。
- **R3 gate 記名**: 外部 gate（実 GUI = synvelis#114・
  AudioLens native f64 consumer = `scarecrow173/ruviform#294`・
  MIDI comp selection 永続化 = synvelis#126・実 device/実 RT
  計測 = 実機試験・comp kind VCS-05 apply whitelist 拡張 =
  別 slice deferred・統合 CI 証拠 = merge 後 main SHA の CI）
  は `NotRun`/`Deferred` とし、所有 Issue を記名する。
- **R4 mock 昇格禁止**: headless fixture・fake driver・
  offline renderer・`syn.*` dispatch の PASS を実 GUI・実
  device・実 RT・consumer の証拠に昇格しない。
- **R5 index 非接触**: `docs/**/README.md` 管理 index は
  編集・再生成・`index apply` のいずれも行わない
  （Coordinator が統合時に再生成。index 未同期は許容済み）。
- **R6 範囲遵守**: 変更は worker spec の編集許可範囲（新規
  doc chain 7 file・`docs/impl/exp/
  rcd-04-residual-closeout-exp.jsonl`・
  `docs/discovery/rcd-04-residual-closeout.md`・
  `evidence/sv118/rcd-04-closeout-verification.md`・drift
  修正対象 6 file）に限定され、`crates/`・`ui/`・共有設定・
  他 slug docs・`docs/**/README.md`・`evidence/` 他 dir を
  変更しない。
- **R7 簿記整合**: status 変更（task `in-progress`→`done`、
  impl `in-progress`→`completed`）と矛盾する stale 記述は
  事実に整合させるが、当時の作業記録本文は書き換えない
  （歴史の改竄をしない）。

## Examples

- **E1 AC-1 RT vs Offline PCM**: `rcd_04_take_comp_pcm.rs::
  rcd04_comp_pcm_rt_vs_offline_fixture`（PR #219）と
  `rcd_04_rt_comp_pcm.rs` の
  `rcd_04_rt_engine_comp_pcm_matches_offline_export`・
  `rcd_04_rt_engine_unsupported_mapping_fails_closed`
  （PR #319、HeadlessDriver/RoutingEngine/export 3 経路
  bit-exact + fail-closed 分類）が Issue AC-1 の Source
  Handle/Fade/相関素材/Transient/Beat Anchor/Tempo/Loop 行に
  対応する。実 RT thread・実 device は NotRun。
- **E2 AC-2 MIDI 境界・Undo/Redo・Stable ID**:
  `rcd_04_midi_phrase.rs` 11 tests（`midi_phrase_plan_has_no_
  hanging_notes_or_leaks`・`midi_phrase_chase_covers_kind_
  union_and_mid_phrase_events_play`・`midi_phrase_stable_
  ids_survive_undo_redo_save_reload`・`midi_phrase_preserves_
  native_ticks_without_requantize` 等、PR #219）、
  `rcd_04_midi_phrase_boundary.rs` 2 tests（実 engine 配送
  経路、PR #374）、`rcd_04_vcs_stable_id.rs` 4 tests
  （`comp_stable_ids_survive_save_reload_and_diff`・
  `comp_transition_diff_is_expressed_by_its_own_stable_id`・
  `apply_selected_fails_closed_on_comp_transition`・
  `comp_stable_ids_survive_restore_as_copy`、PR #374）、
  `rcd_04_comp_cas.rs::comp_ids_survive_undo_redo_save_
  reload_and_diff`（PR #219）が対応。実 GUI 照合は
  synvelis#114 gate で Deferred。
- **E3 AC-3 Recovery/CAS/Stale+read-only**:
  `rcd_04_recovery_partial.rs` 4 tests（`rcd04_partial_hold_
  cancel_then_adopt_retry`・`rcd04_partial_discard_requires_
  clear_probe`・`rcd04_group_commit_separates_partial_and_
  held_streams`・`rcd04_corrupt_journal_is_fail_closed`、
  PR #219）、`rcd_04_comp_cas.rs::comp_group_cas_rejects_
  stale_revision_without_partial_apply`（PR #219）、
  `rcd_04_product_e2e.rs` 5 tests（`rcd04_recovery_review_
  product_e2e`・`rcd04_recovery_corrupt_and_discard_guards_
  product_e2e`・`rcd04_stale_gesture_and_readonly_history_
  product_e2e`・`rcd04_comp_transition_product_e2e`・
  `rcd04_midi_phrase_product_e2e`、PR #300）が headless 分に
  対応。実 GUI・実 OS 入力分は synvelis#114 gate。
- **E4 AC-4 MD-A/B 回帰・raw 不変・AudioLens f64**:
  `rcd_04_comp_cas.rs` の `comp_ops_never_mutate_takes_
  sources_or_assets`・`project_comp_is_deterministic_and_
  preserves_provenance`・`comp_switch_is_single_cas_undo_
  and_leaves_raw_immutable`（PR #219）+ lane 全体 pass で
  MD-A/B 回帰を実測。AudioLens native f64 は
  `scarecrow173/ruviform#294` consumer gate で NotRun/
  Deferred。
- **E5 簿記 drift**: merge 済み 3 task file の frontmatter
  status が `done`、3 impl/ir file の status が
  `completed` であり、Done When/Verification の stale `[ ]`
  項目が事実（merge PR 番号・本 closeout evidence 参照）に
  整合していることを diff で確認する。

## Guarantees

- closeout verification evidence の各行が実在 test/merge
  PR/lane に trace できる。
- 未実測 lane が Pass と記録されない。
- doc chain の status/relations が実態と一致する（index の
  再生成は Coordinator 側 — 本 slice では index 未同期を
  許容する）。

## Non-goals

- RCD-04 機能そのものの新規検証（既存 test-spec の責務）。
- 外部 gate の実施・Pass 昇格。
- `*-msrv`・`*-full` 系 lane（Daily Full CI 委譲）。
- docs index 再生成・`doc_status` audit（Coordinator 側）。

## Risk

照合表が実在しない test 名や過去記録の転記ミスを含むと、
closeout の信頼性が損なわれる。test 名は `#[test]`/`fn`
列挙で実在確認し、lane 結果は実行 exit code のみを記録
する。
