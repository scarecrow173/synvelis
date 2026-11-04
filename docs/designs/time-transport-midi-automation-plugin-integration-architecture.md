# Synvelis × Ruviform — Time / Transport / MIDI / Automation / Plugin Integration Architecture v0.5

- **Status:** Approved design baseline (2026-11-01). API implementation / PoC pending.
- **Decision tracker:** [Synvelis #20](https://github.com/scarecrow173/synvelis/issues/20)
- **Already approved time foundation:** [Synvelis #19](https://github.com/scarecrow173/synvelis/issues/19) / [Ruviform successor ADR](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/configurable-ppqn-resolution-independent-musical-time.md) / [migration #189](https://github.com/scarecrow173/ruviform/issues/189)
- **Synvelis dependencies:** [#6 Automation](https://github.com/scarecrow173/synvelis/issues/6), [#10 Project Model](https://github.com/scarecrow173/synvelis/issues/10), [#11 Timeline Editing](https://github.com/scarecrow173/synvelis/issues/11), [#12 Recording](https://github.com/scarecrow173/synvelis/issues/12), [#17 Persistence](https://github.com/scarecrow173/synvelis/issues/17), [#18 Execution Graph](https://github.com/scarecrow173/synvelis/issues/18)
- **Ruviform dependencies:** [#113 Plugin Host](https://github.com/scarecrow173/ruviform/issues/113), [#184 Generic Graph](https://github.com/scarecrow173/ruviform/issues/184), [#186 Control Event](https://github.com/scarecrow173/ruviform/issues/186), [#187 Clock/Device](https://github.com/scarecrow173/ruviform/issues/187), [#188 Timed MIDI](https://github.com/scarecrow173/ruviform/issues/188), [#189 Beat/PPQN](https://github.com/scarecrow173/ruviform/issues/189)

> この文書は2026-11-01に合意済みの**設計・正本・実行契約**を記録する。具体的Rust API/型名、Ruviform crate再編、Plugin Adapterの完全対応、RT実機保証、各定量Budgetは未実装・未検証であり、PoCで確定する。Ruviform現行TransportStatus=Stopped/Playing/Paused、EventKind=ParamValue、固定960の旧コードは目標APIと混同しない。

## 1. 目標構造

~~~mermaid
flowchart TD
  subgraph Synvelis ["Synvelis Authoring Domain"]
    P["Project Snapshot / Rational Beat / TempoMap"]
    S["MIDI Source / Automation / Plugin Binding"]
    T["Transport Intent / Automation Write"]
  end
  subgraph Prepare ["Non-RT Preparation"]
    L["Semantic Lowering"]
    C["Time / Event / Graph Compiler"]
    PLAN["Immutable Prepared Plan"]
  end
  subgraph Exec ["Ruviform RT / Offline"]
    CL["Device or Offline Clock"]
    SP["Bounded Transport Span Scheduler"]
    MI["Timed MIDI / Note Safety"]
    PA["Parameter Control / Event Delivery"]
    DSP["Plugin RT Endpoint / DSP"]
  end
  P --> L
  S --> L
  L --> C --> PLAN --> SP
  T --> SP
  CL --> SP
  SP --> MI --> DSP
  SP --> PA --> DSP
~~~

Project Authoring State、Materialized Raw Media、Active Planは別々の正本。**Command accepted ≠ Audio Engine applied ≠ Saved Revision**。Project Revision R、TempoMap Revision T、Active Plan Generation Aの相違は正常な一時状態として取り扱う。SynvelisはDAWの意味論、Ruviformは汎用Time/Event/Plugin/Clock能力を持つ。

## 2. 時間・分解能・世代契約

| Domain | Authority | Invariant |
| --- | --- | --- |
| Canonical Beat Position | Projectの正規化有理拍位置または同等のLossless Value | PPQN非依存、負値/7連符等を表現可能な範囲で保持 |
| Project PPQN | SynvelisのProject Setting | **新規既定960、可変**。Tick投影にのみ文脈として作用 |
| Project Media Time | MediaTime等の明示的な時間座標 | Beat TimeとはTempoMapで変換、両者を同一視しない |
| Source Native Time | 個々のSource | Audio Frame、SMF Division/Tick、録音MIDI Timestamp/Device Epochは不変 |
| Engine Frame | Audio Driver/Offline Driver | Project Loop Wrapでも基本的に処理Frameは単調 |
| Device Clock Domain / Epoch | Device Adapter | ADC/DAC Timestamp出典・連続性/信頼度。異なるDomainを無検証比較しない |
| Recording Session Frame | Recording Coordinator | Loopで巻き戻さず、Raw Captureの原時刻とPlacementを分離 |

ProjectRevision、TempoMapRevision、TransportGeneration、ExecutionPlanGeneration、ClockEpochId、必要に応じたPluginInstanceGenerationを別に保持。Timestamp信頼度はVerified/Estimated/Unknownを識別。

**変換:** Source Native Anchor → Project Beat/Media Mapping → Immutable Compiled TempoMap → Engine Frame/Event Offset。Beat→TickはPPQN+明示的丸め。中間は必要範囲で有理精度を維持し、整数Frame境界への丸めは最後に1回。Repeated frame↔tick↔frame base hopping禁止。Piecewise-constant TempoMapは現行方針を維持（Rampは別設計）。Overflow・分母上限・整数幅は明示Fail/PoCで定義。

## 3. Transport Command / Applied Receipt

| Command | Meaning | Required behavior |
| --- | --- | --- |
| Play/Pause/Stop | 再生/停止状態 | Stop時のNote Release/Tail Policyを明示 |
| Seek | Project Position不連続 | 通常Device Clock Epochをリセットしない |
| SetLoop/ClearLoop | Loop区間・反復Policy | Loop WrapはTransport Discontinuity |
| Tempo/Time Signature Edit | Project Transaction | 新TempoMapRevisionへのPrepared Transition |
| Device Restart | Device/Clock再準備 | Clock Epoch更新、Project位置変更は別判断 |

**Pipeline:** Command → Validation → nonRT Prepare → Bounded Scheduling → RT Apply → Applied Receipt。Receiptは要求のTarget/Transaction、Applied Engine Frame、TransportGeneration、PlanGeneration、成功/拒否理由を表す。UIはRequested/Pending/Applied/Rejectedを区別する。即時Applyを無条件保証せず、Latencyを観測可能とする。

Recording Session FrameはLoop Wrapで巻き戻さない。Device RestartでClock Epochが変わってもProject TimelineのClipを推測で移動しない。Transportが切り替わるBlockの異なるGenerationのイベントを同一状態と偽装しない。

## 4. Half-open Transport Span Scheduling

1. DriverがFrame range [F,F+N) を提供。
2. Prepared PlanはTempo変更、Seek、Loop Wrap、Clip Boundary、Transport/Plugin Context変更の候補を特定する。
3. **Transport Contextが変わる位置**で論理Spanへ分割（毎MIDI Eventで無条件再Processしない）。
4. SpanのProject Position、LoopIteration、TempoRevision、TransportGeneration、Plugin Transport Contextを確定。
5. Eventの**正本位置はEngine Frame**とし、Callback/Span/Plugin Process Callの相対Offsetは各起点を明示して配送境界で再計算する。Block/Span境界は半開区間で重複/欠落を抑制。
6. RT実行は事前確保されたMax Span/Event/Buffer容量以内。超過するPlanは非RTでRestrict/Rejectする。

**具体例:** 256-frame Callbackの100 Frame目にLoop Wrapが起きる場合、[0,100) と [100,256) に論理分割し、PluginのTransport Snapshotは各区間で正しいProject Positionにする。1 process callあたり1 Transport Snapshotしか受け付けないAdapterは2呼出し等のCapability整合を要するが、**Activated Pluginのmin_frames/max_framesに違反する分割は行わない**（§14）。Loop次Iterationの発音は別のEvent Occurrenceで識別するが、同じIterationのSource Eventを二重送信しない。

## 5. Timed Event Contract

概念上のEnvelope（**未実装API**）:

~~~text
TimedEvent {
  target: RuntimeTargetId,
  plan_generation: PlanGeneration,
  transport_generation: TransportGeneration,
  engine_frame: EngineFrame, // canonical occurrence position; 未実装の概念フィールド
  stable_source_event_id: Option<EventId>,
  loop_occurrence: Option<Iteration>,
  source_sequence: u64,
  payload: Midi | Parameter | Control | Boundary
}
~~~

- 正本のEngine FrameをCallback/Span/Plugin Process Callの各Originから相対Offsetへ変換する。実Plugin EventListのFrame Offsetは**各Plugin process call開始からの相対値**であり、Span相対値を無検証で再利用しない。Deadline/Generation/Target Bindingを検証する。
- 同一Frameは明示的な因果依存→Sourceの安定順序を保持。「NoteOffは必ずすべてのNoteOnより先」といった普遍優先順位は採らない。
- 既存Ruviform EventListのFrame Offset昇順・同Offsetの安定順序・zero-frame flushの契約を尊重する。Semantic TimelineとWire EventListを同一の正本にはしない。
- 古いPlan Generation/Plugin Instanceへ属するEventを無検証の新Targetへ配送しない。
- Continuous ParameterはTarget/補間/制御精度等を検証した場合のみCoalescing可能。NoteOff/Transport等の状態イベントには無条件Coalescingしない。
- MIDI Note/CC、Parameter、Transport、Diagnostic/Probeは異なるCapacity/Loss/Fault Policyを持つ。Drop/Overflowを正常処理と報告しない。

## 6. MIDI Source / Note Ledger / Safety

- ImportしたMIDIの元PPQN/SMF Division/Tick、Raw CaptureのDevice Timestamp/Clock Epochを保持。Projectへは明示Mapping、Quantizeは別Derived Versionとして適用。
- 初期はMIDI1 Note On/Off、CC、Pitch Bend中心。MPE/MIDI2/Per-noteはCapability拡張可能な余地を維持。
- NoteLedgerはTarget/Plugin Instance/Channel/Note Identity、Note重なり・発音系列、Sustain CC64等を必要範囲で扱う。
- Seek/Loop/Stop/Comp Boundary/Clip Delete/Plugin ReplaceにはRelease、Reset、Controller Chaseのポリシーを明示。元Raw Noteを破壊的に修正しない。
- MIDI1同Channel/同Pitchの重複Noteに完全なNote IDが存在するとは限らない。Plugin内部のVoice情報はホストが必ず取得できるわけではない。したがって「NoteLedgerにより全PluginでStuck Noteゼロ」を保証しない。
- NoteOffの喪失はEvent Integrity Fault。対象Plugin/AdapterのCapabilityに応じてAll Notes Off・Reset・Instance RestartなどのDefined RecoveryとDegraded状態を提示。
- MIDI CompはRecording #12で合意したTake/Phrase単位を初期対象とし、高度なNote単位Compを最初から要求しない。

## 7. Automation / Modulation / Gesture

**既存#6の合意を継承:** Base Value → Automation/ManualからReference → Modulation Contributions → Clamp/Quantize/Smoothing → Host-resolved → Dispatched Event → Plugin-reported/Observed。Automation Off≠Modulation Offであり、Base Valueも保存する。

| Scope | Time anchor | Example |
| --- | --- | --- |
| Track / Bus | Project Beatまたは明示Absolute | Fader/Send/Plugin Parameter |
| Clip local | Clip-relative + Playback Transform | Gain Envelope、Clip Expression |
| Note local | Note-relative | Note Expression |
| Project-global | Project Context | Macro / global control |

- Track AutomationとClip Gain/Fadeを原則別段として扱う。複数Automation/Modulation経路の競合を黙ってlast-writer-winsにしない。
- Block/Control/Sample-accurate/Per-noteの適用精度とPlugin Capabilityを実Adapterごとに明示。Host-resolved値がPluginの内部Smoothing後の値とは限らない。
- Off/Read/Touch/Latch/Write、Write Arm、Gesture State、Recording Transactionの区別を維持。Mode選択だけではRecordingが始まらない。
- Gesture開始時にStable TargetId、Plugin Instance/Binding Generation、Arm、TransportGenerationを固定。InspectorのFollow変更で記録対象を替えない。
- Automation WriteはRTがProjectを直接変更せず、非RT ProjectControllerへ確定する。デフォルトはModulation前Referenceを記録し、LFO適用済み値を再度Modulationする事故を避ける。
- Loop Multi-passの無断上書きを避け、Touch Return長・精度・録音間引き閾値はPoCで決める。

## 8. Plugin Lifecycle / State Capture / Event Generation

SynvelisはProject Stable PluginInstanceId、Plugin Identity、TargetId、Opaque State ContentRefを所有。Ruviform HostはPluginHandle、Control Commands、L0/L1、RT Endpoint、State I/Oを所有。旧/新Planで同一Stateful Pluginを同時Processせず、退役はnonRT。

Plugin State Captureの結果は次のように区別:

| Level | Meaning |
| --- | --- |
| Coordinated | Processingとの整合点を合意して取得 |
| Quiescent | Processing停止/静止状態で取得 |
| Best-effort | Snapshot取得成功、Audio Frameとの厳密整合は未保証 |
| Unsupported | 取得/復元できない |

State取得成功だけで「特定Frameに同期した完全Snapshot」と見なさない。RTに無期限停止やOS I/Oを求めない。取得失敗/古いStateはDegraded/明示Errorとし、Plugin MissingやVersion差があってもOpaque Bytesを保全する。

Stable TargetId→Plugin-local ParamId Mappingは非RT検証。旧Plan/Transport/Plugin Generationに紐付くEventを新しいInstanceへ誤配送しない。CLAP/VST3などのTransportへはPPQN固定960を仮定せず、Canonical Beatから規格固有精度で変換する。

## 9. Realtime / Offline / Recording

| Area | Live | Offline | Recording |
| --- | --- | --- | --- |
| Time/Tempo | 同じPrepared Meaning | 同じ意味論 | Timestamp + Placement Mapping |
| Driver | Device Clock | Offline Frame Driver | Capture Device |
| MIDI/Automation | Bounded RT Event | 同じEvaluation & Order | Raw MIDI + 明示Automation Arm |
| Plugin | Live Instance | 独立Instance + State Restore | Raw Input CaptureとMonitor分離 |
| Failure | Last-good/安全なFallback | Fail/Retry | Fail-stop/Verified Raw Range |

固定Test Nodeによる比較は同一Frame/入力/条件で行う。全PluginのBit-perfect Offline再現性は保証しない。Graph PDCとADC/DACの録音配置補正は分離。Device Clock断絶とLoop Wrapは別のFault/Discontinuity。

## 10. Diagnostics / Provenance

InspectorはConfigured、Reference/Resolved、Submitted/Applied、Plugin-reported、Observed、Unavailable/Staleを区別。可能なところでProjectRevision、TempoRevision、PlanGeneration、TransportGeneration、ClockEpoch、Beat/Frame、Source Event Id、Target Idを関連付ける。Source→Clip/Take→Span→Runtime Event→Plugin Target→Probeの観測を目指す。全PCM/全Eventの常時保存は必須にせず、Diagnostic Queueは録音Writer/NoteSafetyとは別のLoss Policyを持つ。

## 11. SynvelisとRuviformの責務

| Capability | Synvelis | Ruviform |
| --- | --- | --- |
| Project Beat/PPQN Setting、Clip/Snap/Undo | Own | Generic Beat/Ppqn/Conversion |
| Editable TempoMap/Transport UI Policy | Own | Immutable Time Conversion/Command/Applied Receipt |
| MIDI Source、Take/Comp/Editor | Own | Input/Timed Event/Clock/Plugin Delivery |
| Automation Write/Touch/Latch | Own | Bounded Control/Event Delivery |
| Plugin Instance in Project | Stable Id / Opaque State/Mapping | Host/Endpoint/Adapter/State I/O |
| Audio Device/Offline Integration | DAW monitoring/render policy | Clock/RT Device/Offline Driver primitive |
| Signal Flow/Inspector semantics | Own | Timestamped generic Diagnostic/Probe |

Ruviformの現行crate構造を固定せず、reuse/extend/extract/replace/deprecateを比較して移行。ただしVideo/CLI等の他Consumer、RT/NonRT依存方向、既存Accepted ADR、API互換性を検証する。

## 12. TT-A〜Dと受入テスト TT-001〜TT-024

| ID | Input | Expected |
| --- | --- | --- |
| TT-001 | Project PPQN 960/1000/1920 | Beat Position不変、Tick文脈維持 |
| TT-002 | Beat = 1/7 | 正本に無断丸めなし |
| TT-003 | MIDI Source Division 480/1000 | Native Tick/Timebase保持 |
| TT-004 | Playback中Tempo変更 | 新旧TempoRevisionを混在させない |
| TT-005 | Negative Pre-roll | 正確・有界な変換 |
| TT-006 | 256 FrameでLoop at 100 | 2 Span、各Plugin Transportが正しい |
| TT-007 | 複数周Loop | EventのOccurrence区別、予期しない二重配送なし |
| TT-008 | Seek pending | Requested/Applied FrameとGeneration区別 |
| TT-009 | Device Restart | ClockEpoch更新、Project位置を勝手に変更しない |
| TT-010 | Same-Frame Note On/Off | Stable Causal Order |
| TT-011 | Plugin Replace後Old Plan Event | 旧Target Eventを誤配送しない |
| TT-012 | Event Budget Overflow | Error/Fault/分類済Loss Policy、NoteOff安全化 |
| TT-013 | MIDI overlapping same-pitch Note | Note Ledgerと限界を検証 |
| TT-014 | Sustain CC64 + Seek/Loop | Release/Reset/Chase |
| TT-015 | Automation Off + Modulation ON | Base復帰、Modulation維持 |
| TT-016 | Read/Touch/Latch/Write | Arm/Record/Gestureの意味を#6と整合 |
| TT-017 | Gesture中Inspector Follow変更 | Targetが移動しない |
| TT-018 | Recording中LFO | Referenceのみ記録、二重Modulationなし |
| TT-019 | Plugin Latency/State Failure | Last-good/Degraded、RT非Block |
| TT-020 | Processing中SaveState | Consistency Levelと成否を正しく表示 |
| TT-021 | CLAP/VST3 / Variable PPQN | Transport位置/丸めを確認 |
| TT-022 | Realtime vs Offline Fixture | 同条件でイベントFrame/DSP結果一致 |
| TT-023 | Project Loop下Raw MIDI Recording | Session/Source Timestamp不変 |
| TT-024 | Event/Transport Probe | Source→Plan→Targetの時刻/世代/信頼度を追跡 |

**PoC:** TT-A=Beat/Tempo/Transport/Applied Receipt、TT-B=Timed MIDI/NoteLedger、TT-C=Automation/Control、TT-D=Plugin State/Offline/Trace。

## 13. Approved architecture decisions (14)

1. Clock/Epoch、Engine Frame、Transport/Tempo/Plan Revisionを分離。
2. PPQN非依存Beat/Native Anchorと最終Frame丸め（#19継承）。
3. Transport Command/Prepare/Applied Receipt。
4. Half-open Transport Span、Plugin Context整合。
5. Generation/Target/Stable Source Orderを持つTimed Event。
6. MIDI/Parameter/Transport/ProbeごとのLoss/Fault Policy。
7. Native MIDI Timebase/Raw Timestampを非破壊保持（#19継承）。
8. Note Ledger/Release/Reset/Controller Chase。
9. Plugin Control PrecisionとHost/Plugin値区別。
10. Automation Write/UndoをRT Deliveryから分離（#6継承）。
11. Plugin State Capture整合LevelとMissing State保全。
12. Single RT ownerとPlan/Target/Plugin Generationの照合。
13. Live/Offline/Recordingの同一時間意味論、異なるDriver/Failure。
14. Ruviform汎用責務 / Synvelis DAW意味論分離。

**未確定:** Beatの実型/整数幅とPPQN上限、Tempo Ramp、Max Span/Event Budget、Plugin Adapterごとの正確なState Capture/Precision、MIDI1同音高重なりとCC64復帰、Touch Return Time、Sample-accurate対応、具体crateと移行順。いずれもPoCで採否を決め、既存コードが実装済みであるとは報告しない。

**Next integrated topic:** Media / Import / Export / Offline Render / UI / Diagnostics横断統合。

## 14. 横断監査によるTransport Span / Plugin Process互換契約（2026-11-01）

**位置付け:** v0.5の「Transport Contextの変更点で論理Spanを分割する」目標は維持する。ただし、論理SpanとPluginの物理`process`呼出しは同義ではない。既存[Ruviform Plugin ProcessConfig](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-plugin-core/src/processor.rs)はActivated `min_frames ≤ frames ≤ max_frames`、[PluginEndpoint](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-plugin-host/src/endpoint.rs)には経路別のBlock Gateがある。[Ruviform #190](https://github.com/scarecrow173/ruviform/issues/190)でPoC追跡。

### 14.1 Frame座標の規約

- **Canonical Event Position:** 非RT Compiled Eventは`EngineFrame`絶対位置を持つ（具体Rust型名はPoC）。Loop反復は`LoopOccurrence`でSource Eventの識別と区別。
- **Callback:** Driverが`[callback_start, callback_start + N)`を提供。Callback-relative offset = event_engine_frame − callback_start。
- **Logical Transport Span:** Context不連続で`[span_start, span_end)`へ分割。Span-relative offset = event_engine_frame − span_start。
- **Physical Plugin Process Call:** 実際にAdapterへ渡す`[process_start, process_end)`。**Plugin EventListの`frame_offset`はevent_engine_frame − process_start**。CLAP `clap_event_header.time`もProcess Call開始相対。Span/Callback Offsetをそのまま転送しない。
- 全て半開区間。境界`event_engine_frame == span_end`/ `process_end`は次区間で1回だけ配送し、Eventの因果依存・同一Frame安定順序を維持。0-frame flushは専用制御境界で検証し、通常Audio Sample区間と混同しない。
- 例: Callback [0,256)、Loop at 100、Event at 120ではCallback Offset=120、後半Span/Process [100,256)のOffset=20。Event at 100は次Span先頭のOffset 0。

### 14.2 Physical Processの能力判定

1. **Prepared PlanのPreflight（非RT）:** 既存Plugin/Adapterのmin/max Block、最大分割数、L0/L1有効Block契約、Transport Contextの更新方式、Stateful DSP/PDC/Tail、時刻精度を確認。各物理Process Chunkは契約に従う。必要な能力を確認できないPluginは「対応済み」と扱わない。
2. **許容Block分割:** 各物理ChunkがActivated `min_frames ≤ frames ≤ max_frames`を満たし、対応するTransport Snapshot/Stateful時間連続性を正確に保持できるならSpanに沿って呼び出してよい。
3. **単一Block内のSample-accurate Transport Event:** 規格が可能にする場合は分割せず配送可能。CLAPは`CLAP_EVENT_TRANSPORT`をBlock内のsample offsetで提供できるが、**対象Pluginの実対応、Ruviform Adapter実装、イベント順序が検証された場合に限る**。VST3やその他規格にも一律に同じ保証を適用しない。
4. **どちらも不可能:** **正確なLoop/Seek/Tempo境界を処理できたと偽装しない**。新しいTransport/Graph/Plugin要求は非RT Preflightで`UnsupportedExactTransport`等の明示拒否/診断とし、適用前ならLast-good状態を維持する。Liveで明示的な精度劣化Policyをユーザーが選ぶ場合のみ量子化/延期を認め、`AppliedReceipt`には**実際の適用Engine Frame・精度低下**を記録する。Offline Exact Renderは無断量子化しない。RT内部で制約違反の短いBlockを無理に送らない。
5. **動的な境界増加:** LoopやTempo変更を後から加える場合も、Active PlanのCapabilityを再評価。既存連続処理中のPluginへ未検証のContextを差し込まず、適切なPlan/Transport GenerationでPrepare/Applyする。RTでCapacity超過や未対応が判明した場合は定義済みFault、Last-good/Fallback、Observationを使用し、不整合なSilent Successを禁止。

### 14.3 受入テスト追加

| ID | Fixture | Expected |
| --- | --- | --- |
| TT-025 | Callback 256 / Wrap at100 / plugin min_frames=1 | 100+156の適法物理分割、Transport Context正確 |
| TT-026 | Callback 256 / Wrap at100 / plugin min_frames=128 | 不正な100-Frame呼出しなし。検証済Block内Event方式がなければPreflight Rejectまたは明示User-approved Degraded Policy |
| TT-027 | CLAP in-block Transport Event / no proven support | Adapter/PluginごとのCapabilityと検証。規格に存在するだけでSample-accurate保証しない |
| TT-028 | Callback event at100、120、256 | 境界Event重複なし、ProcessOffset=0/20、256は次Callbackの0 |
| TT-029 | L0/L1・Zero-frame flush・多周Loop・Stateful Plugin | Endpoint Block Gate、時刻順、Note Ledger、PDC/Tail、PlanGeneration維持 |
| TT-030 | Offline exact vs explicit live degraded | Offlineは無断丸めせずFail、Liveはユーザーが選択した精度だけReceiptに表示 |

**範囲:** 新しい6受入ケースは既存TT-001〜TT-024に追加される監査補遺であり、他の合意済み要件を置換しない。使用するRust型名、Plugin Capability判定・Runtime Event配送、精度劣化UIはPoCで確定。
