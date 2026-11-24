---
id: "SPEC-034XmK9qVrTfNwBzYdHsLp"
type: "spec"
status: "proposed"
title: "TIM-02 SMF app ops — syn.project.import_smf/export_smf 配線 + capabilities MIDI 節 (synvelis#126)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
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
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/tim-01-app-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/253"
  defers:
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/synvelis/issues/129"
    - "https://github.com/scarecrow173/synvelis/issues/130"
    - "https://github.com/scarecrow173/ruviform/issues/317"
    - "https://github.com/scarecrow173/ruviform/issues/294"
  deferred-by: []
---

# TIM-02 SMF app ops — `syn.project.import_smf`/`export_smf` 配線 + capabilities MIDI 節 (synvelis#126)

## What Should Be Built

Issue #126（TIM-02）の headless 残件のうち、製品 op 面を
`synvelis-app` の `syn.*` op registry に配線する。
`tim-02-smf-format01-import-export` が確立した
`parse_smf` / `export_smf` / `ProjectOp::ImportMidiSmf` /
`SmfConversionReport` を利用し、project 層に新規コードを
追加しない。

- `syn.project.import_smf`: params の SMF ファイル path を
  `std::fs` で読み、`parse_smf` → `ProjectOp::ImportMidiSmf`
  を session apply する。返却に採番された `source_id` と
  import サマリ（format・division・tracks・controllers・
  bytes）を含める。file IO・parse・apply の失敗は typed
  error として `ErrorBody` で返す。
- `syn.project.export_smf`: params の `source_id`・division/
  format option・出力 path から `export_smf` を呼び、
  `SmfFile` bytes を出力 path へ書き出す。返却に
  `ConversionReport { exact_count, rounded_count,
  max_error, unsupported_events }` を含める。export は
  read-only op とし、undo 履歴を汚染しない。
- `syn.app.capabilities` に `midi` 節を additive に追加し、
  SMF op・WinMM backend の OS 別状態（Windows=実装済・
  実 port=NotRun、macOS/Linux=Unsupported）・SMPTE/
  Format2（synvelis#129）・MIDI2（synvelis#130 /
  ruviform#317）を正直に区別表示する。
- player clip→MIDI は **plan → compile → engine 証拠**
  までを配線する。`midi_sources` 由来 clip を
  `plan_midi_clips` → `MidiClipPlan` として plan し、
  `MidiCompSelection` → `compile_midi_comp` で engine
  frame の MIDI event 列に compile して `Engine` 側の
  typed evidence として保持する。plugin node への最終
  配送（`ParamDispatch::dispatch_midi` 系の node seam）と
  `RoutingControlDispatch` の plan merge は**未配線のまま**
  とし、別 slice の decision-grade 変更として明記する。

## Why Now

`syn.project.set_ppqn`（TIM-01）で `syn.project.*` op の
配線経路（service op table → dispatch match → app handler
→ binding mint）が確立済みであり、SMF core・player
lowering・doc 検証・PPQN cross-golden の土台も main 済み。
残る製品面の gap は op registry への登録と clip→MIDI の
app 側 compile 証拠だけであり、本 slice が Issue #126 の
headless 残件の製品 op 面を閉じる。

node 配送 seam は `routing_control.rs` / `routing_engine.rs`
の plan merge に関わる設計判断（instrument node = MIDI
plan、processor node = param plan の single-plan split）を
要し、本 slice の lease 範囲を超えるため Coordinator 確定で
意図的に未配線とする。

## Users and Value

- DAW 利用者: headless op 経路から SMF を project に
  取り込み・書き出しでき、undo/redo・save/reload を通じて
  provenance が保持される。
- 開発者: op 経路 fixture で import→undo/redo→save/reload→
  export→再 import の整合と typed error を決定的に検証
  できる。`syn.app.capabilities` が MIDI 機能の実装済/
  NotRun/Unsupported/Future を公開能力として正直に返す。

## Scope

### In Scope

- `crates/synvelis-app/src/service.rs`: `syn.project.*` op
  table への `import_smf`/`export_smf` エントリ追加と
  dispatch match arm。
- `crates/synvelis-app/src/app.rs`: `project_import_smf` /
  `project_export_smf` handler・`smf_error_body` /
  `smf_io_body` typed error 写像・`capabilities()` への
  `midi` 節追加。
- `crates/synvelis-app/src/binding.rs`: 2 op の mint。
- `crates/synvelis-app/src/engine.rs`: rebuild 時の MIDI
  clip compile 証拠（`MidiClipEvidence`）と getter。
- `crates/synvelis-player/src/lowering.rs`: `plan_playbacks`
  の midi_sources 由来 clip 処理（`SkippedPlayback::MidiClip`
  + `plan_midi_clips` → `MidiClipPlan`）。
- `crates/synvelis-app/tests/tim02_smf_app.rs`（新規）:
  `SharedService`/`UiRuntime::dispatch_op` fixture。
- `docs/specs`・`docs/designs`・`docs/test-specs` の
  `tim-02-app-ops-smf` 3 文書と evidence TIM-02 節の更新。

### Out of Scope

- plugin node への MIDI 配送最終 hop（`dispatch_midi` install）
  と `RoutingControlDispatch` midi seam / `routing_engine`
  plan merge — decision-grade の後続 slice。
- `crates/synvelis-project/`・`crates/synvelis-recording/`・
  `synvelis-player` の `lowering.rs` 以外・`synvelis-app`
  列挙外ファイル・`ui/` の変更。
- 実 OS MIDI backend・RF #629 仮想 WinMM Port・RF #630 Test
  Instrument の L3・AudioLens f64 oracle（al#127）— 全て
  外部 gate として NotRun/別 gate。
- SMPTE division・SMF Format 2（synvelis#129）・MIDI 2.0/
  MPE（synvelis#130・ruviform#317）。
- `synvelis-gateway` 層への公開（synvelis#46 契約次第）。
- 旧物理 MIDI 3 ignored test の Pass 化（歴史的 NotRun 維持）。
- 新規依存・新規 CI lane・`Cargo.toml`・`mise.toml` の変更。

## Requirements

- `import_smf` は file 不存在・読取失敗を IO typed error、
  `parse_smf` の `SmfError`（Malformed/Unsupported/
  Overflow/NotFound）を `details.smf_error` kind 付きで返し、
  transport 再生中は mutation gate で拒否する。
- `export_smf` は未知 `source_id` を `not_found`、division
  0 を `unsupported` で拒否し（source 存在確認を division
  既定値解決より先に行う）、書込失敗を IO typed error で返す。
  成功時は `SmfFile` bytes を出力 path へ書き出し、
  `ConversionReport` 全 field を返却に含める。
- 両 op は `expected_revision` を任意受理し、TIM-01 の
  `project_set_ppqn` と同じ op 経路・error 規約に従う。
- `capabilities()` の `midi` 節は未配線領域を `unavailable`/
  `unsupported`/`partial` + reason で正直に返し、成功と
  混同させない。
- `plan_playbacks` は midi_sources 由来 clip を audio pass で
  `MissingSource` fault にせず、MIDI plan として別 pass で
  plan する。compile 失敗は typed evidence 文字列として記録
  し、panic しない。muted clip は証拠を生成しない。

## Acceptance Criteria

- [ ] `syn.project.import_smf`/`export_smf` が op registry
  経由で dispatch され、TIM-01 と同型の typed error /
  receipt を返す。
- [ ] `tests/tim02_smf_app.rs` で import→undo/redo→
  save/reload→export→再 import の整合と typed error
  （malformed/unsupported/not found/IO/transport 中拒否）
  が固定される。
- [ ] `syn.app.capabilities` の `midi` 節が SMF op・OS 別
  backend 状態・SMPTE/Format2/MIDI2 の区別を返す。
- [ ] midi_sources 由来 clip が engine frame の MIDI event
  列に compile され、`MidiClipEvidence` として取得できる。
  node 配送未配線が doc・evidence・PR に明記される。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native` / `lint-app-native` /
  `test-msrv` / `build` / `prek` が通る。外部 gate は
  NotRun/別 gate と記録する。

## Deferred Design Concerns

- plugin node への MIDI 配送 seam — `routing_control.rs`/
  `routing_engine.rs` の single-plan split 設計判断 |
  後続 slice（Coordinator が follow-up issue を起票）|
  clip の MIDI event が instrument node に届かない | 高
- 実 OS MIDI backend・仮想 Endpoint/L3 — ruviform#253・
  RF #629/#630 | 外部 gate 整備時 | 実機 timing 未検証 | 中
- AudioLens f64 oracle 非回帰 — al#127 別 gate | 同左 |
  float 精度差の見逃し | 低
- `MidiClipPlan` の routing_engine 側消費 — plan は生成済み
  だが merge 未実装 | node seam 実装時 | plan が死に体化 |
  中

## Sources and Relations

- Issue #126（TIM-02 tracker）: 本 slice は auto-close せず
  `Refs #126` のみ。
- `docs/specs/tim-02-smf-format01-import-export.md`・
  `docs/designs/tim-02-smf-format01-core.md`: SMF core と
  provenance document の契約。
- `docs/specs/tim-01-app-ppqn.md`: `syn.project.*` op 配線の
  前例（op table/dispatch/handler/mint/fixture 構造）。
- `docs/specs/tim-02-player-midi-rt-offline.md`: player MIDI
  RT/Offline 結合の別 slice。
- ruviform#253: OS MIDI backend（Windows WinMM 実装済・
  実 port 未検証、macOS/Linux 未実装）。
