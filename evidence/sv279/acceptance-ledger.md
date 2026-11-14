# Acceptance Ledger — synvelis#279 CB-02 Audio Clip/Take/Comp headless スライス

Date: 2026-11-14. Slice: CB-02 `VariantScope::AudioClip` /
`VariantScope::TakeUsage` 実ドメイン Adapter + comping entity
adopt materialize + headless fixture/回帰テスト。
実ドメイン Adapter 残り（CB-03〜08 = #280〜285）・
Compare Orchestrator（#285）・GPUI（#286）・Action/Gateway
統合（#287）・Cross-Gate/実機受入（#288）は DEFER。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/279
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis SHA | feat/279-cb02-audio-clip-adapter HEAD（ledger は同 commit 内） |
| OS / CPU | Windows 11 / Intel Core i9-9900K |
| Toolchain | mise 管理 Rust（`mise run` 経由） |
| Project schema | v14（schema 非変更 — variant model への entity 追加なし） |

## 判定語彙

- **Pass (headless fixture)**: `crates/synvelis-project/tests/
  cb02_audio_take.rs` が `mise run test synvelis-project` で
  green。
- **NotRun**: 実 OS/実 Plugin/実 RT/実 Render/実機依存の判定 —
  本スライスでは実行していない。
- **Deferred**: 後続 Issue の範囲として明示的に送った部分。

## 受入条件 → 証拠対応

| AC | 判定 | 証拠 |
| --- | --- | --- |
| CB-AUDIO-RCP-01 | Pass (headless) | `cb_audio_rcp_01_clip_branch_render_reuse_and_partial_invalidation`・`cb_audio_rcp_01_take_branch_render_reuse_and_partial_invalidation` — scope content digest が render-reuse identity として branch 横断・adopt 前後で一致し、trim/placement/mute・take 選択/range/transition の各編集が部分失効として digest を移動。composed render view は candidate state を live shared raw material 上に解決し take/source/asset を複製しない。実 Render 再利用評価・cross-branch render cache は Deferred（#289〜296 系）。 |
| CB-CLIP-01 | Pass (headless) | `cb_clip_01_shared_raw_asset_candidates_survive_fork_save_reload` — 同一 Raw Asset を共有する 2 Clip 加工候補の Fork/Save/Reload で元 Blob SHA（`raw-blob-sha-01`）と Source Metadata が bitwise 不変。 |
| CB-CLIP-02 | Pass (headless) | `cb_clip_02_capability_report_matches_edit_enforcement` — `AudioClipCapabilities` が Trim/Placement/Mute/ClipNotes=Supported・Loop/SimpleStretch/Pitch/Fade/ExternalRecipe=Unsupported を報告し、edit guard が同一 `playback_mapping_capability` で非対応 mapping を typed `Unsupported` 拒否。未サポートを成功扱いしない。 |
| CB-TAKE-01 | Pass (headless) | `cb_take_01_take_selection_and_processing_are_distinct_scoped_edits` — 同一 Take の selection edit（`SetCompRegionCandidate` で take_a0→take_a1）と processing edit（`SetCompRegionRange`）が別 scope digest を持ち、未編集 region の candidate/project_range・transition・source handle が bit 同一（無断上書きなし）。 |
| CB-TAKE-02 | Pass (headless) | `cb_take_02_adopt_writes_only_scoped_comp_entities_single_cas_undo` — Adopt が scoped comp closure のみを 1 CAS + 1 Undo で materialize（selection + 新規 comp mint を含む）、Raw Recording/Recovery データと別 lane（group_b/comp_b）の take group が bitwise 不変。undo で live comp state・採用 pointer が復元。 |
| CB-TAKE-03 | Pass (headless) | `cb_take_03_recording_active_adopt_rejects_typed`（録音中 session 関与の adopt を `VariantError::RecordingActive` で型付き拒否・live byte 一致）、`cb_take_03_missing_source_and_unknown_source_time_reject`（missing source=typed `NotFound`、handle 外 source 時刻=typed `Validation`、存在しない take 参照=reject、adopt preflight の共有 material 喪失検出）、`cb_take_03_out_of_scope_and_unsupported_comp_ops_reject`（scope 外 take group・mapping capability 外 op を mutation なしで拒否）。実 Recording Writer・実デバイス/入力 pipeline が絡む録音中切替の実機確認は NotRun/Deferred（#12/#115〜#118 系・後続 Gate）。 |
| CB-TAKE-04 | Pass (headless) | `cb_take_04_candidate_reusable_until_discard_and_lineage_survives_reload` — 非採用候補は明示 Discard まで render read adapter 経由で再比較可能、digest が編集ごとに移動、Undo/Redo/Save/Reload で同一 Source lineage（branch-local minted ID・provenance）が復元。 |

## NotRun / Deferred 一覧

- 実 OS provider・実 Plugin host・実 RT・実 Render pipeline・
  実録音/Import pipeline・workspace/GPUI projection: NotRun
  （headless fixture のみ。Fake/Domain Oracle の PASS を実機
  PASS に昇格しない）。
- CB-TAKE-03 の実機側（実 Recording Writer・実デバイス/入力
  pipeline での録音中対象切替）: Deferred — headless では
  「録音中の採用に対する明示安全条件/型付き拒否」の判定経路
  までを fixture で証明（#12/#115〜#118 系・後続 Gate の責務）。
- 残ドメイン Adapter（CB-03〜08 = Issue #280〜285）・
  Compare Orchestrator（#285）・GPUI（#286）・Action/Gateway
  統合（#287）・Cross-Gate/実機受入（#288）: Deferred。
- comp 変更の VCS `apply_selected` 経路: Unsupported/Deferred
  — comp kind は VCS apply surface 外のまま（`rcd_04` 契約
  維持）。`CompOp` replay 型の設計は後続 Issue。
- `Loop`/`SimpleStretch` mapping の実処理: Unsupported —
  lowering が `Natural` のみ受理する現行契約では adapter の
  typed `Unsupported` が正しい verdict。
- 実測なしの性能保証・Semantic Merge/Blend・全 Branch 常時
  RT 駆動: 対象外（Issue 本文の「対象外」どおり）。
