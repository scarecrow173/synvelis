---
id: "SPEC-034X9BMCsZFv8cyn2cAg8d"
type: "spec"
status: "draft"
title: "Signal Flow 設計正本 — 階層・Typed Port/Connection・Semantic Routing・Probe 表示/編集・SourceMap/Configured/Observed 受入仕様"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/4"
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
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
    - "docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md"
    - "docs/designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/4"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/108"
    - "https://github.com/scarecrow173/synvelis/issues/109"
    - "https://github.com/scarecrow173/ruviform/issues/184"
    - "https://github.com/scarecrow173/ruviform/issues/185"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
  defers: []
  deferred-by: []
---

# Signal Flow 設計正本 — 階層・Typed Port/Connection・Semantic Routing・Probe 表示/編集・SourceMap/Configured/Observed 受入仕様

## What Should Be Built

Issue #4「Signal Flow の階層・型付き Port/Connection・Routing 操作・
標準 Probe を定義する」の**設計正本タスク**。コード実装は行わず、
Signal Flow を「階層的な探索・接続の直接編集・ノード出力/入力の
観測・診断につながる DAW View」として定義する単一の設計正本を
`docs/` の文書チェーンとして確定する。

具体的に確定する内容（設計正本の対象領域）:

- **Signal Flow 階層モデル**: Semantic Hierarchy（処理/所有構造）と
  Visual Hierarchy（表示整理）の分離、Container/Processor/Endpoint
  の Node 分類、Local/Expanded/Overview の探索 scope、Container
  展開・折りたたみ、Semantic Zoom、Audio Main / Send・Return /
  Sidechain / MIDI・Event / Automation の意味レイヤー。
- **Typed Port/Connection ルール**: `SemanticPort` の方向・信号
  Domain・Role・Channel Layout・Multiplicity・Availability・
  Probe Capability、`RoutingConnection` の Main/Send/Sidechain
  区別、接続検証の分類（方向・信号型・Channel・循環・Port 不在）。
- **Semantic Routing 表示/編集ルール**: Node click=Inspect、
  Node move=View State、Port Drag=Candidate Preview→明示
  Confirm=1 Project CAS/Undo、Connection click=調査、
  Effect Chain 専用ハンドルのみが DSP 順序を変更、Mixer との
  単一正本整合。
- **Probe 表示/編集ルール**: 全論理ノードの標準 Probe 能力宣言、
  Off/Monitor/Capture、能力・Unavailable の一貫表示、
  Configured/Observed/Inferred/Unavailable の区別。
- **SourceMap/Configured/Observed 受入仕様**: Project
  `RoutingModel`（Configured）→ Active Plan → 実測 Observed の
  世代証拠分離、Unknown/Stale を推定値に置換しない規則、Issue #4
  の受入項目をシナリオ ID で追跡可能にする対応表。
- **実装 Owner 境界**: 実装は #104（SF-01）〜#109（SF-06）が Owner。
  本タスクは境界の明記までで、`crates/`・Ruviform・CI 設定には
  触れない。

## Why Now

Issue #4 は 2026-11-04〜10 の合意で製品・UX 方針が確定し、
SF-01〜06 の実装 Issue（#104〜109）と Ruviform Generic
Graph/Probe（#184/#185/#308/#309）が main に大部分統合済み。
一方で「Signal Flow の階層・Port/Connection・Routing 編集・
Probe 表示の**設計正本**」を一箇所に固定した文書がなく、
#108（Signal Flow GUI）以降の slice が個別 spec/design に分散
した契約を参照している。sf-03-map（#106）が lowering/activation
の統合設計権威として機能したのと同型に、Signal Flow View 側の
統合設計権威を確立し、後続の編集・Probe 共有・実機受入 slice が
独自解釈で契約を再実装するのを防ぐ。

## Users and Value

- #104〜109 の実装 Owner・レビューア: Signal Flow の階層・
  Port/Connection・編集・Probe 表示の authoritative な参照先。
- #109（SF-06 oracle）・#114（実 OS GUI gate）の受入担当:
  headless で検証済みの契約と NotRun 項目を区別して評価できる
  scenario ID 対応表。
- 将来の Signal Flow slice（編集 UX・Probe 共有・Visual Group
  高度 UI）の実装者: 未決事項と決定済み事項の境界を誤認しない。

## Scope

### In Scope

- `docs/specs/signal-flow-design.md`（本書）・
  `docs/designs/signal-flow-design-design.md`・
  `docs/plans/signal-flow-design-plan.md`・
  `docs/tasks/signal-flow-design-task.md`・
  `docs/test-specs/signal-flow-design.md`・
  `docs/impl/ir/signal-flow-design.md`・
  `docs/impl/exp/signal-flow-design.md` の文書チェーン。
- `evidence/sv4/` 配下の設計根拠・既存 SF 系 spec/design との
  整合確認記録。
- 触った docs dir の管理 index 再生成（`docs/*/README.md`）。
- review-protocol による文書レビューと blocker 解消。

### Out of Scope

- `crates/` 以下のコード変更、`Cargo.toml`/`Cargo.lock`/
  `mise.toml`/`.github/workflows/` 等の設定変更。
- Signal Flow 機能の実装そのもの（実装 Owner は #104〜109）。
- 既存 SF-01〜06 文書の supersede/書き換え（本チェーンは上位の
  設計正本として追加し、slice 文書は実装記録として残す）。
- Ruviform Generic Graph/Compiler/Probe/Clock の変更・再実装。
- 実 OS GUI・実 Plugin・実 Device の受入（#114/#109 の外部 gate）。
- `docs/designs/overview.md` 等、本チェーン以外の docs ファイルの
  内容変更。

## Requirements

設計正本が満たすべき内容要件:

- **SFD-R1 階層モデル**: Semantic Hierarchy と Visual Hierarchy の
  分離、Node 分類（Container/Processor/Endpoint）、scope
  （Overview/Local/Expanded）・Collapse・Semantic Zoom・Role
  Layer の表示規則を、DSP/Project を変更しない view 状態として
  定義する。
- **SFD-R2 Typed Port/Connection**: `SemanticPort` の属性集合と
  `RoutingConnection` の kind（Main/Send/Sidechain）・Channel
  Map・Enabled の意味を定義し、方向・信号 Domain・Role・
  Multiplicity・Cycle・Port 不在の拒否/説明可能な未対応状態を
  `RoutingEditError` 系の分類へ対応付ける。
- **SFD-R3 Semantic Routing 編集**: 調査（Inspect）と編集
  （Routing Command）の入口分離、gesture の fixation→preview→
  confirm→1 CAS/1 Undo の境界、Cancel/失敗時の実行 Graph 保全、
  Mixer との単一正本整合を定義する。
- **SFD-R4 Probe 表示/編集**: 標準 Probe 能力の宣言モデル、
  Off/Monitor/Capture の意味、`Requested`〜`BoundActive`/
  `Unbound`/`Partial`/`Lost`/`Unavailable` の状態語彙、
  Probe 操作が Processor Bypass・Routing を変更しない不変条件、
  Input/Output/Sidechain 等の能力・Unavailable 表示の一貫性を
  定義する。
- **SFD-R5 SourceMap/Configured/Observed**: `ProjectRevision`
  （Configured）・plan generation（Prepared/Published）・RT
  `active_generation`（Active）・Observed 証拠の分離、
  `RoutingSourceMap` の単一権威、Unknown/Stale/Unavailable を
  推定値に置換しない規則を定義する。
- **SFD-R6 受入仕様の追跡可能性**: Issue #4 の受入項目を
  シナリオ ID で識別し、既存の SF-MODEL/VALID/ACT/PROBE/UX/
  COMPAT Case family（#109 が Test Owner）と headless/実機の
  lane 別状態へ対応付ける。
- **SFD-R7 Owner 境界**: Synvelis（Project 正本・意味論・View）と
  Ruviform（Generic Graph/RT/Probe）の責務分界、#104〜109 との
  実装依存関係を明記し、本タスクが設計文書のみであることを
  固定する。
- **SFD-R8 未決事項の分離**: 新しい製品トレードオフ・契約破壊・
  定量 SLO・OS サポート宣言は「提案/決定待ち」として採用決定と
  分離し、実測や人間承認を捏造しない。

## Acceptance Criteria

- [ ] 文書チェーン 7 ファイル（spec/design/plan/task/test-spec/
  impl ir/impl exp）が存在し、front matter の `type`/`status`/
  `relations` が規約に適合し、相互参照が解決する。
- [ ] `docs/designs/signal-flow-design-design.md` が SFD-R1〜R8 の
  全領域をカバーし、Issue #4 の受入項目 6 件それぞれに 1 つ以上の
  シナリオ ID が割り当てられている。
- [ ] `evidence/sv4/` に設計根拠（Decision Log・代替比較）と
  既存 SF 系 spec/design との整合確認記録がある。
- [ ] `docs/*/README.md` 管理 index が再生成され、
  `doc_maintenance.js index plan` が対象 dir で `in-sync`。
- [ ] `mise run fmt-check` が exit 0。
- [ ] review-protocol レビューで blocker が残っていない。
- [ ] 実装・設定ファイルに差分がない（docs と evidence のみ）。

## Deferred Design Concerns

- Visual Group の保存・共有粒度・複数 View での扱い | Issue #4
  本文で PoC 決定待ちと明記 | Visual Group 高度 UI slice 着手時 |
  ViewState/Workspace schema の拡張判断を誤ると Project 正本と
  表示状態が混線する。
- 複数 Channel Mapping の UI 提供方式・既定 | Issue #4 で未決 |
  channel map 編集 UI slice | 暗黙 up/downmix を招く UI 既定を
  固定してしまう危険。
- 特殊 Feedback（遅延付き loop 等）の許可条件 | 初期一律 Reject
  が合意済み | Runtime Capability 検証と別明示合意 | 安全側拒否の
  まま機能不足に留まる可能性。
- Graph Canvas 描画・ドラッグ閾値・Semantic Zoom の数値 |
  Issue #4 で未決 | 実 GUI slice（#108/#114） | headless 契約と
  実 GUI 受入の境界が曖昧になる。
- SF-ACT-05〜09・SF-PROBE/SF-UX の実機部分 | headless では
  検証不能 | 実機・実 OS・実 Plugin gate（#109/#114） |
  headless Pass を実機受入と誤認する危険。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/4>
- 実装 Owner: #104（SF-01 Semantic Routing Model）・#105（SF-02
  Routing Transaction）・#106（SF-03 Lowering/SourceMap/
  Activation）・#107（SF-04 Semantic Probe）・#108（SF-05 Signal
  Flow GUI）・#109（SF-06 Cross Gate）。
- Ruviform Generic: <https://github.com/scarecrow173/ruviform/issues/184>
  （Typed Graph/Port）・<https://github.com/scarecrow173/ruviform/issues/185>
  （Probe/Capture）・<https://github.com/scarecrow173/ruviform/issues/308>
  （Probe rebind）・<https://github.com/scarecrow173/ruviform/issues/309>
  （Probe time alignment）。
- 既存正本（depends-on 参照）: SF-01〜06 の spec 群と
  `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`・
  `docs/designs/sf-05-slice{1,2}-*.md`・
  `docs/designs/playback-mixer-generic-execution-graph-architecture.md`。
