---
id: "DISC-034YpZlAbwLzlPh4pcs42X"
type: "discovery"
status: "resolved"
title: "RCD-04 residual closeout — Issue #118 AC 照合対象・簿記 drift・外部 gate 棚卸しメモ"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
    - "docs/specs/rcd-04-residual-closeout.md"
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
    - "docs/specs/rcd-02-residual-closeout.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-residual-closeout.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv118/rcd-04-closeout-verification.md"
  defers: []
  deferred-by: []
---

# RCD-04 residual closeout — Issue #118 AC 照合対象・簿記 drift・外部 gate 棚卸しメモ

## Summary

Issue #118（RCD-04 tracker）の residual closeout に先立ち、
AC 照合の root 証拠・簿記 drift 対象・外部 gate の棚卸しを
行った。結論: headless 実装は merge 済み PR #219/#300/#319/
#374 で充足済み、残件は簿記のみ。ただし実 GUI（synvelis#114）・
consumer（`scarecrow173/ruviform#294`）・MIDI 永続化
（synvelis#126）等の外部 gate が残るため Issue は open 維持
（confidence: high — merge commit・test 名・frontmatter を
直接確認済み）。

## Exploration Goal

closeout slice の編集境界と AC 照合表を確定するため、(1)
Issue #118 受入条件 4 行に対応する実在 test/merge PR の
所在、(2) merge 済みなのに `in-progress` のまま残る
task/impl doc の特定、(3) 本 slice では Pass に昇格できない
外部 gate の帰属 Issue の確定、の 3 点を棚卸しする。

## Key Issues and Assumptions

- RCD-04 headless 実装は全て merge 済み（PR #219=`9798be5`
  crate fixture・PR #300=`a030e1f` product E2E・PR #319=
  `ff33ffb` RT engine comp PCM・PR #374=`4946a59` MIDI
  boundary/stable-ID）— confirmed（git log / PR record）。
- `docs/tasks/rcd-04-task1-comp-midi-fixture.md`（done）と
  `docs/impl/ir/rcd-04-comp-transition-midi-phrase-crate-
  fixture-slice.md`（completed、PR #219 分）は既に整合済み
  で修正対象外 — confirmed（frontmatter 確認）。
- docs index（`docs/**/README.md`）は Coordinator が統合時に
  再生成するため本 slice では一切触れない — confirmed
  （worker spec 明示）。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| 既存 test/merge 記録を引用する consolidate のみ | 再 derive 不要・証拠の一貫性 | 新規検証は生まれない | yes |
| 新規 fixture/test で再検証 | 独立した新証拠 | 範囲外（docs+evidence only）・重複 | no |
| 外部 gate を headless 結果で代替 | 記録が単純 | POLICIES 違反（mock 昇格禁止） | no |

## Tentative Conclusions and Hypotheses

- confirmed: AC-1（RT vs Offline PCM）は
  `rcd_04_take_comp_pcm.rs::rcd04_comp_pcm_rt_vs_offline_
  fixture`（PR #219）+ `rcd_04_rt_comp_pcm.rs` 2 tests
  （PR #319）に対応。実 RT thread/実 device は NotRun。
- confirmed: AC-2（MIDI 境界・Undo/Redo・Stable ID）は
  `rcd_04_midi_phrase.rs` 11 tests（PR #219）+
  `rcd_04_midi_phrase_boundary.rs` 2 tests +
  `rcd_04_vcs_stable_id.rs` 4 tests（PR #374）+
  `rcd_04_comp_cas.rs::comp_ids_survive_undo_redo_save_
  reload_and_diff`（PR #219）に対応。
- confirmed: AC-3（Recovery/CAS/Stale+read-only）は
  `rcd_04_recovery_partial.rs` 4 tests +
  `comp_group_cas_rejects_stale_revision_without_partial_
  apply`（PR #219）+ `rcd_04_product_e2e.rs` 5 tests
  （PR #300、headless `syn.*` dispatch）に対応。実 GUI・
  実 OS 入力は synvelis#114 gate。
- confirmed: AC-4（MD-A/B 回帰・raw 不変・AudioLens f64）は
  `rcd_04_comp_cas.rs` の不変性 3 tests（PR #219）+ lane
  全体 pass。AudioLens native f64 は `scarecrow173/
  ruviform#294` consumer gate で NotRun/Deferred。
- confirmed: 簿記 drift は merge 済み 3 task（status
  `in-progress`→`done`）+ 3 impl/ir（`in-progress`→
  `completed`）の計 6 file。

## Open Questions

- なし（closeout 範囲は worker spec で確定済み。外部 gate
  の実施は各所有 Issue の責務で本 slice の対象外）。

## Promotion Candidates

- [x] spec-doc: `docs/specs/rcd-04-residual-closeout.md`（作成済み）
- [ ] adr-doc needed: なし（簿記 slice で新規アーキテクチャ
      判断なし）

## Sources and Relations

- Issue #118（RCD-04 tracker — auto-close しない）
- merge 済み PR: #219・#300・#319・#374
- 先例: `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/rcd-02-residual-closeout*`（sv#116）・
  `alib-02-residual-closeout*`（sv#86）・
  `evidence/sv116/rcd-02-closeout-verification.md`
- 外部 gate: synvelis#114・synvelis#126・synvelis#84・
  synvelis#95・`scarecrow173/ruviform#294`・実機試験

## Evidence

- 2026-11-16: worktree `G:\orca\workspaces\synvelis\
  sv118-rcd04-closeout`、branch `feat/118-rcd04-closeout`、
  base `origin/main`=`0b57299`。`git status` clean。
- 2026-11-16: RCD-04 test file 棚卸し（実在 path 確認済み）—
  `crates/synvelis-project/tests/rcd_04_{comp_cas,
  midi_phrase,vcs_stable_id}.rs`・
  `crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.
  rs`・`crates/synvelis-recording/tests/rcd_04_{recovery_
  partial,take_comp_pcm}.rs`・`crates/synvelis-render/tests/
  rcd_04_rt_comp_pcm.rs`・`crates/synvelis-app/tests/
  rcd_04_product_e2e.rs`（test 名は
  `evidence/sv118/rcd-04-closeout-verification.md` の AC
  照合表に 1:1 記録）。
- 2026-11-16: drift 対象 6 file の frontmatter 確認 —
  `docs/tasks/rcd-04-task-{midi-undo-stableid,
  rt-engine-comp-pcm,task1-product-e2e}.md` が
  `status: "in-progress"`、`docs/impl/ir/rcd-04-{midi-undo-
  stableid,product-e2e,rt-engine-comp-pcm}-slice.md` が
  `status: "in-progress"`（いずれも merge 済み PR に対応）。
