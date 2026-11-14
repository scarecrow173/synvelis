---
id: "EXP-AUTX07GuiViews"
type: "experiment"
status: "completed"
title: "AUTX-07 GUI views 実験ログ（synvelis#266 headless slice）"
created: "2026-11-16"
updated: "2026-11-16"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/266"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/automation_ui.rs"
      - type: "file"
        path: "crates/synvelis-app/src/automation.rs"
      - type: "file"
        path: "crates/synvelis-theme/src/status.rs"
      - type: "file"
        path: "docs/impl/ir/autx-07-gui-views.md"
      - type: "file"
        path: "docs/impl/exp/autx-07-gui-views.md"
      - type: "file"
        path: "docs/test-specs/autx-07-gui-views.md"
      - type: "file"
        path: "evidence/sv266/README.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/state.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/view.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-theme/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/autx-07-gui-views.md"
  implemented-by: []
  depends-on:
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-atomic-macro.md"
    - "docs/automation/design-workflow-run.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-07-gui-views.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/autx-07-gui-views.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "実 GPUI/実 OS lane（synvelis#114/#267）"
    - "Script Editor 同一 source 照合（synvelis#274）"
    - "assets/themes への status-* token 値追加（範囲外）"
    - "synvelis-automation live adapter 接続（#264）"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "workspace projection / app ViewSpec の 2 段分離（gateway 非依存維持）"
      - "palette category は Tabs でなく Button 列（component key 重複回避）"
      - "macro_step_requests の step 毎 availability 再検査"
      - "review-protocol 指摘の是正: Pending phase の cancel axis、capture_target の空 id 拒否、JobCancel の共有 fail-closed gate + tagged operation 検証 + 一意解決、preset executable の authority field 再導出"
    rejected:
      - "app→gateway dev-dependency による fixture 直接駆動"
      - "Workflow Editor への action button 追加（approval mint は authority 側）"
---

# AUTX-07 GUI views experiment log

Issue: https://github.com/scarecrow173/synvelis/issues/266

## 採用した設計判断

### 1. projection を workspace、spec を app に分離

`synvelis-workspace::automation_ui` が Gateway/Automation 権威を
read model（`ActionCatalogView`/`MacroBrowserView`/`WorkflowPlanView`/
`RunMonitorView`）へ投影し、`synvelis-app::automation` がそれを
`ViewSpec` へ写像する 2 段構成を採用した。

- 理由: `synvelis-app` は `synvelis-gateway` に直接依存しない
  （Cargo.toml 変更は編集範囲外かつ依存追加は ask 必須）。workspace
  側は既に gateway 依存があり、権威型をここで display model に
  落とせば app は文字列/serde 投影だけで spec を組める。
- 棄却案: app に gateway dev-dependency を足して fixture 駆動 —
  依存方向を緩めるだけで利益がない。

### 2. カテゴリ切替は Tabs ではなく Button 列

palette の domain category を `TabsSpec` でなく `Button`（`category`
action + `{"index": n}`）で実装した。Tabs は各 tab が child tree を
持つため、共有 table を tab 内に置くと component key が重複し
`ViewHost::mount` の spec validation に落ちる。Button 列なら table
は 1 key のまま sibling に置け、選択状態は status label に表示する。

### 3. Workflow Editor v1 は表示のみ

`WorkflowPlanView` は step 表 + validation verdict + approval 文字列
を運ぶだけで、button/command を一切生成しない。承認 mint・run 起動は
Gateway/Automation 側の責務（GW-09 表示契約）であり、GUI が authority
を作らないことを spec 構造で固定した。

### 4. Job cancel は descriptor 解決 fail-closed

`JobCancel` action は `synvelis_gateway::JOB_CANCEL_ID` の descriptor
を catalog から探し、見つからなければ `UnknownCapability` を返す。
request args は `{"operation": <id>}`、target は `None` — cancel の
request と confirmed の区別は `CancelLink` 軸が担い、GUI は confirmed
を捏造しない。

### 5. `macro_step_requests` に step 毎 availability 検査を追加

初版は preset 全体の `executable` flag のみ見ていたが、Retired/Disabled
な step capability が request に化ける経路があった。palette の
`single()` と同じ fail-closed 規則（`Available` のみ lowering）を
step 毎に適用するよう修正し、回帰 test
（`macro_apply_rejects_unknown_preset_and_non_executable_step`）を追加。

## 試して棄却したもの

- `AutomationViewAction` の serde derive — action は in-process の
  decode 産物であり wire を越えないため `PartialEq` のみ。
- `AutomationViewState` への `deny_unknown_fields` — 採用。
  `ViewState` の tolerant deserialize（`serde_json::Value` 経由）と
  組み合わせ、newer snapshot の未知 field は `None` degrade になる
  ことを test で固定した。

## 検証証跡

- `cargo test -p synvelis-workspace --test autx07_automation_ui`:
  29 passed（fail-closed matrix・target capture・request lowering・
  job status・serde tolerant）。
- `cargo test -p synvelis-app --test autx07_automation`:
  7 passed（decode matrix・empty-state spec・mount）。
- `cargo test -p synvelis-theme`: 17 passed（status vocabulary 3 +
  既存 contract 14）。
- `cargo clippy --locked --all-targets`（3 crate）: warning 0。
- 実 OS/GPUI lane・#274 Script Editor・automation crate live 接続は
  NotRun/Deferred（`evidence/sv266/README.md`）。
