---
id: "DESIGN-TiM01uIxYzAbCdEfGh2"
type: "design"
status: "approved"
title: "TIM-01 PPQN 選択 UI + Snap/Grid ポリシー — 設計"
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
    - "docs/specs/tim-01-ppqn-ui.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim-01-ppqn-ui.md"
    - "docs/designs/tim-01-ppqn-preview-design.md"
    - "docs/designs/tim-01-app-ppqn-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-01-ppqn-preview-design.md"
    - "docs/designs/tim-01-app-ppqn-design.md"
    - "docs/designs/tim-01-configurable-ppqn-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-01-ppqn-ui.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-ui.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN 選択 UI + Snap/Grid ポリシー — 設計

## Context

TIM-01 の headless 契約は merge 済み: `ProjectOp::
SetProjectPpqn`（expected_revision CAS + `old_ppqn` ドメイン
CAS + `1..=32767` + open-recording 拒否）、app 層
`syn.project.set_ppqn`（session gate + policy wire 写像）、
非破壊 `syn.project.preview_ppqn`（サイト列挙・inexact/
overflow・policy 別可否・`applies`/`rejection`）、
`syn.project.new` の optional `ppqn`（既定 960）。

Issue #125 の UI 契約（L34）は「New Project 既定 960・
選択可能範囲表示・変更前 Preview（Snap/Grid/SMF export への
影響）・Explicit Confirm/Cancel・Undo。UI は SV-GUI-02/03
（ViewSpec/Binding/Gesture）を消費し別時間モデルを持たない」。
本 slice はこれを headless/ViewSpec レベルで実装し、Snap/Grid
の `SnapUnit::Explicit(960)` が抱える PPQN 変更時の意味変化に
対する追従ポリシーを決定する。

## Scope

### In Scope

- `synvelis-workspace`: `src/ppqn_ui.rs`（新規 read model
  `PpqnUi`/`PpqnPending`/`PpqnPreviewView`/`PpqnConfirm`/
  `PpqnPhase`/`PpqnUiError` + `RECOMMENDED_PPQN`/
  `SMF_EXPORT_NOTE`/`PREVIEW_SITE_CAP` + wire-name helper）、
  `src/snap.rs`（`DEFAULT_SNAP_QUANTUM_TICKS`/
  `SnapPpqnEffect`/`snap_unit_ppqn_effect`）、
  `src/state.rs`（ViewState コメント + 暫定 module mount）。
- `synvelis-app`: `screens.rs`（`PpqnScreenState` + Project
  tab）、`app.rs`（publish 投影・preview feed-back・commit
  re-sync・preview JSON の policy 報告）、`ui_ops.rs`
  （`UiOpsState.ppqn_ui` + `ppqn_*` gesture helper +
  undo/redo re-sync）、`binding.rs`（`project_preview_ppqn`/
  `project_set_ppqn`/`project_undo`/`project_redo` mint）。
- headless fixture: `crates/synvelis-app/tests/
  tim01_ppqn_ui.rs`（新規 11 件）。

### Out of Scope

- 実 GUI widget intent（`rvf.ui.command.v1` の parameterized
  Action）による confirm/cancel — shell の Action 経路は
  params を運べず、fake button で誤魔化さない。実 GUI 配線は
  後続 slice・外部 gate。
- SMF export 実装の変更（`effects.smf_export` は報告のみ）。
- `synvelis-project` の PPQN コマンド意味変更。
- Presentation Driver 時刻正本移動・実機/製品 Oracle。
- `synvelis-workspace/src/lib.rs` の変更（lease — coordinator
  が統合時に `pub mod ppqn_ui` へ移設）。
- `synvelis-gateway`/`synvelis-player`/`synvelis-library`/
  `controller.rs`/`store.rs` の変更。

## Components and Boundaries

- **`ppqn_ui.rs`（workspace）**: view 所有の gesture 状態機械。
  `PpqnUi { current_ppqn, pending }` が
  `Idle → Selected → Previewed` を遷移管理する。
  - `select(new_ppqn, policy)`: `ppqn_in_range`（project 層の
    `1..=32767` 定義を再利用、範囲ルールを重複実装しない）で
    view 側 reject → pending 確立。再 select は pending を
    置き換え旧 preview を破棄。
  - `preview_params()`: pending から `syn.project.preview_ppqn`
    の wire params を生成（policy/rounding は
    `ppqn_policy_wire` の単一語彙）。
  - `record_preview(&PpqnChangePreview, revision)`:
    `new_ppqn` が pending candidate・`old_ppqn` が live
    PPQN と一致する場合のみ記録 — 不一致は
    `PreviewMismatch` で拒否し confirm を arm しない。
  - `confirm_request()`: `preview.applies` のときのみ
    `PpqnConfirm`（old/new/policy/rounding +
    `expected_revision = preview 時 revision`）を返す —
    confirm は必ず preview を経て preview 時 CAS を同梱する。
  - `cancel()`: pending を破棄（非破壊 — 何も apply されて
    いない）。`observe_current(ppqn)`: live PPQN の外部変化を
    検出し pending を invalidate。`committed(ppqn)`:
    `set_ppqn` 成功・project new/open で pending 消費 +
    live 値更新。
  - `PpqnPreviewView::from_preview`: `PpqnChangePreview` を
    表示投影に変換 — サイト一覧は `PREVIEW_SITE_CAP = 32` で
    bounded（件数は正確、行のみ cap）、`project_revision`
    staleness guard、`SnapGridEffect`（既定量子の tick 数 +
    beat 間隔 before/after）、`SMF_EXPORT_NOTE`。
  - 時刻変換・サイト列挙・可否判定は一切持たない —
    `synvelis-project` の結果を消費するだけの projection
    + gesture-state 層。
- **`snap.rs`**: `DEFAULT_SNAP_QUANTUM_TICKS = 960` を
  `GridConfig`/`SnapConfig` default と PPQN 効果報告で共有。
  `SnapPpqnEffect`/`snap_unit_ppqn_effect(unit, old, new)` は
  ポリシーの観測可能な半分 — Explicit は tick 数維持で
  `beats_before = ticks/old_ppqn`・`beats_after =
  ticks/new_ppqn` を報告、Auto は全 `None`（zoom-derived）。
- **`state.rs`**: `ViewState.grid`/`snap` のコメントに
  tick-preservation ポリシーを明記。`#[path = "ppqn_ui.rs"]
  pub mod ppqn_ui` + re-export で暫定 mount — `lib.rs` は
  sv107 lease のため coordinator が統合時に top-level module
  へ移設する（`#[path]` は本 file の dir 基準で
  `src/ppqn_ui.rs` に解決され、canonical 配置は不変）。
- **`screens.rs`**: `PpqnScreenState { current_ppqn,
  pending_new_ppqn, pending_policy, preview }` —
  `main_screen_spec` に Project tab（`tab.project.panel` >
  `ppqn.status`/`ppqn.preview`/`ppqn.effects`/`ppqn.sites`）
  を末尾追加。全 component は read-only 表示（Label/Table、
  `selection: None`、action なし）— spec は `MainScreenState`
  からの投影であり gesture authority を持たない。
- **`ui_ops.rs`**: `UiOpsState.ppqn_ui: RefCell<PpqnUi>` —
  `RefCell` は `publish_project_length(&self)` が唯一の
  全 mutation path 共通 re-sync 点だから（`Service::call`
  tail・`main_screen_cell` hand-out・fixture-level
  `session.apply` までカバー）。`ppqn_select`/
  `ppqn_preview_pending`/`ppqn_confirm`/`ppqn_cancel`/
  `ppqn_ui()` accessor を追加 — 各 helper は live PPQN を
  `observe_current` で再同期してから gesture を進める。
  `op_project_undo`/`op_project_redo` 成功時も re-sync。
- **`app.rs`**: `publish_project_length` が
  `observe_current` + pending preview（revision guard 付き）
  を `screen.ppqn` に投影。`project_preview_ppqn` は preview
  計算後に `observe_current` + `record_preview` を feed
  back（pending 不一致・bare preview は記録しない）。
  `project_set_ppqn` 成功で `committed`、project
  new/open で `committed`（同一 PPQN でも pending は
  別 project の preview を指すため必ず消費）。
  `ppqn_preview_json` の `effects.snap_grid` は決定済み
  ポリシーを `snap_unit_ppqn_effect` で定量化して報告。
- **`binding.rs`**: `ViewBinding` に `project_undo`/
  `project_redo`/`project_preview_ppqn(params)`/
  `project_set_ppqn(params)` を追加 — view が mint するのは
  実 `syn.project.*` intent のみで、gesture 状態は view 側
  `PpqnUi` が持つ（adapter は translation layer のまま）。

## Snap/Grid ポリシー決定（本 slice の核心）

**決定: `SnapUnit::Explicit` は project tick 数を verbatim
維持する。PPQN 変更で変わるのは musical（beat）間隔のみ。**

`SnapUnit::Explicit(i64)` の契約は「gesture domain の宣言
基底単位（project tick）で固定された量子 — zoom に追従しない」
（`snap.rs` doc）。PPQN は musical 時間の grid 解像度であり、
tick は既に「PPQN 分解能での離散位置」として永続化される
Rational Beat 表現の語彙である。したがって:

- **明示量子は tick 固定のままが正しい既定動作** —
  `Explicit(960)` は「960 tick 刻みでスナップする」という
  編集者の選択であり、PPQN を 480 に変えても 960 tick は
  依然として同じ tick 刻み（beat 意味は 1 → 2 beat に変わる
  が、これは PPQN 変更の必然的帰結で preview が報告する）。
- **rebinding（beat 等価で再計算）は lossy で安全でない** —
  任意の tick 量子を「同じ beat 間隔」に張り替えると、新
  PPQN での tick 数が整数にならない組合せが普通にある
  （例: `Explicit(100)` @960 = 0.1041… beat → @1000 では
  104.166… tick で正確に表現不能）。保存済み preset
  （`preset.rs` の `PresetViewState`）は project command
  から書き換えられず、view-local state と persisted
  preset でポリシーが乖離する危険がある。
- **Auto は zoom-derived で固定量子を持たない** —
  追従対象が存在しない。

実装は「維持 + 報告」: 保存値は一切書き換えず、
`snap_unit_ppqn_effect` が tick 数不変・beat 間隔 before/
after を計算して preview/screen spec の informational
表示に供する。per-view の任意 `Explicit` 量子も同一規則
（`ViewState.grid`/`snap` は PPQN に追従しない —
state.rs コメントで契約明記）。

## Gesture Lifecycle と CAS/staleness

```
select(new_ppqn, policy)          // view-side range check only
  → preview_params()              // syn.project.preview_ppqn wire
  → [op] preview_ppqn(project)    // non-destructive, &Project
  → record_preview(preview, rev)  // candidate/live-PPQN guard
  → confirm_request()             // applies のみ + expected_revision
  → [op] syn.project.set_ppqn     // double CAS + session gates
  → committed(new_ppqn)           // or observe_current on external change
cancel()                           // pending drop — never mutates
```

- preview は revision を記録し、confirm はそれを
  `expected_revision` に同梱 — project が preview 後に
  変化していれば `set_ppqn` の CAS が正直に reject する。
- `observe_current` は live PPQN の変化で pending を
  invalidate（undo/redo/open/new/fixture apply を網羅 —
  `publish_project_length` が全 mutation path の後に走る
  唯一の `&self` 共通点）。
- preview の `project_revision` は screen 投影の表示 guard、
  `expected_revision` は mutation の CAS guard — 二重に
  stale confirm を防ぐ。

## Op wire 契約（消費のみ・新規 op なし）

```
syn.project.preview_ppqn   // 既存 — new_ppqn/policy(/rounding)
syn.project.set_ppqn       // 既存 — old/new/policy(/rounding)/expected_revision?
syn.project.undo|redo      // 既存 — gesture re-sync の対象
syn.project.new            // 既存 — optional ppqn（New Project 既定 960）
```

`ViewBinding` はこれらを `Intent` として mint するだけ —
gesture 状態機械は view 側 `PpqnUi`、権威は project 層。

## Data and Control Flow

1. View/test が `app.ppqn_select(new, policy)` →
   `PpqnUi::select`（範囲 reject は `PpqnUiError::OutOfRange`）。
2. `app.ppqn_preview_pending()` → `observe_current` →
   `preview_params()` → `project_preview_ppqn(params)`
   （実 op 経路: `preview_ppqn(&Project)` → JSON +
   `record_preview` feed-back）→ preview JSON 返却。
3. `publish_project_length`（service tail / cell hand-out）が
   `observe_current` + pending preview を `screen.ppqn` に
   投影 → `main_screen_spec` が Project tab を再構成。
4. `app.ppqn_confirm()` → `observe_current` →
   `confirm_request()`（`applies` のみ）→ `project_set_ppqn`
   → 成功で `committed`。失敗（stale CAS・session gate・
   inexact under exact）は pending を残し正直に reject。
5. `app.ppqn_cancel()` → `cancel()` — pending 破棄のみ。

## Non-Functional Constraints

- 第二の時間モデル禁止 — beat 表示は project PPQN からの
  導出テキストのみ、tick 変換は project 層に一元化。
- 検証ロジックの重複禁止 — 範囲は `ppqn_in_range`、
  可否は `preview.applies`、変換は `reproject_ppqn`。
- spec は表示専用 — `PpqnScreenState`/`PpqnPreviewView` は
  gesture authority を持たず、actionButton を持たない
  （`rvf.ui.command.v1` Action は params を運べない制約を
  fake で迂回しない）。
- `PpqnUi` は session-scoped view state — 永続化しない、
  project を mutate しない。

## Risks and Trade-offs

| リスク | 対策 |
| --- | --- |
| `lib.rs` lease で `ppqn_ui` が `state::ppqn_ui` に暫定 mount | `#[path]` で canonical file 配置を維持、coordinator が統合時に top-level module へ移設（doc に明記） |
| pending preview が非 PPQN 変化で stale 化 | `project_revision` を screen guard + `expected_revision` CAS の二重防御 — confirm は正直に reject、view は再 preview で回復 |
| fake parameterized widget で gesture を誤魔化す誘惑 | Action 経路が params を運べないことを確認済み — display-only spec + typed op helper + ViewBinding mint に限定し、実 GUI 配線は外部 gate として明示 |
| `PpqnUi` が app と view で二重化 | `UiOpsState` に単一 instance、`publish_project_length` の `observe_current` が唯一の re-sync 点 — fixture apply を含む全経路を網羅 |
| snapshot/preset との乖離 | tick-preservation は `ViewState`（view-local・session）と persisted preset の双方に同一規則を適用 — preset 書き換えは行わず報告のみ（preset.rs は lease 外） |

## Deferred Design Concerns

- 実 GUI widget intent による select/confirm/cancel —
  `rvf.ui.command.v1` の Action params 拡張または
  dedicated gesture channel が必要（後続 slice・外部 gate）。
- `synvelis-workspace/src/lib.rs` への top-level
  `pub mod ppqn_ui` 移設 — coordinator 統合作業。
- `synvelis-app/src/lib.rs` の `screens::` re-export に
  `PpqnScreenState` 追加 — 同上（lease 外）。
- SMF export division の project PPQN 連動 — 別 slice
  （現状は明示 option・報告のみ）。
- Presentation Driver 時刻正本移動・実機/製品 Oracle —
  別 slice（NotRun）。

## Verification Notes

- [x] `cargo test -p synvelis-app --test tim01_ppqn_ui` —
      11 件 green（workspace read model・app gesture・
      screen spec・snap/grid policy）
- [x] `cargo check -p synvelis-workspace -p synvelis-app
      --all-targets` — error/warning なし
- [x] `mise run` 全検証 lane — 全 9 lane pass、
      結果は `docs/impl/ir/tim-01-ppqn-ui.md` に記録
