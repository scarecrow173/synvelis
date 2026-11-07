---
id: "TSPEC-Wa6Ax4SQSRfWvuKIzs8fCf"
type: "test-spec"
status: "approved"
title: "SV175-01 検証仕様: NativeShell lifecycle + ViewBinding wire 形"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/175"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/designs/sv175-native-shell-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv175-01-native-shell-binding.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv-gpui-01-native-app-shell.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV175-01 検証仕様

検証は `ruviform-ui-test::FakeDriver` で headless。実 OS window/
GPU/IME/focus は本仕様の対象外（未観測は `NotRun` 明記）。

## shell（`tests/shell.rs`）

| # | 観点 | 手順 | 期待 |
| --- | --- | --- | --- |
| S1 | window 作成/Ready | `open_main`→`FakeDriver.pump` | `Created`/`Ready` で window が ready、初期 app-state notify が inbox へ |
| S2 | Action→typed result | `Windowed{Action(app.info)}` inject | `RoutedEvent` が origin window へ `Result{ok:true,data}` で返送 |
| S3 | view attach/tracking | `Action(syn.view.attach)` | result ok 後 `views[view]==window`、以後 view-targeted notify が window へ |
| S4 | control gesture | attach→`syn.control.dispatch` begin→preview→commit | receipt notify（accepted/applied）が window へ到達・projection が `observed` 更新を含む |
| S5 | gesture cancel | begin→cancel | cancelled phase receipt |
| S6 | project save/reload | `project.new`→`project.save`→`project.open` | store 書込・再 open で `loaded` 復帰（`ui_ops` 相当経路） |
| S7 | close lifecycle | `Windowed{CloseRequested}` inject | `lifecycle(Closing)` event が window へ → Approve → `Destroyed` |
| S8 | teardown | view attach 済み window の `Destroyed` | `syn.view.close` が dispatch され view binding/subscription が解放、`windows` 空なら `exit()` |
| S9 | quit flag | `syn.app.quit` Action → `Tick` | 全 window へ `request_close`（close_pending 記録） |
| S10 | unrouted fallback | `dispatch_op` 起因 `RoutedEvent{window:None}` | 全 ready window へ broadcast |

## binding（`tests/binding.rs`）

| # | 観点 | 期待 |
| --- | --- | --- |
| B1 | intent wire 形 | op=`syn.control.dispatch`、params.intent=`{"type":"begin",...}`、ctx.generation=attach 応答採用値 |
| B2 | id 相関 | `pending` に intent.id→op、Result で `CallResult{op}` |
| B3 | generation adopt | attach result `data.generation` が以後の ctx.generation |
| B4 | notify demux | `syn.control.receipt`/`projection`/`view.state`/`viz`/無 channel→`AppState` 各 Notice 化 |
| B5 | view_id 絞込 | 他 view 宛 notify は `Ignored` |
| B6 | gesture mint | `gesture_begin` が `g<view>-<seq>` を返し同 ID で preview/commit/cancel |
| B7 | outbox | `take_outbox` が生成順 intent を返す |

## 実施しない検証

- RF387 renderer 実接続（候補待ち・`NotRun`）、`synvelis-native`
  実機起動の OS 観測（`NotRun`）、`ime`/`native_surface_slot`
  機能動作（`Unsupported`/backend 依存）。
