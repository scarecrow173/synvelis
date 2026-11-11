---
id: "IMPL-034Vsqaiq4B9YQrsfPw8Sb"
type: "impl"
status: "completed"
title: "TIM-03 session→driver/routing wiring — BoundarySessionSpec vend・driver session/mirror・routing scoped API"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/tim03_session_wiring.rs"
      - type: "file"
        path: "docs/specs/tim-03-session-driver-wiring.md"
      - type: "file"
        path: "docs/designs/tim-03-session-driver-wiring-design.md"
      - type: "file"
        path: "docs/plans/tim-03-session-driver-wiring-plan.md"
      - type: "file"
        path: "docs/tasks/tim-03-task1-session-driver-wiring.md"
      - type: "file"
        path: "docs/test-specs/tim-03-session-driver-wiring.md"
      - type: "file"
        path: "docs/impl/ir/tim-03-session-driver-wiring-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/process_boundary.rs"
      - type: "file"
        path: "crates/synvelis-player/src/host.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_engine.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_control.rs"
      - type: "file"
        path: "crates/synvelis-player/src/driver.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/l1_e2e.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_b.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/tim02_midi_playback.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/tim-03-task1-session-driver-wiring.md"
    - "docs/plans/tim-03-session-driver-wiring-plan.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim-03-task1-session-driver-wiring.md"
    - "docs/impl/ir/tim-03-request-plan-provenance.md"
    - "docs/impl/ir/tim-03-attach-wiring-capability-attachconfig-composition-root.md"
    - "docs/impl/ir/tim-03-process-boundary-implementation-record.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/designs/tim-03-session-driver-wiring-design.md"
    - "docs/test-specs/tim-03-session-driver-wiring.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/tim-03-task1-session-driver-wiring.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-03 session→driver/routing wiring — BoundarySessionSpec vend・driver session/mirror・routing scoped API

## Summary

Issue #127 の session→driver/routing 接続 slice: task1 の oracle・
task2 の attach-time 投影・task3 の `ProcessBoundarySession` を、
実際の dispatch 経路（`HeadlessDriver` attached plugin・
`RoutingControlDispatch` generation surface）へ配線した。
attach 解決済み入力を `BoundarySessionSpec` として
host→engine→control の vend path で運び、driver は per-node
session + `TransportMirror` で捏造メタデータを実 transport 状態
投影に置き換え、routing は generation-scoped session と
RT-observed generation gate 付き scoped stage API を開いた。
headless fixture で 10 test を追加。

## Context

`docs/tasks/tim-03-task1-session-driver-wiring.md` /
`docs/plans/tim-03-session-driver-wiring-plan.md` /
`docs/specs/tim-03-session-driver-wiring.md` /
`docs/designs/tim-03-session-driver-wiring-design.md` /
`docs/test-specs/tim-03-session-driver-wiring.md` に従う。
task3 IR（`tim-03-request-plan-provenance.md`）の Follow-up
「session API + fixture まで — driver/routing composition は後続
slice」を本 slice で充足。`ProcessorConnection` は
`synvelis-render` が exhaustive destructure するため field 追加
不可 — `ProcessorControl` + generation-retained state 経由で
boundary 入力を運ぶ設計判断は design doc に記録。

## Implementation

### Changes

- `crates/synvelis-player/src/process_boundary.rs`:
  `BoundarySessionSpec`（capability/mode/consent/budget —
  `ProcessBoundarySession::open` が必要とする attach 解決済み
  入力の束）を追加。`open` が `attach_delivery` を再評価するため
  attach と矛盾する spec は fail-closed。
- `crates/synvelis-player/src/host.rs`:
  `ProcessorControl::boundary: Option<BoundarySessionSpec>`、
  `ProcessorHost::boundary_attach`（default `None`）、
  `PluginHostProcessors::boundary_spec` への解決集約
  （`boundary_delivery`/`boundary_attach` が同一 spec を共有）、
  `connect` の fail-closed 順序（boundary 評価を instantiate
  前に）を維持。
- `crates/synvelis-player/src/routing_engine.rs`:
  `RetainedConnection::boundary` に spec を保持し
  `take_processor_control` で一回限り vend。`stage_build` で
  `connect` 前に `boundary_attach` を呼び同一 request から解決
  （clippy 対応で clip half を `stage_playbacks`/`StagedPlayback`
  に抽出する局所 refactor を併施）。
- `crates/synvelis-player/src/routing_control.rs`:
  `NodeSurface::boundary`、`vend_nodes` での generation-scoped
  `ProcessBoundarySession::open`（instance id は binding session
  と同一 mint、provenance は surface の capability/graph
  generation pin）。open 拒否は typed `without_control` evidence
  で fail-closed。`BoundaryStageError` + scoped API
  （`boundary_session`/`boundary_provenance`/
  `boundary_plan_window`/`boundary_stage_window`）—
  `boundary_stage_window` は RT-observed
  `active_generation == surface.generation` を要求し、
  `current` provenance は session pin + live
  `port_capability_generation` + surface generation から再構成
  （caller は provenance を注入できない）。同一 generation の
  `sync` は session/dispatch を保持したまま binding のみ更新、
  別 generation は fresh instance/session を vend。
- `crates/synvelis-player/src/driver.rs`:
  `AttachedNode::boundary`、`AttachedDispatch`（dispatch +
  session? + `TransportMirror` + last_span）、activation 時の
  session open + anchor staging を publish 前に実行
  （`ActivateError::BoundarySession` で abort）。
  `TransportMirror` は scheduler の `apply`/`do_wrap` 則を投影:
  status/position anchor/loop/occurrence を command 成功時に
  fold、`transport_generation` は pending 予測 + drained
  `AppliedReceipt` で補正、`plan_generation` は dispatch の
  staged plan、`clock_epoch` は driver epoch、
  `tempo_revision` は 0（plan が tempo map を持たない）。
  `occurrence_before` で apply head の wrap preempt を再現
  （`Stop`/`Seek`/`ClearLoop` は head wrap を含めない、
  `Play` は inclusive）。transport API（`seek`/`set_loop`/
  `clear_loop`/`panic`/`resume_attached_transport`）を typed
  `Result<(), BoundarySessionError>` 化 — sessioned node は
  `stage_window` 経由、undeclared node は従来の lenient
  best-effort を維持（旧来 strict だった `set_loop` 系の
  `QueueFull` 伝播は `BoundarySessionError::QueueFull` に写像）。
  `set_loop` は node ごとの frame base で `MediaRange` を構成。
  `resume_attached_transport` は裸 `Play` ではなく
  `set_clock`+`Seek`+`Play` の re-anchor（activation と同一）。
  `attached_boundary`/`attached_span` accessor を公開。
- `crates/synvelis-player/tests/tim03_session_wiring.rs`（新規・
  10 test）: driver/routing 両経路の fixture。
  `tim03_sw_01`（session open + provenance pin + undeclared
  sibling）、`tim03_sw_05`（span ctx honesty — loop wrap の
  position/occurrence fold・panic/resume・receipt-derived
  generation）、`tim03_sw_10`（segment legality + oracle-illegal
  window typed refusal）、`tim03_sw_04_06`（`dispatch_params`
  が既存 seam で配送 + forged provenance 全軸 refusal で plan
  generation 不変）、`tim03_sw_06` driver（foreign `NodeId` →
  `NodeMismatch`）、`tim03_sw_07`（sessioned staging の
  `QueueFull` typed）、`tim03_sw_08`（unsupported spec が
  activation を fail-closed）、`tim03_sw_02`（routing session
  vend + pin が binding session と同一 identity）、
  `tim03_sw_03`（plan/stage window + `InactiveGeneration` +
  oracle refusal）、`tim03_sw_06` routing（forged provenance +
  generation swap で fresh pin + RT-observed gate）。
- 既存 test の signature 追従: `AttachedNode.boundary: None`
  追加（`l1_e2e`/`pc_b`/`tim02` 等 8 fixture）、fallible 化した
  transport method の call site 更新。
- docs chain: spec/design/plan/task/test-spec 新規 + 本 record、
  managed index 再生成。

### Decisions During Implementation

- **`ProcessorConnection` に field を足せない**:
  `synvelis-render` が exhaustive destructure し編集範囲外。
  boundary 入力は `ProcessorControl`（one-shot vend）+
  `RetainedConnection` の generation-retained state で運ぶ —
  design doc の責務分界どおり。
- **spec 解決は一箇所**: `PluginHostProcessors::boundary_spec`
  が `BoundaryDeclaration` → `BoundarySessionSpec` を唯一解決し、
  `connect`（→`attach_delivery`→`AttachConfig`）と
  `boundary_attach`（→vend）が同一入力を共有。downstream は
  policy/budget を再導出しない。
- **`boundary_attach` は `connect` と同一 `ProcessConfig`
  導出**: `ProcessConfig::new(sample_rate, 1, max_frames)` +
  `max_frames == 0` guard を鏡写し — spec が実 attach と乖離
  する経路を構造的に排除。
- **session identity は binding session と同一 mint**:
  `vend_nodes` は `PluginBindingSession` と
  `ProcessBoundarySession` に同じ `StablePluginInstanceId` を
  与え、provenance の capability/graph generation も surface
  pin で一致 — binding/boundary の identity 不整合を排除。
- **`TransportMirror` は命令成功時のみ fold**: stage が
  `QueueFull`/oracle refusal した command は mirror に記録
  しない。node-side `Rejected` receipt は generation 軸のみ
  訂正（`AppliedReceipt` が command kind を持たない —
  doc comment に明記した既知の限界）。caller-side の
  `control()`/`stage_window` 直接 staging は mirror に見えず
  receipt が次 block で補正（param plan は transport
  generation を bind しないため不活性）。
- **`occurrence` の既知 edge**: 同一 head の command batch で
  `SetLoop` が raw position を新 loop end より先に残し、後続の
  preempt 命令（`Stop`/`Seek`/`ClearLoop`）がある場合、mirror
  の凍結 occurrence が scheduler より進み得る。`occurrence`
  は trace/diagnostic metadata のみ（event gating は
  status/position/generation を読む）— 配送レベルではなく
  証拠レベルの乖離として doc comment に記録し、今回は修正
  しない（発生条件が狭く影響が証拠のみのため）。
- **legacy lenient/strict の保存**: 旧来 `let _ =` だった
  `seek`/activation/resume の queue drop 寛容を維持し、旧来
  `?` だった `set_loop`/`clear_loop`/`panic` の `QueueFull`
  伝播を `BoundarySessionError::QueueFull` に写像 — undeclared
  node の observable 挙動を変えない。
- **`boundary_plan_window` は active-generation gate なし**:
  純粋評価で staging を伴わない。staging 側
  （`boundary_stage_window`）が RT-observed generation を
  要求するため unsafe path はない。

## Related Experiments

### Adopted

- なし（experiment log なし）。

### Rejected

- なし。

## Validation

- 補助証拠（canonical = mise）:
  - `cargo test -p synvelis-player --locked` → 全 suite 0 failed
    （`tim03_session_wiring` 10 test を含む）。
- canonical（`mise run`、結果は PR 本文に記録）:
  - `fmt-check` / `check` / `lint`（clippy `-D warnings`）。
  - `test`（`cargo test --workspace --locked`）。
  - `test-rt`（`rt_` filter）。
  - `test-msrv`（rust 1.89 toolchain）。
  - `check-headless-deps`。
- `git fetch origin main && git merge origin/main`（ruviform pin
  `0f5b5737` → `b02942c2` を含む 7 commit 取込）後に上記を
  再実行 — 結果は PR 本文に記録。
- review-protocol: Reviewer A（cooperative）/ Reviewer B
  （adversarial）の分離 pass + candidate validation + judge を
  実施。CONFIRMED blocking なし — `command_boundary` の不要な
  `pub(crate)` widening を private に戻し、mirror の occurrence
  edge を doc comment に記録（P3 相当の是正）。

## Risks

- `TransportMirror` は driver が command した状態の投影であり、
  node 側の実 scheduler state を直接観測しない — stage 後 apply
  前の窓では commanded-state を報告する（receipt が generation
  軸を補正）。実 transport state owner が scheduler 側にある
  部分の完全な観測は別 slice。
- Tempo/Loop の自動導出（boundary offset の tempo map・loop
  occurrence 導出）は未実装 — 本 slice は caller 供給 offset の
  配線まで。precision certification 実測・実 plugin/OS・
  consumer 照合は別 slice/別 gate。
- `lib.rs` は編集範囲外のため新規 pub 型
  （`BoundarySessionSpec`/`BoundaryStageError`）は module path
  （`process_boundary::`/`routing_control::`）経由でのみ到達
  可能 — crate root re-export は後続 slice で検討。
- 実 CLAP/VST3/OS matrix・AudioLens f64/native PCM 実測・実 GUI
  は NotRun（別 gate）。Issue #127 は Open のまま。

## Follow-ups

- Tempo map revision・loop occurrence の自動導出を span context
  へ接続する slice（現状 `tempo_revision = 0` 固定・caller
  offset 供給）。
- `ProcessorControl::boundary`/`BoundaryStageError` 等の
  crate root re-export（`lib.rs` 編集を伴う後続 slice）。
- `TransportMirror` の occurrence edge（同一 head SetLoop
  overflow + preempt command）を scheduler と完全一致させる
  場合は batch-aware な occurrence tracking が必要 — 証拠
  レベルのため必要性は低い。
