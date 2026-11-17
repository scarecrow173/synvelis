# sv511 — CB-07 materialization 残件 evidence

Issue: `scarecrow173/synvelis#511` / branch `feature/511-cb07-materialization`
OS: Windows (mise managed toolchain)。cargo 直接実行は補助証拠、
canonical = mise。

## Lane 別 Pass/NotRun

| Lane | 判定 | 根拠 |
| --- | --- | --- |
| OS-L1 headless/fixture | **Pass** | 下記 mise 検証コマンド全 pass（`mise run test synvelis-project` 55 test binary 全 ok — cb01〜cb11 系回帰なし） |
| GUI-L2 | **NotRun** | 実 GUI timeline は synvelis#114 の責務 — 本 slice は engine/apply/ppqn のみ |
| EXT-L3 実 render/RT/plugin | **NotRun** | 実 render pipeline・実 RT・実 plugin 受入は synvelis#286/#287/#288 系の別 gate |
| 実 CLI/MCP/GUI E2E | **NotRun** | headless fixture lane のみ — render 解決は composed project の型付き契約まで |

## 実行した検証（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass |
| `mise run check synvelis-project` | Pass |
| `mise run lint synvelis-project` | Pass |
| `mise run test synvelis-project` | Pass（55 test-result ok・0 failed — cb_arr_11/08/14/16 新契約 pin + cb01〜cb11 非回帰） |
| `mise run test-rt` | Pass（`cargo test --workspace --locked rt_` — variant/apply の RT 契約に波及なし） |
| `mise run check-headless-deps` | Pass（新 test は既存 headless fixture 依存のみ） |
| `mise exec -- prek run --all-files` | Pass |

補助証拠（canonical = mise）: `cargo test -p synvelis-project
--test cb07_arrangement` 18/18。

## 新規・更新 test pin

- `cb_arr_11_delete_candidate_adopts_and_renders_materialized`
  （旧 `cb_arr_11_delete_candidate_editable_but_adopt_defers_typed`
  を新契約へ書き換え）: RemoveNote+RemoveClip candidate が
  `AdoptOutcome::Applied`・`objects == 2`・revision +1・
  undo_steps 1 の 1 CAS/Undo で adopt され、宣言範囲外 entity
  （clip_a_edge/clip_a_out/clip_a_abs/clip_b_in/clip_foreign/
  note_foreign/lane membership）が不変、undo で完全復元。
  `resolve_render_project` が同一 plan で composed view を
  materialize（clip_a_in/note_in 除去・他は保持）。
- `cb_arr_08_mask_gating_clips_only_and_notes_only`: notes-only
  candidate（RemoveNote+AddNote）が `objects == 2`（Removed
  note + Added note）で adopt される新契約 assert へ更新。
- `cb_arr_14_ppqn_rejects_captured_payloads_then_reprojects_scope_
  time`: `Captured` payload 存在時の `SetProjectPpqn` が
  `Error::Validation` で typed 拒否（live/ppqn/scope/revision
  不変）→ `discard_branch` で全 `Live` 化後は scope `time` が
  新 grid へ再射影・boundary inventory 安定。
- `cb_arr_16_sweep_delete_adopts_and_cascades_notes`（新規）:
  clips-only mask で `RemoveClipsInRange` が clip_a_in のみを
  scope-owned removal（`objects == 1`）として adopt し、
  scope 非所有の note_in が clip cascade で除去されることを pin。

## 設計判断の記録

- **Removed materialization は adopt 経路限定**: VCS
  `apply_selected`（`apply/plan.rs` `change_supported`）は全
  kind の `Removed` を従来どおり `Unsupported` — sv#513 の
  共有 surface を変更しない。`removal_supported_kind` は
  `Clip | Note` のみで、他 scope-owned kind は typed
  `Unsupported` を維持（slice 横断不変条件）。
- **PPQN は typed 拒否**: candidate payload の再射影ではなく
  `Captured` payload 存在時の `SetProjectPpqn` 拒否を選択。
  frozen adopt 履歴も `Captured` であり再 adopt 可能なため
  区別せず全 `Captured` を対象 — 旧 grid transplant を構造的に
  防ぐ保守的契約。payload 再射影は後続 slice の検討事項。
- **absolute anchor は `AbsoluteAnchor` 継続**: tempo-map 変換
  policy 未確立のため musical scope 所有拡張は行わず、
  `absolute_anchor: Unsupported` + `AbsoluteAnchor` boundary
  報告の fail-closed を維持（cb_arr_04/05 が pin）。

## 未検証・残件

- 実 OS/実 Plugin/実 Render pipeline・実 GUI timeline の受入
  — synvelis#286/#287/#288/#114 系の別 gate として NotRun。
  headless Pass を実機 Pass に昇格しない。
- CB-ARR-04 の Automation/Tail/Loop/Tempo 完全な影響域伝播、
  CB-ARR-05 の `Unaligned`/`Unknown` diff verdict — #285/#288。
- candidate payload の PPQN 再射影（typed 拒否の代替案）—
  後続 slice。
- tempo-map 変換による absolute anchor の musical scope 所有
  拡張 — tempo-map conversion policy 確立後の検討事項。
- `apply/plan.rs` `change_supported` の VCS `Removed` 対応 —
  sv#513 系の別設計。
- docs index（`docs/**/README.md`）は Coordinator が統合時に
  再生成。
