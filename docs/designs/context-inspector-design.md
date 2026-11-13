---
id: "DESIGN-6ClrSXGAyry8iwAqt4t32c"
type: "design"
status: "approved"
title: "Context Inspector 設計 — Follow/Explore/Pinned 状態機械・TargetKind Provider・情報階層・Probe/編集契約（synvelis#5）"
created: "2026-11-14"
updated: "2026-11-14"
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
    - "docs/specs/context-inspector.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/context-inspector-plan.md"
    - "docs/tasks/context-inspector-task.md"
    - "docs/test-specs/context-inspector.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/context-inspector.md"
    - "docs/designs/workspace-docking-state-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Context Inspector 設計

## 概要

Context Inspector を「共通フレーム + 対象別 Content Provider」の
Dockable View として定義する。本書は Issue #5 の合意済み UX を、
既存の workspace/project/player 実装が既に持つ語彙と契約へ対応付け、
状態機械・情報階層・編集安全・Probe 意味論・永続化を固定する。
**本書は設計定義であり、製品 View の実装は #112/#108、RT Probe は
#107 の Owner が行う。**

## スコープと Owner 境界

| 責務 | Owner | 既存の対応物 |
| --- | --- | --- |
| View Instance ごとの状態保持・Snapshot・Window/Dock | Ruviform #181/#182 + Synvelis `synvelis-workspace` | `ViewStateRegistry`/`ViewState`/`WorkspaceSnapshot`(schema v3)・tolerant decode |
| Workspace 状態・Focus・Command・Link Group | Synvelis（#2/#3、WS-A/ED-A） | `state.rs`/`focus.rs`/`link.rs`/`selection.rs`/`workspace.rs` |
| Inspector Follow/Explore/Pinned・Target 解決・Provider | Synvelis（本 Issue #5 → 実装は #112/#108 系 slice） | `InspectionFocus`/`InspectionMode`/`FollowSource` の先例（`inspection_focus`/`signal_flow.inspection`/`control_relation`） |
| Signal Flow 投影・Port/Connection・routing gesture | Synvelis #108（SF-05） | `signal_flow_ui.rs`/`SignalFlowEntityRef`/`RoutingGesture` |
| Control Relation・edge gesture・recording status | Synvelis #102（UI-C5、統合済み） | `relation_ui.rs`/`ControlRelationViewState`/`control_edge.rs`/`syn.control.*` ops |
| Semantic Probe Point・ProbeMode・Binding/Lease・RT 証拠 | Synvelis #107（SF-04）+ Ruviform #185/#308/#309 | `probe.rs`/`probe_trace.rs`/`SemanticProbePoint`/`ProbeSubscriptionKey`/`ProbePrincipal` |
| 製品 View/GPUI 描画・実 Window/IME/DPI | Synvelis #112/#114 + Ruviform #257/#386/#387 | driver-neutral ViewSpec/Component 契約（`syn.*` ops） |
| Project 編集 Command・Undo・Revision | Synvelis `synvelis-project`（VS-A/F0） | `ProjectController`/`ProjectOp`/`ProjectRevision` |

DAW 固有の TargetKind・Track・Inspector の Follow/Pin/診断文は
Ruviform に入れない。Ruviform 側に不足があれば upstream Issue 化
し、Synvelis 側で恒久的な重複実装をしない。

## 既存実装との対応付け

Issue #5 の各概念は、先行 slice で確立済みの型・機構へ写像する。
**新しい並行機構を作らず、既存契約の上に Inspector 固有の意味論を
載せる**のが本設計の基本方針。

| Issue #5 の概念 | 既存の正本 | 対応 |
| --- | --- | --- |
| Follow/Explore/Pinned mode | `InspectionMode`（`state.rs`） | 再利用。3 mode・既定 Follow は確立済み |
| FollowSource（View/Group 絞り込み） | `FollowSource`（`state.rs`） | 再利用。`None` = 自 Group の全イベント |
| Explore 履歴（session-only） | `InspectionFocus.explore_back/fwd`（`#[serde(skip)]`） | 同規則を Inspector target 型へ適用 |
| Link Group 経由の対象配信 | `LinkGroupManager::deliver`/`map_to_track`/`signal_flow_entity_from_semantic`（`link.rs`/`signal_flow_ui.rs`） | event target は semantic mapping 後の `SemanticTarget::Track` のみ。Follow-mode + follow_source 一致でのみ retarget、`(receiver,event_id)` dedup、再 emit なし |
| Linked Context（highlight のみ・編集対象でない） | `LinkedContext`（`state.rs`） | 再利用。link 受信は editing selection を書き換えない |
| Editing Selection（view-local・domain 別） | `ObjectSelections`/`EditingSelection`/`SelectionDomain`（`selection.rs`） | Inspector は読まない/書かない。複数選択の入力としてのみ参照 |
| Window/Active View/Input Focus の分離 | `FocusState`/`InputContext`（`focus.rs`） | Focus 変化は FollowSource を切り替えない根拠 |
| View ごとの追加 Inspector 状態 | `ViewState.signal_flow`/`control_relation`（`state.rs`） | `ViewState.inspector`（仮）を同型の `Option` + tolerant decode で追加する設計 |
| Command 対象の発行時固定 | `control_edge_begin`/`RoutingGesture` fixation・ws-a §6 `CommandContext` | Begin 時に `TargetId+SubTarget+expected_revision` を固定し、late receipt で retarget しない先例 |
| 全件 preflight・all-or-nothing | `ProjectController` 単一検証・gesture の stale drop | 一括編集は 1 Command として controller へ渡し、部分適用を構造的に排除 |
| Probe 共有・View 閉鎖で誤停止しない | `ProbeSubscriptionKey`/`ProbePrincipal::{Session,View}`/lease（`probe_trace.rs`） | Inspector View は `View` principal の lease であり、明示 Monitor の session hold は残る |
| Probe 状態語彙 | `ProbeMode`/`ProbeResolveState`/`ProbeStatusKind`/`ProbeStopReason`/`ProbeUnobservedReason` | 表示は verbatim 分類、独自の「測れた/測れない」語彙を作らない |
| 観測境界（Main/Sidechain/Output） | `SignalBoundary`（`probe.rs`） | `Input`/`Output`/`Sidechain`/`PreInsert`/`PostInsert`/`PreFader`/`PostFader` をそのまま提示 |
| 比較・詳細 Capture | `syn.probe.compare`/`syn.probe.control`/`syn.probe.state` ops、`ProbeComparison*` 型 | Inspector は要約 + Diagnostic View への導線のみ |

## TargetKind と entity vocabulary

Inspector の対象空間は **既存 entity 参照の tagged union**
（設計名 `ContextInspectionTarget`）とし、新しい domain id を
発明しない。各 variant は既存の安定 ID をそのまま包む。

| TargetKind | variant（既存型） | Link event で到達可能か |
| --- | --- | --- |
| Track | `SemanticTarget::Track` / `SignalFlowEntityRef::Track` | 可（link event の target は semantic mapping 後常に Track） |
| Bus / Strip | `SignalFlowEntityRef::{Bus,Strip}` | 不可 → Explore/明示 inspect 導線 |
| Clip | `SemanticTarget::Clip` | 不可（Clip 選択は owning Track へ写像されて配送。Clip 自体は link target にならない）→ Explore/inspect 導線 |
| Lane | `SemanticTarget::Lane` | 不可（Track へ写像）→ 同上 |
| Source | `SemanticTarget::Source` | 不可（`map_to_track` が `None` → event 不発）→ 同上 |
| Plugin / Processor | `SignalFlowEntityRef::Processor` | 不可（link event は `SemanticTarget` のみ運ぶ）→ Explore/inspect 導線 |
| Parameter | `ControlEntityRef::Parameter(ParameterTargetId)` | 不可 → `syn.control.relation` 系の inspect 導線 |
| Connection | `SignalFlowEntityRef::Connection(RoutingConnectionId)` | 不可 → Signal Flow からの明示 inspect 導線 |
| Send / Port | `SignalFlowEntityRef::{Send,Port}` | 不可 → 同上 |
| Probe | `SemanticProbePoint{entity,boundary,port}` | 不可 → Observations section / 専用 Probe 対象として明示選択 |
| MIDI Note/Event | （#10/#12 model 確定待ち） | 将来。Provider 位置のみ予約 |

link event が運ぶのは `map_to_track` による semantic mapping 後の
`SemanticTarget::Track` のみである（Clip/Lane 選択は owning
Track へ写像、Source 選択は event 自体が不発、Bus/Strip/
Processor/Parameter/Connection/Send/Port/Probe は
`EditingSelection`/`SemanticTarget` の対象外）。したがって
Inspector の **Follow retarget が link event で直接届くのは
Track 対象に限られ**、他 kind は Explore（Relationships 等からの
明示 inspect）・Pinned・Context Menu の明示導線で到達する。
これは WS-A が確立した「link の topic は track selection」の
粒度をそのまま継承するものであり、link event の粒度拡張は
本設計の対象外（必要になれば link.rs の別 slice）。

**設計上の決定**: union は「一つの対象 ID 体系への正規化」ではなく
「既存参照の同格な同居」とする。TargetKind ごとに Provider が必要と
する entity 型が異なるため、共通ヘッダーが表示するのは
`kind + label + 所在 + revision/世代 evidence` であり、内部 id の
単一化は行わない。`SemanticTarget` → union への変換は明示関数
（`Track/Lane/Clip/Source` のみ成功、他は `None`）で fail-closed と
し、写像不能な対象を推測で別対象へ置き換えない。

## 状態機械: Follow / Explore / Pinned / Unresolved

`ContextInspectorViewState`（View Instance 単位、`ViewState` の
`Option` field として snapshot 永続化・tolerant decode）は次を持つ:

- `target: Option<ContextInspectionTarget>` — 現在の解決済み対象。
- `mode: InspectionMode` — Follow（既定）/ Explore / Pinned。
- `follow_source: Option<FollowSource>` — Follow の追従元絞り込み。
- `explore_back` / `explore_fwd: Vec<ContextInspectionTarget>` —
  session-only（`#[serde(skip)]`、永続化しない）。
- `unresolved: Option<InspectorUnresolved>` — 対象喪失の typed
  理由（`TargetDeleted`/`ViewMissing`/`ProjectMismatch` 等）。
  `unresolved` 中は `target` を保持したまま「未解決」表示とし、
  別対象への自動切替をしない。

### 遷移規則

| 現在 | イベント | 遷移 |
| --- | --- | --- |
| Follow | FollowSource 一致の link event（有効対象） | target 更新（Follow 継続） |
| Explore | Relationships/Probe/Diagnostic からの明示 inspect | target 更新・prev を back stack へ push・fwd clear |
| Explore | 戻る/進む | stack walk（target のみ移動、外部状態不変） |
| Explore | **FollowSource 一致の新しい有効な選択イベント** | **Follow へ復帰**（target=event 対象、explore 履歴クリア） |
| Explore | FollowSource 不一致の event・Window Focus 変化・別 Group event・描画更新 | 変化なし（探索維持） |
| 任意 | Pinned 明示操作 | mode=Pinned、target 固定。以後 link event では retarget しない |
| Pinned | 明示的 Follow 復帰操作 | mode=Follow（follow_source は保持） |
| 任意 | 対象削除/Unavailable 化 | `unresolved` 設定。target は保持、表示は未解決。戻る/Follow への明示導線 |

Issue #5 §3.3 の「Explore 中に FollowSource から新しい有効な選択
イベントが届いたら Follow へ戻す」は、既存 `deliver()` の
「Explore/Pinned は retarget しない」契約を**変更しない**。
`deliver()` は `linked_context` 記録と Follow-mode focus 更新のみを
担い、Inspector の Explore→Follow 復帰は Inspector 側のイベント
消費規則として定義する（link.rs の共有 semantics を壊さない）。
実装時は link event を Inspector が subscribe/derive する経路か、
workspace に Inspector 専用の delivery hook を additive に追加する
かを #112/#108 slice が選択する — 本設計は契約のみを固定する。

### 「戻る」の意味

Inspector の back/forward は**探索履歴**であり、Project Undo・
ブラウザ履歴・Arrangement の選択/表示位置とは無関係。戻る操作は
`explore_back` pop + `explore_fwd` push のみを行い、編集選択・
viewport・再生状態・link context を一切書き換えない
（既存 `explore_back`/`explore_forward` と同規則）。

## 情報階層とレイアウト

### 共通 Context Header

全 TargetKind で同一構造: 対象 kind + label、所在（所属 Track/
Strip/Chain 位置）、Follow 元と mode、Pinned 状態、back/forward、
対象別の明示的編集導線、未解決時の理由と復帰導線。

### Section（固定タブではない）

`Overview → Relationships → Properties → Observations →
Diagnostics` の優先順で、内容が存在しない Section は描画しない。
各 Section は Provider が供給する display DTO であり、
`Editable`/`ReadOnly`/`Unsupported`/`Unavailable` の能力状態を
値と区別して表示する。

| Section | 内容（TargetKind 別に Provider が決定） |
| --- | --- |
| Overview | 主要属性・状態・頻用操作（Track: gain/pan/mute・bus kind、Clip: 位置/長さ、Processor: bypass/順序、Parameter: 現在値/単位/範囲、Connection: source→target、Probe: mode/boundary） |
| Relationships | 所属・入出力・Send/Sidechain・Automation/Modulation 関連・対象へ移動できるリンク（`relation_ui`/`signal_flow`/`routing` の既存 read model を再利用） |
| Properties | 詳細属性と編集 UI。編集不可は `ReadOnly`/`Unsupported`/`Unavailable` で明示 |
| Observations | 標準 Probe の観測値・取得時刻/状態・詳細 Capture/Diagnostic View への導線 |
| Diagnostics | 無効接続・欠落・未解決・異常候補・観測根拠と限界（`coverage`/`ProbeResolveState`/staleness evidence を verbatim に） |

### レイアウト規則

- **Compact**（狭い Dock/Siderail）: Overview を入口に、Section を
  縦積み折りたたみで表示。重要な状態/異常は折りたたみ状態でも
  認識できる要約を header/Overview に出す。
- **Expanded**（広い Window/別モニター）: 同一 Provider/データを
  複数カラムへ配置。別 Inspector 実装ではない。
- Section 開閉・内部スクロール・表示設定は **View Instance ごとに
  独立**して保持する（`ViewState` 単位）。対象種別ごとの開閉記憶は
  PoC 後判断（Issue #5 §10）。
- レイアウト変更で対象・Follow 元・編集対象・Probe ON/OFF は
  変わらない。長時間の波形/スペクトラム比較・多数観測点は専用
  Diagnostic View へ展開し、Inspector には要約と導線のみ置く。

## 複数選択と一括編集

### Selection 形状と表示

| Selection | 表示 |
| --- | --- |
| 単一対象 | 対象の詳細情報 |
| 同じ種類を複数 | 共通プロパティ + 対象件数。同値/Mixed/編集不可を区別 |
| 異種を複数 | 共通情報 + 対象一覧。適用可能な共通編集のみ提示 |
| 選択なし | FollowSource と空状態を明示 |
| Pinned | Follow 元の複数選択によらず固定対象を表示 |

複数選択の入力は follow 元 View の `ObjectSelections`（domain 別
`EditingSelection`）を read model 経由で読み、Inspector 自身は
selection を複写・変更しない。Mixed 値を単一値に見せない:
全同値/一部異なる/一部編集不可を区別し、混在値からの新規入力は
操作対象を明示する。

### 一括編集 Command

- 初期方針は**明示的な絶対値設定**のみ。相対変更（ドラッグ相対・
  nudge）は既定で提供せず、提供する場合は明示操作として後続仕様化。
- Command 発行時に**全 Target ID と必要な Parameter/Connection
  ID をスナップショット**し、`ProjectSessionId + expected revision`
  を固定して単一 Project Controller へ渡す。
- **preflight で全対象を検証し、原則 all-or-nothing**。一部対象が
  編集不可・不適合・消滅した場合は、適用可能なものだけを黙って
  変更せず、理由と対象を表示する。部分適用を将来提供する場合は
  別の明示操作/確認と結果報告を必須とする。
- 実際の編集のみが Project Undo/Redo に登録される。Inspect/
  Explore・Section 開閉・Probe 表示は Undo に含めない
  （`ws_a_05` の「layout ops は project/engine に触れない」と同契約）。

### 非同期安全性（CI-07）

編集 Begin で `TargetId + SubTarget + expected_revision` を固定
し（`control_edge`/`routing_gesture` fixation と同型）、非同期
更新・Follow 対象変更・Window 移動・late receipt があっても、
commit は fixated revision で検証し、stale なら drop/拒否する。
**表示中の対象を commit 時に読み直して対象をすり替えない**。
Pinned からの明示編集も同じ Command 経路を通り、対象は表示中の
Pinned 対象 ID で固定される。

## Probe 連携と Diagnostics

### Probe の所有と共有

- **Probe Off が既定**。Inspector の表示・選択・Section 展開だけで
  Monitor を自動開始しない。
- 明示的に ON にした Probe は Session 所有: `ProbePrincipal::Session`
  の hold + 各 View の `ProbePrincipal::View` lease で構成され、
  Inspector の対象変更・Window 移動・View 閉鎖で誤停止しない
  （`probe_trace.rs` の lease/`by_view`/`probe_release_subscriber`
  機構をそのまま利用）。Window 閉鎖は自 View の lease のみを解放し、
  session hold が残る限り Monitor/Capture は継続する。
- 同一 `SemanticProbePoint` を 2 つの Inspector が表示しても
  `ProbeSubscriptionKey` は 1 つで lease が集約され、Probe 実行を
  重複生成しない（`sf05_probe_share.rs` の
  `two_views_share_one_probe_subscription` 等が先例証拠）。
- Off / Monitor / Capture は `ProbeMode` の語彙をそのまま使い、
  Processor の `Bypass` とは別操作として表示する。Capture は
  期間・予算・進行状態を持ち、完了物は保存区間として
  Diagnostic View/比較へ導線を出す。

### 観測値の分類（証拠の正直さ）

Inspector は次を**別証拠**として表示し、混同しない:

- `Configured`（設定が存在）/ `Prepared` / `Published` /
  `RT Applied`（`BoundActive`）/ `Observed`（RT 観測証拠）/
  `Unavailable`（取得不可）/ `Stale`（旧 generation）/
  `Data Loss`（`LostBlocks`/`LostRecords`/`Unobserved`）。
- `ProbeResolveState`（Requested→…→BoundActive/StaleGeneration/
  Lost/Stopped）を lifecycle として verbatim に投影し、
  `requested`/`queued` を RT 確定観測と誤表示しない。
- **Measured zero PCM（実信号の無音）と Missing/Unknown/
  Unobserved/Unsupported を分離**する。無音を「データなし」と
  表示せず、データなしを無音と表示しない。
- 原因候補は `Inferred`（根拠付き推定）として最初の `Observed`
  anomaly と区別し、「根本原因」を無条件に断定しない
  （測定方法・基準・遅延補償・取得漏れを確認した表現）。
- Probe 比較には `ProbeId`/`Node/PortId`/`GraphRevision`/
  sample-block position/sample rate/channel layout/latency/
  data-loss を必要に応じて示す（`syn.probe.compare` の
  `ProbeComparison*` 契約に従う）。

### 観測境界

同一 Processor（`SemanticSignalEntityRef::Processor`）の観測可能な
境界は `SignalBoundary` の語彙で列挙する: `Input`/`Output`/
`Sidechain`（および strip では `PreInsert`/`PostInsert`/
`PreFader`/`PostFader`）。各境界の `Configured/Observed/
Unavailable/Unsupported` を個別に表示し、「Main/Sidechain/
Output のどこが観測できるか」を誤解なく示す。Plugin 内部の非公開
地点・非 Audio domain は `Unavailable`/`Unsupported` の理由を
表示し、取得できると偽装しない。

## 永続化と復元

- `ContextInspectorViewState` は `ViewState` の `Option` field と
  して `WorkspaceSnapshot` に乗る（`signal_flow`/`control_relation`
  と同じ tolerant decode: malformed payload は `None` へ安全劣化、
  snapshot 全体を reject しない）。schema version は既存の
  migration 規則に従う additive 変更とする。
- **復元するもの**: FollowSource・Pinned target・Section
  expand/collapse・scroll/presentation（可能な範囲）。
- **復元しないもの**: Explore 履歴（`#[serde(skip)]`、再起動で
  初期化）、実行中 gesture、session-only の lease。
- **再水和**: Dock 間・Window 間移動・WebView/driver 再生成で
  論理 `ViewInstanceId` と状態を再水和し、View/Probe/Project Job
  を再生成しない（Issue #5 §6、#108 の Window reparent 規則）。
- **未解決の扱い**:
  - FollowSource の View ID が復元不能 → 曖昧な別 View へ追従せず、
    未解決表示 + 再バインド操作。
  - Pinned target が削除済み/他 Project → 存在しない対象を別物に
    置換せず、未解決表示（`unresolved` state）。
  - in-memory `capture`/`restore` は serde を通らないため
    session-only field の扱いは JSON 境界で規定する
    （`signal_flow` と同じ契約）。

## 編集の安全性と View/Command 分離

- **View Context ≠ Editing Command Target**。Inspector の
  Follow/Explore/Pinned・link 由来 highlight は編集対象を
  自動決定しない。`LinkedContext` を破壊的 Command の対象に
  転用しない。
- 右クリック/Context Menu 経由の編集は、クリックされた対象 ID を
  Command へ明示して渡す（ws-a §6 の `explicit_target_ids`）。
- 入力境界: Text/IME 編集中は `InputContext` が入力を優先し、
  `Ctrl/Cmd+Z` 等が DAW 編集コマンドとして誤発火しない
  （`focus.rs`/`ws_a_03` の契約）。Plugin GUI Focus/Modal の
  境界は #2/#3 に従う。
- Project 変更の反映待ち・エラーと、Inspector View の一時
  ローディング・データ Unavailable を区別する
  （`revision_inbox`/coverage/staleness evidence で表示）。

## Driver-neutral 契約

- Inspector の View/Component 定義は Ruviform の driver-neutral
  ViewSpec/Component 契約（#257/#386/#387）に従い、GPUI Entity・
  DOM/SolidJS・OS 固有型を `ContextInspectionTarget`・Provider
  DTO・状態機械に持ち込まない。
- wire/op 語彙は既存 `syn.*` op 群（`syn.signal_flow.inspection`/
  `syn.control.relation`/`syn.probe.*`/`syn.view.*`/`syn.viz.*`）の
  規約に揃える: tagged id、kebab-case enum、`Option`→`null`、
  gesture/stale outcome は error ではなく typed 値。
- 実 Window/Focus/IME/DPI/Plugin editor の受入は #114/#176 と
  Ruviform #313/#314 の Gate であり、本設計の headless 契約と
  分離して記録する。

## Decision Log

| # | 決定 | 採否 | 根拠 |
| --- | --- | --- | --- |
| D1 | 共通フレーム + TargetKind 別 Provider、固定タブではなく適応 Section | 採用 | Issue #5 §1/§2 の合意。情報が無い Section を空表示しない |
| D2 | `InspectionMode`/`FollowSource`/explore stack 規則の再利用と、View Instance 単位 `Option` state + tolerant decode | 採用 | `inspection_focus`/`signal_flow`/`control_relation` の確立済み契約と同型。新規並行機構を作らない |
| D3 | `ContextInspectionTarget` を既存 entity 参照の tagged union とし、新 domain id を発明しない | 採用 | Track/Clip/Processor/Connection/Parameter/Probe の正本 ID は既に存在。union は参照の同格な同居であり単一 ID 体系への正規化ではない |
| D4 | Link Follow の retarget 対象は link event が運ぶ semantic mapping 後の `SemanticTarget::Track` に限定。Clip/Lane は owning Track へ写像、Source は event 不発、Bus/Strip/Processor/Parameter/Connection/Send/Port/Probe は Explore/明示 inspect 導線 | 採用 | `map_to_track`/`deliver`/`signal_flow_entity_from_semantic` の確立済み契約。link event は Track のみを運び、写像不能対象を推測変換しない fail-closed が既存契約 |
| D5 | Explore 中の FollowSource 一致・新規選択イベントで Follow 復帰（履歴クリア）。他 Focus/Group/描画イベントでは解除しない | 採用 | Issue #5 §3.3/CI-05 の明示要求。`deliver()` の共有 semantics は変更せず Inspector 側消費規則とする |
| D6 | 一括編集は絶対値設定のみ・発行時全対象 snapshot・単一 controller preflight・all-or-nothing | 採用 | Issue #5 §4 の合意。無言の部分適用を構造的に排除。相対変更・部分適用は後続明示仕様 |
| D7 | Probe は Session 所有 + View lease。Off 既定・自動 Monitor なし・View 閉鎖で session hold を止めない | 採用 | Issue #5 §5 + `probe_trace.rs` の確立済み lease 機構。`sf05_probe_share` が重複生成なしを既証明 |
| D8 | 観測分類は `ProbeResolveState`/`ProbeStatusKind`/`Configured`/`Observed`/`Inferred`/`Unavailable`/`Stale`/`Data Loss` を verbatim 投影し、無音≠欠損を分離 | 採用 | Issue #5 §5 + SF-04 の「requested/queued を RT 確定と誤表示しない」契約 |
| D9 | Section/scroll/presentation は View Instance 単位で永続化、Explore 履歴は session-only | 採用 | Issue #5 §6 + `signal_flow`/`control_relation` の serde 規則 |
| D10 | Pinned/FollowSource 喪失は `unresolved` typed state + 再バインド導線、勝手な代替対象表示をしない | 採用 | Issue #5 §3.5/§6 + `uic5_03_pinned_target_stays_unresolved_after_delete` の先例 |
| D11 | 編集 Command は Begin 時に Target+Revision を固定し late receipt で retarget しない | 採用 | `control_edge`/`routing_gesture` fixation・ws-a §6 `CommandContext` の確立済み契約 |
| D12 | 固定 5 タブレイアウト | **不採用** | Issue #5 §2 で明示的に否定。適応 Section が合意 |
| D13 | Inspector 表示/選択/展開での自動 Monitor 開始 | **不採用** | Issue #5 §5。Probe Off 既定 |
| D14 | 一括編集の部分適用 | **保留（後続）** | Issue #5 §4。別の明示操作/確認と結果報告が前提 |
| D15 | Probe 実行状態・Capture 結果の再起動永続化・自動再 Capture | **未決→非保証** | Issue #5 §5/§10。View 表示設定と Probe 実行状態を一体の永続 field にしない |
| D16 | 相対値による一括変更の既定提供 | **保留（後続）** | Issue #5 §4/§10 |
| D17 | Explore 履歴の永続化 | **不採用** | Issue #5 §6。再起動・Explore 離脱で初期化が合意 |
| D18 | DAW TargetKind/Follow/Pin/診断文を Ruviform 共通層へ配置 | **不採用** | Issue #5 §8。再利用可能な契約のみ Ruviform、DAW 意味論は Synvelis |
| D19 | MIDI Note/Event Provider の本 slice 実装 | **保留** | #10/#12 の Model 確定待ち。AC 対象外、情報階層位置のみ予約 |

## 代表 Scenario と Expected Oracle

### Normal

| # | Scenario | Expected Oracle |
| --- | --- | --- |
| N1 | Arrangement で Bass 選択 → Inspector Follow → Relationships から Music Bus → Compressor → Sidechain へ Explore → 戻る | 各段で共通ヘッダー+対象別 Provider が一貫表示。戻るで Bass へ復帰。Arrangement の editing selection・viewport・link context は不変（CI-01/CI-03） |
| N2 | Inspector A=Pinned(Bass)、Inspector B=Follow(Drums)。両方を別 Window へ移動 | 移動後も各 View の target/mode/follow_source/section state が保持。A は Bass 固定、B は Drums の link event に追従（CI-04） |
| N3 | 3 Track を同種複数選択（gain 同値 2・異値 1）→ Mixed 表示 → 絶対値で一括設定 | 共通値/Mixed/件数が正確。commit は発行時 snapshot の全対象に preflight→1 Command→全件適用 or 全件拒否。Undo は実編集のみ（CI-06/CI-13） |
| N4 | 同一 Compressor の Main/Sidechain/Output Probe を Inspector A/B で表示 → A を閉じる | `ProbeSubscriptionKey` は 1 つ・lease 集約。A 閉鎖で A の lease のみ解放、session hold の Monitor/Capture は継続（CI-08/CI-10） |
| N5 | Probe を Monitor → 実信号無音区間 → Data Loss 発生 → Stale generation | Off/Monitor/Capture/無音/欠損/Stale が互いに区別されて表示。`requested`/`queued` は RT 観測と別表示（CI-09） |
| N6 | Compact→Expanded レイアウト変更 | 対象・編集内容・Probe mode・section 開閉が不変。同一データのカラム配置のみ変化（CI-02） |

### Failure / 境界

| # | Scenario | Expected Oracle |
| --- | --- | --- |
| F1 | Pinned 対象が Project から削除 | `unresolved` 表示・理由 typed。別対象へ自動切替しない。戻る/Follow への明示導線あり（CI-11） |
| F2 | 再起動。FollowSource View が復元不能、Pinned は有効、Explore 履歴あり | FollowSource/Pinned/Section は復元、Explore 履歴は空。不在 View には未解決+再バインド UI（CI-12） |
| F3 | 一括編集で 1 対象が編集不可/消滅 | preflight が全件検証し全体拒否。理由・対象を表示。部分適用なし・Undo エントリなし（CI-06） |
| F4 | 編集 Begin → 非同期更新中に Follow 対象が別 Track へ変化 → commit | fixated `TargetId+revision` で検証。stale なら drop/拒否し、別 Track/Parameter へ誤適用しない（CI-07） |
| F5 | Window Focus 変化・別 Group の link event・描画更新が Explore 中に到着 | 探索は解除されない。FollowSource 一致の新規選択 event のみ Follow 復帰（CI-05） |
| F6 | Inspector の malformed persisted state | `None`/既定へ安全劣化し snapshot 全体を reject しない（tolerant decode 契約） |
| F7 | Probe/Inspector の表示・探索・Section 開閉のみ実行 | Project revision/undo depth・DSP bypass・routing・RT に変化なし（CI-13） |
| F8 | Follow 元で Clip/Lane/Source/Bus/Processor 等を選択 | link event が運ぶのは `SemanticTarget::Track` のみ（Clip/Lane→owning Track、Source→event 不発、Bus/Processor は `EditingSelection` の対象外）。Inspector Follow は Track を表示し、選択対象自体へは retarget されない。Relationships/明示 inspect の Explore 経路で到達（D4） |

## 検証戦略と Gate 対応

- **Headless/OS-L1**: 状態機械・Provider DTO・一括編集 preflight・
  Probe lease・snapshot 復元は `synvelis-workspace`/`synvelis-app`
  の fixture で検証可能（`docs/test-specs/context-inspector.md` の
  UX Fixture 定義に従う）。既存被覆との対応は
  `evidence/sv5/coverage.md`。
- **GUI-L2（#114/#176, Ruviform #313/#314）**: 実 Window・Focus・
  IME・DPI・複数 Window・Plugin 入力の受入は別 Gate。headless/GPUI
  -simulated の成功を実機 Pass として記録しない。
- **EXT-L3**: 実 Plugin 規格のパラメータ能力・観測可能性差分・
  実機 Probe は #107/#109 系の実機 Gate。`Unsupported`/`Unavailable`
  の正直な表示が前提。
- **設計レビュー**: 本書の Decision Log・状態遷移・Owner 境界・
  Scenario/Oracle を review-protocol で仕様整合・反例レビューし、
  Confirmed P0/P1 を解消してから承認提案とする（Issue #5 の
  2026-11-10 判断）。

## 未決定・PoC 後に決める事項（Issue #5 §10 の引き継ぎ）

- 各 Provider の全プロパティ一覧・表示順位・Field の具体
  コントロール・幅ブレークポイント。
- 相対変更・多対象部分適用の明示 UI/Undo 粒度。
- Explore 履歴の最大深度・Source/Target ID 復元失敗時の具体 UI・
  Section 開閉の種別ごとの記憶方式。
- Probe ON/OFF のセッションをまたぐ永続化・Capture 結果の保存形式・
  UI 刷新頻度。
- 各 Plugin 規格のパラメータ能力・制御値・観測可能性の差分。
