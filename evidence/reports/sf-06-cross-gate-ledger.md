# SF-06 Cross-Gate Evidence Ledger — headless slice (Issue #109)

Issue #109「SF-06 Routing/Plan/Probe/GUI Cross Gate — Fault
Matrix・OS/RT/Device/Consumer 別受入」の headless slice で
実測した証跡。**headless Pass を実 OS GUI / device / plugin /
consumer 受入へ昇格しない** — 未計測 lane は `NotRun`、環境
非対応は `Unsupported` と記録する。

## 計測環境（2026-11-10）

| 項目 | 値 |
|---|---|
| Synvelis SHA | `e78be01`（origin/main base）+ branch `scarecrow173/sv109-sf06` commit `255307e`（後続 commit は ledger/検証更新のみ） |
| Ruviform resolved SHA | `0f5b57372e9a43f84415f7964b2f9636774174f0`（`Cargo.lock` git dep、全 ruviform crate 共通） |
| AudioLens revision | 未計測（consumer lane NotRun） |
| OS / CPU / FS | Windows 11 Pro / Intel Core i9-9900K / NTFS（C:） |
| Device + Plugin | なし（headless — audio device・native plugin host 未接続） |
| Toolchain | Rust `1.99.0`（mise pin）、`cargo test --workspace --locked` |
| Project schema | v7 envelope（routing = schema v6 由来、v5→v6 migration 検証済み） |
| Graph / Clock generations | persisted 層では N/A — runtime graph/clock generation は SF-03/SF-04 lane（`RoutingActivationReceipt`/`RuntimeClockEpochRef`）が正本。本 slice はそれらを変更していない |

## Lane 定義（Issue #109 §1 Stage に対応）

| Lane | 環境 | Pass にできるもの |
|---|---|---|
| T0 headless (synvelis-project) | deterministic unit/integration test | ID/ref/schema migration/CAS/Undo/Diff・persisted 契約 |
| T1 generic graph (ruviform) | Ruviform offline fixture | Domain/Port/Channel/PDC/Publish queue — **本 slice では再計測せず linked evidence のみ** |
| T2 headless E2E (synvelis-player/app/workspace) | Playback/Recording/Offline/Probe integration | 小規模 PCM・control/plan source map・fault recovery |
| T3 real OS GUI/Device | Windows/macOS/Linux + device/plugin | 実 Render/Focus/latency — **全て NotRun** |
| T4 both consumers | Synvelis + AudioLens resolved SHA matrix | public API/precision/RT/perf — **全て NotRun** |

## Case ID registry & lane matrix

凡例: `Pass` = 当該 SHA で実測 Pass、`Fail` = 実測 Fail、
`NotRun` = 未計測、`Unsupported` = 環境/能力非対応。
`Pass*` は persisted 層のみの部分 Pass で、runtime/PCM 部分は
別 lane の `NotRun` を伴う（Issue の「Partial passはSlice別明示」
ルール）。セル内の test file 名は headless evidence の
fixture/command 対応。

### SF-MODEL（Project & Legacy）

| caseId | scope（Issue #109 転記） | T0 | T1 | T2 | T3 | T4 |
|---|---|---|---|---|---|---|
| SF-MODEL-01 | 新規 Project・Master/Track/Strip/Bus/Return・processor order・Save/Reload/Undo — exactly-one Master・single DSP Strip owner・stable typed ID/serialization・ordered FX | Pass（routing_sf01, sf06_oracle） | NotRun | NotRun | NotRun | NotRun |
| SF-MODEL-02 | Track/Bus/Send/Chain ordering — ordered FX・strip ownership | Pass（sf06_oracle invariant, routing_sf01 reload） | NotRun | NotRun | NotRun | NotRun |
| SF-MODEL-03 | Send Config/Connection duplicate/dangling・invalid Owner/Port/Chain — 1:1 Send link・Map key==entity ID・invalid snapshot Reject | Pass（sf06_oracle invariant+corruption, routing_sf01 orphan test） | NotRun | NotRun | NotRun | NotRun |
| SF-MODEL-04 | Missing Plugin/Port・Graph ID reissued・opaque state — Stable Semantic Port preserved・Unresolved/not misrouted・Old State retained | Pass*（routing_sf01 unresolved test — persisted 層のみ。runtime misroute は T2 NotRun） | NotRun | NotRun | NotRun | NotRun |
| SF-MODEL-05 | v5 read-only migrate×2・legacy Comp/Clip・external insert_count — deterministic IDs/watermark/Direct-to-Master・no invented persist FX・old bytes/HEAD protected・PCM measured | Pass*（sf06_oracle migration oracle + routing_sf01 v5 tests — migration determinism/HEAD 保護。PCM golden は T2 NotRun） | NotRun | NotRun | NotRun | NotRun |
| SF-MODEL-06 | Track/Strip/Bus 単一 DSP 所有・Master exactly-one・ID watermark/Legacy v5 determinism・Visual Group/Probe 操作で Project/Routing/Undo 不変 | Pass*（sf06_oracle invariant + sf05 visual_ops test — visual 部分のみ。probe 操作の Project 不変は T2 NotRun） | NotRun | NotRun | NotRun | NotRun |
| SF-MODEL-07 | Track delete with refs — View/RT-only no Project Undo/Diff・delete requires explicit referenced composite | Pass*（routing_sf01 + sf06_oracle fault referenced-delete — reject 側のみ。composite UX は T3 NotRun） | NotRun | NotRun | NotRun | NotRun |

### SF-VALID（Project & Legacy validation）

| caseId | scope | T0 | T1 | T2 | T3 | T4 |
|---|---|---|---|---|---|---|
| SF-VALID-01 | Connection Main+Pre/Post Send+Return+Sidechain/role/channel — semantic/Generic Port Validation both pass・no implicit map/convert | Pass（sf06_oracle invariant direction/role, sf_02 sidechain/send/return tests） | NotRun | NotRun | NotRun | NotRun |
| SF-VALID-02 | Port direction/domain/role・channel mapping・multiplicity/capabilities | Pass*（sf_02 approved-map test・sf06_oracle empty-map reject・routing_sf01 empty_channel_map — rejection 側。multiplicity/capability 網羅は T1/T2 NotRun） | NotRun | NotRun | NotRun | NotRun |
| SF-VALID-03 | Mute/Solo・Sidechain・Bus Solo・Return/Master・Raw Capture — Pre before fader・Post after fader・Muted Track audible Sends+Sidechain 0・Solo dependency closure・recorded raw bytes unchanged | Pass*（sf06_oracle audibility sweep — model-level mute が全出力 edge（send/sidechain 供給含む）を止めることを 2^14 組合せで実測。PCM/Raw bytes は T2/T3 NotRun） | NotRun | NotRun | NotRun | NotRun |
| SF-VALID-04 | Solo dependency closure・Return/Master 非過剰遮断 | Pass*（sf06_oracle audibility sweep — soloed upstream supplier ∪ master 到達 downstream、muted 非 relay を全組合せで実測。PCM は T2/T3 NotRun） | NotRun | NotRun | NotRun | NotRun |
| SF-VALID-05 | Direct Monitor unavailable・feedback(d-delay included)・bad Domain/Direction/Format/Plugin capability — Unsupported/Reject vs Uncontrolled・no implicit graph deletion/fake support | Pass*（routing_sf01 unresolved explicit・sf_02 typed rejects — persisted 層の Unresolved/Reject 区別。Direct Monitor/Format/d-delay は T2/T3 NotRun） | NotRun | NotRun | NotRun | NotRun |
| SF-VALID-06 | feedback cycle — zero delay Reject・declared/delayed feedback policy | Pass*（sf_02 `a_feedback_cycle_rejects_as_a_typed_cycle`・routing_sf01 `the_strip_graph_cannot_cycle`・sf06_oracle fault cycle + invariant acyclicity — zero-delay reject のみ。declared delayed feedback は T1/T2 NotRun） | NotRun | NotRun | NotRun | NotRun |
| SF-VALID-07 | Temporary CreateBus→Connect・stale ProjectRevision/Plugin generation・multiwindow conflicts — 1 Project CAS+Undo・failure leaves Live/Revision/Undo invariant・no leaked temp Stable IDs | Pass*（sf_02 stale revision/capability・sf06_oracle fault matrix 8 系統 — CAS/Undo/状態不変。multiwindow conflict は T3 NotRun） | NotRun | NotRun | NotRun | NotRun |
| SF-VALID-08 | 仮参照 CreateBus+CreateConnection を 1 Project Controller Commit・候補全体 Validation・Error 時 Project/Revision/Undo 不変・Generator ID 消費/receipt/remap/再発行の正しい証拠 | Pass（sf_02 multi-edit intent/preview/deterministic receipt/dup-temp/rejected-intent tests・sf06_oracle fault matrix） | NotRun | NotRun | NotRun | NotRun |

### SF-ACT（Execution Graph/Processor/fault）

| caseId | scope | T0 | T1 | T2 | T3 | T4 |
|---|---|---|---|---|---|---|
| SF-ACT-01 | Legacy v5→new Lowerer same playback+Comp/Format/Insert — PCM/peak/diff・source binding・no silent skips | NotRun（#106 lowering の ACT evidence は sf03_l1 系 test が T2 でカバー — case-level ACT 判定は別 gate） | NotRun | Pass*（sf03_l1 lowering/PCM equivalence tests — ACT-01 の lowering/PCM 部分のみ） | NotRun | NotRun |
| SF-ACT-02 | Track→Bus→Send Pre/Post→FX→Return→Master・mute/solo/PDC/offline — signal-level/latency/sample frame & route source map | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-ACT-03 | Candidate preflight while remote edit/plugin-cap change then Project CAS — speculative plan never published unless committed BuildKey/source map exactly matches | Pass*（sf_02 `preview_routing_mutates_nothing_and_receipts_are_deterministic` — preflight 非破壊/receipt 決定性のみ。BuildKey/SourceMap 照合は T2 NotRun） | NotRun | NotRun | NotRun | NotRun |
| SF-ACT-04 | Publish while audio stopped or frame=0 then RT process — Published/Active distinct・Active receipt only from SharedStatus.active_generation | NotRun | NotRun | Pass*（sf03_l1 `sf03_04_publish_then_active_receipt` — receipt 区別のみ。RT process 実測は T3 NotRun） | NotRun | NotRun |
| SF-ACT-05 | publish QueueFull after some install commands queued — No new plan published・original processor/registry persists・stale discarded | NotRun | NotRun | NotRun（#106 fault taxonomy の QueueFull fixture は別 slice） | NotRun | NotRun |
| SF-ACT-06 | R1/R2/R3 edits rapidly・compile cancellation・Device Restart/Epoch change・retired overflow — Superseded discarded・no old port/processor wrong-target | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-ACT-07 | Plugin parameter fast changes on R2 while R1 Active — active mapping check・stale NodeEvent/ParamEvent never applied to another instance | NotRun | NotRun | Pass*（sf03_l6 `sf03_l6_01_automation_tick_delivers_on_active_generation` — active generation 照合のみ。plugin 変更/R2-R1 は T3 NotRun） | NotRun | NotRun |
| SF-ACT-08 | Missing source/Plugin/Port/Unsupported mapping・preview vs final render — coverage quality & explicit fail-closed/Degraded | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-ACT-09 | Project rename/visual layout vs Graph relevant edit・Undo/Redo/new Clock config — no unnecessary compile・Graph fingerprint correct | NotRun | NotRun | Pass*（sf03_l4 equivalence tests — name-only/undo equivalence。fingerprint/compile 回数の ACT 判定は T2 継続） | NotRun | NotRun |

### SF-PROBE（Physical observation/fault）

| caseId | scope | T0 | T1 | T2 | T3 | T4 |
|---|---|---|---|---|---|---|
| SF-PROBE-01 | Semantic Pre/Post/Send/Sidechain point→published source map・unavailable internal plugin tap — real slot only・Unsupported/Unknown vs audio zero | NotRun | NotRun | Pass*（sf04_project_probe resolution/honesty tests — point→map 解決と Unresolved 区別。published source map の ACT 接続は T2 継続） | NotRun | NotRun |
| SF-PROBE-02 | Off/Monitor/Capture & bypass comparison — Off perf/PCM unchanged・Monitor metrics correct・Capture bounded | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-PROBE-03 | Enqueue installation vs RT ack/late generation vs Port changed — Queued!=RT installed・stale drops・no wrong-slot PCM | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-PROBE-04 | Graph swap mid Capture・#308 ClearAll/Bind queue full — distinct capture segments・no auto recapture・partial/unbound explicit | NotRun | NotRun | NotRun（RF#308 linked — 未実装保証を adapter で代替しない） | NotRun | NotRun |
| SF-PROBE-05 | Pool exhaustion・telemetry full・dropped block・stale stamp — LostBlocks/LostRecords/Unobserved/gap mask != real silence | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-PROBE-06 | 2+ windows/Inspectors same Semantic Point・close one・new device — Session-owned single binding/subscription | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-PROBE-07 | same/different Clock/Epoch/Latency/Channel/rate・Seek/Loop — RF#309 align/conditional/incomparable・no guessed Musical time | NotRun | NotRun | Pass*（sf04_s3_probe_discontinuity epoch/capture-segment tests — epoch 切断の headless 側。RF#309 align 本体は T1 linked） | NotRun | NotRun |
| SF-PROBE-08 | Bass level drop traced through available probes・missing plugin — observed first difference vs Inferred root cause・unknown distinct | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-PROBE-09 | RT perf・no allocation/lock/IO・Native PCM f64 AudioLens・Capture budget — no RT/DSP regression・no fake bit-exact from lossy probe | NotRun | NotRun | NotRun | NotRun | NotRun |

### SF-UX（GUI/Cross-product）

| caseId | scope | T0 | T1 | T2 | T3 | T4 |
|---|---|---|---|---|---|---|
| SF-UX-01 | Semantic vs Visual Group/collapsed edge — Fold/expand/group/move/zoom/role filter で DSP/Project Revision/Undo unchanged・no fake collapsed edge/Role | NotRun | NotRun | Pass*（sf_05 workspace + sf05_signal_flow app tests — hierarchy 投影/role filter/collapse。DSP/PCM 側は T2 継続） | NotRun | NotRun |
| SF-UX-02 | drag intent/commit/undo — Port drag preview→cancel or approved command・invalid map/other window edits・stale target no-op・1 atomic Commit+Undo | NotRun | NotRun | Pass*（slice2 merge — routing gesture fixation/dry-run/drop/confirm/cancel・stale revision/capability/window・1 `ProjectOp::Routing` commit=1 undo を `sf_05_s2`/`sf05_signal_flow` tests で検証。実 pointer/OS 経路は T3/T4 NotRun） | NotRun | NotRun |
| SF-UX-03 | Inspector Follow/Explore/Pinned — inspection vs editing selection separate・no wrong-target mutation | NotRun | NotRun | Pass*（slice2 merge — Signal Flow inspection Follow/Explore/Pinned・session-only 履歴・link 追従・selection 分離を `sf_05_s2` tests で検証） | NotRun | NotRun |
| SF-UX-04 | Control Layer #102 not Audio routing — Automation/Control 表示と Routing/RT 対象・世代の照合 | NotRun | NotRun | NotRun（#102 依存） | NotRun | NotRun |
| SF-UX-05 | stale/late receipt — Configured R2/Active R1・Probe Missing/On/Off・Capture single-owner・status truthful・no duplicate capture/fake Observed | NotRun | NotRun | Pass*（sf05_signal_flow `published_without_rt_observation_is_not_displayed_as_active`・`absent_engine_reports_unattached_and_unobserved` — status 真実性の headless 部分） | NotRun | NotRun |
| SF-UX-06 | real 3OS GUI/HiDPI/keyboard/a11y focus — OS evidence per environment・no headless-only pass | NotRun | NotRun | NotRun | NotRun（#114 gate — Windows/macOS/Linux 実機） | NotRun |
| SF-UX-07 | Workspace Schema v2→vNext/unknown/corrupt saved view — layout versioned migration・Project Schema/PCM unaffected | NotRun | NotRun | Pass*（slice2 merge — `SignalFlowViewState` の `ViewInstanceId` 単位 persist・schema v2→v3 migrate・malformed degrade を `sf_05_s2` tests で検証。Project Schema/PCM 非影響は契約上不変） | NotRun | NotRun |
| SF-UX-08 | real Windows/macOS/Linux native/WebView・DPI・a11y・keyboard・Window detach/close — OS evidence per environment | NotRun | NotRun | NotRun | NotRun | NotRun |

### SF-COMPAT（DAW/Consumer non-regression）

| caseId | scope | T0 | T1 | T2 | T3 | T4 |
|---|---|---|---|---|---|---|
| SF-COMPAT-01 | no-Signal-Flow ordinary DAW Gate A — Create/Edit/Record/Save/Reload/Playback/Offline Export・legacy Clip→Master Golden 非退行 | NotRun | NotRun | NotRun（#18 EX-001〜020 の既存 evidence を link — Gate A 判定は別 gate） | NotRun | NotRun |
| SF-COMPAT-02 | Gate B structure diagnosis — Bass Pre/Post→Bus→Master・Kick Sidechain の Configured/Prepared/Published/Active/Probe Observed 区別・first observed change・root cause 正直 | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-COMPAT-03 | Gate C control source bidirectional — PC-C2/C5 Automation/Modulation/Depth driver 双方向探索・Audio Connection 型分離 | NotRun | NotRun | NotRun（#102 依存 — SF 側だけで Gate C Pass にしない） | NotRun | NotRun |
| SF-COMPAT-04 | Raw Record/Direct/Software Monitor・PDC/ADC/DAC・Offline render・Project Save — capture と Probe の混同なし・bit/latency recorded | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-COMPAT-05 | AudioLens f64/native decoder/analysis/export/RT・Synvelis f32/plugin/transport — no implicit accuracy/feature/latency/perf regression | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-COMPAT-06 | Ruviform #308/#309/#307 API changes・Rust/Cargo.lock/2 consumer issue closure — exact resolved SHA・per consumer Pass/Fail/NotRun・no permanent SHA freeze | Pass*（`cargo test --workspace --locked` が resolved SHA `0f5b5737` で exit 0 — lockfile 再現性のみ。API 変更/consumer closure は T1/T4 NotRun） | NotRun | NotRun | NotRun | NotRun |

### SF-XR（4領域合意 — 2026-11-04 追加）

| caseId | scope | T0 | T1 | T2 | T3 | T4 |
|---|---|---|---|---|---|---|
| SF-XR-01 | CTL-01/02 — 1 Track/Bus→1 Strip・Host ParameterTarget 唯一正本・Send Gain/PrePost Mute・Serialization/Undo/Graph SourceMap 一致・異 Layout ChannelMap 明示 Consent | Pass*（sf_xr_01 — persisted strip/send/consent/serialization/undo 部分。fixture は player test 内で project 契約を照合） | NotRun | Pass（sf_xr_01 — `lower_semantic` の `RoutingSourceMap`/typed `GraphSpec` 一致・mute prune。実 channel 変換 DSP/PCM は T3 NotRun） | NotRun | NotRun |
| SF-XR-02 | RCD-01/03 — CaptureGate と InputMonitor/OldPlayback/DirectMonitor 分離・Punch/Loop 境界・Plugin PDC/ADC/DAC 補正の実 PCM/Raw File 比較 | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-XR-03 | RCD-02/04 — Verified/Partial/Unverified Media・Comp Transition の実 Source Handle と Latency 差・MD-A/B Playback Golden・Raw/Asset 不変 | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-XR-04 | TIM-01/02 — PPQN 変化時 Beat/SourceNative/Comp/MIDI Tick・Note Chase/Transport/Epoch/Graph Mapping 一貫性 | NotRun（#125/#126 依存） | NotRun | NotRun | NotRun | NotRun |
| SF-XR-05 | TIM-03/04 — Plugin Process 合法 Block/Offline Exact vs Degraded・Plugin State 品質/Graph Active/Source Map 独立 Receipt | NotRun（#127/#128 依存 — sv127 並行実装中） | NotRun | NotRun | NotRun | NotRun |

### SF-XG（GUI Cross-product — 2026-11-04 追加）

| caseId | scope | T0 | T1 | T2 | T3 | T4 |
|---|---|---|---|---|---|---|
| SF-XG-01 | GUI 経由 Port 接続・編集/Cancel/Undo→Project 正本/PCM 一致 | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-XG-02 | Graph Publish のみ成功・RT callback なし→Pending で旧 Graph 維持 | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-XG-03 | GraphSwap + Capture Ruviform 未対応→UI Unsupported/Fail Closed・実対応時のみ RT Capture 区間実測 | NotRun | NotRun（RF#308 依存） | NotRun | NotRun | NotRun |
| SF-XG-04 | stale Window/Plugin Port/Graph Generation→誤接続・誤 Param 配送 0 | NotRun | NotRun | NotRun | NotRun | NotRun |
| SF-XG-05 | Legacy v5→new Project/Save/Reload＋PCM Golden・AudioLens 精度/RT 非退行 | NotRun | NotRun | NotRun | NotRun | NotRun |

## Headless evidence rows（本 slice の実測値 — Issue #109 schema）

`caseId | Synvelis SHA | Ruviform resolved SHA | OS/CPU/FS/Device+Plugin |
Graph/Project/Clock generations | fixture & command | expected vs observed |
result | resource/performance/fault | CI/PR link`

Synvelis SHA は全行 `255307e`（base `e78be01` 上の branch
commit）で統一。Ruviform resolved SHA は `0f5b5737…`。
OS/CPU/FS/Device+Plugin は環境ブロックの値で統一（headless
のため Device+Plugin = none）。generations 列は persisted
層のため `project_revision=live CAS`・`graph/clock=N/A
（runtime lane）` と記す。

| caseId | Synvelis SHA | Ruviform resolved SHA | OS/CPU/FS/Device+Plugin | generations | fixture & command | expected vs observed | result | resource/perf/fault | CI/PR |
|---|---|---|---|---|---|---|---|---|---|
| SF-VALID-03/04 | 255307e | 0f5b5737 | Win11 Pro/i9-9900K/NTFS/none | project CAS per op・graph/clock N/A | `cargo test -p synvelis-project --test sf06_oracle sf_valid_03_04` — mix topology 7 strip 全 2^14 mute×solo 組合せ + chain topology 4 strip 2^8×2 variant（b→master edge 無効化含む）で persisted `ControlModel` perturb → `audible_strips` vs `AudibilityOracle::audible` | expected: 全組合せで SUT==oracle（muted source の全 edge 停止・solo upstream supplier ∪ master 到達 downstream・muted 非 relay）。observed: 16384+512 組合せ全て一致 | Pass | sweep ~1s・alloc なし（BTreeMap/Set のみ）・fault: なし | [PR #248](https://github.com/scarecrow173/synvelis/pull/248)（`Refs #109`・branch `scarecrow173/sv109-sf06`） |
| SF-MODEL-03/06 + SF-VALID-01/02 | 255307e | 0f5b5737 | 同上 | 同上 | `cargo test -p synvelis-project --test sf06_oracle sf_model_valid` — fresh/mix/chain/chain-disabled の persisted model に `routing_invariant_violations`（map key==id・master 一意・bus↔strip 1:1・chain/port back-ref・Main 1in/1out・processor 単一 chain・port owner・sidechain=input・output→input・send↔conn 1:1・非 send edge 一意・gain_target・enabled graph 非循環・Mute/Solo 単一権威）+ `validate_references` 併用 | expected: violations 空 + validate_references Ok。observed: 4 fixture 全て空/Ok | Pass | 同上 | 同上 |
| SF-MODEL-03（oracle 自己検証） | 255307e | 0f5b5737 | 同上 | 同上 | `sf_model_valid_oracle_detects_corruption` — orphaned send・第二 master bus・duplicate main edge・duplicate mute 権威・dangling endpoint の 5 corruption を注入 | expected: 各 corruption で非空 violation。observed: 全て検出 | Pass | 同上 | 同上 |
| SF-VALID-07/08 | 255307e | 0f5b5737 | 同上 | 同上 | `cargo test -p synvelis-project --test sf06_oracle sf_valid_07_08` — mix fixture に stale CAS・stale capability generation・feedback cycle・duplicate temp ID・Send-kind AddConnection・empty channel map・第二 master・referenced delete の 8 系統を順に `apply` | expected: 全て Err + live revision/serialized project byte 不変。observed: 全 8 系統で reject・状態不変を確認。末尾の成功 op+undo で project 内容が pre-fault へ正確に復帰（revision +2） | Pass | fault 注入のみ・性能影響なし | 同上 |
| SF-MODEL-05 | 255307e | 0f5b5737 | 同上 | 同上 | `cargo test -p synvelis-project --test sf06_oracle sf_model_05` — 3 track の v5 envelope を `InMemoryStore` 経由で load、宣言 fixture（track 数）から期待投影（strip=track+1・Main edge=track・processor/send 空・各 track→master 直結）を導出して比較。再 load 決定性・save 新 head を検証 | expected: migration 投影が宣言と一致・`c2.live()==c3.live()`・save が新 snapshot head。observed: 全て一致・`live_revision=saved_rev+1`（implicit migration step） | Pass | 同上 | 同上 |
| SF-MODEL-01/02・SF-VALID-08（既存 test の再確認） | 255307e | 0f5b5737 | 同上 | 同上 | `mise run test`（`cargo test --workspace --locked`）— `routing_sf01.rs`（master bootstrap・typed stable ID・watermark・save/reload・undo・orphan send reject・unresolved port・empty map・track delete reject・send gain target・no cycle）と `sf_02.rs`（multi-edit intent 1 CAS/1 undo・pre/post send→return・sidechain・stale revision/capability・typed port rejects・approved map・cycle・rejected intent 不変・preview 非破壊/receipt 決定性・dup temp・adopted gain seal・mixed edits）を含む workspace 全 test | expected: 全 test exit 0。observed: workspace test 全 Pass（本 slice 追加分を含む） | Pass | CI 全量 — 本 slice 追加分のみ上記個別 row に分離 | 同上 |
| SF-ACT-01/04/07・SF-ACT-09・SF-PROBE-01/07・SF-UX-01/05（既存 T2 test の linked evidence） | 255307e | 0f5b5737 | 同上 | 同上 | `mise run test` — `sf03_l1.rs`（lowering deterministic・fingerprint/semantic eq・semantic audio bitexact・two-track mix・strip mute prune・disabled conn not lowered・publish→active receipt・failure preserves last-good）・`sf03_l4.rs`（name-only/undo equivalence）・`sf03_l6.rs`（automation tick on active generation）・`sf04_s3_probe_discontinuity.rs`（seek epoch bump・capture segment cut）・`sf04_project_probe.rs`（boundary resolve・send tap・explicit port agreement・map resolution honesty・unresolved）・`sf05_signal_flow.rs`/`sf_05.rs`（hierarchy 投影・collapse・local scope・role filter・stale・visual_ops 不変・Published≠Active・unattached engine） | expected: 各 slice の case-level 部分が Pass。observed: workspace test 全 Pass — ただし ACT/PROBE/UX の case 全体判定は runtime/device lane 未計測のため matrix では `Pass*`/`NotRun` を維持 | Pass*（partial — case-level 判定は別 gate） | 同上 | 同上 |
| SF-COMPAT-06（lockfile 再現性のみ） | 255307e | 0f5b5737 | 同上 | 同上 | `mise run check`/`test`/`test-rt` — `--locked` で `Cargo.lock` 解決 SHA `0f5b5737` が再現されることを確認 | expected: lockfile-authoritative な解決で build/test exit 0。observed: exit 0（`--locked` 使用） | Pass*（lockfile のみ — API 変更/consumer closure は NotRun） | 同上 | 同上 |
| SF-XR-01（CTL-01/02 cross-gate fixture） | 43511f0 | 0f5b5737 | 同上 | project revision CAS per intent・graph/clock generation = source-map `generation`（7/8）で相関・runtime lane は N/A | `cargo test -p synvelis-player --test sf_xr_01`（`mise run test` workspace 内でも実行）— `sfxr01_01`: 2 track+1 group bus+master の persisted model（strip 1:1・`StripOwner` back-ref・master exactly-one）→ 5-edit `RoutingEditIntent` を 1 CAS commit → `InMemoryStore` save/load JSON+revision 一致 → undo/redo 完全復元 → `lower_semantic` で `semantic_nodes`（strip 一意・bus→strip node 解決・4 distinct）/`send_taps`（PostFader=main-out 一致・PreFader=同 node 別 port）/send connection `Resolved` edge の spec `channel_map`（persisted `[1,0]` dst→src → `(src,dst) [(0,1),(1,0)]`）/`SendGain.gain`==persisted base を照合 → `SetStripMute` で mute strip 接触の全 edge（send 含む）が `NotLowered(MutedStrip)`・`send_taps` 消失・mute target が `ControlOwner::Strip`/`Mute`。`sfxr01_02`: mutated staging clone で `routing_preflight` consent matrix（mismatch+map 無し→requires-decision・map+未承認→requires-decision・map+Approved→staged に send/conn/map/Send-owned SendLevel target・不正 map→unsupported-map）+ commit 経路で未承認 map が revision/project bytes/undo stack を不変に保ち Approved が map を永続化 | expected: 全 persisted 不変条件・serialization/undo・source-map/graph 一致・consent/atomicity が観測通り。observed: 2 test 全 Pass — 上記を全て実測 | Pass（headless のみ — 実 channel 変換 DSP・OS GUI・native plugin・consumer は NotRun） | sweep なし・alloc は test harness のみ・fault: consent reject 系のみ | 本 PR（`Refs #109`・branch `feature/109-sfxr01-oracle`） |

## Linked upstream evidence（再採取ではなく link）

- **Synvelis #18 EX-001〜020** — 再生/graph 受入 matrix。
  Gate A/B/C 判定の正本は #18・#109 §6。headless EX 系 test
  は workspace test で実行済み（`mise run test`）。
- **Ruviform #193 G2/G7 cross-check** — generic graph/probe
  cross-check の正本は Ruviform 側。本 slice は T1 lane を
  再計測せず、resolved SHA `0f5b5737` を link として記録。
- **Ruviform #308**（Graph Swap Probe Capture/Rebind）・
  **#309**（Frame/Clock/Epoch/Latency alignment）— SF-PROBE-04/
  07・SF-XG-03 の依存。未実装の保証を Synvelis adapter で
  代替したとは宣言しない（Issue #109 §6 ルール）。
- **Synvelis #114**（実 GPUI OS GUI）・**#176**（native plugin
  dock）— T3 lane の Owner。本 ledger の headless Pass を
  これらの Pass に換算しない。
- **Synvelis #219/#294**（両 consumer RT/PCM/precision）・
  **Ruviform #219/#294**・**AudioLens** — T4 lane。本 slice は
  Ruviform generic を変更していないため consumer check の
  新規実行は不要（変更があれば両 consumer check・Synvelis #28・
  AudioLens no-impact 根拠が必須 — Issue #109 受入条件）。
- **close 済み generic Ruviform #184/#185** — 本 Gate の拡大
  だけで未実装扱いしない（Issue #109 明記）。

## Pass rules の適用メモ

- 「Failed assertion・stale RT value・queue-loss 未観測・
  反映待ちを Pass へ推論しない」— 本 ledger の `Pass*` は
  persisted/headless 層の部分 Pass であり、runtime/PCM/GUI/
  device/consumer の未計測部分は全て `NotRun` のまま。
- 「No capability なら Unsupported、環境未構築なら NotRun」—
  macOS/Linux・native plugin・audio device・consumer 実機は
  本環境に無いため `NotRun`（`Unsupported` ではない —
  capability 自体は存在するが未計測）。
- 「事前定義された受入能力が Unsupported の場合、製品 Gate
  全体が Pass とはならない」— SF-06 全体の Pass 判定は
  全 lane の evidence が揃うまで保留。本 ledger は headless
  slice の中間証跡であり、Issue #109 の Completed 昇格を
  意味しない。
