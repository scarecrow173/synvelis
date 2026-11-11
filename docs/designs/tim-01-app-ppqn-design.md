---
id: "DESIGN-034VtzPNCX6aVKTaxy71ke"
type: "design"
status: "approved"
title: "TIM-01 app 層 PPQN 統合 — syn.project.set_ppqn 設計"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/specs/tim-01-app-ppqn.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim-01-app-ppqn.md"
    - "docs/designs/tim-01-configurable-ppqn-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-01-configurable-ppqn-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-01-app-ppqn.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-app-ppqn.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 app 層 PPQN 統合 — syn.project.set_ppqn 設計

## Context

TIM-01 headless slice は `ProjectOp::SetProjectPpqn`（expected_revision
CAS + `old_ppqn` ドメイン CAS + `1..=32767` + open-recording 拒否 +
scratch clone atomic validation）を `synvelis-project` に実装済み。
`synvelis-gateway` の op 名 `set-project-ppqn` も登録済み。
残るのは app 層統合のみ: 製品経路（`syn.*` service op）から
PPQN を変更する手段がなく、設計上 app 層責務とされた前提条件
（transport-stopped・automation-write・merge-commit）が未検証。

`docs/designs/tim-01-configurable-ppqn-design.md` の責務境界:
「synvelis-app / gateway: transport-stopped・automation-write・
merge-commit などの UI/セッション前提条件。本 slice では
`synvelis-project` に重複実装しない。」

## Scope

### In Scope

- `synvelis-app` の `syn.*` service 面への `syn.project.set_ppqn`
  op 追加（`service.rs` の `OPS` + `dispatch`、`app.rs` の
  `SynvelisApp` メソッド）。
- app 層前提条件の実装と拒否。
- `project_info` への `ppqn` 投影。
- headless fixture `crates/synvelis-app/tests/tim01_ppqn_app.rs`。

### Out of Scope

- PPQN 選択 UI widget・Preview/Confirm フロー（別 slice）。
- Presentation Driver への時刻正本移動（別 slice）。
- `synvelis-workspace`/`synvelis-gateway`/`synvelis-project` の
  変更 — public accessor は完備、gateway op 名は登録済み。
- 実 OS/実 GUI・製品 Oracle。

## Components and Boundaries

- **`service.rs`**: `OPS` に `OpInfo` 1 件追加、`dispatch` に
  `"syn.project.set_ppqn" => self.app.project_set_ppqn(params)`
  を追加。`syn.project.*` は `self.app.*` 呼出の既存パターン
  （`syn.project.new`/`open`/`save`/`reload`/`info` と同型）。
- **`app.rs`**: `SynvelisApp::project_set_ppqn(&mut self,
  params: &Value) -> Result<Value, ErrorBody>` を追加。
  `project_info` に `"ppqn": project.ppqn.0` を追加。
- **`synvelis-workspace`（変更なし・利用のみ）**:
  `ProjectSession::transport()` → `TransportState.playing`、
  `ProjectSession::control()` → `ControlSession::{recording,
  record_enabled}`、`ProjectSession::apply(op, expected_revision)`
  → controller CAS + `engine.on_project_revision` 通知。
- **`synvelis-project`（変更なし・利用のみ）**:
  `ProjectOp::SetProjectPpqn`、`PpqnConversionPolicy`、
  `PPQN_MIN`/`PPQN_MAX`/`ppqn_in_range`。

## Op wire 契約

```
syn.project.set_ppqn
params:
  old_ppqn:          u64 → u32, required   // domain CAS
  new_ppqn:          u64 → u32, required   // target PPQN
  policy:            "exact" | "round", required
  rounding:          "floor" | "ceil" | "toward_zero" |
                     "away_from_zero" | "nearest_even",
                     required iff policy == "round"
  expected_revision: u64, optional          // controller CAS;
                                           // default = live revision
```

- `policy` は必須 — `PpqnConversionPolicy` の doc 契約
  （"The policy is required command input — a PPQN change can
  never silently quantize"）を wire でも維持し、暗黙の `Exact`
  default を置かない。
- `rounding` は `policy == "round"` のときのみ必須。
  `policy == "exact"` で `rounding` が指定された場合は
  `invalid_params`（矛盾 param を黙って無視しない）。
- `expected_revision` 省略時は live revision を使う
  （`syn.monitoring.*` の write-through と同じ既定）。指定時は
  controller CAS が stale を拒否する — wire 経路での
  stale-revision 検証を可能にする。
- 成功時の返り値は `project_info()`（`ppqn`・`revision`・
  `saved` を含む）— `syn.project.new/open/save` と同じ形状。

## 前提条件（app 層 gate）

`session.apply` の前に `SynvelisApp` が検査する。いずれか違反で
`invalid_params` を返し project は無変更:

| 条件 | accessor | 拒否理由 |
| --- | --- | --- |
| transport 停止中のみ | `session.transport().playing == false` | transport playing — RequiresStop 相当 |
| merge-commit in flight なし | `session.control().recording().is_none()` | open recording transaction（PC-C1 merge-commit 未完了） |
| automation-write なし | `!session.control().record_enabled()` | automation record enabled — write 供給源が armed |

判定順は transport → recording transaction → record_enabled。
open transaction は `record_start` が `record_enabled` を要求
するため、record_enabled を先に検査すると transaction 検査が
到達不能になる。より具体的な「merge-commit in flight」を
先に報告する。

### accessor 選定の根拠

- **`session.transport().playing`**: `syn.transport.set` が動かす
  製品 transport stub。Issue #125 契約の「Transport停止」に
  対応。`ControlSession::playback()` は automation/control 側の
  別 transport（BeatPoint 領域）であり、rolling であっても
  project mutation を生まない（automation 評価は read、
  write は `record_enabled`/open transaction が gate 済み）。
  よって本 gate の対象外とし、design で明示する。
- **`session.control().recording()`**: open `RecordingTransaction`
  = PC-C1 の未 commit merge（fixed_revision CAS を持つ書込み
  transaction）。PPQN 変更は revision を進めるため、in-flight
  transaction の commit と競合し得る — fail-closed で拒否。
- **`session.control().record_enabled()`**: automation record の
  session toggle。`true` の間は transport rolling 時に write が
  生産され得るため「Automation Writeなし」の保守的解釈として
  拒否。transport 停止必須との組合せで実害は限定的だが、
  Issue 契約の fail-closed を優先する。

### エラー写像

- params 不正・前提条件違反 → `invalid_params`（`syn.monitoring.*`
  が domain 拒否に使う既存 convention。wire に `busy`/
  `requires_stop` code は存在しない）。
- `session.apply` の `synvelis_project::Error`（CAS mismatch・
  範囲外・open recording session・変換拒否）→ `invalid_params`
  に `"syn.project.set_ppqn: {e}"` で写像
  （`op_monitoring_set` と同じ写像）。

## Data and Control Flow

1. `UiRuntime::dispatch_op("syn.project.set_ppqn", params)`
   → `AppService::call` → `dispatch` →
   `SynvelisApp::project_set_ppqn(params)`。
2. params 検証（`old_ppqn`/`new_ppqn`/`policy`/`rounding`/
   `expected_revision`）→ `PpqnConversionPolicy` へ写像。
3. 前提条件 3 件を session accessor で検査（違反 →
   `invalid_params`、project 無変更）。
4. `session.apply(ProjectOp::SetProjectPpqn{..}, expected)` —
   controller が expected_revision CAS → validate（old_ppqn CAS・
   範囲・open-recording・scratch 再投影）→ apply（再投影）→
   `engine.on_project_revision` 通知 → control session の
   foreign-change seal。
5. `project_info()` を返す（`ppqn` 投影済み）。
6. `AppService::call` tail の `publish_project_length` が
   main-screen 投影を更新（既存機構）。

Undo/Redo は `syn.project.undo`/`syn.project.redo`（既存 op）が
controller history を辿る。Save/Reload は `syn.project.save`/
`syn.project.reload`/`syn.project.open` の既存 bundle 経路。

## Non-Functional Constraints

- 検証ロジックの重複実装禁止 — app 層は前提条件と policy 写像
  のみを持ち、CAS/範囲/変換拒否は `synvelis-project` の
  authoritative 実装に委ねる。
- op は non-RT 境界（UI/host thread）で動作。service call tail
  の `publish_project_length` が投影を更新する既存契約を維持。
- `1..=32767` 範囲の再検証は project 層が行う（app 層で
  範囲を再実装しない）。

## Risks and Trade-offs

| リスク | 対策 |
| --- | --- |
| `record_enabled` 拒否が厳しすぎる（transport 停止中は write が生じない） | Issue 契約は「Automation Writeなし…の状態で許可」と fail-closed を要求。緩和は後続 slice で spec 改訂を要する判断として記録 |
| `control.playback` Playing を gate しない判断の誤り | automation write は `record_enabled`+open transaction で gate 済み、評価は project を変更しない — design で明示し test-spec に境界を記す |
| `expected_revision` 省略時の利便性と stale 検証の両立 | 省略時は live revision（monitoring write-through と同型）、指定時は CAS が効く — test で両経路を検証 |
| op 名・param 形状の後続 UI との齟齬 | PPQN 選択 UI は別 slice。wire 契約は本 design で固定し、UI slice はこれを消費する |

## Deferred Design Concerns

- PPQN 選択 UI widget・Preview/Confirm — 別 slice
  （Issue #125 の UI 受入は GUI slice の責務）。
- Presentation Driver 時刻正本移動 — 別 slice。
- 実機/製品 Oracle — 別 slice（NotRun 継続）。

## Verification Notes

- [x] `crates/synvelis-app/tests/tim01_ppqn_app.rs` が wire CAS/
      stale 拒否・playing 中拒否・transaction open 中拒否・
      record_enabled 拒否・範囲外拒否・undo/redo・save/reload・
      engine revision 通知・`project_info.ppqn` を検証する
      （11 tests green）
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
      `test-msrv`/`check-app-native`/`lint-app-native` が exit 0
