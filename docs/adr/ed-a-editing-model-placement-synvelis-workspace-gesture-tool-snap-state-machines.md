---
id: "ADR-034QAmdUp9fg7PPVCOQtcx"
type: "adr"
status: "accepted"
title: "ED-A editing-model placement: synvelis-workspace gesture/tool/snap state machines + synvelis-project batch ops"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/38"
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
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/designs/ed-a-editing-model-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/190"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# ED-A editing-model placement: synvelis-workspace gesture/tool/snap state machines + synvelis-project batch ops

## Context and Problem Statement

Issue #38 の #11 合意編集モデルは (a) session scoped の View 側機構
(Selection/Tool/Snap/Gesture Preview) と (b) Project 正本への mutation
(clip の Move/Split/Delete/Create と atomic 複数対象 commit) の 2 面を持
つ。どちらをどのクレート・どの境界に置くか、ruviform 契約との分界をどう
守るかを決める必要がある。

## Decision Drivers

* AGENTS.md のマルチリポ運用: DAW 固有の編集意味は Synvelis 側、再利用可
  能な基盤は ruviform に重複実装しない・upstream 不具合を local
  workaround で隠さない (#11 §7)。
* WS-A (ADR ws-a) で確立した境界: Project 正本は `synvelis-project` の
  `ProjectController` 一箇所、View/Workspace 状態は `synvelis-workspace`。
* `ruviform-media-core` の `Tick`/`TickDuration` は unchecked `Add`/`Sub`
  のみ — 時間演算は checked API のみという lint 規約がある
  (`TickTimeMapping` は変換用に公開済み)。
* Gesture の Preview は Project を触らない View 側一時状態。Undo の単位は
  「1 gesture = 1 transaction」で、`ProjectController::apply` の scratch
  clone + 全件検証で atomicity を満たしたい。

## Considered Options

* Option 1: Selection/Tool/Snap/Gesture state machine は
  `synvelis-workspace` に新 module (`selection.rs`/`tool.rs`/`snap.rs`/
  `gesture.rs`) として追加。atomic multi-target commit は
  `synvelis-project` の `ProjectOp` に `Composite`/`SplitClip`/
  `RemoveClipsInRange` を追加して 1 `apply` = 1 undo で通す。
* Option 2: `synvelis-project` に editing 層 (tool/snap/gesture) を含める。
* Option 3: 編集用の新クレート `synvelis-editing` を新設する。

## Decision Outcome

Chosen option: "Option 1", because Tool/Snap/Gesture/Preview は明確に
session scoped の View 側責務 (issue #11 §6 の state 境界) であり
`synvelis-workspace` の ViewState に属する。一方 atomic commit・undo・
検証は Project 正本の責務であり、`ProjectController::apply` が既に
validate-all → scratch apply → atomic swap を提供するため、複数対象 commit
は `ProjectOp::Composite` として domain 層に置くのが authoritative な境界
— caller 側で複数 `apply` を連結する workaround を避けられる。

### Consequences

* Good, because 「1 gesture = 1 undo transaction」が controller の既存
  invariant (1 apply = 1 undo push + atomic swap) に自然に写像され、
  validate-all-then-commit を呼び出し側の規約ではなく op 自身の契約に
  できる。
* Good, because `SplitClip`/`RemoveClipsInRange` は Track/Lane/Clip model
  を知る domain op として project 層に置かれ、workspace は gesture の
  意味解釈 (どの clip をどう動かすか) だけを担当する。
* Good, because ViewState が serde されるため gesture/selections/tool の
  永続化境界が型で明示できる (Gesture は `#[serde(skip)]`)。
* Bad, because `ViewState` の persisted shape が変わり
  `WORKSPACE_SCHEMA` を 2 へ bump する (PoC 内の不整合を schema version
  で正直に表す)。
* Bad, because `synvelis-project` の op 集合が増え、media-domain 未対応の
  op (SplitClip) のような部分実装が残る — spec の deferred concerns に
  記録する。

### Confirmation

* `Composite` の validate が全 inner op に対して実行され、apply は既存の
  scratch-clone 経路のみを通ることを `mise run test` の atomicity test で
  確認。
* ruviform crate への変更・再実装がないこと (`git diff` / `cargo tree`)。
* Tool/Snap/Gesture が project の undo stack・`Project` 集約・
  `revision` に一切書き込まないことを ED-A-04 test で確認。

## Implementation Plan

* Affected paths: `crates/synvelis-project/src/ops.rs`,
  `crates/synvelis-workspace/src/{selection,tool,snap,gesture}.rs` (新規),
  `state.rs`/`command.rs`/`workspace.rs`/`session.rs`/`persist.rs`/`lib.rs`
  (変更), `crates/synvelis-workspace/tests/ed_a.rs` (新規)。
* Patterns to follow: WS-A の module 分割・`#[must_use]`・`missing_docs`・
  pedantic 準拠、`ProjectOp` の validate/apply 分離。
* Patterns to avoid: Preview の project 混入、gesture commit の複数 apply
  連結、px/デバイス固有値の入れ込み、ruviform 型の再実装。
* Dependencies or configuration: 新 dep なし (serde/ruviform-ui/
  synvelis-project のみ)。

## Verification

* [ ] `mise run fmt-check` / `check` / `lint` / `test` 全 PASS。
* [ ] 合意 5 項目それぞれの headless 受入 test が存在し PASS。
* [ ] `cargo tree` で ruviform への依存追加・変更がない。
* [ ] impl record に未検証項目 (px 閾値・OS キーバインド・実 device・
  Magnetic mode・混合 domain 演算) が正直に記録される。

## Pros and Cons of the Options

### Option 1

* Good, because 責務境界が issue #11 §6 の state owner 表と 1:1 に対応し、
  ruviform 契約を汚さず upstream issue 化の判断材料を残せる。
* Bad, because 2 クレートをまたぐ変更になる。

### Option 2

* Good, because mutation と gesture が 1 crate に近い。
* Bad, because session scoped の UI 状態 (tool/snap/preview) が永続
  Project 集約へ混入し、WS-A ADR の境界を壊す。

### Option 3

* Good, because 編集機構の独立性が最大になる。
* Bad, because Gesture は ViewState・LinkGroup・CommandRouter・session と
  強く結合しており、分離すると薄い facade の往復が増えて YAGNI に反する。
  WS-A が既に編集機構の受け皿として設計されている。

## More Information

- 上位 spec: `docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md`
- 設計: `docs/designs/ed-a-editing-model-design.md`
- 前提 ADR: `docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md`
- ruviform #189 (rational beat) / #190 (transport span) — 再実装しない契約
