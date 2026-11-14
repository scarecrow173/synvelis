---
id: "EXP-cb02-audio-clip-adapter"
title: "CB-02 Audio Clip/Take/Comp Adapter 設計実験メモ (synvelis#279)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: CB-02 Audio Clip/Take/Comp headless スライス（synvelis#279）

## E1 comp 編集を `ProjectOp` に追加するか、別 union にするか

**仮説**: `CompOp`（comping registry 操作）を candidate edit
経路へ載せる方法は、`ProjectOp` variant 追加か、別の op 型か。

- 案 A（`ProjectOp` へ variant 追加）: `synvelis-gateway` の
  `op_name` 等が `ProjectOp` を網羅 match しており wildcard
  なし — 追加は leased crate の source 互換を壊す。却下。
- 案 B（additive union `BranchEditOp`）: `Project(ProjectOp)` /
  `Comp(CompOp)` の 2 バリアント union を `DomainAdapter::
  edit_candidate` の引数型に据え、engine に
  `BranchEditOpIntent` + `edit_branch_op` を追加。既存
  `edit_branch`・`BranchEditIntent { op: ProjectOp }`・4 引数
  `adopt_branch` は wrapper/据置で source 互換を維持。
  `synvelis-render/tests/rcp01_recipe.rs` が旧 API を直接
  呼ぶため、この互換性は必須条件だった。

**観察**: union 化により `FakeDomainAdapter`（ProjectOp のみ
受理）と `TakeUsageAdapter`（CompOp のみ受理）が同じ engine
経路を共有し、scope 閉じ込め・CAS・lineage の契約を分岐なく
適用できる。`edit_branch` wrapper が `ProjectOp`→
`BranchEditOp::Project` 変換を担うため既存呼出側は無変更。

## E2 adapter-aware adopt を `adopt_branch` 拡張か別関数か

**仮説**: `DomainAdapter::preflight_adopt`（録音中拒否・共有
material 喪失検出）を adopt 経路へ組み込む方法。

- 案 A（`adopt_branch` に adapter 引数追加）: 既存呼出
  （cb01 fixture・`synvelis-render` の rcp01 test）を全て
  書き換える必要があり leased crate を編集できない。却下。
- 案 B（`adopt_branch_adapted` 追加）: 4 引数 `adopt_branch`
  は adapter 無し経路として温存し、preflight が必要な側は
  新関数を呼ぶ。内部 adopt 実装は共通で、`adopt_branch_adapted`
  のみが `preflight_adopt` を candidate 検証後・write plan
  前に呼ぶ。

**観察**: CB-01 の fake fixture adopt は従来通り 4 引数で
動作し、CB-02 の実 adapter adopt は `adopt_branch_adapted`
経由で録音中・missing source の型付き拒否を得る。
preflight 失敗は live project・undo 履歴・branch content に
一切触れない（`cb_take_03_recording_active_adopt_rejects_typed`・
`cb_take_03_missing_source_and_unknown_source_time_reject` で
live byte 一致を確認）。

## E3 comp entity を VCS apply surface にも開くか

**仮説**: `entity::supported_kind` に comping 4 kind を加え
れば adopt materialize と VCS apply の両方が通るのではないか。

- 実装初期は `supported_kind` 自体を拡張したところ、既存
  `rcd_04_vcs_stable_id::apply_selected_fails_closed_on_
  comp_transition` が fail — 同テストは「comp kind は diff
  可視だが VCS-05 apply whitelist 外」を契約として固定して
  いた。
- 分析: VCS `apply_selected` は user が選択した部分変更を
  生 entity JSON で transplant する。comp entity は
  region overlap・candidate handle window・transition 整合
  という entity 間不変条件を持ち、部分選択 apply では
  `CompOp::validate` 相当のドメイン検証を迂回する。variant
  adopt は domain-validated candidate のみを materialize
  するため安全だが、VCS apply に同じ権限を与えるのは別の
  契約変更になる。

**観察**: `supported_kind`（VCS apply surface）は comp kind を
含まないまま据置き、variant adopt 専用の
`entity::adopt_supported_kind`（supported_kind ∪ comping 4
kind）を新設して `plan_adopt_writes` の gate に適用。
`entity_value`/`write_entity` の comp arm は payload 機構と
して残し、gate のみを経路別に分離した。結果として
`rcd_04` 契約と CB-02 adopt の両方が成立
（`apply_selected` は comp kind を引き続き
`ApplyError::Unsupported` で fail-closed）。

## E4 capability guard をどこに置くか

**仮説**: `PlaybackMapping::Loop`/`SimpleStretch` は model 上
表現可能だが lowering が `Natural` のみ受理する。この
「表示できるが処理できない」差をどこで honest に扱うか。

- 案 A（candidate edit を許し render 時に失敗）: 候補が
  作れてしまい「未サポートを成功扱い」する経路が残る。却下。
- 案 B（adapter の edit guard で typed `Unsupported`）:
  `playback_mapping_capability` を edit guard と
  `AudioClipCapabilities`/`TakeUsageAdapter` の報告で共有し、
  表示 verdict と強制 verdict を同一関数に固定。

**観察**: `CompOp::validate` 自体は非 Natural mapping を
reject しない（lowering が拒否する設計）ため、adapter が
`check_comp_op`/`edit_scoped_project_op` の guard 段で
`VariantError::Unsupported` を返すのが正しい境界。
`cb_clip_02_capability_report_matches_edit_enforcement` で
報告と強制の一致を fixture 固定した。
