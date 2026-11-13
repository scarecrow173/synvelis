# sv11 Acceptance Coverage — Timeline Editing / Selection / Tool System

synvelis#11 の受入条件 TE-01〜14 と既存実装/テスト証拠の対応台帳。
spec `docs/specs/timeline-editing.md`・design
`docs/designs/timeline-editing-design.md`・test-spec
`docs/test-specs/timeline-editing.md` の TE-F/R/E/G・F-UX-01〜08 と対応
する。

状態語彙（POLICIES.md）: Requirement=`Required`/`Deferred`/
`NotApplicable`、実行結果=`Pass`/`Fail`/`NotRun`/`Unsupported`。
本台帳は **実行結果**を分類する。`Pass (headless)` は headless/
unit 契約の実証であり、製品 GUI・実機 Pass ではない。Pixel/OS 入力
と Headless の合格を混同しない。

## サマリ

- `Pass (headless)`: 既存 headless/unit テストが契約を実証。
- `Partial`: 一部経路・一部状態のみ実証（視覚面・実画面経路・
  literal 件数など残部は後続 Gate）。
- `NotRun`: 設計契約はあるが本 slice で未検証。
- `Deferred`: GUI/実機/実 Plugin の後続 Gate（#112/#114・EXT-L3・
  ruviform#243）。
- `Unsupported`: capability 非対応を明示。

## AC 別台帳

| AC | 要求 | 主な既存証拠（テスト/実装） | 状態 | 後続 Owner/Gate |
| --- | --- | --- | --- | --- |
| TE-01 Click/Drag 誤判定なし・Gesture 中 Tool 不切替 | Smart 優先順・fixation で操作種/対象固定 | `crates/synvelis-workspace/tests/ed_a.rs::ed_a_02_tool_resolution_and_fixation`（Smart 優先順 Handle>Body>LaneEmpty>Background・Dedicated 固定・Gesture 中 zone 移動で操作不変） | **Pass (headless)** | 実 px Drag Threshold・Cursor/Tooltip・Mouse/Pen/Touch 差は #112/#114（Deferred） |
| TE-02 極小/近接 Handle の編集意図確認・Alternative Tool | 曖昧 hit は候補+推奨 Tool を返す | `ed_a_02` の `Ambiguous { candidates, suggested_tools }` 経路（意味的 probe） | **Partial** | 実 Hit Size/px 閾値・Cursor/Tooltip/Alternative Tool UI は #112/#114・Issue #11 §9（Deferred） |
| TE-03 Time+Clip Selection 両立・Time-only Delete は Ripple しない | Object/Time/Lane Scope 分離・range 削除は独立 command | `ed_a_01_selection_model_separation`（View ごと独立・domain 別・Primary 明示・`clip.remove` は time selection を読まない）、`ed_a_01b_time_selection_and_range_delete`（Time Selection 単独で `clip.remove`=Unresolved・`clips.delete-range` 独立） | **Pass (headless)** | 実画面の Delete Menu/Feedback・OS modifier は #112/#114（Deferred） |
| TE-04 5 Clip 同時 Move で相対位置保持・1 Undo 全件復旧 | 多対象 Gesture = 全件検証・Atomic Commit・1 undo | `ed_a_04_preview_atomic_undo`（複数対象 Composite=1 revision=1 undo・ghost target で全体拒否・undo/redo 全件復旧）、`ed_b_01_media_multi_move_is_one_undo`（media domain）、`synvelis-project/tests/controller_store.rs`（atomic apply/stale revision） | **Pass (headless)** | 「5 件」の literal case は未個別実施 — Composite は件数非依存機構のため headless 契約で実証済み。実 Drag/Undo UX は #114（Deferred） |
| TE-05 Musical/Absolute 混在 Move/Snap で暗黙変換なし | 時間 Domain 明示・未宣言変換は拒否 | `ed_a_03_snap_grid_time_domain`（musical/media 混在 move 拒否・domain filter）、`ed_b_04_mixed_clip_move_ok_trim_rejects`・`ed_b_04_mixed_slip_uses_tempo_map`・`ed_b_04_abs_musical_move_ok_trim_rejects`・`ed_b_04_abs_musical_slip_rejects`・`ed_b_06_domain_mismatch_rejects_at_start`（probe/gesture domain 不整合の開始時拒否） | **Pass (headless)** | 完全な混合 Domain 安全演算契約は後続設計（design deferred）。実画面 Snap/Move は #114 |
| TE-06 Grid 非表示でも Snap 独立動作・Zoom で明示 Snap 分解能不変 | Grid/Snap 別設定・Explicit unit は zoom 不変・Auto のみ追従 | `ed_a_03_snap_grid_time_domain`（grid.visible=false で snap 動作・explicit unit の zoom 不変・auto のみ zoom 追従） | **Pass (headless)** | Snap Guide/設定 UI の実画面は #112/#114（Deferred） |
| TE-07 Snap Guide が候補・時間を説明・微小移動で振動しない | winner label で採用候補を説明・hysteresis で保持 | `ed_a_03_snap_grid_time_domain`（`SnapWinner { position, source, label }`・hysteresis で prev winner 保持・priority Marker>ClipEdge>Playhead>Grid） | **Partial** | Snap Guide の実描画・視認性・Threshold/Hysteresis 数値は #112/#114・Issue #11 §9（Deferred）。headless は label/hysteresis 機構のみ |
| TE-08 Drag 中 Esc/PointerCancel/Window 破棄で Project 不変 | cancel 経路で preview 破棄・revision/undo 不変 | `ed_a_04_preview_atomic_undo`（Escape/PointerCancel/InputScopeLost/remove_view→WindowClosed の各経路で revision 不変・gesture 破棄）、`uic5_02_preview_and_cancel_leave_project_untouched` | **Pass (headless)** | 実 Esc キー・実 Window close/破棄・PointerCancel OS 入力は #114（Deferred） |
| TE-09 Drag 中の他 Window 変更で誤 Target/Revision に適用しない | fixation revision で stale 拒否・retarget なし | `ed_a_04`（gesture 中 `SetProjectName` で revision 前進→`StaleRevision` 拒否・project 不変）、`ws_a_03_command_routing`（explicit target/stale/kind mismatch）、`uic5_02_commit_drops_on_stale_revision`・`uic5_02_commit_drops_on_window_epoch_change`、`uic5_04_foreign_change_conflicts_then_explicit_rebase_pending`、`ed_a_05`（link event が fixation を変えない） | **Pass (headless)** | 実 2 Window 同時編集・実 window epoch 変化は #114（Deferred） |
| TE-10 Piano Roll Linked Source 編集→共有 Clip 反映・Arrangement Selection 不変 | EditBinding は表示 binding・Source 共有編集・selection 非干渉 | `synvelis-project/tests/midi_source.rs::linked_clips_share_source_edits_and_detach_is_explicit`（S1 への note 編集が両 clip に反映・`DetachClipSource` で明示 independent）、`midi_source.rs::undo_redo_cover_note_edits_and_detach`、`ed_a_05_linked_context_separation`（`open_in_editor` が selection/command target を変えない） | **Partial** | 実 Piano Roll 画面・Note 編集 gesture→EditBinding の製品経路は #112/#114（Deferred）。project 層の linked source 共有は実証済み |
| TE-11 Inspector/Signal Flow/Version Diff の Explore/Highlight が Edit Command を上書きしない | Linked Context/Inspection Focus は非破壊・read-only view は mutate しない | `ed_a_05_linked_context_separation`（gesture 中 link event が fixation/selection を変えない・binding は command target に解決されない）、`uic5_02_inspect_is_not_edit`、`uic5_03_*`（follow/explore/pinned）、`rel_c5.rs`（relation 投影）、`vcs04_history.rs`（diff/選択 display-only） | **Pass (headless)** | 実 Inspector/Signal Flow/Version Diff の 3 View 同時 GUI は #102/#112/#114（Deferred） |
| TE-12 Read-only VCS Snapshot 選択で Live Project 不変 | 過去 commit 選択/Highlight は read-only・Apply は明示 Command | `vcs04_history.rs::view_mode_and_clear_selection_are_display_only`・`vcs_ops_never_touch_project_engine_or_transport`・`live_endpoint_goes_stale_when_revision_advances`（endpoint stale 検出）・`uic5_02_inspect_is_not_edit` | **Pass (headless)** | 実 VCS Diff UI・Apply Selected Change の製品 Command 経路は #95/#112/#114（Deferred） |
| TE-13 Remote Asset Drag&Drop 失敗で不完全 Clip/Manifest を残さない | Fetch+Validate 完了まで Clip/Manifest を確定しない | `crates/synvelis-library/tests/import.rs`（`import_streams_verifies_publishes_and_commits`・`digest_mismatch_is_integrity_error`・`publish_failure_resumes_from_manifest`・`missing_lane_target_is_rejected_without_clip`・`wrong_track_lane_target_is_rejected`・`incomplete_job_and_blob_are_protected_from_gc`・`byte_budget_is_enforced_during_staging`）、`import_recovery.rs` | **Partial** | 実 OS Drag&Drop→Timeline 取込の製品経路は #9 系実装+#112/#114（Deferred）。import pipeline の検証境界は実証済み |
| TE-14 再生中も Gesture Preview/履歴操作が RT Audio Callback をブロックしない | Preview/履歴は非 RT 側・transport/revision/undo を不変に保つ | `workspace_model.rs::playback_dock_window_churn_preserves_transport_revision_and_history`（playing 中 churn で transport/revision/graph_builds/undo/view state 不変）、`ws_a_05_layout_ops_leave_project_and_engine_alone`、`aut_04_rt.rs::record_event_rt_alloc_free_bounded_and_typed`（RT alloc-free bounded） | **Partial** | 実 RT Audio Callback の非ブロック計測・実 playback 中の実機 gesture は EXT-L3/#114（Deferred）。headless は非 mutation 契約のみ |

## 集計

| 状態 | 件数 | AC |
| --- | --- | --- |
| Pass (headless) | 9 | TE-01, TE-03, TE-04, TE-05, TE-06, TE-08, TE-09, TE-11, TE-12 |
| Partial | 5 | TE-02, TE-07, TE-10, TE-13, TE-14 |
| NotRun | 0 | — |
| Deferred | （上記 Pass/Partial の GUI・実機面を含む） | #112/#114・EXT-L3・ruviform#243 |
| Unsupported | 0 | — |

## 証拠の参照先

- `crates/synvelis-workspace/tests/`: `ed_a.rs`（ED-A-01..05:
  selection/tool/snap/gesture-undo/linked-context）、`ed_b.rs`
  （ED-B-01..07: media domain move/trim/slip/marquee・混在
  domain）、`ws_a.rs`（view/selection/linkgroup/command/dock）、
  `workspace_model.rs`（playback churn）、`uic5.rs`/`uic5_04.rs`
  （inspect/focus/stale/foreign change）、`vcs04_history.rs`
  （read-only history/diff）、`aut_04_rt.rs`（RT alloc-free）。
- `crates/synvelis-project/tests/`: `midi_source.rs`（linked
  source 共有・detach・note 編集 undo/stale）、`md_a.rs`
  （ClipPlacement/Playback・undo）、`controller_store.rs`
  （atomic apply/stale revision/undo-redo）。
- `crates/synvelis-library/tests/`: `import.rs`・`import_recovery.rs`
  （remote asset verify/publish/失敗 resume/target 拒否）。

## 未被覆・後続へ委譲

- 製品 Arrangement/Piano Roll/Audio Editor/Automation Lane の実
  画面編集・Snap Guide 描画・Tool Cursor/Tooltip → #112（実装）/
  #114（受入）・ruviform#243。
- 実 OS 入力（Esc/PointerCancel/Window 破棄/2 Window 実競合/
  IME/Keyboard）→ #114。
- 実 RT Audio Callback 非ブロック計測・実 playback 中 gesture →
  EXT-L3/#114。
- 実 OS Drag&Drop→Timeline 取込 → #9 系実装 + #112/#114。
- 数値閾値（px Hit Size・Snap Threshold/Hysteresis・OS keybind
  既定）→ Issue #11 §9 の PoC 後決定事項。
- Take/Comp Editor UI 詳細 → #12 §14/§15 の後続設計。
