---
id: "SPEC-QIpUDjWLLusOo4TEiv6CjZ"
type: "spec"
status: "approved"
title: "Recording / Take / Comping — Session・Take・Comp・Raw Writer・Monitor・Recovery・Source Native Frame の基本契約（synvelis#12）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
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
  implements: []
  implemented-by: []
  depends-on:
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
    - "docs/specs/workspace-model.md"
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/specs/timeline-editing.md"
    - "docs/specs/x-recorded-verified-source-playback-export.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/recording-take-comping-design.md"
    - "docs/plans/recording-take-comping.md"
    - "docs/tasks/recording-take-comping-task.md"
    - "docs/test-specs/recording-take-comping.md"
    - "evidence/sv12/owner-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/3"
    - "https://github.com/scarecrow173/synvelis/issues/4"
    - "https://github.com/scarecrow173/synvelis/issues/5"
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/8"
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/13"
    - "https://github.com/scarecrow173/synvelis/issues/14"
    - "https://github.com/scarecrow173/synvelis/issues/15"
    - "https://github.com/scarecrow173/synvelis/issues/16"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/18"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/36"
    - "https://github.com/scarecrow173/synvelis/issues/38"
    - "https://github.com/scarecrow173/synvelis/issues/42"
    - "https://github.com/scarecrow173/synvelis/issues/50"
    - "https://github.com/scarecrow173/synvelis/issues/52"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/95"
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/110"
    - "https://github.com/scarecrow173/synvelis/issues/111"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/113"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/117"
    - "https://github.com/scarecrow173/synvelis/issues/118"
    - "https://github.com/scarecrow173/synvelis/issues/119"
    - "https://github.com/scarecrow173/synvelis/issues/120"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/synvelis/issues/134"
    - "https://github.com/scarecrow173/synvelis/issues/135"
    - "https://github.com/scarecrow173/synvelis/issues/136"
    - "https://github.com/scarecrow173/synvelis/issues/137"
    - "https://github.com/scarecrow173/synvelis/issues/138"
    - "https://github.com/scarecrow173/synvelis/issues/139"
    - "https://github.com/scarecrow173/synvelis/issues/145"
    - "https://github.com/scarecrow173/synvelis/issues/147"
    - "https://github.com/scarecrow173/synvelis/issues/148"
    - "https://github.com/scarecrow173/synvelis/issues/151"
    - "https://github.com/scarecrow173/synvelis/issues/157"
    - "https://github.com/scarecrow173/synvelis/issues/166"
    - "https://github.com/scarecrow173/synvelis/issues/167"
    - "https://github.com/scarecrow173/synvelis/issues/176"
    - "https://github.com/scarecrow173/synvelis/issues/184"
    - "https://github.com/scarecrow173/synvelis/issues/187"
    - "https://github.com/scarecrow173/synvelis/issues/188"
    - "https://github.com/scarecrow173/synvelis/issues/230"
    - "https://github.com/scarecrow173/synvelis/issues/251"
    - "https://github.com/scarecrow173/synvelis/issues/253"
    - "https://github.com/scarecrow173/synvelis/issues/278"
    - "https://github.com/scarecrow173/synvelis/issues/279"
    - "https://github.com/scarecrow173/synvelis/issues/288"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/307"
    - "https://github.com/scarecrow173/synvelis/issues/313"
    - "https://github.com/scarecrow173/synvelis/issues/314"
    - "https://github.com/scarecrow173/synvelis/issues/365"
    - "https://github.com/scarecrow173/synvelis/issues/368"
    - "https://github.com/scarecrow173/ruviform/issues/185"
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/187"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/190"
    - "https://github.com/scarecrow173/ruviform/issues/196"
    - "https://github.com/scarecrow173/ruviform/issues/243"
    - "https://github.com/scarecrow173/ruviform/issues/251"
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/307"
    - "https://github.com/scarecrow173/ruviform/issues/313"
    - "https://github.com/scarecrow173/ruviform/issues/314"
    - "https://github.com/scarecrow173/ruviform/issues/388"
    - "https://github.com/scarecrow173/ruviform/issues/389"
---

# Recording / Take / Comping — 基本契約（synvelis#12）

## What

Issue #12 は、Synvelis の Recording / Take / Comping の **基本アーキテクチャを定義する設計 Issue** である。合意済みの基本契約として、以下を本 spec で固定する。

- **Recording Session** — Record 要求から Stop/Finalize までの全体。複数 Track・複数 Pass を含みうる。
- **Recording Pass** — Session 内の 1 回の transport 巡回。Loop 反復ごとに新 Pass。通常録音は 1 Pass、Punch In/Out は Capture Gate の Segment を変えるが Pass は変えない。
- **Capture Stream** — 1 入力/Lane/Clock Epoch の物理データ系列。
- **Capture Segment** — Session Frame・Source Frame・Placement Map・Clock/Format・整合性を結ぶ解決済み区間（半開 `[start, end)`）。
- **Take** — Lane の録音/Comp 候補。通常 Pass と 1 つ以上の Capture Segment を参照する。
- **Take Group** — Lane/Project 区間が所有する比較可能な候補集合。Session 全体の Pass identity とは別物。
- **Comp / Comp Region / Comp Transition** — Source を非破壊に投影する選択モデル。Comp は raw Source/Asset を所有・書き換えない。
- **Stable ID の分離** — `RecordingSessionId`/`RecordingPassId`/`CaptureStreamId`/`CaptureSegmentId`/`TakeId`/`TakeGroupId`/`CompId`/`CompRegionId`/`CompTransitionId` は互いに別 identity。
- **Frame/時間の分離** — Device/Capture Clock・Clock Domain/Epoch、Session Frame（単調・Loop で wrap しない）、Source Frame（Writer が検証した実保存 PCM の local frame）、Project Time（transport/timeline・musical Tick/PPQN960・Loop で wrap しうる）、MIDI timestamp（高精度・録音時に整数 tick へ強制 quantize しない）。
- **RT/Writer/Recovery 境界** — RT Callback は bounded な境界 event のみ emit し、Project Registry/Undo 変更・動的確保・lock・UI IPC・Project 変更・OS I/O を行わない。handoff は事前確保の固定容量 buffer/queue と明示的 Filled/Free ownership。Queue full・writer stall・free-buffer 枯渇・overflow は RT を block せず fail-stop し、可能なら検証済み prefix を保全する。
- **Writer flow** — Staging → verified ranges → Journal Checkpoint → Finalize → Project Controller atomic reference mutation。OS write 成功と durable sync は別物で、crash recovery は verified/synced frontier を識別する。Commit identity で retry を冪等にし、Undo は元 media を即削除しない。
- **Monitor/Latency** — Monitor Off/Auto/On・record arm・transport state・punch gate・mute/solo・hardware direct-monitor facts は raw capture と独立。Monitoring 状態は raw capture bytes/source span を変えない。Latency は measured/reported/estimated/unknown と clock epoch を区別し、配置補正は native placement として Raw Source を破壊しない。
- **Editor/Comp** — `Effective Comp`/`Take Source`/`Compare/Ghost` は別 display mode。編集対象は Comp 設定と明示 Editable Source Version を区別し、Raw Capture version 0 は直接編集不可。gesture は開始時に ID/revision を固定し stale 変更を拒否する。

**重要な位置づけ**: Issue #12 は「設計合意済み（基本アーキテクチャ）／詳細スキーマ・個別アルゴリズムは後続 PoC で確定。実装済みを意味しない」と明記されている。本 spec chain と `evidence/sv12/owner-matrix.md` は、実装 Owner（#115 RCD-01・#116 RCD-02・#117 RCD-03・#118 RCD-04）と既存 fixture が消費できる契約・受入 Owner Matrix を固定するものであり、実装・実 OS・実 Device・実 Plugin の受入を主張しない。

## Why

Synvelis の録音系は、VS-A〜VS-D（#13〜#16、closed）と RCD-01〜RCD-04（#115〜#118）の各 slice が個別に headless fixture を実装してきた。しかし **Session/Pass/Stream/Segment/Take/Take Group/Comp の概念 schema、Clock Epoch/Session Frame/Source Frame/Project Time の分離、RT/Writer/Recovery の責務境界、Monitor/Latency/Comp/Editor の製品契約**を一枚の正本として固定した文書と、**69 件の受入条件を実装 Owner Issue・既存 fixture・OS Gate へ対応付ける Owner Matrix** が存在しなかった。Issue #12 §13 の 69 受入項目は、実装済み証拠・headless 証拠・実機未検証を混同せず分類する必要がある。

本 spec はその欠落を埋め、後続実装 slice が「何を・誰が・どの検証水準で」証明すべきかを一意に参照できるようにする。

## Users and Value

- **DAW ユーザー**: 録音→Take/Comp→編集→保存→復旧の全経路で、raw 録音が非破壊に保たれ、Take/Comp/Placement が独立に検証・復元できる。
- **後続実装 Owner（#115〜#118）**: 契約・Fixture・Case ID・OS Gate 分類から実装・検証できる。
- **Coordinator/レビュアー**: 「設計合意」「headless fixture 成功」「製品受入済み」「実機受入済み」を区別できる証拠分類を得る。

## Acceptance Criteria

Issue #12 §13 の受入条件を AC-01〜AC-69 として固定する。各行の現在の被覆状況は `evidence/sv12/owner-matrix.md` の台帳を正本とし、本 spec は要求のみを定義する。`NotRun`/`Deferred`/`Unsupported` は POLICIES.md の実行結果・要件状態語彙に従う。

### 録音内容・同期・時間（AC-01〜AC-05）

- [ ] **AC-01**: Audio 録音した PCM と Sample Frame を保持し、Stop→Save→Load→Play で元内容が再現できる。
- [ ] **AC-02**: MIDI Note/CC/Pitch 等を高精度 Timestamp で取得し、録音時の意図しない Quantize を行わない。
- [ ] **AC-03**: 同 Clock の複数入力を同時記録して Track 間の相対同期が保持され、未 Arm には録音されない。
- [ ] **AC-04**: Count-in/Pre-roll/Punch Gate 境界で有効 Sample/Event の重複・欠落・範囲外上書きがない。
- [ ] **AC-05**: Loop 録音で Source の物理位置が単調増加し、各周回 Take は同 Project 区間へ対応する。

### Monitor・Latency（AC-06〜AC-07, AC-33, AC-49〜AC-56）

- [ ] **AC-06**: Off/Auto/On と Hardware Direct Monitoring の区別、入力と既存 Playback の切替が検証できる。
- [ ] **AC-07**: 報告/推定/不明 Latency を区別し、配置補正が Raw Source を破壊しない。
- [ ] **AC-33**: Auto Monitoring の Punch 待機/入力/復帰を表示と可聴経路両方で検証し、Direct Monitoring を制御済みと偽装しない。
- [ ] **AC-49**: Arm ON/Monitor Off/Record Gate Open でも正しい Input Source Frame を録音し、Software Monitor Off が記録を止めない。
- [ ] **AC-50**: Arm ON/Monitor Auto/停止・再生・Count-in・Pre-roll・Punch 待機/Gate Open/Post-roll の初期状態表を UI/Audio Path 両方で検証する。
- [ ] **AC-51**: Capture Gate の Block 途中 In/Out が Sample 単位で決定され、Monitor Gain Ramp で録音 Sample の追加/削除がない。
- [ ] **AC-52**: Monitor On/Off/Auto と Existing Playback、他 Track、Hardware Direct の状態を区別し、意図しない二重監視へ注意を出す。
- [ ] **AC-53**: Software Monitor FX 高 Latency Plugin でも Raw Capture 内容/Placement Evidence は不変で、Low-Latency Mode が通常再生/Export Graph を無断変更しない。
- [ ] **AC-54**: ADC/DAC timestamp と Callback/Reported Latency の重複計算がなく、未知 Clock と Stale 測定を sample accurate と表示しない。
- [ ] **AC-55**: 異なる TempoMap 区間で Sample→Project Time 配置を再現し、Native Source Frame を破壊的に変更しない。
- [ ] **AC-56**: 外部 Hardware Direct が User-reported/Unknown のとき制御結果を捏造しない。

### Comp・Editor・Undo（AC-08, AC-20, AC-24, AC-27〜AC-32, AC-38〜AC-48）

- [ ] **AC-08**: Comp は指定候補区間のみ再生し、元 Take/Source を非破壊保持する。Crossfade と MIDI Note-off が安全に処理される。
- [ ] **AC-20**: Comp Region は互いに非重複で、Transition が必要な二つの有効 Source Handle を参照し、境界編集で元 Source は不変。
- [ ] **AC-24**: Native Source Frame は録音後の TempoMap/Comp 編集でも変更されず、Clip 側 Placement/Mapping で解決する。
- [ ] **AC-27**: 空 Lane 初回正常録音で Clip 自動生成、既存 TakeGroup 追録/Loop/Punch は既存 Comp を暗黙変更せず元 Source を破壊しない。
- [ ] **AC-28**: Collapsed/Expanded Take Lane 切替で Active Comp と Undo 履歴が不変。
- [ ] **AC-29**: Take Click/Time Selection/Audition は Comp 採用を変更せず、Audition 終了で元状態へ復帰する。
- [ ] **AC-30**: 明示 Adopt/Comp Paint のみ Comp Region を変更し、Drag Preview 中は Project 不変、Esc/Cancel で変更なし、Commit は全件 Validation・単一 Undo。
- [ ] **AC-31**: Region Boundary と Transition Handle の操作・Target/Revision 分離で、Source Handle 不足/欠落/古い Revision を正しく拒否・表示する。
- [ ] **AC-32**: Grouped Comp の一部 Track 不整合で成功対象だけ無言変更せず、複数 Track 同時を単一 Undo Transaction で扱う。
- [ ] **AC-38**: Comp Clip を MIDI Editor で開き、複数 Take の Note/CC 表示でも Source 正本は不変。
- [ ] **AC-39**: Effective Comp/Take Source/Compare 切替、Take Click、Audition で Project Revision/Undo が増えない。
- [ ] **AC-40**: Source 編集は Editable Source Version 明示 Binding 必須で、Raw Capture を直接変更しない。
- [ ] **AC-41**: Source-local Audio 編集と Comp Region/Transition 編集、Arrangement Clip Move/Trim が Command/Target/Undo で区別される。
- [ ] **AC-42**: Comp Region 境界を跨ぐ MIDI Note On/Off の Raw Timestamp が不変で、再生時に Virtual Release/State 遷移が生成される。
- [ ] **AC-43**: CC64 On 状態切替で不必要な音の持続が残らず、採用後 Note/Controller イベントが決定的順序で再生される。
- [ ] **AC-44**: Seek/Stop/Loop/Comp 切替時の Voice ownership/CC state を Mock Plugin で検証し、未対応 Plugin 能力は制限を明示する。
- [ ] **AC-45**: MIDI Comp 編集後の Undo/Redo、Save/Load で採用 Region と Event/Controller 再生結果が一致する。
- [ ] **AC-46**: Independent/Linked MIDI Source 編集伝播が意図どおりで、録音 Source を誤上書きしない。
- [ ] **AC-47**: 複数 Window による Source/Comp 編集競合を Revision で検出し、入力 Gesture の Target を無断切替しない。
- [ ] **AC-48**: Audio/MIDI Comp 表示と Audio/Plugin RT 処理が Project Controller/UI 描画・NonRT Source 生成でブロックされない。

### Transaction・Session 運用・復旧（AC-09〜AC-14, AC-19, AC-22〜AC-23, AC-25〜AC-26, AC-34〜AC-37, AC-57〜AC-58, AC-65〜AC-67）

- [ ] **AC-09**: Recording Transaction は Project Controller で検証し、Undo/Redo/Save/Load 後も Stable ID と関連性が維持される。
- [ ] **AC-10**: 1 Session 複数 Track、二重 Record 要求、複数 Window、録音中の危険な編集競合を安全に扱う。
- [ ] **AC-11**: RT Callback の no-alloc/no-block/no OS I/O を Mock と計測で確認する。
- [ ] **AC-12**: Queue Overflow、Disk Full、Writer pause/crash、Device Disconnect、Project Commit 失敗、アプリ強制終了を注入し、欠落/部分録音を識別できる。
- [ ] **AC-13**: Journal + Staging + Asset Manifest の再起動後照合で、復旧可能区間が失われず、二重登録されない。
- [ ] **AC-14**: 正常終了後は通常の P0 Project 管理に収まり、VCS 未接続でも独立して録音・再生できる。
- [ ] **AC-19**: Loop 境界を跨ぐ MIDI Note On/Off が元 Timestamp を保ち、再生で stuck note を発生させない。
- [ ] **AC-22**: マルチ Track 途中障害で Session Fail-stop し、Track ごとの有効保存範囲と欠落範囲を正確に示す。
- [ ] **AC-23**: Checkpoint/Media Finalize/Project Commit 前後の異常終了で復旧可能データを維持し、再試行で二重 Asset/Take/Clip を生成しない。
- [ ] **AC-25**: Track Arm、Input、Monitor、Transport Record、Automation Write Arm を独立に操作し、表示と Capture の実行対象が一致する。
- [ ] **AC-26**: 録音状態 Preflight→Capturing→Stop→Finalizing→Ready の各状態表示が障害と混同しない。
- [ ] **AC-34**: Recovery UI が Verified/Partial/Invalid と実際の無音を区別し、復旧検証・明示取込・保留・破棄を安全に行う。
- [ ] **AC-35**: Recovery 取込再試行で二重生成せず、他 Window Project Edit との Revision 競合を正しく解決/表示する。
- [ ] **AC-36**: Window/Dock/Inspector 移動・リンク/フォーカス変更だけで Record Target、Audition、Active Comp、Project Undo が誤変更されない。
- [ ] **AC-37**: RT 内で UI 描画/Comp Preview/Media File IO/Project Commit を行わず、非 RT Prepare→RT-safe Publish を守る。
- [ ] **AC-57**: 初期既定で Punch 外に隠れた Recording Source が存在せず、明示 Handle 取得時も SourceRange/Integrity Validation なしに Crossfade しない。
- [ ] **AC-58**: Recording Gate/Monitor/Writer のいずれかの故障を独立報告し、復旧データと現在の可聴経路の状態を混同しない。
- [ ] **AC-65**: 保存済 Stream/Segment 別の Valid/Invalid/Discontinuous 範囲と Journal を比較して検証可能。
- [ ] **AC-66**: Writer/Checkpoint/Finalize/Project Commit 任意境界で Crash/Restart しても確認できた Media を保護し、二重 Asset/Take/Clip を生成しない。
- [ ] **AC-67**: Project Save/Reload と Session Undo/Redo で ID、Source Placement、元 Media 参照が正しく復元される。

### 検証・証拠（AC-15〜AC-18, AC-21, AC-59〜AC-64, AC-68〜AC-69）

- [ ] **AC-15**: 4/4・120BPM・48kHz・4 小節 Loop を 3 周すると Pass が 3 件でき、Session Frame が `[0,384000)`, `[384000,768000)`, `[768000,1152000)` と増加し、Project 区間は同一。
- [ ] **AC-16**: 8 Track 同時録音で各 Take が対応 Pass を参照し、複数入力の Clock/Frame 同期が検証可能。
- [ ] **AC-17**: Punch/Loop 境界が Block 途中に来ても、半開区間処理により同一 Sample/Event を二重帰属しない。
- [ ] **AC-18**: Punch の Gate Off や Writer の欠落区間があっても、Session Frame⇔Source Frame は Segment 別の正しい Mapping となり、偽の連続性を作らない。
- [ ] **AC-21**: Grouped Comp は全 Track の同期参照を検証し、単一 Undo Transaction で切替/復元する。
- [ ] **AC-59**: Mock Input/Output Clock・Device timestamp・FX delay と Loopback 実測を段階的に検証し、Backend 別能力と Fallback/Reject 条件を記録する。
- [ ] **AC-60**: Normal 録音で Frame Index/Channel ID から検算できる PCM 全区間一致、Sample 重複/欠落/並べ替えがない。
- [ ] **AC-61**: Punch In/Out が Block 途中でも Gate 外を録音せず、正確な SourceFrame⇔SessionFrame mapping を保持する。
- [ ] **AC-62**: Loop 3 Pass の Project 時間 Wrap と SessionFrame 単調増加が一致し、各 Pass の Take 参照と Project 登録を検証する。
- [ ] **AC-63**: Queue Full/Free Block 枯渇/Writer stall で RT を Block せず、Heap-owned buffer を RT で Drop せず、Fault と Partial Recovery を判定する。
- [ ] **AC-64**: 2ch+/Multi-track で同 Clock 内 Frame 整合性を保持し、Independent Device Clock の正確さは主張しない。
- [ ] **AC-68**: Counter/Instrumentation と RT Contract Test で process 中の alloc/blocking/OS I/O/危険 Drop を検出し、Mock 合格を全実 Backend 保証としない。
- [ ] **AC-69**: 実施前に GitHub の Project/Recording/RT 契約との API 適合チェックを行い、必要な Ruviform 拡張は #187/#188 等へ関連付ける。

## Non-goals

- Recording/Take/Comping の**製品実装** — 実装 Owner は #115（RCD-01 実 Device Capture）・#116（RCD-02 Writer/Journal/Recovery）・#117（RCD-03 Monitor/Punch/Latency）・#118（RCD-04 Comp/MIDI Phrase/Recovery E2E）。本 spec は契約のみを定義する。
- Creative Branch の Take/Comp 候補（Fork/Adopt/Audio Diff）— Synvelis #279（CB-02）・#278（CB-01）・#288（CB-11）の別作業。Raw Session/Source/Take Capture/Journal/Recovery は Issue #12 が所有し、Fork/Adopt/Audio Diff は書き込まない。
- Project Controller CAS/Undo・Asset persistence・Fault Matrix の CB-01/CB-11 Gate — #278/#288 の責務。
- Ruviform Generic Device Input/File/Clock を DAW 固有仕様へ変更すること — Ruviform 拡張要求は Ruviform Issue（#187/#188/#251/#253/#294 等）として関連付ける。
- 詳細スキーマ・個別アルゴリズム（queue サイズ数値、checkpoint 間隔、comp 既定値、UI 配置）— Issue #12 が明記する後続 PoC 決定事項。本 spec は契約（分離・不変条件・拒否）のみを固定する。
- Recording 中の Automation Write・Parameter Control Transaction — #98（PC-C1）系の責務。本 spec は Arm 独立性と競合拒否の境界のみを定義する。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI/本番 crate の変更 — 実装 Owner の独立 PR の責務。
- 実 Window/OS GUI・実機・実 Device・実 Plugin の受入 — #114（SV-GUI-05）・EXT-L3 の別受入 Gate。Fake/Headless/Offscreen の成功を実機 Pass と換算しない。

## Sources and Relations

- Issue #12 本文（基本アーキテクチャ合意・概念 schema・Frame/時間分離・RT/Writer/Recovery/Monitor/Comp/Editor 契約・受入条件 69 項・Owner split・phase R0〜R7・mock-first vertical slice 条件・変更境界）
- `docs/designs/recording-take-comping-design.md`（設計正本: Owner 境界・概念 schema・Frame/時間写像・RT/Writer/Recovery 設計・Monitor/Latency/Comp/Editor 決定・Scenario/Oracle・Decision Log）
- `docs/test-specs/recording-take-comping.md`（Fixture 契約: Feature/Rule/Example/Guarantee）
- `evidence/sv12/owner-matrix.md`（69 受入条件 × 実装 Owner Issue × 既存 fixture × OS Gate の台帳）
- 上流契約 spec: VS-A #13（Session/Pass/Take contract）、VS-B #14（audio capture/frame/clock boundary）、VS-C #15（writer/journal/recovery）、VS-D #16（commit E2E/idempotency/undo/reload）、RCD-01 #115・RCD-02 #116・RCD-03 #117・RCD-04 #118（実装 slice）、MD-A #42（TakeGroup/Comp/Effective Comp 投影・ClipPlacement/Playback）、MD-B #50（MIDI Source Note 編集）、PC-C1 #98（parameter control/recording transaction）、TIM-02 #126（MIDI phrase native tick）、X slice（recorded source playback/export）
- 関連 Issue: #2（Workspace Model）、#3（共通 UX/Link Groups）、#4/#5/#6/#7/#8/#9（Context Inspector/Automation/Visual Diff/Workflow/Asset Library）、#10（Track/Lane/Clip/Source/Timeline）、#11（Timeline Editing）、#17/#18/#19/#20（可変 PPQN・Time/Transport）、#36/#38/#42/#50/#52/#65（WS-A/ED-A/MD-A/MD-B/MIDI source/ED-B）、#84（StagedFile）、#95（VCS）、#105（SF-02）、#106/#110/#111/#112/#113/#114（GUI/製品 E2E）、#115〜#118（実装 Owner）、#119/#120/#121/#122（RCD-F01・control owner/time scope）、#126/#127（MIDI 永続化/future）、#131〜#139（automation modes/region replace/loop write/curve LOD/later）、#145/#147/#148/#151/#157/#166/#167/#176/#184/#230/#251/#253/#294/#307/#313/#314/#365/#368（CI/cache/impact/matrix/real plugin/GitHub tracker/evidence/stale vendor/index/missing-section）
- Ruviform 側参照: ruviform#185/#186/#187/#188/#189/#190/#196/#243/#251/#253/#294/#307/#313/#314/#388/#389（Probe・parameter precision・input/clock・MIDI・musical time・transport・plugin precision・GUI/device/fake-driver/frame-base capability）
