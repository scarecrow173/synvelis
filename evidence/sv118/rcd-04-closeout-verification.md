# Evidence — synvelis#118 RCD-04 residual closeout: AC 照合・canonical lane 実測・gate 棚卸し・簿記 drift 修正

Issue #118（RCD-04 tracker — Comp Transition・MIDI Phrase・
Recovery Review の製品 E2E）の residual closeout 証拠。
headless 実装は merge 済み PR #219/#300/#319/#374 で充足済み
であり、本 slice は docs+evidence only の簿記である。
外部 gate（実 GUI・consumer・MIDI 永続化・実 device/実 RT）
は NotRun/Deferred のまま正直に記録し、headless の PASS を
昇格しない。Issue #118 は auto-close しない（open 維持）。

## Environment (all rows unless noted)

| Field | Value |
| --- | --- |
| Synvelis branch | `feat/118-rcd04-closeout` |
| Base commit | `0b57299edb5394d0358e5f9dbf5665df540ca3fc` (= `origin/main` at dispatch; docs+evidence changes are uncommitted at measure time — code surface identical to base) |
| OS | Windows (MINGW64/MSYS, win32), NTFS |
| Toolchain | rustc 1.99.1 (58cf8f9c3 2026-10-13) / cargo 1.99.1 via mise; MSRV lane (rust 1.89) is Daily Full CI delegated — NotRun |
| Change surface | docs/evidence only — zero `src/`, test, fixture, manifest, `Cargo.lock`, or `docs/**/README.md` changes |
| Prior evidence consolidated (not re-derived) | merge 済み PR #219 (`9798be5`)・#300 (`a030e1f`)・#319 (`ff33ffb`)・#374 (`4946a59`) と既存 RCD-04 test/ledger |

## Canonical lane (mise tasks)

実測は本 worktree の現在コード状態（= base `0b57299` +
docs-only 変更）で `mise run`/`mise exec` 経由にて実施。
実行した lane のみ Pass と記録する。

| Command | Expected | Observed | Status |
| --- | --- | --- | --- |
| `mise run fmt-check` | exit 0 | exit 0 | Pass |
| `mise exec -- prek run --all-files` | exit 0 | exit 0 (all hooks Passed) | Pass |
| `mise run test synvelis-project` | exit 0 | exit 0 — 600 tests passed / 0 failed across 54 test binaries; `rcd_04_comp_cas` 6・`rcd_04_midi_phrase` 11・`rcd_04_vcs_stable_id` 4 全 pass | Pass |
| `mise run test synvelis-recording` | exit 0 | exit 0 — 216 tests passed / 0 failed across 22 test binaries; `rcd_04_recovery_partial` 4・`rcd_04_take_comp_pcm` 1 全 pass | Pass |
| `mise run test synvelis-player` | exit 0 | exit 0 — 343 tests passed / 0 failed across 49 test binaries; `rcd_04_midi_phrase_boundary` 2 pass | Pass |
| `mise run test synvelis-render` | exit 0 | exit 0 — 90 tests passed / 0 failed across 11 test binaries; `rcd_04_rt_comp_pcm` 2 pass | Pass |
| `mise run test-rt` | exit 0 (`rt_` filter) | exit 0 — 130 `rt_`-matching tests passed / 0 failed across workspace test binaries | Pass |
| `mise run test synvelis-app` (opt-in) | exit 0 | exit 0 — 355 tests passed / 0 failed; `rcd_04_product_e2e` 5 tests 全 pass（`rcd04_recovery_review_product_e2e`・`rcd04_recovery_corrupt_and_discard_guards_product_e2e`・`rcd04_stale_gesture_and_readonly_history_product_e2e`・`rcd04_comp_transition_product_e2e`・`rcd04_midi_phrase_product_e2e`） | Pass |
| `mise run check` / `lint` / `build` / `test`（workspace 全件）/ `test-msrv` / `*-full` / `check-app-native` / `lint-app-native` | — | not run in this slice（docs+evidence only; `*-msrv`/`*-full` は Daily Full CI 委譲） | NotRun |
| `mise run test-recording-real-fs` | — | not run（本 slice の必須 lane リストに非該当 — RCD-02/real-FS durability scope であり RCD-04 AC の対象外） | NotRun |
| docs index 再生成・`doc_status` audit | — | not run（Coordinator が統合時に実施 — worker spec で明示的に対象外） | NotRun |

## RCD-04 test 棚卸し（実在 `#[test]` 数 — `rg`/列挙で確認）

| Binary | Tests | AC 対応 |
| --- | --- | --- |
| `crates/synvelis-project/tests/rcd_04_comp_cas.rs` | 6 | AC-3 CAS・AC-4 raw/Asset 不変・AC-2 stable-ID (Undo/Redo/Save/Reload/Diff) |
| `crates/synvelis-project/tests/rcd_04_midi_phrase.rs` | 11 | AC-2 MIDI phrase 境界・Hanging Note/Controller Leak・native tick |
| `crates/synvelis-project/tests/rcd_04_vcs_stable_id.rs` | 4 | AC-2 VCS stable-ID・apply fail-closed |
| `crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs` | 2 | AC-2 実 engine 配送経路 boundary |
| `crates/synvelis-recording/tests/rcd_04_recovery_partial.rs` | 4 | AC-3 Recovery partial/cancel/retry/corrupt fail-closed |
| `crates/synvelis-recording/tests/rcd_04_take_comp_pcm.rs` | 1 | AC-1 RT vs Offline PCM fixture |
| `crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs` | 2 | AC-1 RT engine comp PCM 3 経路 bit-exact + fail-closed |
| `crates/synvelis-app/tests/rcd_04_product_e2e.rs` | 5 | AC-2/AC-3 product E2E（`syn.*` op dispatch、headless） |

## Issue #118 受入条件 — 1:1 照合

Issue 本文「受入条件」4 行の原文を主 ledger とし、各行を
実在 test 名・merge 済み PR 記録・実測 lane に対応付ける。
root 証拠は既存 test/merge 記録の引用に限定し、新規
test/fixture は作らない。

| # | Issue line (原文) | Evidence | Status |
| --- | --- | --- | --- |
| AC-1 | Source Handle範囲/Fade区間/相関素材/Transient/Beat Anchor/Tempo/LoopでRT PCMとOffline PCMの差を記録。 | `crates/synvelis-recording/tests/rcd_04_take_comp_pcm.rs::rcd04_comp_pcm_rt_vs_offline_fixture`（PR #219）・`crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs::rcd_04_rt_engine_comp_pcm_matches_offline_export` + `rcd_04_rt_engine_unsupported_mapping_fails_closed`（PR #319、HeadlessDriver/RoutingEngine/export 3 経路 bit-exact + `UnsupportedFormatOrTimeMap`/`UnrenderableComp` fail-closed 分類）。headless deterministic fixture で記録済み | Pass (headless) / 実 RT thread・実 device 計測 = NotRun（実機 gate — 下記棚卸し） |
| AC-2 | MIDI phrase境界でHanging Note/Controller Leakなし、Undo/Redo、Save/Reload、VCS DiffでStable ID維持。 | `crates/synvelis-project/tests/rcd_04_midi_phrase.rs` 11 tests（`midi_phrase_plan_has_no_hanging_notes_or_leaks`・`midi_phrase_chase_covers_kind_union_and_mid_phrase_events_play`・`midi_phrase_stable_ids_survive_undo_redo_save_reload`・`midi_phrase_preserves_native_ticks_without_requantize` 等、PR #219）・`crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs` 2 tests（`rcd_04_boundary_lowers_to_exact_wire_stream`・`rcd_04_boundary_driver_delivers_without_hanging_or_duplicate_voices`、実 engine 配送経路、PR #374）・`crates/synvelis-project/tests/rcd_04_vcs_stable_id.rs` 4 tests（`comp_stable_ids_survive_save_reload_and_diff`・`comp_transition_diff_is_expressed_by_its_own_stable_id`・`apply_selected_fails_closed_on_comp_transition`・`comp_stable_ids_survive_restore_as_copy`、PR #374）・`rcd_04_comp_cas.rs::comp_ids_survive_undo_redo_save_reload_and_diff`（PR #219） | Pass (headless) / 実 GUI での同条件照合 = Deferred（synvelis#114） |
| AC-3 | Recovery partial+cancel+retry、Group Comp全体CAS、2Window Stale Gesture/History view read-onlyを実GUI＋headlessで検証。 | headless 分: `crates/synvelis-recording/tests/rcd_04_recovery_partial.rs` 4 tests（`rcd04_partial_hold_cancel_then_adopt_retry`・`rcd04_partial_discard_requires_clear_probe`・`rcd04_group_commit_separates_partial_and_held_streams`・`rcd04_corrupt_journal_is_fail_closed`、PR #219）・`rcd_04_comp_cas.rs::comp_group_cas_rejects_stale_revision_without_partial_apply`（PR #219）・`crates/synvelis-app/tests/rcd_04_product_e2e.rs` 5 tests（`rcd04_recovery_review_product_e2e`・`rcd04_recovery_corrupt_and_discard_guards_product_e2e`・`rcd04_stale_gesture_and_readonly_history_product_e2e`・`rcd04_comp_transition_product_e2e`・`rcd04_midi_phrase_product_e2e`、PR #300、`syn.*` op dispatch 経路） | Pass (headless) / 実 GUI・実 OS 入力分 = NotRun/Deferred（synvelis#114） |
| AC-4 | 既存MD-A/Bの回帰、raw PCM/Asset不変、AudioLens native f64非回帰。 | `rcd_04_comp_cas.rs` の `comp_ops_never_mutate_takes_sources_or_assets`・`project_comp_is_deterministic_and_preserves_provenance`・`comp_switch_is_single_cas_undo_and_leaves_raw_immutable`（PR #219）+ 上記 canonical lane（synvelis-project/recording/player/render/app）の全体 pass で MD-A/B 回帰・raw PCM/Asset 不変を実測。AudioLens native f64 consumer 照合は別 gate | Pass (MD-A/B 回帰 + raw/Asset 不変の Synvelis 側分) / AudioLens native f64 = NotRun/Deferred（`scarecrow173/ruviform#294`） |

## Slice 受入条件（Coordinator 確定）— 簿記照合

| # | Slice 条件 | Evidence | Status |
| --- | --- | --- | --- |
| S-1 | Doc chain 7 files（`rcd-04-residual-closeout` slug）+ `docs/impl/exp/*-exp.jsonl` + `docs/discovery/` memo | `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/rcd-04-residual-closeout*.md`（7 file）+ `docs/impl/exp/rcd-04-residual-closeout-exp.jsonl` + `docs/discovery/rcd-04-residual-closeout.md` 作成。`docs/**/README.md` index は一切再生成・編集していない（Coordinator が統合時に再生成 — 未同期は許容済み） | Pass |
| S-2 | `evidence/sv118/rcd-04-closeout-verification.md` に AC 1:1 map・lane 実測・gate 棚卸し | 本ファイル | Pass |
| S-3 | merge 済み 3 task file status `in-progress`→`done` + 3 impl/ir `in-progress`→`completed` + stale checkbox 事実整合 | `docs/tasks/rcd-04-task-{midi-undo-stableid,rt-engine-comp-pcm,task1-product-e2e}.md`・`docs/impl/ir/rcd-04-{midi-undo-stableid,product-e2e,rt-engine-comp-pcm}-slice.md` の frontmatter/checkbox を修正（merge PR 番号・本 evidence 参照を注記。当時の作業記録本文は不変） | Pass |
| S-4 | review-protocol proportionate・blocker 0 | docs+evidence 小 slice の self-review — PR 本文に記録 | Pass |
| S-5 | honesty: 未測定 lane を Pass と書かない | 本ファイルの lane 表・gate 棚卸しは実行済み lane のみ Pass | Pass |

## 残存外部 gate (NotRun/Deferred — 所有 Issue 記名)

| Gate | 内容 | Owner | Status |
| --- | --- | --- | --- |
| 実 GUI・実 Window（Take/Comp/Recovery Review の製品 GPUI Scenario・2Window Stale Gesture/History view read-only の実 OS 入力検証） | headless/`syn.*` dispatch のみ実測済み。実 GPUI window・実 OS 入力は未検証 | synvelis#114 | Deferred |
| AudioLens native f64 consumer 照合（raw-recording 非回帰の consumer 側） | Synvelis 側は raw PCM/Asset 不変を pin 済み。consumer 接続・f64 精度比較は別 repo | `scarecrow173/ruviform#294` | NotRun |
| MIDI comp selection の project 永続化・native-tick/chase 意味論 | plan 評価層のみ・永続化未確定 | synvelis#126 | Deferred |
| project 側 HEAD CAS（staged HEAD/Publish/project-side HEAD CAS） | comp 適用の CAS は headless で pin 済み。project HEAD 進化は別 gate | synvelis#84 | Deferred |
| VCS/history 関連（comp kind の VCS-05 apply whitelist 拡張） | 現行 `ApplyError::Unsupported` fail-closed が契約で `apply_selected_fails_closed_on_comp_transition` に pin 済み。whitelist 拡張は typed entity extraction/transplant/membership reconcile を含む別 slice | 別 slice（対象 Issue 起票・依存登録は Coordinator 側） | Deferred |
| 実 device/実 RT thread 計測（Source Handle/Fade/相関素材等の RT vs Offline PCM 差の実機記録） | headless deterministic fixture のみ。実機音質保証ではない | 実機試験 | NotRun |
| 実 OS・真の power-loss/実 fault 注入 | headless/fixture のみ | 実機試験 | NotRun |
| 統合/app 層 acceptance・実 GPUI/audio device | app 層 headless E2E は PR #300 で実測済み。実 device/GUI は上記 gate | synvelis#97・#114 | Deferred |
| `*-msrv`・`*-full` 系 lane | Daily Full CI 委譲（ユーザー指定により本 slice では実行しない） | Daily Full CI | NotRun |
| `test-recording-real-fs`（opt-in real NTFS lane） | 本 slice の必須 lane リストに非該当（RCD-02/real-FS durability scope） | — | NotRun |
| 統合 CI 証拠 | merge 後 main SHA の CI を Coordinator が確認 | Coordinator | Deferred |
| docs index 再生成・`doc_status` audit | Coordinator が統合時に実施 | Coordinator | Deferred |

headless fixture・fake driver・offline renderer・`syn.*`
op dispatch の PASS は上記 gate の証拠に昇格しない。他
consumer（AudioLens 等）の結果を Synvelis の Pass に流用
しない。

## Doc 監査 (実走結果)

- `mise exec -- prek run --all-files`: exit 0（全 hook
  Passed）。新規 doc chain・evidence・drift 修正 file を含む
  全 file が hook を通過。
- `mise run fmt-check`: exit 0。
- docs index 再生成・`doc_maintenance.js index apply`・
  `doc_status` audit は本 slice では**実行していない**
  （Coordinator が統合時に実施 — worker spec で明示的に
  対象外。index 未同期は許容済み）。RCD-02 先例
  （`evidence/sv116/rcd-02-closeout-verification.md`）では
  index regen を実施したが、本 slice の契約は異なり index
  非接触が正しい。

## 再現手順

```sh
# worktree root (G:/orca/workspaces/synvelis/sv118-rcd04-closeout)
mise install
mise run fmt-check        # cargo fmt --all -- --check
mise exec -- prek run --all-files
mise run test synvelis-project
mise run test synvelis-recording
mise run test synvelis-player
mise run test synvelis-render
mise run test-rt          # cargo test --workspace --locked rt_
mise run test synvelis-app   # opt-in: rcd_04_product_e2e の所在 package
```

照合に使った列挙コマンド（補助証拠）:

```sh
grep -c "#\[test\]" crates/synvelis-project/tests/rcd_04_{comp_cas,midi_phrase,vcs_stable_id}.rs \
  crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs \
  crates/synvelis-recording/tests/rcd_04_{recovery_partial,take_comp_pcm}.rs \
  crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs \
  crates/synvelis-app/tests/rcd_04_product_e2e.rs
git log --oneline --grep "rcd.04\|RCD-04"  # merge PR #219/#300/#319/#374 の確認
```
