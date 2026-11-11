---
id: "DESIGN-mB7tYwKsPqNvXcZaE5rGjH"
type: "design"
status: "approved"
title: "SF-05 slice3 設計 — probe subscriber/capture dedup の app/UI 配線"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
    - "docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice3 設計: probe subscriber/capture dedup の app/UI 配線

## Context

SF-04 slice5 が `ControlSession` に実装した lease/capture
ownership（`probe_subscribe`・`probe_unsubscribe`・
`probe_release_subscriber`・`probe_release_all`・
`probe_capture_for`・`probe_abort_capture`・
`probe_lease_holders`・`probe_subscriber_keys`・
`probe_capture_owner`）は workspace 層で完結している。
app 層には既に `syn.probe.control`/`syn.probe.state` op
（SF-04 slice4、session authority のみ）と `op_view_close`
の view teardown があるが、view-scoped 経路が無い。

```
┌───────────────────────────── app (synvelis-app) ─────────────┐
│ syn.probe.control {action}   syn.probe.state                 │
│   request/set_mode/capture/  bindings[].holders              │
│   stop_capture/remove        bindings[].capture_owner        │
│   subscribe/unsubscribe/     subscribers[]{view_id,keys}     │
│   capture_for/abort_capture  (NEW)                           │
│                                                             │
│ syn.view.close ──▶ release_view_probe_leases(inst)  (NEW)    │
│   └─ engine.with_probe_bridge ─▶ probe_release_subscriber    │
│                                                             │
│ capture_screen_state ──▶ signal_flow_probe_rows (NEW)        │
│   └─ main view の lease → SignalFlowProbeRow[]               │
└─────────────────────────────────────────────────────────────┘
        │ per-call &mut dyn ProbeTracePort
        ▼
┌──────────── workspace ControlSession ────────────┐
│ leases: BTreeMap<Key, BTreeSet<ProbePrincipal>>  │
│ by_view: BTreeMap<ViewInstanceId, BTreeSet<Key>> │
│ capture_owners: BTreeMap<Key, ProbePrincipal>    │
└──────────────────────────────────────────────────┘
```

## Overview

### op 層（`ui_ops.rs`）

`syn.probe.control` の `action` 語彙を 5→9 に拡張する。
新 action は全て `params.view_id`（`"view-{n}"`、必須）を
`sf_view_instance` で `ViewInstanceId` へ解決する —
未登録 view は workspace の detached pool へ
`ViewType::SignalFlow` で adopt され、lease は
`ViewInstanceId`（永続 identity）に従う。

| action | session API | params | 戻り値 |
| --- | --- | --- | --- |
| `subscribe` | `probe_subscribe(project, port, inst, target, point, kind, mode)` | `view_id`,`target`,`point`,`kind`,`mode` | `{key, view_id, targets}` |
| `unsubscribe` | `probe_unsubscribe(port, inst, key)` | `view_id`,`key` | `{key, view_id, released[], removed[]}` |
| `capture_for` | `probe_capture_for(port, inst, key, max_frames)` | `view_id`,`key`,`max_frames` | `{key, view_id, max_frames, owner}` |
| `abort_capture` | `probe_abort_capture(port, inst, key)` | `view_id`,`key` | `{key, view_id}` |

session authority の既存 5 action（`request`/`set_mode`/
`capture`/`stop_capture`/`remove`）は変更しない — view
lease を取らず `ProbePrincipal::Session` のまま。

subscriber 解決は `engine.with_probe_bridge` 突入**前**に
行う（`sf_view_instance` が workspace registry を mutate
するため、session borrow と競合しない順序にする）。
`view_id` はこれら action で必須 — probe lease は view
単位の所有物であり、暗黙の main-view default は付けない
（誤った view への lease 付与を防ぐ fail-closed）。

### view close hook（`ui_ops.rs`）

`op_view_close` の dock-domain 分岐で
`dock_view_close` の**前**に
`release_view_probe_leases(info.instance)` を呼ぶ:

```rust
fn release_view_probe_leases(&mut self, inst: ViewInstanceId) -> Result<(), ErrorBody> {
    let Some(engine) = self.routing_engine.clone() else { return Ok(()) };
    engine.with_probe_bridge(|port| {
        self.workspace.session_mut().control_mut()
            .probe_release_subscriber(port, inst)
            .map_err(probe_trace_err)?;
        Ok(())
    })
}
```

- engine 未接続 → port が存在し得ない → lease も存在
  し得ない → no-op（捏造しない）。
- release 失敗（port error）→ close 全体を abort。
  処理済み key は整合済みで retry 可能
  （`probe_release_subscriber` の契約）。
- `close_vcs_view`（`syn.vcs.visual{action:"close"}`）にも
  同じ hook を `remove_view` 前に置く — VCS view も
  probe lease を持ち得るため同一不変量を適用する。
- `op_window_lifecycle_destroyed` は `ReleaseViews`→
  `op_view_close`・`DetachViews`→`op_view_detach` に
  routing 済みのため変更不要。detach は lease 保持
  （`ViewInstanceId` が window を生き延びる契約どおり）。
- `view-{n}` で dock metadata を持たない view（sf op で
  adopt された `ViewType::SignalFlow` view 等）は現行
  `dock_domain_view` が `syn.view.close` を拒否する —
  これは既存の close 経路欠落であり本 slice の回帰では
  ない。close できない view の lease は残るが、view が
  生きている限り lease 保持は正しい状態。

### state 投影（`ui_ops.rs`）

`op_probe_state` の各 binding に
`"holders": ["session"|"view-{n}", ...]` と
`"capture_owner": "session"|"view-{n}"|null` を追加し、
top-level に `"subscribers": [{"view_id","keys":[...]}]`
を追加する。`ProbePrincipal` → wire は
`"session"` / `format!("view-{}", inst.get())` の文字列。
binding ledger の key 集合から `View` holder を集め、
各 view の `probe_subscriber_keys` で keys を引く
（leased key は必ず live binding を持つため
bindings 走査で漏れない）。

`probe_trace_err` に `NotSubscriber`/`NotCaptureOwner`
を `INVALID_PARAMS` へマップする追加 — lease 非保持・
非 owner は呼び出し側の権限 fault であり、ambiguous な
internal error にしない。

### 画面（`screens.rs` + `capture_screen_state`）

`MainScreenState` に `signal_flow_probes:
Vec<SignalFlowProbeRow>` を追加する — `signal_flow`
field の**兄弟**として置く。`publish_project_length`
（app.rs、編集範囲外）が `screen.signal_flow` を
`from_view` で丸ごと再構築するため、probe rows を
`SignalFlowScreenState` 内に入れると毎回 wipe される。
sibling field は publish が触らない。

`capture_screen_state`（ui_pump tail、毎 pump）の末尾で
`screen.signal_flow_probes = self.signal_flow_probe_rows()`
を実行。`signal_flow_probe_rows` は main view
（`main_view_instance()`）の `probe_subscriber_keys` を
走査し、各 key の binding（requested_mode/state）・
lease holders・capture owner を `SignalFlowProbeRow`
（point/kind/mode/state/holders/capture_owner の
String 列）へ投影する。engine 無し → 空 vec。

`main_screen_spec` の Signal Flow tab に
`signal_flow.probes` read-only `Table` を追加する
（nodes/edges table と同じ `selection: None` 契約）。
プローブ ON/OFF の操作自体は view command ではなく
`syn.probe.control` op が担う — 画面は状態表示に留め、
project/DSP state を変更しない。

### IPC 型（`ui/src/ipc.ts`）

`OpName` union に `"syn.probe.control"` と
`"syn.probe.state"` を追加 — 現状 probe ops が
frontend 型から全欠落している。

## Session detach / project 差し替え時の lease

`probe_release_all` は session stop 相当の全解放 API
として存在するが、本 slice では**呼ばない**。lease は
`ControlSession`（session scope）に属し、engine detach
や project open は session を破棄しない。project 差し替え
後の runtime probe は既存の generation/stale 機構
（`StaleGeneration`/binding ledger）が処理し、lease の
session 生存は workspace 層の契約どおり。session 破棄
時の解放は `ProjectSession`/`ControlSession` drop で
port 側 binding も一緒に死ぬ（port は engine 所有で
session と寿命が違う場合の扱いは SF-04 slice4 の
engine attach/detach 契約に従う — 本 slice では新たな
release タイミングを追加しない）。

## Failure / Honesty

- engine 未接続: subscriber action は既存どおり
  `SERVICE_ERROR`（fail-closed）。state は
  `engine_attached:false` + `subscribers:[]`。
- lease 非保持 view の `unsubscribe`/`capture_for`/
  `abort_capture` → `NotSubscriber` → `INVALID_PARAMS`。
- 非 owner view の `abort_capture` → `NotCaptureOwner`
  → `INVALID_PARAMS`。
- provider 失敗は `ProbePortError` 経由で既存
  `probe_trace_err` マッピング（`Failed`→`SERVICE_ERROR`、
  named→`INVALID_PARAMS`）。失敗時 lease/owner/correlation
  は workspace 層が不変に保つ。
- `syn.probe.state` は read-only — drain しない。
  `ui_pump` の `drain_probe` が唯一の drain owner のまま。
