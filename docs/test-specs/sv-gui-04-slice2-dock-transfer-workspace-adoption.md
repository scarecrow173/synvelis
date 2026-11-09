---
id: "TSPEC-3hJ8nB5cV1xZ7kM2qW9eRt"
type: "test-spec"
status: "approved"
title: "SV-GUI-04 slice2 テスト仕様 — dock transfer workspace adoption + GUI op 配線"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
    - "docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice2 テスト仕様 — dock transfer workspace adoption + GUI op 配線

## Purpose

`WorkspaceManager` の dock semantics（adoption seam・two-phase
move・detached pool・close disposition）と GUI op/shell 配線の
契約を、FakeDriver + NativeShell + `syn.*` op dispatch の
決定的 headless fixture で固定する。window/view ownership が
三層（workspace dock・`ui.views`・`shell.views`）で drift しない
こと、saga の全失敗経路が source を無傷に rollback すること、
`Destroyed` が disposition に従い view を pool/除去へ正しく
振り分けることを検証する。

**記録上の注意**: 本 spec の全検証は headless/fake の証拠であり、
実 OS window の Pass/Fail 最終証拠（#114 gate）ではない。fake
証拠を native PASS へ昇格させない。

## Feature

```
Feature: Dock transfer over the live GUI op surface
  As a Synvelis composition root
  I want dock-domain views to move between windows through
    syn.window.dock_* ops with WorkspaceManager as the single
    window/view authority
  So that detach/reattach, move abort/rollback, and new-window
    creation failure never lose view, project, or UI state
```

## Rules

- RULE-1: `adopt_window` は host 生成 window を mint せず登録し、
  重複 id を `DuplicateWindow` で拒否する。`adopt_view` は
  caller 指定 `ViewInstanceId` で registry/state を登録し、
  `next_view` を前進させ、dock target への配置を registry 登録
  より先に行う（失敗時に部分登録を残さない）。
- RULE-2: `syn.window.adopt` は冪等 — 一貫 params の再 adopt は
  ack、矛盾 params は error。`syn.window.list` は windows・
  docked/detached views・pending moves を公開する。
- RULE-3: `dock_prepare`→`dock_commit` の two-phase move は
  workspace dock・`ui.views`・`shell.views` の三権威を同一 owner
  に揃える。pending 中の view は busy — attach/detach/他 dock
  op を `invalid_params` で拒否する。
- RULE-4: `dock_abort` は source ownership と UI binding を
  全層で保持する。destination window の `Destroyed` は pending
  move を解放し、後続 `dock_commit` は失敗して source に残る。
- RULE-5: `dock_detach`/`dock_reattach` は detached pool 経由の
  one-shot move。`syn.view.detach` も dock-domain view を pool
  へ送り、`syn.view.list` は `attached:false`/`window_id:null`
  で報告する。
- RULE-6: `dock_prepare_new_window` は `AwaitingWindow` pending
  を立て app→shell `WindowRequest` を push する。shell は
  `create_window` → `Created`/`Ready` lifecycle（`dock_view`
  付き）→ 例外 attach → `dock_commit` の saga を駆動する。
- RULE-7: `AwaitingWindow` 中の `syn.view.attach` 例外は
  lifecycle が名付けた destination window のみ許可 — 他
  window への attach・destination 命名前の attach は busy 拒否。
- RULE-8: saga の失敗経路 — `create_window` 失敗・destination
  の Ready 前 `Destroyed`・attach/commit 失敗 — は全て
  `dock_abort` + orphan window 破棄で source に rollback し、
  pending move・window request・UI binding の leak を残さない。
- RULE-9: window `Destroyed` は記録済み `WindowCloseDisposition`
  を適用する — `DetachViews` は docked view を pool へ退避させ
  `syn.view.close` を replay せず、`ReleaseViews` は view を
  workspace/UI 双方から除去する。
- RULE-10: dock-domain scoping — `view-{n}` かつ metadata 登録済み
  の view のみ dock-aware。`"console"` 等の legacy view_id は
  UI-only 経路を維持し、全 `syn.window.dock_*` op は
  `invalid_params` で拒否する。
- RULE-11: dock transfer/close は Project/Graph/Recording/Job
  state を暗黙停止しない — `syn.project.info` の truth は move
  前後で不変。

## Examples

- EX-1: view-1@w1・view-2@w2 の fixture で
  `dock_prepare{view-1→w2}`→`dock_commit` — shell
  `view_window`・`syn.view.list`・`syn.window.list` の三者が
  w2 で一致し pending_moves は空。pending 中の attach/detach/
  dock op は `invalid_params`。
- EX-2: `dock_prepare`→`dock_abort` — owner は w1 のまま、
  `syn.window.list` に pending が残らない。
- EX-3: `dock_prepare{view-1→w2}` 後に w2 を `destroy_window`
  → `dock_commit` は失敗、view-1 は w1 に残り、w2 の view-2 は
  `DetachViews` で pool 入り（`attached:false`）。
- EX-4: `dock_detach{view-1}`→pool（`attached:false`・
  detached_views 入り）→`dock_reattach{view-1→w2}` で w2 に
  復帰。`syn.view.detach`→`syn.view.attach` でも同じ pool 経路。
- EX-5: `dock_prepare_new_window{view-1,title}`→ shell が
  window を生成・`Created` で adopt・`Ready` で例外 attach +
  commit — view-1 の owner が新 window に確定し pending は空。
  saga 中の w2 への stray attach は拒否される。
- EX-6: `multi_window` Unsupported で `create_window` 失敗 →
  `dock_abort{failed}` で view-1 は w1 に untouched、pending
  move なし、新 window なし。
- EX-7: saga destination を `Ready` 前に `destroy_window` →
  `on_destroyed` が saga abort → view-1 は w1 のまま、orphan
  window 除去、pending なし。
- EX-8: `syn.window.adopt` の冪等性 — 同一 title の再 adopt は
  `adopted:true`、矛盾 title は error。`syn.window.list` が
  両 window と docked views を報告。
- EX-9: `ReleaseViews` disposition を持つ window の `Destroyed`
  → view-1 が `syn.view.list` から消え、他 window の view-2 は
  生存。
- EX-10: `dock_prepare`→`dock_commit` の前後で
  `syn.project.info` が完全一致（project truth 不変）。
- EX-11: `"console"` legacy view — attach/detach は UI-only で
  成功、全 `syn.window.dock_*` op は `invalid_params`。
- EX-12（workspace 層）: `adopt_window`/`adopt_view` の重複拒否・
  pool/window target・`next_view` 前進・`host.create_window`
  非呼出し・adopted window destroy で DetachViews pool。
  （`crates/synvelis-workspace/tests/adopt.rs`）
- EX-13（回帰更新）: `close_veto.rs` の dirty quit・
  `view_mount.rs` の window destroy teardown — 旧「destroy で
  view close」期待を `DetachViews` pool 契約
  （`attached:false`・`window_id:null` で残存）に更新。

## Verification

- `cargo test -p synvelis-app --test dock_ops` — 12 case 全 PASS
  （EX-1〜EX-11 の app/shell 面）。
- `cargo test -p synvelis-workspace --test adopt` — 8 case 全 PASS
  （EX-12）。
- `cargo test -p synvelis-app --test close_veto --test view_mount`
  — 更新済み期待で PASS（EX-13）。
- `cargo test -p synvelis-app -p synvelis-workspace` — 全 suite
  非回帰 PASS（`two_window.rs`/`sv113_dock.rs`/`shell.rs`/
  `ui_ops.rs` 等を含む）。
- 正本検証は `mise run fmt-check`/`check`/`lint`/`test`（+ 存在
  すれば native 系 task）。
- 実 OS での window/dock 証拠・`SYNVELIS_SECOND_WINDOW` native
  smoke は #114 gate のため本 slice では **NotRun**。env-gated
  smoke 経路が使えない場合も fake 証拠を native へ昇格しない。
