---
id: "TSPEC-5P0o8UbBZq07LRZPbXsEoV"
type: "test-spec"
status: "approved"
title: "Context Inspector テスト仕様 — Follow/Explore/Pinned・Selection/Linked Context・Probe・一括編集の UX Fixture 定義（synvelis#5）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/5"
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
    - "docs/specs/context-inspector.md"
    - "docs/designs/context-inspector-design.md"
    - "docs/tasks/context-inspector-task.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/context-inspector.md"
    - "docs/designs/context-inspector-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Context Inspector テスト仕様 — UX Fixture 定義

## Purpose

synvelis#5 の受入条件 CI-01〜13 が要求する**検証契約を Fixture
として定義**する。本書は設計のみの slice であり、ここで定義する
Rule/Example は将来の実装 slice（#112 製品 View・#108 Signal
Flow 投影・#107 RT Probe 配線）が満たすべき契約である。既に
headless 実装・Fixture が存在する部分は `evidence/sv5/coverage.md`
で既存証拠へ写像し、設計のみの項目は `NotRun` と正直に記録する。
実 Window/GUI/plugin/高 DPI/IME/a11y は本 spec の対象外であり、
headless の成功をもって GUI Pass を主張しない。

## Feature

Context Inspector（共通フレーム + TargetKind 別 Provider）が、
Follow/Explore/Pinned の追従・探索・固定、複数選択の共通値/Mixed
表示と all-or-nothing 一括編集、Session 所有の Probe 共有・
観測分類、View Instance 単位の永続化を、Driver-neutral な
契約として提供する。

## Rules

### R1. 情報階層の一貫性（CI-01）

- 全 TargetKind（Track/Clip/Plugin=Processor/Parameter/Connection/
  Probe）は共通 Context Header（kind+label・所在・Follow 元・
  mode・Pinned・back/forward・編集導線）を持つ。
- Section は `Overview → Relationships → Properties →
  Observations → Diagnostics` の優先順で、内容が無い Section は
  描画しない。対象が変わっても階層構造は変わらない。
- Field は `Editable`/`ReadOnly`/`Unsupported`/`Unavailable` の
  能力状態を値と区別して表示する。

### R2. レイアウト不変性（CI-02）

- Compact=縦積み折りたたみ、Expanded=複数カラムは同一
  Provider/データの配置差のみ。レイアウト変更で対象・Follow 元・
  編集対象・Probe mode は不変。
- Section 開閉・scroll/presentation は `ViewInstanceId` 単位で
  独立保持され、他 View・他 Window の状態と混ざらない。

### R3. Follow/Explore/Pinned 状態機械（CI-03/04/05/11/12）

- Follow は `follow_source`（`None`=自 Group 全 event /
  `View(id)` / `Group(id)`）が一致する link event のみで
  target を更新する。`(receiver,event_id)` dedup・再 emit なし。
  link event が運ぶ target は semantic mapping 後の
  `SemanticTarget::Track` のみであり（Clip/Lane→owning Track、
  Source→event 不発）、Follow retarget が直接届くのは Track
  対象に限る。他 kind は Explore/明示 inspect 経路で到達する。
- Explore は Inspector 内部履歴（session-only）のみを辿り、
  editing selection・`inspection_focus`・`linked_context`・
  viewport・再生状態を書き換えない。
- Explore 中に FollowSource 一致の**新しい有効な選択イベント**が
  届いたときのみ Follow へ復帰（履歴クリア）。Window Focus
  変化・別 Group event・描画更新・link 非配送では解除しない。
- Pinned は link event で retarget せず、明示操作でのみ Follow
  へ戻れる。Pinned は読み取り専用を意味しない。
- 対象削除/Unavailable 時は `unresolved` typed state を表示し、
  別対象へ自動切替しない。戻る/Follow への明示導線を持つ。
- 再起動後は FollowSource/Pinned/Section を可能な範囲で復元し、
  Explore 履歴は初期化。不在 View/対象には未解決+再バインド UI。

### R4. View Context ≠ Command Target（CI-03/07/13）

- Inspector の表示・探索・Section 開閉・link 受信は Project
  revision・undo depth・DSP bypass・Audio Routing・RT を変更しない。
- 編集 Command は Begin 時に `ProjectSessionId + TargetId +
  SubTarget/ParameterId/ConnectionId + expected_revision` を固定
  し、commit は fixated 値で検証。stale/対象消滅は drop/拒否し、
  表示中の別対象へすり替えない。
- `LinkedContext`（link 由来 highlight）は編集対象に自動転用
  しない。Context Menu 編集はクリック対象 ID を明示して渡す。
- Text/IME/Plugin GUI/Modal の `InputContext` が入力を優先し、
  DAW 編集コマンドを誤発火させない。

### R5. 複数選択と一括編集（CI-06）

- 単一/同種複数/異種複数/選択なし/Pinned の 5 形状で表示規則が
  定義され、Mixed 値を単一値に見せない（同値/一部異なる/一部
  編集不可を区別）。
- 一括編集は発行時に全 Target+Parameter/Connection ID を
  snapshot し、単一 Project Controller が全件 preflight。
  失敗時は理由・対象を表示し、無言の部分適用をしない
  （all-or-nothing）。実編集のみが Undo に登録される。

### R6. Probe 共有・所有・分類（CI-08/09/10）

- Probe Off が既定。表示/選択/展開だけでは Monitor を開始しない。
- 明示 ON の Probe は Session 所有（`Session` hold + View lease）。
  View 閉鎖は自 View lease のみ解放し、session hold の Monitor/
  Capture を維持する。同一 `SemanticProbePoint` の複数 Inspector
  は `ProbeSubscriptionKey` を共有し実行を重複生成しない。
- Off/Monitor/Capture は `ProbeMode`、lifecycle は
  `ProbeResolveState`、status は `ProbeStatusKind`/
  `ProbeStopReason`/`ProbeUnobservedReason` を verbatim 投影。
- `Measured zero PCM`（実無音）と `Missing/Unknown/Unobserved/
  Unsupported`、`Stale`（旧 generation）、`Data Loss`
  （`LostBlocks`/`LostRecords`）を区別する。
- 原因候補は `Inferred` であり `Observed` anomaly・根本原因断定と
  区別する。
- Processor の観測境界は `SignalBoundary`（`Input`/`Output`/
  `Sidechain`、strip では `PreInsert`/`PostInsert`/`PreFader`/
  `PostFader`）で列挙し、利用可否を境界ごとに表示。詳細
  Capture/比較は Diagnostic View/`syn.probe.compare` へ導線。

### R7. 永続化（CI-12）

- `ContextInspectorViewState` は `ViewState` の `Option` +
  tolerant decode で snapshot 永続化。malformed payload は
  `None` へ劣化し snapshot 全体を reject しない。
- FollowSource/Pinned/Section/scroll は復元、Explore 履歴・
  session-only lease・実行中 gesture は復元しない。
- Dock/Window 移動・driver 再生成で論理 View ID と状態を再水和し、
  Probe/Project Job を再生成しない。

## Examples

- Given Arrangement で Bass 選択・Inspector Follow、When
  Relationships から Music Bus→Compressor→Sidechain を順に
  inspect し back を 3 回、Then 各 target で対応 Provider が表示
  され Bass へ戻り、Arrangement selection/viewport は不変（R1/R3/R4）。
- Given Inspector A=Pinned(Bass)・B=Follow(Drums)、When 両 View を
  別 Window へ detach→attach、Then 各 target/mode/follow_source/
  section state が保持され B は Drums の新規 event に追従（R2/R3）。
- Given Explore 中、When 別 Group の link event・Window Focus
  変化・描画更新、Then mode/target/履歴は不変。When FollowSource
  一致の新規選択 event、Then Follow へ復帰し履歴クリア（R3）。
- Given 3 Track 選択（gain=[-6,-6,0]）、When Inspector が共通値を
  表示し -3 を一括適用、Then Mixed→適用後は全件 -3、Undo 1 回で
  全戻し。1 対象が編集不可なら全体拒否で部分適用なし（R5）。
- Given 同一 Compressor を A/B Inspector で Monitor、When A を
  close、Then `ProbeSubscriptionKey` 数不変・session hold で
  Monitor 継続。B も close して session hold が無ければ View
  lease 解放後の契約どおり（R6）。
- Given Processor の `Sidechain` boundary を選択、When 観測値を
  要求、Then `SignalBoundary::Sidechain` の `Configured/
  Observed/Unavailable` が boundary 別に表示され、Main/Output と
  混同しない（R6）。
- Given 編集 Begin（Track X, rev R）→ Follow が Track Y へ変化
  →commit、Then fixated X/rev R で検証。stale なら drop、Y へ
  誤適用しない（R4）。

## Guarantees

- Inspector の全状態は `ViewInstanceId` 単位で独立し、Window/Dock
  移動で失われない。Explore 履歴だけは session-only で再起動時に
  初期化される。
- 「調査対象の表示」と「編集 Command の対象」は別経路であり、
  表示・探索・link 受信が Project/Undo/DSP/Routing を変更しない
  ことが Fixture で検証される。
- Probe の重複実行・View 閉鎖による誤停止・無音と欠損の混同は
  起きない（既存 lease/分類契約の再利用で保証）。
- headless/GPUI-simulated の Fixture 成功は実 GUI acceptance を
  装わず、実機 Gate（#114/#176/Ruviform #313/#314）へ別証拠
  として送る。

## Non-goals

- 実 Window・dock・focus・IME・DPI・accessibility の実機検証
  （Deferred → #114 系 Gate）。
- 製品 View/Provider の実装（#112/#108）、RT Probe 実装（#107）。
- MIDI Note/Event Provider（#10/#12 model 確定待ち）。
- 相対値一括編集・部分適用・Probe 永続化の具体方式（Issue #5
  §10 の PoC 後決定）。

## Risk

- `deliver()` が Explore→Follow 復帰を持たない → R3 は Inspector
  側消費規則として定義し、実装時に link event の subscribe/derive
  経路を #112/#108 が選択する。link.rs の共有 semantics は不変。
- `SemanticTarget` 非対応 kind への link 追従期待 → R3/D4 で
  fail-closed を明文化し、Explore/inspect 導線を保証する。
- 既存 3 focus（inspection_focus/signal_flow/control_relation）
  との state 配置重複 → 本 spec は契約のみ固定し、`ViewState` への
  field 配置は実装 slice が既存 `Option`+tolerant decode 規則で
  決定する。
