---
id: "PLAN-034UqTzD6N7R0o468wXtrQ"
type: "plan"
status: "approved"
title: "Implement SV-GUI-04 slice2 dock transfer workspace adoption"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
  implemented-by: []
  depends-on:
    - "docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md"
    - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
    - "docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md"
    - "docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  derived-by: []
  verifies: []
  verified-by: []
  test-spec-skip: "docs-only slice: 本 plan は実装 slice（task/test-spec/impl 文書は実装 dispatch で作成）の先行設計文書であり、本スライス自体に検証すべき実装を持たない。Phase の verification は plan 本文の Verification Matrix と design doc の Verification Notes に記録済み"
  references: []
  defers: []
  deferred-by: []
---

# Implement SV-GUI-04 slice2 dock transfer workspace adoption Implementation Plan

## Goal

`synvelis-workspace` の adoption seam（`adopt_window`/`adopt_view`）を
追加し、`synvelis-app` に `syn.window.dock_*` op family・`syn.view.*`
の dock-aware 拡張・`NativeShell` lifecycle forwarding・`Destroyed`
disposition flow を実装して、Issue #113 の dock transfer（Detach→
Reattach・move abort/rollback・window 生成失敗の安全復帰・非破壊
guarantee）を headless/fake fixture で検証可能にする。

## Scope

**1 本の実装 stream** — workspace API → app op → shell lifecycle →
fixture → env-gated native の順に依存し、同一 release intent
（Issue #113 dock transfer の GUI op 配線）を共有するため分割しない。
ただし各 Phase は独立に merge 可能な単位で区切り、Phase 4
（env-gated native 証拠）は `SYNVELIS_SECOND_WINDOW` 系 gate の
既存パターンに従う別 dispatch でもよい。

**本 plan は docs slice（spec/ADR/design/plan）の成果物であり、
実装は後続 dispatch が本 plan に従って行う。** 本 docs slice 自体は
コードを一切変更しない。

## Upstream Documents

- Spec: `docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md`
- ADR: `docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md`
  （決定 D1–D5）
- 設計正本: `docs/designs/workspace-docking-state-architecture.md`
  §7.1/§7.2

## Design Inputs

- `docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md`
  — adoption API 署名・op family 表・`UiOpsState.dock_moves`・
  wire codec・`syn.view.*` 拡張順序・`NativeShell` forwarding・
  `Destroyed` disposition flow・new-window saga の全詳細。
- `docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md`
  — headless `WindowHandle`/dock fixture の既存契約（前提・
  整合対象）。
- `docs/adr/sv-gui-04-01-two-window-second-view.md` — 2-window
  mount の composition root seam。

## File Map

| Path | Responsibility | Notes |
| --- | --- | --- |
| `crates/synvelis-workspace/src/workspace.rs` | `adopt_window`/`adopt_view` 追加、`next_view` 前進、`DuplicateWindow` error 連携 | `create_window_at`/`spawn_view` の登録部と同構成。`host.create_window` は呼ばない |
| `crates/synvelis-workspace/src/error.rs` | `WorkspaceError::DuplicateWindow(WindowId)` variant 追加（additive） | `Display`/`Error` impl 追従。既存 variant の意味は変えない |
| `crates/synvelis-workspace/tests/`（既存 test file または新規 `adopt.rs`） | adoption 単体 test | `sv113_dock` 系 fixture を再利用 |
| `crates/synvelis-app/src/ui_ops.rs` | `op_dock_*`/`op_window_adopt`/`op_window_list`、`UiOpsState.dock_moves`/`DockMove`、wire codec helper、`syn.view.*` dock-aware 拡張、`ViewEntry` pool 表現 | `syn.*` op の params/result/`ErrorBody`/`codes` 規約に従う |
| `crates/synvelis-app/src/service.rs` | 新 op の dispatch 登録 | `syn.window.dock_*`/`syn.window.adopt`/`syn.window.list` |
| `crates/synvelis-app/src/app.rs` | workspace への薄い accessor（adopt/on_window_event/session_mut 経路）、`window_close_veto` read API、lifecycle forwarding 用内部 op または直接経路 | sealed app の公開面を最小化 |
| `crates/synvelis-app/src/shell.rs` | `on_windowed` lifecycle → workspace forwarding、`on_destroyed` disposition flow 改訂、`observe_result` の Detach owner クリア、new-window `WindowRequest` drain + orchestration | `CloseRequested` 分岐は close-veto dispatch との差分前提に注意 |
| `crates/synvelis-app/src/screens.rs` | view_id → `ViewType`/title 解決 helper、view spec → `ViewHost` 生成の一般化（new-window mount 用） | 既存 `main_view_id`/`secondary_view_id` lockstep を維持 |
| `crates/synvelis-app/src/bin/synvelis-native.rs` / `synvelis.rs` | 必要に応じ new-window mount 経路の配線 | 変更は最小限、既存 mount を壊さない |
| `crates/synvelis-app/tests/`（新規 `dock_transfer.rs` 等） | fake/headless fixture test | `FakeDriver` + `NativeShell` + op dispatch で全 saga を検証 |
| `crates/synvelis-app/tests/two_window.rs` / `sv113_dock.rs` | 既存 test の additive 回帰確認（原則無変更） | 破壊的変更が必要になったら設計を見直す |

## Implementation Sequence

### Phase 1 — synvelis-workspace adoption seam

- [ ] `WorkspaceError::DuplicateWindow(WindowId)` variant を `error.rs` に追加し `Display`/`Error` を実装する。
- [ ] `WorkspaceManager::adopt_window(id, title, geometry)` を `workspace.rs` に実装する — 重複拒否 → `dock.register_window` → `WindowRecord{veto_close:false, disposition:DetachViews}`。
- [ ] `WorkspaceManager::adopt_view(id, kind, title, target: Option<DockTarget>)` を実装する — `registry.contains` 重複拒否（`DockError::DuplicateView`）→ `dock.attach_view`/`add_detached` → `registry.register` → `states.ensure` → `next_view = max(next_view, id.get())`。
- [ ] adopt の失敗経路を確認する — `adopt_view` が `dock.attach_view` で失敗した場合 registry/states を汚さない順序（dock 先・registry 後）を守る。
- [ ] `crates/synvelis-workspace` の test に adoption case を追加する — 重複 window/view 拒否、pool/window 両 target、`next_view` 前進（adopt view-1 後の `spawn_view` が `ViewInstanceId(2)` 以降を mint）、`host.create_window` 非呼出し（`FakeWindowHost`/`HeadlessWindowHost` の create log が空であること）。
- [ ] `mise run check` → `mise run lint` → `mise run test`（workspace 関連 target）を PASS させる。

### Phase 2 — synvelis-app op 層（dock_moves + op family + view.* 拡張）

- [ ] `ui_ops.rs` に wire codec helper（`parse_window_id`/`parse_view_instance`）を追加する。
- [ ] `UiOpsState` に `dock_moves: HashMap<String, DockMove>` と `DockMove::{Prepared{token,target}, AwaitingWindow{title}}` を追加する。
- [ ] `op_window_adopt`（`syn.window.adopt`）を実装する — params 検査 → `adopt_window` → result。重複は冪等成功または `DuplicateWindow` error（冪等を選ぶ — `Created` 再送・fixture 再現のため）。
- [ ] `op_window_list`（`syn.window.list`）を実装する — windows/docked/detached/pending move の観測面。
- [ ] `op_dock_prepare` を実装する — view adopt 検査 → `begin_transfer` → `dock_moves` 登録。`ViewBusy`/`AwaitingWindow` 中は拒否。
- [ ] `op_dock_commit` を実装する — `Prepared` は `commit_transfer`→UI 付け替え（`controls.attach_view`+`viz.attach_view`+`ui.views` 更新、public `syn.view.attach` op は経由しない）、`AwaitingWindow` は `move_view` one-shot→`dock_moves` 除去。`TargetUnavailable` は dock 自動 rollback に委ね error 返却。
- [ ] `op_dock_abort` を実装する — `Prepared` は `abort_transfer`、`AwaitingWindow` は `dock_moves` 除去 + UI 復帰（source window が live なら再 attach、死んでいれば detach のみ）。
- [ ] `op_dock_detach`/`op_dock_reattach` を実装する — `detach_to_pool`/`move_view` + UI detach/attach の one-shot 版。
- [ ] `op_dock_prepare_new_window` を実装する — `AwaitingWindow` 登録 + `WindowRequest` を共有 cell（`outbox`/`clock`/`quit` と同型 `Rc<RefCell>`）へ push。
- [ ] `syn.view.attach`/`detach`/`close` を dock-aware に拡張する — **dock-domain scoping**（`view-{n}` parse 可能かつ composition root が `ViewType`/title metadata を持つ view のみ。`"console"`/`"meters"` 等の任意 view_id は legacy UI-only 経路を維持）を実装し、design doc「`syn.view.*` の dock-aware 拡張」の順序（workspace 更新 → UI 更新 → notify）に従う。`AwaitingWindow` 中は pending destination window への attach のみ UI-only で許可。`ViewEntry`/`shell.views` の pool 表現を additive に拡張する。
- [ ] `syn.view.close` は transfer 中 view を `dock_abort` 相当で先に解除してから `remove_view` する順序を実装する。
- [ ] `service.rs` に全 op を登録する。
- [ ] `app.rs` に workspace forwarding accessor（`adopt_window`/`adopt_view`/`on_window_event`/`session_mut` の既存経路確認を含む）と `window_close_veto` read API、`WindowRequest` queue の `pub` accessor を追加する。
- [ ] op 層の単体/fixture test を追加する — prepare→commit/abort、pool detach/reattach、`AwaitingWindow` の pending 可視化と busy 拒否。
- [ ] `mise run check`/`lint`/`test` を PASS させる。

### Phase 3 — NativeShell lifecycle + Destroyed disposition + new-window saga

- [ ] `shell.rs` の `on_windowed` に lifecycle forwarding を追加する — `Created`→`adopt_window`（spec→`WindowGeometry` helper 経由）、`Resized`/`Moved`/`Focused`→`HostEvent` 相当、`ScaleFactorChanged`→記録のみ（workspace 非写像）。app への到達経路（直接 method or 内部 op）を確定して実装する。
- [ ] `observe_result`/`track_view_op` を拡張する — `ViewOp` に `DockCommit`（owner は result `data["to"]` から解決）/`DockReattach`（params `window_id`）/`DockDetach`（owner クリア）を追加し、`ViewOp::Detach` を「association クリア」へ変更する（pool view の `origin_view` command を fail closed に）。design doc「shell bookkeeping」節の規則に従う。
- [ ] `on_destroyed` を disposition flow に改訂する — `workspace.on_window_event(Destroyed)` → `drain_dock_events` の `WindowRemoved.views` に対し `DetachViews`→`syn.view.detach` replay（`syn.view.close` は replay しない）、`ReleaseViews`→`syn.view.close` replay（現行）。`RelocateTo` は本 slice では到達不能（setter 経路を作らず既定 `DetachViews` のみ — design doc Deferred concerns 参照）。
- [ ] `WindowRequest` drain を `handle`/`on_tick` に追加し、new-window saga を orchestration する — `driver.create_window` → `Created` で `adopt_window` → view spec から `ViewHost` mount（`screens.rs` helper）→ `Ready` で `shell.attach_view` → 成功で `dock_commit` 内部 dispatch、失敗で `dock_abort` + `destroy_window`。
- [ ] `CloseRequested` 分岐は現行構造を維持し、veto 判定が `WindowRecord.veto_close` を読む seam（`window_close_veto` accessor）のみ用意する — decision 本体は close-veto dispatch の差分に委ねる。
- [ ] `NativeShell`/`FakeDriver` fixture test を追加する — window `Destroyed`（DetachViews）で view が pool へ退避し `syn.view.close` が replay されない、`ReleaseViews` で除去される、new-window saga の成功/生成失敗/attach 失敗/Ready 前 Destroyed の各 rollback。
- [ ] `tests/two_window.rs`/`sv113_dock.rs` の additive 回帰を確認する（原則無変更で PASS）。
- [ ] `mise run check`/`lint`/`test` を PASS させる。

### Phase 4 — env-gated native 証拠（後続 dispatch 可）

- [ ] `SYNVELIS_SECOND_WINDOW` 系 env-gate に倣い、dock transfer の native 観測 gate を追加する — window 生成失敗注入・close→detach→reattach・move abort の実 OS 証拠。
- [ ] headless/fake Pass を実 OS Gate へ昇格しない記録方針を test-spec/impl doc に明記する。
- [ ] `RelocateTo`・layout restore（#177）・DPI/IME（#388/#389/#114）・Plugin Dock（#176）は本 plan の対象外として `NotRun` を維持する。

## Dependencies and Risks

### Dependency Graph

| Step | Depends on | Blocks |
| --- | --- | --- |
| Phase 1 (workspace adopt) | — | Phase 2, 3 |
| Phase 2 (op 層) | Phase 1 | Phase 3 |
| Phase 3 (shell lifecycle) | Phase 2 | Phase 4 |
| Phase 4 (native gate) | Phase 3 | — |

Critical path: Phase 1 → 2 → 3 → 4。Phase 1/2 は headless で完結し
Phase 3 で初めて driver 経路に触れる。

### Risk Register

| # | Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- | --- |
| 1 | dock/UI drift（両権威の更新順序ミス） | med | high | 更新を op 層 helper に集約、`syn.window.list`+`syn.view.list` で両権威を assert、全 op 組合せの fixture test |
| 2 | `AwaitingWindow` saga リーク（window 生成要求への無応答） | med | med | `dock_abort` を `AwaitingWindow` からも合法化、window `Destroyed`/timeout で pending 掃除、`syn.window.list` で可視化 |
| 3 | close-veto dispatch との merge 衝突（`shell.rs`/`service.rs`） | med | med | 本 plan は veto record の read seam のみ規定、`on_windowed` の `CloseRequested` 分岐は差分調整前提を明記 |
| 4 | `ViewEntry`/`shell.views` pool 表現変更による既存前提の破壊 | med | med | detach→owner クリアは fail-closed 方向、`tests/two_window.rs`/`sv113_dock.rs` の additive 回帰で検出 |
| 5 | `adopt_view` の kind/title 解決が未知 view_id で破綻 | low | med | 未知 view_id は `invalid_params` で明示拒否（silent skip で権威分界を壊さない） |
| 6 | `WindowGeometry` 単位（physical vs logical px）の混在 | med | low | `Resized`/`Moved` は driver の physical 値をそのまま記録、DPI slice で統一 — impl doc に単位を明記 |

### Rollback Strategy

- **Trigger**: Phase 2/3 で既存 `syn.view.*` 契約または
  `tests/two_window.rs`/`sv113_dock.rs` が回帰した場合。
- **Procedure**: Phase 単位で revert — Phase 3 の shell 変更を
  戻せば op 層は dormant に戻り（adopt が呼ばれなければ
  `WorkspaceManager.dock` は再び空）、Phase 2 の op 追加は
  additive なので service 登録を外せば無害。Phase 1 の
  `adopt_*` は新規 API で既存呼出しが無いため revert が最も
  安全。
- **Verification**: revert 後に `mise run check`/`lint`/`test` と
  `tests/two_window.rs`/`sv113_dock.rs` が main と同じ結果に
  なること。

## Task Breakdown

上記 Implementation Sequence の checkbox が task 粒度
（1 action/step）。Phase 内の順序は記載順が依存順。
独立 stream はなく、Phase 1→4 の直列が critical path。

## Verification

| Step | Verification command or criteria | Pass condition |
| --- | --- | --- |
| Phase 1 | `mise run check`/`lint`/`test`（workspace target） | exit 0、adoption test 全 PASS |
| Phase 2 | `mise run check`/`lint`/`test` + op fixture test | exit 0、prepare→commit/abort・pool detach/reattach・AwaitingWindow busy 拒否が PASS |
| Phase 3 | `mise run check`/`lint`/`test` + shell fixture test | exit 0、Destroyed→DetachViews で `syn.view.close` 非 replay・ReleaseViews で除去・new-window saga 全 rollback が PASS |
| Phase 3 回帰 | `tests/two_window.rs`/`sv113_dock.rs` | 無変更で PASS（additive のみ） |
| Phase 4 | env-gated native run（`SYNVELIS_*` gate） | 実 OS で detach/reattach/abort/生成失敗の観測証拠 — **本 docs slice では NotRun** |
| 全体 | `mise run fmt-check` | exit 0 |

## Open Questions / Gaps

- `NativeShell` → sealed `SynvelisApp` への lifecycle forwarding
  経路（直接 method vs 内部 op）は impl slice で確定 — design
  doc は「意味論」を規定し経路選択を残す。
- `WindowRequest` 共有 cell の具体型（`Rc<RefCell<VecDeque>>`
  か専用 struct か）は impl slice で `outbox`/`quit` の既存
  パターンに合わせて確定。
- `ViewEntry`/`shell.views` の pool 表現（`attached: bool` 追加
  vs `Option<WindowId>`）は impl slice で既存利用箇所を見て確定。
- close-veto dispatch `ctx_2aed081be01f` の成果物が main に
  着いた場合、`on_windowed` の `CloseRequested` 分岐と
  `respond_close` decision の接続点を merge 時に調整する。

## Self-Review

- [x] Every spec requirement maps to a task — spec の 6 決定事項
  （host 扱い・adoption seam・権威分界・Destroyed policy・
  slice 分割・受入対応表）は ADR D1–D5 → design → 本 plan の
  Phase 1–4 に写像済み。
- [x] No placeholders remain — 全 task が具体的 file/behavior/検証を
  持つ。`Open Questions` は impl slice へ明示的に委ねた gap で
  あり placeholder ではない。
- [x] File paths, relations, status values, and terminology are
  consistent — design doc の op 表・API 署名・flow と一致。

## Implementation Handoff

- Recommended mode: inline execution（Phase 1→4 は直列依存で
  独立 stream なし。subagent 分割の利益が薄い）。
- Independent work streams: none（Phase 4 のみ別 dispatch 可）。
- User approval required before dispatch: yes（実装着手は別
  dispatch の判断）。
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute task-by-task
  with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- 想定 reviewer focus: (a) `syn.view.*` dock-aware 拡張の更新順序
  が design doc と一致するか、(b) `AwaitingWindow` saga の
  abort/rollback が全失敗経路を網羅するか、(c) `Destroyed` の
  `DetachViews`/`ReleaseViews` 分岐が `syn.view.close` replay の
  置換として正しいか、(d) close-veto 差分前提が merge 時に
  問題にならないか。
- Upstream: spec → ADR（D1–D5）→ design（op 表・flow・API）。
- 前提として main の `HeadlessWindowHost`/`tests/sv113_dock.rs`/
  `tests/two_window.rs`/`NativeShell::attach_view`/`on_destroyed`
  の現行契約を変更しない。
