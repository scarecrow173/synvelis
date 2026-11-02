---
id: "SPEC-034PCA01Kf8vNq2wXyZmAb"
type: "spec"
status: "approved"
title: "PC-A Parameter Control Model — Scope/Breakdown/StateMachine headless PoC"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/40"
    - "https://github.com/scarecrow173/synvelis/issues/6"
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
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md"
    - "docs/designs/pc-a-parameter-control-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/185"
  defers: []
  deferred-by: []
---

# PC-A Parameter Control Model — Scope/Breakdown/StateMachine headless PoC

## What Should Be Built

Issue #40: #6 (Automation/Modulation 統合 Parameter Control) で合意済みの
基本モデル + 4 決定を、#36/#38 が確立した `synvelis-workspace` +
`synvelis-project` 上に **headless PoC** として実装し、合意の各契約が
不変条件として守られることを headless test で検証する。Ruviform 側の
param control events (rf#186: `ParamInfo`/`ParamFlags`/`ParamValue`/
`ParamWriteMeans`/`check_param_write`/`ParamEvent`/`TimedParamEvent`/
`EventList`)、musical time (rf#189: `BeatPosition`/`Ppqn`/`TickPosition`/
`TempoMap`/`Rounding`)、timed input chase (rf#188: `InputStamp`/
`ClockAnchor`/`map_stamp_to_frame`/`EngineFrame`)、probe 観測語彙
(rf#185: `TickTimestamp`/`MediaTimestamp`/`TimestampProvenance`) を消費
し、DAW 固有の Automation/Modulation 意味は synvelis に保持する。

## Why Now

#6 の合意は 4 決定 (Scope/合成規則/Breakdown/状態機械) + 論理モデル +
品質要件まで確定しており、WS-A が単一 Project 正本・Command・Link の
基盤を、ED-A が Gesture/Transaction/Undo の型を確立した。Parameter
Control Model はそれらの直上に載る検証対象であり、real-GUI lane や実
Plugin adapter へ渡す前に「一つの正本・Scope 分離・競合検出・Value
Breakdown・記録状態機械」を契約として固定しておく必要がある。

## Users and Value

- DAW ユーザー: 同じ Parameter が Arrangement Lane・Signal Flow・
  Inspector で矛盾なく表示され、Clip Envelope が Track Automation を
  黙って上書きせず、Touch/Latch/Write の記録が予期せぬ上書きや二重
  Modulation を起こさない。
- 開発者: Parameter 制御の正本・評価規則・競合検出・記録 Transaction
  が契約としてテスト固定され、後続の real-GUI・実 adapter・Bake
  Modulation の基礎になる。

## Scope

### In Scope

1. **Parameter Control Model 正本**: `ParameterTarget`/
   `AutomationLane`(Binding)/`ModulationSource`/`ModulationAssignment`/
   `ControlEvaluation`/`ControlObservation` の論理モデルを
   `synvelis-project` の Project 集約へ追加。Lane・SignalFlow・
   Inspector の 3 投影は同一正本の読み取り関数であり二重の系列を
   持たないことを test で固定する。
2. **所有 Scope 分離**: `ControlScope::{Track, Clip, Note, Project}` と
   時間座標の明示。Track/Project scope は Project 音楽時間
   (`BeatPosition`)、Clip-local は Clip 相対 Beat、Note-local は
   Note 相対 Beat。Clip Envelope は Clip-scoped target にのみ作用し、
   Track Volume Automation を暗黙上書きしない。Automation point は
   `BeatPosition` 有理数で保持し、PPQN 変更で黙って quantize され
   ない。
3. **競合検出**: 同一 ParameterTarget・同一評価時刻に重複する
   Automation region を暗黙優先順位で潰さず、`AutomationConflict`
   として明示検出し、caller が `ConflictPolicy::{Reject, Replace}` を
   明示して解決する。重複のない複数 region は同一論理系列へ包含
   される。
4. **Value Breakdown**: `ControlEvaluation` が Base/Automation/
   Reference/各 Assignment 寄与 (source・transform・depth・寄与値)/
   合計/Clamp・量子化/Host-resolved (normalized + plain)/評価時刻・
   tempo map revision を出所付きで返す。`ControlObservation` は
   Plugin-reported・Probe 実測・測定 timestamp + provenance を持ち、
   `AtPlayhead`/`Live`/`Captured`/`Stale`/`Unavailable` を区別する。
5. **状態機械**: UI Mode `Off/Read/Touch/Latch/Write` は session
   state (`ControlSession`) であり、内部の `PlaybackState`/`WriteMode`/
   `WriteArm`/`GestureState`/`RecordingTransaction` と分離される。
   **mode 選択だけでは Project を更新しない。**
   - Off: Automation 停止・manual gesture は Base を編集 (undo 対象の
     通常編集)。
   - Read: 操作中のみ手動優先、解放で既存 Automation へ復帰、書込み
     なし。
   - Touch: Gesture 区間のみ書込み (Arm+Automation Record+Transport
     成立時のみ)、終了で復帰。
   - Latch: Release 後も最終値で書込み継続、Transport/Record 停止
     または Mode 変更で終了。次回再生に記録を持ち越さない。
   - Write: 明示 Arm+Record+Transport 成立対象のみ、記録開始時の
     Reference 基準値から継続書込み。
   - **記録は Modulation 合成前の Manual/Reference 値のみ** —
     Host-resolved の暗黙書き戻し・二重適用を禁止。
   - 1 録音 session = 1 Project Undo transaction。Loop Record は
     loop wrap で recording を明示 seal し、同区間を黙って上書き
     しない。
   - Command 発行時に Target/Revision/Arm を fixation として確定 —
     Inspector Follow 変化で Target がすり替わらない。
6. **Control Graph**: Source→Assignment→Target (+Assignment depth を
   別 Target が駆動する階層 modulation) の DAG を非 RT で compile・
   検証し、無遅延循環を `ControlCycle` で拒否。評価順序は topo 順に
   固定 (順序独立の決定的加算)。control-rate を基本とし、RT 側は
   固定順序の有界評価のみ (PoC は headless 決定評価で検証)。
7. **Capability 尊重**: `ParamFlags` (AUTOMATABLE/MODULATABLE/STEPPED/
   READONLY) と synvelis `ValueKind` (Continuous/Stepped/Boolean/
   Choice) の両方を尊重。Read-only は write/base 編集を拒否、
   非 AUTOMATABLE は TimedEvent を拒否 (BlockBoundary へ明示
   degrade)、Boolean/Choice/Stepped への連続 modulation は量子化
   規則なしに許可しない。非線形写像 (`ParamMapping`) では Depth が
   normalized domain の増分であり plain 増分と同義でないことを
   検証する。
8. **時間整合**: Automation point は `BeatPosition` 保持で tempo
   change に不変、tick↔beat は `TickPosition`/`Ppqn` 経由の明示変換、
   clip-local は clip anchor からの明示写像、loop 境界・offline
   render と再生の評価一致を同一 grid で検証する。入力 gesture の
   native timestamp は `map_stamp_to_frame` で一度だけ frame へ
   写像し、`MapRejection` を defined rejection として返す。

### Out of Scope

- 実 GUI/描画・ノブ可視化 (headless 契約のみ、real-GUI lane は
  ruviform #243 側)。
- VST3/CLAP 実 plugin への適用 — plugin capability は fixture
  `ParamInfo` で契約検証。
- Modulation→Automation Bake・Multi-pass Take 記録・Feedback/Delay
  制御 graph (循環は拒否)。
- MIDI Control Mapping の実装 (#6 の別概念。rf#188 は timestamp 写像
  契約として消費するのみで、CC→Param の写像機能は作らない)。
- Note entity model — `ControlScope::Note` は型レベルの座標分離のみ
  (評価は `Unavailable` を返す)。
- Sample-accurate/audio-rate modulation、smoothing の実適用。
- Region 編集 UI・曲線間引き・点選択 tool。

## Requirements

- Automation/Modulation の永続データは `Project::control` にのみ
  存在し、全ての変更は `ProjectController` 経由のみ (1 apply = 1
  undo)。Mode/Arm/Gesture/Recording/Observation/Playback は session
  scoped であり Project・undo 履歴・`revision_inbox` に混入しない。
- 時間演算は checked API のみ (`BeatPosition::checked_add` 等。
  `Tick`/`TickDuration` の unchecked `Add`/`Sub` は使わない)。
- ruviform 契約型の再実装・重複実装をしない。`ruviform-plugin-core`
  を新規 workspace 依存として追加し (rf#186 契約の消費)、
  `Cargo.lock` の ruviform rev は `94c5bf8` 維持 (同一 rev の crate
  追加のみ)。upstream 欠陥を local workaround で隠さない。
- 値の wire 表現は `ParamValue` の normalized `[0,1]` ドメインに
  統一し、Depth は normalized 増分として合成してから denormalize
  する (Hz 増分と同義にしない)。
- 失敗時は project 正本が不変で、理由が呼び出し側へ返る。

## Acceptance Criteria

- [ ] **PC-A-01**: ParameterTarget/Lane/Source/Assignment が
  `Project::control` に単一正本として保持され、Lane・SignalFlow・
  Inspector 投影が同一内容を返す。op による変更が全投影へ即座に
  反映される (系列の複製なし)。
- [ ] **PC-A-02**: Track-scope と Clip-scope の Automation が独立に
  評価され、Clip Envelope が Track Volume Automation を上書き
  しない。Clip の移動が envelope の評価位置を (point 書換えなしに)
  追従させる。Scope 間の時間座標が混同されない。
- [ ] **PC-A-03**: 同一 target・同一評価時刻に重複する region を
  `Reject` policy で `AutomationConflict` として検出 (競合 lane・
  range・位置を報告)。`Replace` は明示上書きとして許可され、
  silent priority resolution は存在しない。load 時の
  `validate_references` も重複を検出する。
- [ ] **PC-A-04**: Breakdown が Base/Automation/Reference/各
  Source 寄与 (assignment id・raw・transformed・depth・寄与)/
  clamp・quantize/Host-resolved (normalized + plain) を含み、
  Plugin-reported/Observed が `AtPlayhead`/`Live`/`Captured`/
  `Stale`/`Unavailable` で区別される。
- [ ] **PC-A-05**: Mode 選択だけでは revision が進まない。
  Off=Base 編集 (undo 対象)、Read=復帰・無書込み、Touch=Arm+
  Record+Transport 成立時のみ gesture 区間書込み、Latch=解放後も
  継続・次回再生に持ち越さない、Write=成立対象のみ継続書込み。
  記録値は Manual/Reference (pre-modulation)、1 session=1 undo、
  loop wrap で seal、Inspector Follow 変化で target 不変。
- [ ] **PC-A-06**: 無遅延循環 (例: depth 駆動の相互参照) が compile
  で `ControlCycle` 拒否。評価順序は topo 固定で決定的、複数
  Assignment の加算が順序独立。
- [ ] **PC-A-07**: READONLY は write/base を拒否、非 AUTOMATABLE は
  TimedEvent を `NotAutomatable` で拒否・BlockBoundary への明示
  degrade、Boolean/Choice/Stepped は量子化規則付きでのみ許可、
  Immediate/PerNote は `UnsupportedMeans`、非線形写像で Depth≠
  plain 増分。
- [ ] **PC-A-08**: Tempo event 追加後も Beat 保持 point の評価が
  不変、tick↔beat 変換が `Ppqn`/`TickPosition` 経由で正確、
  clip-local 時間が clip 移動に追従、loop wrap で評価が一貫、
  offline render と再生が同一 grid で一致。`map_stamp_to_frame` が
  native stamp を一写像し rejection を返す。
- [ ] **PC-A-09**: `mise run fmt-check`/`check`/`lint`/`test` 全
  green、impl record に検証表・ruviform rev・未検証項目を正直に
  記録。

## Deferred Design Concerns

- Touch Return Time の既定・復帰形状 | #6 §9 | 実評価 contract 確定
  後 | PoC は release 境界の点列のみ。
- Automation Region 重複の編集 UI・曲線間引き・点選択 | #6 §9 |
  real-GUI lane で確定。
- Clip Stretch/Loop 中の envelope 評価写像 | #6 §9 | Clip time
  model 確定後 | PoC は Musical anchor の 1:1 写像のみ。
- Note-local 評価・Note entity | #10 | entity model 追加後 |
  `Scope::Note` は型のみで評価 `Unavailable`。
- Multi-pass Take/Loop Record の累積・Comp | #12 | Take 仕様で確定 |
  PoC は 1 周で明示 seal。
- Sample-accurate 配送・Plugin 規格別精度 | rf#186 PoC | adapter
  PoC 後 | PoC は capability 宣言と degrade のみ。
- LFO 波形/Transform の拡張集合・Smoothing 責務 | #6 §9 | 製品仕様
  で確定 | PoC は Sine/Triangle/Square + Identity/Invert/Abs/Square。

## Sources and Relations

- Issue #40 本文 (スコープ・受入証拠・非対象・制約)
- Issue #6 本文 (合意基本モデル + 4 決定の原仕様)
- `docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md`
  (WS-A 基盤)
- `docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md`
  (ED-A: Gesture/Transaction/Undo の型)
- `docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md`
  (F0: ProjectController・Stable ID・revision/undo)
- `docs/designs/configurable-ppqn-musical-time-architecture.md` (#19/rf#189)
- `docs/designs/time-transport-midi-automation-plugin-integration-architecture.md`
  (#20/rf#186/#188/#190)
- ruviform #186 (param control events)・#189 (musical time)・
  #188 (timed input/chase)・#185 (probe 観測語彙)
