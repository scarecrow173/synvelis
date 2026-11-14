# SF-05 ui/ slice coverage map（Issue #108 / synvelis#108）

ui/ headless view-model consumer path の op/action ごとの case
写像。本 slice の新規証拠は `ui/test/signal-flow-view-model.test.ts`
（8 tests）と `ui/test/probe-view-model.test.ts`（9 tests）。
service 側の意味論的正しさは既存 slice1..3 + `coverage.md`
（Rust 側台帳）に委譲し、本台帳は **consumer が wire contract を
正確に運ぶか** のみを記録する。

凡例: ✅ headless test で証明 / ◐ 部分的 / ⛔ 外部 Gate（NotRun）

## `syn.signal_flow.*`（`signal-flow-view-model.test.ts`）

| op / action | case | 判定 |
| --- | --- | --- |
| `syn.signal_flow.view` | params 完全一致（`view_id`/`scope`/`root`/`roles`/`collapsed`/`max_nodes`/`max_edges`/`expected_revision`）+ `view_id` 省略時 main-view default | ✅ |
| `syn.signal_flow.view` response | `coverage:"Stale"` verbatim・Debug 語彙（`"Ready"`/`"Resolved {…}"`/`"PreFader"`）非改名・`published_generation`≠`observed_active_generation`・probe `confirmed_mode:null` 未取得表現 | ✅ |
| `syn.signal_flow.visual` | `move_node`/`set_group`/`fold`/`unfold`/`zoom`/`pan`/`link_focus`/`set_scope` の params 完全一致 + ack `view_generation` の staleness token 更新 | ✅ |
| `syn.signal_flow.inspection` | `inspect`/`set_mode`/`set_follow_source`/`back`/`forward` の params + returned focus（target/mode/follow_source/explore depths）保持 | ✅ |
| `syn.signal_flow.routing` | `begin`/`hover`（port・null=空 canvas）/`drop`/`confirm`（`channel_map` 任意）/`cancel`（reason 語彙）/`state` の params + fixation/assessment/dropped/outcome/gesture typed 保持 | ✅ |
| `syn.signal_flow.routing` outcome | `applied{revision}`/`cancelled{reason}`/`stale` を verbatim 返し stale を retarget しない | ✅ |
| wire error | `invalid_params` reject の code/message 伝播・失敗 ack が `viewGeneration` を進めない・malformed `view_id` を事前検証せず wire reject を観測 | ✅ |
| 非捏造 | 初期 `projection`/`inspection`/`gesture` = `undefined`、`viewGeneration` = 0 | ✅ |

## `syn.probe.*`（`probe-view-model.test.ts`）

| op / action | case | 判定 |
| --- | --- | --- |
| `syn.probe.control` session | `request`/`set_mode`/`capture`/`stop_capture`/`remove` の params 完全一致・`view_id` 非送信 | ✅ |
| `syn.probe.control` view-scoped | `subscribe`/`unsubscribe`/`capture_for`/`abort_capture` が常に `view_id:"view-{n}"` を送信（constructor 必須化） | ✅ |
| `syn.probe.state` | populated projection verbatim（`requested_mode:"capture"`≠`confirmed_mode:"monitor"`、`bound-active`、holders/capture_owner/subscribers/segments/drain/traces） | ✅ |
| `syn.probe.state` detached | `engine_attached:false` → 正直な空集合（error でもゼロ捏造でもない） | ✅ |
| `syn.probe.compare` | `{request}` verbatim 転送・`quality.kind` ∈ `incomparable`/`conditional`/`aligned` を昇格せず保持 | ✅ |
| `syn.probe.compare` detached | `engine_attached:false` → `{observation:null}`（synthesized observation なし） | ✅ |
| wire error | `internal`（no routing engine）・`invalid_params`（malformed `view_id`）の code/message 伝播 | ✅ |
| 非捏造 | 初期 `state`/`comparison` = `undefined` | ✅ |

## Issue #108 AC との関係（本 slice が触れる範囲のみ）

| AC | 本 slice の寄与 | 判定 |
| --- | --- | --- |
| SF-UX-01..04, SF-UX-07 | consumer 層の typed op surface を提供（意味論は service 側で証明済み） | ✅ headless（consumer）/ ⛔ GUI 挙動は外部 Gate |
| SF-UX-05 | `published`≠`active`・probe 未取得・`coverage`/`status` verbatim を consumer が保持することを test で証明 | ✅ headless |
| SF-UX-06 | view-scoped lease action の `view_id` 規約・state の holders/capture_owner/subscribers 投影を consumer が正確に運ぶことを証明 | ✅ headless |
| Windows L2/L3・SF-UX-08 | — | ⛔ NotRun（#114 gate・`scarecrow173/ruviform#631` harness） |
| #109/#103 oracle join | — | ⛔ NotRun（#109 管轄） |

Issue #108 は open 継続 — 本 slice は ui/ headless consumer path
の一部のみを閉じる。
