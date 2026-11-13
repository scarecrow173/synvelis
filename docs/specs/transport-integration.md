---
id: "SPEC-034X7wGMLTCbfLxfME4Odo"
type: "spec"
status: "approved"
title: "Transport Integration — Span/Seek/Loop/Tempo・MIDI/Plugin Event Offset・Automation FrameBase・State Contract・Capability/Degraded (TT-001〜030 design contract)"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/20"
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
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/specs/automation-modulation.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/specs/recording-take-comping.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/19"
  derived-by:
    - "docs/designs/transport-integration-design.md"
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/transport-integration.md"
  defers: []
  deferred-by: []
---

# Transport Integration — Span/Seek/Loop/Tempo・MIDI/Plugin Event Offset・Automation FrameBase・State Contract・Capability/Degraded

## What Should Be Built

Issue #20 の canonical 設計契約: Time / Transport / MIDI /
Automation / Plugin Integration v0.5（Span・Event・State
Contracts）と、その監査補遺 TT-025〜030（Transport Span /
Plugin Process 互換契約）を一つの正本として文書化する。
Engine Frame を event 位置の正本とし、Plugin Event offset が
実 `process` 呼出し開始相対であること、Transport Command →
Applied Receipt、Half-open Span Scheduling、Timed Event
Envelope、Native MIDI / Note Ledger / Sustain / Chase、
Automation Write 分離と FrameBase、Plugin State Capture
整合 Level、Capability 別 Exact/Degraded/Unsupported/Unknown
判定を、TT-001〜030 の各受入ケースへ写像して固定する。

上流 baseline `time-transport-midi-automation-plugin-integration-
architecture.md`（v0.5 Approved）と
`configurable-ppqn-musical-time-architecture.md`（#19 Approved
方向）に加え、TIM-01〜04（#125〜128）の headless 実装済み
slice と Ruviform pinned revision（`fd1d4d5`）の generic
契約を統合し、実装 Owner が参照する正本とする。

本 slice は **docs/ 設計契約 + evidence 台帳のみ**。実装は
#125〜128（TIM-01〜04、headless slice 実装済み）・#129/#130
（SMF SMPTE・MIDI2/MPE、後続）・Ruviform #190/#188/#186/
#189/#307（汎用契約）が担う。設計完了を実装・実 Plugin・
実 OS・実 MIDI device の受入 PASS とみなさない。

## Why Now

- 上流 architecture v0.5 は 14 合意項目と TT-001〜024 を、
  §14 が TT-025〜030 を定義済みだが、canonical
  spec/design/plan/task/test-spec/impl の文書鎖と TT→実
  fixture/oracle/状態の evidence 台帳が未整備。
- TIM-01〜04 の headless 実装 slice（PPQN・SMF native
  division・process boundary capability/degraded・plugin
  state snapshot）が main に存在し、TT-001〜030 の各
  ケースに対応する検証済み fixture がある。これらを Issue
  #20 の受入条件へ正直に写像する正本が必要。
- Ruviform `fd1d4d5` には `TransportScheduler`/
  `AppliedReceipt`/`BlockContract`/`InBlockTransportSupport`/
  `NoteLedger`/`BeatPosition`/`build_call_event_list` と
  TT-ID を冠した upstream test（`tt008`/`tt009`/`tt010`/
  `tt012`/`tt025`〜`tt030`）が存在し、Synvelis 側の写像と
  独立に照合できる。
- 実装 Owner（#125〜128 の残件・#129/#130・ruviform#190 系）
  が着手・受入判定する前に、時間座標・Capability/Degraded・
  Exact/Offline Fail-closed の判断基準を固定しないと、
  consumer ごとの解釈差異と「設計変更だけで実機 PASS」の
  誤認が生じる。

## Users and Value

- 実装者（#125〜128・#129/#130・ruviform 側）: 時間座標・
  世代・offset 起点・precision class・state quality の
  判断基準と、各 TT case の現状観測。
- Reviewer: Owner 分離（Synvelis DAW 意味論 vs Ruviform
  generic primitive）と evidence の誠実さ
  （headless≠実機受入、Exact/Degraded/Unsupported/Unknown
  と Pass/Fail/NotRun の分離）を一意に判定できる。
- 将来の consumer（Offline render・Recording・probe/
  control・GUI）: 同一 Engine Frame 正本・receipt・
  capability 語彙を共有できる。

## Scope

### In Scope

- 時間・分解能・世代の分離契約（Device Clock Epoch、Engine
  Frame、Recording Session Frame、Project Musical/Media
  Time、Source Native Time、TempoMapRevision、
  TransportGeneration、ExecutionPlanGeneration）。
- PPQN 非依存 Rational Beat Position・Project PPQN（既定
  960・可変）・Source Native Tick/Division の非破壊性。
- Transport Command → Validation → nonRT Prepare →
  Bounded Scheduling → RT Apply → Applied Receipt
  （Requested/Pending/Applied/Rejected の区別）。
- Half-open Transport Span Scheduling と、Logical Span ≠
  Physical Plugin Process Call の区別。
- Timed Event Envelope（Engine Frame 正本・Plan/
  Transport Generation・Stable Event Identity・Loop
  Occurrence・Source Sequence）と、Plugin Event offset =
  実 process 呼出し開始相対の規約。
- Physical Process legality（Activated `min_frames ≤
  frames ≤ max_frames`）と Capability 判定
  （Exact/FrameAccurate/BlockOnly/DegradedWithConsent/
  Unsupported/Unknown、`InBlockTransportSupport`）。
- Offline Exact の Fail-closed と、Realtime Degraded の
  明示的許可/同意（`DegradedConsent`）規則。
- Native MIDI Division/Tick/Event 順保持、同 Frame 安定
  因果順、Plan Generation/Overflow の型付き Fault、
  NoteLedger/Sustain CC64/Chase/Release/Reset の安全性。
- Automation/Modulation の評価段階分離、Write Mode/
  Inspector Gesture Target 固定、Project Commit と RT
  Applied の時点分離、LFO を Reference 記録へ混入させない
  規則。
- Plugin State Capture 整合 Level（Coordinated/Quiescent/
  BestEffort/Unsupported）と、File Save ≠ Plugin State
  Exact ≠ RT Active の receipt 分離。
- Single RT Owner/Generation 照合と、旧世代 Event の誤
  配送禁止。
- Live/Offline/Recording の共有評価意味論と、Driver/
  Instance/Failure Policy の区別。
- Synvelis/Ruviform の責務分離と、Ruviform 不足時の
  upstream Issue 運用。
- TT-001〜030 の各ケースについて、実装 Owner・実行
  手段/Fixture・Revision・Pass/Fail/NotRun/Unsupported・
  実 Plugin/OS の未実施範囲を `evidence/sv20/` に台帳化。

### Out of Scope

- 実装コード（`crates/`、`Cargo.toml`、`Cargo.lock`、
  `mise.toml`、CI、本番 crate）。実装は #125〜128・
  #129/#130・Ruviform 各 Issue の責務。
- Plugin Host/RT Scheduler/Process Offset 等の Ruviform
  所有共通実装の変更・再実装。
- 設計変更だけによる実 Plugin State/OS Capability の
  PASS 主張。実 VST3/CLAP・実 OS MIDI・実 device・実 GUI
  の受入（EXT-L3/GUI-L2）。
- `docs/specs/musical-time.md` 等の sv#19 並行作業文書の
  編集・作成（参照のみ）。
- SMF SMPTE Division/Format2（#129）・MIDI2/MPE/Per-note
  （#130）の初期契約 — 明示 Unsupported/Deferred として
  記録するのみ。
- 新規の製品方針変更・互換性破壊・数値 SLO・OS サポート
  宣言（Issue #20 の dispatch 判断どおり別管理）。

## Requirements

- **TT-R-01 時間・世代の分離**: Device Clock Domain/Epoch、
  Engine Frame、Recording Session Frame、Project
  Musical/Media Time、Source Native Time、
  TempoMapRevision、TransportGeneration、
  ExecutionPlanGeneration を別の意味で保持し、異なる
  Domain の値を無検証で比較・加算しない。Timestamp
  信頼度は Verified/Estimated/Unknown を識別する。
- **TT-R-02 PPQN 非依存の音楽時間**: Canonical Beat
  Position は分解能非依存の有理数等の Lossless 表現。
  Project PPQN は既定 960・可変で Tick 投影にのみ文脈
  として作用し、異 PPQN の Tick 整数値を黙って比較・
  加算しない。整数 Frame の丸めは変換連鎖の最後の境界
  で一度だけ行い、表現不能は暗黙丸めでなく明示
  Fail/PoC 定義の結果とする。
- **TT-R-03 Transport Applied Receipt**: Play/Pause/Stop/
  Seek/SetLoop/ClearLoop/Tempo 編集/Device Restart を
  Command → Validation → nonRT Prepare → Bounded
  Scheduling → RT Apply → Applied Receipt として扱い、
  Requested/Pending/Applied/Rejected を区別する。
  Receipt は Target/Transaction・Applied Engine Frame・
  TransportGeneration・PlanGeneration・成否/拒否理由を
  表し、Accepted ≠ Applied を偽装しない。Loop Wrap は
  Transport Discontinuity であり Device Clock Epoch 更新
  ではなく、Recording Session Frame を巻き戻さない。
- **TT-R-04 Half-open Span Scheduling**: Transport
  Context（Tempo/Seek/Loop Wrap/Clip Boundary/Transport/
  Plugin Context）が変わる位置で論理 Span `[start,end)`
  へ分割し、各 Span の Project Position・LoopIteration・
  TempoRevision・TransportGeneration・Plugin Transport
  Context を確定する。境界 `== span_end` は次区間で
  1 回だけ配送し、重複/欠落を抑制する。RT 実行は事前
  確保容量以内とし、超過 Plan は非 RT で Restrict/Reject
  する。
- **TT-R-05 Timed Event / Engine Frame 正本**: Event の
  正本位置は Engine Frame。`TimedEvent` は Target・
  Plan/Transport Generation・Engine Frame・Stable Event
  Identity/Source Sequence・Loop Occurrence を必要に
  応じて保持し、古い世代 Event を新 Target へ無検証
  配送しない。同一 Frame は明示的因果依存→Source の
  安定順序を保持し、「NoteOff は必ず NoteOn より先」
  の普遍優先順位は採らない。
- **TT-R-06 Plugin Event Offset 起点**: Plugin EventList
  の `frame_offset`・CLAP `clap_event_header.time` は
  **各物理 process 呼出し開始相対**。Callback-relative・
  Span-relative offset を無検証で転送せず、配送境界で
  `event_engine_frame − process_start` へ再計算する。
  全て半開区間で、0-frame flush は専用制御境界として
  通常 Audio 区間と混同しない。
- **TT-R-07 Physical Process legality / Capability**:
  Activated Plugin の各物理 Chunk は `min_frames ≤
  frames ≤ max_frames` を満たす。Preflight（非 RT）で
  min/max Block・最大分割数・L0/L1 Block 契約・Transport
  Context 更新方式・Stateful DSP/PDC/Tail・時刻精度を
  確認し、必要能力を確認できない Plugin を「対応済み」
  と扱わない。Capability は Exact/FrameAccurate/
  BlockOnly/DegradedWithConsent/Unsupported/Unknown を
  区別し、`InBlockTransportSupport` は Verified のみを
  in-block transport 対応とみなす（規格に存在するだけで
  Sample-accurate を保証しない）。
- **TT-R-08 Offline Exact / RT Degraded**: Exact Offline
  要求時は未保証精度を Fail-closed とし、無断の量子化・
  精度低下を行わない。Realtime の Degraded 動作は
  明示的許可/同意（`DegradedConsent` 相当）と
  Capability 判定を必須とし、`AppliedReceipt`/診断に
  実際の適用 Engine Frame と精度低下を記録する。
  RT 内で制約違反の短い Block を無理に送らない。
- **TT-R-09 Native MIDI / Note Safety**: Import した
  MIDI の元 PPQN/SMF Division/Tick・Raw Capture の
  Device Timestamp/Clock Epoch を Raw として保全し、
  Project PPQN 変更・録音・Quantize で破壊的変更を
  しない（Quantize は別 Derived Version）。NoteLedger
  は Target/Plugin Instance/Channel/Note Identity で
  状態を追跡し、Seek/Loop/Stop/Comp Boundary/Plugin
  Replace の Release/Reset/Controller Chase を明示する。
  NoteOff 喪失は Event Integrity Fault とし、対象
  Plugin/Adapter Capability に応じた Defined Recovery と
  Degraded 状態を提示する。「NoteLedger により全 Plugin
  で Stuck Note ゼロ」は保証しない（MIDI1 同 Channel/
  同 Pitch 重複・Plugin 内部 Voice 制約は Capability/
  Best-effort）。
- **TT-R-10 Automation / Write 分離**: Base Value →
  Automation/Manual Reference → Modulation →
  Clamp/Smoothing → Host-resolved → Dispatched →
  Plugin-reported/Observed の段階を区別する。Off/Read/
  Touch/Latch/Write・Write Arm・Gesture State・Recording
  Transaction を #6 と整合させ、RT Event 配送は Project
  を直接変更せず非 RT ProjectController へ確定する。
  Gesture 開始時に Stable TargetId・Plugin
  Instance/Binding Generation・Arm・TransportGeneration
  を固定し、Inspector Follow 変更で記録対象を替えない。
  既定は Modulation 前 Reference を記録し、LFO 適用済み
  値を再 Modulation する事故を避ける。Block/Control/
  Sample-accurate/Per-note の精度は Adapter ごとに評価
  する。
- **TT-R-11 Plugin State Capture**: State Capture 結果を
  Coordinated/Quiescent/Best-effort/Unsupported で区別
  し、Snapshot 取得成功を「特定 Frame に同期した完全
  Snapshot」と見なさない。File Save 成功・Plugin State
  Exact・RT Active は別 receipt。取得失敗/古い State は
  Degraded/明示 Error とし、Plugin Missing/Version 差が
  あっても Opaque Bytes を保全する。RT に無期限停止や
  OS I/O を求めない。
- **TT-R-12 Single RT Owner / Generation**: Stateful
  Plugin を旧/新 Plan で同時駆動せず、退役は nonRT。
  Stable TargetId→Plugin-local ParamId Mapping は非 RT
  検証とし、旧 Plan/Transport/Plugin Generation に紐付く
  Event を新 Instance へ誤配送しない。RT 危険
  Destructor/無期限 Wait/alloc/OS I/O/Project Mutation を
  持ち込まない。
- **TT-R-13 Live/Offline/Recording 共有意味論**: 時間と
  Event 順序規則を共有するが、Device/Offline Driver・
  独立 Plugin Instance・Raw Capture/Monitor・Failure
  Policy は区別する。固定 Test Node の同一 Frame/入力/
  条件比較で検証し、全 Plugin の Bit-perfect Offline
  再現性を無条件保証しない。
- **TT-R-14 Synvelis/Ruviform 責務分離**: Generic
  Beat/Clock/Transport Scheduling/Timed Event/Plugin
  Host/Probe 能力は Ruviform、DAW Track/Clip/Take/Comp・
  Automation Write/Undo・Project 設定/Source 意味論は
  Synvelis。Ruviform の現行 crate 構造維持を目的にせず、
  reuse/extend/extract/replace/deprecate を比較する。
  generic 不足は Synvelis 側で恒久的重複実装せず
  Ruviform Issue へ起票する。
- **TT-R-15 Evidence Honesty**: TT-001〜030 の各ケースに
  ついて実装 Owner・実行手段/Fixture・Revision・
  Pass/Fail/NotRun/Unsupported・実 Plugin/OS の未実施範囲
  を記録する。`Exact/Degraded/Unsupported/Unknown`（精度
  Class）と `Pass/Fail/NotRun/Unsupported`（検証結果）を
  混同せず、headless 証拠を実機受入に繰り上げない。
  設計レビュー Pass と実測実装 Pass を区別する。

## Acceptance Criteria

- [ ] **TT-AC-01（TT-A）**: TT-001〜009 — Rational
  Beat/TempoMap、Clock Epoch、Transport Command/Applied
  Receipt、Loop Span、Plugin Transport Context の正本・
  境界を Fixture で検証する設計・写像が存在する。
- [ ] **TT-AC-02（TT-B）**: TT-010〜014 — Native MIDI
  Division/Tick、同 Frame Event 順、Plan
  Generation/Overflow、NoteLedger/Sustain/Chase の安全性
  を検証する設計・写像が存在する。
- [ ] **TT-AC-03（TT-C）**: TT-015〜018 — Automation/
  Modulation、Write Mode/Inspector Gesture Target の分離・
  精度を再現し、Project Commit と RT Applied の時点を
  混同しない設計・写像が存在する。
- [ ] **TT-AC-04（TT-D）**: TT-019〜024 — Plugin State
  整合、Lifecycle/Latency、CLAP/VST3、Offline、Raw
  Recording、Trace を Capability 別に検証する設計・
  写像が存在する。
- [ ] **TT-AC-05**: TT-025〜030 — Engine Frame を正本と
  し、Plugin Event offset が実 process 呼出し開始相対
  であることを Loop/Seek/Tempo/小 Block で検証する設計・
  写像が存在する。
- [ ] **TT-AC-06**: TT-025〜030 — Callback 256 Frame・
  Loop at 100・Plugin min_frames=128 で 100 Frame の
  違法 process 呼出しを起こさず、許容分割・実証済み
  Block 内 Transport Event・明示的 Degraded/Unsupported
  のいずれかを正しく選択する設計・写像が存在する。
- [ ] **TT-AC-07**: TT-025〜030 — Exact Offline 要求時は
  未保証精度を Fail-closed とし、Realtime の Degraded
  動作には明示的許可/Capability 判定を求める設計・写像
  が存在する。
- [ ] **TT-AC-08**: PPQN960 既定・可変設定と Source-
  native Tick/Division の非破壊性を Synvelis #125/#126
  の責務へ接続し、Ruviform #190/#188 の汎用契約と独立に
  検証する写像が存在する。
- [ ] **TT-AC-09**: 各 TT ケースについて実装 Owner・実行
  手段/Fixture・Revision・Pass/Fail/NotRun/Unsupported・
  実 Plugin・OS の未実施範囲を `evidence/sv20/` の台帳に
  記録する。

## Deferred Design Concerns

- 実 VST3/CLAP plugin・実 OS MIDI backend・実 device・
  実 GUI の TT gate 実測 | headless/mock 設計契約のみ |
  synvelis#114・EXT-L3・ruviform#253/#196 | 実装 Owner が
  headless Pass を実機受入と誤読する。
- SMF SMPTE Division/Format2（#129）・MIDI2/MPE/Per-note
  （#130） | 初期契約は明示 Unsupported/Deferred |
  synvelis#129/#130 | 将来拡張時に native timebase 契約の
  再確認が必要。
- Tempo Ramp・BeatPosition の整数幅/分母上限の最終値・
  Max Span/Event Budget の数値・Touch Return Time・
  Sample-accurate 対応の細部 | PoC で確定（architecture
  §13 未確定事項を継承） | #125〜128・ruviform#189/#190 |
  実装時に契約値が変わる可能性。
- Plugin Adapter ごとの正確な State Capture/Precision・
  MIDI1 同音高重なりと CC64 復帰の限界 | Capability/
  Best-effort として公開 | ruviform#196・synvelis#127 |
  「全 Plugin で Stuck Note ゼロ」等の過剰保証を防ぐ。
- AudioLens native f64 精度の非回帰 | 型レベル維持・
  実測は別 Gate | synvelis#294・ruviform#294 | f64
  consumer 経路の実測未実施。

## Sources and Relations

- Issue #20 本文（TT-001〜030 受入・Owner 分担・禁止事項・
  2026-11-10 dispatch 判断）。
- `docs/designs/time-transport-midi-automation-plugin-
  integration-architecture.md`（v0.5 Approved — 14 合意
  項目・TT-001〜024 表・§14 TT-025〜030 補遺の上流
  baseline）。
- `docs/designs/configurable-ppqn-musical-time-architecture.md`
  と synvelis#19 / ruviform#189（PPQN・Rational Beat の
  Approved 方向）。
- `docs/designs/playback-mixer-generic-execution-graph-
  architecture.md`（v0.4 — Plan 世代・PDC・失敗分類の
  関連 baseline）。
- TIM-01〜04 spec/design/test-spec/impl（#125〜128 の
  headless 実装済み slice: `tim-01-*`・`tim-02-*`・
  `tim03-*`・`tim-03-*`・`tim-04-*`）。
- `docs/specs/sf-03-map-semantic-runtime-contract.md`・
  `docs/specs/pc-c4-07-transport-mirror-observe.md`・
  `docs/specs/automation-modulation.md`・
  `docs/specs/ctl-01-*`・`docs/specs/recording-take-comping.md`
  （世代・transport mirror・Automation Write・Recording の
  確定済み縦断契約）。
- Ruviform pinned revision `fd1d4d5` の generic 契約:
  `ruviform-media-core`（`BeatPosition`/`Ppqn`/`TempoMap`/
  `EngineFrame`/`TransportScheduler`/`AppliedReceipt`/
  `BlockContract`/`InBlockTransportSupport`/`NoteLedger`/
  `transport_schedule`）・`ruviform-plugin-core`
  （`ProcessConfig` min/max frames・`transport_bridge`
  process-relative offset・`chase_span_events`）・
  `ruviform-plugin-host`（`PluginEndpoint` block gate）。
- 関連 Issue: synvelis #6/#10/#11/#12/#17/#18/#19/#20/
  #21/#22/#23/#98〜103/#106/#107/#109/#125〜130、
  ruviform #113/#184/#186〜190/#191/#196/#253/#294/#307/
  #309。
