# Synvelis-driven Ruviform Target Architecture v0.7.1

- **Version:** v0.7.1 / 2026-11-01
- **Status:** **Approved architecture baseline**. 具体API、新規crate、PoC、移行/実装・RT実機検証は未完了。
- **Decision / Tracking:** [Synvelis #22](https://github.com/scarecrow173/synvelis/issues/22) / [Ruviform #192](https://github.com/scarecrow173/ruviform/issues/192)
- **Ruviform technical target:** [Synvelis-driven Target Architecture and Migration](https://github.com/scarecrow173/ruviform/blob/main/docs/designs/synvelis-driven-target-architecture-and-migration.md)
- **Ruviform boundary ADR:** [Synvelis-driven Generic Execution Boundaries](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/synvelis-driven-generic-execution-boundaries.md)
- **Synvelis approved prerequisites:** [#17 Project/Asset/Persistence](https://github.com/scarecrow173/synvelis/issues/17) / [#18 Playback/Mixer/Graph](https://github.com/scarecrow173/synvelis/issues/18) / [#19 Musical Time](https://github.com/scarecrow173/synvelis/issues/19) / [#20 Transport/MIDI/Automation/Plugin](https://github.com/scarecrow173/synvelis/issues/20) / [#21 Media/Import/Export/Offline/UI/Diagnostics](https://github.com/scarecrow173/synvelis/issues/21)

> **設計正本の区別:** Synvelisの合意済みProject/DAW意味論は上記各Issue/設計書を正本とする。本書はそれらを実装可能にする**Ruviform汎用能力・責務境界・Target Migration**の正本であり、旧設計を無断に書き換えない。

## 1. Goal / Scope / Non-goals

目標は「汎用的な次世代DAW」としてのSynvelisを実現し、同時にRuviformがAudio、Video、Plugin Host、CLI、UI等の独立Consumerから再利用可能であること。現行28 crateを温存すること自体を目的とはしないが、既存の有効な基盤・公開API・Accepted ADRを理由なく廃止することも避ける。

**Ruviformの責務:** Time/Clock/RT、Typed Graph、Audio Buffer/PDC/Execution、Timed MIDI/Parameter、Plugin Host/Adapter、Generic Media Read/Write、Offline Driver、Generic UI/Probe/Viz。

**Synvelisの責務:** Canonical Project/Asset/Source/Track/Lane/Clip、Take/Comp、Automation Write/Undo/Manual gesture、Arrangement/Piano Roll/Mixer、Routingの制作用意味、Remote Library Provider、Import Commit/Render Recipe/Stem/Bounce/Freeze、Workspace/Signal Flow/Inspectorの製品意味。

ProvenanceやDiagnosticsはRuviformの測定値・Frame/Generation/Loss/Confidenceを材料とし、Synvelisが原因調査の解釈を加える。RT側へProject Undo、File I/O、UI Renderを持ち込まない。

## 2. 現行調査から確定した事実と修正

- Ruviform Workspaceは[28 crate](https://github.com/scarecrow173/ruviform/blob/main/crates/README.md)で構成され、基盤、Audio/Video、Media I/O、Plugin Host/Adapter、UI/Viz/CLIを含む。
- [plugin-host/Cargo.toml](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-plugin-host/Cargo.toml)では\`ruviform-audio\`は**dev-dependency**であり本番通常依存ではない。前回v0.7の「Plugin Hostが通常依存する」という表現はv0.7.1で訂正された。
- [PluginEndpoint::process_audio](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-plugin-host/src/endpoint.rs)は\`ruviform-audio-core\`等を用いる。Audio Engineの具象Typeを必須とするHost/Engine循環は**現状発生していない**。
- [Audio Graph](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-audio/src/graph.rs)は現時点で順序AudioNode PullのGraph。Typed Multi-Port Graph、Source/Sidechain/PDC Compilerは[追加要件 #184](https://github.com/scarecrow173/ruviform/issues/184)。
- [Audio PlanSwap](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-audio/src/plan.rs)は\`RwLock<Arc<GraphPlan>>\`/\`try_read\`/last-good/非RT retireで動作。同期方針は[Accepted ADR](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/audio-engine-snapshot-swap.md)で確定済み。新方式へ変更するにはPoCと後継ADRが必要。
- [MIDI/Param EventList](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-plugin-core/src/event.rs)はFrame Offset/Stable Order/Zero-frame Flushを持つが、現EventKindはParamValue中心。Timed MIDI/NoteLedger/Target Generationは未実装。
- [Musical Time後継ADR](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/configurable-ppqn-resolution-independent-musical-time.md)は**Accepted**。Ruviform実装は固定960からの移行前。
- [OfflineDriver](https://github.com/scarecrow173/ruviform/blob/main/crates/ruviform-exec/src/offline.rs)は独立Chunkのみ並列決定性の保証。Stateful Pluginの時間並列の安全性は導かない。
- [Audio/Video/Plugin](https://github.com/scarecrow173/ruviform)やNLE\`ruviform-edit\`は他Consumerの基盤として維持。DAWのTake/Comp/UndoをNLE Timelineへ強制統合しない。

## 3. 採用する12のアーキテクチャ判断

| # | Approved architecture direction |
| --- | --- |
| 1 | Synvelisの合意済み要件からRuviform Generic Capabilityを導出する |
| 2 | RuviformはAudio/Video/Plugin/CLI/UIに再利用可能、Synvelis専用化しない |
| 3 | Synvelis Project Semantic GraphとRuviform Graph/Execution/Mediaの境界を明示 |
| 4 | 非RT Typed Graph IR/ValidatorとAudio Buffer/PDC Compiler・Prepared Runtimeを分離 |
| 5 | Plugin HostとAudio Graphの接着Adapterを独立した**責務**とする。独立crate化はPoC |
| 6 | Immutable Plan/Schedule GenerationとStateful RT Processor Owner/Plugin Instance寿命を分離 |
| 7 | PPQN非依存Beat・可変PPQN default960、Clock/Transport/Timed Eventを汎用能力へ |
| 8 | Project Source/Asset IdentityとGeneric Reader/Decoder/Prefetchを分離 |
| 9 | Stateful-safe Offline Render、実Encoder/Muxer/Output Finalizationを実現 |
| 10 | Application/Project JobとWindow/View Jobの寿命を分離 |
| 11 | RuviformのProbe/EvidenceとSynvelisの原因推定・診断意味論を分離 |
| 12 | 既存ADR/公開API/他Consumerを保護してCompat Path付きで段階Migration |

## 4. Target Architecture（Conceptual）

~~~mermaid
flowchart TB
  subgraph App["Synvelis Application"]
    P["Project Controller / Track / Automation / Asset"]
    U["Workspace / Mixer / Inspector / Library"]
  end
  subgraph Comp["Runtime Composition"]
    L["Semantic Lowering + Source Map"]
    BR["Audio↔Plugin Port Adapter (crate optional)"]
    RR["Render / Import Composition"]
  end
  subgraph Engine["Generic Ruviform Execution"]
    G["Typed Graph IR / Validator"]
    AC["Audio Compiler / PDC"]
    AR["Prepared Audio Runtime / RT Owner"]
    PH["Plugin Host + Endpoint"]
    EX["OfflineDriver / Generic Executor"]
  end
  subgraph Base["Generic Ruviform Foundation"]
    TIME["Rational Beat / Clock / TempoMap"]
    RT["RT primitives / retirement"]
    IO["Media I/O / WAV backend"]
    OBS["Observability / Probe"]
    HOST["UI Host / Dock / Viz"]
  end
  P --> L
  P --> RR
  U --> HOST
  L --> G --> AC --> AR
  L --> BR
  BR --> AR
  BR --> PH
  RR --> EX
  RR --> IO
  AR --> RT
  AR --> TIME
  AR --> OBS
  PH --> RT
~~~

この図は**コンポーネント責務の論理構造**であり、現存Cargo dependency graphでも完成済みGraph Compilerでもない。

## 5. 新旧crate配置の決定・保留

| Current | Target | Status |
| --- | --- | --- |
| math / numeric / dsp / audio-analysis | 既存Kernel/FFT/Analysis保持 | 維持 |
| rt / observability | RT所有/退役＋有界Evidence/Probe | 機能拡張 |
| media-core | Rational Beat、Validated Ppqn、TempoMap/Clock Value | 拡張。独立Time crateは保留 |
| audio-core / video-core / video / video-proc | 基本ドメイン/Video再利用を保持 | 原則維持 |
| media-io / io-symphonia / io-avi | Generic Decode/Format維持 | Codec能力拡張 |
| **io-wav相当** | PCM WAV Encode/Mux/Finalize | 新Backend推奨（crate名はPoC） |
| audio-device | Input/Duplex/DeviceClock/Latency | 拡張 |
| plugin-core/host/clap/vst3/ofx/export | 標準独立Host＋Endpoint＋Adapter | 原則維持、Timed Event等拡張 |
| audio | Typed DAG実行、Audio Compiler、PDC、RT State | 重点再設計/旧Graph Compat |
| **graph候補** | NonRT Node/Port/Edge/Validator | 独立crate抽出が有力だが未決定 |
| **audio-plugin-bridge候補** | Plugin Endpoint→Graph Node Adapter | 専用責務は採用、独立crate化はPoC |
| exec | Generic Offline/Cancel/Progress | 保持、上位Audio Stateful Renderと統合 |
| ui / ui-webview / viz | Generic Window/Dock/Bounded UI Data | 拡張 |
| edit | NLE向け編集モデル | 維持。DAW Projectと統合しない |
| cli-sample | 非DAW Consumerの回帰 | 維持 |

**原則:** Graph IRがAudio Device/Plugin Hostなしで構築/検証可能、Audio EngineがPlugin Hostを通常依存せず、Plugin HostがAudio Engineを通常依存せず、観測Primitiveは上位Engineを必要としない。dev-onlyテスト依存まで同じ意味で禁止しない。

## 6. 公開API境界（設計契約。存在するAPIではない）

- \`GraphSpec\` / \`ValidatedGraph\`：NonRT Graph Node/Port/Edge/SignalDomain/Role/Channel Layout、Cycle/Multiplicity/Capabilityの検証。
- \`AudioCompiler\` / \`PreparedPlan\`：NonRT Schedule/Buffer/PDC/Event Budget、Runtime NodeとTarget Bindingの準備。Format変換を黙って挿入しない。
- \`RtExecutionState\`（概念）：Stateful Processor Instanceの単一RT Owner。Schedule Generationとの安全な対応/Activation/NonRT Retirement。
- \`ActivationTicket\` / \`AppliedReceipt\`：Command受付と実際のRT Applied Frame/Plan/Transport Generationを分離。
- \`TimedEvent\`：Target/Binding/Plan/TransportGeneration、FrameOffset、Stable Source Order、Loop Occurrence、分類別Overflow。
- \`MediaReader\` / \`Prefetch\` / \`OutputWriter\`：RuviformはCodec/Buffer/IO、SynvelisはStable Source/Asset/Project Commit。
- \`ProbeRecord\` / \`JobReceipt\`：Generic Execution Evidence/Progress/Cancel、SynvelisはIncident Diagnosis/Job Ownership Scope。

### Plan ownership comparison

- **Plan-owned Processor:** 実行PlanがProcessorを所有。構造は簡潔だがGraph変更時にPlugin Stateの移行/再生成が課題。
- **Stable Runtime Slot:** ScheduleとInstanceの寿命を分ける。Plugin維持に有利だが、Schedule切替/RCU/retirement/target generationが難しい。

後者を**目標候補**とするが、初期段階で実証なしに最終採用しない。旧\`PlanSwap\`の\`RwLock<Arc<GraphPlan>>\`/\`try_read\`を無断廃止しない。同期方式変更時は後継ADRを作成する。Gap-free graph swapや全PluginのBit-perfect Offline結果も無条件保証しない。

## 7. Migration M0–M7と並行可能なPoC

| Stage | Target | Evidence/Gate |
| --- | --- | --- |
| M0 | 28 crateのCargo通常/dev/optional/feature DAG、Public API、ADR、Consumer Baseline | 回帰可能なBaselineと互換性影響表 |
| M1 | Typed Graph/Port/Edge/ValidatorをMock-first | Device/Plugin HostなしでGraph検証 |
| M2 | Audio Compiler/Buffer Lifetime/PDC/Prepared Runtime、RT State Ownership比較 | Fan-in/out/Sidechain、PDC、RT NoAlloc/非RT退役 |
| M3 | Variable PPQN/TempoMap、Transport Span、Timed MIDI/Param Event | 同一Frame Stable Order、Loop/Seek、旧960互換 |
| M4 | Plugin Endpoint→Graph、Multi-bus、L0/L1、Generation/State | Single Owner/Plugin Restore/PDC/Regression |
| M5 | Device Input/Duplex/Clock、Media Reader/Prefetch、Recording/Playback | Mock-first Raw Timestamp/Fail-stop/Placement |
| M6 | PCM WAV Encoder/Muxer、Sequential Stateful Render、Output Validation | Import→Playback→Export→Reopen E2E |
| M7 | Multiwindow/Docking、Job Scope、Probe/Diagnostics、Compat Cutover | WindowClose Job Safety＋Video/CLI/Plugin/UI回帰 |

\`M1→M2→M3→...\`の厳密なウォーターフォールを強制せず、WAV Encoder PoC、Device Duplex、Dock/Probe等の独立課題は並列実施可能。RT/ABI/FS/OS保証は対象API/Platformに応じて測定する。

## 8. Initial E2E: Minimum DAW Production Slice

1. Local WAV Sourceを読み込みFormat/Duration/Hash/Versionを検証。
2. Verified Immutable MediaとしてProjectのAsset/Sourceへ明示CommandでCommit。
3. Track/Clip AuthoringからTyped GraphとSource MapへLowering。
4. Mock/DeviceでGraph Playback（非RT Decoder/Prefetch、RT Buffer、Transport）。
5. 同じTime/Event/Graph semanticsのOffline Sequential Render。
6. PCM WAVを実Encoder/Muxerへ渡しDrain/Finalize/Validation/Safe Publish。
7. 書出WAVを再読込しSample/Duration/Channelを確認。

高度なStem/Freeze/Remote/Plugin MIDI/Automation Write/UI Multiwindowは初期Sliceの条件外だが、Time/Source/Graphを別の暫定形式で二重実装しない。

## 9. Architecture Verification TA-001〜TA-012

| ID | Acceptance |
| --- | --- |
| TA-001 | 通常Cargo依存DAG非循環。Dev/Optional/Feature/Target別に明示 |
| TA-002 | Graph/FoundationにSynvelis Project/Track/Take/Undo型が存在しない |
| TA-003 | Graph IRがDevice/HostなしでBuild/Validateできる |
| TA-004 | Audio CompilerがFan-in/out、Sidechain、PDC、Buffer Lifetimeを解決 |
| TA-005 | Plugin Host→Audio Engineの通常依存なし。Adapterは正方向 |
| TA-006 | Stateful Plugin/DSPに単一RT Owner、旧/新Planで二重Processなし |
| TA-007 | RT内Alloc/Dealloc/Blocking/非RT Destructorなし。退役容量テスト |
| TA-008 | Plan/Transport/Instance/Target Generation跨ぎの誤Event配送なし |
| TA-009 | Variable PPQN、Transport Span、MIDI/Param Sample Eventの精度 |
| TA-010 | PCM WAVの実Encode→Finalize→Reopenと内容検証 |
| TA-011 | Window Close≠Application/Project Jobの無断Cancel |
| TA-012 | Audio/Video/Plugin/CLI/UI ConsumerのCompat Regression |

既存のEX、TT、MR、PPQN、Recording VSの受入も併用。CI greenは全RT環境での完全安全性を証明しない。実装前にAPI/ABI/Platform/API Deprecation Policyを作成し、旧Consumerを段階移行する。

## 10. ADRとの関係と未確定事項

- 維持：[RT Contract](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/realtime-contract-and-verification-policy.md)、[Plugin RT Ownership](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/plugin-rt-ownership-split.md)、[Media I/O Boundary](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/media-io-backend-abstraction.md)、[UI Architecture](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/ui-architecture-webview-hybrid.md)。
- Acceptedで実装待ち：[Variable PPQN](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/configurable-ppqn-resolution-independent-musical-time.md)。
- **後継判断が必要になる可能性あり：** [Audio Snapshot Swap](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/audio-engine-snapshot-swap.md)の同期/Plan状態方式。現ADR自体は変更しない。
- 新規境界ADR：Ruviform汎用Capability所有とSynvelis DAW意味論の分離、NonRT Graph IR vs Audio Runtime、Single RT Ownerの方向のみを確定。crate新設やState Registry具体方式は確定しない。
- **未確定：** Graph IRのcrate/API、Bridge独立crate化、Stable Runtime Slotの型/ライフサイクル、旧PlanSwap置換方式、PPQN/分母上限、L1適用精度、Span/Event Budget、Codec/Tail/FS Publish、UI/GPU Backend。

**結論:** Synvelis v0.3〜v0.6の合意をRuviform目標能力へ投影し、Graph/Audio実行/Stateful ownershipの再配置を中心に、Compat維持・PoC検証付きM0〜M7で段階移行する。合意したのは構造と検証条件であり、RuviformのRustコード改変や実装完了ではない。

## 11. 初期E2E 2系統の依存解決（横断監査 2026-11-01）

**決定の位置付け:** v0.7.1 §8「Local WAV→Playback→Export」は**Playback/Render側の最小Production-oriented Slice**であり、既存[Recording Vertical Slice #13〜#16](https://github.com/scarecrow173/synvelis/issues/12)を廃止・後回しにするものではない。M0〜M7はRuviform汎用能力のMigration群で、SynvelisのE2Eを1列で待機させる工程順を意味しない。

| Stream | 最小依存 | 完了判定 |
| --- | --- | --- |
| **Common F0** | Stable Project/Asset/Source/Clip ID、ProjectController/Save/Reload、Session/Source/Engine Frame区別、Verified Media/Manifestの正本。Recording [VS-A #13](https://github.com/scarecrow173/synvelis/issues/13)は有力な最初の実装入口 | 同一ID・時刻・保存契約を両E2Eが共有、別実装/暫定時間型を増やさない |
| **E2E-L（Playback/Export）** | F0 + Decode/Media Reader/NonRT Prefetch + Typed Audio Graph/Playback + [Ruviform #191](https://github.com/scarecrow173/ruviform/issues/191) PCM WAV実Encode/Finalize | Local WAV→Verified Source→Track/Clip→Playback→Offline Master WAV→再読込/Channel/Frame検証 |
| **E2E-R（Recording）** | F0 + [VS-B #14](https://github.com/scarecrow173/synvelis/issues/14) Mock Input/RT Capture → [VS-C #15](https://github.com/scarecrow173/synvelis/issues/15) Raw PCM + Journal/Recovery → [VS-D #16](https://github.com/scarecrow173/synvelis/issues/16) Idempotent Commit/Undo/Reload | Mock→RT Gate→Staging→Verified Range→Project Commit→Save/Reload/Undo、Queue Fault/Crash Recovery |
| **E2E-X（Cross-Integration）** | E2E-LとE2E-Rがそれぞれ通過した後 | 録音SourceをPlayback/Offline Exportへ渡し、Source ID/Source Frame/Project Placement/Outputの整合を確認 |

**Dependency DAG（概念）:**

~~~mermaid
flowchart TD
 F["F0: Project / Stable IDs / Media & Time Foundation"]
 R1["VS-B: Mock RT Capture"]
 R2["VS-C: Staging Writer / Recovery"]
 R3["VS-D: Project Commit / Recording E2E"]
 L1["Media Decode / Prefetch"]
 L2["Typed Graph / Playback"]
 L3["PCM WAV Encoder / Offline Render"]
 L4["E2E-L: Playback / WAV Export"]
 X["E2E-X: Recorded Source → Playback / Export"]
 F --> R1 --> R2 --> R3
 F --> L1 --> L2 --> L4
 F --> L3 --> L4
 R3 --> X
 L4 --> X
~~~

- VS-Bは**Mock Input**で進められ、実Device Duplex [Ruviform #187](https://github.com/scarecrow173/ruviform/issues/187)は初期VS-Dの必須前提ではない。
- VS-Cの初期候補は**Raw PCM + Metadata/Journal**であり、外部Export向けWAV Encoder #191に依存させない。後から同じMedia Sink能力へ統合するときもRecovery/Safe Checkpointの保証は保つ。
- E2E-Lは録音の全VS完了に依存しない。E2E-RもTyped Graph/実Plugin/External WAV Exportに依存しない。
- F0におけるTimeは[可変PPQN・有理Beat設計](https://github.com/scarecrow173/synvelis/issues/19)を前提にし、既存960コードを本仕様として固定化しない。
- 両E2Eの完了判定を混同しない。優先順位/工数/具体crate/テストCIへの展開は**M0 Inventory後**に確定。今回の作業は設計整合性の補足であり、M0実装へは進まない。

**残る設計制限:** Plugin Block/TransportのExact Capabilityは[Synvelis v0.5 §14](time-transport-midi-automation-plugin-integration-architecture.md)、[Ruviform #190](https://github.com/scarecrow173/ruviform/issues/190)に従う。両E2Eは初期Pluginなしでも成立させ、Plugin完全対応済みと誤認しない。
