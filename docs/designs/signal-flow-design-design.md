---
id: "DESIGN-034X9CQwZrVbNmLpKsTuVw"
type: "design"
status: "approved"
title: "Signal Flow 設計正本 — 階層・Typed Port/Connection・Semantic Routing・Probe 表示/編集・世代証拠"
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
  implements:
    - "docs/specs/signal-flow-design.md"
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
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/signal-flow-design.md"
    - "https://github.com/scarecrow173/synvelis/issues/4"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/signal-flow-design.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/108"
    - "https://github.com/scarecrow173/synvelis/issues/109"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/184"
    - "https://github.com/scarecrow173/ruviform/issues/185"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
---

# Signal Flow 設計正本 — 階層・Typed Port/Connection・Semantic Routing・Probe 表示/編集・世代証拠

本書は Issue #4 の**設計正本**。Signal Flow を「階層的な探索・接続の
直接編集・ノード出力/入力の観測・診断につながる DAW View」として
定義する統合設計権威であり、SF-01〜06 の実装済み/slice 文書が個別に
持つ契約を、View 層の観点で一箇所に集約する。実装は対象外 — 実装
Owner は #104〜109 であり、本書はそれらが従うべき表示・編集・受入の
意味論を固定する。

既存文書との関係: 本書は `docs/specs/sf-01..06`・
`docs/designs/sf-03-map-*`・`docs/designs/sf-05-slice{1,2}-*` を
**再掲せず参照**し、Signal Flow View の階層・Port/Connection・
編集・Probe 表示・世代証拠の対応関係だけを正本化する。契約の詳細
（型・状態機械・失敗分類）は各正本 doc を authoritative とする。

## Context

Issue #4 は「Signal Flow の階層・型付き Port/Connection・Routing
操作・標準 Probe を定義する」設計 Issue。2026-11-04〜10 の本文で
製品原則（Create → Understand → Refine）・Semantic/Visual
Hierarchy 分離・直接編集 UX・標準 Probe・Ruviform/Synvelis
Ownership が合意済み。SF-01（#104 RoutingModel/Schema v6）・
SF-02（#105 RoutingTransaction/Mute/Solo）・SF-03（#106
Lowering/SourceMap/Activation）・SF-04（#107 Semantic Probe）・
SF-05（#108 Signal Flow GUI）・SF-06（#109 Cross Gate）は
headless 実装が main に統合済みで、残るのは編集 UX の完成・
Probe 共有・実機受入。

本書が必要な理由: Signal Flow View の契約が SF-01〜06 の各 spec/
design と Issue #4 本文の双方に分散しており、「どの表示操作が
Project を変更し、どれが ViewState のみか」「Configured と
Observed をどう区別して表示するか」の単一参照先がない。後続
slice が独自解釈で契約を再実装する危険を断つため、View 層の
意味論を本書に固定する。

## Scope

### In Scope

- Signal Flow View の階層モデル（Semantic/Visual Hierarchy、
  Node 分類、scope/Collapse/Zoom/Role Layer）。
- Typed Port/Connection の表示・編集ルールと検証分類。
- Semantic Routing（Main/Send/Sidechain/Return）の表示・編集
  ルールと Mixer 整合。
- Probe 表示/編集ルールと状態語彙。
- SourceMap/Configured/Observed の世代証拠と受入仕様
  （scenario ID 対応表）。
- #104〜109・Ruviform との Owner 境界と実装依存関係。

### Out of Scope

- `crates/` のコード変更、manifest/lockfile/CI 設定。
- SF-01〜06 各文書の supersede/書き換え。
- Ruviform Generic Graph/Compiler/Probe/Clock の変更。
- 実 OS GUI・実 Plugin・実 Device の受入（#114/#109 外部 gate）。
- 未決事項の決定（Visual Group 保存粒度・Channel Map UI 既定・
  特殊 Feedback・GUI 数値閾値 — Decision Log で「提案/決定待ち」
  として分離）。

## Components and Boundaries

| 層 | Owner | 責務（本書での位置づけ） | やらないこと |
| --- | --- | --- | --- |
| Semantic 正本 | `synvelis-project`（#104/#105） | `RoutingModel`/`SemanticPort`/`RoutingConnection`/`SendConfig`、CAS・semantic validation、Mute/Solo 権威 | runtime ID 生成・永続化、View 状態の保持 |
| Lowering/Activation | `synvelis-player` + `synvelis-project` 契約型（#106） | `RoutingSourceMap`・`RoutingBuildKey`・`RoutingActivationState`・世代証拠 | semantic 正本の変更、View 専用写像の複製 |
| Probe | `synvelis-player`/`synvelis-project`（#107）+ Ruviform broker | `SemanticProbePoint`→runtime 解決、`ProbeResolveState`/binding/segment | Processor Bypass/Routing の変更、第二の Probe 正本 |
| Signal Flow View | `synvelis-workspace`/`synvelis-app`（#108） | `signal_flow_graph` 投影・`SignalFlowView`/`SignalFlowVisualState`・gesture・inspection・persist | DSP/Project/Undo への直接書き込み、第二 domain model |
| Cross Gate | #109（Test Owner） | SF-MODEL/VALID/ACT/PROBE/UX/COMPAT Case family・ledger・oracle | 本書 scenario ID の再定義（対応付けのみ） |
| Ruviform Generic | ruviform-graph/ruviform-audio typed/ruviform-plugin-host | Typed Graph validator・Compiler・publish/RT activation・probe rebind/clock align | Synvelis 意味論（Send Level/PrePost/Mute/Solo/Undo）の保持 |

**境界不変条件**: runtime `NodeId`/`PortId`/`ConnectionId`/`ProbeId`/
plan `generation` は runtime-scoped で Project に永続化しない。
ViewState（node 位置・Visual Group・fold/zoom・inspection focus・
routing gesture）は `ViewInstanceId` 単位の非 DSP 状態であり、
`ProjectRevision`/Undo/Graph build/PCM を変更しない。

## Canonical Design

### A. Signal Flow 階層モデル

#### A.1 二つの階層

- **Semantic Hierarchy**: 実際の処理・所有構造と一致する階層。
  `Track > ChannelStrip > ProcessorChain > Processor`、および
  `Bus > ChannelStrip`、`Return`、`Master`（論理 Bus で厳密 1 件）。
  表示順・Folder・Track template は DSP Routing と無関係
  （Issue #10 の Track/Lane/Clip/Source と実処理 Graph の分離に
  従う）。
- **Visual Hierarchy**: ユーザーが作業上ノードをまとめるだけの
  表示グループ（例: 「Drums」「Sidechain Network」）。
  `SignalFlowVisualState.groups`（`SignalFlowVisualGroup`:
  id/label/members/collapsed）として `ViewInstanceId` 単位の
  ViewState に保持し、Project/Routing/Undo/Semantic Diff の
  いずれにも影響しない。

#### A.2 Node 分類

| 分類 | 対応する semantic entity | 公開する境界 |
| --- | --- | --- |
| Container Node | Track（StripOwner 経由）・Bus・Return・Master・Effect Rack 相当の strip | strip の Main In/Out・Send/Sidechain tap 点 |
| Processor Node | `Processor`（chain 内の instrument/EQ/Comp 等） | processor の入力/出力 port（Audio Domain） |
| Endpoint Node | Hardware Input/Output・Master 出力境界 | device endpoint（実 device 非接続時は Unavailable） |

接続・分岐・合流は木ではなく独立した Edge（`RoutingConnection`
1:1）として表す。Container の折りたたみは表示の集約であり、
内部 edge を一本化・削除しない — 折りたたみ時も Main/Send/
Sidechain の各接続の意味を維持し、必要に応じて代表 port と
省略数を表示する。

#### A.3 Scope・Collapse・Zoom・Role Layer

- **Scope**: `Overview`（strip node のみ）/`Local{root}`
  （root 周辺の induced subgraph）/`Expanded`（processor node
  と Chain edge を展開）の 3 系統。`History` scope は snapshot
  基盤未搭載のため `coverage=Unsupported`。
- **Collapse**: `SignalFlowViewOptions.collapsed`（entity 単位）
  で processor 展開を畳む。node/edge 自体は消さない。
- **Semantic Zoom**: Overview/Standard/Detailed で表示情報量を
  変える。表示省略は「不在」を意味しない。
- **Role Layer**: `SignalFlowEdgeRole { Main, Sidechain, Send,
  Chain }` のフィルタ。Audio Main / Send・Return / Sidechain /
  MIDI・Event / Automation は意味的レイヤーとして切替可能で、
  フィルタは Routing を変更しない。現行 `SignalFlowLayer` は
  `Audio` のみ有効（`#[non_exhaustive]`、他 layer は未定義）。

### B. Typed Port/Connection

#### B.1 SemanticPort の属性

`SemanticPort`（SF-01 正本、`synvelis-project`）が持つ意味属性:

- `PortOwner`（Strip/Processor 等の所有者）と安定 `SemanticPortId`
  — runtime `PortId`/Plugin local port 番号とは別型。
- Direction（Input/Output）、Signal Domain（Audio/MIDI-Event/
  Control — Sidechain は独立 Domain ではなく Audio の補助 Role）、
  Role（Main/Aux/Sidechain/Parameter 等）、Channel Layout、
  Multiplicity、`PortBinding`（Resolved/Unresolved）、
  `channels`（SF-02 で persisted 追加）。

#### B.2 RoutingConnection の kind と Send

`RoutingConnection { id, source, destination, kind, enabled,
channel_map }` が接続の唯一正本。`kind`:

- `Main`: strip の主経路（track→bus→master、bus→master 等）。
- `Send(SendId)`: `SendConfig { id, connection, tap:
  PreFader|PostFader, gain_target }` が同一 Connection への
  厳密 1:1 参照。別の接続先を保存しない。`gain_target` は
  `Project::control` の `ParameterTargetId` 権威のみ。
- `Sidechain`: compressor 等への制御側入力。Main と混同しない
  独立 kind。

同一 (source, destination) に Main と Send/Sidechain が併存し得る
（SF-VALID-02）— 表示・編集では kind ごとに区別し、二重 Edge
として描画しない（同一 `RoutingConnection`/`Send` の dedup は
SF-UX-01 の契約）。

#### B.3 接続検証の分類

接続 gesture の reject/説明可能な未対応は `RoutingEditError`
（SF-02 正本）の typed 分類に対応付ける:

| 検証対象 | 分類 | 表示・編集上の扱い |
| --- | --- | --- |
| Port 方向（Output→Input のみ） | `InvalidDomainOrDirection` | hover/preview で非適合を提示、confirm で reject |
| 信号 Domain（Audio/MIDI-Event/Control の混在） | `InvalidDomainOrDirection`/`UnsupportedChannelMap` | 非対応理由を提示、暗黙変換なし |
| Channel 数不一致 | `UnsupportedChannelMap` / `RequiresUserDecision` | `ExplicitChannelMap` + 明示 consent なしに commit しない |
| 循環/Feedback | `FeedbackCycle` | 全 cycle 初期一律 Reject（遅延有無を問わない） |
| Port 不在/Unresolved binding | `UnresolvedPort`/`StalePortCapability` | 別 Port への黙った付替え禁止、Unresolved 保持 |
| Multiplicity/所有重複 | `DuplicateOwnership`/`DanglingReference` | persisted 契約違反として reject |
| 汎用 Graph 違反 | `GenericGraphRejected` | Ruviform Validator の reject をそのまま分類 |

編集失敗は `ProjectOp::Routing` transaction の reject であり、
Live Revision・Undo・接続・実行 Graph を不変に保つ
（SF-TX-01/SF-VALID-07）。preflight preview は Project を変更
しない。

### C. Semantic Routing 表示/編集ルール

#### C.1 操作入口の分離（調査 vs 編集）

| 操作 | 意味 | Project への影響 |
| --- | --- | --- |
| Node 本体 click | Inspection Focus 更新（`SignalFlowInspectionFocus`、Follow/Explore/Pinned） | なし（session-only の focus 履歴を除く） |
| Node 展開アイコン | Semantic Container の展開/折りたたみ | なし（ViewState の collapsed のみ） |
| Node 本体 drag | 表示位置のみ変更（`visual_move_node`） | なし |
| Port 起点 drag | 接続作成/付替えの Candidate Preview→明示 Confirm | confirm で 1 `ProjectOp::Routing` = 1 CAS/1 Undo |
| Connection click | Connection の調査（Inspector 連携） | なし |
| Connection 右クリック/Inspector | 接続解除・Enable・Send Level・Pre/Post の明示編集 | 明示 Command のみ |
| Effect Chain 専用順序ハンドル | Processor の実処理順序変更（`ReorderProcessor`） | 1 CAS/1 Undo。通常の node drag とは別操作 |
| Node/Browser メニュー | Processor 追加・削除等 | 明示 Command |
| Probe ON/OFF | 観測のみ制御 | なし（§D） |

#### C.2 Gesture の境界

Port Drag（`RoutingGesture`、SF-05 slice2 / SF-UX-03）の lifecycle:

1. **Begin**: source port・発生 view/window・`expected_project_
   revision`・`expected_port_capability_generation` を fixation。
2. **Hover**: dry-run assessment のみ。project/undo/engine に触れない。
3. **Drop**: candidate を固定（port role 整合・channel layout・
   stale revision/capability/window transfer を検出）。
4. **Confirm**: 1 回の `ProjectOp::Routing` transaction として
   commit（= 1 Undo 境界）。channel 変換は `ExplicitChannelMap`
   consent なしに commit しない。
5. **Cancel**: project revision・undo・runtime activation・
   graph build・PCM を一切変えない。

stale revision/capability・window transfer・port role 不整合は
commit 前に検出し、誤 target 変更・部分永続化を起こさない
（SF-VALID-07）。late receipt は generation 照合で拒否する。

#### C.3 Mixer との整合

Signal Flow と Mixer（strip fader/pan/send control）は同一
`RoutingModel`/`Project::control` を正本とし、二重の編集正本を
持たない。Send Level は `SendConfig.gain_target` →
`ParameterTargetId` の制御経路であり、Signal Flow 上の Send
edge 表示と Mixer の send strip は同一値を参照する。Mute/Solo
は `ControlOwner::Strip` + `ParameterRole::Mute/Solo` の
Boolean target 権威で、audibility 評価（`audible_strips`）が
Main/Pre/Post Send/Sidechain の可聴経路を決定する — 物理 edge
削除ではない（SF-TX-07/08）。

### D. Probe 表示/編集ルール

#### D.1 標準 Probe 能力

全論理ノード（strip/processor/endpoint 相当）は観測能力・観測
可能な境界を構造として宣言できる。ただし無意味な仮想バッファや
Plugin 非公開内部地点のキャプチャは保証しない。Main
Input/Output・Aux/Sidechain・Instrument の MIDI/Event と Audio
Output・Control Source/Target は Domain に応じた観測可能性を
公開し、physical tap がない・非 Audio port は `Unavailable`/
`Unsupported`、偽 PCM を出さない（SF-PROBE-01）。

#### D.2 実行段階

- **Off**（初期既定）: 観測停止。不要な PCM copy/解析なし。
- **Monitor**: Peak/RMS 等の軽量な継続観測。
- **Capture**: 有界な期間/容量で PCM 等を取得。終了後は
  Monitor へ戻せる。

#### D.3 状態語彙

`SemanticProbePoint { entity, boundary, semantic_port }` →
`RoutingSourceMap`（Active generation）経由で runtime
`ProbePoint`/実 slot を非 RT 解決する。`ProbeId` は Broker
生存期の一時 ID で Project/VCS の正本ではない。

| 状態（`ProbeResolveState`） | 意味 |
| --- | --- |
| `Requested`/`QueuedForBinding` | bind 要求済み・enqueue 済み。RT install 未確認で観測成功ではない |
| `PendingRtInstall`/`BoundActive` | RT 側 install/bound の実 Ack 証拠（`BoundActive` のみ RT 観測済み） |
| `PartiallyRebound`/`Lost`/`Stopped` | generation rebind の部分状態・binding 喪失・probe 除去 |
| `Unavailable`/`Unsupported` | physical tap なし・観測 domain 外 |
| `Unresolved`/`StaleGeneration` | entity/port 参照不在・SourceMap 失効・旧 generation の観測 |

Issue #4 本文の `Unbound`/`Partial`/`Unknown` は概念名であり、
実装の正本語彙は上記 enum（`synvelis-project::probe`）に従う。
`Unknown`/`Unobserved` は engine 未接続・map なしの表示側状態
（`SignalFlowNodeRuntime::Unobserved` 等）であり、binding state
とは別層として区別する。

#### D.4 不変条件

- Probe ON/OFF・Monitor/Capture は Processor Bypass・Connection・
  DSP/PCM/perf 基準・Raw Recording/Source Blob を変更しない
  （SF-PROBE-02）。
- Graph 変更時の PCM Capture 区間分断・自動再 Capture なし。
  Monitor は再 Binding の実成功後に継続（#308/#309 の契約が
  満たされるまで安全 Gate/Unsupported を明示）。
- 複数 Probe の Clock Domain/Epoch/Frame/Latency/Gap 整列は
  Ruviform #309 の契約を利用し、Synvelis は Send/Bus/Sidechain
  経路と Automation/Mute/Solo による説明のみを担う。
- Recording Raw Writer（Lossless/Fail-stop）と Lossy Probe PCM
  を混同しない。first anomaly *observed* ≠ proven root cause
  （SF-PROBE-08）。

### E. SourceMap/Configured/Observed 受入仕様

#### E.1 世代証拠の分離

| 証拠 | 名前空間 | Owner | 意味 |
| --- | --- | --- | --- |
| Configured | `ProjectRevision` | `synvelis-project` | semantic configured 状態（CAS 権威） |
| Build Key | `RoutingBuildKey`（revision+fingerprint+capability/engine/source-format generation+clock epoch+policy） | `synvelis-project`/`synvelis-player` | 非 RT compile/validate/lower/prepare の identity |
| Prepared/Published | plan `generation` | `synvelis-player`/Ruviform | `GraphRuntime::publish` 成功点（非 RT 証拠） |
| Active | `active_generation`（`SharedStatus`） | Ruviform `RtRuntime` | RT `process(frames>0)` が実際に駆動する世代 |
| Observed | probe binding/segment・`RoutingActivationReceipt`・control trace | `synvelis-player`/`synvelis-workspace` | RT 観測された実測証拠 |
| Last-good | `last_good_generation` | engine | 最後に RT 観測された Active 世代。失敗経路で後退しない |

`Configured R_new / Active R_old` を区別して表示する。停止中・
frames=0・経過時間だけで publish/active の成否を断定せず、
`PublishedAwaitingCallback` 相当の Pending を明示する
（sf-03-map §2.2 の契約を View 層へ適用）。

#### E.2 SourceMap の単一権威

`RoutingSourceMap` は lowering と同時に生成され
`PreparedRouting` に同梱され、publish 時に build key/generation/
map の内部整合（`BadSourceMap`）を検査される。semantic
Node/Port/Connection → generation-scoped runtime
Node/Port/Connection の**唯一の非正本対応表**であり、Control/
Probe/UI/Offline が同じ Map を読む。semantic ID→runtime ID の
写像は generation-scoped で plan swap/plugin reload/port
reorder で失効し、Project へ runtime ID を永続化しない。
View は `source_map_revision`/`source_map_generation` と live
`ProjectRevision` を比較して `stale` を表示する（独自の「active」
判定を持たない）。

#### E.3 表示語彙

Signal Flow の status 表示は以下を区別する（`SignalFlowStatus`/
`SignalFlowNodeRuntime`/`SignalFlowProbeStatus` の合成規則）:

- `Configured`（receipt の `RoutingActivationState` verbatim）
- `Prepared`/`Published`/`Active`（各 generation を個別表示、
  `Publish != Active` を潰さない）
- `Observed`（RT-confirmed の probe/activation 証拠）
- `Inferred`（根拠付き推定 — first anomaly observed の区間強調
  まで。根本原因の断定はしない）
- `Unavailable`/`Unresolved`/`Unsupported`/`NotLowered`/`Stale`/
  `Lost`（取得不能・失効・未対応を区別）
- `Unknown`/`Unobserved`（engine 未接続・map なし —
  Silence/0 値に置換しない）

#### E.4 受入仕様（scenario ID）

Issue #4 の受入項目を scenario ID で追跡する。正式 Case ID
registry は #109（SF-06）が Test Owner であり、本書の ID は
設計レベルの追跡子であって ledger 行を再定義しない。lane
（headless/実機）の最新状態は `evidence/reports/sf-06-cross-gate-
ledger.md` と各 slice の impl 記録を参照。

| Scenario ID | Issue #4 受入項目 | 主な既存契約 | lane 状態（2026-11-14 時点） |
| --- | --- | --- | --- |
| SFD-HIER-01 | Bass→Music Bus→Master を Local/Expanded で探索、Container 展開・折りたたみ後も接続が正しく表示 | SF-UX-01/02（`signal_flow_graph` scope/collapse/dedup）、SF-MODEL-01/02 | headless Pass（sf-05 slice1）。実 GUI 描画は SF-UX-08/#114 NotRun |
| SFD-ROUTE-01 | Bass→Reverb Return への Send 作成、Main と混同せず Mixer と整合 | SF-VALID-02/03、SF-TX-04、`SendConfig` 1:1、`SendTap` | headless Pass（routing model/transaction）。実 PCM 検証は SF-03/実機 |
| SFD-ROUTE-02 | Kick→Compressor Sidechain 作成、Main と混同せず Mixer と整合 | `ConnectionKind::Sidechain`、SF-TX-04/07/08 | headless Pass（audibility 評価）。実機 NotRun |
| SFD-VALID-01 | Port 方向/信号型/未対応 Channel/循環/Port 不在の接続を拒否または説明可能な未対応状態にし、編集失敗で実行 Graph を壊さない | `RoutingEditError` 分類、SF-TX-03/05/06、SF-VALID-06/07 | headless Pass（project 層）。runtime 影響の実機確認は SF-ACT 系 |
| SFD-EDIT-01 | ノード移動は DSP 順序に影響せず、Effect Chain の明示的ハンドル操作だけが DSP 順序/Undo を変更 | `visual_move_node` vs `ReorderProcessor`、SF-UX-02/03、SF-MODEL-06 | headless Pass（revision/undo/graph-build 不変を検証済み） |
| SFD-HIER-02 | Semantic Container と Visual Group が独立、Visual Group の作成/解除が音楽的処理へ影響しない | `SignalFlowVisualState.groups` vs `RoutingModel`、SF-MODEL-06、SF-UX-02/07 | headless Pass。Visual Group 高度 UI・保存粒度は未決（§H） |
| SFD-PROBE-01 | Probe ON/OFF が Processor Bypass・Routing を変更せず、Input/Output/Sidechain 等の能力・Unavailable 表示が一貫 | SF-PROBE-01/02/03、`ProbeResolveState`、SF-UX-05/06 | headless Pass（sf-04 受入集約）。実機/実 plugin は NotRun |

Issue #4 §7 の残り受入（2 地点 PCM 比較・Inspector Follow/Explore/
Pinned・Link Group・Multi-Window・50〜100 track 規模・音声処理と
UI の分離）は #109 の SF-PROBE-04〜09・SF-UX-04〜08・SF-COMPAT
系および #114 gate に対応付け、本書では重複定義しない
（SFD-PROBE-01 の表示規則がこれらの前提となる）。

## Data and Control Flow

```
ProjectSession ──project()/revision()──▶ RoutingModel (Configured)
        │                                    │ signal_flow_graph()
        │                                    ▼
SessionRoutingEngine ──receipt()/source_map()/probe_bindings()──▶
        │ SignalFlowRuntimeEvidence           SignalFlowView
        │ (app snapshot)                      {revision, coverage,
        ▼                                      nodes, edges, status}
   synvelis-app ──▶ SignalFlowScreenState / GPUI view
        │
        ├─ syn.signal_flow.view    (read-only 投影 + status)
        ├─ syn.signal_flow.visual  (visual-only mutation → ViewState)
        ├─ syn.signal_flow.inspection (focus mode 履歴、session-only)
        └─ syn.signal_flow.routing (gesture begin/hover/drop/confirm/cancel)
              │ confirm のみ
              ▼
        ProjectOp::Routing (SF-02 transaction)
              │ 1 CAS / 1 Undo / preflight preview
              ▼
        ProjectController::apply → RoutingModel commit
              │
              ▼（非同期・別証拠）
        SF-03 prepare→publish→observe_activation (世代証拠)
              │
              ▼
        SF-04 probe resolve/bind (generation-scoped SourceMap)
```

読み取り経路（view/visual/inspection）は session・project・
engine の永続状態を変更しない。書き込みは `confirm` 時の
`ProjectOp::Routing` のみが CAS/Undo を生成し、失敗時は
pre-commit 状態を保持する。activation/probe の証拠は commit
とは別経路で到着し、UI は `Configured R_new / Active R_old`
を generation 付きで区別する。

## Non-Functional Constraints

- RT/UI 分離: 表示・ドッキング・観測操作だけで Graph 再構築や
  RT ブロックを発生させない。view query は read model 投影で
  あり、engine 内部状態を複製しない。
- 決定性: 投影は `BTreeMap` 順の決定的順序。`max_nodes`/
  `max_edges` 超過は決定的 prefix で `Truncated`。
- 第二正本の禁止: ViewState・表示状態を Project/Graph の第二
  正本にしない。`RoutingConnection`/`SendConfig`/`RoutingModel`
  が唯一の routing 正本。
- Ruviform 非重複: Generic Graph validator・Compiler・PDC・
  Probe broker・clock align を Synvelis で再実装しない。不足は
  Ruviform Issue として記録する。
- 証拠の誠実性: Unknown/Unobserved/Zero/Silence/Unavailable を
  区別し、推定値・fake PCM・silent success で埋めない。

## Risks and Trade-offs

- scenario ID が #109 ledger の Case ID と混同される危険 →
  本書 ID は `SFD-*` の設計追跡子と明記し、正式 Case は #109
  registry を参照させる。
- View 層の契約を本書に集約することで、slice 文書との二重管理
  が生じる危険 → 本書は「対応関係の正本」に限定し、型・状態
  機械の詳細は各正本 doc を authoritative と明記。
- 未決事項を本書で決定してしまう危険 → §H で採用/非採用/保留を
  分離し、Issue #4 が「提案/決定待ち」とした項目は決定しない。
- headless 契約と実 GUI 受入の境界曖昧化 → lane 別状態を
  scenario 表に明記し、実機部分は NotRun として残す。

## Decision Log（採用・非採用・根拠）

| # | 決定 | 採用/非採用/保留 | 根拠 |
| --- | --- | --- | --- |
| D1 | Signal Flow View は `RoutingModel` の read-only 投影 + `ViewInstanceId` 単位 ViewState で構成し、第二 domain model を作らない | 採用 | SF-05 slice1 実装済み・Issue #4「表示と実行の分離」 |
| D2 | Semantic Hierarchy と Visual Hierarchy を別状態として保持 | 採用 | Issue #4 合意・SF-MODEL-06 |
| D3 | Port Drag=Candidate Preview→confirm で 1 `ProjectOp::Routing`（1 CAS/1 Undo）、hover/cancel は Project 不変 | 採用 | SF-05 slice2/SF-UX-03・SF-TX-01 |
| D4 | Node move/Visual Group/fold/zoom は `SignalFlowVisualState` のみ変更 | 採用 | SF-UX-02・SFD-EDIT-01 |
| D5 | Effect Chain 順序は専用ハンドル（`ReorderProcessor`）のみが変更 | 採用 | Issue #4・SF-02 |
| D6 | Main/Send/Sidechain を `ConnectionKind` で区別し、同一 pair の併存を dedup せず kind 別 edge とする | 採用 | SF-VALID-02・SF-TX-04 |
| D7 | 接続検証は `RoutingEditError` typed 分類に対応付け、暗黙変換・別 Port 付替えをしない | 採用 | SF-TX-03/05/06・SF-VALID-06 |
| D8 | Probe は Semantic Point→SourceMap(Active)→runtime slot の非 RT 解決、状態語彙は `ProbeResolveState` 系を使用 | 採用 | SF-04・SF-PROBE-01..08 |
| D9 | `Configured/Prepared/Published/Active/Observed/Inferred/Unavailable/Unknown/Stale` を generation 付きで区別表示 | 採用 | sf-03-map・SF-UX-05・Issue #4「音声に関する説明の誠実さ」 |
| D10 | Visual Group の保存・共有粒度（Project 永続 vs ViewState のみ・複数 View 共有） | 保留（提案/決定待ち） | Issue #4 で PoC 決定待ち。現行は `ViewInstanceId` 単位 ViewState |
| D11 | 複数 Channel Mapping の UI 提供方式・既定 | 保留（提案/決定待ち） | Issue #4 で未決。`ExplicitChannelMap`+consent が現行契約 |
| D12 | 特殊 Feedback（遅延付き loop）の許可 | 非採用（初期） | 全 cycle 一律 Reject が合意済み。Runtime Capability 検証と別明示合意まで |
| D13 | Inspect Mode/Edit Mode の大きな切替を必須化 | 非採用 | Issue #4「全体 Inspect/Edit Mode 切替を初期仕様として必須にしない」。操作入口の分離で代替 |
| D14 | Graph Canvas の新規共通 UI を Ruviform に要求 | 非採用（現時点） | Issue #4「両 Consumer 等から具体的な共通要求が発生した場合のみ」。framework-neutral 契約（#257）の範囲で実施 |
| D15 | Probe PCM Capture を Recording Writer と同一機構にする | 非採用 | Lossy Probe PCM と Lossless/Fail-stop Recording を混同しない（Issue #4・SF-PROBE-05） |

## Deferred Design Concerns

- Visual Group の保存・共有粒度・高度な作成 UI | Issue #4 で PoC
  決定待ち | Visual Group UI slice | ViewState/Workspace schema の
  拡張判断を誤り Project 正本と混線する。
- 複数 Channel Mapping の UI・既定 | 未決 | channel map 編集 UI slice |
  暗黙 up/downmix の UI 既定を固定する危険。
- 特殊 Feedback 許可条件 | 初期 Reject 合意 | Runtime Capability 検証・
  別明示合意 | 機能不足に留まる可能性（安全側）。
- Graph Canvas 描画・ドラッグ閾値・Semantic Zoom 数値・
  Accessibility/IME/HighDPI | 未決 | 実 GUI slice（#108/#114） |
  headless 契約と実 GUI 受入の境界が曖昧になる。
- SF-UX-06 Probe 共有/capture dedup・SF-UX-08 実 GUI 受入・
  SF-ACT/PROBE の実機部分 | #107 配線・#114 gate・#109 oracle |
  後続 slice | headless Pass を実機受入と誤認する危険。
- Ruviform #308/#309 の残存 gap（QueueFull 原子性・clock align
  実測） | upstream Issue | upstream gate | Synvelis 側で恒久的に
  重複実装しない方針を維持し、safe Gate/Unsupported を明示。

## Verification Notes

- [ ] 本書が SFD-R1〜R8 の全領域（階層・Port/Connection・
  Routing 編集・Probe・SourceMap/世代証拠・scenario ID・Owner
  境界・未決分離）をカバーする。
- [ ] scenario ID 表（§E.4）が Issue #4 の受入項目 6 件を全て
  カバーし、各 ID に既存契約と lane 状態が対応付いている。
- [ ] `docs/specs/signal-flow-design.md`・本書・plan・task・
  test-spec・ir・exp の文書チェーンが存在し relations が解決する。
- [ ] `evidence/sv4/` に Decision Log・代替比較・既存 SF 文書との
  整合確認記録がある。
- [ ] `mise run fmt-check` exit 0、触った docs dir の index が
  `in-sync`。
- [ ] `crates/`・`Cargo.toml`・`Cargo.lock`・`mise.toml`・
  `.github/workflows/` に差分がない。

## Sources and Relations

- spec: `docs/specs/signal-flow-design.md`
- Issue: <https://github.com/scarecrow173/synvelis/issues/4>
- 実装 Owner: #104/#105/#106/#107/#108/#109、実機 gate #114。
- Ruviform: #184/#185/#308/#309（#257 は UI framework-neutral
  契約の参照先）。
- 上位設計: `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`、
  `docs/designs/sf-05-slice{1,2}-*.md`、
  `docs/designs/playback-mixer-generic-execution-graph-architecture.md`、
  `docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md`。
- 証拠: `evidence/sv4/`（design-rationale・consistency-check）。
