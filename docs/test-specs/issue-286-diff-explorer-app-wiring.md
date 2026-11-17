---
id: "TSPEC-034ZeN710ka50ujS8RWsVD"
type: "test-spec"
status: "draft"
title: "issue-286-diff-explorer-app-wiring"
created: "2026-11-17"
updated: "2026-11-18"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/286"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-09-diff-explorer.md"
    - "docs/tasks/issue-286-diff-explorer-app-wiring.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/creative-branch-design.md"
  derived-by: []
  verifies:
    - "docs/specs/creative-branch.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-286-diff-explorer-app-wiring

## Purpose

CB-09 headless view-model（`cb-09-diff-explorer`）を
`synvelis-app` へ配線した際に、view lifecycle・op dispatch・
CAS 境界・evidence provenance が headless 契約どおりに
保たれることを固定する。この spec は synvelis#286 の
app-wiring slice の検証意図を記録し、実 GPUI 実窓・実
plugin/RT 受入が必要になった時点で別 test-spec へ引き継ぐ。

## Feature

CB-09 Difference Explorer app wiring — detached
`ViewType::VariantDiff` view の open/close・`ViewSpec`
projection/mount・`syn.variant-diff.visual`/`view`/`action`
op・CB-08 orchestrator compare・gateway CAS adopt の
end-to-end headless 経路

## Rules

- `syn.variant-diff.*` op は登録済み `ViewType::VariantDiff`
  view のみ受理し、他 kind・未登録 view id は typed error
  で拒否する。
- view-local action（select/audition/layer visibility/
  set-target 等）は project authority を一切変更しない。
- authority-bound action は persisted target の pin、または
  action 自身の explicit pin のみを使い、implicit・stale・
  malformed pin は fail-closed に reject する。
- compare は CB-08 orchestrator 経由で semantic/signal/
  perceptual evidence を採取し、pin-scoped session に保持
  する。projection は live authority/context を毎回再導出
  し、pin が drift した evidence を available のまま見せない。
- adopt-preview は simulate のみで commit しない。adopt は
  明示 `params.approval` を要求し、CAS（project revision・
  branch revision・scope digest）を通る。preview/compare
  intent が commit 経路に入ることはない。
- adopt は compare target を捏造しない — target 未設定の
  view は adopt 後も `UNAVAILABLE` を正直に返す。
- `Unavailable`/`Unknown`/`Unsupported` の evidence state を
  available や zero-diff に昇格しない（headless 契約の
  app 側での維持）。
- runtime publish observation は adopt receipt JSON の
  `/runtime/state` が decode できた時だけ記録し、欠落・
  decode 失敗時は観測を捏造しない。
- project bundle root が変わった時、engine・session・
  publish observation を破棄して evidence が project を
  跨がない。

## Examples

- visual open/close と kind gate:
  `cb09_app_visual_open_close_and_kind_gate` — open で
  detached `variant-diff` view が mint され、close で
  view-local state ごと破棄される。他 view kind への
  `syn.variant-diff.*` dispatch は reject。
- target seed 付き open:
  `cb09_app_open_with_target_seed` — `params.target` の
  persisted `VariantDiffTarget` が view state に seed
  され、以後の projection がそれを使う。
- view-local action の authority 非接触:
  `cb09_app_view_local_actions_never_touch_authority` —
  select/audition/layer visibility が project revision・
  branch revision を動かさない。
- compare の pin-scoped evidence:
  `cb09_app_compare_stores_pin_scoped_evidence` — compare
  が persisted semantic target を記録し、session evidence
  が左右 pin と project revision に scope される。
- stale/implicit pin の fail-closed:
  `cb09_app_compare_rejects_stale_and_implicit_pins` —
  drift した branch revision・pin 無し action は typed
  reject。
- preview は commit しない:
  `cb09_app_adopt_preview_never_commits` — adopt-preview が
  preview evidence を返すが project revision・adopted
  branch を変更しない。
- adopt の approval 必須:
  `cb09_app_adopt_requires_explicit_approval` —
  `params.approval` 無しは reject。
- adopt の CAS commit:
  `cb09_app_adopt_commits_through_cas` — approval 付き
  adopt が adopted branch を commit し、revision 前進時のみ
  session notify。target 未 mint のため view は
  `UNAVAILABLE` を維持する。
- stale revision の CAS reject:
  `cb09_app_adopt_rejects_stale_revision_cas` — authority
  drift 後の adopt は typed reject で commit しない。
- ViewSpec mount:
  `cb09_app_variant_diff_spec_mounts_on_view_host` —
  `syn.variant-diff.view` が返す `ViewSpec` を parse し
  `mount_variant_diff_view` が `variant-diff.*` component
  を mount する。
- project 切替時の state 分離:
  `cb09_app_project_switch_clears_variant_diff_state` —
  `project.new`/`open` で engine・per-view session・
  publish observation が破棄され、旧 project の pin・
  receipt が新 project の view id 空間へ漏れない
  （identical bundle copy で branch id が再利用されても
  session evidence は衝突しない）。
- workspace facade（`cb09_diff_explorer.rs` 追加分）:
  `cb09_engine_compare_projects_cb08_layers`,
  `cb09_engine_compare_resolves_arrangement_adapter`,
  `cb09_engine_compare_maps_signal_provider_provenance`,
  `cb09_engine_adopt_preview_and_commit_keep_cas`,
  `cb09_engine_adopt_rejects_typed`,
  `cb09_engine_compare_rejects_stale_and_unknown_pins`,
  `cb09_engine_authority_and_context_reflect_live_state` —
  orchestrator/gateway seam の typed dispatch・provenance
  保持・CAS 境界を固定。

## Guarantees

- `syn.variant-diff.*` op は view kind gate・pin 検証・
  CAS を通った経路でのみ project authority に到達する。
- view-local state と project/variant authority の分離が
  app 層でも維持される。
- semantic/signal/perceptual evidence の分離と honest
  state 語彙が app projection まで一貫する。
- adopt-preview が commit 経路に混入しないことが型と
  test の両方で保証される。

## Non-goals

- 実 GPUI window・実レンダリング・focus/keyboard 経路 —
  mount smoke（ViewHost への component mount 確認）まで。
  実 driver 受入は synvelis#288 系 gate。
- 実 plugin state・PCM/RT signal 測定・perceptual 評価 —
  signal/perceptual は provider seam の provenance 付き
  evidence として扱い、実測定は #288 系 gate。
- CB-10 gateway typed contract slice・CB-11 cross-gate の
  受入。

## Risk

- app 側の evidence pin-scope 判定や CAS notify 条件を
  誤ると、stale evidence の表示や無駄な engine rebuild・
  さらには authority の不正更新を招く。test は
  stale/implicit reject・preview 非 commit・revision 変化
  時のみ notify を固定している。
