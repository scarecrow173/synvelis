# Synvelis × Ruviform — Media / Import / Export / Offline Render / UI / Diagnostics 統合アーキテクチャ

- **Decision tracker:** [Synvelis #21](https://github.com/scarecrow173/synvelis/issues/21)

- **Version:** v0.6 / 2026-11-01
- **Status:** **Approved design baseline（合意済み・2026-11-01）**。API・PoC・Backend実装は未完了。
- **Prerequisites（合意済み）:** [Project / Asset / Persistence #17](https://github.com/scarecrow173/synvelis/issues/17)、[Playback / Mixer / Generic Graph #18](https://github.com/scarecrow173/synvelis/issues/18)、[Configurable PPQN #19](https://github.com/scarecrow173/synvelis/issues/19)、[Time / MIDI / Automation / Plugin #20](https://github.com/scarecrow173/synvelis/issues/20)
- **Existing UX decisions:** [Product #1](https://github.com/scarecrow173/synvelis/issues/1)、[Workspace #2](https://github.com/scarecrow173/synvelis/issues/2)、[Interaction #3](https://github.com/scarecrow173/synvelis/issues/3)、[Signal Flow #4](https://github.com/scarecrow173/synvelis/issues/4)、[Inspector #5](https://github.com/scarecrow173/synvelis/issues/5)、[Workflow #8](https://github.com/scarecrow173/synvelis/issues/8)、[Asset Browser #9](https://github.com/scarecrow173/synvelis/issues/9)
- **Ruviform issues:** [#181 Window](https://github.com/scarecrow173/ruviform/issues/181)、[#182 Docking](https://github.com/scarecrow173/ruviform/issues/182)、[#184 Typed Graph](https://github.com/scarecrow173/ruviform/issues/184)、[#185 Probe](https://github.com/scarecrow173/ruviform/issues/185)、[#186 Parameter](https://github.com/scarecrow173/ruviform/issues/186)、[#187 Device](https://github.com/scarecrow173/ruviform/issues/187)、[#188 MIDI](https://github.com/scarecrow173/ruviform/issues/188)、[#189 Musical Time](https://github.com/scarecrow173/ruviform/issues/189)、[#190 Transport](https://github.com/scarecrow173/ruviform/issues/190)

> 合意済みの設計事項とPoCで未確定の実装選択を区別する。**従来から採用済み**：Single Writer Project、Immutable Media/Source、Pinned Remote Copy、NonRT Prefetch、Generic Graph/PDC、共通Time/Event、Dockable Workspaceと選択式Probe。**v0.6で新たに採用**：Media/Render Job Orchestrator、Media Capability/Provenance契約、Output/Render Publish Policy、Export/Freeze/Bounceの厳密な分離、UI Job Survival、Diagnostics Evidence/Time Correlationと受入基準。APIの存在・Encoder/Muxerの実動作・Plugin Renderの決定性は未確認。

## 1. 現行実装の棚卸し

| Component | 現状（GitHubコードで確認） | 目標と不足 |
| --- | --- | --- |
| `ruviform-media-io` | [lib.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-media-io/src/lib.rs) にDemuxer、Decoder、Encoder、Muxerのtraitを分離。全体nonRT。Encoder/Muxerは実装Backendが確認されていない旨を明記 | Encode/Container Output/Capability/Finalizationの実Backendを検証 |
| `ruviform-io-symphonia` | [lib.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-io-symphonia/src/lib.rs) でDemux/Decodeのみ。`make_encoder`/`make_muxer`はUnsupported | 少なくとも実用的なPCM WAV Outputを成立させるBackend |
| `ruviform-exec` | [offline.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-exec/src/offline.rs) のOfflineDriver/JobとProgress/Cancel。Chunk並列の決定性条件は**独立Chunk**のみ | Stateful DSPやPluginの継続状態を区間ごとに無条件並列化しない |
| `ruviform-ui` | [lib.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-ui/src/lib.rs) にIntent/Service/Job、binary Data Registry、UI Bridgeが存在 | Synvelis Project Command、Job Status/diagnostics projectionを追加 |
| `ruviform-ui-webview` | WebView/Native Hostは現状単一Window。#181/#182は設計・PoC課題 | Multi-window/Docking、View State/Job ownership分離 |
| `ruviform-ui::UiRuntime` | [runtime.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-ui/src/runtime.rs) でCloseRequested時にrunning jobのCancelを要求 | **長時間Render Jobの寿命をWindow/単一UiRuntimeに固定しない**場合のOrchestrator境界 |
| `ruviform-ui::DataRegistry` | [data.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-ui/src/data.rs) の`rvf://data`有界Binary Pull（既定max chunk 16 MiB） | 波形/FFT/サムネイルをJSON PushせずBlob/Rangeで配信 |
| `ruviform-audio` | [graph.rs](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-audio/src/graph.rs) はOrdered AudioNode Graph。Typed Graph/PDCは#184 | Prepared Audio GraphとSource Resolver/Offline Driver連携 |

## 2. End-to-end Architecture

~~~mermaid
flowchart TB
    CAT["Library Providers / Catalog"]
    META["Catalog Index + Preview Cache"]
    IMP["Import Job: Fetch/Verify/Materialize"]
    PROJECT["Synvelis Project Controller / Canonical Snapshot"]
    ASSET["Verified Immutable Project Media Store"]
    LOWER["Semantic Lowering / Media Resolver"]
    PLAN["Ruviform Prepared Graph / Source + Time + Events"]
    RT["Realtime Driver + Prefetch"]
    RENDER["Offline Render Orchestrator"]
    SINK["Encode/Mux/Staging/Publish"]
    DIAG["Probes / Telemetry / Provenance"]
    UI["Workspace / Browser / Inspector / Signal Flow / Jobs"]
    CAT --> META --> UI
    CAT --> IMP --> ASSET
    IMP -->|"Commit command"| PROJECT
    PROJECT --> LOWER
    ASSET --> LOWER --> PLAN
    PLAN --> RT
    PLAN --> RENDER --> SINK
    RT --> DIAG
    RENDER --> DIAG
    DIAG --> UI
    PROJECT --> UI
    SINK -. "Explicit Bounce/Freeze/Import Commit" .-> PROJECT
~~~

**3 authoritative domains:** Canonical Project Authoring、Verified Immutable Media Bytes、Active Runtime/Job Executionは別。Catalog Index/Preview/Peak/Thumbnail/Probe dataはDerived/CacheでありProjectの正本ではない。

**1 UI contract:** Create/通常DAW制作の中心を維持し、Inspector/Signal Flow/History/Diagnosticsを必要に応じて開く。デバッググラフを通らないと音楽制作できない構造にしない。

## 3. Media Capability and Format Model

### 3.1 形式判定

- Container（WAV、FLAC等）とCodec（PCM、FLAC等）とPCM Layout（Interleaved/Planar、Sample Format、Channel Layout、Sample Rate）は別の概念。
- Capabilityは**Probe可能/Metadata取得可能/Demux可能/Decode可能/Encode可能/Mux可能/Random Seek可能/Preview可能**を分ける。入力可能な形式と出力可能な形式が同じとは仮定しない。
- `Declared`（コンテナ申告）、`Decoded/Verified`（実測）、`Inferred`（解析推定）、`Unavailable`を区別。未知Sample Rate/Duration/BPM/Keyを確定値で埋めない。
- Native PCM/Original Fileは破壊しない。f64 Native Decodeとf32 Playback/Renderが別パスの場合、変換履歴を保持。Import/Proxy/RenderのSRC、Channel Map、Scale、Clipping/Ditherは明示する。
- DecoderのSeekは必要に応じて指定位置より手前からのDecode/Warm-upが要る。Codec/Container Seek Accuracyとサンプル精度Trimの違いを宣言する。
- 形式対応一覧はBackend Capabilityと実際のテストに基づいて更新。拡張子だけの判定は避ける。

### 3.2 Media Resolver

`SourceId → Source Version → AssetId / ContentRef → Verified Blob → Decoder/Reader → Prefetch → Engine Frame`という解決経路を採る。

概念インターフェイス（**未実装**）:

~~~text
MediaResolveRequest { source_id, source_version, project_revision, desired_format, region, playback_policy }
MediaResolveResult { canonical_content_ref, native_format, transform, availability, integrity, provenance }
~~~

- Sourceに対応する元Blobが欠落した場合はMissing。Decoded Frameを偽造してVerifiedと扱わない。
- Live Playbackは非RT Reader/Prefetch→有界RT Ring/Slice。Seek/Loop/複数Clip/Rate/Stretchを考慮して新しいCache/Future Frameの世代を区別。
- Sourceが同じBlobを共有していてもAssetId/SourceId/Provenanceは別。Blob dedupは論理IDの統合を意味しない。
- Asset/Sourceが更新された際に旧PrefetchのBufferを新世代へ混入させない。

## 4. Library / Catalog / Preview / Import

### 4.1 Remote-aware Asset Library（既存#9の合意を継承）

- Provider: Local、Git（Hosted API vs Generic Git差）、SVN、P4、SMB/NAS、WebDAV、S3互換、Project Assets。すべてのConnector/Providerが同じSearch、Revision、Preview、Checksumを持つとは仮定しない。
- Metadata-first Page/Filter/Sort/Search。`IndexCoverage`、`IndexedAt`、`ProviderRevision`、`Offline/Stale/Unavailable`、`ResultCompleteness`を表示し、検索していないRepositoryの未検出を「存在しない」にしない。
- Catalog正本はProvider Server。Cached Metadataは取得当時のObservation。ProviderのPath表示とByte IdentityとPinned Revisionを区別する。
- Previewは**明示要求**でRange/Proxy/部分Decodeできれば少量取得。Codecによっては全Byte取得が必要。Preview CacheはEvict可能で正式Project Mediaとは別。
- Provider Token/CredentialはProject/JobManifest/Diagnostics/Log/URLに含めない。Preview/Import範囲の権限・Trust/Provenanceは表示可能な分だけ提示。

### 4.2 Import Job State Machine（新規詳細提案）

~~~mermaid
flowchart LR
    S["Selected Pinned Revision"] --> F["Fetching → Staging"]
    F --> V["Verify Bytes/Format/Identity"]
    V --> M["Durable Immutable Media"]
    M --> C["Project Command Commit"]
    C --> D["Registered Asset/Source/Clip"]
    F --> E["Error/Cancel"]
    V --> E
    M --> O["Recoverable Orphan"]
    C --> O
~~~

- JobはSnapshotやProviderの**固定Revision/Version**を保持し、mutableなBranch/Timestampだけを暗黙Pinとしない。
- Jobは期待Project Revision/Target Track・Lane/Placement Intentを記録。長時間Download後にProjectが変更された場合、Conflict/Rebase/再確認を行い、別Trackに勝手にClipを置かない。
- Blob Verify/耐久化とProject Controller Commitは別。Commit失敗時のVerified OrphanはRecoverableとして保護。
- Drag & DropでRemote Assetを配置する場合、Pending Ghost/Jobとして明示表示。未検証のAudio Source/ClipをProject正本として先行公開しない。
- Cancel/Retryは段階ごとに意味を変える。正式Commit後のUndoはProject変更として扱い、物理Mediaを即削除しない。
- Importは原則Copy into Project。External Referenceは明示選択の高度モードで、Offline/Missingと既存Snapshotの保護を設計する。

## 5. Unified Non-RT Job Orchestrator（新規提案）

実行は汎用`OfflineJob`/`UiRuntime`を再利用可能だが、**DAW Jobの結果・所有・依存関係はSynvelisのApplication/Project Session側で管理する**。

| Job Type | Inputs | Outputs | Publish / Commit |
| --- | --- | --- | --- |
| Catalog/Analyze/Preview | Provider URI/Revision, demand | Metadata/Preview/Peaks | Cache only |
| Import | Pinned Provider Revision, Target Intent | Verified Blob/Asset/Source | Project Command Commit |
| Recording Finalize | Session/Writer Journal | Verified Raw Media | Idempotent Project Commit (#12/#17) |
| Freeze | Input/Plugin/Plan Snapshot | Derived Audio + preserved original chain | Atomic Project change after verification |
| Bounce/Render-in-Place | Chosen Render Point/Range | Derived Audio/Source | Explicit Project Command; originals retained |
| Master/Stem Export | Frozen Render Spec | External Files + Receipt | **No implicit Project Mutation** |
| Analysis/Diagnostics | Source/Graph/Probe scope | Derived evidence/caches | No Project Mutation |

Job states: `Queued → Preflight → Prepared → Running → Draining/Finalizing → Validating → Publishing → Completed` with `Cancelling`, `Cancelled`, `Failed`, `Recoverable`, `Degraded` as explicit outcomes. Cancel acceptance is not same as cancellation completion; Finalize/Publish can be noninterruptible short phases. Failed writes never yield a `Completed` receipt.

### 5.1 Job Identity / Snapshot

Each Job has stable `JobId`, `JobKind`, Owner/ProjectSessionRef (or app-global), InputSnapshot/Blob/Plugin State references, Capabilities, Resolved Output Plan, Output Temp target, Progress/Phase, Cancellation/Failure policy, Diagnostic Evidence and immutable Result Receipt. Progress may be approximate/not monotonic across replan phases; Phase progress and terminal state are separate.

**Important:** 現行`UiRuntime`はWindowの`CloseRequested`でJob Cancelを要求する。長時間RenderをWindow閉鎖で止めない方針を採る場合、Job OwnerはWindow/単一UiRuntimeから分離する。Windowに紐付く短命なPreviewと、アプリ/Project Sessionに紐付くRender/Import/Recording Finalizeを混同しない。アプリ終了時のCancel/Wait/Recoverableは明示Policy。バックグラウンド永続Jobやアプリ終了後の継続を無条件保証しない。

### 5.2 Priority / QoS

RT Audio/Recording Capture > Capture Writer安全性 > Playback Prefetch > nonRT Active Plan Prepare > Import/Export/Analysis/Preview（実機負荷測定により微調整）。UI/JobsがRT Ownerへ直接Blocking I/Oや重いハッシュ計算を呼び出さない。単一ディスク/CPU/GPU負荷をJob Admission/Concurrency/Backpressureで調整。数値ThresholdはPoC保留。

## 6. Offline Render Pipeline and Render Contract

~~~mermaid
flowchart TD
    A["Render Request: Master/Stem/Freeze/Bounce"]
    B["Preflight: Snapshot/Assets/Plugins/Range/Capabilities"]
    C["Freeze Input Manifest & State Evidence"]
    D["Prepare Independent Offline Graph"]
    E["Warm-up / Preroll / PDC / Tail"]
    F["Evaluate Time, MIDI, Automation, DSP"]
    G["Sample Format / SRC / Channel Map / Dither"]
    H["Encode / Mux / Drain / Finalize"]
    I["Validate Output / Durable Publish"]
    J["Receipt / Optional Project Commit"]
    A --> B --> C --> D --> E --> F --> G --> H --> I --> J
~~~

### 6.1 Render Source Snapshot（新規）

- `RenderSpec`はProject Snapshot Revision、TempoMap Revision、Asset/Source Versions、Plugin Instance/State Capture Evidence、Graph/Automation/Transport Configuration、Range Anchor、Channel/Output Format、Render Mode、Tail/Preroll/Latency Policyを固定する。
- **Project Live≠Saved≠Active Plan≠Render Source Snapshot**。通常ExportはLive Projectの指定Revisionを非RTで固定してよいが、Save済みVersion限定などを選択できる。Render中にProjectを編集しても実行中Specを自動変更しない。
- Plugin State CaptureがBest-effort/Unsupportedの場合、「Renderできた」「完全再現できる」の保証を区別。欠落Plugin/Asset、未対応Output Format、Render不可能なExternal Sidechain/Realtime-only Plugin等はPreflightで明示Rejectまたは明示PolicyのDegraded。
- Source Missingで音声をゼロ埋めして成功扱いすることは既定で禁止。ユーザーが明示的にsilence substituteを選択する場合はReceiptに記録。

### 6.2 Stateful DSP / Parallel Offlineの制限

Ruviform`OfflineDriver::run_parallel`の結果一致は**Chunkが相互独立**である前提。Delay/Reverb/Compressor/Instrumentなどの状態を持つDSPに対し、任意の時間Rangeを分割して無条件に並列処理してはいけない。

初期実装は**時間方向Sequential Execution** + 各Node/Channel等の明示可能な独立単位に限った並列化（または単一Thread）を推奨。状態Checkpointの決定性、Tail/Overlap、境界一致、独立Plugin CloneのCapabilityが検証された場合のみTemporal Parallel Renderへ拡張する。

### 6.3 Range / Preroll / PDC / Tail

- Render RangeはBeat/Media/Clip-localのAnchorを明示し、非RTでEvent/Frameに変換。範囲は半開で、同Frame Event二重評価なし。
- Lookbehindが必要なPlugin/Clip Stretch/Stateful InstrumentではPreroll/Warm-upを準備。Render先頭だけ無条件のゼロ状態で開始して正しい結果と断定しない。
- Graph PDCとExport上の補償・波形Leading Silenceは分離。Master/Stemの時間Alignmentは共通Frame原点と実Output Tapに基づく。
- TailはFixed Time、Until Silence(閾値/保持時間/最大長)、User Defined、Unsupported/Unknown等のPolicy候補を持つ。無限TailはMax Durationや明示停止を要求。無断Trim、Length不一致を成功扱いしない。
- Loopの「反復して書き出し」と「単一Regionのみ書き出し」、Tail跨ぎ、停止後のMIDI Release/ControllerはRender Specで区別。

### 6.4 Output/Encode correctness

- Format negotiationでContainer/Codec/Sample Rate/Bit Depth/Channels/Metadata/Seekabilityを検証。
- PCM→整数PCMへの変換時はClipping/Level、Rounding、Dither/Noise Shape、Channel Map、SRC、Endianness、Metadataを規則化。Ditherは最終量子化の単一段階でのみ適用（多段の意図しないDither禁止）。
- L/R/多ChannelとMaster/Control Room、Pre/Post Fader、Pre/Post FXのExport Tapを明示し、Monitor-only Pluginを暗黙混入させない。
- EncoderのDrain→MuxerのFinalize→Close/Sync→Output検証→Output Path Publishを分離。Close/Dropのbest-effortだけで完成とは断定しない。
- File OutputをStaging/Temporaryへ書き、検証後にDestinationへ安全Publish。同名Destinationは事前にOverwrite Policyと既存File保護/Backupを確認。Remote FS、異ボリューム、部分FinalizationはCapability/Recoveryとして管理。
- 初期対応Formatを「WAV PCMのみ」等で狭く検証することは許容。ただし**現時点でBackend未実装**なので、MVP要件を満たすOutput Adapterの実装は計画へ含める。

## 7. Master / Stem / Bounce / Freeze / Exportを区別

| Action | Render Point | Destination | Project Mutation | Source Lifetime |
| --- | --- | --- | --- | --- |
| Master Export | Master Render Tap | User-chosen external file | 原則なし | Project素材は不変 |
| Stem Export | Selected tracks/buses/taps/solo isolation policy | External file set | 原則なし | Project素材は不変 |
| Bounce in Place | Region/Track/Bus output | Verified Project Media | Explicit Project Command | Original Source/Clipを保持・非破壊Lineage |
| Freeze | Plugin/Instrument processing result | Derived Source + Freeze Descriptor | Freeze/Unfreeze Command | Original Plugin Chain/Automation/State/Referenceを保持 |
| Render-in-Place | Selection + chosen Render Point | Project Media and editable Clip | Explicit Command | 元のVersionを履歴・Undo用に保護 |
| Export Selected Clips | Chosen Clip-local Time/Format | External files | 原則なし | Source Native/Mediaは維持 |

- Stemの数、Name、Bus/FX共有処理、Post-Master/Pre-Master、Solo/Mute、Sidechainの扱いは**明示的なRender Recipe**として固定。MasterとStemの合計が同一になるとは、非線形Bus/Shared Master Effectsがある場合に保証できない。
- Freeze中のPlugin/ParameterのUI表示は「凍結状態」を明示。変更時にはUnfreezeまたはRe-freezeを要求し、元Stateを勝手に捨てない。
- Freeze/BounceでVerified Blobが完成してもProjectの参照Commit成功とは別。失敗時は旧ProjectとLive Playbackを維持し、Orphan/Retryを保護。
- ExportファイルはProject Assetではない。明示的な「Import rendered result」等があるときのみProjectへCommit。

## 8. UI / Workspace / Job Lifecycle Integration

### 8.1 View Layer

既存合意に従いArrangement、Mixer、Audio/MIDI Editor、Browser/Library、Signal Flow、Inspector、Transport、Export/Job Monitor、Diagnostic ViewはDockable View Instanceとし、同じProjectSessionの正本を参照する。各WindowのView/Inspection/Selection StateとProject Command Targetを分離する。Ruviform #181/#182の実装状況を過大評価しない。

- BrowserはRemote Metadata、Revision/Availability/Cache/Import Statusを表示。Drag → Pending/Preflight → Verified Import/Project Commitとなり、未検証Clipを確定表示しない。
- Export DialogはRange/Format/Tail/Stem/Plugin State Capability/Output Destination/Estimated DurationをPreflightで表示し、明示StartでJobを開始。
- Job PanelはQueued/Running/Cancelling/Finalizing/Publishing/Recoverable/Failed/CompletedとScope/Revision/Elapsed/Progressを区別。Cancel/Retry/Reveal/Review Errorを提供。Window Closeで全Project Jobを無条件停止しない。
- InspectorはFollow/Explore/Pinned、TargetごとのProviderとPort Probe/Asset Integrity/Output Metadata/Render Receipt/Plugin Missingを統一的に表示。複数Inspectorで同じProbeを二重生成しない。
- Signal FlowはProject Semantic Routingを表示し、Graph PlanのNode/Port→Semantic ID MappingとRevision/Latency Evidenceで実際のObservedとの違いを表示。表示フィルタはRouting Mutationではない。
- History/VCS読み取りはLive/Active/Working Treeに影響させない。Project Save、Render、VCS Commitは独立した操作。

### 8.2 UI data movement

UI Event/IPCは小さいControl/State/Metadataに使用。大量Waveform Peaks、Spectrum、Spectrogram、PCM Previewは`rvf://data`相当の**有界Pull**や別の描画DataSourceで配布。Render/Probe/Preview JobがUI描画をブロックしない。必要なSelection/Revision/Source Version/Graph Generationを各DataSource Keyに含め、古いデータの誤表示を避ける。

### 8.3 UI safety

- Focus/IME/Plugin GUIに対してDAW Shortcutsを奪わない。Job Cancel対象とTrack/Clip編集TargetをWindow移動時に変更しない。
- OS Credential/Pinned Provider Access Token、個人Path、秘密Source URIをJob/Diagnostic UIで不用意に展開しない。
- Browserの未信頼Remote Filename/MetadataはTextとして扱い、HTML/Script/Path/Archive Traversal/Local Scheme Injectionを拒否。WebView Scheme側も許可済みDataSourceのみアクセス可能。
- Window Crash/Monitor DisconnectはProject/録音/Import/ExportのAuthoritative Statusを変えず、Viewを復元してJobへ再接続できる設計を目指す。アプリプロセス自体が終了した後の実Job継続は別のCapability。

## 9. Diagnostics / Observability / Explanation

### 9.1 4レベルの真実

| Level | Example | Guarantees |
| --- | --- | --- |
| Configured | ProjectにSendが存在、Bypass設定、Automation Curve | Project Snapshotの設定上の事実 |
| Prepared / Applied | Plan GenerationでSidechain Portを解決、PDC=384 framesと計画 | Compiler/Execution Ackに基づく事実 |
| Observed | ProbeでGain Reduction / Peak / PCMを時刻付で測定 | 計測点/範囲/Clock/欠落に限定した事実 |
| Inferred / Unavailable | 「原因候補」「内部Plugin Stateは不明」 | EvidenceとConfidenceを示す。推定≠確定因果 |

Source → Clip/Take/Asset → Project Routing/Automation → Lowered Graph Node/Port → Active Plan Generation → Probe/Output Result の有向関係をInspector/Signal Flow/Diagnosticsが表示可能にする。

### 9.2 Observation Envelope

~~~text
ObservationEnvelope (conceptual, not implemented):
  observation_id, time_range, clock_domain/epoch, engine_frame,
  project_revision, tempo_revision, plan_generation,
  source_id / track_id / clip_id / plugin_instance_id / node_id / port_id,
  measurement_kind / unit / channel_layout / sampling_policy,
  validity, drop_count, provenance, confidence
~~~

- ProbeはOff(既定) / Monitor / Capture。全論理NodeにProbe能力の宣言を持たせるが、全Nodeの全PCMを常時保存しない。存在しないBufferや非公開Plugin内部を観測できると偽らない。
- ProbeのSession/ControllerはInspector Viewのライフサイクルと独立。2つのInspectorが同じProbeを開いてもCaptureを重複生成しない。
- Signal同士の比較は同じ実行Frame/Clock/Graph GenerationとPDC/経路Latencyを考慮し、時間が揃わない測定値をそのまま引き算しない。
- Diagnostics Budget超過はCapture Loss/Unavailableとして可視化。**録音WriterのFail-stop要求をProbeにも、ProbeのDrop許容を録音にも適用しない**。
- ある点で初めて異常が観測されたことと、原因Nodeが特定されたことは別。原因候補は測定・コンパイルEvidenceに基づいて表示する。

### 9.3 Example Investigation: 意図しない音量低下

1. Mixer上でBassが小さく聞こえる。InspectorでSource、Fader、Send、Compressor/Sidechain、Automationの設定を確認。
2. Configured GraphとActive Planを比較し、現在のRevisions/Latency/Availabilityを確認。
3. 必要なInput/Output/Sidechainの標準Probeを明示Onし、同じFrame範囲でPeak/RMSやGain Reduction（取得可能な場合）を観測。
4. どこから差分が生じたかを限定的に報告。PluginがGain Reductionを報告しないならUnknownのまま、音声差分の原因を断定しない。
5. Refinementとして明示Parameter/Connection Commandを発行。Graph再Compile/RT Applied Receiptを確認。Inspectorの探索操作自体はProject Mutationしない。

## 10. Security / Integrity / Failure Matrix

| Failure | Detection | Safe result |
| --- | --- | --- |
| Remote Provider Offline/Stale | Provider availability + cached metadata timestamp | BrowserはStale、既存Imported Projectはローカル再生 |
| Import途中切断/Hash mismatch | Staging Hash/Length/Revision verification | 未検証MediaをAsset/ClipとしてCommitしない |
| Project Revision mismatch during import | ExpectedRevision/Target ID validation | Rebase/Review/Reject、誤Trackへ配置しない |
| Decoder/Codec Unsupported | Negotiated capability/ErrorKind | Missing/Unsupported UI、Proxy/Explicit Convertが可能なら提示 |
| Playback Prefetch Underrun | Source/Frame/Plan evidence | Defined safe output + Underrun報告、正常再生と偽装しない |
| Render Plugin Missing/Realtime-only | Preflight Capability | 原則Reject、Explicit Degraded policy時のみ続行 |
| Encoder/Muxer unavailable | Capability query | Export開始を拒否、未完成ファイルをSuccessとしない |
| Encoder/Output Finalize Error | Drain/Finalize/Size/Decode check | 旧出力保護、Staging/Recoverableを残す |
| Render cancelled | Cooperative Cancel + Result | PartialをCompletedとしてPublishしない |
| Output overwrite / disk full | Destination Policy/Free Space/Error | 既存ファイルを破壊しないよう事前保護/Recovery |
| UI Window close | Window vs Job ownership | Project/Render/Recordingを暗黙停止しない（app exit policy別） |
| Probe drop | Drop counter/Frame gap | 観測欠落を表示、原因推定を保留 |
| Asset/Plugin external changes | Pinned Snapshot/Generation/Version | Running Jobに勝手に反映しない |

## 11. 責務マトリクス

| Capability | Synvelis | Ruviform |
| --- | --- | --- |
| Catalog Provider/Remote Revision/VCS semantics | Library/Provider Adapter | Reusable generic I/O primitives where justified |
| Project AssetId/SourceId/Clip/Provenance/Retention | Own | No DAW Project Schema |
| Media Codec/Container/Native PCM | Import/Export policy | Generic Demux/Decode/Encode/Mux/Format capabilities |
| Playback Streaming | Source→Clip mapping/underrun UX | Generic Reader/Prefetch/Audio Graph capability |
| Offline Execution | Render Recipe/Job Orchestrator/Stem/Freeze/Bounce/Commit | Generic OfflineDriver, Execution Graph, DSP and Codec Sink |
| Job Ownership / UI | Project/Application Job Registry + View state | Generic nonRT Job/Cancellation/Service/Bounded Data Transfer |
| Multi-window/Docking | Workspace/Inspector/Browser/Signal Flow | Generic Host/View/Dock Layout ports (#181/#182) |
| Probe/Diagnostics | DAW semantic/provenance/causal explanation | Generic RT-safe observation/capture, clock/latency evidence (#185) |
| Safe Output Publish | Destination/Overwrite/Project commit policy | Generic Filesystem/Sink/Codec finalization primitives as needed |

Ruviformについては現在のcrate分割を絶対条件にせず、必要な汎用能力から再編を判断する。既存Approved ADR/APIと衝突する具体変更は別のADR/移行Issueとテストを要する。

## 12. Initial Vertical Slices / Priority

### MR-A — Local Media Playback & Export (P0)

ProjectにAudio Sourceを取り込み→Verified Blob/Clipとして登録→Track/Audio Graph再生→Master PCM WAV（実Output Backend必須）→一度Closeして再Openし比較。Recording後も同じMedia StoreへFinalize/Commit。VCS/Remote LibraryやProbe高度機能なしでも通常DAWが成立する。

### MR-B — Safe Remote Import & Asset Browser (P0 foundation, P1 providers)

Metadata-only Local/Mock Remote→Pinned Revision選択→Range Preview（可能なFormat）→非RT Fetch/Verify→Project Copy/Commit→Offline Playback。ProviderごとCapabilityを段階導入。Git/SVN/P4/SMB/S3の全Backend同時実装はP0要件としない。

### MR-C — Offline Stem/Bounce/Freeze (P1)

Render Snapshot/Plugin Clone/State Capture→Sequential Graph→Preroll/PDC/Tail→Stem/Master/Tap指定→Encode/Validate/Publish。Bounce/FreezeはDerived Project Mediaへ参照Commit/Undo。Master/Stemが必ず加算一致するなど非線形処理で不成立な保証をしない。

### MR-D — Workspace / Diagnostics / Trace (P1 foundation → P2 advanced)

Dockable Job Monitor/Browser/Inspector/Signal Flow、同一Project複数Window、標準ProbeのOff/Monitor/Capture、2地点の時刻整列比較、Provenance/PlanGeneration。Ruviform #181/#182/#185/既存UiRuntimeをPoCで統合。

## 13. Acceptance Matrix MR-001〜MR-028

| ID | Scenario | Expected |
| --- | --- | --- |
| MR-001 | Local WAV/PCM import | Verified BlobとStable Asset/Source/Clip ID |
| MR-002 | Native PCM differing sample rate/layout | 明示変換履歴、元Bytes不変 |
| MR-003 | Unsupported codec/container | Capability Error、誤判定なし |
| MR-004 | Same Content 2 Remote Origins | Blob dedup可能、AssetId/Provenance別 |
| MR-005 | Remote metadata-only paginated search | 全Byte同期なし、IndexCoverage表示 |
| MR-006 | Provider Offline/metadata stale | Stale明示、Imported Projectは再生 |
| MR-007 | Preview unsupported/partial fetch | Preview Capability/ErrorとCacheを分離 |
| MR-008 | Pinned Revision changed upstream | Project Imported Assetを黙って置換しない |
| MR-009 | Import Cancellation/Hash Mismatch | Project半端Commitなし、Recoverable Staging |
| MR-010 | Import完了時Project Target changed | ExpectedRevision/Target再検証、誤Clip配置なし |
| MR-011 | Prefetch underrun/seek | RT File I/Oなし、Frame/Plan損失診断 |
| MR-012 | Local Mix→PCM WAV output | 実BackendのEncode/Mux/Finalize後に有効なFile |
| MR-013 | Output Codec Unsupported | Preflightで拒否、Success報告なし |
| MR-014 | Export Range/Loop/Tempo | Half-open区間・イベントの重複欠落なし |
| MR-015 | Preroll/Tail/PDC | 音声先頭/末尾とStem Alignmentが仕様どおり |
| MR-016 | Stateful Reverb / Instrument Offline | Chunk parallel無断適用なし、State連続 |
| MR-017 | Stem Tap / Master Fx / Sidechain | 各Recipeを明示し、非線形合計一致を偽保証しない |
| MR-018 | Final Integer PCM / Dither / SRC | 変換回数・品質・メタ情報の検証 |
| MR-019 | Encoder drain / mux finalize failure | Failed/Recoverable、旧File保護、Incomplete非公開 |
| MR-020 | Export中Projectを編集 | Renderは固定Snapshotで完了、Live変更保持 |
| MR-021 | Plugin Missing / Best-effort State | 明示Preflight/Degraded、偽の再現性保証なし |
| MR-022 | Bounce/Freeze then Undo/Unfreeze | 元Source/Plugin/Automation保持、冪等Commit |
| MR-023 | Cancel / Window Close / App Exit | Job Owner別に定義済み停止/継続、Terminal State可視化 |
| MR-024 | Two docked Inspector/Probe Views | 同一Probeは二重起動しない、View State独立 |
| MR-025 | Probe Off/Monitor/Capture and Bypass | ProbeはDSPを変更しない。Loss/Unavailableを識別 |
| MR-026 | 2 Probes with PDC and Clock correlation | 同Frame/Graph Generationで比較、欠落はUnknown |
| MR-027 | Source→Track→Node→Parameter→Observation trace | Configured/Applied/Observed/Inferredを区別 |
| MR-028 | Invalid Metadata/Credentials/Path Traversal | Safe Reject、Secret Leak無し、Project Blob保護 |

## 14. 合意済みの14のアーキテクチャ判断

1. **Media capability/identity/provenance**をCodec/Container/Native PCM/Source/Asset別に管理し、UnknownとUnsupportedを保持する。
2. **Source Resolver**をProject Stable IDからVerified BlobとnonRT Reader/Prefetchへ結ぶ共有Portとする。
3. **Browser Catalog/Preview Cache**と正式Project Mediaを完全に分離し、Remote Metadata-first/固定Revision Copyを維持する。
4. **Import**をPinned Fetch→Stage→Verify→Durable Materialize→ExpectedRevision Project Commitの非RT Jobとし、Recoverable Orphanを保護する。
5. **DAW Job Orchestrator**でImport/Render/Bounce/Freeze/Analysis/Recoveryの状態・取消・Outcome/Receiptを統合し、Job OwnerはWindowから独立できる設計にする。
6. **Render Snapshot/Preflight**を独立した不変入力契約とし、Live/Saved/Active Planの差を説明する。
7. **Offline Render**は同じGraph/Time/Event意味論を使い、Plugin Instanceは別。Stateful Graphの時間方向並列化を根拠なく行わない。
8. **Render Range/Preroll/PDC/Tail**を明示し、Plugin State/Unknown Tail/External input等の再現性境界をCapabilityとして報告する。
9. **PCM Output Pipeline**のFormat/SRC/Channel/Dither/Encode/Drain/Mux/Validate/Atomic Publishを非RTで段階化し、失敗を成功と偽らない。
10. **Master/Stem/External Export**と、ProjectにCommitする**Bounce/Render-in-Place/Freeze**を別Recipeとし、元Source/FX Stateを保存する。
11. **UI/View/Job lifecycle**を分離し、Window Close/Monitor切替/Inspector操作がProjectや長時間Jobを意図せず変更しない。
12. **Large UI data**をBounded Binary Pullと世代付きProjectionで配信し、すべてをJSON Pushしない。
13. **Diagnostics Evidence/Provenance**をConfigured/Prepared/Applied/Observed/Inferred/Unavailableに分類し、時間・Graph/Project Revisionを付けた限定的な因果説明を可能にする。
14. **Ruviform generic / Synvelis DAW policy**という責務境界を維持し、出力Backend実装とUI/Docking/Probeの能力不足をPoCで補う。

## 15. 未確定と次の作業

- 実際のPCM WAV Writer Backend（RIFF/RF64等の対応範囲）、FLAC/MP3等のEncode優先度、許容Format/Bit Depth/Metadataと実機I/O。
- Tail Silence検出の数値閾値、最大長、Plugin Warm-up/Offline Native support、Stem isolationの既定Policy。
- Project Render SnapshotとPlugin State Captureの厳密同期/回復手順、Output overwrite durability、File/Network FS capability。
- Browser各Providerの具体API、認証UI、Security、Indexing、Remote Preview byte budget、Cache eviction。
- Window Close/App Exit時のJob Survival、再接続、UI WebView/Native GPU backend、Probe budget/Diagnostics payload。
- Performance Budget/Resource Admission、Audio RT vs nonRT Import/Render同時実行のOS別検証。
- 具体的Rust API/Crate placementとRuviform既存ADR/Consumersの移行計画。

**現時点:** v0.6の14項目はSynvelis #21で合意済み。GitHubの設計文書と関連Issueへ記録しているが、Ruviform Generic Media Output/Job/Observabilityの実装・PoC・各Backend能力は未確認。