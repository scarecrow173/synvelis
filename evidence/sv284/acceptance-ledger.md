# Acceptance Ledger — synvelis#284 CB-07 Arrangement headless スライス

Date: 2026-11-15. Slice: CB-07 `VariantScope::Arrangement`
実ドメイン Adapter（`ArrangementAdapter`）+ range 導出
ownership・`ArrangementDomainMask` 編集面 gating・境界依存
typed inventory・in-range 削除の arrangement removal policy・
headless fixture/回帰テスト。残ドメイン（CB-08 = #285 比較
orchestration）・GUI timeline（#114）・cross-gate（#288）・
scope-removal の adopt/render materialization・実機受入は
DEFER。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/284
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | scarecrow173/sv284-cb07-arrangement-adapter HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 / Intel Core i9-9900K |
| Toolchain | mise 管理 Rust 1.99.1（`mise run` 経由） |
| Project schema | v14（schema 非変更 — 永続 model への entity 追加なし） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-project/tests/
  cb07_arrangement.rs` が `mise run test synvelis-project`
  で green。
- **NotRun**: 実 OS/実 Plugin/実 RT/実 Render/実デバイス
  入力 pipeline 依存の判定 — 本スライスでは実行していない。
- **Deferred**: 後続 Issue/スライスの範囲として明示的に送った
  部分。
- **Unsupported**: project 表現または engine surface が存在
  せず、型付き拒否が正しい verdict のもの。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| CB-ARR-01 | Pass (headless) | `cb_arr_01_candidate_edits_roundtrip_save_reload_and_isolate` — 2 Track×複数 clip の add/move/extent/mute/playback + clip-local note add/remove を含む候補が fork→edit→save→load で round-trip し、live・sibling branch・foreign clip/note は不変。`cb_arr_08_mask_gating_clips_only_and_notes_only` — mask 宣言面のみが編集可能。`cb_arr_09_move_out_and_sweep_containment_reject_typed` — in-range `RemoveClipsInRange` が clip 削除+note cascade を受理し、`clip_a_out`/`clip_a_abs`/`clip_b_in`/`note_foreign` は温存。`cb_arr_10_split_clip_scope_membership` — 両半分が scope 内に収まる `SplitClip` は受理・adopt で新 clip が minted id で materialize。`cb_arr_13_split_clip_notes_mask_gate` — `clips`-only mask で split 点以降の note 移動を起こす `SplitClip` は `Unsupported`、full mask では受理され adopt で note が新 clip へ再親付けされる。`cb_arr_11_delete_candidate_editable_but_adopt_defers_typed` — `RemoveClip`/`RemoveNote` の削除候補が編集・diff 可能であることを固定。 |
| CB-ARR-02 | Pass (headless) | `cb_arr_02_semantic_diff_classifies_clip_and_note_changes` — mute 変更が `Modified`、`AddClip` が `Added`、`RemoveNote` が `Removed` として `ObjectKind::Clip`/`Note` で分類され、非 scope clip（foreign/out-of-range/absolute）は diff に現れない。Track/小節範囲・Stable ID の照合は scope 導出と tagged stable_id が担う（timeline GUI の表示層は #114 Deferred）。 |
| CB-ARR-03 | Pass (headless) | `cb_arr_03_adopt_materializes_scoped_changes_in_one_cas` — mute/move/extent/playback/note add/clip add の複数 Track・複数 Clip 変更が live revision +1・undo 1 step の 1 CAS で materialize され、undo/redo が全体を復元/再適用。foreign/out-of-range/absolute clip・sources・assets は serialize 比較で不変。`cb_arr_12_disjoint_track_scopes_edit_independently` — track_a/track_b の disjoint scope が互いに干渉せず個別 adopt される（CB-R-03-4）。 |
| CB-ARR-04 | Pass (headless) | `cb_arr_04_boundary_inventory_reports_non_owned_lane_clips` — scoped lane 上の非所有 clip が `OutsideRange`/`AbsoluteAnchor` で typed inventory され、scope 所有 clip・foreign track clip は含まれない。`cb_arr_04_typed_rejects_foreign_and_out_of_scope_targets` — foreign clip/clip_a_out/note_foreign は `ScopeViolation`、非受理 op family・`CompOp` は `Unsupported` で、拒否後に candidate/live は不変。`cb_arr_09_move_out_and_sweep_containment_reject_typed` — move-out（`MoveClip`/`SetClipExtent`）は `Unsupported`、`AddClip` 範囲外は `ScopeViolation`、`RemoveClipsInRange` の `time` 超過・foreign lane 混入は `ScopeViolation`。Automation/Tail/Loop/Tempo の完全な影響域伝播解析は Deferred（下記）。 |
| CB-ARR-05 | Pass (headless・部分) | `cb_arr_05_absolute_anchor_reported_and_render_pin_identity` — absolute anchor/extent clip は scope 非所有で直接編集は `ScopeViolation`、boundary で `AbsoluteAnchor` 報告、`absolute_anchor` capability は `Unsupported`。scoped candidate 編集は candidate の scope digest のみを動かし adopted/live mirror・shared-context digest を保持（RCP-01 identity 契約）。live 側 absolute clip 変更は shared-context digest のみを動かし比較 context を stale にする。Tempo/Section mapping 不整合時の `Unaligned`/`Unknown` diff verdict・tempo-map 変換は Deferred（CB-08・後続 slice）。 |
| CB-ARR-06 | Pass (headless) | `cb_arr_06_branch_ids_unique_and_stale_scope_rejects` — branch-local で mint された clip/note ID が `lineage.local_ids` に記録され live entity と衝突せず、intent 後の live scoped 編集で `StaleScope` 拒否・live 不変。`cb_arr_11_delete_candidate_editable_but_adopt_defers_typed` — 削除候補の adopt は `VariantError::Unsupported`・render 解決は `RenderInputError::Unsupported` で拒否され live/revision/branch 不変（部分 commit なし）。`cb_arr_01` — save→load で variant set・scope・branch ref・candidate payload が round-trip。`cb_arr_07_recording_gate_scoped_vs_foreign_lane` — scoped lane の open recording session で `RecordingActive` 先行拒否、foreign lane の session は gate しない。 |
| CB-ARR-RCP-01 | Pass (headless・部分) | `cb_arr_05_absolute_anchor_reported_and_render_pin_identity` — `BranchRenderPin` の scope/shared-context digest 合成が branch identity を提供し、scoped 編集は scope digest のみ・shared context は不変、境界 clip の live 変更は shared-context digest のみを動かす。`cb_arr_11` — scope-removal candidate の render 解決は型付き `Unsupported` で「半 materialize された render」を構造的に防ぐ。Tempo/Loop/Automation/外部 Sidechain を跨ぐ時間失効の完全な伝播解析・実 Render 再利用評価は Deferred（#285/#288・実機 lane）。 |

## 補足: scope validation 回帰

`cb_arr_fork_scope_conflict_and_invalid_scope_reject` —
空 track 集合・duplicate track・空 range・空 mask・未知
track は `VariantError::Project`、既存 scope と overlap する
Arrangement scope は `ScopeConflict`/`Project`、非
Arrangement scope kind は `InvalidScope` で型付き拒否
（silent widen なし）。`cb_arr_15_adapter_scope_binding_and_
boundary_validation` — `ArrangementAdapter::edit_candidate`
に非 Arrangement scope を渡すと `InvalidScope`（adapter
専用 removal policy が foreign scope へ漏れない）、未知
track を指す scope の `boundary_dependencies` は
`validate_scope` と同じ構造検証で `NotFound`。`cb_arr_14_
ppqn_reprojects_arrangement_scope_time` — `SetProjectPpqn`
（960→480, Exact）で `Arrangement.time` が
`arrangement_scope` site として clip と同一 grid へ変換
され、boundary inventory の分類が変わらない。
`cb01`〜`cb06` 全 suite green —
`adapter.rs` の共有 op-gate 追加（`is_arrangement_op`/
`check_arrangement_op_targets`/`edit_scoped_project_op_
with_removal` seam）による既存 adapter 挙動変更なし
（dispatch は scope kind で分岐し、既定 removal policy は
従来の fail-closed を維持）。

## NotRun / Deferred 一覧

- 実 OS provider・実 Plugin host（実 VST3/CLAP/AU instance）・
  外部 Plugin editor/Native Dock GUI・実 RT・実 Render
  pipeline・実デバイス入力 pipeline・workspace/GPUI
  projection・実 timeline GUI（#114）: NotRun（headless
  fixture のみ。Fake/Domain Oracle の PASS を実機 PASS に
  昇格しない）。
- scope entity removal（clip/note/closure）の adopt/render
  materialization: `plan_adopt_writes`/`apply` の `Removed`
  write 対応は engine 側後続 slice — 本 slice では型付き
  `Unsupported` が正直な verdict。
- tempo-map 変換を伴う absolute anchor/extent clip の musical
  scope 写像: 変換 policy 確立後の後続 slice。
- CB-ARR-04 の Automation/Tail/Loop/Tempo 完全な影響域伝播、
  CB-ARR-05 の `Unaligned`/`Unknown` diff verdict:
  CB-08 比較 orchestration（#285）・cross-gate（#288）の責務。
- Compare orchestration・semantic merge・blend: CB-08（#285）
  Deferred。engine の set 内 branch 追加 op（literal な同時
  N>2 candidate）: CB-02〜06 と同じく後続 engine slice。

## CB-ARR 実機・外部 lane 対応表（全行 NotRun）

| Lane | 対象 | 判定 | 理由 / 後続 |
| --- | --- | --- | --- |
| OS-L1 headless | `cb07_arrangement.rs` 17 件 | Pass | `mise run test synvelis-project` green |
| GUI-L2 | 実 timeline GUI での arrangement 編集・A/B 表示 | NotRun | #114 実 GUI timeline の責務 |
| EXT-L3 | 実 plugin/実 OS/実 RT/実 Render pipeline | NotRun | #286/#287/#288 系の実機 lane |
| Compare orchestration | CB-08 semantic merge・blend・Unaligned verdict | Deferred | #285 |
| Cross-gate | RCP-01 実 render 再利用評価・時間失効伝播 | Deferred | #288・#290 系 |
