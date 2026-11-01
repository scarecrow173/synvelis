# Synvelis — Configurable PPQN / Resolution-independent Musical Time

- **Status:** Approved architecture direction (2026-11-01). Detailed API, migrations and PoC pending.
- **Decision tracker:** [Synvelis #19](https://github.com/scarecrow173/synvelis/issues/19)
- **Ruviform migration:** [#189](https://github.com/scarecrow173/ruviform/issues/189)
- **Successor ADR:** [Variable PPQN / Rational Beat Position](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/configurable-ppqn-resolution-independent-musical-time.md)
- **Superseded historical ADR:** [Fixed 960 Music Time](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/musical-time-model.md)
- **Related Synvelis:** [#6 Automation](https://github.com/scarecrow173/synvelis/issues/6), [#10 Project Model](https://github.com/scarecrow173/synvelis/issues/10), [#11 Timeline](https://github.com/scarecrow173/synvelis/issues/11), [#12 Recording](https://github.com/scarecrow173/synvelis/issues/12), [#17 Persistence](https://github.com/scarecrow173/synvelis/issues/17), [#18 Execution Graph](https://github.com/scarecrow173/synvelis/issues/18)

> Synvelisが採用したのは**設計方針**である。RuviformのRust実装は現時点で固定PPQN=960、`Tick(i64)`・`TempoMap`等が960に依存している。新しい型/APIやMigrationは未実装。

## 1. 採用した設計

1. 新規Synvelis ProjectのPPQN既定値は**960**。ProjectはPPQNを設定・保持できる。Project形式はSchema Versionによって互換性を明示する。
2. Music Timeの正本はPPQN=960整数Tickの格子に閉じない。**正規化された有理数Beat Positionまたは意味的に同等の損失のない表現**を用いて、7連符等の分母の異なる拍位置も正確に扱えるようにする。
3. Tickは独立した正本ではなく**特定PPQNを伴う表現**。異なるPPQNを持つTickの整数値を黙って比較/加算しない。
4. Audio Source Native Frame、MIDI Source Native Timebase/Tick/Original Timestamp、Project Musical/Media Time、Engine Frame、Device Clock Epoch、Recording Session Frameを区別する。
5. PPQN変更は音楽位置を変える操作ではなく、Quantize・Stretch・Raw Event Retimingと混同しない。Sourceの元Bytesと元Timestampを非破壊で保持する。
6. RuviformはGeneric Beat/PPQN/Time Conversion、TempoMap、Plugin Transport Adapterを担当。SynvelisはProject Default/Settings/Undo/Migration、Clip Source Mapping、Recording SourceとUIを担当。

## 2. 論理的な時間表現

~~~mermaid
flowchart TD
    P["Project Musical Position\nRational Beat"]
    R["Project PPQN\nConfigurable / Default 960"]
    I["Imported MIDI Source\nNative Division / Tick"]
    D["Recorded MIDI\nDevice Timestamp / Epoch"]
    T["Tempo Map\nBeat ↔ Media Time"]
    F["Engine Frame/Event Offset"]
    X["CLAP / VST3 Transport"]
    R -. "Tick projection" .-> P
    I -->|"lossless native retention + explicit mapping"| P
    D -->|"native time anchor + explicit mapping"| P
    P --> T --> F
    P -->|"adapter conversion"| X
~~~

Canonical Musical Positionの具体的な整数幅、分母・正規化規則、Comparison、Hash、Overflow ContractはPoCで決定する。`Ratio<n,d>`のような概念で考えるが、無制限精度・無限大の分母を保証しない。不可表現・Overflowの場合は意図しない丸めではなく明示的な結果/エラーを返す。

## 3. Timebaseと保存の分離

| 時間/識別 | 所有・不変条件 |
| --- | --- |
| Project Beat Position | PPQN非依存のMusic Positionの正本 |
| Project PPQN | Project設定（既定960、Validationあり） |
| Source Native Division / Tick | MIDI Fileの元時刻。SMFはPPQN以外のDivisionもあり、元形式を保存 |
| Source Recorded Timestamp | Device Clock/Epoch等、Raw Recorded Eventは不変 |
| Project Media Time | TempoMapでBeatと変換。変換時だけ明示丸め |
| Engine Frame | RT/Offlineの処理位置。Loop/Seek時もProject Timeとは独立 |
| Plugin Transport Position | 規格別変換。現在のRuviform固定960前提は将来廃止 |

`ProjectPPQN=960`であることと、全Noteが960分解能の整数位置にあることは同義ではない。

## 4. Project PPQN変更の契約

- ProjectControllerの単一CommandとしてValidation/Undo/Redoを提供する（#17）。
- 音楽位置が正本であるため、単なるPPQN設定変更でClip/Note/Automationの意味的な位置は変わらない。
- Clipや外部リンクが元Tick値を保持している場合は、元Division/Mappingを維持し、強制Re-quantizeしない。
- 不足した表現精度/Overflow/外部Export Lossはユーザーへ可視化する。Unsupported Rangeは定義済みRejectまたは設定差分の提示。
- 新しいPPQNのExportやMIDI Importに必要なTick Projectionは明示丸め、失われる精度を示す。
- Project Save/Reload、Snapshot Diff、VCS、Autosave/MigrationがPPQN設定とRaw Sourceを分離し保持する。

## 5. Ruviform基盤の拡張（旧APIからの移行）

| 現行実装 | 移行目標 |
| --- | --- |
| `Ppqn` only 960 | Validated Configurable Resolution（範囲はPoC） |
| `Tick(i64)` = implicit 960 | Tick+PPQN文脈または型安全変換、Beat Positionと分離 |
| `TempoMap` hardcoded 960 | Beat Position基準の変換・Immutable Compiled Map |
| `MusicalTimeInfo` ppqn=960 | 実際の分解能 / 規格独立なBeat解釈 |
| CLAP/VST3のtick/960 | 正しいBeat Position→規格別Time Conversion |
| Legacy fixed960 tests | 可変PPQN + 正確さ + Compatibility + RT tests |

既存RuviformのAccepted ADRは後継ADRで正式に置き換えられたが、**Rustコードや旧APIは置換していない**。既存#90/#91を過去の実装記録として保全し、新しい移行作業はRuviform #189で行う。

## 6. Playback / Automation / Recordingへの接続

- **Playback:** CompilerはProject Musical PositionとSource-relative Mappingを使い、Device/Offline DriverのFrameへ変換する。Loop/Tempo Changeでイベントを二重配送しない。
- **Automation:** Track/Clip/Note-localの時間Scopeを維持し、Curvesは同じMusical Positionの評価規則を使用する。
- **Recording:** Raw MIDI Device Timestamp/Clock Domainを保全し、Project Placementは別のMapping。既定960 Tickへ即時丸めない。
- **Plugin:** Parameter EventはBlock内Frame Offsetで配送できる一方、Source Music TimeはBeat Positionで保存する。Plugin Transportの精度と入力イベントのTimestampは独立。
- **Offline:** Liveと同じ時間変換契約でイベント順序と時間位置を評価し、非決定性はPlugin/Driver Capabilityで区別する。

## 7. 受入ケース（PPQN-01〜PPQN-12）

| ID | Input / Change | Expectation |
| --- | --- | --- |
| PPQN-01 | New Project | PPQN既定960 |
| PPQN-02 | Settings→1920 | Save/Reload/Undo後も設定と位置を保持 |
| PPQN-03 | Source Division 480/960/1000/1920 | Source Native Tick/Divisionを保持 |
| PPQN-04 | 1/7 Beat Position | 整数960 Tickに無断量子化しない |
| PPQN-05 | 同Tick値・異PPQN | 誤比較を防止、明示変換が必要 |
| PPQN-06 | Project PPQN 960→1000→960 | Clip/MIDI/Automation/Raw Timestampは無変更 |
| PPQN-07 | Tempo Change・Negative Preroll | Media/Beat変換が定義済み丸めで一貫 |
| PPQN-08 | Seek/Loop/Stop | Note-on/offの重複/欠落が起こらない |
| PPQN-09 | Plugin Transport CLAP/VST3 | Beat位置と丸め誤差を正しく適用 |
| PPQN-10 | Exportできない分母 | 明示的なLoss/Quantize選択を提示 |
| PPQN-11 | 旧固定960 Snapshot | 読取/移行後の位置とSourceが一致 |
| PPQN-12 | Overflow・範囲・RT | 明示Reject、安全・有界・無割当、旧API回帰 |

## 8. PoC保留

- `BeatPosition`の整数幅/分母制限、固定分解能Tickとのプロトコル
- 可変PPQNの最大値と有効設定時機、MIDI ImportのSMF SMPTE Division対応範囲
- Project PPQN変更のUI、Piano Roll GridとSnap、Script/API compatibility
- CLAP/VST3の精度境界、Plugin TransportとMIDI Event配送、Tempo Ramp
- ベースライン実装への移行順序、形式Migrationの適用方法

**結論:** 可変PPQN（Synvelis default 960）とPPQN非依存の正確な音楽位置を採用。Raw Source情報の保持と明示Quantizeを維持し、汎用Time Foundationの実装はRuviform #189で追跡する。
