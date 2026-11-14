---
id: "TSPEC-rcp01RenderRecipeTests"
type: "test-spec"
status: "approved"
title: "RCP-01 Canonical Render Recipe / typed dependency closure / temporal invalidation fixture tests"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/290"
  verifies:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/adr/rcp-00-render-recipe.md"
  supersedes: []
  superseded_by: []
---

# RCP-01 Canonical Render Recipe fixture tests

## Purpose

synvelis#290 RCP-01 スライスの headless fixture が保証すべき
振る舞いを固定する。canonical `RenderRecipe` の決定的
`RecipeKey`、typed dependency closure、temporal
invalidation、`RecipeResolution = Resolved|Unknown|Unsupported`
と `Validity = Match|Stale|Unknown` の fail-closed 契約を、
`crates/synvelis-render/tests/rcp01_recipe.rs` で検証する。
実 Plugin host・実 OS device・実 codec・実 render 実行は本
spec の対象外（`evidence/sv290/` で NotRun/Deferred 記録）。

## Feature

`synvelis-project::render_input` が live/branch render 対象を
canonical semantic 値へ写像し、`synvelis-render` の
`resolve_recipe` が render boundary から typed closure を
逆走査して `RenderRecipeV1` と `RecipeKey` を生成する。

## Rules

- **Rule RCP-KEY-CANON**: Map 挿入順・display name（UI
  rename）・render trigger 等の非音響 provenance は canonical
  bytes/key に影響しない。Plugin parameter・Sidechain・
  external asset digest・tempo・automation・renderer profile
  の音響内容変更は key を変える。
  Examples: `rec01_display_rename_and_insertion_order_do_not_move_key`,
  `rec01_acoustic_changes_move_key`.
- **Rule RCP-FAIL-CLOSED**: Plugin state quality Unknown /
  degraded、external asset unpinned・weak digest、source
  unverified、live clock/device、non-finite 値、旧 schema /
  schema skew は `Resolved` ではなく型付き `Unknown` /
  `Unsupported` を返し、Exact CacheHit として扱わない。
  Examples: `rec02_plugin_state_unknown_and_degraded_fail_closed`,
  `rec02_unpinned_asset_unverified_source_live_clock`,
  `rec02_nonfinite_and_schema_skew_fail_closed`.
- **Rule RCP-CLOSURE**: Plugin/Parameter/Source/FX
  reorder/Send/Return/Bus/Master/Sidechain/MIDI/comping/
  automation の変更は render boundary への typed closure に
  反映され、closure 外の無関係 subgraph 変更は key を動かさない。
  Examples: `rec03_closure_covers_signal_and_unrelated_subgraph_isolated`.
- **Rule RCP-TEMPORAL**: 既知 dirty interval は lookahead +
  PDC で後方、declared tail で前方に保守的に拡張され、loop
  overlap・seek boundary・temporal-domain 変更・未知 extent は
  全域失効へ落ちる。過少区間再利用を許さない。
  Examples: `rec04_dirty_interval_expansion_and_fail_closed_full`,
  `rec04_loop_seek_and_tail_expansion`.
- **Rule RCP-BRANCH**: Branch 共有 upstream edit は全該当
  branch を失効させ、branch 内部 edit は他 branch を
  unchanged に保つ。adopt 後の project revision 変化のみでは
  等価 render を無効化しない。branch/set/revision/payload
  locator 等の authority 識別子は hashed identity に含めない
  （同一音響 graph の別 branch は同一 key）。
  Examples: `rec05_shared_edit_invalidates_all_branches_internal_edit_isolated`,
  `rec05_revision_only_change_does_not_invalidate`.
- **Rule RCP-RECORD**: `RecipeRecord::verify` は schema version
  と canonical bytes からの key 再計算を検証し、破損・collision
  ・version skew は `evaluate_validity` が `Unknown` を返す
  （黙って `Match` にしない）。
  Examples: `rec06_record_verify_and_collision_fail_closed`.

## Examples

- `rec01_display_rename_and_insertion_order_do_not_move_key`:
  track/clip/plugin の display rename と entity 追加順を
  変えても key 不変。
- `rec01_acoustic_changes_move_key`: plugin parameter・
  sidechain 接続・send/return 経路・automation・tempo・
  renderer profile の各変更で key が変化。
- `rec02_plugin_state_unknown_and_degraded_fail_closed`:
  `NoPluginStates`・degraded evidence で `Unknown`。
- `rec02_unpinned_asset_unverified_source_live_clock`:
  unpinned asset・unverified source・live clock で
  `Unknown`/`Unsupported`。
- `rec02_nonfinite_and_schema_skew_fail_closed`: NaN/inf 値と
  project/render-input schema skew で fail-closed。
- `rec03_closure_covers_signal_and_unrelated_subgraph_isolated`:
  closure が routing/strip/processor/source/automation を
  含み、無関係 track の追加が key を動かさない。
- `rec04_dirty_interval_expansion_and_fail_closed_full`:
  lookahead/PDC/tail/loop/seek の interval 拡張と、未知
  extent・temporal-domain の全域失効。
- `rec04_loop_seek_and_tail_expansion`: loop overlap の全周回
  カバーと seek boundary を跨ぐ tail 拡張。
- `rec05_shared_edit_invalidates_all_branches_internal_edit_isolated`:
  live shared context 編集で両 branch の key が変化、branch
  内部編集では他 branch 不変。
- `rec05_revision_only_change_does_not_invalidate`:
  revision のみの変化（no-op commit 相当）で key 不変。
- `rec06_record_verify_and_collision_fail_closed`:
  正常 record の `Match`/`Stale` 判定と、digest 改竄・
  schema skew record の `Unknown` 判定。
