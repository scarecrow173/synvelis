---
id: "SPEC-034YpSBp6LbjHyZJstrgDt"
type: "spec"
status: "implemented"
title: "RCD-04 residual closeout — Issue #118 AC 照合・canonical lane 実測・gate 棚卸し・簿記 drift 修正"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/designs/rcd-04-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/test-specs/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/impl/exp/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/impl/exp/rcd-04-residual-closeout-exp.jsonl"
      - type: "file"
        path: "docs/discovery/rcd-04-residual-closeout.md"
      - type: "file"
        path: "evidence/sv118/rcd-04-closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/tasks/rcd-04-task-midi-undo-stableid.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task-rt-engine-comp-pcm.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task1-product-e2e.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-midi-undo-stableid-slice.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-product-e2e-slice.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-rt-engine-comp-pcm-slice.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/rcd-04-residual-closeout-design.md"
    - "docs/plans/rcd-04-residual-closeout.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/specs/rcd-04-midi-undo-stableid.md"
    - "docs/specs/rcd-04-product-e2e.md"
    - "docs/specs/rcd-04-rt-engine-comp-pcm.md"
    - "docs/impl/ir/rcd-04-comp-transition-midi-phrase-crate-fixture-slice.md"
    - "docs/impl/ir/rcd-04-midi-undo-stableid-slice.md"
    - "docs/impl/ir/rcd-04-product-e2e-slice.md"
    - "docs/impl/ir/rcd-04-rt-engine-comp-pcm-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/discovery/rcd-04-residual-closeout.md"
    - "evidence/sv118/rcd-04-closeout-verification.md"
  defers: []
  deferred-by: []
---

# RCD-04 residual closeout — Issue #118 AC 照合・canonical lane 実測・gate 棚卸し・簿記 drift 修正

## What Should Be Built

Issue #118（RCD-04: Comp Transition・MIDI Phrase・Recovery
Review の製品 E2E）の headless 実装は PR #219・#300・#319・
#374 で main に統合済みである。本 spec が定めるのは
**簿記クローズアウト**のみであり、新規のプロダクション実装・
test・fixture の追加は含まない。

- Issue 本文の受入条件 4 行を、実在する test 名・merge 済み
  PR の記録と 1:1 で照合し、`evidence/sv118/
  rcd-04-closeout-verification.md` に記録する（既存 test 名の
  列挙・merge PR 引用の consolidate のみで再 derive しない）。
- canonical lane（`mise run test synvelis-project` /
  `synvelis-recording` / `synvelis-player` / `synvelis-render`
  / `test-rt`）を現行 main ベースの worktree で実走し、
  Pass/Fail を記録する。opt-in `mise run test synvelis-app`
  （`rcd_04_product_e2e` の所在 package）は任意実行とし、
  実行しない場合は NotRun + merge PR #300 の記録を引用する。
  `*-msrv`・`*-full` 系は Daily Full CI 委譲で NotRun とする。
- 外部 gate（実 GUI = synvelis#114・AudioLens native f64
  consumer = `scarecrow173/ruviform#294`・MIDI comp selection
  永続化 = synvelis#126・実 device/実 RT 計測 = 実機試験・
  comp kind VCS-05 apply whitelist 拡張 = 別 slice deferred・
  統合 CI 証拠 = merge 後 main SHA の CI）は `NotRun`/
  `Deferred` とし、所有 Issue を記名する。headless/fixture
  の PASS を実 GUI・実 device・実 RT・consumer の Pass に
  昇格しない。
- 簿記 drift を修正する: merge 済みの
  `docs/tasks/rcd-04-task-{midi-undo-stableid,
  rt-engine-comp-pcm,task1-product-e2e}.md` の status
  `in-progress` → `done`、`docs/impl/ir/rcd-04-{midi-undo-
  stableid,product-e2e,rt-engine-comp-pcm}-slice.md` の status
  `in-progress` → `completed`。status と矛盾する stale な
  Done When/Verification の `[ ]` 項目は事実に整合させる
  （merge 済みで実施済みの項目は `[x]` + merge PR 番号注記、
  当時実施していない項目は `[ ]` のまま正直に残すか本
  closeout evidence への参照を付記。歴史の改竄はしない）。
- doc status の簿記を実態に整合させ、Implementation Record
  `docs/impl/ir/rcd-04-residual-closeout.md` と Experiment
  Log `docs/impl/exp/rcd-04-residual-closeout.md`（+
  run-trace `docs/impl/exp/rcd-04-residual-closeout-exp.jsonl`）
  に結果を固定する。棚卸しメモ `docs/discovery/
  rcd-04-residual-closeout.md` に closeout 対象の探索結果を
  記録する。

## Why Now

RCD-04 の headless 実装は全て merge 済みだが、Issue の受入
条件各行と実測証拠の対応表・残存 gate の所在・task/impl の
status 簿記が一箇所に集約されていない。Issue は実 GUI
（synvelis#114）・consumer（`scarecrow173/ruviform#294`）・
MIDI 永続化（synvelis#126）等の外部 gate が残るため open
継続が前提であり、「何が headless で証明済みで、何がどの
Issue の gate か」を canonical doc に固定することで、後続の
close 判定と並行 Worker の衝突を防ぐ。sv#116 の
`rcd-02-residual-closeout`・sv#86 の `alib-02-residual-
closeout` 先例と同型の closeout slice。

## Users and Value

- **Issue #118 owner / reviewer**: 受入条件各行の証拠所在と
  外部 gate の owner が一目で分かり、close 判定の材料になる。
- **後続 Worker / 保守者**: どの doc・fixture・lane が RCD-04
  の何を証明するかの正本にたどり着ける。
- **doc-driven-dev 監査**: status/relations が実態と一致し、
  stale-upstream・orphan の誤検出を減らす。

## Scope

### In Scope

- `docs/` 配下の RCD-04 / Issue #118 関連 doc chain の
  status・relations の実態整合（編集許可範囲に限定）。
- 新規 doc chain: 本 spec・design・plan・task・test-spec・IR・
  exp（`rcd-04-residual-closeout*`）+ 棚卸しメモ
  `docs/discovery/rcd-04-residual-closeout.md` + run-trace
  `docs/impl/exp/rcd-04-residual-closeout-exp.jsonl`。
- `evidence/sv118/rcd-04-closeout-verification.md` の新規作成
  （AC 照合表・canonical lane 実測・gate 棚卸し）。
- merge 済み 3 task file の status 修正（`in-progress`→
  `done`）と stale checkbox の事実整合、merge 済み 3 impl/ir
  file の status 修正（`in-progress`→`completed`）と merge PR
  対応付けの注記。
- canonical mise lane の実走と結果記録。
- PR 作成（Issue の close・merge は行わない）。

### Out of Scope

- プロダクションコード（`crates/`）・`ui/`・Cargo/mise/
  workflow 定義の変更。新規 test・fixture・harness の作成
  （AC の root 証拠は既存 test 名・merge PR/CI 記録の引用に
  限定 — consolidate のみ）。
- 外部 gate の NotRun→Pass 昇格、または gate 自体の実施
  （実 GUI・実 device・実 RT 計測・consumer・MIDI 永続化・
  comp kind apply whitelist 拡張）。
- `*-msrv`・`*-full` 系 lane（Daily Full CI 委譲）。
- docs index（`docs/**/README.md`）の編集・再生成・
  `index apply` 実行 — Coordinator が統合時に再生成する。
- docs index regen・`doc_status` audit を本 slice の検証・
  作業として実施すること（Coordinator 側の責務）。
- Issue #118 の編集・close（merge と Close は Coordinator が
  判断）。
- 他 Issue 所有の残 gate 実装（synvelis#114 実 GUI・
  `scarecrow173/ruviform#294` consumer・synvelis#126 MIDI
  永続化・実機試験）。

## Requirements

- Issue 本文の各受入条件行を、実在する test 名・merge 済み
  PR の記録と 1:1 で対応付ける。対応する証拠が無い行は理由
  付きで残す。
- canonical lane は `mise run <task>` 経由で実走し、Pass/Fail
  を分けて記録する。未実行を Pass と書かない。
- 外部 gate は `NotRun`/`Deferred`/`Unsupported` とし、所有
  Issue（synvelis#114・synvelis#126・
  `scarecrow173/ruviform#294`・実機試験・別 slice）を記名
  する。
- headless fixture・fake driver・offline renderer の結果を
  実 GUI・実 device・実 RT・consumer の証拠として扱わない。
- doc 間の relations は双方向整合を保ち、既存の legacy id
  形式 finding（repo-wide migration 対象）を新規に増やさない。
- `docs/**/README.md` 管理 index は手編集も再生成もしない
  （Coordinator が統合時に `doc-maintenance` で再生成）。

## Acceptance Criteria

Issue 本文「受入条件」4 行の照合結果（詳細は
`docs/impl/ir/rcd-04-residual-closeout.md` と
`evidence/sv118/rcd-04-closeout-verification.md`）:

- [x] AC-1 Source Handle範囲/Fade区間/相関素材/Transient/
  Beat Anchor/Tempo/Loop で RT PCM と Offline PCM の差を記録
  — `crates/synvelis-recording/tests/
  rcd_04_take_comp_pcm.rs::rcd04_comp_pcm_rt_vs_offline_
  fixture`（PR #219）・`crates/synvelis-render/tests/
  rcd_04_rt_comp_pcm.rs` の
  `rcd_04_rt_engine_comp_pcm_matches_offline_export` +
  `rcd_04_rt_engine_unsupported_mapping_fails_closed`
  （PR #319、3 実装経路 bit-exact + fail-closed 分類）で
  headless 実測済み。実 RT thread 計測・実 device は
  NotRun（実機 gate）。
- [x] AC-2 MIDI phrase境界で Hanging Note/Controller Leak
  なし、Undo/Redo、Save/Reload、VCS Diff で Stable ID 維持
  — `rcd_04_midi_phrase.rs` 11 tests（PR #219）・
  `rcd_04_midi_phrase_boundary.rs` 2 tests（実 engine 配送
  経路、PR #374）・`rcd_04_vcs_stable_id.rs` 4 tests
  （PR #374）・`rcd_04_comp_cas.rs::
  comp_ids_survive_undo_redo_save_reload_and_diff`（PR #219）
  で headless 実測済み。実 GUI での同条件照合は
  synvelis#114 gate で Deferred。
- [x] AC-3 Recovery partial+cancel+retry、Group Comp 全体
  CAS、2Window Stale Gesture/History view read-only を実
  GUI＋headless で検証 — headless 分は
  `rcd_04_recovery_partial.rs` 4 tests・`rcd_04_comp_cas.rs::
  comp_group_cas_rejects_stale_revision_without_partial_
  apply`（PR #219）・`rcd_04_product_e2e.rs` 5 tests
  （`syn.*` op dispatch 経路、PR #300）で実測済み。実 GUI・
  実 OS 入力分は synvelis#114 gate で NotRun/Deferred。
- [x] AC-4 既存 MD-A/B の回帰、raw PCM/Asset 不変、
  AudioLens native f64 非回帰 — `rcd_04_comp_cas.rs` の
  `comp_ops_never_mutate_takes_sources_or_assets`・
  `project_comp_is_deterministic_and_preserves_provenance`・
  `comp_switch_is_single_cas_undo_and_leaves_raw_immutable`
  （PR #219）+ 指定 package lane の全体 pass で MD-A/B 回帰
  を実測。AudioLens native f64 は consumer gate
  （`scarecrow173/ruviform#294`）で NotRun/Deferred — 他
  consumer の結果を流用しない。
- [x] 簿記 drift 修正（slice 受入条件）— merge 済み 3 task
  file の status を `done`、3 impl/ir file の status を
  `completed` に修正し、status と矛盾する stale checkbox を
  事実に整合させた（詳細は
  `docs/impl/ir/rcd-04-residual-closeout.md`）。

## Deferred Design Concerns

- 実 GUI（Take/Comp/Recovery Review の製品 GPUI Scenario・
  2Window Stale Gesture/History view read-only の実 OS 入力
  検証）| headless/`syn.*` dispatch のみ実測済み | synvelis#114
  | headless PASS を実 GUI PASS と誤認する簿記 drift。
- AudioLens native f64 / raw-recording 非回帰の consumer 照合
  | Synvelis 側は raw PCM/Asset 不変を pin 済み |
  `scarecrow173/ruviform#294` | 他 consumer の結果を Synvelis
  の Pass に流用する誤記。
- MIDI comp selection の project 永続化 | plan 評価層のみ・
  #126 の native-tick/chase 意味論未確定 | synvelis#126 |
  plan 層証拠を永続化済みと誤認。
- 実 device/実 RT thread 計測（Source Handle/Fade/相関素材等の
  RT vs Offline PCM 差の実機記録）| headless deterministic
  fixture のみ | 実機試験 | fixture の一致を実機音質保証と
  誤認。
- comp kind の VCS-05 apply whitelist 拡張 | 現行は
  `ApplyError::Unsupported` fail-closed が契約で
  `apply_selected_fails_closed_on_comp_transition` に pin
  済み | 別 slice（typed entity extraction/transplant/
  membership reconcile を含む設計が必要） | fail-closed を
  暗黙に緩める変更。
- Issue #118 の close 判定 | 上記外部 gate が残存 | gate 所有
  Issue の完了後 | 未証明行を閉じたままにする誤判定。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/118>
  （RCD-04 tracker — auto-close しない。実 GUI=synvelis#114・
  consumer=`scarecrow173/ruviform#294`・MIDI 永続化=
  synvelis#126 等の外部 gate が残存）。
- 統合済み実装: PR #219（`9798be5` — Comp Transition /
  MIDI Phrase crate fixture slice: `rcd_04_comp_cas`・
  `rcd_04_midi_phrase`・`rcd_04_recovery_partial`・
  `rcd_04_take_comp_pcm`）・PR #300（`a030e1f` — product
  E2E: `crates/synvelis-app/tests/rcd_04_product_e2e.rs`）・
  PR #319（`ff33ffb` — RT engine comp PCM:
  `crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs`）・
  PR #374（`4946a59` — MIDI phrase boundary / Undo /
  VCS stable-ID: `rcd_04_midi_phrase_boundary`・
  `rcd_04_vcs_stable_id`）。
- 関連 spec: `rcd-04-comp-transition-midi-phrase-fixture`・
  `rcd-04-midi-undo-stableid`・`rcd-04-product-e2e`・
  `rcd-04-rt-engine-comp-pcm`。
- 棚卸し: `docs/discovery/rcd-04-residual-closeout.md`。
- 証拠: `evidence/sv118/rcd-04-closeout-verification.md`。
