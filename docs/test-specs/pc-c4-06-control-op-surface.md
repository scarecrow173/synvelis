---
id: "TSPEC-7c4e9a1b2d3f4e5a6b7c8f0a3b4"
type: "test-spec"
status: "approved"
title: "PC-C4-06 control op surface test spec"
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
  implements:
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
    - "docs/plans/pc-c4-06-control-op-surface-plan.md"
    - "docs/tasks/pc-c4-06-control-op-surface-task.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-06-control-op-surface-plan.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
    - "docs/plans/pc-c4-06-control-op-surface-plan.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-c4-04-app-composition.md"
  defers: []
  deferred-by: []
---

# PC-C4-06 control op surface test spec

## Purpose

Issue #101 の PC-C4-06 slice が、`syn.control.fast_control` /
`syn.control.status` / `syn.control.observe` を `AppService` に
公開し、返す証拠が全て genuine typed evidence（`FastControlOutcome`・
`SurfaceReport`/`TickReport`/`ReportDrainReport` counter・
`DispatchStage` observation）であることを、headless
`UiRuntime::dispatch_op` fixture で固定する。`Reported` stage のみが
workspace plugin-report channel へ feed されること、generation/
capability gate が fail-closed であることを最重要保証とする。
実 VST3/OS backend・実 GUI・AudioLens・oracle・L1 wire 検証は対象外
（NotRun）。

## Feature

`syn.control.*` typed-evidence op surface（headless `AppService`）

## Rules

### PC-C4-06-01 — surface 無し fail-closed

- **Given**: routing engine を attach していない `SynvelisApp` を
  `AppService` に包んだ `UiRuntime`。
- **When**: `syn.control.status` / `syn.control.fast_control` /
  `syn.control.observe` を呼ぶ。
- **Then**: `status` は `engine:false`・`generation`/`surface`/
  `tick`/`frame_base`/`last_error` が `null`・`drain.attached:false`
  を返す。`fast_control`/`observe` は well-formed params でも
  `service_error` で reject され、捏造した write/観測を返さない。

### PC-C4-06-02 — status の typed evidence 投影

- **Given**: catalog vend → publish → attach 済み surface
  （generation=1・`bound_targets=1`）を持つ app。
- **When**: `syn.control.status` を呼ぶ。
- **Then**: `engine:true`・`generation:1`・`frame_base` が
  `{numerator:1,denominator:48000}`・`surface.generation:1`・
  `surface.bound_targets:1`・`controlled_processors` が processor
  tagged id・`unbound` 空・`active_generation:null`（RT 未観測）・
  `tick:null`（pump 未駆動）・`drain` が default（`attached:false`）
  を返す。RT activate + write 後は `active_generation` と `tick`
  counter（`attached`/`generation`/`gated:false`/`evaluated>=1`）が
  実値で現れる。

### PC-C4-06-03 — fast_control の typed outcome

- **Given**: RT-active surface と bound param-7 target。
- **When**: `syn.control.fast_control` に
  `{target, value, means:"timed_event", position_frames,
  project_revision}` を渡す。
- **Then**: `{"outcome":"delivered"}` を返す。`position` は surface
  verified `frame_base` で stamp される。
- **And**: `means:"immediate"`（contract が honor 不能）は
  `{"outcome":"blocked","block":"lowering","block_detail":<string>}`
  — typed `UnsupportedMeans` reject であり silent downgrade しない。
  存在しないが well-formed な target id は
  `{"outcome":"blocked","block":"unbound_target"}`。
  malformed params（未知 means・非 tagged target）は
  `invalid_params` で reject。

### PC-C4-06-04 — observe の stage 選別と Reported feed

- **Given**: fast write delivered → `engine.run` で plan 適用済み
  → drain 前の ledger。
- **When**: caller span（`plan_generation` = live plan generation）
  で `syn.control.observe` を呼ぶ。
- **Then**: observation row が stage を verbatim に持ち
  （`scheduled`/`dispatched`/`dropped`/`reported`/`unavailable`）、
  `dispatched` row は `call_start`/`frame_offset` を併記する。
  `fed==0` — host 解決済み stage は plugin-report channel に
  到達しない。`receipts` は `AppliedReceipt` の
  `plan_generation`/`transport_generation`/`status`/`applied_frame`
  を構造化して返し、caller が live generation を学べる。
- **And**: composition-root drain（`PluginHost::
  drain_output_reports`）で plugin report が ledger を `Reported`
  に進め、`note_control_drain` で記録した後の `observe` は
  `{"stage":"reported","value":<plugin-reported>}` を返し
  `fed==1`。workspace `ControlSession` の `plugin_reported` が
  `Estimated` stamp で `Stale` 観測状態になる。host が dispatch した
  value（0.7）ではなく plugin が報告した値が feed される。

### PC-C4-06-05 — generation gate fail-closed

- **Given**: surface generation=1 の app。
- **When**: `syn.control.observe` に `generation:2`（stale）または
  surface に存在しない processor tagged id を渡す。
- **Then**: `observe_cycle` 到達前に `service_error` で reject
  され、いかなる観測・feed も起きない。malformed span
  （未知 `status`・欠落 field）は `invalid_params`。

### PC-C4-06-06 — drain 記録の honest 既定値

- **Given**: drain を一度も実行していない app。
- **When**: `syn.control.status` を呼ぶ。
- **Then**: `drain` は `ReportDrainReport::default()` 相当
  （`attached:false`・counter 全 0）を返し、「drain 実施済み」を
  装わない。composition root が `note_control_drain` で記録した
  場合のみ実 drain の counter（`drained`/`param_reports`/
  `reported`/`generation_gated`/`inactive_gated`）が現れる。

### PC-C4-06-07 — NotRun 宣言

- `observe_cycle` の per-block 物理 call-window の RT runtime 内
  自動供給・`drain_output_reports` の engine 定期配線・production
  composition root の `PluginHost` 接続・実 VST3/OS backend
  （ruviform#253）・実 GUI（#114/#102）・AudioLens（#294）・
  oracle（#103）・wire schema の L1 検証は NotRun。実 plugin 値・
  parameter 列挙の捏造経路は存在しない（vend catalog +
  empty-catalog 経路は pc-c4-04 のまま）。

## Fixtures

- `crates/synvelis-app/tests/pc_c4_06_control_op_surface.rs`（新規）:
  - `control_ops_fail_closed_without_surface` — PC-C4-06-01。
  - `fast_control_and_status_return_typed_evidence` —
    PC-C4-06-02/03。
  - `observe_reports_stages_and_feeds_only_reported` —
    PC-C4-06-04/06（drain 記録 → status 反映を含む）。
  - `observe_fails_closed_on_stale_generation` — PC-C4-06-05。
  - fixture は `pc_c4_04_app_composition.rs` と同一の reporting
    plugin・project 構成（import clip + rec processor + param-7
    target + automation lane）を `SharedService` 経由で
    `AppService` に包み、全 drive を `UiRuntime::dispatch_op`
    経由で行う。`plan_generation` は receipts の適用済み世代 +
    op 末尾 pump の +1 規則で組み立て、receipts 自体が適用世代を
    検証する。

## Evidence

- `mise run test`（`cargo test --workspace --locked`、canonical）
  で新規 fixture 4 test green。
- `mise run check` / `mise run lint` / `mise run fmt-check`。
- `mise run test-rt` / `mise run test-msrv` / `mise run
  check-app-native` / `mise run lint-app-native` / `mise run build`。
- 実 plugin backend・実 GUI・L1 wire: NotRun（follow-up gate）。

## Guarantees

- `syn.control.*` op は `FastControlOutcome`・
  `SurfaceReport`/`TickReport`/`ReportDrainReport`・
  `DispatchStage` observation の typed evidence のみを返し、
  plugin 値・parameter 列挙・report を捏造しない。
- `Reported` stage の observation のみが workspace
  plugin-report channel へ feed され、`Scheduled`/`Dispatched`
  （host 解決済み）は plugin-reported として relabel されない。
- generation/capability gate と `StablePluginInstanceId`/
  `PluginMismatch` の fail-closed 意味論が service 境界まで
  維持される。
- drain 未実施・surface 未装着・RT 未観測は `null`/`false`/default
  で正直に示され、証拠のない状態を「観測済み」と装わない。

## Non-goals

- `observe_cycle` の RT 内 call-window 自動供給・
  `drain_output_reports` の engine 配線・production composition
  root の `PluginHost` 接続（後続 slice）。
- 実 plugin backend・実 GUI・AudioLens・oracle gate。
- wire schema の L1 検証（実 driver/GUI 結合）。
- `status` への live `plan_generation` 露出（player accessor が
  必要 — observe receipts が代替証拠）。
- parameter enumeration の新規経路（vend catalog +
  empty-catalog は pc-c4-04 のまま）。

## Risk

- caller が誤った `plan_generation` を span に供給すると
  projection gate で `Scheduled` 残留 — fail-safe 側（誤配送
  ではない）が receipts 構造化で学習経路を提供する。
- op 末尾の `ui_pump` が automation dispatch で plan generation
  を進めるため、連続 observe の span は世代を追う必要がある —
  fixture test が receipts でこの規則を検証する。
