---
id: "DESIGN-4kLmNpQrStUvWxYz0123"
type: "design"
status: "approved"
title: "TIM-01 PPQN Preview op + New Project PPQN — 設計"
created: "2026-11-12"
updated: "2026-11-12"
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
  implements:
    - "docs/specs/tim-01-ppqn-preview.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim-01-ppqn-preview.md"
    - "docs/designs/tim-01-configurable-ppqn-design.md"
    - "docs/designs/tim-01-app-ppqn-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-01-configurable-ppqn-design.md"
    - "docs/designs/tim-01-app-ppqn-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-01-ppqn-preview.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-preview.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN Preview op + New Project PPQN — 設計

## Context

TIM-01 headless Command slice は `ProjectOp::SetProjectPpqn`
（expected_revision CAS + `old_ppqn` ドメイン CAS + `1..=32767` +
open-recording 拒否 + scratch clone atomic validation）と、
app 層 `syn.project.set_ppqn`（transport/recording/automation の
session gate + policy wire 写像）を merge 済み。

Issue #125 の受入条件には「変更前 Preview（Snap/Grid/SMF export
への影響）」と「New Project PPQN 指定」が残る。本 slice は
この 2 件を headless で実装する。Preview は「コマンドを実行する
前に影響を確認する」read-only 契約であり、set_ppqn の session
gate を必要としない（preview は project を変更しないため
transport/recording 状態に影響されない）。

`controller.rs` の `preview_routing` は先行例だが、本 slice では
使用しない — preview は `ppqn.rs` の `&Project` 自由関数として
実装し、controller/store には触れない（並行 worker が占有中）。

## Scope

### In Scope

- `synvelis-project`: `preview_ppqn` 自由関数 +
  `PpqnChangePreview`/`PpqnPreviewSite` 公開型 + `lib.rs`
  re-export。サイト列挙の `reproject_ppqn_with` への共通化
  （refactor、振る舞い不変）。
- `synvelis-app`: `syn.project.preview_ppqn` op（`service.rs`/
  `app.rs`）、`syn.project.new` の optional `ppqn` param、
  `binding.rs` の `project_new` への ppqn 追加。
- headless fixture: `crates/synvelis-app/tests/
  tim01_ppqn_preview.rs`（新規）、`crates/synvelis-project/tests/
  tim_01_ppqn.rs` への preview unit 追記 + `rt_` fixture。

### Out of Scope

- PPQN 選択 UI widget・Preview/Confirm ダイアログ。
- Snap/Grid `SnapUnit::Explicit(960)` の PPQN 追従ポリシー。
- SMF export 挙動の変更（`export_smf` の `options.division` は
  明示 param であり project ppqn に連動しない）。
- Presentation Driver 時刻正本移動・実機/製品 Oracle。
- `synvelis-workspace`/`synvelis-gateway`/`synvelis-player`/
  `synvelis-library`/`controller.rs`/`store.rs` の変更。

## Components and Boundaries

- **`ppqn.rs`**: `PpqnSite`（crate-private サイト識別子:
  kind/instance/part）を導入し、`reproject_ppqn` のサイト走査を
  `reproject_ppqn_with(project, convert)` に抽出 — 変換コールバック
  を受ける単一の列挙経路。`reproject_ppqn` は policy 駆動の
  `reproject_tick` を渡す wrapper、`preview_ppqn` は identity
  変換 + 分析評価を渡す dry-run。
- **`preview_ppqn(&Project, new_ppqn, policy)`**:
  1. `ruviform_ppqn` で old/new を解決（範囲検証は project 層
     authoritative、`reproject_ppqn` と同一）。
  2. 分析 pass: scratch clone 上で `reproject_ppqn_with` を
     identity 変換で走らせ、各サイトを floor/ceil の
     `TickPosition::from_beat` で評価 — 一致=exact、不一致
     =inexact（`projected` に要求 policy の丸め結果）、
     変換 error=overflow。
  3. 権威 pass: 別 scratch clone で実 `reproject_ppqn` を要求
     policy で dry-run → `applies`/`rejection`。
  4. `exact_ok`/`round_ok` を導出して `PpqnChangePreview` を返す。
- **`app.rs`**: `project_preview_ppqn(&self, params)` —
  `ppqn_param`/`ppqn_policy` を再利用し、
  `self.workspace.session().project()` の `&Project` に
  `preview_ppqn` を適用。JSON 写像は `ppqn_preview_json`。
  session gate なし・`&self`（非破壊）。
- **`project_new_with_ppqn(path, name, ppqn)`**: `project_new`
  の後方互換 wrapper + 本体。fresh session の project に
  `ProjectOp::SetProjectPpqn{policy: Exact}` を bundle 作成前に
  適用 — 範囲検証・ppqn/tempo-map invariant は project 層の
  authoritative 経路。fresh project は空のため変換は常に exact。
- **`service.rs`**: `OPS` に `syn.project.preview_ppqn` 追加、
  `syn.project.new` dispatch で `optional_u32_param("ppqn")` を
  解析して `project_new_with_ppqn` へ。
- **`binding.rs`**: `project_new(path, name, ppqn)` — `ppqn` は
  `Option<u32>`、`json!` の null 許容で既存 emit 形状を維持
  （wire 側は absent/null を `None` として扱う）。

## Op wire 契約

```
syn.project.preview_ppqn
params:
  new_ppqn: u64 → u32, required    // target PPQN (1..=32767 は project 層)
  policy:   "exact" | "round", required
  rounding: "floor"|"ceil"|"toward_zero"|"away_from_zero"|"nearest_even",
            required iff policy == "round"
```

返り値（JSON）:

```
old_ppqn, new_ppqn, policy, rounding(null|name),
site_count,
inexact:        [{kind, instance, part, tick, projected}],
overflow_sites: [{kind, instance, part, tick, projected}],
exact_ok, round_ok, applies, rejection(null|string),
effects: { snap_grid, smf_export }   // informational strings
```

- `site.kind`: `tempo_event`/`signature_event`/`clip_anchor`/
  `clip_extent`/`take_group_range`/`comp_region_range`/
  `comp_transition_range`。`instance` は entity の tagged id
  またはイベント index、`part` は ranged site の `"start"`/
  `"end"`。
- `projected`: 要求 policy 下での変換後 tick。Exact 下の inexact
  site と overflow site は `null`。
- `applies`/`rejection` は要求 policy の dry-run 結果 —
  `set_ppqn` が同 params で拒否されるかの権威的答え。
- `exact_ok`/`round_ok` は policy クラス別の概略可否
  （i64 境界の個別 rounding 差は `applies` が権威）。

```
syn.project.new
params:
  path:  string, required
  name:  string, optional
  ppqn:  u64 → u32, optional       // 省略/null = 960 既定
```

- `ppqn` の wire 形状検査（u64→u32）は service 層
  `optional_u32_param`。`1..=32767` の domain 検証は
  `project_new_with_ppqn` が `SetProjectPpqn` 経由で project
  層に委譲 — app 層で範囲を再実装しない。
- `ppqn == 960`（既定値と同一）は op を skip し revision churn
  なし。範囲外は bundle 作成前に失敗し stray directory を残さない。

## 判定と error 写像

- preview: params 不正・`new_ppqn` 範囲外 → `invalid_params`
  （`set_ppqn` と同じ convention）。
- project.new: wire 形状不正 → `invalid_params`、domain 範囲外
  → `AppError::store` → `service_error`（project 層
  `Error::Validation` を既存 store error 経路で surface、
  message に理由を保持）。

## Data and Control Flow

1. `UiRuntime::dispatch_op("syn.project.preview_ppqn", params)`
   → `AppService::call` → `dispatch` →
   `SynvelisApp::project_preview_ppqn(params)`。
2. `ppqn_param`/`ppqn_policy` で wire → `PpqnConversionPolicy`。
3. `session.project()` → `&Project`（**読み取りのみ**、
   `session.project()` は `controller.live()` を返す公開
   accessor）。
4. `preview_ppqn(project, new_ppqn, policy)` →
   `PpqnChangePreview` → `ppqn_preview_json` → 返却。
   project/revision/engine notification は一切不変。

`project_new` 経路: workspace+fresh session 構築 →（ppqn 指定時）
`session.apply(SetProjectPpqn{Exact})` → `DirectoryBundleStore::
create` → `save` → `project_info`。

## Non-Functional Constraints

- preview は `&Project` のみを受け、live state・revision・
  engine notification を変更しない（2 clone 上の dry-run のみ）。
- サイト列挙は `reproject_ppqn_with` 単一経路 — preview と
  mutating command が同じサイト集合を見ることを構造で保証。
- 検証ロジックの重複実装禁止 — 範囲・CAS・変換拒否は
  `synvelis-project` authoritative、app 層は wire 写像のみ。
- `Tick`/`TickDuration` の unchecked 算術は使用しない
  （`TickPosition::from_beat`/`to_beat` の checked 変換のみ）。

## Risks and Trade-offs

| リスク | 対策 |
| --- | --- |
| preview と実コマンドの報告乖離 | サイト列挙を `reproject_ppqn_with` に共通化し、`applies`/`rejection` を実 `reproject_ppqn` の clone dry-run で権威化 |
| 分析 pass の分類と Round 個別 rounding の境界差（i64 極値） | `overflow_sites` は floor/ceil envelope で保守的に分類、`applies` を権威と明記（実用上の musical 値では不発） |
| `project_new` での ppqn 適用が bundle 作成を遅らせる | workspace 構築は infallible、ppqn 検証失敗時に stray directory を残さない順序を選択 |
| preview の `effects` が将来の Snap/Grid/SMF 変更で陳腐化 | informational string に留め、structural な契約は site/feasibility のみに限定 |

## Deferred Design Concerns

- PPQN 選択 UI widget・Preview/Confirm ダイアログ — 別 slice
  （本 op の wire 契約を消費する）。
- Snap/Grid 既定量子の PPQN 追従ポリシー — UI slice で決定。
- SMF export の division 既定連動 — 別 slice。
- Presentation Driver 時刻正本移動・実機/製品 Oracle — 別 slice。

## Verification Notes

- [x] `crates/synvelis-project/tests/tim_01_ppqn.rs` に preview
      unit 6 件 + `rt_` fixture 1 件を追加し green
- [x] `crates/synvelis-app/tests/tim01_ppqn_preview.rs` に
      headless fixture 6 件を追加し green
- [x] `mise run` 全検証 lane（fmt-check/check/lint/test/test-rt/
      check-app-native/lint-app-native/test-msrv/build）— 結果は
      `docs/impl/ir/tim-01-ppqn-preview.md` に記録
