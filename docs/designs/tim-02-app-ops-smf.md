---
id: "DESIGN-034XmM4rWsUgOxCaZeItMq"
type: "design"
status: "approved"
title: "TIM-02 SMF app ops — op 配線 + MIDI clip compile 証拠設計"
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
  implements:
    - "docs/specs/tim-02-app-ops-smf.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-02-smf-format01-core.md"
    - "docs/designs/tim-01-app-ppqn-design.md"
    - "docs/designs/tim-02-player-midi-rt-offline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/synvelis/issues/129"
    - "https://github.com/scarecrow173/synvelis/issues/130"
  deferred-by: []
---

# TIM-02 SMF app ops — op 配線 + MIDI clip compile 証拠設計

## Context

spec `tim-02-app-ops-smf` を、TIM-01 `syn.project.set_ppqn`
の op 経路前例と SMF core の既存 API のみで実現する。
project 層（`parse_smf`/`export_smf`/`ImportMidiSmf`/
`SmfConversionReport`）は main 済みで新規コード不要。

既存事実（調査済み）:

- `service.rs` の `syn.project.*` op table に
  `import_smf`/`export_smf` エントリなし。unknown op は
  既存の拒否経路で弾かれる。
- `capabilities()` に `midi` 節なし。`recorder_capabilities`
  が `unavailable`+reason の正直表示パターンの前例。
- `plan_playbacks`（lowering.rs）は audio clip のみ処理し、
  `clip.source_id` が `midi_sources` 由来の clip を
  `MissingSource` skip → `skipped_fault` で build fault に
  していた（MIDI clip が audio build を fault させる誤分類）。
- plugin node への MIDI 配送には `RoutingControlDispatch` の
  midi seam + `routing_engine` の plan merge（instrument
  node = MIDI plan / processor node = param plan の
  single-plan split）が必要 — `routing_control.rs` は
  lease 外かつ設計判断を要するため、Coordinator 確定
  （msg_f545cf178c4c, option 1）により **plan → compile →
  engine 証拠まで**を本 slice の範囲、node 配送を未配線の
  まま別 slice とする。

## Scope

### In Scope

- op 配線: `service.rs` op table + dispatch match、
  `app.rs` handler、`binding.rs` mint。
- `app.rs` の params/error helper 共有化（`string_param`・
  `optional_u32_param`・`expected_revision_param`・
  `project_mutation_gate`・`smf_error_body`・`smf_io_body`）。
- `capabilities()` への `midi` 節追加。
- `lowering.rs`: `SkippedPlayback::MidiClip` variant・
  `plan_midi_clips`/`plan_midi_clip`/`MidiClipPlan`・
  comp zone 解決の共有化（`comp_zones`/`superseding_zone`
  抽出）。
- `engine.rs`: `MidiClipEvidence`・`Inner.midi_evidence`・
  rebuild 時 compile・`midi_clip_evidence()` getter。
- `routing_engine.rs` / `export.rs`: 新 enum variant の
  match arm（compile-forced ripple、PR に明記）。
- `tests/tim02_smf_app.rs` fixture。

### Out of Scope

- `RoutingControlDispatch` midi seam・routing_engine plan
  merge・`dispatch_midi` node install（decision-grade、
  後続 slice）。
- project/recording crate・UI・gateway 層。

## Components and Boundaries

### op 経路（TIM-01 踏襲）

```text
UiRuntime::dispatch_op("syn.project.import_smf", params)
  → service.rs op table（登録名 + params schema 説明）
  → dispatch match → AppService::project_import_smf(params)
  → string_param("path") / expected_revision_param /
    project_mutation_gate
  → fs::read → parse_smf → ProjectOp::ImportMidiSmf
  → session.apply → { source_id, format, division, tracks,
                      controllers, bytes, revision }
```

`export_smf` は `source_id`（`u64` tag）+ `division`/
`format` option + `path` を受け、**source 存在確認を
division 既定値解決より先に**行う（未知 source を
division-0 guard の `unsupported` で誤爆させない）。
`export_smf` → `SmfFile.to_bytes()` → `fs::write`。
返却は `ConversionReport` 全 field + `bytes` + `path`。
export は session を変更しない read-only op（undo 履歴に
乗らない）。

### typed error 写像

- `smf_error_body(&SmfError)`: `Malformed`/`Unsupported`/
  `Overflow`/`NotFound` → `ErrorBody` に kind を
  `details.smf_error` へ写像。
- `smf_io_body(op, path, &io::Error)`: `service_error`
  kind + `details.io_error`（NotFound/PermissionDenied/
  その他を区別）。
- session apply 失敗・params 不正は `invalid_params`。

### `midi` capabilities 節

`midi_capabilities()`（関連関数、`self` 不要）が以下を返す:

- `smf`: `available` + 2 op 名・`format0`/`format1`・
  `ppqn 1..=32767`。
- `backend`: windows = `implemented`/`winmm`/
  `real_port: not_run`（ruviform#253、実 port 検証は外部
  gate）、macos/linux = `unsupported`。
- `unsupported`: `smpte_division`・`smf_format2`（→
  synvelis#129）、`midi2`（→ synvelis#130/ruviform#317）。
- `player_clip`: `partial` — clip→engine-frame MIDI event
  compile は headless で動作、instrument node 配送 seam は
  別 gate と明記。

### player clip→MIDI（plan → compile → evidence）

```text
plan_playbacks (audio pass)
  └─ midi_sources 由来 clip → SkippedPlayback::MidiClip（fault ではない）
plan_midi_clips
  └─ 各 midi clip → MidiClipPlan { clip, selection: MidiCompSelection,
        lanes: smf_documents[source].controller_events() }
engine.rs rebuild（audio prepare 前）
  └─ compile_midi_clip_evidence: plan → compile_midi_comp →
     compile_midi_events → MidiClipEvidence { clip_id, source_id,
        events, error }
```

- `MidiClipPlan` は clip 単位の single-region
  `MidiCompSelection`（clip の musical range 全体）と、
  `smf_documents` provenance の controller lane を保持する。
- `routing_engine.rs` の `skipped_fault` は `MidiClip` →
  `None`（意図的 silence — MIDI pass が所有）。`export.rs`
  の offline render 経路では `UnrenderableClip`（audio
  render 不能を正直に拒否 — 従来 `MissingSource` で拒否
  していた実効挙動を保存）。
- compile は `TimeBase::sample_rate`（Result 受け）を使い、
  fallback channel 0。失敗は `MidiClipEvidence.error` の
  typed 文字列として記録し panic しない。
- muted clip は証拠を生成しない。

## Data and Control Flow

```text
SMF file ──op──▶ parse_smf ──▶ ImportMidiSmf ──▶ Project
   { midi_sources[sid], smf_documents[sid] }
        │ save/load │        ▼ export_smf + fs::write
        ▼            SMF file + ConversionReport
midi clip（source_id ∈ midi_sources）
   ──plan_midi_clips──▶ MidiClipPlan
   ──compile_midi_comp/compile_midi_events──▶ MidiClipEvidence
   （node 配送は未配線 — routing seam は後続 slice）
```

## Non-Functional Constraints

- fail-closed: malformed/overflow/unsupported/not found/IO/
  transport 中は全て typed error。unknown source が
  division guard に吸収されないよう存在確認を先に行う。
- no RT 持込み: SMF file IO は headless 境界の `std::fs`
  のみ。compile 証拠は rebuild（non-RT）で生成し RT
  callback に触れない。
- honesty: node 配送未配線・外部 gate NotRun を
  capabilities・doc・evidence・PR に明記。
- additive: capabilities 節・op エントリ・enum variant は
  全て追加のみ。既存 op の拒否経路は維持。

## Risks and Trade-offs

- `SkippedPlayback::MidiClip` variant 追加により
  `routing_engine.rs`/`export.rs` の match が compile-forced
  で更新必要 → lease 外の ripple として最小 arm のみ追加し
  PR に明記。`skipped_fault` で `None` を返すのは誤
  `MissingSource` fault の修正でもある。
- `MidiClipPlan` は生成されるが routing_engine merge 未実装
  のため現時点では消費者なし → 意図的な死に体として doc/
  evidence に明記し、seam 実装時の受け皿とする。
- engine 証拠は adapter 層の診断用で永続化しない →
  rebuild ごとに再計算され、`midi_clip_evidence()` で
  取得可能。保存形式の後方互換に影響しない。

## Deferred Design Concerns

- node 配送 seam — `routing_control.rs` midi dispatch +
  `routing_engine` plan merge | Coordinator 起票の後続
  issue | clip MIDI が instrument に届かない | 高
- routing_engine 側の `MidiClipPlan` 消費 | seam 実装時 |
  plan 未接続 | 中
- 実 OS backend・L3・oracle — 外部 gate | 同左 | 中/低

## Verification Notes

- [ ] `mise run test synvelis-app` で tim02_smf_app 14 tests
  全緑 + 全 crate suite 回帰なし
- [ ] `mise run check`/`lint`/`test`（app/player/render）・
  `test-rt`・`check-app-native`・`lint-app-native`・
  `test-msrv`・`build synvelis-app`・`prek` 全 lane 実走
- [ ] 外部 gate（実 OS backend・RF #629/#630 L3・AL f64
  oracle）は NotRun/別 gate と記録
