---
id: "DESIGN-jrb8PxsJgSO7YaSuwiQUcL"
type: "design"
status: "approved"
title: "Recording / Take / Comping — Session・Take・Comp・Raw Writer・Monitor・Recovery・Source Native Frame の基本設計（synvelis#12）"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/recording-take-comping.md"
  implemented-by: []
  depends-on:
    - "docs/specs/recording-take-comping.md"
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
    - "docs/specs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/specs/rcd-01-device-recording-headless-fixture.md"
    - "docs/specs/rcd-01-real-backend-readiness.md"
    - "docs/specs/rcd-01-real-input-coverage.md"
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
    - "docs/specs/rcd-03-monitor-punch.md"
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/specs/rcd-04-midi-undo-stableid.md"
    - "docs/specs/rcd-04-product-e2e.md"
    - "docs/specs/rcd-04-rt-engine-comp-pcm.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/x-recorded-verified-source-playback-export.md"
    - "docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md"
    - "docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md"
    - "docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/designs/mda-comp-effective-projection-placement-design.md"
    - "docs/designs/midi-source-note-editing-design.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/designs/rcd-04-rt-engine-comp-pcm-design.md"
    - "docs/designs/rcd-03-monitor-punch-design.md"
    - "docs/designs/rcd-03-monitoring-punch-latency.md"
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/recording-take-comping.md"
    - "docs/tasks/recording-take-comping-task.md"
    - "docs/test-specs/recording-take-comping.md"
    - "evidence/sv12/owner-matrix.md"
    - "docs/specs/timeline-editing.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/recording-take-comping.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/117"
    - "https://github.com/scarecrow173/synvelis/issues/118"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/278"
    - "https://github.com/scarecrow173/synvelis/issues/279"
    - "https://github.com/scarecrow173/synvelis/issues/288"
    - "https://github.com/scarecrow173/ruviform/issues/187"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/294"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/117"
    - "https://github.com/scarecrow173/synvelis/issues/118"
    - "https://github.com/scarecrow173/synvelis/issues/114"
---

# Recording / Take / Comping — 基本設計（synvelis#12）

## 概要

Issue #12 の基本アーキテクチャ合意を、実装 Owner が消費できる責務境界・概念 schema・Frame/時間写像・RT/Writer/Recovery/Monitor/Comp/Editor 契約へ落とす。**本設計は契約の固定であり、実装完了を意味しない**。詳細スキーマ・個別アルゴリズム（queue 容量、checkpoint 間隔、comp 既定値等）は Issue #12 が後続 PoC 決定事項と明記しており、本設計でも確定しない。

設計の背骨は 4 つの分離である:

1. **Capture（物理真実）と Placement/Comp（投影）** — Raw Source/Asset は非破壊・append-only。Take/Comp/ClipPlacement/Monitor/Editor はすべてその上の投影または transactional reference であり、raw を書き換えない。
2. **Session/Pass/Stream/Segment/Take の identity 分離** — Pass は transport 巡回、Stream は物理系列、Segment は解決済み時間対応、Take は候補意味、Take Group は Lane/Project 区間の比較集合。Stable ID は相互に独立。
3. **Session Frame / Source Frame / Project Time の分離** — 単調・wrap しない処理時間、Writer 検証済み実保存 PCM、transport/timeline 時間を別 domain として扱い、Segment ごとの mapping でのみ橋渡しする。偽の連続性を作らない。
4. **RT / Non-RT / Project Controller の境界** — RT は bounded emit のみ、handoff は固定容量 queue、Writer は Staging→verified→Journal→Finalize→atomic commit、Recovery は verified/synced frontier を識別する。

## Owner 境界

| 領域 | Owner | 本設計での責務 | 主な既存証拠 |
| --- | --- | --- | --- |
| Recording Session/Pass/Take/Comp 基本契約 | 本 spec chain（#12） | 概念 schema・identity・lifecycle の正本 | VS-A〜VS-D spec |
| 実 Device Capture・同 Clock 複数入力・Raw Fidelity | **#115 RCD-01** | device adapter・arm/capture gate・fail-stop・PCM fidelity | `input_adapter.rs`, `device_recording.rs`, `real_input.rs` |
| Writer/Staging/Journal/Checkpoint/Finalize/Recovery/Asset Manifest | **#116 RCD-02** | durable write・verified frontier・crash recovery・commit 冪等 | `writer.rs`, `journal.rs`, `staging.rs`, `recovery.rs`, `vs_c.rs`, `recovery_review.rs` |
| Monitor/Punch/Loop/Latency・Raw Capture 不変性 | **#117 RCD-03** | monitor plan・punch gate・latency 品質・route 分離 | `monitor.rs`, `monitor_gate.rs`, `monitor_punch_session.rs`, `monitor_arm.rs` |
| Comp/Take Group/Effective Comp・MIDI Phrase・Recovery E2E・Stable ID | **#118 RCD-04** | comp 選択・phrase 評価・project commit・Undo/Redo/Save/Load | `md_a.rs`, `rcd_04_*.rs`, `rcd_04_product_e2e.rs` |
| TakeGroup/Comp 投影・ClipPlacement/Playback | MD-A #42（済） | Effective Comp 投影・placement 時間契約 | `md_a.rs`, `mda-takegroup-comp-*` spec |
| MIDI Source Note 編集・Linked/Independent | MD-B #50・#52（済） | note 編集・source 共有・detach | `midi_source.rs` |
| Parameter Control/Recording Transaction/Control Trace | PC-C1 #98（済） | automation write arm・recording transaction | `pc_c1.rs`, `aut_04_rt.rs` |
| MIDI Phrase Native Tick・chase | TIM-02 #126（済） | native tick 保持・phrase 評価・no-hanging | `tim_02_midi_phrase.rs`, `tim02_midi_*.rs` |
| Recorded Source→Playback/Export | X slice（済） | verified source の playback/export 統合 | `x-recorded-verified-source-playback-export.md` |
| Workspace/View/Selection/Command/Dock | WS-A/ED-A/TE #11（済） | view state・gesture・command routing・link group | `ws_a.rs`, `ed_a.rs`, `uic5*.rs` |
| GUI 製品 E2E・実機 Gate | **#114 SV-GUI-05** | 実 window・実入力・実 OS 受入 | `rcd_04_product_e2e.rs`（headless） |
| Creative Branch Take/Comp 候補 | #278/#279/#288（別件） | Fork/Adopt/Audio Diff — raw を書かない | — |
| Ruviform Generic Device/File/Clock 拡張 | Ruviform #187/#188/#251/#253/#294 等 | device input・clock・MIDI・frame-base capability | `real_input.rs`（probe） |

**境界ルール**:

- Issue #12 が所有するのは **Raw Session/Source/Take Capture/Journal/Recovery の基本契約**。Creative Branch の Fork/Adopt/Audio Diff（#279 CB-02）はこれらを書き込まない。
- Recording/Monitor/Punch/Automation Write が active な状態での Adoption は明示的安全検証が必要で、非対応ケースは reject/defer する（#279 境界）。
- Project Controller CAS/Undo・Asset persistence・Fault Matrix は CB-01 #278・CB-11 #288 の Gate であり、本設計はその手前の recording transaction 境界のみを定義する。
- Ruviform へは Generic Device Input/File/Clock capability のみ要求し、DAW 固有 semantics（Take/Comp/Session）は Synvelis 側に置く。

## 基本契約

### C1. Identity・Lifecycle

各 entity は独立した Stable ID を持ち、相互に代替しない。

| Entity | ID | 所有する意味 | 所有しないもの |
| --- | --- | --- | --- |
| Recording Session | `RecordingSessionId` | Record 要求→Stop/Finalize の全体・複数 Track/Pass | 個別物理系列・候補選択 |
| Recording Pass | `RecordingPassId` | Session 内 1 回の transport 巡回・execution state（Scheduled/Active/Closed）と result（Complete/Partial/Failed/Empty） | Session 全体の lifecycle |
| Capture Stream | `CaptureStreamId` | 1 入力/Lane/Clock Epoch の物理データ系列 | 時間対応の解決 |
| Capture Segment | `CaptureSegmentId` | Session Frame⇔Source Frame⇔Placement⇔Clock/Format⇔整合性の解決済み区間（半開） | 生 PCM の複製 |
| Take | `TakeId` | Lane の録音/Comp 候補・Pass と Segment 参照 | Project への配置確定 |
| Take Group | `TakeGroupId` | Lane/Project 区間の比較可能候補集合 | Session 全体 Pass identity |
| Comp | `CompId` | 非破壊投影の選択モデル | raw Source/Asset の所有 |
| Comp Region | `CompRegionId` | Comp 内の採用区間・非重複 | Source bytes |
| Comp Transition | `CompTransitionId` | 2 有効 Source Handle 間の境界遷移 | PCM の書き換え |

Lifecycle の分離: Session が lifecycle を所有し、Stream が物理系列を所有し、Segment が時間対応を所有し、Take が候補意味を所有し、Source/Asset が非破壊永続化の真実を所有する。

### C2. Frame・時間写像

| Domain | 定義 | 性質 |
| --- | --- | --- |
| Device/Capture Clock・Clock Domain/Epoch | timestamp 起点・sample rate・timestamp 品質 | 異なる domain の直接比較は禁止 |
| Session Frame | 録音処理の単調時刻 | Loop で wrap しない |
| Source Frame | Writer が検証した実保存 PCM の local frame 範囲 | punch-gated/missing 区間は absent/compacted。RT queue 受理だけでは確立しない |
| Project Time | transport/timeline・musical Tick/PPQN960・絶対 MediaTime | Loop で wrap しうる |
| MIDI timestamp | 高精度取得時刻 | 録音時に整数 tick へ強制 quantize しない |

不変条件:

- Segment 範囲は半開 `[start, end)`。Block 途中の Loop/Punch 境界は半開区間で分割し、境界 sample/event は一意に新 Pass/Segment へ帰属する（同一 sample の二重帰属なし）。
- 同一 rate・無 gap の正常 Segment 内では Session duration = Source duration。この不変条件は punch gate-off・writer gap・stream 変更を跨がない。
- Source Frame は **実際に保存・検証された** data のみから確立する。推定・queue 滞留分を含めない。
- TempoMap snapshot・project anchor・loop iteration・mapping policy を Pass/Segment ごとに保持し、写像を後から説明可能にする。
- Ruviform `Tick`/`TickDuration` の unchecked `Add`/`Sub` は直接使わず、Synvelis 側は checked 演算を使う（lint 対象）。
- Punch Gate Off・Writer 欠落区間では Session⇔Source の連続性を偽装しない。Segment 別 mapping のみを信頼する。

### C3. RT Capture・Handoff

- RT Callback は **bounded な Pass/Segment 境界 event のみ emit** する。Project Registry/Undo 変更・動的確保・lock・UI IPC・Project 変更・OS I/O は行わない。
- handoff は **事前確保の固定容量 buffer/queue** と明示的 Filled/Free ownership を使う。queue-full policy は定義済みで、block せず fail-stop する。
- RT queue full・writer stall・free-buffer 枯渇・overflow は RT を block せず fail-stop し、可能なら検証済み prefix を保全する。
- Heap-owned buffer を RT で Drop しない。RT 内に UI 描画・Comp Preview・Media File IO・Project Commit を置かない（Non-RT Prepare→RT-safe Publish）。

### C4. Writer・Staging・Journal・Recovery・Commit

flow: `Staging → verified ranges → Journal Checkpoint → Finalize → Project Controller atomic reference mutation`

- OS write 成功と durable sync は別物。crash recovery は **verified/synced frontier** を識別し、未検証 tail を finalize しない。
- Commit identity により retry を冪等にする。Undo は元 media を即削除しない。
- Stream/Segment 別の Valid/Invalid/Discontinuous 範囲と Journal を比較検証可能にする。
- Journal + Staging + Asset Manifest の再起動後照合で、復旧可能区間を失わず二重登録しない。
- Writer/Checkpoint/Finalize/Project Commit の任意境界で crash/restart しても確認済み media を保護し、二重 Asset/Take/Clip を生成しない。
- マルチ Track 途中障害では Session を Fail-stop し、Track ごとの有効保存範囲と欠落範囲を正確に示す。
- Recovery は Verified/Partial/Invalid と実際の無音を区別し、検証・明示取込・保留・破棄を安全に行う。Gate/Monitor/Writer の故障は独立報告し、復旧データと可聴経路を混同しない。

### C5. Monitor・Punch・Loop・Latency

- Monitor Off/Auto/On・record arm・transport state・punch gate・mute/solo・hardware direct-monitor facts は **raw capture と独立**。Monitoring 状態は raw capture bytes/source span を変えない（同一 PCM・同一 span を複数 monitor state で検証済み）。
- Hardware direct monitoring は外部/device fact であり `DeviceControlled`/`UserReported`/`Unknown` を区別する。Unknown/double monitoring は明示 notice を出し、制御結果を捏造しない。
- Native placement を latency correction として使わない。Latency report は measured/reported/estimated/unknown 品質と clock epoch を区別し、authoritative round-trip を reported/PDC と二重加算しない。未知 clock・stale 測定を sample accurate と表示しない。
- Auto Monitoring の Punch 待機/入力/復帰を表示と可聴経路の両方で検証する（可聴経路は GUI-L2/EXT-L3 Deferred）。
- Capture Gate の Block 途中 In/Out は Sample 単位で決定し、Monitor Gain Ramp は録音 Sample を増減しない。
- Punch 外に隠れた Recording Source は初期既定で存在しない。明示 Handle 取得時も SourceRange/Integrity Validation なしに Crossfade しない。
- 異なる TempoMap 区間で Sample→Project Time 配置を再現し、Native Source Frame を破壊的に変更しない。Software Monitor FX 高 Latency Plugin でも Raw Capture 内容/Placement Evidence は不変。Low-Latency Mode は通常再生/Export Graph を無断変更しない。

### C6. Comp・Take Group・Editor

- Comp は **指定候補区間のみ再生** し、元 Take/Source を非破壊保持する。Comp は source provenance 上の投影であり、合成済み置換 Source ではない。Comp boundary/transition 編集は Comp model のみを変え、PCM と Source Frame は不変。
- Comp Region は互いに非重複。Transition は必要な二つの有効 Source Handle を参照し、handle 不足/欠落/古い revision を拒否する。境界編集で元 Source は不変。
- Grouped Comp は全 Track の同期参照を検証し、一部 Track 不整合で成功対象だけ無言変更しない。切替/復元は単一 Undo Transaction。
- Audio Comp は非破壊投影、MIDI Comp は **event/voice-state 評価**（PCM mixing ではない）。初期方向は phrase/take-level comp、任意 note-level comp は後続。
- MIDI raw events・高精度 timestamp は不変。境界 action は virtual Note Off/controller action を合成しうるが raw event を削除・短縮しない。Note/voice ownership・sustain CC64・controller chase/reset・seek/loop/stop・plugin capability 制限を明示する。全 plugin が Note ID/MPE/MIDI2/Note Expression を完全追跡できる保証はしない。
- `Effective Comp`/`Take Source`/`Compare/Ghost` は別 display mode。編集対象は Comp 設定と明示 Editable Source Version を区別し、Raw Capture version 0 は直接編集不可（derived source version が必要）。View selection・inspection・audition・project edit は分離。
- Gesture target fixation は開始時に ID/revision を固定し、stale 変更を retarget せず拒否する。Arrangement/Audio Editor/MIDI Editor は project provenance を共有するが view-local state は独立。
- Take Click/Time Selection/Audition・Collapsed/Expanded 切替・Effective Comp/Take Source/Compare 切替は Comp 採用・Project Revision/Undo を変更しない。

### C7. Transaction・Session 運用

- Recording Transaction は Project Controller で検証し、Undo/Redo/Save/Load 後も Stable ID と関連性を維持する。
- Track Arm・Input・Monitor・Transport Record・Automation Write Arm は独立に操作し、表示と Capture の実行対象が一致する。録音中の危険な編集競合・二重 Record 要求・複数 Window は安全に扱う（revision 検出・明示 reject）。
- 録音状態は Preflight→Capturing→Stop→Finalizing→Ready を区別し、障害と混同しない。正常終了後は通常の P0 Project 管理に収まり、VCS 未接続でも独立して録音・再生できる。
- Window/Dock/Inspector 移動・リンク/フォーカス変更だけで Record Target・Audition・Active Comp・Project Undo を誤変更しない。

## 概念状態モデル

```
Session (RecordingSessionId)
 ├─ Pass[1..n] (RecordingPassId)         execution: Scheduled→Active→Closed
 │    result: Complete|Partial|Failed|Empty
 │    └─ Stream[1..m] (CaptureStreamId)  per input/lane/clock-epoch
 │         └─ Segment[1..k] (CaptureSegmentId)  half-open [start,end)
 │              session_frame ⇔ source_frame ⇔ placement ⇔ clock/format ⇔ integrity
 ├─ Take[1..t] (TakeId)                  per lane candidate → pass + segments
 │    └─ TakeGroup (TakeGroupId)         per lane/project-range comparable set
 └─ Comp (CompId)                        non-destructive projection
      ├─ CompRegion (CompRegionId)       non-overlapping adopted ranges
      └─ CompTransition (CompTransitionId)  two valid source handles
```

Pass 実行状態（Scheduled/Active/Closed）と結果（Complete/Partial/Failed/Empty）は別軸。Session は lifecycle、Stream は物理系列、Segment は時間対応、Take は候補、Source/Asset は永続化真実を所有する。

## Scenario / Oracle

| Scenario | Oracle |
| --- | --- |
| Normal recording | PCM bit-exact・frame/channel index から検算・Stop→Save→Load→Play で元内容再現 |
| Punch in block 途中 | half-open 分割で境界 sample を一意帰属・gate 外を録音しない・Source⇔Session mapping 正確 |
| Loop 3 周 | Pass 3 件・Session Frame 単調増加・Project 区間同一・各 Pass の Take 参照検証 |
| Queue full / writer stall / disk full | RT 非 block・fail-stop・検証済み prefix 保全・欠落/部分を識別 |
| Crash at checkpoint/finalize/commit 境界 | verified/synced frontier から復旧・二重 Asset/Take/Clip なし |
| Monitor state 切替 | raw capture bytes/span 不変・route 分離・double-monitor notice |
| Comp adopt/paint/boundary edit | region 非重複・transition handle 検証・raw Source 不変・単一 Undo |
| MIDI phrase comp | native tick 保持・virtual boundary action・no stuck note・CC64 chase |
| Stale gesture/revision conflict | fixation ID/revision で拒否・retarget なし |

## Decision Log

| ID | 決定 | 根拠 |
| --- | --- | --- |
| D-01 | Pass execution state と result を別軸にする | Loop/Punch で pass が増えても result 判定を独立させるため |
| D-02 | Source Frame は Writer 検証済み data のみから確立 | RT queue 受理と永続化を混同しないため（偽の連続性を作らない） |
| D-03 | Session⇔Source mapping は Segment 別のみ信頼 | punch/writer gap で連続性を偽装しないため |
| D-04 | Monitor/Latency は raw capture と独立・品質分離 | monitor 操作が録音内容を変えないことを検証可能にするため |
| D-05 | Comp は投影・境界編集は Comp model のみ | raw PCM/Source Frame を不変に保つため |
| D-06 | MIDI Comp は phrase/take-level を初期方向 | 任意 note-level は後続・raw event 不変 |
| D-07 | Creative Branch Take/Comp 候補は #279/#278/#288 へ分離 | Issue #12 境界・raw を Fork/Adopt/Audio Diff が書かないため |
| D-08 | 詳細 schema/数値は後続 PoC へ defer | Issue #12 の明示的範囲・実装 Owner が fixture で確定するため |

## 検証戦略

- **headless/unit fixture**（既存）: `vs_b.rs`/`vs_c.rs`/`vs_d.rs`/`device_recording.rs`/`input_adapter.rs`/`monitor_gate.rs`/`monitor_punch_session.rs`/`recovery_review.rs`/`md_a.rs`/`rcd_04_*.rs`/`tim_02_*.rs` 等で契約を実証。`Pass (headless)` は headless/unit の実証であり製品 GUI・実機 Pass ではない。
- **RT contract**: alloc/drop-free・no-block・bounded emit を計測（`vs_b_rt_alloc.rs`/`input_adapter_rt_alloc.rs`/`aut_04_rt.rs`）。
- **process-level crash**: `recording-poc` の `vsc_process.rs`/`vsd_process.rs` で kill→recover を実証。
- **OS Gate 分類**: `POLICIES.md` の OS-L1/GUI-L2/EXT-L3 に従い、実 device/GUI/plugin は `NotRun`/`Deferred` として `evidence/sv12/owner-matrix.md` に記録。Fake/Headless 成功を実機 Pass に換算しない。
- **本 slice の検証**: docs-only のため `mise run fmt-check` と doc-maintenance index 再生成・doc-status audit が対象。Rust build/test は `NotRun`/`NotApplicable`。

## 未解決・後続へ委譲

- queue 容量・checkpoint 間隔・comp 既定値・UI 配置等の数値 → 後続 PoC/実装 Owner。
- 実 Device・実 OS GUI・実 Plugin・実 loopback latency → #114・EXT-L3・Ruviform capability Issues。
- Independent Device Clock の正確さ → #119（RCD-F01）別件。
- Creative Branch Take/Comp 候補 → #278/#279/#288。
- Take Lane/Comp Editor の UI 詳細 → #112/#114・#12 §14/§15 後続設計。
