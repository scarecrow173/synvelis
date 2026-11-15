---
id: "EXP-cb07-arrangement-adapter"
title: "CB-07 Arrangement Adapter 設計実験メモ (synvelis#284)"
created: "2026-11-15"
updated: "2026-11-15"
---

# 実験記録: CB-07 Arrangement headless スライス（synvelis#284）

## E1 candidate 側の scope removal を受理するか

**仮説**: Arrangement 候補は range 内 clip/note の削除を
表現できるべきか — CB-01〜CB-06 の共有 adopt 経路は
candidate 側 scope entity の消失を fail-closed で拒否する
（`plan_adopt_writes`・render 解決とも `Unsupported`）。

- 調査: 削除は arrangement 編集の基本操作であり、受理
  しなければ候補が「部分的な arrangement」しか表現できない。
  一方で adopt の apply/entity materialization は scope
  object の消失をまだ書き戻せず、render resolution も
  entity 消失を fail-closed にする。候補編集と adopt を
  同じ拒否規則で縛ると「編集できるが adopt できない」
  中途半端な面になる。
- 案 A（共有 confined-edit を Arrangement だけ removal 許容に
  変更し、adopt 側も removal materialization を実装）:
  apply/entity の消失書き戻しは CB-07 の境界を超える
  横断変更で、既存 adapter の fail-closed 契約も揺らぐ。
  却下。
- 案 B（編集面のみ removal を受理・adopt/render は型付き
  defer のまま）: `edit_scoped_project_op` に removal policy
  seam を追加し、Arrangement のみ「真の object 消失」を
  受理。control closure 等の unsupported removal は
  fail-closed を維持。adopt/render は従来どおり
  `Unsupported` で拒否し、slice 全体の defer として
  明示する。

**観察**: 案 B を採用。`cb_arr_11_delete_candidate_
editable_but_adopt_defers_typed` で note→clip の順序削除が
候補編集として受理され save/reload・semantic diff
可能なこと、adopt が `Unsupported` で型付き defer
されることを検証。既存 adapter は removal policy
未指定で従来挙動を維持（cb02〜cb06 回帰なし）。

## E2 range 外へ動く clip をどう扱うか

**仮説**: scope 所有 clip を range 外へ move/extent 縮小
する編集は「削除」か「scope 外への移動」か — どちらの
解釈でも静かに rescope してはいけない。

- 調査: move/extent op は対象 clip が scope 所有なら
  `require_scope` を通る。しかし適用後に clip が range と
  重ならなくなると、所有導出（`scope_objects` の
  musical overlap）から外れ、scope digest が変わる —
  実質的な rescope が編集 op 経由で起きる。
- 案 A（適用後の重なりで再判定）: 適用済み変更の
  rollback が必要で fail-closed 契約に反する。却下。
- 案 B（post-edit の scope-removal diff で検査）:
  適用後に scope object 集合から外れた entity が
  candidate に実在する（move-out/extent 縮小/split で
  kept half が range 外）なら `Unsupported` — entity は
  消えていないので「削除」ではなく未追跡の live 変更。
  削除は `RemoveClip` 系で明示する — 「消す」と
  「動かす」を op の種類と entity 消失の有無で区別する。
  新規生成 half が scope 外に落ちる場合は
  `check_created_within_scope` が `ScopeViolation` を返す。

**観察**: 案 B を採用。`cb_arr_09_move_out_and_sweep_
containment_reject_typed` で clip を range 外へ動かす
move/extent 編集が `Unsupported`（entity 残存の
scope 脱出）、範囲外 sweep が `ScopeViolation`（op の
意図範囲が scope 超過）で拒否され live 不変であることを
検証。

## E3 absolute anchor/extent clip の ownership

**仮説**: `TimeAnchor::Absolute` または absolute extent を
持つ clip は musical range との重なりを stable に導出
できない。tempo map 変換 policy なしに所有判定すべきか。

- 調査: musical range は beat/tick 座標、absolute anchor は
  実時間座標。変換には tempo map の versioned 解釈が必要で、
  CB-07 の headless slice には tempo-map conversion policy
  がない。所有に含めると「どの range に属するか」が
  実装依存の推定になり、fingerprint/conflict の正本性を
  損なう。
- 案 A（tempo map で変換して所有判定）: conversion policy
  の設計・実装は本 slice の範囲外。却下。
- 案 B（所有から除外し boundary inventory で型付き報告）:
  選択 track lane 上の absolute clip を
  `ArrangementBoundaryReason::AbsoluteAnchor` として列挙し、
  range 外 musical clip は `OutsideRange` として区別する。
  silent ignore にしない。

**観察**: 案 B を採用。`cb_arr_04_boundary_inventory_
reports_non_owned_lane_clips` で両 kind が型付き列挙
されること、`cb_arr_05_absolute_anchor_reported_and_
render_pin_identity` で absolute clip が render pin
identity に正しく現れ候補が foreign context を
壊さないことを検証。

## E4 mask（clips/notes）の enforcement 面

**仮説**: `ArrangementDomainMask` の `clips`/`notes` は
scope ownership と直交する編集面の gate — notes-only
scope でも parent clip の confinement が必要か。

- 調査: notes は clip-local entity で、note op の対象解決
  は parent clip 経由。notes-only scope が clip を所有
  しないと note 追加/削除の confinement が clip 境界を
  越える。mask を ownership に混ぜると「notes だけ編集
  できるが clip は動かせない」という粒度が表現できない。
- 案 A（mask を ownership 導出に畳み込む）: mask 違いの
  scope が同じ entity 集合を所有し digest が衝突する。
  却下。
- 案 B（ownership は mask 非依存・op 受理面のみ mask
  gate）: `fields.clips` が clip 構造/編集 op、
  `fields.notes` が clip-local note op を gate。
  notes 宣言時は parent clip も confinement 対象に含める
  （所有ではなく confinement の話として
  `scope_objects` 側で導出済みの clip entity を利用）。

**観察**: 案 B を採用。`cb_arr_08_mask_gating_clips_
only_and_notes_only` で clips-only scope の note op
拒否・notes-only scope の clip 構造 op 拒否を検証。
capability 報告（`capabilities`）が同一 mask 判定を
反映することを同 test 内の mask 別 capability 照合で
確認。

## E5 recording gate の lane 写像

**仮説**: Arrangement scope は複数 track × lane を跨ぐ
— recording gate はどの lane を検査すべきか。foreign
track の recording は adopt を止めるべきか。

- 調査: scope が所有するのは選択 track の lane 上の
  clip/note entity。foreign track の recording session は
  scope entity と無関係で、止める理由がない。一方で
  選択 track のいずれかの lane で recording が Open
  なら、その lane の clip 実体が adopt 書き戻しと競合
  し得る。
- 案 A（project 全 lane 検査・CB-05 bus strip と同じ
  fail-closed）: foreign track の recording まで止め、
  track 選択の意味が薄れる。却下。
- 案 B（選択 track の全 lane を検査）: scope の semantic
  ownership（track 集合）と一致する最小の安全面。
  `preflight_adopt` が各 track の全 lane を走査し
  `RecordingActive` を型付き返却。

**観察**: 案 B を採用。`cb_arr_07_recording_gate_
scoped_vs_foreign_lane` で scoped track lane の
recording Open が adopt を `RecordingActive` で
拒否し live 不変であること、foreign track の
recording では adopt が進むことを検証。

## E6 `RemoveClipsInRange` の sweep range

**仮説**: range sweep op の対象範囲が宣言 range を
はみ出す場合、はみ出し部分をどう扱うか。

- 調査: `RemoveClipsInRange` は「この範囲の clip を
  消す」op で、対象 clip の confinement だけでは
  sweep range 自体の範囲が scope 外にはみ出す問題を
  検出できない。宣言 range 外の clip を巻き込むと
  foreign clip の削除要求になる。
- 案 A（sweep range を無視して clip 単位で confinement
  のみ検査）: sweep range 外の clip も「範囲内」と
  誤認して削除し得る。却下。
- 案 B（sweep range ⊆ 宣言 range を要求）: sweep
  range が宣言 range に含まれない場合は `ScopeViolation`
  — op の意図した効果範囲が scope を超えることを
  pre-edit で型付き拒否（lane 違反も同じ verdict）。

**観察**: 案 B を採用。`cb_arr_09_move_out_and_sweep_
containment_reject_typed` で範囲外 sweep が
`ScopeViolation`、範囲内 sweep が受理されることを検証。

## E7 review で確定した追補規則（split note migration・PPQN・adapter 結束）

実装 review（cooperative + adversarial の独立 2 系統）で
確定した追補。いずれも fail-closed 方向の修正で、受理面を
広げない。

- **SplitClip の note 再配置は mask 外副作用**:
  `SplitClip` は split 点以降の clip-local note を新 clip
  へ無条件に再親付けする（`ops.rs` apply）。`clips` のみ
  の mask では note は scope object でないため、candidate
  だけが note の `clip`/`start` を書き換え、adopt の
  scope-object write には現れず live と静かに乖離する。
  対応: `fields.notes` 未宣言時、split 点以降に note を
  持つ clip の split を `Unsupported` で拒否
  （`check_split_note_migration`）。split 点より前の
  note は kept clip に残り乖離しないため受理する。
  `cb_arr_13_split_clip_notes_mask_gate` で clips-only
  拒否・非移動受理・full mask での adopt 後 note 再親付け
  を検証。
- **mask 判定は target confinement に先行**: op family が
  未宣言なら target の所在に関わらず `Unsupported` —
  capability report（mask 相対）と enforcement を一致
  させる。宣言済み family の foreign target は従来どおり
  `ScopeViolation`。`cb_arr_08` の `RemoveNote` 判定を
  `ScopeViolation`→`Unsupported` に更新。
- **`SetProjectPpqn` は scope 宣言も再射影する**:
  `VariantScope::Arrangement.time` は persisted
  musical-tick 値であり、`reproject_ppqn_with` の walk
  対象に追加（`arrangement_scope` site、preview と共有）。
  これがないと PPQN 変更で clip は変換されるのに scope
  window だけ旧 grid に残り、所有導出が黙って別 clip を
  指す。`cb_arr_14_ppqn_reprojects_arrangement_scope_time`
  で変換と boundary inventory の安定性を検証。
  残存リスク: branch content store 内の candidate payload
  は project 外のため本 walk の対象外 — 全 domain 共通の
  slice 横断論点として IR に記録し別途追跡する。
- **adapter↔scope の結束を edit 経路でも強制**:
  `edit_branch_op` は adapter と set.scope を独立に受け取る
  ため、`ArrangementAdapter::edit_candidate` は
  非 Arrangement scope に `InvalidScope` を返す
  （`validate_scope` と同じ guard）。これがないと
  arrangement 専用の removal policy が foreign scope の
  note 削除を受理し得た。`cb_arr_15` で検証。
- **`boundary_dependencies` は構造検証を先行**:
  `validate_scope_structure` を通し、未知 track 等の
  malformed scope を typed error にする（silent partial
  report を防ぐ）。同じく `cb_arr_15` で検証。
