---
id: "DESIGN-v1cBPgJUA5gYtWIH40kczJ"
type: "design"
status: "approved"
title: "Musical Time — 設計契約マップ（synvelis#19）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/musical-time.md"
  changes:
    added:
      - type: "file"
        path: "docs/designs/musical-time-design.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/musical-time.md"
  implemented-by: []
  depends-on:
    - "docs/specs/musical-time.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/specs/tim-01-ppqn-config.md"
    - "docs/specs/tim-01-app-ppqn.md"
    - "docs/specs/tim-01-ppqn-preview.md"
    - "docs/specs/tim-01-ppqn-ui.md"
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/designs/tim-01-configurable-ppqn-design.md"
    - "docs/designs/tim-01-ppqn-config-design.md"
    - "docs/designs/tim-01-app-ppqn-design.md"
    - "docs/designs/tim-01-ppqn-preview-design.md"
    - "docs/designs/tim-01-ppqn-ui-design.md"
    - "docs/designs/tim-02-smf-format01-core.md"
    - "docs/designs/tim-02-ppqn-smf-cross-golden-design.md"
    - "docs/designs/tim-02-player-midi-rt-offline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/musical-time.md"
    - "docs/plans/musical-time.md"
    - "docs/test-specs/musical-time.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/specs/project-model.md"
    - "evidence/sv19/case-ledger.md"
  refines:
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  refined-by: []
  derives-from:
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv19/case-ledger.md"
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/129"
    - "https://github.com/scarecrow173/synvelis/issues/130"
    - "https://github.com/scarecrow173/synvelis/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/189"
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

# Musical Time — 設計契約マップ（synvelis#19）

## Context

Issue #19 は Musical Time の横断設計・ADR Issue であり、
Configurable PPQN・Resolution-independent Musical Time・
Native MIDI Fidelity を要求する。調査の結果、要求の大半は
TIM-01 (#125)・TIM-02 (#126) の先行 slice で **既に
実装・fixture 化済み** であり、本 slice の実作業はそれら
の契約を Issue #19 の受入条件へ写像し、横断的一貫性
（時間ドメイン分離・明示変換・証拠分類）を docs chain
として固定することである。

前提となる実装事実（本 slice でソース確認済み）:

- `crates/synvelis-project/src/ops.rs`:
  `SetProjectPpqn { old_ppqn, new_ppqn, policy }` と
  `ImportMidiSmf { document }` が実在する。
- `crates/synvelis-project/src/ppqn.rs`:
  `PpqnConversionPolicy::{Exact, Round(Rounding)}`、
  `TickPosition::to_beat`/`from_beat` 経由の再投影、
  scratch clone atomic validate、Floor/Ceil による
  Exact 境界判定が実装済み。
- `crates/synvelis-project/src/smf.rs`:
  `parse_smf`/`export_smf`/`SmfDocument`/`ConversionReport`
  （exact_count/rounded_count/max_error/unsupported_events）
  が実装済み。export tick 変換は i128 の
  `round_half_up` で厳密。
- `crates/synvelis-project/src/time.rs`:
  `MusicalTick`/`MidiTick`/`MediaTimeValue`/`MediaDuration`
  が別 newtype。`ProjectTime::{Musical,Media}`。
- `crates/synvelis-project/src/midi.rs`:
  `MidiDivision`（metrical PPQ division）・`MidiSource`・
  `MidiNote` が native tick を保持。
- `crates/synvelis-project/src/control.rs`:
  `BeatPoint` が有理数 beat（num/den、den≠0 不変条件）で
  automation point を格納 — tempo 変更で再 quantize されない。
- `crates/synvelis-project/src/store.rs`:
  `SCHEMA_VERSION: u32 = 13`、`migration.rs` が versioned
  step を管理。
- `crates/synvelis-player/src/midi.rs`・`tim02_midi_playback.rs`:
  plan→wire lowering、loop/seek/stop/panic/chase、
  RT/offline parity が fixture 化済み。
- `crates/synvelis-app/tests/tim01_ppqn_*.rs`:
  transport 停止・recording transaction・record_enabled の
  app 層 gate、preview の non-destructive 性、UI snap/grid
  ポリシーが fixture 化済み。

## Scope

### In Scope

- Issue #19 受入条件 8 項目の契約写像（spec R-MT-01〜08）。
- 時間ドメイン分離・型境界・明示変換経路の固定。
- PPQN reprojection の対象/非対象 site の確定。
- SMF Format 0/1 fidelity・ConversionReport・event 順序の
  確定。
- 受入証拠台帳 `evidence/sv19/case-ledger.md`。
- 先行 slice fixture との被覆監査（fixture 存在はソース
  pin、実測は先行 slice の SHA/環境を参照）。

### Out of Scope

- コード実装（TIM-01/02/03/04・Ruviform#189 等の Owner
  Issue の責務。本 slice は契約固定のみ）。
- 実 MIDI device・実 plugin・実 OS/GUI の検証
  （synvelis#253・ruviform#253/#307・TIM-03/04 の別 gate）。
- SMF SMPTE/Format 2/MIDI 2/MPE（TIM-F01/F02 Deferred）。
- Ruviform generic Rational Time/Clock API の再定義。

## Components and Boundaries

### 所有境界（Owner マップ）

| 契約領域 | Owner | 本 slice の扱い |
| --- | --- | --- |
| Project PPQN op・reprojection | TIM-01 (#125) `synvelis-project` | 契約参照・fixture 被覆監査 |
| app 層 gate・preview・UI snap/grid | TIM-01 (#125) `synvelis-app`/`synvelis-workspace` | 契約参照 |
| SMF Format 0/1 codec・provenance | TIM-02 (#126) `synvelis-project::smf` | 契約参照 |
| PPQN×SMF cross-golden | TIM-02 (#126) `tim_02_ppqn_smf.rs` | 契約参照 |
| player MIDI RT/offline・loop/seek/panic | TIM-02 (#126) `synvelis-player` | 契約参照 |
| recording phrase/note safety | TIM-02 (#126) `synvelis-recording` | 契約参照 |
| plugin process/transport precision | TIM-03 (#127) | Deferred（capability 分類のみ固定） |
| plugin state snapshot 品質 | TIM-04 (#128) | Deferred |
| generic rational musical time | Ruviform#189 | 外部 Owner（再定義禁止） |
| 実 MIDI device backend | synvelis#253/ruviform#253 | Deferred（別 gate） |
| SMPTE/Format2/MIDI2 | TIM-F01 (#129)/TIM-F02 (#130) | Deferred/Unsupported |

### 時間ドメイン境界（設計の核心）

```text
source-native MIDI tick ──(native division, provenance)──▶ SmfDocument
        │ paired_notes() 明示経路のみ
        ▼
project musical position ──(MusicalTick on project PPQN grid)
        │ 正本: exact rational beat (ruviform BeatPosition)
        │ 変換: to_beat → BeatPosition → from_beat
        │ policy: Exact | Round(declared Rounding)
        ▼
media time (MediaTimeValue/Duration, rational frames)
        │ TickTimeMapping::tick_to_media（checked）
        ▼
engine/plugin frame (i64 frame base) ──▶ device/recording frame
```

各段の変換は **一方向・明示・checked** であり、逆方向や
暗黙の tick 比較・加算は存在しない。`MusicalTick` と
`MidiTick` は別 newtype で、それぞれ `i64` との
`From`/`TryFrom` のみを持ち、domain を跨ぐ `From`（例:
`From<MusicalTick> for MidiTick`）は提供しない
（`time.rs` の macro・`midi.rs` で確認済み）。

### PPQN reprojection の site 分類

**再投影対象（musical site — exact beat 経由）:**

- tempo map の beat 位置・time signature 位置
  （`tempo_map.tempo_events`/`signature_events` の tick）
- musical clip の anchor/extent（`TimeAnchor::Musical`・
  `ClipExtent::Musical`）
- comp range の musical 端点（take_groups target_range・
  regions project_range の `ProjectRange::Musical`）
- monitoring punch mark（PR #385 で silent retime 修正済み
  — `preview_ppqn` が inexact punch mark を報告し、
  `walk` が再投影する）

**再投影不要（既に exact rational beat 正本 — walk 対象外）:**

- automation `BeatPoint`/`AutomationPoint.pos` — 有理数
  beat（num/den）で格納されるため、PPQN 変更・tempo 変更で
  tick 換算・再 quantize が一切発生しない
  （`tim_01_ppqn.rs` `seeded_project` の `BeatPoint::new(4,3)`
  が PPQN sweep で bit-identical に保存されることで実証）。

**保存対象（不変 — reprojection しない）:**

- `MidiSource.division`・`MidiNote.start/len`（`MidiTick`）
- `SmfDocument` 全体（native tick/event order/raw provenance）
- 絶対 media anchor/extent（`TimeAnchor::Media`・
  `ClipExtent::Media`・`ProjectRange::Media`）
- device timestamp・recording/session frame・media frame
- plugin frame base・plugin instance identity
- `SnapUnit::Explicit` の保存 tick 数（UI 設計で「tick 固定・
  beat 間隔のみ変化」と確定 — `tim-01-ppqn-ui-design.md`）

## Data and Control Flow

```text
[PPQN 変更]
UI preview (syn.project.preview_ppqn, non-destructive)
  → SetProjectPpqn{old_ppqn,new_ppqn,policy}
  → project revision CAS + old_ppqn domain CAS
  → scratch clone validate（atomic・部分適用禁止）
  → musical site を to_beat→from_beat で再投影
  → commit + 単一 undo entry

[SMF import]
SMF bytes → parse_smf → SmfDocument（native division/tick）
  → ImportMidiSmf → MidiSource(native MidiTick) + smf_documents
  → immutable guard（provenance 破壊防止）

[SMF export]
export_smf(project, source_id, options{format,division})
  → provenance document or synthesized notes
  → i128 round_half_up tick 変換
  → ConversionReport{exact_count,rounded_count,max_error,
                     unsupported_events, event_count}
  → deterministic ordering（NoteOff→NoteOn at same tick）

[再生]
compile_midi_comp → MidiPlaybackPlan (MusicalTick)
  → compile_midi_events → tick_to_media → engine frame (i64)
  → ParamDispatch::dispatch_midi → plugin node (RT)
```

## Non-Functional Constraints

- **deterministic**: parse/export/ID 採番/整列は全て決定的。
  running status 不使用・最短 varlen・`(tick,kind rank,
  track,seq)` 整列。
- **fail-closed**: malformed/overflow/unsupported は typed
  error。未知 event を executable MIDI に変換しない。
- **no implicit quantization**: native tick 保存・変換は
  export/ppqn-change 時のみ明示的・誤差を report。
- **RT 境界**: `compile_midi_events`/`dispatch_midi`/driver
  transport method は non-RT orchestration。RT 経路
  （`rt.process`）は allocation-free（upstream `PluginNode`
  契約 + `rt_` fixture の `StatsAlloc` 実測）。
- **checked arithmetic**: ruviform `Tick`/`TickDuration` の
  unchecked `Add`/`Sub` は lint 対象。Synvelis 側の時間
  演算は全て `checked_add`/`checked_sub`/`checked_mul` 系。

## Design Decisions

### D1 — 正本は exact rational beat、grid tick は投影

`MusicalTick`（project PPQN grid）は表示/編集分解能の
投影であり、正本は ruviform `BeatPosition`（有理数 beat）。
PPQN=960 の整数 tick に乗らない 1/7 beat 等の位置は
`BeatPoint`/`TickPosition` の有理数表現で正確に保持され、
整数 tick への暗黙量子化は型・op・fixture の全層で禁止。
整数 tick が必要な文脈（SMF export・grid snap）では
明示 policy が必須。

### D2 — PPQN 変更は musical site のみ再投影、native/絶対
###     時間は不変

`SetProjectPpqn` は musical site を exact beat 経由で
再投影するが、source-native MIDI tick/division/raw bytes、
絶対 media anchor、device/recording frame、plugin identity
には触れない。これにより 960→1000→960 の往復で musical
position が一致し、raw MIDI timestamp・source provenance
は PPQN に依存しない。

### D3 — 変換 policy は Exact | Round(declared) の 2 値

`PpqnConversionPolicy::Exact` は非正確変換を typed error
で拒否（floor/ceil 両側で一致確認）。`Round(Rounding)` は
ruviform の宣言済み丸め mode を必須とし、既定値・暗黙
fallback を持たない。SMF export は別経路で
`round_half_up` + `ConversionReport` を使うが、これも
明示的で誤差を報告する — 「暗黙 quantize 禁止」の精神は
共通（export は source→target division の明示変換であり、
project PPQN 変更ではない）。

### D4 — Tick は PPQN 文脈必須の別型群

`MusicalTick`/`MidiTick`/`MediaTimeValue`/device frame は
別 newtype。型を跨ぐ演算は `TickTimeMapping`・
`to_beat`/`from_beat`・`tick_to_media` 等の明示関数のみ。
「異 PPQN の tick を誤って比較・加算できない」は
`TickPosition` が `Ppqn` を保持する ruviform 層と、
Synvelis の newtype 分離の二重で担保する。

### D5 — SMF は provenance document として保存、note 射影は
###     明示経路のみ

`SmfDocument` を `project.smf_documents` に保持し、
`MidiSource` は `paired_notes()` の決定的射影。import 由来
source は immutable guard で provenance 破壊を防ぎ、
明示 detach のみが編集可能 copy を作る。export は
provenance document を優先し、なければ notes から合成
（meta なし）— 「raw source を破壊的 quantize しない」
の実装形。

### D6 — Capability 分類で plugin 精度を正直に扱う

plugin transport 精度は `Exact`/`FrameAccurate`/
`BlockOnly`/`DegradedWithConsent`/`Unsupported`/`Unknown`
の class で分類。Synvelis が DAW 精度 policy（どの class を
受理するか）を、Ruviform が generic 変換/runtime adapter
を所有。headless fixture の成功を実 plugin の精度証明に
換算しない。

## Alternatives Considered

| 案 | 判定 | 理由 |
| --- | --- | --- |
| Musical position を整数 tick のみに統一 | **Rejected** | 1/7 beat 等が整数 960 tick に乗らず、正本が失われる。Rational beat 正本 + tick 投影が正しい分離。 |
| PPQN 変更時に全 tick を比率換算で一括 retime | **Rejected** | source-native MIDI tick/raw bytes を破壊する。native tick は division と共に不変保存し、musical site のみ beat 経由で再投影。 |
| PPQN を project ではなく view/session の属性にする | **Rejected** | PPQN は project の編集分解能であり save/reload で保持されるべき。view-local では project 間で不整合。 |
| SMF export で暗黙 floor/round | **Rejected** | `ConversionReport` で exact/rounded/max_error を報告する明示変換が要件。暗黙 quantize は禁止。 |
| `MusicalTick`/`MidiTick` を単一 `Tick` 型に統一 | **Rejected** | PPQN 文脈の異なる tick を型で区別できず、誤比較・誤加算を防げない。newtype 分離が安全側。 |
| plugin 精度を単一「対応/非対応」にする | **Rejected** | `Exact`〜`Unknown` の連続的 class が実態（CLAP/VST3 で精度が異なる）。binary では DegradedWithConsent 等を表現不能。 |

## Failure / Recovery Semantics

| 失敗 | 検出 | 回復 |
| --- | --- | --- |
| PPQN 範囲外（0, >32767） | `SetProjectPpqn` validate | typed `Error::Validation`、project 無変更 |
| Exact 変換が非正確 | floor/ceil 不一致 | typed error、scratch clone 破棄で atomic 拒否 |
| tick 変換 overflow | `checked_*` / `TickPosition` error | `Error::Overflow`、部分適用なし |
| SMF malformed/truncated varlen/length 不一致 | `parse_smf` | `SmfError::Malformed{offset,detail}` |
| SMF unsupported（SMPTE/Format2/zero division） | `parse_smf` | `SmfError::Unsupported`/`Malformed` |
| SMF tick 蓄積 overflow | varlen checked accumulate | `SmfError::Overflow` |
| unclosed note（note-on に対応 off なし） | `paired_notes()` | `Error::Validation` |
| CAS mismatch（revision/old_ppqn） | apply 時照合 | typed conflict、無変更 |
| recording open 中の PPQN 変更 | op validate | typed error、無変更 |
| app gate 違反（transport playing 等） | `SynvelisApp` 前提検査 | `invalid_params`、無変更 |

## Fixture Mapping（被覆監査の要約 — 詳細は evidence/sv19）

| 要件 | 代表 fixture |
| --- | --- |
| R-MT-01 PPQN 既定/変更/保存 | `tim_01_ppqn.rs` sweep matrix・`tim01_ppqn_app.rs`・`tim01_ppqn_preview.rs`・`tim01_ppqn_ui.rs` |
| R-MT-02 Rational beat 正本 | `ppqn_denominator_boundary_is_explicit`・automation `BeatPoint` fixture・ruviform `BeatPosition` |
| R-MT-03 Non-destructive reprojection | `ppqn_change_preserves_*`・`ppqn_exact/round_change_preserves_smf_*`・punch mark fixture |
| R-MT-04 Explicit conversion | `PpqnConversionPolicy`・`smf_export_ppqn_down_reports_rounding_and_max_error` |
| R-MT-05 Tempo/Transport/Loop | `tim02_midi_playback.rs`・`tim_02_midi_phrase.rs`・`tempo.rs` test |
| R-MT-06 SMF Fidelity | `tim_02_smf.rs`・`tim_02_ppqn_smf.rs`・cross-golden |
| R-MT-07 Compatibility | `alib02_v5_real.rs`・`monitoring_persist.rs`・schema v13 migration |
| R-MT-08 Safety/Evidence | overflow/denominator/RT fixture・`evidence/sv19/case-ledger.md` |

## Deferred Design Concerns

- **実 MIDI device backend** — synvelis#253/ruviform#253。
  headless SMF fixture は file/contract 検証であり、実機
  device timestamp・driver 精度は別 gate。
- **実 plugin transport 精度** — TIM-03 (#127)・
  ruviform#307。capability class の実機検証は別 slice。
- **Plugin state snapshot** — TIM-04 (#128)。
- **SMF SMPTE/Format2/MIDI2** — TIM-F01 (#129)/
  TIM-F02 (#130)。
- **Ruviform#189 generic rational time** — 外部 Owner。
  Synvelis は consumer として契約を参照するのみ。
- **merge 後対象 SHA の CI 証拠** — 本 slice は docs のみ。
  統合受入は Coordinator が対象 SHA CI で確認。

## Verification Notes

- 本 slice は docs/evidence のみ。実装 crate の
  `check`/`lint`/`test` は再実行しない — fixture の存在は
  ソース pin、先行実測は SHA/環境付きで参照。
- 正規検証: `mise run fmt-check`（docs/設計スライス）。
- docs index は `doc-maintenance` の `index apply` で再生成、
  `doc_status.js lint` で touched dir を検査。
