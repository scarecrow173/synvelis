---
id: "DESIGN-uivmNbXmQvR2wTbLpNcJyH"
type: "design"
status: "approved"
title: "SF-05 ui/ slice 設計 — Signal Flow・Probe headless view-model consumer path"
created: "2026-11-18"
updated: "2026-11-18"
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
    - "docs/specs/sf-05-ui-viewmodel.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sf-05-slice3-probe-subscriber-wiring-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-05-ui-viewmodel.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-ui-viewmodel.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 ui/ slice 設計: Signal Flow・Probe headless view-model

## Context

`ConsoleViewModel`（`ui/src/viewmodel/console-view-model.ts`）と
`VizSource`（`ui/src/viewmodel/viz-source.ts`）が既存の DOM-free
consumer 先例。本 slice の 2 VM も同じ境界に置く:

- SolidJS/DOM/Element に依存しない。
- 全 IPC は `../ipc.ts` の `call` 経由（raw bridge adapter は
  `ipc.ts` のみ）。
- VM が wire contract の唯一の typed carrier であり、将来の
  SolidJS surface は second truth を持たない。
- push channel は `syn.signal_flow.*`/`syn.probe.*` に存在しない
  ため、両 VM は **pull 型**（明示 refresh/command）とする。

## Overview

### `ipc.ts`

`OpName` union に `syn.signal_flow.view|visual|inspection|routing`
を既存順序の末尾（`syn.viz.source_changed` の後、`syn.probe.*`
の前）へ追加。`call<T>`/`onEvent`/`dataUrl` は不変。
`conformance.test.ts` の validOp op リストへ新 7 op
（signal_flow 4 + probe 3）を追記し、「registered syn.* op」の
網羅性を回復する。

### `SignalFlowViewModel`

options object constructor（`{ viewId?: string }`）。`viewId`
省略時は `view_id` param 自体を送らず service の main-view
default に委ねる（`sf_view_param` が省略を許容）。

保持する状態:

- `projection`: `syn.signal_flow.view` の response を **verbatim**
  （`SignalFlowViewWire`）。`coverage`（`Complete|Truncated|Stale|
  Unavailable|Unsupported`）・`scope`・node `state`/`runtime`・
  edge `role`/`port_role`/`tap`・status `activation`/`fault`/
  probe `state`/`requested_mode`/`confirmed_mode` は全て
  `format!("{:?}")` の Debug 文字列を literal union で受け、
  wire 綴りのまま保持する。**request 側語彙**（`scope:"local"`、
  `roles:["main"]`、inspection `mode:"follow"`）は serde 側の
  別語彙であり、型を分離して混同を防ぐ。
- `viewGeneration`: visual ack / view response の
  `view_generation`（= visual state の `visual_revision`）。
  visual op 成功ごとに更新し staleness token とする。
- `inspection`: 最後の `syn.signal_flow.inspection` response の
  focus（target/mode/follow_source/explore stack depths）。
- `gesture`: 最後の `routingState()` poll（`undefined` = 未 poll、
  `null` = gesture 無し）。他の routing op は response を返すが
  この cache を更新しない。

メソッドは op ごとに 1:1:

- `loadView(query)` → `syn.signal_flow.view`。query の各 field は
  defined のときだけ param 化（`undefined` を wire に出さない）。
  response の `view_generation` が safe integer でなければ
  `RangeError`（wire contract 違反を黙って通さない）。
- visual 8 action → `syn.signal_flow.visual`。ack の
  `view_generation` が safe integer なら token を更新。
- inspection 5 action → `syn.signal_flow.inspection`。
- routing 6 action → `syn.signal_flow.routing`。outcome は
  discriminated union（`applied{revision}`/`cancelled{reason}`/
  `stale`）をそのまま返す。**stale を retarget・昇格しない**。

### `ProbeViewModel`

options `{ viewId: string }` — **必須**。view-scoped action が
`view_id` を要求する以上、constructor で lease-owning view を
固定し implicit default を構造的に排除する。

- session-scoped（`request`/`setMode`/`capture`/`stopCapture`/
  `remove`）: `view_id` を **送らない**。
- view-scoped（`subscribe`/`unsubscribe`/`captureFor`/
  `abortCapture`）: `view_id: this.viewId` を常に送る。
- `refreshState()` → `syn.probe.state`（引数なし）。
  `engine_attached:false` の honest 空集合をそのまま保持。
- `compare(request)` → `syn.probe.compare` に
  `{request}` を verbatim 転送。`ProbeCompareResult` は
  discriminated union（`engine_attached:true → observation` /
  `false → observation:null`）。`quality.kind` ∈
  `aligned|conditional|incomparable|unknown` を verbatim 保持。

### wire 語彙の 2 系統

| 面 | 語彙 | 例 |
| --- | --- | --- |
| `syn.signal_flow.*` response | Rust `Debug` | `"Complete"`, `"BoundActive"`, `"PreFader"` |
| `syn.signal_flow.*` request params | serde/手書き | `scope:"local"`, `roles:["main"]`, `mode:"follow"` |
| `syn.probe.*` 全域 | serde kebab-case | `"bound-active"`, `"off"`, `"metrics"` |
| `syn.probe.*` entity tag のみ | serde variant 名（rename なし） | `"Track"`, `"Bus"`（PascalCase — kebab ではない） |

VM の `*Wire` 型はこの綴りを pin し、signal-flow 側と probe 側で
同名 enum（例: `ProbeMode`）が別 union になることを型で明示する。
`SemanticSignalEntityRef` は `#[serde(tag, content)]` で
`rename_all` を持たないため、`entity` tag だけは PascalCase
variant 名になる — boundary/kind/mode の kebab-case と同じ
payload 内で混在する。

## Failure / Honesty

- wire error は `call` の reject（`IpcError` code/message）を
  そのまま伝播。VM は成功を偽装しない（engine 未接続の
  `syn.probe.control` は wire error として surface）。
- `malformed view_id` は VM で事前検証せず wire にそのまま渡し、
  service の `invalid_params` reject を consumer が観測できる
  ようにする（VM 独自の第二 validation を持たない）。
- `engine_attached:false` → `syn.probe.state` は空集合、
  `syn.probe.compare` は `observation:null`。VM はこれを
  「利用不可」として保持し、ゼロ値や synthesized observation
  で埋めない。
- `coverage:"Stale"`・`status.stale:true`・`stale` routing
  outcome は wire のまま — 「routing なし」「正常」への畳み込み
  を行わない。
