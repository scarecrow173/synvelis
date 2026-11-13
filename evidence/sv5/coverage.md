# Context Inspector 受入条件 被覆台帳（Issue #5 / synvelis#5）

本ファイルは `docs/specs/context-inspector.md` の CI-01〜13 と
`docs/test-specs/context-inspector.md` の Rule に対する、**既存
Fixture/実装の被覆状況**を記録する。本 slice は設計定義のみで
あり、製品 View（#112/#108）・RT Probe（#107）・実機 Gate
（#114/#176, Ruviform #313/#314）の実装・受入は対象外。

凡例（POLICIES.md §3）:

- **Pass (headless)**: 既存の headless/OS-L1 Fixture が当該契約の
  不変条件を実際に検証済み。
- **Partial**: 契約の一部（状態機械・語彙・lease 等）は既存証拠
  あり、Inspector 固有の結合は未実装。
- **NotRun**: 設計のみ。実装・Fixture は後続 slice。
- **Deferred**: 実機/GUI Gate 待ち（NotRun と併記）。

| AC | 要求（要約） | 既存証拠（ファイル/テスト） | 設計契約の所在 | 判定 |
| --- | --- | --- | --- | --- |
| CI-01 | 6 TargetKind で共通 header + Provider、階層一貫 | 部分的: `signal_flow_ui.rs` の node kind 投影（TrackStrip/BusStrip/Processor + label/status）、`relation_ui.rs` の RelationView、`arrangement_ui.rs`/`viz_ui.rs` の read model、`screens.rs` Inspector tab（param readout）。統一 Context Inspector は無し | design §TargetKind/§情報階層、test-spec R1 | **NotRun**（Provider 未実装。read model 部品のみ既存） |
| CI-02 | Compact 折りたたみ / Expanded 複数カラム、View 独立 Section State | 部分的: `ViewStateRegistry` の view-local state 独立（`ws_a.rs::ws_a_01`）、`viewport` per-view。Section state・responsive layout は無し | design §レイアウト、test-spec R2 | **NotRun** |
| CI-03 | Arrangement 選択→Follow→Explore(Bus/Compressor/Sidechain)→戻る、編集選択/表示位置不変 | 強い部分証拠: `WorkspaceManager::inspect`/`explore_back` は editing selection・project・undo に触れない（`uic5.rs::uic5_02_inspect_is_not_edit`、`sf_05_s2.rs::signal_flow_inspect_is_independent_of_selection_and_arrangement_focus`）。link 受信は selection を書き換えない（`ws_a.rs::ws_a_02`）。対象 chain の Provider 遷移は未実装 | design §状態機械/§Scenario N1、test-spec Examples | **Partial**（不変条件は headless 証明済み、製品 scenario は NotRun） |
| CI-04 | A=Pinned(Bass), B=Follow(Drums)、別 Window 移動で保持 | 部分的: per-view `InspectionFocus` は view-local（`ws_a_01`）、mode/follow_source の retarget 規則（`uic5.rs::uic5_03_follow_source_scoping_and_modes`、`sf_05_s2.rs::signal_flow_link_delivery_retargets_only_follow_mode`）、detach/reattach・layout 復元で ViewState 保持（`ws_a.rs::ws_a_04*`、`crates/synvelis-app/tests/sv113_dock.rs`/`two_window.rs`）。2 Inspector 同時の製品 scenario は無し | design §状態機械/§Scenario N2、test-spec Examples | **Partial**（機構は証明済み、Inspector A/B 実装は NotRun） |
| CI-05 | Follow 元以外の Focus/Group event で探索解除しない。Follow 元の新選択で Explore→Follow | 部分的: `deliver()` は Explore/Pinned を retarget しない（`uic5_03_*`、`signal_flow_link_delivery_retargets_only_follow_mode`、`ws_a_02`）。**Explore→Follow 復帰規則はどの実装にも存在しない**（deliver は Follow mode のみ retarget）→ design D5 で Inspector 側消費規則として定義、実装は後続 | design D5・§状態機械、test-spec R3 | **Partial**（非解除側は証明済み、復帰側は設計のみ→実装時に検証） |
| CI-06 | 複数 Track 共通値/Mixed 正確表示、一括編集は発行時対象群を全件検証・無言の部分適用なし | 部分的: `ObjectSelections` は domain 別複数 target を保持（`selection.rs`）、command target は発行時確定（`ws_a.rs::ws_a_03`）。複数対象の property read model・一括編集 Command は無し | design §複数選択と一括編集、test-spec R5 | **NotRun**（Command/preflight 未実装） |
| CI-07 | 編集 Begin→非同期更新中に Follow 対象変化でも誤適用しない | 強い先例: `control_edge_begin` が endpoint+revision+window epoch を fixation、stale/epoch 変化で commit drop（`uic5.rs::uic5_02_commit_drops_on_stale_revision`/`uic5_02_commit_drops_on_window_epoch_change`/`uic5_02_focus_selection_change_never_retargets`）、routing gesture 同型（`sf_05_s2.rs::routing_gesture_stale_revision_drops_without_retarget`/`routing_gesture_window_transfer_drops_stale`）。Inspector 発の param 編集 Command は無し | design §非同期安全性、test-spec R4 | **Partial**（fixation/stale-drop 機構は証明済み、Inspector 経路は NotRun） |
| CI-08 | 2 Inspector で同一 Compressor Probe → 重複生成なし。片方 close でも明示 Monitor 維持 | **Pass (headless)**: `crates/synvelis-app/tests/sf05_probe_share.rs` — `two_views_share_one_probe_subscription`/`view_close_releases_only_own_lease`/`capture_is_owner_only_and_retreats_to_session_on_close`/`unsubscribe_releases_view_lease_and_fail_closed`。workspace 側 lease/by_view/`probe_release_subscriber`（`probe_trace.rs`）、player 側冪等 install（`synvelis-player/tests/sf04_probe_acceptance.rs::sf04_probe_same_key_request_is_idempotent_single_install`）。UI 側の 2 Inspector 表示は未実装 | design §Probe 連携、test-spec R6 | **Pass (headless)**（機構）。Inspector UI 配線は NotRun |
| CI-09 | Off/Monitor/Capture/Unavailable/Stale/Data Loss と実無音の識別 | **Pass (headless)**（語彙・状態機械）: `ProbeMode`/`ProbeResolveState`/`ProbeStatusKind`/`ProbeStopReason`/`ProbeUnobservedReason`（`synvelis-project/src/probe.rs`）、`sf04_s2_probe_trace_bridge.rs::sf04_s2_mode_off_maps_to_disabled_and_unrouted_record_returns_false`（Off が観測を出さない）、`sf04_s8` fault fixtures（QueueFull/LostBlocks/LostRecords/Unobserved）、`sf04_s7` compare 状態。`evidence/sv107/coverage.md` SF-PROBE-02/04/05/07/08。Inspector での表示は未実装 | design §観測値の分類、test-spec R6 | **Pass (headless)**（状態語彙）。表示側は NotRun |
| CI-10 | 同一 Processor の Main/Sidechain/Output 観測境界 + Diagnostic View 導線 | 部分的: `SignalBoundary`（`Input`/`Output`/`Sidechain`/`PreInsert`/`PostInsert`/`PreFader`/`PostFader`）と `SemanticProbePoint{entity,boundary,port}` が存在、`probe_resolution_*`/`map_resolution_states_*` で境界解決を検証済み、`syn.probe.compare`/`syn.probe.control`/`syn.probe.state` ops 存在（`sf04_probe_compare_op.rs`/`sf04_probe_wiring.rs`）。境界一覧 UI・Diagnostic View 遷移は無し | design §観測境界、test-spec R6 | **Partial**（境界語彙・解決・op は既存、Inspector UI は NotRun） |
| CI-11 | Pinned 対象削除時に未解決表示・自動切替しない | 強い先例: `uic5.rs::uic5_03_pinned_target_stays_unresolved_after_delete`（SemanticTarget 系）。probe entity 消失→`Unresolved` unbind・再 attach fresh bind（`sf04_probe_acceptance.rs::sf04_probe_removed_entity_unbinds_and_reattach_binds_fresh`）。ContextInspectorTarget 全 kind の unresolved 表示は未実装 | design D10・§状態機械、test-spec R3 | **Partial**（機構は証明済み、Inspector 実装は NotRun） |
| CI-12 | 再起動後 FollowSource/Pinned/Section 復元・Explore 初期化・不在 View/対象 UI | 部分的: `WorkspaceSnapshot` schema v3 + `ViewState` serde、`signal_flow`/`control_relation` の tolerant decode（malformed→`None`）、snapshot roundtrip テスト（`sf_05_s2.rs::capture_restore_roundtrips_signal_flow_state_per_view`/`snapshot_malformed_signal_flow_degrades_to_none`/`inspection_focus_type_roundtrips_through_snapshot`、`uic5.rs::uic5_control_relation_snapshot_roundtrip_and_degrade`）、preset は inspection_mode のみ保持し explore stacks を空にする（`preset.rs`）。`ContextInspectorViewState` は未定義 | design §永続化、test-spec R7 | **Partial**（永続化機構は証明済み、新 state/UI は NotRun） |
| CI-13 | Probe/Inspector 表示のみで Project 履歴・Bypass・Routing 不変 | **Pass (headless)**: `ws_a.rs::ws_a_05_layout_ops_leave_project_and_engine_alone`、`uic5_02_inspect_is_not_edit`、`relation_ui.rs`/`signal_flow_ui.rs` は read-only projection（mutation なし旨 module docs 明記）、`sf04_s2_probe_trace_bridge.rs::sf04_s2_mode_off_maps_to_disabled_and_unrouted_record_returns_false`。Inspector 固有 View は未実装だが不変条件クラスは証明済み | design §編集の安全性、test-spec R4 | **Pass (headless)**（不変条件）。製品 Inspector での再検証は後続 |

## ギャップ一覧（後続 slice への引き継ぎ）

1. `ContextInspectionTarget` union・`ContextInspectorViewState`・
   Section Provider・情報階層 UI — **未実装**（#112/#108）。
2. Explore→Follow 復帰規則のイベント消費経路 — link.rs の共有
   semantics は不変のまま、Inspector 側 subscribe/derive または
   additive delivery hook が必要（design D5）。
3. 複数選択の共通値/Mixed read model と all-or-nothing 一括編集
   Command — 未実装。
4. `SemanticTarget` が運べない kind（Processor/Parameter/
   Connection/Probe）への link 追従 — fail-closed + 明示 inspect
   導線で契約化済み、実装は後続。
5. 実 Window/Focus/IME/DPI/Plugin・実機 Probe — **Deferred**
   （#114/#176・Ruviform #313/#314・#107/#109 の Gate）。

## 検証実行記録（本 slice）

- `mise run fmt-check`: docs-only のため Rust 整形のみ確認（結果は
  PR 本文に記録）。
- docs index: `doc_maintenance.js index plan` → `index apply` で
  触れた dir を再生成（結果は PR 本文に記録）。
- 製品 AC の実行結果は全て `NotRun`/`Partial`/`Pass (headless)`
  の分類どおり。統合受入は merge 後対象 SHA の CI 証拠で確認する
  （POLICIES.md §5）。
