---
id: "TSPEC-cb01VariantSetFixtureT"
type: "test-spec"
status: "approved"
title: "CB-01 Creative Branch variant model/controller fixture tests"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/278"
  verifies:
    - "docs/specs/project-model.md"
  supersedes: []
  superseded_by: []
---

# CB-01 Creative Branch variant model/controller tests

## Purpose

synvelis#278 CB-01 スライスの headless fixture が保証すべき
振る舞いを固定する。Project-authoritative な versioned
VariantSet/Branch モデル、single-writer CAS・Undo/Save/Reload
契約、scope 閉じ込め、stale fail-closed、project commit と
runtime publish の receipt 分離を、Fake Domain Adapter レベルで
検証する。実 OS/実 Plugin/実 RT/実 Render に依存する判定は
本 spec の対象外（`evidence/sv278/` で NotRun/Deferred 記録）。
spec/design が CB-02 以降で再設計された場合でも、ここに記す
不変条件（CAS 原子性・fail-closed・receipt 分離）は維持される
べき保証であり、本 test spec はその判定基準として残す。

## Feature

`Project` が所有する `variant_sets` 永続モデルと、それに対する
`ProjectController` 経由の Fork/Edit/Adopt/Discard 操作。
branch content は digest 参照であり blob を埋め込まない。

## Rules

- **Rule CB-SCOPE-TYPE**: 7 種の `VariantScope` は typed
  serializer/reloader で往復し、未知 kind tag・空 track 選択・
  空 domain mask・未解決 root 参照を拒否する。wire 形式は
  `kind` tag で self-describing。
  Examples: `cb_model_01_scope_typed_serde_roundtrip`,
  `cb_model_07_invalid_scope_and_unsupported_op_rejected`.
- **Rule CB-PERSIST**: variant set を含む project snapshot は
  save→load で同一 state に復元され、v13 body は migration
  ladder 経由で v14 へ昇格する。stable ID・asset・HEAD を壊さない。
  Examples: `cb_model_01_variant_set_save_load_roundtrip`,
  `cb_model_01_v13_snapshot_migrates_to_v14`.
- **Rule CB-ISOLATION**: 非採用 branch の edit は live domain
  state を一切変更しない（clip・notes・asset が bit 同一）。
  採用 branch の edit は live op + branch bookkeeping を 1 CAS
  で適用し、Undo/Redo/Save/Reload で分岐しない。
  Examples: `cb_model_02_candidate_edit_leaves_live_untouched`,
  `cb_model_03_adopted_edit_single_cas_and_roundtrip`.
- **Rule CB-ADOPT-CAS**: expected project revision・branch
  revision・scope digest のいずれかが stale なら adopt を拒否し、
  live・undo 履歴・HEAD・候補のいずれにも部分適用がない。
  未承認 intent も拒否。
  Examples: `cb_model_04_stale_adopt_rejected_without_partial_apply`.
- **Rule CB-ADOPT-ATOMIC**: 独立複数 object の adopt は 1 commit +
  1 undo。redo は ID・参照を再発行せず安定に戻る。
  Examples: `cb_model_05_multi_object_adopt_single_commit_single_undo`.
- **Rule CB-LINEAGE**: branch-local に mint された ID は live
  空間予約により live/sibling と衝突せず、adopt で同一 ID のまま
  materialize される。lineage による provenance が記録され、
  ambiguous lineage は reject。candidate payload は digest 不一致を
  検出して fail-closed で拒否する。
  Examples: `cb_model_06_branch_local_ids_no_collision_and_lineage`,
  `cb_model_06_candidate_payload_digest_is_verified`,
  `variant::engine::tests::ambiguous_lineage_is_rejected`.
- **Rule CB-REJECT**: overlapping scope・invalid scope・unsupported
  op・共有 MIDI source（ambiguous ownership）・scope 外 edit target
  は typed reject であり、無断置換・推測正規化を行わない。
  Examples: `cb_model_07_overlapping_scope_rejected`,
  `cb_model_07_shared_midi_source_scope_rejected`.
- **Rule CB-PLUGIN-PORT**: Missing Plugin adopt は structural
  state・plugin identity・semantic ports・control targets が
  検証できる場合に限り安全に許可し、新規 resolved binding が
  unavailable plugin を指す場合は typed reject（部分適用なし）。
  unknown port・方向不一致は既存 preflight が拒否する。
  Examples: `cb_model_rcp_01_missing_plugin_safe_adopt_and_reject`.
- **Rule CB-RECEIPT-SPLIT**: project CAS 成功と runtime publish
  は別 receipt。publish が Failed/Pending でも project commit は
  維持され、undo は実 commit に対して機能する。runtime が
  current であるかのような偽装をしない。
  Examples: `cb_model_08_runtime_failure_keeps_project_committed`,
  `cb_model_08_pending_publish_is_not_reported_as_current`.
- **Rule CB-RCP-IDENTITY**: scope fingerprint（scope digest）は
  同一 scoped 内容に対し branch 横断・adopt 前後で一致する
  render-reuse identity。
  Examples: `cb_model_rcp_01_scope_digest_is_render_reuse_identity`.
- **Rule CB-DISCARD**: discard は非採用 branch のみ除去し、
  live・採用 branch に影響しない。payload は unreferenced orphan
  として残る。採用 branch の discard は拒否。
  Examples: `discard_branch_removes_candidate_only`.

## Examples

上記 Rules の Examples に列挙した test 関数が具体 scenario。
fixture は `crates/synvelis-project/tests/cb01_variant.rs` と
`src/variant/engine.rs` の unit test。

## Guarantees

- variant state は `ProjectController` single-writer CAS のみが
  変更し、undo/redo/save/reload の既存契約をそのまま継承する。
- stale/divergent lineage・scope・revision は常に fail-closed で、
  部分適用・自動 rebase・推測を行わない。
- project commit と runtime activation の証拠は分離され、
  publish 失敗が project truth を偽装しない。
- candidate payload は immutable 参照 + digest であり、project
  model に blob を埋め込まない。

## Non-goals

- 実ドメイン adapter（audio/comping/midi/routing 個別実装）、
  Comparison Orchestrator、GUI、VCS、実 plugin、RT、render
  pipeline の検証 — 後続 Issue（#279〜296 系）の範囲。
- `synvelis-workspace` session glue 配線 — 並列 worker との衝突
  回避のため DEFER。
- scope entity の削除を伴う edit（CB-01 では typed reject）。
- legacy v5 系 snapshot の variant 固有移行（pass-through のみ）。

## Risk

- これらの保証を落とすと、branch 間の live 汚染・stale adopt の
  部分適用・ID 衝突・digest 偽装 payload の混入が起こり得る。
  特に CAS 検査と receipt 分離は CB-02 以降の全 adopt 経路が
  依存する土台であり、退行は critical path 全体を危うくする。
