---
id: "EXP-sv19-musical-time"
type: "experiment"
status: "completed"
title: "Musical Time — 設計契約写像の実験記録（synvelis#19）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/19"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/musical-time.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/musical-time-design.md"
    - "docs/specs/musical-time.md"
    - "docs/tasks/musical-time-task.md"
    - "docs/plans/musical-time.md"
    - "docs/test-specs/musical-time.md"
    - "evidence/sv19/case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv19/case-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/129"
    - "https://github.com/scarecrow173/synvelis/issues/130"
    - "https://github.com/scarecrow173/synvelis/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/307"
  deferred-by: []
---

# Musical Time — 設計契約写像の実験記録（synvelis#19）

## 仮説

Issue #19 の dispatch 時点で、Musical Time の要求の大半は
TIM-01 (#125)・TIM-02 (#126) の先行 slice により main
統合済みであり、本 task の実作業はコード変更ではなく
docs chain・証拠台帳・被覆監査に限定されるのではないか
— PPQN/time/SMF の既存 spec/design・実装ソース・fixture
の棚卸しで確認する。

## 方法

1. **Issue #19 body の取得** — 受入条件 8 項目・実装指針・
   「実 MIDI/OS Gate は NotRun」「設計完了を実装 PASS と
   混同しない」を確認。
2. **doc-driven-dev-graph の実行** — `.agents/skills/
   doc-driven-dev-graph/scripts/` の `inspect_graph.js`/
   `route_graph.js` を repository-local に実行。graph
   inspection は成功したが route projection は
   `status: blocked`/`condition: blocked`/`next: probe` で、
   blocker は `artifact-graph`・多数の pre-existing broken
   relations（`defers`/`depends-on`/`references`/`related`/
   `source` target）・`focus-required`。これは repository
   全体の既存状態であり本 slice 範囲外 — 修復せず記録。
3. **実装ソースの pin** — `ops.rs`（`SetProjectPpqn`/
   `ImportMidiSmf`）・`ppqn.rs`（`PpqnConversionPolicy`/
   `to_beat`/`from_beat`）・`smf.rs`（`SmfDocument`/
   `export_smf`/`ConversionReport`）・`time.rs`（別 newtype）・
   `midi.rs`（`MidiDivision`/`MidiSource`）・`control.rs`
   （`BeatPoint` 有理数 beat）・`store.rs`（`SCHEMA_VERSION=13`）
   を確認。
4. **fixture 棚卸し** — `tim_01_ppqn.rs`・`tim_02_smf.rs`・
   `tim_02_ppqn_smf.rs`・`tim02_midi_playback.rs`・
   `tim_02_midi_phrase.rs`・`tim01_ppqn_{app,preview,ui}.rs`
   の test 関数名を列挙し、受入条件への写像を監査。
5. **先行 docs の参照** — sv17 `project-persistence` chain
   を template として front matter/relation/status/ledger
   構造を踏襲。

## 観測結果

- **仮説は支持された** — TIM-01/02 の契約はほぼ全て実装・
  fixture 化済み。本 slice は契約写像・被覆監査・証拠分類
  の docs/evidence 作業に限定できた。
- **時間ドメイン分離は型で実現済み** — `MusicalTick`/
  `MidiTick`/`MediaTimeValue`/device frame が別 newtype、
  `TickPosition` が `Ppqn` を保持、変換は `to_beat`/
  `from_beat`/`tick_to_media` の明示経路のみ。
- **PPQN reprojection の境界が明確** — musical site のみ
  beat 経由で再投影、source-native/絶対 media/device/
  plugin frame は不変。punch mark の silent retime は
  PR #385 で修正済み。
- **SMF fidelity は provenance document で実現** —
  `SmfDocument` 保存・`paired_notes()` 明示射影・immutable
  guard・`ConversionReport` で誤差報告。
- **被覆の隙間を特定** — 「1/7 beat」専用 fixture・実 SMF
  ファイル E2E・実 device/plugin 検証は未構成/別 gate。
  これらを ledger で NotRun/Deferred として正直に分類。

## 採用した判断

- **docs chain は sv17 `project-persistence` の構造を踏襲** —
  front matter（`SPEC-`/`DESIGN-`/`PLAN-`/`TASK-`/`TSPEC-`/
  `IMPL-`/`EXP-` prefix・relation field・status 語彙）を
  一貫させ、lint/reciprocal 整合を確保。
- **受入条件は `SV19-C01`〜`SV19-C08` で写像** — 各 Case に
  根拠 fixture・Expected/Observed・Status・丸め誤差証拠を
  割当。fixture 存在はソース pin、実測は先行 slice 参照。
- **「1/7 beat」は専用 fixture 不在を明記** — denominator
  boundary fixture が同値の性質を被覆するが、1/7 そのもの
  の pin はなく、generic rational time は ruviform#189 の
  別 gate として Deferred/NotRun に分類（過大主張を回避）。
- **実機・実 device/plugin は一貫して Deferred** — headless
  fixture の Pass を実機受入に換算しない（spec R-MT-08）。

## 棄却した代替案

- **コード変更を含める** — 却下。実装は TIM-01/02/03/04・
  ruviform#189 の Owner Issue の責務。本 slice の編集範囲は
  docs/evidence のみ。
- **「1/7 beat」専用 fixture を本 slice で新設** — 却下。
  fixture 新設は実装 crate の変更であり範囲外。必要なら
  Ruviform#189/TIM Owner の別 gate。
- **graph の broken relations を修復** — 却下。repository
  全体の pre-existing 状態で本 slice 範囲外。記録のみ。
- **fixture を本 slice で再実行** — 却下。タスク指定の正規
  検証は `mise run fmt-check`（docs/設計スライス）。実装
  crate の test は対象外であり、先行実測を SHA/環境付きで
  参照する方が正直。

## 未解決の所見

- **graph-router の pre-existing broken relations** —
  `route_graph.js` が `blocked`/`focus-required` を返す
  原因は repository 全体の relation 不整合。本 slice の
  新規文書は新たな broken relation を追加していないが、
  graph 全体の健全性は別タスクでの監査が必要。
- **実 SMF ファイル（外部 DAW 由来）の E2E** — headless
  合成 fixture は file/contract 検証であり、実ファイルの
  import/export round-trip は TIM-02/実機 lane の別 gate。
- **merge 後対象 SHA の CI 証拠** — 本 slice は docs のみ
  でコード変更なし。統合受入は Coordinator が対象 SHA CI
  で確認するまで未確定。
