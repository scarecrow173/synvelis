---
id: "IMPL-0340LyK9sQ2OUSutsQvSw"
type: "impl"
status: "completed"
title: "TIM-03 request plan — per-request ProcessBoundarySession + precision provenance"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/tim03_request_plan.rs"
      - type: "file"
        path: "docs/tasks/tim03-task3-request-plan.md"
      - type: "file"
        path: "docs/impl/ir/tim-03-request-plan-provenance.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/process_boundary.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "docs/specs/tim03-process-boundary.md"
      - type: "file"
        path: "docs/designs/tim03-process-boundary-design.md"
      - type: "file"
        path: "docs/plans/tim03-process-boundary.md"
      - type: "file"
        path: "docs/test-specs/tim03-process-boundary.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/tim03-task3-request-plan.md"
    - "docs/plans/tim03-process-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim03-task3-request-plan.md"
    - "docs/impl/ir/tim-03-attach-wiring-capability-attachconfig-composition-root.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim03-process-boundary.md"
    - "docs/impl/ir/tim-03-process-boundary-implementation-record.md"
    - "docs/impl/ir/tim-03-attach-wiring-capability-attachconfig-composition-root.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/tim03-task3-request-plan.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-03 request plan — per-request ProcessBoundarySession + precision provenance

## Summary

Issue #127 の per-request slice: task1 の non-RT oracle と
task2 の attach-time `AttachDelivery` 投影を、attached
`PluginNodeControl` に対する request 単位の orchestration API
`ProcessBoundarySession` として配線した。provenance pin
（instance/node/capability generation/graph generation）を全
request で照合し、stale/mismatch は typed
`BoundarySessionError` で staging 前に拒否 — 誤 Param/MIDI
配送 0 を fixture で固定。headless mock で 10 test を追加。

## Context

`docs/tasks/tim03-task3-request-plan.md` /
`docs/plans/tim03-process-boundary.md` /
`docs/specs/tim03-process-boundary.md` /
`docs/designs/tim03-process-boundary-design.md` に従う。
task1 IR（`tim-03-process-boundary-implementation-record.md`）と
task2 IR（`tim-03-attach-wiring-...`）の Follow-ups「per-request
boundary plan の runtime 配線」を本 slice で充足。

## Implementation

### Changes

- `crates/synvelis-player/src/process_boundary.rs`:
  - `PrecisionProvenance`: 4 軸 pin（`StablePluginInstanceId`/
    `NodeId`/capability generation/graph generation）。
    transport/plan generation は upstream scheduler 所有の
    event-scoped 軸であり `ParamDispatch` の receipt-gated
    observation が既に fault するため複製しない（設計判断）。
  - `BoundarySessionError`: upstream typed error
    （`ProcessBoundaryError`/`InstallPlanError`/
    `FrameBaseMismatch`/`DispatchError`）を variant として保持し、
    provenance mismatch は named variant で表現。
  - `ProcessBoundarySession::open`: `AttachClockContext::of` で
    verified clock context を取得し、`attach_delivery` を再評価
    — `Unsupported`/`OfflineUncertified` は session 生成前に
    fail-closed（`connect` と同一 typed refusal）。mode/
    consent/budget は session 固定で、oracle と scheduler の
    delivery policy が乖離しないことを保証。
  - request API（全て provenance 照合を staging/install の前に
    実行、mismatch は何も残さない typed refusal）:
    - `plan_window(current, window_frames, boundaries)` —
      純粋 evaluate。session 固定 mode/consent/budget。
    - `stage_window` — evaluate 成功後に `(TransportCommand,
      due_frames)` を `control.request` へ push。window 内 due
      は `Seek`/`Other` 境界として legality 検査済み。
    - `stage_plan` — window 内 due の `install_plan` を
      `PlanInstall` 境界として検査後に install。
    - `dispatch_params`/`dispatch_midi` — provenance +
      `dispatch.node()` pin 一致 + `check_frame_base` を通して
      から `ParamDispatch` へ委譲。foreign node/stale clock
      mapping は write 解決前に拒否。
- `crates/synvelis-player/src/lib.rs`: `PrecisionProvenance`/
  `BoundarySessionError`/`ProcessBoundarySession` re-export。
- `crates/synvelis-player/tests/tim03_request_plan.rs`（新規・
  10 test）: 実 `PluginNode`/`GraphRuntime`/`ParamDispatch`
  fixture。`tim03_cap_*` 3（unsupported typed refusal・offline
  exact fail-closed・realtime degraded consent 必須）、
  `tim03_req_*` 5（boundary matrix・min/max fail-closed・
  同時 loop/seek/tempo/param・plan install boundary・
  stage_window の provenance 事前拒否）、`tim03_prov_*` 3
  （stale generation 全軸で配送 0・foreign dispatch refusal・
  valid provenance での実配送）。
- docs chain: spec/design/plan/test-spec の deferred 記述を
  本 slice 充足に更新、task3 + 本 record 追加、index 再生成。

### Decisions During Implementation

- **provenance は配送文脈の generation pin**: task3 の
  `PrecisionProvenance` は「どの instance/node/世代に対して
  配送するか」の照合であり、`TimingPrecision` クラス自体の
  認定根拠（どの計測が `ExactWithinVerifiedCapability` を
  正当化するか）ではない。認定 provenance は実機 gate と
  併せて後続 slice に残す。
- **transport/plan generation は複製しない**: upstream
  `TransportScheduler`/`SpanSink`/`event_engine_frame` が
  event-scoped generation を既に照合し stale tag は fault +
  非配送。consumer 側で二重管理すると所有権が曖昧になるため、
  session は instance/node/capability/graph/frame-base の
  4+1 軸に限定（doc comment + design に明記）。
- **dispatch は head install で window 評価不要**: param/MIDI
  dispatch は callback head（due=0、offset-0 apply）に install
  され span を分割しないため illegal interior boundary を
  作らない。window legality は `plan_window`/`stage_window`/
  `stage_plan` の責務 — 責務分離を test で固定。
- **node の 1 plan 制約を test で固定**: `PluginNodeControl` は
  node あたり 1 plan を保持し last-writer-wins。Param plan
  install 後に MIDI plan を install すると前者が上書きされる
  upstream 契約を fixture で観測・文書化（本 slice で scheduler
  所有権を変えない）。
- **stale refusal は staging 前**: provenance 照合を
  `control.request`/`install_plan`/`ParamDispatch` の write
  解決より前に置き、mismatch 時は queue にも ledger にも
  何も残さない（誤配送 0 を test で固定）。

## Related Experiments

### Adopted

- なし（experiment log なし）。

### Rejected

- なし。

## Validation

- 補助証拠（canonical = mise）:
  - `cargo test -p synvelis-player --test tim03_request_plan`
    → 10 passed, 0 failed。
  - `cargo clippy -p synvelis-player --tests` → warning なし。
- canonical（`mise run`、全て exit 0）:
  - `fmt-check` / `check` / `lint`（clippy `-D warnings`）。
  - `test`（`cargo test --workspace --locked`、全 suite
    0 failed。`tim03_request_plan` 10・`tim03_process_boundary`
    17・`tim03_attach_wiring` 10・`tim03_process_boundary_rt`
    1 を含む）。
  - `test-rt`（`rt_` filter、RT hot path alloc-free 系を含む）。
  - `test-msrv`（rust 1.89 toolchain、全 suite 0 failed）。
- `git fetch origin main && git merge origin/main` 後の再検証:
  PR 本文に結果を記録。

## Risks

- `ProcessBoundarySession` を `driver`/`routing_control` の実
  dispatch 経路へ接続する composition は未実装 — 本 slice は
  session API + fixture まで。接続時に provenance の供給元
  （`PluginBindingSession`/project generation との結線）が
  必要で後続 slice。
- boundary offset の Tempo map/Loop occurrence 自動導出は
  未実装（caller が offset を与える — task1 からの継続 non-goal）。
- 実 CLAP/VST3/OS matrix・AudioLens f64/native PCM 実測・実 GUI
  は NotRun（別 gate: Ruviform #196/#253、synvelis #294/#114/
  #176）。
- doc-driven-dev-graph の route は pre-existing の broken
  relation（本 slice 無関係の doc 群）で blocked — TIM-02
  先例どおり tim03 chain を直接更新し、本 blocker は scope 外
  の既存問題として記録。

## Follow-ups

- `ProcessBoundarySession` を `driver`/`routing_control` の実
  dispatch 経路へ接続する composition — 後続 slice として
  計画されていたが、`tim-03-session-driver-wiring-slice.md`
  （`BoundarySessionSpec` vend・driver session/mirror・
  routing scoped API）で充足済み。
- `TimingPrecision` certified class の認定 provenance は未定義
  — task3 の `PrecisionProvenance` は配送文脈の generation pin
  であり認定根拠ではない。認定は実機 gate と併せて後続 slice
  （plan `tim03-process-boundary.md` Open Questions / Gaps）。
- boundary offset の Tempo map/Loop occurrence 自動導出は
  未実装 — caller が offset を与える non-goal が継続
  （`tim-03-session-driver-wiring-slice.md` の Follow-ups にも
  `tempo_revision = 0` 固定として残存）。
- 実 CLAP/VST3/OS matrix・AudioLens f64/native PCM 実測・実 GUI
  は NotRun — 別 gate（Ruviform #196/#253、synvelis
  #294/#114/#176）。
- doc-driven-dev-graph route を block する pre-existing の
  broken relation は scope 外の既存問題 — 修復は別途。
