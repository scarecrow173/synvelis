---
id: "TSPEC-TiM01wIdGeTnPw04"
type: "test-spec"
status: "approved"
title: "TIM-01 PPQN 選択 widget intent — test spec"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "crates/synvelis-app/tests/tim01_ppqn_widget.rs"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-01-ppqn-ui.md"
    - "docs/test-specs/tim-01-ppqn-preview.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-01-ppqn-widget.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-01-ppqn-widget.md"
  verified-by: []
  references:
    - "docs/tasks/tim-01-ppqn-widget.md"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN 選択 widget intent — test spec

## Purpose

Issue #125 widget slice — Project tab の params 付き Button が
実 `rvf.ui.command.v1` `Action` を mint し、shell 経路で
`syn.project.ppqn` に届き、`ppqn_*` gesture → `syn.project.*`
ops → command `Result` receipt → `ppqn.*` spec 再投影が 1 本の
鎖として通ることを `crates/synvelis-app/tests/tim01_ppqn_widget.rs`
で固定する。fixture は `common::mounted`（app → mounted
`ViewHost` → `NativeShell` → `FakeDriver`）— `synvelis-native`
と同じ composition で、`press()` は `ViewHost::invoke` の実経路。
`ppqn_*` helper 単体・`PpqnUi` read model・preview/set wire 契約の
網羅は `docs/test-specs/tim-01-ppqn-ui.md` 等が担い、本 spec は
widget intent 境界のみ固定する。

## Feature

推奨値 Button（`ppqn.select.<v>.<policy>`）・Confirm/Cancel
Button（`ppqn.confirm`/`ppqn.cancel`）・`syn.project.ppqn`
gesture op の 3 action 意味（select = select+preview 融合、
confirm = preview CAS 付き、cancel = 非破壊）を、receipt と
`MainScreenState`/spec 再投影の両側で固定する。

## Rules

- `ppqn.select` Button は `{new_ppqn, policy, rounding?}` を
  params verbatim で運び、target は `view`、action は
  `ppqn.select`。`ppqn.confirm`/`ppqn.cancel` Button は params
  なし。
- select の receipt は `syn.project.ppqn` の Result —
  `{view_id, selected, preview}`。pending candidate +
  revision-fresh preview が `ppqn.*` surface に投影される
  （live PPQN は不変）。
- confirm は preview applicable のときのみ適用し
  `{view_id, confirmed, project}` を返す。pending 無し・
  rejected preview の confirm は reject — implicit commit
  しない。
- cancel は `{view_id, canceled}` — pending 無しでも成功し、
  live PPQN は不変。
- pending 中の再 select は candidate を置き換え、confirm は
  最新 candidate を適用する。
- forged/malformed params は `invalid_params` または shell/
  binding 境界の reject — pending を残さない。

## Examples

- `project_tab_mounts_ppqn_widgets`: `ppqn.candidates`/
  `ppqn.gesture` panel が spec に存在し、8 Button の
  target/action/params 宣言（`new_ppqn`・`policy`・
  `rounding` = `nearest_even` for round）と Confirm/Cancel の
  params 無しを固定。
- `widget_select_runs_preview_and_updates_surface`: press
  `ppqn.select.480.exact` → Result ok → pending 480/exact +
  `preview.applies` → `ppqn.preview` label が "960 → 480" を
  描画、live 960 不変。
- `widget_confirm_applies_after_select`: select → confirm →
  `{confirmed: 480, project.ppqn: 480}` → `ppqn.status` が
  "PPQN 480" に再投影、`syn.project.info` oracle 一致。
- `widget_cancel_discards_pending`: select 1000 round → cancel
  → `{canceled: true}` → pending 消滅・live 960 不変。
- `widget_reselect_replaces_candidate`: 480 exact → 1920 round
  再 select → confirm は 1920 を適用。
- `widget_confirm_and_cancel_without_pending`: pending 無しの
  confirm は Result reject・live 不変、cancel は ok +
  `{canceled: false}`。
- `widget_exact_preview_rejection_blocks_confirm`: odd-tick
  bundle（clip anchor = musical tick 7）で select 480 exact →
  preview `applies: false` + inexact sites → confirm reject・
  live 960 不変 → 同一 candidate の 480 round は applies →
  confirm で 480 適用。
- `widget_forged_select_params_rejected`: forged
  `rvf.ui.command.v1` `ppqn.select` — `{new_ppqn:0}` /
  `{new_ppqn:32768}` / `{policy:"bogus"}` /
  `{policy:"exact", rounding:"floor"}` / `{policy:"round"}`
  rounding 無し / `{rounding:"bogus"}` / `new_ppqn` 文字列 /
  `new_ppqn` 欠落 / `policy` 欠落 — 全て reject、pending 残さず。
- `widget_unknown_action_and_target_boundary`: `ppqn.bogus` →
  op が未知 action を reject。`project` target + params →
  binding が "not representable" で reject。非 object params
  （array）→ shell が早期 reject。
- `op_path_selects_arbitrary_value`: typed `syn.project.ppqn`
  driver 経路で任意値 2400 が select/preview/confirm 可能 —
  widget 非搭載値も同じ op が担う。

## Guarantees

- 全 widget intent は `decode_view_command` の params 許可
  判定を通るのみ（`view` target）— domain target への params
  wire 拡張なし。
- gesture の全 mutation は `syn.project.set_ppqn` の double
  CAS + session gate を経由 — widget 経路に別の変更経路は
  存在しない。
- reject は常に command `Result`（Button receipt settle）として
  返る — 沈黙の丸め・implicit commit・pending 残留なし。
- spec の `ppqn.*` 表示は `publish_project_length` の
  revision-guarded 投影のまま — widget 追加で表示 authority
  は変わらない。

## Non-goals

- 実 OS/実 Window/pixel/IME（GUI-L2 — `synvelis#114` +
  `ruviform#631` 外部 gate、NotRun/Unsupported のまま）。
- 任意値入力 widget・SMF export division 連動・
  Presentation Driver 時刻正本・`PpqnUi` read model 自体の
  網羅（先行 test-spec 管轄）。

## Risk

- `press()` の `ViewHost::invoke` が mint する intent envelope
  に依存（command_id・`origin_view`）— `common::mounted` の
  `pump_view_intents` が `RvfView` の mint と同型なので実経路
  準拠。forged intent 系 case は wire 形状を自前で組み、
  厳密 decode の貫通を直接固定する。
