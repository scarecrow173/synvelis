---
id: "IMPL-034PCA12Xz9hCd5kLmZnOp"
type: "impl"
status: "completed"
title: "PC-A parameter control model headless PoC"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/40"
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/185"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_a.rs"
      - type: "file"
        path: "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
      - type: "file"
        path: "docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md"
      - type: "file"
        path: "docs/designs/pc-a-parameter-control-design.md"
      - type: "file"
        path: "docs/plans/pc-a-parameter-control.md"
      - type: "file"
        path: "docs/tasks/pc-a-task1-control-model-deps.md"
      - type: "file"
        path: "docs/tasks/pc-a-task2-control-ops-conflicts.md"
      - type: "file"
        path: "docs/tasks/pc-a-task3-eval-graph-dispatch.md"
      - type: "file"
        path: "docs/tasks/pc-a-task4-control-session.md"
      - type: "file"
        path: "docs/tasks/pc-a-task5-acceptance-tests.md"
      - type: "file"
        path: "docs/tasks/pc-a-task6-verification-impl-record.md"
      - type: "file"
        path: "docs/test-specs/pc-a-e2e-parameter-control.md"
      - type: "file"
        path: "docs/impl/ir/pc-a-parameter-control-headless-poc.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-project/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-project/src/error.rs"
      - type: "file"
        path: "crates/synvelis-project/src/id.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-workspace/src/error.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/session.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-a-parameter-control.md"
    - "docs/tasks/pc-a-task1-control-model-deps.md"
    - "docs/tasks/pc-a-task2-control-ops-conflicts.md"
    - "docs/tasks/pc-a-task3-eval-graph-dispatch.md"
    - "docs/tasks/pc-a-task4-control-session.md"
    - "docs/tasks/pc-a-task5-acceptance-tests.md"
    - "docs/tasks/pc-a-task6-verification-impl-record.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/designs/pc-a-parameter-control-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-a-e2e-parameter-control.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-A parameter control model headless PoC

## Summary

Issue #40 — issue #6 で合意された Automation/Modulation parameter
control model の headless PoC — を `crates/synvelis-project` (persisted
authoritative model + evaluation) と `crates/synvelis-workspace`
(session state machines) に実装した。ruviform contract は
`ruviform-plugin-core` (param write/capability・rf#186) と
`ruviform-media-core` (musical time・rf#189、timed input/chase・rf#188、
observation provenance・rf#185) を直接消費し、local reimplementation
は行っていない。受入 8 領域を `tests/pc_a.rs` 9 test で検証。
project `SCHEMA_VERSION` は 1 → 2。

## Context

Spec: docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md
ADR: docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md
Design: docs/designs/pc-a-parameter-control-design.md
Plan: docs/plans/pc-a-parameter-control.md / Test spec:
docs/test-specs/pc-a-e2e-parameter-control.md。

## Implementation

### Changes

- `Cargo.toml` / `crates/synvelis-project/Cargo.toml` /
  `crates/synvelis-workspace/Cargo.toml`: `ruviform-plugin-core` を
  workspace git dep として追加 (既定 branch 追従、固定 SHA なし)。
  `synvelis-workspace` に `ruviform-media-core` も追加。
  `Cargo.lock` は ruviform 全 crate を `94c5bf8` → `fdd65da9` に
  再解決 (branch-tracking policy の当然の結果)。
- `synvelis-project/src/control.rs`: persisted control model 一式。
  `BeatPoint` (正規化有理数 beat 座標、checked 演算)、`BeatRange`
  (half-open)、`ControlScope` (`TrackProject` / `ClipLocal` /
  `NoteLocal` = 明示的 unsupported)、`ValueKind` (validate +
  quantize)、`ParamMapping` (Linear/Exponential)、
  `TargetCapability` (fixture → ruviform `ParamInfo`/`ParamFlags`
  bridge)、`ParameterTarget`、`AutomationLane`/`AutomationRegion`/
  `AutomationPoint`/`Interpolation`、`ModulationSource` (Lfo/Macro/
  StepSequence)、`ModulationAssignment` (transform/polarity/depth/
  depth-source/enabled)、`ControlModel` (project 直下の単一
  authoritative store)、`detect_conflicts` (同一 target・同一評価
  時間の重複検出 — silent priority なし)、`ConflictPolicy`
  (`Reject`/`Replace`)、`ControlEvaluation` (reference/automation/
  per-source contribution/raw/clamped/quantized/plain + provenance)、
  `resolve_scope_pos` (scope 座標解決)、`evaluate_control`、
  `ControlGraph::compile` → `EvalPlan` (zero-delay cycle reject +
  deterministic topological order)、`dispatch_check` /
  `lower_event` (ruviform `check_param_write` → `TimedParamEvent`
  lowering; `TimedEvent` 非対応 capability では design 指定の
  `BlockBoundary` fallback のみ)。
- `synvelis-project/src/ops.rs`: control 系 `ProjectOp` 追加
  (`AddParameterTarget`/`SetTargetBase`/`AddAutomationLane`/
  `WriteAutomationRegion`/`RemoveAutomationRegion`/
  `AddModulationSource`/`AssignModulation`/`SetAssignmentEnabled`/
  `RemoveAssignment` 等)。全て既存の atomic validate→apply
  (scratch) 経路に載り、失敗は live project 不変のまま。
- `synvelis-project/src/project.rs`: `Project.control: ControlModel`
  追加 + `validate_references` に control 参照整合性 (target/
  source/lane/assignment/scope/capability/conflict) を追加。
- `synvelis-project/src/error.rs`: `AutomationConflict` /
  `ControlCycle` / `ScopeUnavailable` / `WriteRejected` variant。
- `synvelis-project/src/id.rs`: `ParameterTargetId` /
  `AutomationLaneId` / `ModulationSourceId` / `AssignmentId`。
- `synvelis-project/src/store.rs`: `SCHEMA_VERSION` 1 → 2。
- `synvelis-workspace/src/control.rs`: `ControlSession` — session
  所有の transient 状態のみ (永続 series を持たない)。
  `AutomationMode` (`Off`/`Read`/`Touch`/`Latch`/`Write`)、
  `WriteArm` (`Armed`/`Disarmed`)、`PlaybackState`/`PlaybackStatus`、
  `ControlGesture` (fixation)、`RecordingTransaction` (points は
  memory 蓄積 → 1 composite commit or abort)、`SealReason`、
  `ControlObservation`/`ObservationState` (`AtPlayhead`/`Live`/
  `Captured`/`Stale`/`Unavailable`)、`feed_plugin_report`/
  `feed_probe` (observation 入力)、`lower` (gesture/recording 値を
  upstream write-check 経由で `TimedParamEvent`/`BlockBoundary`
  に lowering)、`stamp_gesture` (rf#188 `InputStamp`/`ClockAnchor`
  → frame mapping、`Carried`/`Derived`/`Estimated` provenance)、
  `lane_view`/`signal_flow_view`/`inspector_breakdown` (3
  projection は同一 `project.control` を読む read-only view —
  series の複製なし)、`target_scope`。
- `synvelis-workspace/src/session.rs`: `ProjectSession` に
  `ControlSession` を集約 + `control_with` bridge (borrow-safe
  に session の controller + project を同時参照)。
- `synvelis-workspace/src/error.rs`: `StampRejected` variant。
- `tests/pc_a.rs`: PC-A-01..08 受入 9 test。

### Decisions During Implementation

- `BeatPoint`/`BeatRange` は persisted wire DTO とした —
  ruviform `BeatPosition`/`TickPosition` は serde 未実装のため
  runtime 変換は `to_beat`/`from_beat` で行い、永続形式は
  Synvelis 所有。これは upstream contract の再実装ではなく
  persistence 層の責務。
- `TargetCapability` は fixture メタデータ → upstream
  `ParamInfo`/`ParamFlags` への adapter であり、capability 判定の
  authoritative は `check_param_write`/`check_param_write_for`
  (upstream) のまま。write means matrix は Synvelis 側に複製しない。
- `TimedEvent` を受理しない capability への fallback は
  `BlockBoundary` への明示 lowering のみで、rejection を隠さない
  (`Lowered` enum が降格結果を返し、observation 側に残る)。
- recording commit は `ProjectOp::Composite` 1 件 — 1 session =
  1 undo の要件を controller の既存 atomic/undo 意味で満たすため
  新たな undo 経路は作らない。loop wrap は `SealReason::LoopWrap`
  で seal し、記録済み interval を黙って上書きしない。
- command issuance (record_start) が target 集合 + revision を
  fixation するため、Inspector 等の follow 変更は write を
  retarget できない (revision 不一致は `record_write` で reject)。
- mode 選択 (`set_mode`/`set_arm`/`set_record_enabled`) は
  `ControlSession` のみを書き、`ProjectOp` を発行しない —
  project revision/graph rebuild 不変を test で検証。
- Note scope は note entity model が存在しないため
  `ScopeUnavailable` を返すのみ (spec/design で unsupported と
  宣言済み)。

## Related Experiments

### Adopted

なし — 承認済み設計どおりに実装。

### Rejected

- ruviform `BeatPosition`/`TickPosition` を serde 化する local
  patch — upstream 型の永続化責務を Synvelis に持ち込まないため。
- conflict を lane 優先度で暗黙解決する案 — issue #6 の「明示的
  conflict detection」要件に反するため `AutomationConflict`
  reject + 明示 `Replace` policy のみ。

## Validation

| 項目 | 結果 |
| --- | --- |
| `mise run fmt-check` | PASS |
| `mise run check` | PASS (workspace --all-targets) |
| `mise run lint` | PASS (`cargo clippy --workspace --all-targets -- -D warnings`) |
| `mise run test` | PASS (`cargo test --workspace --locked` 全 crate green。pc_a 9 件 + 既存 ed_a/ws_a/vs_*/l1/l2/x 系全件含む) |
| ruviform 依存変更 | `ruviform-plugin-core` 追加。全 ruviform crate `94c5bf8` → `fdd65da9` 再解決 (branch-tracking、固定 SHA なし) |

受入対応: PC-A-01 `pc_a_01_*` (lane/signal-flow/inspector 3
projection が同一 persisted series を観測、view は series を
複製しない)、PC-A-02 `pc_a_02_*` (track/project scope と
clip-local scope の分離、musical-anchored clip move で clip-local
意味を保持し project-time 配置のみ変化、note scope は
`ScopeUnavailable`)、PC-A-03 `pc_a_03_*` (同一 target+時間の
重複は `AutomationConflict` reject・project 不変、明示
`Replace` policy で overwrite、`detect_conflicts`/
`validate_references` の load-time 整合)、PC-A-04 `pc_a_04_*`
(base/automation/reference/per-source contribution/host-resolved/
plugin-reported breakdown + observation `AtPlayhead`/`Live`/
`Captured`/`Stale`/`Unavailable` + stamp provenance)、PC-A-05
`pc_a_05_mode_select_*` + `pc_a_05_recording_*` (mode 選択が
revision/graph_builds 不変、Off gesture 完結は単一 undoable
base edit、Read は write なし、Touch/Latch/Write は armed +
record-enabled + rolling + open transaction gate、recording は
manual/reference 値のみ = double-modulation なし、1 session = 1
composite = 1 undo、loop wrap で seal・silent overwrite なし、
abort は mutation なし)、PC-A-06 `pc_a_06_*` (depth-source
zero-delay cycle reject、deterministic eval order)、PC-A-07
`pc_a_07_*` (readonly 全 write path reject、stepped quantize、
TimedEvent 非対応 → BlockBoundary fallback は観測可能、
unsupported は honest reject)、PC-A-08 `pc_a_08_*` (tempo 変更を
跨ぐ beat↔media 変換一貫性、sample/beat roundtrip、clip-local
座標、loop boundary、playback vs offline evaluate の一致)。

### 既知の repo 健全性(既存・本スライス非因果)

- `doc_status audit` は architecture/vs-c 系の既存 findings を
  含む可能性がある (WS-A/ED-A と同様)。PC-A 成果物についての
  新規 blocker は self-review で解消済み。

### 未検証(正直な記録)

- 実 plugin への param write 適用: 対象外 (fixture capability +
  `check_param_write` 判定 + `TimedParamEvent` lowering まで。
  upstream dispatch は fake engine で検証)。
- 実 GUI/drawing・lane rendering・OS keybinding・pen/touch:
  対象外。
- RT thread での bounded evaluation / non-RT prepare の分離:
  `EvalPlan` は prepare (non-RT compile) と評価データを分離する
  設計だが、実 RT context での allocation-free 検証は未実施
  (`vs_b_rt_alloc` 型の検証は本スライス対象外)。
- offline render driver 経由の evaluation: `evaluate_control` が
  context (playback frame 指定) に一致するかは unit test で検証
  したが、synvelis-render pipeline との統合は未実施。
- rf#185 probe の実 host 配線: `feed_probe`/`feed_plugin_report`
  API と state/provenance は検証済み、実 plugin からの自動
  observation 配線は host 統合時に確認要。
- `mise run test-msrv`: rust 1.89 toolchain 未導入のため未実施
  (pinned 1.99.0 で全検証)。
- schema 1 → 2 の移行経路: なし (意図的破壊的変更、ED-A と同じ
  PoC 方針)。

## Risks

- `SCHEMA_VERSION` 2 で schema 1 snapshot は読めない — PoC 間の
  意図的破壊的変更であり移行経路は提供しない。
- ruviform branch-tracking 依存のため、次回 lockfile 再解決で
  upstream contract が変化する可能性。`check_param_write` の
  means×flags matrix が upstream で変更された場合、本実装は
  upstream 判定に従う (matrix を local 複製していないため挙動は
  upstream に追従するが、fallback 適否の前提が変わり得る)。
- `BeatPoint` の denominator は `u32` (recording commit の
  last-point range 延長は 2^-32 beat — docs/design で文書化済み
  の最小分解能)。極端に高精度な点列では表現不能な差が出得る。
- `detect_conflicts` は O(regions²) — PoC 規模では問題ないが、
  大規模 project では index 化が必要。

## Follow-ups

- 実 plugin adapter 接続時に `TimedParamEvent` の実 dispatch と
  host→`feed_probe`/`feed_plugin_report` の自動配線を統合
  (issue #6 後続)。
- Note entity model 導入時に `ControlScope::NoteLocal` の評価を
  実装 (現状は `ScopeUnavailable` で honest reject)。
- modulation→automation bake、multi-pass take recording、
  feedback/delay control graph は issue #40 スコープ外のまま
  (issue #6 の deferred 項目)。
- GUI lane/dock integration (ruviform #243 系) で
  `lane_view`/`signal_flow_view`/`inspector_breakdown` を直接
  view model として接続可能。
