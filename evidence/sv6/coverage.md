# sv6 Acceptance Coverage — Automation / Modulation / Parameter Control

synvelis#6 の受入条件 AM-01〜13 と既存実装/テスト証拠の対応台帳。
spec `docs/specs/automation-modulation.md`・design
`docs/designs/automation-modulation-design.md`・test-spec
`docs/test-specs/automation-modulation.md` の AM-F/R/E/G と対応する。

状態語彙（POLICIES.md）: Requirement=`Required`/`Deferred`/
`NotApplicable`、実行結果=`Pass`/`Fail`/`NotRun`/`Unsupported`。
本台帳は **実行結果**を分類する。`Pass (headless)` は headless/
unit 契約の実証であり、製品 GUI・実機 Pass ではない。

## サマリ

- `Pass (headless)`: 既存 headless/unit テストが契約を実証。
- `Partial`: 一部経路・一部状態のみ実証。
- `NotRun`: 設計契約はあるが本 slice で未検証（製品 UI 等）。
- `Deferred`: GUI/実機/実 Plugin の後続 Gate（#102/#114/#103）。
- `Unsupported`: capability 非対応を明示。

## AC 別台帳

| AC | 要求 | 主な既存証拠（テスト/実装） | 状態 | 後続 Owner/Gate |
| --- | --- | --- | --- | --- |
| AM-01 Scope/時間座標分離 | Track/Clip/Note/Project の owner×time_scope が混同されない | `pc_a.rs::pc_a_02_scope_separation_clip_follows_move`、`pc_b.rs::pc_b_02_*`（note scope/clip anchor/cascade）、`ctl_01.rs`、`ctl_02.rs`（ParameterTargetId 正本） | **Pass (headless)** | 製品 Arrangement での 3 種別同時表示は #102/#114（Deferred） |
| AM-02 競合 Automation 明示解決 | 同一 Target・同一評価時刻の競合を暗黙優先で潰さない | `pc_a.rs::pc_a_03_conflict_reject_and_replace`、`aut_02.rs`（region replace 境界/curve 保存）、`aut_03.rs::aut_03_replace_collision_preserves_outside_curve` | **Pass (headless)** | UI での conflict 解決操作は #102（Deferred） |
| AM-03 Off=Base・Modulation 維持・Lane 非表示≠停止 | Automation Off は Base 復帰、Modulation 別維持、Lane hide は View 状態 | `pc_a.rs::pc_a_01_single_source_three_projections`、`pc_a_05_mode_select_never_mutates_and_off_read_behave`、`aut_04.rs::automation_view_is_read_only_and_never_mutates_source` | **Pass (headless)** | 製品 Lane hide 操作の GUI 確認は #114（Deferred） |
| AM-04 複数 Assignment 加算・Depth/Polarity/Transform・Clamp 説明 | 順序独立の加算、Assignment 単位パラメータ、Clamped/Saturated 説明 | `pc_a.rs::pc_a_06_cycle_rejected_and_order_deterministic`（順序決定性）、`pc_a_07_capability_honesty`、`pc_c2_relation.rs`/`rel_c5.rs`（depth edge）、`evaluate_control`/`ControlBreakdown` | **Pass (headless)** | 個別 Depth/Polarity/Transform の製品 UI 操作は #102（Deferred）。非線形表示（Depth%≠Hz）は #102 |
| AM-05 非線形/離散/非対応・循環検証 | 誤値・機能保証をしない、cycle を拒否 | `pc_a.rs::pc_a_07_capability_honesty`、`pc_a_06_cycle_rejected_and_order_deterministic`、`pc_c4_binding.rs::pc_c4_02_typed_rejects_no_misdelivery`、`pc_c4_03_composite_control_and_gates` | **Pass (headless)** | 実 Plugin での capability 宣言検証は EXT-L3/#101/#103（Deferred） |
| AM-06 Breakdown 出所付き 7 層分離 | Base/Automation/Reference/Source 寄与/Host-resolved/Plugin-reported/Probe 実測を区別 | `pc_a.rs::pc_a_04_breakdown_and_observation_states`、`trace_c3.rs::trace_c3_01_*`（stage slots/receipt generation）、`pc_c4_03_observation.rs`、`sf04_*` | **Partial** | 製品 Control Breakdown UI の 7 行表示は #102（Deferred）。Plugin-reported/Probe の実機供給は #101/#107 EXT-L3 |
| AM-07 At Playhead/Live/Captured/Stale/Unavailable + 時刻/Latency/Revision | 観測状態を区別し鮮度・世代を参照可能に | `trace_c3.rs::trace_c3_02_context_mismatch_is_stale`/`trace_c3_02_stamp_quality_classification`、`pc_c4_07_transport_observe.rs`、`sf04_s2_probe_trace.rs`/`sf04_s3_*` | **Pass (headless)** | 3 View での鮮度表示 UI は #102/#114（Deferred） |
| AM-08 Read/Touch/Latch/Write 状態機械 | Read=一時優先→復帰、Touch=区間記録→復帰、Latch=継続だが次回へ持ち越さない | `aut_01.rs`（13 tests: read_override/touch_writes/latch_holds/write_seeds/boundaries/fixation）、`pc_c6_oracle_rec.rs::pc_c6_r07_mode_gating_oracle`、`pc_a_05_*` | **Pass (headless)** | 実 Window ジェスチャ（マウス/ペン）での mode 遷移は #114（Deferred） |
| AM-09 Write Arm+Record+Transport ゲート | 明示 Arm+Record+Transport を満たす Target のみ、非対象/未 Arm へ書かない | `pc_a.rs::pc_a_05_recording_gates_undo_seal_fixation`/`pc_a_05_record_start_rejects_unbound_write_lane`/`pc_a_05_record_write_rejects_removed_write_lane`、`pc_c6_oracle_rec.rs::pc_c6_r01/r07`、`pc_c1.rs::rec_c1_05_lifecycle_gates`、`aut_03.rs` | **Pass (headless)** | 製品 Write Arm UI・実機 Transport 条件は #102/#114（Deferred） |
| AM-10 二重 Modulation 防止・1 セッション=1 Undo | 記録は pre-modulation Manual/Reference、1 Undo 単位 | `pc_c1.rs::rec_c1_01_commit_success_one_undo_raw_manual_save_separate`、`pc_c6_oracle_rec.rs::pc_c6_r04_commit_one_undo_exactly_once`、`aut_03.rs::aut_03_multi_lane_pass_is_one_undo`、`aut_01.rs::aut_01_touch_return_is_nondestructive` | **Pass (headless)** | LFO 動作中の実機記録での二重適用なし確認は EXT-L3/#103（Deferred） |
| AM-11 Gesture fixation・Follow/Window 移動で retarget しない | Begin 時に Target/Revision/Arm 固定、stale は明示 reject | `uic5.rs::uic5_02_focus_selection_change_never_retargets`/`uic5_02_commit_drops_on_window_epoch_change`/`uic5_02_commit_drops_on_stale_revision`、`aut_01.rs::aut_01_fixation_captures_begin_context`、`aut_03.rs::aut_03_fixation_survives_mode_and_arm_drift`、`pc_c6_oracle_ui.rs::pc_c6_ui04_two_window_epoch_and_cancel` | **Pass (headless)** | 実 Window 移動・Focus 変更を伴う GUI ジェスチャは #114（Deferred） |
| AM-12 Loop/Tempo/Offline/再生の安定評価・精度明示 | Musical 座標で安定、RT/Offline parity、精度/制限明示、loop wrap で seal | `aut_03.rs`（wrap_seals/pass_advances/loop_change_seals 等 19 tests）、`pc_a.rs::pc_a_08_time_consistency`、`pc_b.rs`、`pc_c4_binding.rs::pc_c4_01_eval_dispatch_receipt_rt_offline_parity`、TIM-01/02 系 | **Pass (headless)** | 実 Plugin・実 device での精度/Offline Exact 検証は EXT-L3/#101/#103（Deferred）。Tempo/Loop 数値は TIM-01/02・#19/#20 |
| AM-13 3 View 整合・調査クリック非破壊 | Signal Flow/Inspector/Arrangement が同一 Relation/Trace を投影、調査は Editing Selection を変えない | `rel_c5.rs`（relation 投影/coverage/stale）、`uic5.rs::uic5_02_inspect_is_not_edit`/`uic5_03_*`、`pc_c6_oracle_ui.rs::pc_c6_ui01/02/03`、`sf_05.rs`/`sf_05_s2.rs` | **Partial** | 製品 3 View 同時表示・実 Window での調査/編集分離は #102/#114（Deferred） |

## 集計

| 状態 | 件数 | AC |
| --- | --- | --- |
| Pass (headless) | 11 | AM-01,02,03,04,05,07,08,09,10,11,12 |
| Partial | 2 | AM-06,13（内訳は各行参照。AM-04 の UI 操作面も Deferred） |
| NotRun | 0 | — |
| Deferred | （上記 Partial/Pass の GUI・実機面を含む） | #102/#114/#103・EXT-L3 |
| Unsupported | 0 | — |

注: `Pass (headless)` の AC も、**製品 GUI・実機での同等動作は
別 Gate（#102/#114/#103）で `Deferred`**。本台帳の Pass は
headless/unit 契約の実証のみを意味する。

## 証拠の参照先

- `crates/synvelis-project/tests/`: `ctl_01.rs`/`ctl_02.rs`、
  `aut_02.rs`、`pc_b_rt.rs`、`pc_c2_relation.rs`、
  `pc_c6_oracle_rel.rs`/`pc_c6_oracle_tr.rs`、`trace_c3.rs`、
  `sf04_project_probe.rs`。
- `crates/synvelis-workspace/tests/`: `pc_a.rs`（PC-A の
  model/eval/breakdown/mode/capability/time 契約）、`pc_b.rs`、
  `pc_c1.rs`、`aut_01.rs`/`aut_03.rs`/`aut_04.rs`/`aut_04_rt.rs`、
  `uic5.rs`/`uic5_04.rs`、`rel_c5.rs`、`sf_05.rs`/`sf_05_s2.rs`、
  `pc_c6_oracle_rec.rs`/`_ui.rs`、`sv47_controls.rs`/`sv48_viz.rs`。
- `crates/synvelis-player/tests/`: `pc_c4_binding.rs`、
  `pc_c4_03_observation.rs`、`pc_c4_05/07_*`、`pc_c6_oracle_pl*.rs`、
  `sf04_*`。

## 未被覆・後続へ委譲

- 製品 Automation Lane・Control Breakdown・Signal Flow の実
  Window 操作と 3 View 同時整合 → #102（UI-C5）/#114（SV-GUI-05）。
- 実 VST3/AU/CLAP・実 device・native dock・実機 precision →
  #101/#103/#107（EXT-L3）。
- Cross-gate Oracle（RT/Plugin/GUI を跨ぐ統合受入）→ #103。
- 数値閾値（Touch Return・LOD・精度）→ PoC 後決定事項。
