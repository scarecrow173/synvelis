# Synvelis × Ruviform — Playback / Mixer / Generic Execution Graph Architecture

- **Status:** Approved design baseline v0.4（2026-11-01）／実装・PoC未着手
- **Decision tracker:** [Synvelis #18](https://github.com/scarecrow173/synvelis/issues/18)
- **Parent baseline:** [Project / Asset / Persistence](project-asset-persistence-architecture.md)（#17）
- **Synvelis inputs:** [#4 Signal Flow](https://github.com/scarecrow173/synvelis/issues/4)、[#6 Automation](https://github.com/scarecrow173/synvelis/issues/6)、[#8 Workflow](https://github.com/scarecrow173/synvelis/issues/8)、[#10 Project Model](https://github.com/scarecrow173/synvelis/issues/10)、[#12 Recording](https://github.com/scarecrow173/synvelis/issues/12)
- **Ruviform inputs:** [#184 Graph](https://github.com/scarecrow173/ruviform/issues/184)、[#185 Probe](https://github.com/scarecrow173/ruviform/issues/185)、[#186 Control](https://github.com/scarecrow173/ruviform/issues/186)、[#187 Duplex](https://github.com/scarecrow173/ruviform/issues/187)、[#188 MIDI](https://github.com/scarecrow173/ruviform/issues/188)、[#59 Timestamp](https://github.com/scarecrow173/ruviform/issues/59)、[#113 Plugin Host](https://github.com/scarecrow173/ruviform/issues/113)

> この文書では合意した12の設計方針を正本として記録する。Rustの公開API/型名、crate構成、具体的な補償アルゴリズム、Kernel/SIMD、Plugin ABIの対応範囲、Buffer容量や実行性能はPoCで検証後に決める。既存のRuviform実装済みAPIと未来の目標設計を区別する。

## 0. 現行Ruviformの確認結果（設計前提）

- [ruviform-audio/graph.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-audio/src/graph.rs) のGraphはVec<Box<dyn AudioNode>>型の順序実行で、AudioNode::pullは1つの音声出力Viewを受ける。複数Portと任意EdgeによるGeneric DAGではない。
- [ruviform-audio/mixer.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-audio/src/mixer.rs) は平面f32入力を加算するMixer。Track Channel Strip、DAW Send、PDCは別責務。
- [ruviform-audio/plan.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-audio/src/plan.rs) はGraphPlanをMutexで保護し、RTからtry_lockでDriveする。PlanSwapはRwLockのtry_readとlast-good cache、nonRT retire collectorでPlanを退役させる。Retire容量超過はRT Drop回避のためmem::forgetとoverflow count。新Graph実行でのStateful Node所有・退役は改めて検証する。
- [plugin-host/endpoint.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-plugin-host/src/endpoint.rs) のPluginEndpointはRT処理Portと非RT Hostを分離する。processは複数Input/Output Busを受け得るがprocess_audioの簡易Graph shellは単一Busで、実AdapterごとのMulti-bus/Sidechain動作は要確認。
- [plugin-core/event.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-plugin-core/src/event.rs) はブロック内Frame Offsetの順序検証を提供し、現状EventKindはParamValue。Timed MIDI全体は未完成。
- [media-core/music.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-media-core/src/music.rs) はPPQN960のTickとimmutable TempoMapを持つ。Raw MIDI Event Timestampを録音時に960 Tickへ破壊的に量子化しない。
- OutputのみのDevice/Sessionと、Input/Duplexの目標設計を混同しない。特に#59 Callback TimestampはChunk分割の意味論を再確認する。

## 1. 合意済みのアーキテクチャ原則

1. Project Authoring StateはSynvelisの単一ProjectControllerが所有する。RuntimeはDAWのTrack/Clipを直接変更しない。
2. GraphはCanonical Projectから導出されるExecution Representationであり、Project保存正本ではない。
3. Semantic Stable IDとRuntime Graph Node/Port/Buffer Slotは別のIdentityで、Mapping/Provenanceを保持する。
4. Graph構築、Plugin Instance準備、Capacity Validation、Buffer確保は非RT。RTはPrepared Planを有界に実行する。
5. 同一Stateful DSP/Plugin Instanceを複数RT Driverで同時処理しない。Plan交代時のOwner移管と非RT退役を保証する。
6. Project Revision、Compiled Plan Revision、Active Plan Revisionを区別。反映失敗時はlast-known-good Planを可能な範囲で保持する。
7. Generic Port/Latency/GraphはRuviform。DAWのTrack/Bus/Send/Solo/UndoはSynvelis。現行crate名へ無条件に合わせない。
8. Device RTとOffline Renderは可能な限り同じ評価規則を利用するが、Clock/Pluginの非決定性/制約を正確に公開する。
9. Channel/layout/port/latency/feedback/precisionの未対応を暗黙に変換・補完しない。

## 2. Project → Graph IR → Prepared Plan

~~~mermaid
flowchart LR
    P["Synvelis Project Snapshot R"]
    L["Semantic Lowering"]
    I["Ruviform Typed Graph IR"]
    V["Validate / Capabilities"]
    C["Compile: DAG / PDC / Buffers / Events"]
    E["Prepared Execution Plan P"]
    A["RT/Offline Activation"]
    P --> L --> I --> V --> C --> E --> A
~~~

### 2.1 3層の正本

| Layer | Owner | Stores |
| --- | --- | --- |
| Project Semantic Routing | Synvelis ProjectController | Track/Bus/Send/Return/Sidechain、FX Chain、Parameter/Automation、Playback Source |
| Generic Typed Graph IR | Ruviform generic capability | Node、Port、Edge、Signal Domain、Layout、Routing/Delay/Capability constraints |
| Prepared Execution Plan | Ruviform executor | 実行順序、RT Stateful Endpoints、Buffer slot、延遅Line、イベント計画、最大Block容量 |

ProjectとIRの間でLossがある場合、Unmappable/Unsupportedを説明し旧Planを継続する。UIのSignal FlowはProject Semantic Projectionであり、RT Graphの物理Node列を直接可視化したものではない。Compilerは相互に対応するProject Entity ID、Runtime Node/Port ID、Plan Generationと診断情報を返す。

### 2.2 概念API（未実装）

- SemanticLowering.lower(ProjectSnapshot, RenderContext) -> GraphBuildRequest + SourceMapping
- GraphValidator.validate(GraphBuildRequest, Capabilities) -> ValidatedGraph
- GraphCompiler.prepare(ValidatedGraph, ExecutorConfig, ResourceBudget) -> PreparedExecutionPlan
- ExecutionController.activate(PreparedExecutionPlan, ExpectedActiveGeneration) -> ActivationReceipt
- ExecutionController.parameter_events(bounded ordered batch) -> Ack / Rejection
- ExecutionController.observe(active generation) -> RuntimeStatus / Diagnostics

ここで型名・関数名は設計例であってRuviformの現行公開APIではない。Activeが必ず一Blockで更新される等、未実装の時間保証を表明しない。

## 3. Typed Port / Bus / Connection

PortはDirection(Input/Output)、SignalDomain(Audio PCM/Timed Event/Control)、Role(Main/Aux/Sidechain/Parameter等)、ChannelLayout/EventFormat、Clock Domain、Multiplicity、CapabilitiesとRuntime Availabilityを表現できること。

- RoleはSignalDomainとは別。Sidechainは多くの場合AudioのAux入力。
- ConnectionはSource Output→Target Inputを結び、Channel Mapping/Enabled/Connection Optionsを検証する。
- Fan-outは同Sourceの出力を複数Consumerへ共有できるが、共有状態を二重駆動しない。
- Fan-inは明示Mix/Aggregationノードまたは同等の評価契約を通す。
- 異なるChannelLayout/Format/Rateを接続する場合、変換を明示してユーザーに見せられるようにする。未承認Implicit Downmixは避ける。
- Missing Port、Invalid Direction、Noncompliant Format、複数接続制限、未対応Bus、未対応Feedbackは定義済みのErrorでRejectする。
- まず無遅延CycleはReject。Delay付きFeedbackは専用のDelay/Scheduling契約をPoC後に導入検討。

## 4. Compilerと実行スケジューリング

Compilationは非RTの次のパスへ分離する。

1. Port/Format/Direction/Channel/Clock Capability検証。
2. DAG構築とCycle検出。接続分岐・合流・Plugin専用Busを保持。
3. Explicit Converter/Channel Mapper/Delay挿入。
4. Node/EdgeのLatency計測・宣言とPDC Delay Planning。
5. Topological Scheduling、Frame/Event Boundary、Node処理回数の確定。
6. Last Consumerを基準にBuffer LifetimeとAlias/Reuse/Capacityを算定。
7. Plugin/Processor/Media SourceをnonRT prepareし、RT Scratchを事前確保。
8. Prepared PlanをValidationし、非RTでActivationへ渡す。

初期の実行方式は**単一RT Audio Driverの決定的順序実行**を推奨。大規模GraphのParallel Evaluationは性能計測で必要性を示してから設計する。固定容量を超えた場合はRTで新規確保せず、NonRTへ報告しDefined Failure/Fallback。

Bufferは分岐した全Consumerが読了するまで上書きしない。In-place可否、Input/Output別名、Plugin内部のBorrow Lifetime、Scratch reuseはCompilerの検証契約とする。Mixerの加算順序を固定して結果の再現性を検証する（全Floating Point環境でBit-perfectを無条件保証しない）。

## 5. Synvelis Mixer → Generic GraphへのLowering

~~~mermaid
flowchart LR
    S["Clip / Instrument / Software Monitor"]
    G["Clip Gain/Fade"]
    F["Insert FX"]
    T1["Pre-Fader Tap"]
    V["Fader/Pan"]
    T2["Post-Fader Tap"]
    O["Main Bus"]
    PS["Pre-Fader Send"]
    QS["Post-Fader Send"]
    S --> G --> F --> T1 --> V --> T2 --> O
    T1 --> PS
    T2 --> QS
~~~

- SynvelisがTrack Channel Strip、Folder、Bus、Return、Main、Send、Sidechain、Mute/Solo、Solo Safe等の意味論を管理する。
- 一般的なPre-Fader SendはInsert後/Fader前、Post-Fader SendはFader後を初期基準とする。Pre-FX Capture/Sendは別の明示Tap。
- FolderのUI階層変更だけではDSP Routeを変えない。
- Mixerで検証したGain/Pan/Send AutomationはRT Fast Control配送とし、毎回構造再Compileしない。
- Mute/SoloはTrackを物理削除することではなく明示的な可聴経路制御Policy。Pre/Post Sendへの影響、Solo Safe、Sidechain/Returnの扱いをSynvelisのCommand/Playback Policyとしてテストする。
- Master/Control Room/Monitor Bus/Export Tapは独立した出力役割。通常のMaster RenderにMonitor専用エフェクトが意図せず含まれない。

### 5.1 ルーティング例

Bass → Music Bus → Main Compressor → Master、Bass → Reverb Return → Master、Kick → Compressor Sidechain。MainとAux Input、SendとMain Output、Return合流をGeneric IRのPort/Edgeとして保持する。Source PCMを分岐ごとに重複Renderするのではなく、Fan-outと必要な各処理経路で共有する。

## 6. PDC / Latency / Clock

同一Clock DomainのDAG合流点で、入力経路iの既知の累積LatencyをL_iとすれば、整列用Delay D_i = max(L_j) - L_i。例: 128/512 Framesの2経路では128側に384 Framesを追加する。

- 経路Latencyの宣言元、Latency Confidence、Plugin/Adapter/Node/Varying Blockの最大値、現在のPlan Generationを区別。
- Plugin Latency変更、Bypass/Insert/Remove、Sidechain、Return、Multi-busを再Compile triggerとして扱う。
- Feedback、有界でないLatency、Clock Domain変換、外部Hardwareは上式だけで完全に扱えない。Unsupported/Unknownや要アダプタを表示。
- **Graph PDC ≠ Input ADC/Output DAC timestamp correction ≠ Recording Placement compensation**。Input Capture Raw SourceをPDCしたPCMにすり替えない。Double Countを防止する。
- Low-latency MonitorはRecorded Capture Gateと別のMonitor Plan/Bias。Hardware Direct Monitoringは能力によりSupported / External-reported / Unknown。
- PDCのDelay Lineは非RT確保し、処理中のLatency変更には安全なPlan準備/切替を必要とする。

## 7. Transport / MIDI / Automation / Event

- Engine Frame、Device Clock、Media Time、Musical Time、Source-local Time、Recording Session Frameを区別し、TempoMapやClock Epochを固定して変換する。
- Timed MIDI Note/CC/OffとPlugin Param Eventは同じFrame Offset/Ordering/Bounded Delivery契約を共有し得るが、意味とPayload/処理Targetは別。
- 生のMIDI取得時刻をPPQN960の整数格子へ破壊的にQuantizeしない。
- Seek/Loop/Stop/Plugin bypass時のNote Off/Controller Chase/Voice Stateは定義済みのFail-safe/Resetを持つ。無条件のAll Notes OffがすべてのPluginで十分とは保証しない。
- Automation/ModulationのBase/Reference/Host-resolved/Observedを区別。対応精度（Block/Control-rate/Sample-accurate/Audio-rate）はPluginごとに交渉し、Source寄与を追跡できるようにする。
- 同一Frame OffsetイベントのOrder、0-Frame Flush、Block境界、Loop、Tempo Change、イベント容量超過をテスト。
- 記録中のAutomation WriteはProject Transactionであり、RT Param Event配送とは別。

## 8. Audio Source Streaming / Prefetch

Decode / Remote Fetch / Format Conversion等のBlocking仕事は非RT Reader/Jobが実施。RTは事前確保したFrame Ring/Sliceを消費するのみ。Seek/Loop/Clip重複/Resampling/Short FadeやBounded Buffer PolicyはPrepared Planへ含める。

Prefetch Miss/Underrunは連続的な正常Audioと偽装せず、Timestamp/Range/Source/Graph Revisionを報告。出力は定義済みの安全な無音/減衰等へ移す。Remote Preview Cacheと正式Project Asset Blobの可用性は分ける。Offline Driverは非RTでReaderを待てるが、Realtime CallbackにはそのPolicyを導入しない。

## 9. RT/NonRT Graph UpdateとStateful Ownership

### 9.1 Fast Control vs Structural

| Update class | Examples | Required path |
| --- | --- | --- |
| Fast Control | Fader/Pan/Send Level/Param/Automation | Prepared bounded event/control path; no topology rebuild |
| Structural | Connection/Plugin/Bus/Channel Layout/PDC/Host lifecycle | NonRT build/validate/prepare → RT safe publish/activate/retire |

- 一つのStateful Plugin/DSP RT Endpointは、ある処理区間で厳密に1つのOwner/Driverのみがmutateする。
- Graphを再Compileする度にPluginの既存Stateを破棄・再作成することは禁止。Shared Stateful Endpointの所有移管（同時処理なし）、または明示的なIndependent Clone+State Restoreが必要。
- 旧PlanがまだDrive可能な間に同一Endpointを新旧二重処理しない。
- GraphPlanの退役はnonRTで行い、RTに最後のArc/Box DestructorやOS Cleanupを実行させない。既存PlanSwapのleak-on-retire-fullを観測し、長時間運用で無制限にリークを累積しないようPublish Throttle/Reject/Capacity/Drain cadenceをPoCで選定。
- 旧Planの参照が残ったまま新PlanにNodeを移す際のownership handoffはPlan Generation/Callback quiescenceで検証する。構造SwapとParameter Eventの世代不整合を明示する。

### 9.2 Revision / Activation Receipt

Project RでCompile PしてもActive Aは遅れ得る。状態例: Editing→Compile Pending→Prepared→Activation Pending→Active、またはCompile Failed/Activation Failed（旧A継続）。UIではProject保存とDSP適用を混同せず、Graph/Control/Source MappingのGenerationで理由を示す。

### 9.3 例外的なクロスフェード

Graph全体のDual Render Crossfadeは任意に実施しない。Stateful Processorが共有されている場合は二重実行する危険がある。独立Clone/Instanceが用意でき、Resource/Latency/Phase条件が明示できる場合だけ採用。初期は安全なBlock Boundary Activationを優先し、Click-less Guaranteeを全面宣言しない。

## 10. Realtime PlaybackとOffline Render

ReplayとOfflineは原則同じGraph IR / Evaluation / DSP順序 / Timed Event契約を使い、Clock DriverとI/O Adapterを差し替える。OfflineはRealtimeの生Plugin Instanceを直接並列駆動せず、別InstanceをPrepare/State Restoreする。

- Deterministic Test Nodesは同条件で同じ結果を出すことをPoCで検証。
- PluginのRandomness、External Device/Clock、L1 Isolation、Native Driver、Non-deterministic Parallelismは能力としてBest-effort/Unavailable/Non-deterministic等を説明する。
- Master Mix、Stem、Freeze/Bounce、Control Room Monitor Tapの違いを保持し、Monitor専用効果を通常Exportへ誤混入させない。
- ExportはnonRT Encode/Muxの完成Fileを検証・安全に公開する。Project Assetへの自動Commitは、ユーザーの要求がある場合を除き行わない。

## 11. Signal Flow / Inspector / Probe連携

- SynvelisはSemantic Track/FX/Port/Connection IDからRuntime Node/PortとActive Plan GenerationへのMappingを構築する。
- Probeが返す値はTimestamp/Frame/Clock/Node/Port/Graph Generation/Channel Layout/Latency Evidence/Loss Statusを含む。
- Configured（Project設定）、Observed（実測）、Inferred（推定）、Unavailable（非対応・未取得）を区別し、因果関係を音声波形の単なる同時変化から断定しない。
- Nodeの全PCMを常時保存しない。Meter / Selected Probe CaptureはBounded Budgetで有効化し、録音WriterのFail-stop/no-lossの品質保証とは異なるProbe Loss Policyを持つ。
- Probe/Inspector/Visual Groupの編集は必要なDSP構造変更を伴わない限りGraphを再Compileしない。

## 12. Ruviformの目標能力と移行戦略

| Workstream | Existing evidence | Target capability | Migration |
| --- | --- | --- | --- |
| Typed Graph & Compiler | ordered Graph/AudioNode, #184 | Multi-bus Ports、DAG、Buffer Lifetime、PDC | New compatible IR/Compiler adapter → phased replacement |
| Prepared RT Plan | GraphPlan/PlanSwap | Stateful owner separation、activation receipt、safe retire | 既存RT tests/old output path regression |
| Plugin Host | #113 Endpoint L0/L1 | Multi-bus、sidechain、latency/state negotiation | Existing control/RT splitを保持、ABI adapterを個別検証 |
| Device Clock | output Device port、#59/#187 | Input/duplex/accurate ADC/DAC Clock/Latency | Capabilityを提示し、不明時は不明と表示 |
| Timed Events | EventKind ParamValue、#186/#188 | MIDI/Control/ParamのBounded Timeline | 既存EventListの意味・orderingを維持 |
| Media/Offline | Audio/Media I/O primitives | Source streaming、nonRT Prefetch、Offline Driver、Encode/Mux | Backend実装と抽象Portを分ける |
| Observability | #185 | Probe/Port/Gene​ration Evidence | 選択的Captureと安全な退役 |

現行crate境界を固定しない。ただしRuviformはSynvelisだけでなく既存Video/CLI等のGeneric Consumerがあるため、変更時は公開API/依存DAG/互換層/移行/RT Regressionを記録。既存Approved ADRに矛盾する場合は正式なADR改訂を経る。具体的なcrate抽出・統合・非推奨化はPoC後に採否を決める。

## 13. PoC順序

- **EG-A Generic Audio Graph:** Source/Gain/Mixer、Fan-out/Fan-in、Send/Return、Aux Sidechain、Channel Mapping、Cycle Reject。Graph CompilerのNoAlloc RT Driver。
- **EG-B PDC/Plugin:** 遅延測定・補償、Plugin Bypass/Latency Change、L0/L1、RT Single Owner、Plan RetirementとGraph Replacement。
- **EG-C Playback/Transport/Streaming:** Media Source Reader、Clip Playback、Seek/Loop、MIDI Events/Automation、Prefetch/Underrun。
- **EG-D Activation/Offline:** Project Revision→Compiled Plan→Active Receipt、Safe Plan Swap、Independent Offline Instances、Master/Stem Export、Probe Evidence。

## 14. 受入マトリクス EX-001〜EX-020

| ID | Test | Expected |
| --- | --- | --- |
| EX-001 | Main Source→Output | Deterministic PCM一致 |
| EX-002 | Main + Send Fan-out | Sourceは一度だけ実行され、経路が一致 |
| EX-003 | 2 Branches→Return | 決定的なMixer順序と想定値 |
| EX-004 | Aux Sidechain Port | Mainと混線せずCapability検証 |
| EX-005 | Channel Layout不一致 | Defined RejectまたはExplicit Converter |
| EX-006 | Zero-delay Feedback | Defined Reject |
| EX-007 | 128/512 Frame並列経路 | 384 Frame補償でImpulse整列 |
| EX-008 | Plugin Latency変更 | NonRT Recompile、旧Plan継続 |
| EX-009 | 再生中Structural Edit | RT非Block、Revision/Statusを報告 |
| EX-010 | Stateful Processor Reuse | 二重process/不正Dropなし |
| EX-011 | Plan Retirement Queue飽和 | RT Drop/UBなし、Fault/ThrottlingまたはReject |
| EX-012 | Fader/Send Automation | Topology Recompileなし、時刻整合 |
| EX-013 | MIDI Note-off after Seek/Loop | Hanging Note防止のDefined Safe Response |
| EX-014 | Prefetch Miss | Underrun明示、RT File I/Oなし |
| EX-015 | Capture/Monitor/Playback | Raw Source/PDC/Directの分離 |
| EX-016 | Offline vs Live Stable Test Node | 同条件で一致 |
| EX-017 | Non-deterministic Plugin | 非保証根拠・Capabilitiesを表示 |
| EX-018 | Master/Stem Export | Tap/Channel Layout/Control Roomの分離 |
| EX-019 | Probe 2 Ports | Clock/Frame/Generation/Latency Evidence整合 |
| EX-020 | Outdated Project vs Active Plan | Pending/Failed/Activeを識別 |

## 15. 今回合意した12項目とPoC保留境界

**12項目の合意:** 3層分離、Typed Ports、Compiler、DAW Mixer意味論、PDC責務分割、Media Prefetch、共通Time/Event、Fast Control vs Structural、Stateful ownership/retirement、Playback/Offline、Source Mapping/Probe、Ruviform target-first migration。

**PoC保留:** 実Rust API、Graph Builder/Compiler配置、Buffer allocator、parallel scheduling、PDCの難条件、Feedback、Latency変動のクロスフェード、Plugin Adapter別Multi-bus実装、実Device timestamp、各OS/Backendの保証、Performance Budget、最終crate構成。

**次の全体設計テーマ:** Time / Transport / MIDI / Automation / Plugin Integration。合意原則を共通の時間・イベント配送・Plugin state契約へ結びつける。

**設計は合意済み。ソース実装・受入試験完了・全Backend可用性は未確認。**
