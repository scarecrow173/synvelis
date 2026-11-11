---
id: "SPEC-7c4e9a1b2d3f4e5a6b7c8d"
type: "spec"
status: "approved"
title: "PC-C4-06 control op surface — syn.control.* typed-evidence ops"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
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
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-03-param-observation-slice.md"
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
  refines:
    - "docs/specs/pc-c4-04-app-composition.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/specs/pc-c4-03-param-observation-slice.md"
    - "docs/test-specs/pc-c4-04-app-composition.md"
  defers: []
  deferred-by: []
---

# PC-C4-06 control op surface — syn.control.* typed-evidence ops

## What Should Be Built

Issue #101 の PC-C4 残件 slice。PC-C4-04
（`docs/specs/pc-c4-04-app-composition.md`）が app composition root に
実装した control surface の deferred API を、`synvelis-app` の
`AppService` 上の `syn.control.*` op として公開する。現状
`service.rs` に登録済みなのは `syn.control.dispatch` /
`subscribe` / `unsubscribe` / `targets` のみで、fast-control write・
control-tick/report status・observation/feed evidence の各 op が
未公開であり、GUI・headless driver・将来の CLI adapter が typed
evidence を取得する経路が無い。

具体的には:

- **`syn.control.fast_control`**: wire params から
  `FastControlWrite` を構成し、attach 済み control surface の
  verified `frame_base` で `position` を stamp して
  `RoutingControlDispatch::fast_control` へ渡す。戻り値は player の
  `FastControlOutcome`（`Delivered` / `Queued{depth}` /
  `Blocked(kind)`）を verbatim に投影し、block 時は記録済み
  `FastControlBlock` の typed detail を併記する。
- **`syn.control.status`**: control surface の typed-evidence
  snapshot — `SurfaceReport`（attach/bind 証拠）・最後の pump 駆動
  `TickReport`・composition root が記録した最新
  `ReportDrainReport`・surface generation・RT 観測
  `active_generation`（未観測は `null` = 常に gated）・verified
  `frame_base`・最後の attach/sync error を返す。engine/surface
  非装着時は全て `null`/`false` で、証拠を捏造しない。
- **`syn.control.observe`**: caller 供給の `TransportSpan` と物理
  call window を `observe_cycle` へ渡し、`DispatchCycle` の
  per-target observation（stage・value・position・ledger revision）・
  drained `AppliedReceipt`・`Reported` stage のみを
  `feed_plugin_report` へ流した件数 `fed` を返す。
- **headless fixture**: `crates/synvelis-app/tests/` に
  `UiRuntime::dispatch_op` 経由で全 op を駆動する `AppService`
  fixture を追加し、typed outcome・stage 選別・generation gate・
  fail-closed を検証する。

## Why Now

PC-C4-04 で制御面の実体（catalog vend・dispatch 所有・tick 駆動・
feed 橋渡し）は composition root に揃ったが、service 境界には
subscription/target 列挙しか公開されていない。実 plugin backend
接続・GUI 配線・headless driver のいずれも、まず「surface が何を
観測したか」を typed evidence として読む op が必要であり、これが
無いと後続 slice（drain wiring・production composition root）は
検証手段を持てない。honest evidence のみを返す契約を service 層で
先に固定することで、捏造値が混入する経路を構造的に排除する。

## Users and Value

- **GUI host / view layer**: param 編集 UI が fast-control write の
  `Delivered`/`Queued`/`Blocked` を区別して表示できる。
- **headless driver / test harness**: `observe` で dispatch ledger
  の stage 遷移（`Scheduled`→`Dispatched`→`Reported`）を決定的に
  駆動・検証できる。
- **将来の CLI adapter**: `AppService` は GUI/headless/CLI で同一の
  service contract を共有するため、本 op 群はそのまま CLI に露出する。

## Scope

### In Scope

- `service.rs` への `syn.control.fast_control` /
  `syn.control.status` / `syn.control.observe` の `OPS` 登録と
  dispatch route。
- `ui_ops.rs` の wire param parser・typed-evidence JSON 投影・
  op handler・`note_control_drain`（composition root からの
  `ReportDrainReport` 記録）。
- `engine.rs` の read-only accessor（`control_frame_base` /
  `control_last_block`）。
- `FastControlWrite.means` の型付けに必要な
  `ruviform-plugin-core` の `synvelis-app` `[dependencies]` 追加
  （Coordinator 承認済み）。
- headless `AppService` fixture test。

### Out of Scope

- `observe_cycle` の per-block 物理 call-window を RT runtime 内で
  自動供給する配線（`drain_output_reports` の engine 配線・
  production composition root の `PluginHost` 接続を含む）。
  本 slice の `observe` は caller 供給 span/call window の明示的
  drive であり、L1 wire 検証・実 plugin backend は後続 slice。
- 実 VST3/OS backend（ruviform#253）・実 GUI（#114/#102）・
  AudioLens（#294）・oracle（#103）gate — 全て NotRun。
- `synvelis-player` / `synvelis-library` の src 変更、
  `docs/*/README.md` index、`mise.toml` の変更。
- parameter enumeration の新規経路 — vend catalog と正直な
  empty-catalog 経路（`host.rs`）は既存のまま維持する。

## Requirements

- **REQ-1**: 3 op が `OPS` に登録され `AppService::dispatch` で
  route される。未登録 op は `unknown_op`、登録済み op の param
  不正は `invalid_params`、surface 不在は `service_error` で
  fail-closed。
- **REQ-2**: `fast_control` は `FastControlOutcome` を verbatim に
  返す。`Delivered`/`Queued`/`Blocked` 以外の成功を装わない。
  block 時は `FastControlBlockKind` の compact kind と記録済み
  `FastControlBlock` detail を返す。
- **REQ-3**: `position` は surface の verified `frame_base` で
  stamp する。`frame_base` が取れない（surface 未装着）場合は
  write を構成せず error。
- **REQ-4**: `status` は `SurfaceReport`/`TickReport`/
  `ReportDrainReport` の counter・generation・frame_base・
  error evidence を verbatim に投影する。drain report は
  composition root が実際に drain した場合のみ記録され、
  初期値は「drain 未実施」を正直に示す（`attached: false`）。
- **REQ-5**: `observe` は caller 供給の `TransportSpan`・call
  window をそのまま `observe_cycle` へ渡す。戻り値の observation
  は stage を偽装せず、`Reported` stage のみが
  `feed_plugin_report` へ渡り `fed` に計上される。
  `Scheduled`/`Dispatched`（host 解決済み）を plugin-reported と
  して relabel しない。
- **REQ-6**: generation/capability gate と
  `StablePluginInstanceId`/`PluginMismatch` の fail-closed 意味論を
  end-to-end で維持する。stale generation の observe は観測前に
  reject される。
- **REQ-7**: plugin 値・parameter 列挙・report を捏造しない。
  caller が供給しない timing/context を op が invent しない。

## Acceptance Criteria

- [ ] `syn.control.fast_control` / `syn.control.status` /
  `syn.control.observe` が `OPS` と dispatch の双方に存在し、
  headless `AppService` fixture で駆動できる。
- [ ] active surface への write が `{"outcome":"delivered"}`、
  typed refusal が `{"outcome":"blocked","block":<kind>}` を返す。
- [ ] `status` が surface 未装着時に全 evidence `null`/`false`、
  装着時に `SurfaceReport`/`TickReport`/記録済み drain・
  generation・`frame_base` を返す。
- [ ] `observe` が caller span で ledger を進め、
  `Scheduled`/`Dispatched` stage が `fed=0`、drain 後の
  `Reported` stage が `fed=1` で workspace plugin-report
  channel に到達する。
- [ ] stale generation・未知 processor・malformed params が
  typed error で fail-closed。
- [ ] canonical verification（`mise run` 各 task）が全て実走で
  green。

## Deferred Design Concerns

- `observe_cycle` の per-block 物理 call-window 自動供給・
  `drain_output_reports` の engine/`PluginHost` 配線・production
  composition root の接続は後続 slice（design の Deferred 節参照）。
- wire schema の L1 検証（実 GUI/driver からの結合確認）は
  外部 gate として NotRun。

## Sources and Relations

- Issue: `scarecrow173/synvelis#101`（PC-C4 control/plugin surface）
- 上流 spec: `pc-c4-04-app-composition`（composition root 実装）、
  `pc-c4-02-plugin-instance-binding`（identity pin）、
  `pc-c4-03-param-observation-slice`（Reported stage 契約）。
