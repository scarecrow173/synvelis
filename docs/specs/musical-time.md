---
id: "SPEC-QOg57ZwECMo94svdnmHZqk"
type: "spec"
status: "approved"
title: "Musical Time — Configurable PPQN・Rational Beat・Source-Native Tick・Tempo/Frame 量子化・SMF Fidelity 設計契約（synvelis#19）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/20"
  changes:
    added:
      - type: "file"
        path: "docs/specs/musical-time.md"
      - type: "file"
        path: "docs/designs/musical-time-design.md"
      - type: "file"
        path: "docs/plans/musical-time.md"
      - type: "file"
        path: "docs/tasks/musical-time-task.md"
      - type: "file"
        path: "docs/test-specs/musical-time.md"
      - type: "file"
        path: "docs/impl/ir/musical-time.md"
      - type: "file"
        path: "docs/impl/exp/musical-time.md"
      - type: "dir"
        path: "evidence/sv19"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/specs/tim-01-ppqn-config.md"
    - "docs/specs/tim-01-ppqn-preview.md"
    - "docs/specs/tim-01-app-ppqn.md"
    - "docs/specs/tim-01-ppqn-ui.md"
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/specs/project-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/tim-01-configurable-ppqn-design.md"
    - "docs/designs/tim-02-smf-format01-core.md"
    - "docs/designs/tim-02-player-midi-rt-offline.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/plans/musical-time.md"
    - "docs/test-specs/musical-time.md"
    - "evidence/sv19/case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/musical-time.md"
  references:
    - "evidence/sv19/case-ledger.md"
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/129"
    - "https://github.com/scarecrow173/synvelis/issues/130"
    - "https://github.com/scarecrow173/synvelis/issues/253"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/13"
    - "https://github.com/scarecrow173/synvelis/issues/14"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/18"
    - "https://github.com/scarecrow173/synvelis/issues/24"
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/190"
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/307"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/129"
    - "https://github.com/scarecrow173/synvelis/issues/130"
    - "https://github.com/scarecrow173/synvelis/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/307"
    - "evidence/sv19/case-ledger.md"
  deferred-by: []
---

# Musical Time — Configurable PPQN・Rational Beat・Source-Native Tick・Tempo/Frame 量子化・SMF Fidelity 設計契約（synvelis#19）

## What Should Be Built

Synvelis Issue #19「Configurable PPQN（default 960）・
Resolution-independent Musical Time・Native MIDI Fidelity」
の設計・ADR Issue として、Musical Time 横断契約を `docs/`
の文書チェーンと `evidence/sv19/` の受入証拠台帳として
固定する。対象は **設計契約の正本化と既存実装・先行
slice との照合** であり、コード実装は TIM-01 (#125)・
TIM-02 (#126)・TIM-03 (#127)・TIM-04 (#128)・TIM-F01
(#129)・TIM-F02 (#130) および Ruviform 側 Owner Issue
（ruviform#189 等）へ分解済みである。

本 spec が固定するもの:

1. **時間ドメインの分離契約** — project musical time
   （PPQN 依存 grid）・exact rational beat（PPQN 非依存の
   正本）・source-native MIDI tick/division・media/frame・
   device/recording frame の各ドメインが型・変換経路・
   保存先を持ち、暗黙の相互変換を禁止する。
2. **Configurable PPQN 契約** — 新規 project の既定
   PPQN=960、選択範囲 `1..=32767`（SMF 15-bit PPQ 由来）、
   PPQN 変更は明示 operation であり quantize/stretch/
   raw event retime ではない。
3. **Non-destructive reprojection 契約** — musical site
   （tempo/signature 位置・musical clip anchor/extent・
   comp range・monitoring punch mark）は exact rational
   beat 経由で再投影し、source-native MIDI tick/division/
   raw bytes/event stream・絶対 media anchor・device
   timestamp・recording/session frame・plugin frame
   base/identity は不変。
4. **Explicit conversion 契約** — `Exact`（非正確変換を
   拒否）と `Round`（宣言された丸め mode 必須）の 2 policy。
   暗黙 quantize を型・op・fixture の全層で禁止する。
5. **SMF Fidelity 契約** — Format 0/1・PPQ division の
   import/export、native division/tick/track/channel/
   event order/tempo/time signature/sysex/provenance/raw
   source bytes の保存、`ConversionReport`
   （exact_count/rounded_count/max_error/
   unsupported_events）による明示的誤差報告。
6. **Evidence 分類契約** — headless fixture 成功を実 MIDI
   device・実 plugin・実 OS/GUI の受入と混同しない。
   Required/Deferred/NotApplicable（要求状態）と
   Pass/Fail/NotRun/Unsupported（実行結果）を分離する
   （`POLICIES.md` §2/§3 準拠）。

## 用語と時間ドメイン

| ドメイン | 正本表現 | 所有 crate | PPQN 依存 |
| --- | --- | --- | --- |
| Project musical position | `MusicalTick`（project PPQN grid 上の i64 tick）＋ 正本は exact rational beat | `synvelis-project` | あり（grid 解像度） |
| Exact rational beat | ruviform `TickPosition`/`BeatPosition`（有理数 beat） | `ruviform-media-core`（汎用） | なし（PPQN 非依存） |
| Source-native MIDI | `MidiTick` + `MidiDivision`（source 固有 division） | `synvelis-project` | source 固有 |
| SMF document | `SmfDocument`（provenance: native tick/event/bytes） | `synvelis-project` | source 固有 |
| Media time | `MediaTimeValue`/`MediaDuration`（有理数 frames） | `synvelis-project` | なし |
| Device/recording frame | session/recording frame（device timestamp） | `synvelis-recording`/`synvelis-player` | なし |
| Plugin transport | frame base + capability class | `synvelis-player`/adapter | なし |

`Tick` は PPQN 文脈なしに意味を持たない。異なる PPQN の
tick 値を暗黙に比較・加算してはならない — 比較・加算は
必ず `TickPosition::to_beat` → `BeatPosition` →
`TickPosition::from_beat` の明示経路、または宣言された
`PpqnConversionPolicy` を通る。

## Requirements

### R-MT-01 Project PPQN

- 新規 project の既定 PPQN は **960**。
- PPQN の有効範囲は `1..=32767`（SMF PPQ 15-bit 範囲に
  整合。Ruviform 内部の rational beat 表現上限とは別概念）。
- PPQN は project の表示/編集分解能であり、正本である
  rational beat・source-native division を破壊しない。
- PPQN 変更は `SetProjectPpqn { old_ppqn, new_ppqn,
  policy }` op による明示操作。`old_ppqn` domain CAS +
  project revision CAS の二重照合、scratch clone 上の
  atomic validate、単一 undo entry、recording open 中は
  拒否。
- app 層 gate: transport 停止・open recording transaction
  なし・automation record_enabled なし（fail-closed）。

### R-MT-02 Rational Beat 正本

- musical position の正本は PPQN 非依存の exact rational
  beat。整数 grid tick に乗らない位置（例: 1/7 beat、
  分母が PPQN と互いに素な有理数位置）は正確に保持し、
  整数 tick への無断量子化を禁止する。
- 分母上限・overflow は typed error として顕在化する。
- `MusicalTick`・`MidiTick`・`MediaTimeValue`・device
  frame は別型であり、型を跨ぐ演算は明示変換関数経由のみ。

### R-MT-03 Non-destructive PPQN Migration

- PPQN 変更時、musical site（tempo/signature 位置、
  musical clip anchor/extent、comp range、monitoring
  punch mark 等）は exact beat を介して再投影する。
- 絶対 media anchor/extent、source-native MIDI
  tick/division/raw bytes/event stream、device timestamp、
  recording/session frame、media frame、plugin frame
  base/identity は **不変**。
- 960→1000→960 の往復で musical position が一致する
  （往復可能性は Exact policy で検証）。

### R-MT-04 Explicit Conversion / Quantize 禁止

- `PpqnConversionPolicy::Exact` は target grid で正確に
  表現できない位置を typed error で拒否する。
- `PpqnConversionPolicy::Round(Rounding)` は ruviform の
  宣言済み丸め mode（Floor/Ceil/TowardZero/AwayFromZero/
  NearestEven 系）を必須とし、暗黙の既定丸めを持たない。
- SMF export の tick 変換も同様に明示的で、結果は
  `ConversionReport` に exact/rounded/max_error/
  unsupported として記録する。暗黙 quantize は
  import/export/ppqn 変更のいずれの経路でも禁止。

### R-MT-05 Tempo / Transport / Loop

- tempo map は beat 基準。tempo change・tempo ramp・
  negative preroll・loop/seek/stop/chase・panic・
  playback/offline の各結果は headless fixture で定義済み
  でなければならない。
- beat→media/frame 変換は exact rational で行い、失敗は
  `Error::Overflow` として顕在化する（unchecked `Tick`
  演算は使わない）。
- loop/seek/stop/panic/chase で note の重複・欠落を起こさ
  ず、panic/end 処理後に sounding note を残さない。
- plugin transport は capability class（`Exact`/
  `FrameAccurate`/`BlockOnly`/`DegradedWithConsent`/
  `Unsupported`/`Unknown`）で分類し、Synvelis が DAW 精度
  policy を、Ruviform が generic 変換/runtime adapter を
  所有する。実 plugin 検証は TIM-03 (#127) の別 gate。

### R-MT-06 SMF Fidelity

- SMF Format 0/1・PPQ division のみ対象。SMPTE division
  （`0x8000` bit）・Format 2・MIDI 2/MPE は明示的
  `Unsupported`/Deferred。
- import は native division・raw tick・track/channel・
  event order・tempo/time signature・sysex・provenance・
  raw source data を保存する。note projection は
  `paired_notes()` の明示経路のみ。
- source 由来 note の直接編集は provenance 破壊として拒否。
  明示的 detach は独立した編集可能 copy を作る。
- export は `ConversionReport` を返し、同 tick event の
  NoteOff→NoteOn 順序を deterministic に保証する。
- SMF Import→Export→Reimport の golden round-trip を
  versioned fixture で判定する。

### R-MT-07 Compatibility / Migration

- 旧 fixed-960 project snapshot は versioned migration で
  互換 load でき、旧 byte 列を破壊しない。
- schema version は `SCHEMA_VERSION` 定数で管理し、
  migration step を `migration.rs` に追加する既存規約に
  従う。
- plugin transport・automation（`BeatPoint` 有理数 beat
  格納）・recording VS との互換は各 Owner contract で
  維持する。

### R-MT-08 Safety / Evidence

- 時間計算の overflow・分母上限・RT 処理量・丸め誤差は
  versioned fixture で検証可能でなければならない。
- `evidence/sv19/case-ledger.md` が受入条件→fixture→
  Expected/Observed→Status の台帳を保持する。
- headless fixture の Pass を実 MIDI device・実 plugin・
  実 OS/GUI の受入と混同しない。実 MIDI device backend
  は RF#253 / synvelis#253 の別 gate。

## 受入条件写像（Issue #19 転記 → Case ID）

| Case | 受入条件 | 主担当 Owner |
| --- | --- | --- |
| SV19-C01 | 新規 Project が 960 を既定値とし、異なる PPQN を設定・保存・再読込できる | TIM-01 (#125) 実装済み |
| SV19-C02 | PPQN=480/960/1000/1920 等の Source が Native Tick/Division を失わず Import される | TIM-02 (#126) 実装済み |
| SV19-C03 | Beat の 1/7 など 960 の整数 Tick に乗らない位置を正確な正本として保持する（必要範囲を検証） | Ruviform#189 + TIM-01 |
| SV19-C04 | Project PPQN 変更前後の音楽的位置と Raw MIDI Timestamp が一致し、明示 Quantize なしに移動しない | TIM-01 (#125) 実装済み |
| SV19-C05 | 異なる PPQN の Tick を誤って比較・加算できず、明示変換を要する | 型境界 + TIM-01 |
| SV19-C06 | Tempo Change / Loop / Negative Pre-roll / MIDI Off / Playback/Offline で結果が定義済み | TIM-02 (#126) + #20 |
| SV19-C07 | 旧 960 Project の Save/Reload、Plugin Transport、Automation、Recording VS と互換移行ができる | TIM-01/03/04 + migration |
| SV19-C08 | 時間計算の Overflow、分母上限、RT 処理量と丸め誤差が検証できる | 全 slice + fixture |

## Out of Scope / Deferred

- **実 MIDI device backend 検証** — synvelis#253 /
  ruviform#253 の別 gate。headless SMF fixture の成功を
  実機成功に換算しない。
- **実 plugin transport 精度の実機検証** — TIM-03 (#127)
  および Ruviform#307 の別 gate。capability 分類と
  headless dispatch 契約のみ本契約が固定する。
- **Plugin state snapshot 品質** — TIM-04 (#128) の別
  slice（`tim-04-plugin-state-snapshot-consistency.md`）。
- **SMF SMPTE division・Format 2・MIDI 2/MPE** — TIM-F01
  (#129)・TIM-F02 (#130) として明示 Deferred/Unsupported。
- **Ruviform generic Rational Time/Clock 変換の再定義** —
  Synvelis 固有 API として再実装しない。ruviform#189 が
  generic configurable rational musical time の Owner。
- **設計完了を実装/実 OS/Plugin Gate の PASS と混同しない**
  — 本 slice は docs/evidence のみ。merge 後の対象 SHA の
  CI 証拠が統合検証。

## Evidence 分類ルール

- `Required` = 本契約で要求される項目。`Deferred` =
  別 Owner/将来 lane。`NotApplicable` = 本 slice 範囲外。
- `Pass`/`Pass*` = 先行 slice の実測参照（SHA・環境明記）。
  本 slice は実装 crate の test を再実行しない。
- `NotRun` = fixture 設定済みだが本 slice 未実走、または
  構成済みテストなし。`Unsupported` = 環境上利用不可。
- Mock/fake/headless の PASS は fault classification・
  contract 検証の証拠であり、実機・実 FS durability・
  実 plugin の証明ではない。
