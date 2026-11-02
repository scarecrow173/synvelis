---
id: "ADR-034PCA02Mh7wQr3xZaNbCd"
type: "adr"
status: "accepted"
title: "PC-A parameter-control placement: synvelis-project persisted model + evaluation, synvelis-workspace session state machines"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/40"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/designs/pc-a-parameter-control-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
    - "docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/185"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# PC-A parameter-control placement: synvelis-project persisted model + evaluation, synvelis-workspace session state machines

## Context and Problem Statement

Issue #40 の #6 合意 Parameter Control Model は (a) Project 永続対象
(Target/Lane/Source/Assignment・Base・Curve) と (b) session scoped の
制御機構 (UI Mode・Arm・Gesture・Recording Transaction・Playback・
Observation) と (c) 純粋な評価規則 (Reference 解決・Modulation 合成・
Clamp/量子化・DAG 順序) の 3 面を持つ。どれをどのクレート・どの境界に
置くか、ruviform 契約 (rf#186/#189/#188/#185) との分界をどう守るかを
決める必要がある。

## Decision Drivers

* AGENTS.md のマルチリポ運用: DAW 固有の Automation/Modulation 意味は
  Synvelis 側、ruviform 契約型の再実装・重複実装はしない、upstream
  不具合を local workaround で隠さない。
* WS-A (ADR ws-a) / ED-A (ADR ed-a) の確立境界: Project 正本は
  `synvelis-project` の `ProjectController` 一箇所、session scoped
  の UI 機構は `synvelis-workspace`。
* 「mode 選択だけでは Project を更新しない」(#6 決定④) — UI Mode が
  Project 集約にあると mode 変更が undo 履歴を汚染するため、Mode/
  Arm/Gesture/Recording は session state である必要がある。
* 評価規則は project データと明示的な runtime 入力のみの純粋関数で
  あり、offline render と再生で同一結果を返す必要がある — workspace
  の session 詳細に依存させると同一性検証が難しくなる。
* `ruviform-plugin-core` (rf#186) は現行 workspace 依存に無い新規
  crate。`ruviform-video-core` までの path 依存を持つが全て同一 git
  rev `94c5bf8` で解決される (rev 固定のまま crate 追加のみ)。
* `BeatPosition`/`ParamValue`/`ParamInfo`/`ParamFlags` は serde を持た
  ないため、`MediaTimeValue` と同じく wire DTO + 境界変換が必要。

## Considered Options

* Option 1: 永続モデル + 評価規則 + capability/配送契約は
  `synvelis-project` の `control` module (model/eval/graph/dispatch)
  へ。UI Mode/Arm/Gesture/Recording/Playback/Observation/projections は
  `synvelis-workspace` の `control` module (`ControlSession`) へ。
* Option 2: 全て `synvelis-workspace` に置き、project は op payload の
  素通しだけを行う。
* Option 3: 新クレート `synvelis-control` を新設する。

## Decision Outcome

Chosen option: "Option 1", because 評価規則は Project データだけを読む
domain 責務であり `synvelis-project` に置くのが authoritative な境界
(ED-A と同じく op の validate/apply 分離に載る)。Mode/Arm/Gesture/
RecordingTransaction/Observation は session scoped の実行状態であり
`synvelis-workspace` の `ControlSession` に属する — mode 選択で
revision が進まないことを型レベルで保証できる。capability 検証と
`ParamEvent`/`TimedParamEvent` への lowering は書込み契約そのものなので
project 側の dispatch 層に置き、workspace は session の mode/arm/transport
を注入するだけにする。

### Consequences

* Good, because 「1 recording session = 1 undo」が既存 invariant
  (1 apply = 1 undo push + atomic swap) に自然に写像される —
  RecordingTransaction の commit は `Composite` 化した erase+write
  op 列の単一 `apply` で実現できる。
* Good, because 評価が純粋関数 (`evaluate(&Project, …, pos)` +
  caller 注入の manual/reference 文脈) になり、offline render と
  再生の一致が同じ呼び出しの 2 回評価として検証できる。
* Good, because Lane/SignalFlow/Inspector 投影は `Project::control`
  への読み取り関数 3 本になり、「二重の系列を持たない」が構造的に
  保証される (複製する場所が存在しない)。
* Good, because `ruviform-plugin-core` への依存は `synvelis-project`
  側だけに閉じ、workspace は `WorkspaceError` 経由で rejection を
  運ぶだけになる。
* Bad, because `Project` の persisted shape が変わり `SCHEMA_VERSION`
  を 1 → 2 へ bump する (PoC 間の意図的破壊的変更として schema
  version で正直に表す。`control` には `#[serde(default)]` を付けて
  旧 snapshot の構造的非互換を局所化する)。
* Bad, because `ruviform-plugin-core` 追加で依存グラフが増える
  (同一 rev の ruviform crate 群のみ、rev 自体は固定)。

### Confirmation

* `ControlSession::set_mode` 等が `session.apply` を一切呼ばないこと
  を PC-A-05 test (mode 選択で revision 不変) で確認。
* ruviform crate への変更・再実装がないこと (`git diff` /
  `cargo tree`)。新規 dep は `ruviform-plugin-core` 1 件のみ、
  rev `94c5bf8` 維持。
* `Project::control` 以外に点列・寄与の複製を保持する構造がない
  ことを PC-A-01 test で確認。

## Implementation Plan

* Affected paths: `crates/synvelis-project/src/control.rs` (新規、
  model+eval+graph+dispatch), `id.rs`/`project.rs`/`ops.rs`/
  `error.rs`/`lib.rs` (変更), `crates/synvelis-workspace/src/control.rs`
  (新規), `session.rs`/`error.rs`/`lib.rs`/`workspace.rs` (変更),
  `crates/synvelis-project/Cargo.toml` + root `Cargo.toml`
  (`ruviform-plugin-core` 追加), `crates/synvelis-workspace/tests/pc_a.rs`
  (新規)。
* Patterns to follow: WS-A/ED-A の module 分割・`#[must_use]`・
  `missing_docs`・pedantic 準拠、`ProjectOp` の validate/apply 分離、
  `MediaTimeValue` 流の wire DTO (`BeatPoint`)。
* Patterns to avoid: Preview/Observation の project 混入、recording
  commit の複数 apply 連結、ruviform 型の再実装、`Mode` の永続化、
  silent priority resolution。
* Dependencies or configuration: `ruviform-plugin-core` git dep 追加
  (同一 rev)。serde は `f64` 直接保存を避け、normalized 値は検証付き
  DTO 経由。

## Verification

* [ ] `mise run fmt-check` / `check` / `lint` / `test` 全 PASS。
* [ ] 受入 PC-A-01..08 それぞれの headless test が存在し PASS。
* [ ] `cargo tree` で ruviform 側の rev 変更・再実装がない
  (plugin-core 追加のみ)。
* [ ] impl record に未検証項目 (実 adapter・sample-accurate・GUI・
  Take multi-pass・Note 実体) が正直に記録される。

## Pros and Cons of the Options

### Option 1

* Good, because 責務境界が #6 §6 論理モデルの責務分割と 1:1 に対応し、
  「mode 選択で project 不変」を型で保証できる。評価が project への
  純粋関数になるため offline/playback 一致の検証が自明になる。
* Bad, because 2 クレートをまたぐ変更 + schema bump になる。

### Option 2

* Good, because 変更が 1 クレートに近い。
* Bad, because 評価・capability・配送契約が session 層に漏れ、
  offline render が workspace の session 詳細に依存する。project op
  の validate に capability 検証が入れられず境界が曖昧になる。

### Option 3

* Good, because 制御機構の独立性が最大になる。
* Bad, because ControlSession は ProjectSession・transport・command
  fixation・view state と強く結合しており、分離すると薄い facade
  の往復が増えて YAGNI に反する。永続モデルは Project 集約に属す
  ため結局 project 側変更は避けられない。

## More Information

- 上位 spec: `docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md`
- 設計: `docs/designs/pc-a-parameter-control-design.md`
- 前提 ADR: `docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md`,
  `docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md`
- ruviform #186 (param write contract) / #189 (BeatPosition) /
  #188 (stamp→frame 写像) / #185 (観測 timestamp/provenance) —
  再実装しない契約
