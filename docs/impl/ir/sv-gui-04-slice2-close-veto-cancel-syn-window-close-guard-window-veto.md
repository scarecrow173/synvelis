---
id: "IMPL-034UrlNf4hC31G8gfjoR8y"
type: "impl"
status: "completed"
title: "SV-GUI-04 slice2 close-veto/cancel — syn.window.close_guard + 末尾 window veto"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/close_veto.rs"
      - type: "file"
        path: "docs/specs/sv-gui-04-02-close-veto.md"
      - type: "file"
        path: "docs/adr/sv-gui-04-slice2-close-veto-live-window-guard-query-app-global-reasons.md"
      - type: "file"
        path: "docs/designs/sv-gui-04-02-close-veto-design.md"
      - type: "file"
        path: "docs/plans/sv-gui-04-02-close-veto.md"
      - type: "file"
        path: "docs/test-specs/sv-gui-04-02-close-veto.md"
      - type: "file"
        path: "docs/impl/ir/sv-gui-04-slice2-close-veto-cancel-syn-window-close-guard-window-veto.md"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv-gui-04-02-close-veto.md"
    - "docs/plans/sv-gui-04-02-close-veto.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-04-02-close-veto.md"
    - "docs/adr/sv-gui-04-slice2-close-veto-live-window-guard-query-app-global-reasons.md"
    - "docs/designs/sv-gui-04-02-close-veto-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-02-close-veto.md"
    - "docs/adr/sv-gui-04-slice2-close-veto-live-window-guard-query-app-global-reasons.md"
    - "docs/designs/sv-gui-04-02-close-veto-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sv-gui-04-02-close-veto.md"
  verified-by:
    - "docs/test-specs/sv-gui-04-02-close-veto.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-GUI-04 slice2 close-veto/cancel — syn.window.close_guard + 末尾 window veto

## Summary

Issue #113 の close-veto/cancel slice。`NativeShell` の
`CloseRequested` が無条件 `Approve` していた部分を、app-global
guard query `syn.window.close_guard` 経由の vetoable decision に
置き換えた。guard は未保存変更（`live_revision` vs
`saved().revision`）・open recording transaction・
`RecorderSlot::active()` を reasons として報告し、shell は
**末尾 live window の close にのみ**適用する — 残存 window が
共有 `ProjectSession` を保持するため非最終 window は
無条件 `Approve`。query の op error・Result 欠落・malformed
payload は fail-closed で `Veto` に倒す。`syn.app.quit` の
per-window `request_close` も同一経路を通るため、複数 window
の quit で一部 veto が決定的に発生する。

## Context

Spec: docs/specs/sv-gui-04-02-close-veto.md
ADR: docs/adr/sv-gui-04-slice2-close-veto-live-window-guard-query-app-global-reasons.md
Design: docs/designs/sv-gui-04-02-close-veto-design.md
Plan: docs/plans/sv-gui-04-02-close-veto.md
Test spec: docs/test-specs/sv-gui-04-02-close-veto.md
Task: docs/tasks/sv-gui-04-02-close-veto.md

`shell.rs` module doc が "unsaved-guard veto" を設計上の hook
として明記しており、Issue #113 の close-cancel 受入（veto による
window/session 保持・Window close で Project/Graph/Recording/Job
が暗黙停止しない・decision query 失敗時の安全な復帰）を満たす
slice。project/session/recording の truth は app 層に一元化
されているため、guard の判定材料は `SynvelisApp` が read-only
で報告し、window topology の知識だけを持つ shell が
`CloseDecision` に写像する責務分界とした。

## Implementation

### Changes

- `crates/synvelis-app/src/app.rs`: `SynvelisApp::close_veto_reasons()`
  を追加。`unsaved_changes`（`saved` 有りなら `live_revision !=
  saved.revision`、無しなら `live_revision.0 > 0` — fresh boot
  project は clean）、`recording_active`
  （`session().control().recording().is_some()` — open recording
  transaction）、`recorder_active`（`RecorderSlot::active()`、
  現状 stub で常に false — RCD 系 slice が実体を配線するまでの
  正直な extension point）を `Vec<&'static str>` で返す。
- `crates/synvelis-app/src/service.rs`: `OPS` に
  `syn.window.close_guard` を登録（`syn.workspace.mount` の後・
  `syn.view.*` の前）し、`dispatch` に
  `{"reasons": self.app.close_veto_reasons()}` を追加。params は
  読まない app-global query で、`ui_pump`/notify を伴わない
  Result op。
- `crates/synvelis-app/src/shell.rs`: module doc の
  `CloseRequested` 項を decision query 記述に更新。
  `CloseRequested` 分岐は `lifecycle(Closing)` 配送を維持した
  まま `respond_close` の decision を新設 `close_decision` に
  委譲。`close_decision` は (1) `driver.windows()` が当該
  window のみ（= 末尾 live window）かを判定し、非最終なら即
  `Approve`、(2) `shell-close-guard-{window}` intent を
  `rt.dispatch_window_intent` で同期 dispatch し返った全
  `RoutedEvent` を `observe_result` + `post_routed` で通常経路に
  流し、(3) 自 intent id の `Result` が `ok` かつ `reasons` が
  well-formed な空配列のときだけ clean とし、それ以外
  （op error・Result 欠落・malformed payload）は `guarded`
  default `true` のまま fail-closed で `Veto` に写像する。
- `crates/synvelis-app/tests/close_veto.rs`（新規）: FakeDriver +
  NativeShell fixture で test-spec 全 case を検証（下記 Validation）。

### Decisions During Implementation

- **guard は reasons のみ返し topology を op に渡さない**:
  `last_window` 判定は `driver.windows()` を持つ shell の責務。
  op contract に window 集合を漏らすと責務分界が崩れる
  （ADR の Considered Options どおり）。
- **dirty 判定は `saved=None && live_revision.0 > 0` を dirty と
  する**: boot project（rev 0・未 save）は失うものが無いため
  clean。`syn.project.new`/`open` は save 済み session を作る
  ため通常経路では `saved=Some` で `live != saved.revision` が
  判定される。
- **fail-closed の既定値を `guarded=true` に置く**: well-formed
  な空 `reasons` だけが `Approve` に反転させる構造にし、
  Result 欠落・error・malformed の全経路が Veto に収束する
  ことを分岐ではなく初期値で保証した。
- **guard query の Result は通常どおり window へ post する**:
  `attach_view` と同じ dispatch→observe→post 経路に載せ、
  `ViewHost::apply_receipt` が未知 command id を無視する既存
  契約に任せる（テストでも `shell-close-guard-*` Result が
  window に届くことを assert）。
- **dirty fixture は seal 前の `workspace_mut().session_mut()
  .apply(ProjectOp::SetProjectName)` で作る**: seal 後に到達
  できる `syn.*` op に project rename は無く、`saved=Some` 系は
  `project_new`（内部で save）を seal 前に呼んでから apply する
  形で `live != saved.revision` 分岐もカバーした。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

`mise run` 系（正本）— すべて exit 0、`origin/main`（`70d6b21`、
RCD-01 #205 merge 済み）取り込み後の merged tree で実行:

- `mise run fmt-check`: exit 0
- `mise run check`: exit 0
- `mise run lint`: exit 0（`collapsible_if` を let-chain に修正済み）
- `mise run test`: exit 0 — `tests/close_veto.rs` 6 件全 pass
  （`close_guard_reports_unsaved_changes` /
  `dirty_last_window_close_is_vetoed` /
  `clean_last_window_close_approves_and_exits` /
  `dirty_non_last_window_close_approves_without_guard_query` /
  `quit_with_dirty_project_partially_vetoes` /
  `quit_with_clean_project_closes_all_windows`）、既存
  `tests/shell.rs`・`two_window.rs`・`sv113_dock.rs` 非回帰
- `mise run build`: exit 0
- `mise run check-app-native`: exit 0
- `mise run lint-app-native`: exit 0
- `mise run build-app-native`: exit 0

補助証拠（canonical = mise）: `cargo test -p synvelis-app
--test close_veto` 単独実行も 6/6 pass。

NotRun: 実 OS での window close/focus/quit 証拠は #114 gate の
ため本 slice 対象外。GPUI native 経路の IPC timeout も NotRun。
`run-app-native` 実窓起動は未実施（build のみ検証）。

review-protocol 自己レビュー実施: acceptance criteria・scope
gusection に対し cooperative/adversarial の 2 pass を実施し、
CONFIRMED P0/P1 なし。P3 メモ: `dirty_app(true)` が temp bundle
を残す（%TEMP% 配下・実害なし）、`recording_active` reason は
open transaction 構築が RCD slice 領域のため fixture 未カバー
（`is_some()` の配線のみ）、test-spec EX-6（query 失敗の
fixture 再現）は現行 op が infallible のため NotApplicable —
fail-closed 経路は `guarded` 初期値設計としてコードレビューで
確認済み。

## Risks

- veto 後の「保存しますか?」確認 UX は無い — veto は「閉じない」
  のみで、利用者は `syn.project.save`/`reload`/`new`/`open` で
  状態を解決してから再度 close する必要がある（spec non-goal、
  `Defer` は将来 slice）。
- `RecorderSlot::active()` は stub のまま — 実 recorder 配線は
  別 dispatch の責務。open recording transaction
  （`control().recording()`）は実状態を読むため、transaction
  経路の veto は既に有効。
- 非最終 window close は guard を通らない — window 固有の
  未保存 UI state を守る仕組みは無いが、view state は現状
  揮発性で永続化対象外のため実害なし（ADR Consequences 記載）。

## Follow-ups

- 保存確認 prompt / `CloseDecision::Defer` の非同期継続
  （将来 slice — spec non-goal）。
- 実 OS close 証拠は #114 gate で採取。
- Detach→Reattach / dock transfer の GUI op 配線は別 slice
  （既知ブロッカー: `WorkspaceManager<ShellWindowHost>` 固定と
  `adopt_window` seam 欠如）。
