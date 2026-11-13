---
id: "DESIGN-2PaoXTMFb4NbwT7TNBcUTc"
type: "design"
status: "approved"
title: "Automation / Modulation 統合 Parameter Control — 設計（synvelis#6）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/automation-modulation.md"
  implemented-by: []
  depends-on:
    - "docs/specs/automation-modulation.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/specs/pc-c5-04-recording-status.md"
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/specs/context-inspector.md"
    - "docs/specs/svt-product-e2e.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/automation-modulation.md"
    - "docs/tasks/automation-modulation-task.md"
    - "docs/test-specs/automation-modulation.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/99"
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/synvelis/issues/134"
    - "https://github.com/scarecrow173/synvelis/issues/135"
    - "https://github.com/scarecrow173/synvelis/issues/136"
    - "https://github.com/scarecrow173/synvelis/issues/137"
    - "https://github.com/scarecrow173/synvelis/issues/138"
    - "https://github.com/scarecrow173/synvelis/issues/139"
    - "https://github.com/scarecrow173/ruviform/issues/185"
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/190"
    - "https://github.com/scarecrow173/ruviform/issues/307"
  defers:
    - "Touch Return Time/Shape の最終既定値・曲線 Decimation 許容誤差・TimeBase/Plugin 精度の数値閾値（Issue #6 §H/§9 の PoC 後決定事項）"
    - "AUT-F01〜F05（#135〜139）— Multi-pass Takes・Envelope/Random Source・外部 MIDI CC Mapping・Audio-rate Modulation・明示 Bake"
    - "実 GUI/実 OS/実 Plugin/実機の受入（#102/#114/#103 の別受入 Gate）"
  deferred-by: []
---

# Automation / Modulation 統合 Parameter Control — 設計

## 概要

synvelis#6 の合意済み基本モデルを、後続実装 slice が消費できる
**製品 UX・設計契約**として固定する。対象は 3 層:

1. **Project/Model 層** — `ParameterTargetId` を単一正本とする
   Control Model。`ControlOwner × AutomationTimeScope` の分離、
   主系列 Automation の競合検出、Modulation Assignment の合成規則、
   Capability Honesty。
2. **Runtime/Dispatch 層** — Read/Touch/Latch/Write の状態機械、
   Recording Transaction（CAS・fixation・Undo seal）、
   Evaluation の安定性と制限。
3. **Presentation 層** — Arrangement Automation Lane・Signal
   Flow Control Layer・Inspector Control Breakdown の 3 View が
   同一 Relation/Trace を整合表示し、調査クリックが Editing
   Selection を変えない UX 契約。

本設計は新しい Control/Graph/Plugin エンジンを作らない。既存の
`synvelis-project` の `ControlModel`（`ParameterTarget`/
`AutomationLane`/`AutomationRegion`/`AutomationPoint`/
`ModulationSource`/`ModulationAssignment`/`ControlOwner`/
`AutomationTimeScope`/`ParameterRole`/`TargetCapability`/
`AutomationConflict`/`ConflictPolicy`/`ControlGraph`/`EvalPlan`/
`evaluate_control`/`ControlBreakdown`）を正本として参照し、
`synvelis-workspace` の `ControlSession`（mode/record_points/
gesture/fixation）、`RecordingTransaction`（prepare/apply/
finalize/seal）、`RelationView`（PC-C2 Index）、`ProbeAdapter`
（SF-04）、`ControlTrace`（PC-C3）の責務分界を維持する。

## スコープと Owner 境界

| 責務 | Owner | 本設計での扱い | 既存の対応先 |
| --- | --- | --- | --- |
| Control Model schema（Target/Lane/Source/Assignment/Owner/TimeScope） | synvelis-project | 参照のみ。新 schema を作らない | `control.rs` `Project::control` |
| `ParameterTargetId` 単一正本・canonical SendLevel | CTL-02 #122 | 契約として消費。重複定義しない | `ParameterTargetId`/`SendLevel`/`ParamFlags` |
| `ControlOwner × AutomationTimeScope` 分離 | CTL-01 #121 | 契約として消費。旧 Scope 表を代用品にしない | `ControlOwner`/`AutomationTimeScope`/`ParameterRole` |
| 評価・合成・clamp・cycle 検証・Breakdown | synvelis-project `evaluate_control`/`ControlGraph` | 契約として消費。第二 Engine を作らない | `pc_a_*`/`ctl_02.rs`/`aut_02.rs` |
| Dispatch（TimedParamEvent/receipt）・RT/Offline parity | PC-B #43 / PC-C4 #101 | 契約として消費 | `pc_b.rs`/`pc_c4_*`/`TimedParamEvent` |
| Recording Transaction（CAS/prepare/apply/finalize/seal/Undo） | PC-C1 #98 | 契約として消費 | `pc_c1.rs`/`rec_c1_*`/`aut_03` |
| Relation Index・双方向投影 | PC-C2 #99 | 契約として消費 | `pc_c2_relation.rs`/`rel_c5.rs` |
| ControlTrace（Applied/Observed 分離・Freshness） | PC-C3 #100 / PC-C3-05 | 契約として消費 | `trace_c3.rs`/`sf04_s2_probe_trace.rs` |
| Plugin Parameter Binding・frame base | PC-C4 #101 | 契約として消費。Plugin-reported と Host-resolved を混同しない | `pc_c4_binding.rs`/`pc_c4_0*` |
| Probe Adapter・Observed Output | SF-04 #107 | 契約として消費。Probe 実測を Plugin-reported と別表示 | `sf04_*`/`pc_c4_03_observation.rs` |
| Relation Inspector・gesture・Recording Status UI | PC-C5 #102 / sv#5 Context Inspector | 契約として消費。製品 GUI 実装は #102/#114 | `uic5*.rs`/`rel_c5.rs`/`sf_05*.rs` |
| Cross-gate Oracle | PC-C6 #103 | 証拠分類の正本 | `pc_c6_oracle_*`/`evidence/sv5/` |
| Automation Mode/Region/Loop/LOD | AUT-01〜04 #131〜134 | 契約として消費 | `aut_01/02/03/04*.rs` |
| **本設計の固有責務** | **設計文書のみ** | UX 操作契約・Scope/Target/Time Mapping 表・画面操作 Fixture・受入台帳 | 本 docs chain + `evidence/sv6/` |

境界ルール:

- 本設計は **第二の Persisted Parameter/Control Engine を
  Synvelis 内に作らない**。`ControlModel` が唯一の永続正本。
- Ruviform 共通 Control/Graph/Plugin 正本へ DAW 固有の
  `AutomationTimeScope`/Clip/Note 概念を持ち込まない。
- 実装は下流 Issue が行う。本 docs chain は契約と証拠分類のみ。

## Scope / Target / Time Mapping

Issue #6 の歴史的 Scope 表を CTL-01/02 の新型へ写像する。
「所有（owner）」と「automation 点の時刻座標（time_scope）」は
別軸であり、混同しない。

| UX 上の Automation 種別 | ControlOwner | AutomationTimeScope | 作用先（Target 例） | 時間座標の解釈 | 補足 |
| --- | --- | --- | --- | --- | --- |
| Track Automation（Track Volume/Pan 等） | `Track`（`AutomationTimeScope::ProjectMusical`） | `ProjectMusical` | `ParameterTargetId` of Track parameter | Project の Musical Time（PPQN tick/BeatPoint） | 再生・Loop・Tempo 変更で Project 座標に追従 |
| Clip Envelope（Clip Gain/Clip Envelope） | `Clip`（`AutomationTimeScope::ClipLocal`） | `ClipLocal` | `ParameterTargetId` of Clip-local parameter | Clip 内相対位置。Clip 移動に追従、Trim/Stretch で再写像 | Track Volume Automation とは別 Target・別処理段階。暗黙上書きしない |
| Note Expression（Note-level velocity/modulation） | `Note`（`AutomationTimeScope::NoteLocal`） | `NoteLocal` | `ParameterTargetId` of Note-local parameter | Note 内相対位置 | PC-B-02 の Note scope 契約。Clip 削除で cascade |
| Project-global 値（例: Project Tempo 連動 Macro） | `Project`（`AutomationTimeScope::ProjectGlobal`） | `ProjectGlobal` | `ParameterTargetId` of Project/Global parameter | Project 全域 | 共有は明示 Assignment のみ。暗黙共有しない |
| Clip Envelope → 他 Plugin Parameter の適用 | （Clip owner とは別）明示 `ModulationAssignment` | （Source 側 scope に従う） | 対象 `ParameterTargetId` | Source 評価 → Assignment で Target へ | 「Clip Envelope を Parameter に適用」は暗黙でなく **明示的 Control Assignment**。DAG の辺として Relation Index に現れる |

原則:

- `AutomationLane` は `ParameterTargetId` に紐づく論理系列。
  `AutomationRegion` はその系列内の編集区間。**同一 Parameter・
  同一評価時刻に同時評価される主要 Automation 系列は原則一つ**。
  複数 Region は同じ論理系列へ包含できるが、同時に競合する系列は
  `AutomationConflict`/`ConflictPolicy::{Reject, Replace}` で
  明示的に検出・解決する。暗黙の優先順位・View 順序・
  「最後に編集した方が勝つ」で潰さない。
- `AutomationTimeScope` は「automation 点がどの時間座標系で
  評価されるか」を表す。`ControlOwner` は「その制御を所有・
  スコープする対象」を表す。旧 Scope 表（Track/Project・
  Clip-local・Note-local・Project-global）はこの 2 軸の
  **組み合わせの表示名**であり、新型の代用品ではない。
- Clip Envelope が Track Volume Automation を上書きしないのは
  「別 Target・別処理段階」であるためであり、優先順位ルール
  ではない。Target が同じ場合のみ競合検出が働く。
- Tempo 変更・Loop・Clip Trim/Stretch・Absolute vs Musical・
  丸めは Project time-model の決定事項（TIM-01/02・#19/#20）
  に従う。本設計は「座標系の名前と分離」を固定し、数値変換
  ルールは TIM 系契約へ委譲する。

## 画面操作 Fixture（UX 操作契約）

実装済み headless 契約を製品 UX 操作へ写像する。各 Fixture は
test-spec の `AM-UX-*` と 1:1 対応し、`evidence/sv6/coverage.md`
で被覆を分類する。

### F-UX-01 Automation Lane の表示と編集

- Arrangement の Automation Lane は `AutomationLaneId` →
  `ParameterTargetId` の投影。Lane の表示/非表示（collapse・
  hide）は **View 状態**であり、Project の `PlaybackState`・
  `automation_enabled` を変更しない。再生中に Lane を隠しても
  Automation は評価・適用され続ける（AM-03）。
- Lane 上の点編集・Region 操作は PC-C1 Recording Transaction /
  AUT-02 Region Replace の契約に従う。Undo は 1 ジェスチャ/
  1 セッション単位。

### F-UX-02 Mode 切替（Off/Read/Touch/Latch/Write）

- Mode 切替 UI は `PlaybackState`（play/stop/pause/record 系）と
  `WriteMode`（Off/Read/Touch/Latch/Write）・`WriteArm`・
  `GestureState`・`RecordingTransaction` を **分離して** 表示・
  制御する。Mode の選択だけでは Project Automation を mutate
  しない（`pc_a_05_mode_select_never_mutates_and_off_read_behave`）。
- **Off**: Automation 停止 → Base Value。手動編集は Base を変更。
  Write しない。Modulation は別系統として維持される（AM-03）。
- **Read**: Automation 有効。手動操作は一時的に優先し、Gesture
  終了後に Automation 値へ復帰。Write しない
  （`aut_01_read_override_lives_only_during_gesture`）。
- **Touch**: Automation 有効。手動操作区間のみ記録し、Gesture
  終了で既存 Lane へ復帰（Touch Return、AUT-01 暫定
  150ms Smoothstep）。記録は Modulation 合成**前**の Manual/
  Reference 値（AM-08/AM-10）。
- **Latch**: Automation 有効。最初の Gesture 以降、手動値が
  Transport/Record 停止または Mode 変更まで継続して記録される。
  終了後の次回再生開始で自動的に記録を再開しない — 再 Arm は
  明示操作（`aut_01_latch_holds_until_boundary`、
  `aut_03_later_passes_write_nothing_until_explicit_rearm`）。
- **Write**: 手動/Write 値が優先。記録には **明示 Arm +
  Automation Record + Transport 条件**が必要で、Arm されて
  いない Target・非対象へは書き込まない（AM-09）。Reference
  基準は Recording 開始時に固定される。

### F-UX-03 Parameter Control Breakdown

Inspector の Parameter 詳細に Control Breakdown を表示する。
各行は **出所（provenance）付き**で区別する:

| 行 | 内容 | 出所 |
| --- | --- | --- |
| Base | 永続化された設定値 | `ParameterTarget.base` |
| Automation | 評価位置の Automation 値 | `AutomationLane`/`AutomationRegion` |
| Reference | Manual Gesture/Mode で調整された参照値 | `GestureState`/`WriteMode` |
| Modulation Contributions | 各 Source ID・Depth・Transform・現在寄与・合計 | `ModulationAssignment` 群 |
| Host-resolved | ホストが送信/適用しようとする値（評価時刻・精度・Revision 付き） | `evaluate_control` + `ControlTrace` |
| Plugin-reported | Plugin が publish した値（利用可能な場合のみ） | PC-C4 Binding の report 経路 |
| Observed Output | Probe 実測の Audio/Control 状態 | SF-04 ProbeAdapter |

原則:

- Host-resolved は「ホストが意図した値」であり、Plugin 内部の
  真の最終適用値と同義にしない。Plugin-reported・Observed
  Output が `Unavailable` の場合は **推測値を出さず
  `Unavailable` と表示**する（AM-06）。
- 通常 View は簡潔な Base/Range/現在位置を示し、詳細は
  Control Breakdown を開いて確認する（#5 Context Inspector の
  Follow/Explore/Pinned 契約に従う）。
- 色だけでなく label・symbol・line style でも区別する
  （#3/#4 の a11y/識別性契約）。

### F-UX-04 Signal Flow / Inspector / Arrangement の整合

- 3 View は同一 `ControlRelation`/`ControlTrace` を投影する。
  Signal Flow は Source → Assignment → Target の接続を、
  Inspector は Parameter/Source/Recording の詳細を、
  Arrangement は Automation Lane/Clip/Note の時間配置を示す。
- Signal Flow の接続は必要な辺だけを展開する。全辺を常時
  描画しない。
- Inspector/Signal Flow のリンクは Arrangement の Lane・
  LFO・Macro・Parameter・Capture/Diagnostic View へ遷移する。
- **調査クリックは他 View の Editing Selection を変更しない**。
  Follow/Explore/Pinned は `InspectionState` であり、Project
  を mutate しない（AM-13）。Editing Selection と Inspection
  Selection を混同しない。

### F-UX-05 観測状態と鮮度

Control Breakdown / Inspector の各値に観測状態を付す:

- `At Playhead`: 評価位置が Playhead に一致。
- `Live`: Transport 再生中の最新評価。
- `Captured`: 録画/キャプチャ済みの値（Playback 位置と独立）。
- `Stale`: 世代（Graph/Control/Transport/Clock Epoch）または
  時刻・Revision が現在と不一致。旧世代を現在値として表示
  しない。
- `Unavailable`: Plugin-reported/Probe 等、その層が値を
  提供しない・未接続。

各表示は評価時刻・Latency・Revision・適用世代を参照可能にする
（AM-07）。`TraceFreshness`/`ObservationState` の語彙を使用し、
新しい状態名を発明しない。

## Decision Log

| # | 決定 | 採用案 | 却下案と理由 | 状態 |
| --- | --- | --- | --- | --- |
| D-01 | Target 正本 | `ParameterTargetId` を唯一の正本とし、Track/Clip/Note/Plugin Parameter の全制御をこれに紐づける | 文字列 path・(track,plugin,param) タプルの並存 — 一意性・rename 耐性・Relation/Trace 紐付けで破綻（CTL-02 #122） | adopted |
| D-02 | Scope 表現 | `ControlOwner × AutomationTimeScope` の 2 軸。旧 Scope 表は表示名 | 旧 Scope enum を拡張して time_scope を内包 — owner と時間座標が混同し、Clip Envelope→Parameter の明示 Assignment を表現できない（CTL-01 #121） | adopted |
| D-03 | 主系列 Automation | 同一 Target・同一評価時刻の主要系列は原則一つ。競合は `AutomationConflict`/`ConflictPolicy` で明示検出・解決 | last-edit-wins・View 順序優先 — 暗黙の上書きでデータ損失・再現性喪失（PC-A-03・AUT-02） | adopted |
| D-04 | Automation Off | Base Value 復帰。Modulation は独立系統として維持。Off≠Modulation Off | Off で Modulation も停止 — Automation と Modulation の責務混同（Issue #6 §基本モデル） | adopted |
| D-05 | Lane 可視性 | View 状態であり Playback/Automation を変更しない | hide=stop — Timeline 非表示が再生へ副作用を持つのは UX 事故（AM-03） | adopted |
| D-06 | Modulation 合成 | `raw(t)=a(t)+Σ[d_i×f_i(s_i(t))]`、合計後 Clamp、`Clamped/Saturated` を説明。Assignment 単位で Depth/Polarity/Transform/Enabled | Source 側に polarity/depth を持たせる — 同一 Source を複数 Assignment で別設定できない。順序依存加算 — 再現性を損なう（PC-A-06） | adopted |
| D-07 | 値の区別 | Base/Automation/Reference/Source 寄与/Host-resolved/Plugin-reported/Probe 実測を出所付きで分離 | 単一「現在値」表示 — どの層の値か不明で誤診・誤編集（PC-A-04・PC-C3） | adopted |
| D-08 | Write 記録値 | Modulation 合成前の Manual/Reference 値を記録（double-modulation 防止） | 合成後値を記録 — LFO 動作中に Automation へ bake され二重適用（AM-10） | adopted |
| D-09 | Undo 単位 | 1 録音セッション/1 Gesture = 1 Project Undo transaction | ポイント毎・Region 毎 Undo — 部分 Undo で系列が破壊される（PC-C1・AUT-03） | adopted |
| D-10 | Gesture fixation | Command Begin 時に Target/Revision/Arm を fixation。Follow/Window 移動で retarget しない | 現在の Focus/Selection を都度参照 — Gesture 中のすり替え（AM-11） | adopted |
| D-11 | Cycle | DAG のみ許容。無遅延循環は拒否。Feedback は明示 delay/評価規則が後続設計で決まるまで非対応 | 暗黙の評価順序で循環を「それっぽく」解く — 結果が実行順序依存になり再現不能（PC-A-06） | adopted |
| D-12 | Capability | Boolean/Choice/Stepped/非線形は明示 capability・変換・量子化規則がある場合のみ Modulation 対象。非対応は `Unsupported`/`Unavailable` | 全 Parameter に Modulation を適用 — 離散値の誤丸め・機能保証違反（PC-A-07・PC-C4-02） | adopted |
| D-13 | Control-rate | 既定は control-rate。sample-accurate/audio-rate は capability 依存・限定的 | 全 Parameter を audio-rate 保証 — Ruviform/Plugin 精度契約と不整合（PC-B・Ruviform #186/#196） | adopted |
| D-14 | Source 配置 | Processor-local・Track-local を優先。Project 共有は明示 Assignment のみ | Project-global 暗黙共有 — 意図しない結合（Issue #6 §基本モデル） | adopted |
| D-15 | 初期 Source | LFO・Envelope・Macro | Audio-rate LFO・外部 MIDI CC・Random — 後続 Feature（AUT-F02/F03・#136/#137） | adopted |
| D-16 | Touch Return | AUT-01 暫定契約（150ms Smoothstep・per-position curve follow）を参照。最終値は PoC 後決定 | 本設計で数値を新規決定 — 実測なしの閾値捏造を避ける | adopted（暫定参照） |
| D-17 | 非線形表示 | Target Parameter の正規化写像に従う。Depth%≠Hz 増分 | Depth を物理単位で表示 — 周波数等で誤解（Issue #6 §基本モデル） | adopted |
| D-18 | Automation Bake | 別の明示 Feature として deferred（AUT-F05・#139）。記録は常に pre-modulation | 記録時に自動 bake — 二重 Modulation（AM-10） | deferred |
| D-19 | 実 GUI/実機 Gate | #102/#114/#103 の別受入。本 docs chain は headless 証拠を `Pass (headless)`/`Partial`/`NotRun`/`Deferred`/`Unsupported` で分類 | headless 成功を GUI Pass と主張 — POLICIES.md 違反 | adopted |
| D-20 | Time 変換数値 | Tempo/Loop/Trim/Stretch の数値ルールは TIM-01/02・#19/#20 契約へ委譲。本設計は座標系名の分離のみ固定 | 本設計で数値変換を再定義 — 二重正本 | adopted |

## 代表 Scenario と Expected Oracle

### Normal Scenarios

**S-N1 Automation Off + Modulation 維持（AM-03/AM-04）**
Track Volume に Automation Lane と LFO Assignment がある状態で
Automation を Off にする。期待: Automation 寄与が 0 になり
Base+Modulation のみが Host-resolved に現れる。Modulation は
停止しない。Oracle: `evaluate_control` の breakdown で
Automation 行=0/なし、Modulation Contributions 非零、
Automation Lane は Project に残存。

**S-N2 複数 Assignment の決定的加算（AM-04）**
1 Target に LFO(depth=0.2, polarity=+, transform=linear)と
Macro(depth=0.5, polarity=-, transform=square)の 2 Assignment。
期待: `raw = a + 0.2×f_lfo(s_lfo) + 0.5×f_macro(s_macro)`、
合計後 Clamp。Assignment の追加順・DAG ノード位置を変えても
結果が同一（集約順序は固定）。Oracle: `pc_a_06` の
order-deterministic と同型。

**S-N3 Touch → Latch → Write の遷移（AM-08/AM-09）**
Transport 再生中に Read→Touch（Gesture 区間のみ記録→復帰）、
Latch（Gesture 後も継続、Transport 停止で seal）、Write
（Arm+Record+Transport を満たす Target のみ）。Oracle:
`aut_01_*`/`aut_03_*`/`pc_c6_r07_mode_gating_oracle` の
headless 契約。製品 GUI ジェスチャは #102/#114 Gate。

**S-N4 Breakdown の出所分離（AM-06/AM-07）**
Plugin が report を提供し Probe が接続された状態で Breakdown
を開く。期待: 7 行が出所付きで表示され、Plugin-reported と
Observed Output が別行。Probe 未接続時は Observed Output=
`Unavailable`（推測値なし）。Oracle: `pc_a_04`/`trace_c3`/
`pc_c4_03_observation` の語彙。

**S-N5 3 View 整合・調査は非破壊（AM-13）**
Signal Flow で Source→Assignment→Target を辿り Inspector で
Breakdown を開き Arrangement の Lane を確認。調査クリックで
Editing Selection が変わらない。Oracle: `rel_c5`/`uic5_02_
inspect_is_not_edit`/`pc_c6_ui01` の headless 契約。3 View
同時表示の製品 GUI は #102/#114。

### Failure Scenarios

**S-F1 同時競合 Automation（AM-02）**
同一 Target・同一評価時刻に 2 系列が競合する編集を試行。
期待: 暗黙上書きせず `AutomationConflict` を検出し
`ConflictPolicy`（Reject/Replace）の明示解決を要求。
Oracle: `pc_a_03_conflict_reject_and_replace`/`aut_02`/
`aut_03_replace_collision_preserves_outside_curve`。

**S-F2 無遅延 Cycle（AM-05）**
Source A→Assignment→Target B、Source B→Assignment→Target A
の循環を構成。期待: `ControlGraph` 構築時に拒否。Oracle:
`pc_a_06_cycle_rejected_and_order_deterministic`。

**S-F3 非対応 Parameter（AM-05）**
Boolean/Choice/Stepped や capability 未申告 Parameter に
連続 Modulation を割り当て。期待: `Unsupported`/`Unavailable`
を明示し、誤った値・機能保証をしない。Oracle:
`pc_a_07_capability_honesty`/`pc_c4_02_typed_rejects_no_misdelivery`。

**S-F4 Write の対象外書き込み試行（AM-09）**
Arm されていない Target または Record/Transport 未成立で
Write 記録を試行。期待: 書き込みを拒否し対象を変更しない。
Oracle: `pc_a_05_recording_gates_undo_seal_fixation`/
`pc_c6_r07`/`rec_c1_05_lifecycle_gates`。

**S-F5 Gesture 中の Follow/Window 移動（AM-11）**
Touch/Write Gesture 中に Inspector の Follow 対象を切替または
Window を移動。期待: fixation により Target/Revision が不変、
書き込み先がすり替わらない。stale Revision は明示 reject。
Oracle: `uic5_02_focus_selection_change_never_retargets`/
`uic5_02_commit_drops_on_window_epoch_change`/
`aut_01_fixation_captures_begin_context`/
`aut_03_fixation_survives_mode_and_arm_drift`/
`pc_c6_ui04_two_window_epoch_and_cancel`。

**S-F6 LFO 動作中の記録で二重 Modulation（AM-10）**
LFO Assignment が有効な Target を Touch/Write で記録。
期待: 記録値は pre-modulation の Manual/Reference であり、
再生時に LFO が Automation に bake されて二重適用されない。
1 セッションは 1 Undo。Oracle: `rec_c1_01_commit_success_one_
undo_raw_manual_save_separate`/`pc_c6_r04_commit_one_undo_
exactly_once`/`aut_03_multi_lane_pass_is_one_undo`/
`aut_01_touch_return_is_nondestructive`。

**S-F7 Loop/Tempo/Offline の安定性（AM-12）**
Loop 再生・Tempo 変更・オフラインレンダリングで同一 Project
を評価。期待: Musical 座標で安定評価、RT/Offline parity、
精度/制限を明示。Loop wrap で seal、次 pass は明示 rearm
まで書き込まない。Oracle: `aut_03_*`/`pc_a_08_time_consistency`/
`pc_b`/`pc_c4_01_eval_dispatch_receipt_rt_offline_parity`/
TIM-01/02 系。実 Plugin 精度は EXT-L3 Gate。

## 検証戦略と Gate 対応

- **設計文書の検証**: `mise run fmt-check`（docs/Markdown の
  軽量チェック。Rust build/test は対象外）。
- **headless 証拠**: 既存 `synvelis-project`/`synvelis-workspace`/
  `synvelis-player` の `pc_*`/`aut_*`/`ctl_*`/`rel_c5`/`uic5*`/
  `sf04*`/`pc_c6_oracle_*`/`trace_c3` テストを `evidence/sv6/
  coverage.md` で AC へ写像し、`Pass (headless)`/`Partial`/
  `NotRun`/`Deferred`/`Unsupported` で分類。
- **GUI Gate（Required/Deferred）**: 製品 Automation Lane・
  Inspector Breakdown・Signal Flow の実 Window 操作は
  #102（UI-C5）/#114（SV-GUI-05）/#103（Cross Oracle）の別受入。
  本 slice は `Deferred`。
- **実 Plugin/実機 Gate（Required/Deferred）**: 実 VST3/AU/CLAP・
  実 device・native dock は EXT-L3。#101/#103/#107 の別受入。
  本 slice は `Deferred`。
- **OS-L1**: Windows は native build/test が Required だが、本
  slice は docs のみで crate 変更なし → `NotApplicable`。
  macOS/Linux OS-L1 は docs のみのため `NotApplicable`。

## 未決定・PoC 後に決める事項

- Touch Return Time/Shape の最終既定値（AUT-01 暫定 150ms
  Smoothstep は既存契約として参照。実測 PoC 後に確定）。
- 曲線 Decimation/LOD の許容誤差・Record Budget 数値
  （AUT-04 の契約範囲。実データ計測後）。
- TimeBase/Plugin 精度の数値閾値・Absolute vs Musical の
  既定（TIM-01/02・#19/#20・Ruviform #189/#196）。
- Sample-accurate/Audio-rate Modulation の対応 Parameter 範囲
  （capability 宣言後）。
- Feedback/delay を伴う Control Graph の評価規則（D-11 の
  deferred。明示 delay 契約が別途必要）。
- Automation Bake（D-18・AUT-F05 #139）。
- Multi-pass Takes・Envelope/Random Source・外部 MIDI CC
  Mapping・Audio-rate LFO（AUT-F01〜F04・#135〜138）。
