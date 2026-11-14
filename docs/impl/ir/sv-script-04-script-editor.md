---
id: "IMPL-034XnRe6ZxDUpnK0Ct3CcF"
type: "impl"
status: "completed"
title: "SV-SCRIPT-04 Script Editor — Analyzer Registry・Luau型診断/補完・driver-neutral ViewSpec 投影"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/274"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-automation/src/analyzer.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/script_editor.rs"
      - type: "file"
        path: "crates/synvelis-app/src/script_editor.rs"
      - type: "file"
        path: "crates/synvelis-automation/tests/script_analyzer.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/script_editor.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/svscript04_script_editor.rs"
      - type: "file"
        path: "docs/impl/ir/sv-script-04-script-editor.md"
      - type: "file"
        path: "docs/impl/exp/sv-script-04-script-editor-analyzer-registry-port-level-adapter-viewspec.jsonl"
      - type: "file"
        path: "docs/test-specs/sv-script-04-script-editor.md"
      - type: "dir"
        path: "evidence/sv274/"
    modified:
      - type: "file"
        path: "crates/synvelis-automation/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/state.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/view.rs"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-theme/src/status.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/274"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv-script-04-script-editor.md"
    - "docs/impl/exp/sv-script-04-script-editor-analyzer-registry-port-level-adapter-viewspec.jsonl"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/266"
    - "https://github.com/scarecrow173/synvelis/issues/267"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/267"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-SCRIPT-04 Script Editor — Analyzer Registry・Luau型診断/補完・driver-neutral ViewSpec 投影

## Summary

SV-SCRIPT-04（synvelis#274）の headless-first スライスを実装した。
`LanguageId`/contract version/`Capabilities` で analyzer を
fail-closed に解決する `AnalyzerRegistry`、注入された
`DefinitionInjection` のみを根拠に構文/型診断と補完を返す
`LuauPortAnalyzer`（port-level fake/adapter）、`SourceRevision`
と model/stub digest で遅延結果を consumer-side に reject する
`ScriptEditorDocument`、`ViewType::ScriptEditor` の workspace
登録、driver-neutral `ViewSpec`/command decode/mount、
diagnostic severity・analyzer status 用の `StatusTone` 語彙を
追加した。実 GPUI 描画・実 OS 受入・実 Luau.Analysis FFI
bridge・CLI verb 配線は本スライスの対象外（lane gate）。

## Context

Issue https://github.com/scarecrow173/synvelis/issues/274 の
headless-first contract slice。消費する入力契約は SV-SCRIPT-01
（`ScriptApiModel`/`LuauStub`/`DefinitionInjection`/analyzer
port）と AUTX-07/RCP-06（workspace `automation_ui` projection +
app `ViewSpec`/decode + theme `StatusTone`）の既存正本。
fail-closed 語彙は ruviform RF-SCRIPT-00 の F-13/F-14/F-18 に
従う。`synvelis-script`/`synvelis-gateway`/`synvelis-cli` は
read-only consumer であり編集していない。

## Implementation

### Changes

- `crates/synvelis-automation/src/analyzer.rs`（新規）:
  `ANALYZER_CONTRACT_VERSION`、`AnalyzerRegistry`
  （`BTreeMap<language, index>` で一意言語所有を強制、
  空 `supported_languages`・重複言語を register 時に拒否）、
  `resolve` が `Unavailable`/`VersionMismatch`/`Unsupported`
  を typed error で返す、`LuauPortAnalyzer`（決定的な
  syntax scanner + 注入 `declare function` 署名に基づく
  unknown-api/wrong-arity 診断と prefix 補完）、
  headless 入口 `analyze_source`/`analyze_with_injection`。
- `crates/synvelis-automation/src/lib.rs`: `analyzer` module
  公開と re-export。
- `crates/synvelis-workspace/src/script_editor.rs`（新規）:
  `ScriptEditorDocument`（source text/language/`SourceRevision`/
  cursor/type definitions/definition pins/pending request/
  diagnostics/completions/API verdict rows/analyzer status/
  view-local selections）、`apply_result` が revision と
  model/stub digest の両方を照合して stale を `StaleResult`
  で拒否、`apply_error` が `Unavailable`/`Unsupported` を
  status 語彙へ写像、`project_script_api` が
  `ScriptApiModel` verdict を行へ投影（Hidden は投影しない）、
  `ScriptEditorViewState`（view-local selections + `show_api`、
  serde tolerant）と `ScriptEditorViewAction`。
- `crates/synvelis-workspace/src/{lib,state,view}.rs` +
  `Cargo.toml`: module 公開、`ViewType::ScriptEditor` 追加、
  `ViewState.script_editor` の tolerant decode、
  `synvelis-script` path 依存追加。
- `crates/synvelis-app/src/script_editor.rs`（新規）+
  `lib.rs`: `SV_SCRIPT_*_TARGET` 定数、
  `ScriptEditorScreenState`、`script_editor_screen_spec`
  （document 無しでは Analyze button を出さない fail-closed、
  status/source/diagnostics/completions/api tables を
  `ruviform_ui::view` kind のみで投影）、
  `decode_script_editor_command`（未知 target/action/不正
  params は `None`）、`mount_script_editor_view`
  （`ViewHost` + `AttachEpochAllocator`）。
- `crates/synvelis-theme/src/status.rs`: `StatusTone` に
  `Unsupported`/`Stale`/`Dirty`/`DiagnosticInformation`/
  `DiagnosticWarning`/`DiagnosticError` と
  `status-unsupported`/`status-stale`/`status-dirty`/
  `status-diagnostic-*` token path を追加。
- テスト: `synvelis-automation/tests/script_analyzer.rs`（12
  件）、`synvelis-workspace/tests/script_editor.rs`（8 件）、
  `synvelis-app/tests/svscript04_script_editor.rs`（8 件）。
  いずれも canonical `registry_snapshot.json` fixture を共有。

### Decisions During Implementation

- Registry は analyzer を `Vec<Box<dyn ScriptAnalyzer>>` +
  言語→index の `BTreeMap` で保持し、同一 analyzer が複数言語を
  advertise する場合も register は analyzer 単位で一度だけ行う。
- contract version 不一致は resolve 時に `VersionMismatch`
  （F-13/F-18 の fail-closed）。diagnostics 深さ要求
  （`Syntax` < `SyntaxAndType`）と completion 要求は
  advertise された capability を超えると `Unsupported`。
- port-level adapter は `DefinitionInjection` の `TypeDefinition`
  テキストから `declare function` 署名だけを読む。Hidden/
  Retired/Disabled/Planned/Unknown/非対応 version の API は
  stub に存在しないため、診断・補完のいずれからも「存在する」
  ことを漏らさない（unknown-api としてのみ観測される）。
- consumer-side stale reject は request 時の
  `SourceRevision` + model/stub digest pin の二重照合。
  digest は `DefinitionInjection` が保持する
  `model_digest`/`stub_digest` を document が pin する。
- `ScriptEditorViewState::default()` は `show_api: true`
  （serde `default_true` と一致させるため `Default` を手実装）。
- API verdict table は `show_api` が true なら行が空でも
  投影する（toggle で hide できる表示状態であり、
  「API 情報が無い」こと自体を表に出す）。

## Related Experiments

### Adopted

- `docs/impl/exp/sv-script-04-script-editor-analyzer-registry-port-level-adapter-viewspec.jsonl`
  — registry 設計・注入定義のみを根拠にする adapter・digest pin
  stale reject・syntax scanner 書き直し・検証結果を記録。

### Rejected

<!-- Record rejected experiment logs and why they were rejected. -->

## Validation

Windows 11 (x86_64)、mise 管理 toolchain で実施。
canonical は `mise run` レーン、cargo 直実行は補助証跡。

- `mise run fmt-check`: Pass
- `mise run check|lint|test synvelis-automation`: Pass
  （test: `script_analyzer` 12 件を含む全 suite green）
- `mise run check|lint|test synvelis-workspace`: Pass
  （test: `script_editor` 8 件を含む全 suite green）
- `mise run check|lint|test synvelis-theme`: Pass
- `mise run check|lint|test synvelis-app`: Pass
  （test: `svscript04_script_editor` 8 件を含む全 suite green）
- dependent 非回帰 `mise run check synvelis-cli`・
  `mise run check synvelis-gateway`: Pass
- `mise run check-app-native`・`mise run lint-app-native`: Pass
- `mise run test-rt`: Pass（rt_ filter 全 suite green、
  harness 非回帰）
- `doc_maintenance.js index apply`（docs/impl/ir・docs/impl/exp・
  docs/test-specs）: index 再生成済み
- `doc_status.js lint`: 本 slice の文書に blocking error なし
  （repo 既存の他文書 error/warning は本 slice 範囲外）

headless/fake driver の Pass であり、実 GPUI・実 OS・実
Luau.Analysis FFI の Pass ではない（evidence/sv274/ §3 参照）。

## Risks

- port-level analyzer は決定的な簡易 scanner であり、実
  Luau.Analysis の型推論ではない。実 FFI bridge 結合時に
  診断の粒度・コード体系が変わり得る（lane gate）。
- `StatusTone::of_label` が裸の `"information"`/`"warning"`/
  `"error"` を diagnostic tone へ写す — 同名 generic label を
  使う既存投影があれば意味が変わる（要 grep 確認済み: 衝突なし）。

## Follow-ups

- 実 GPUI Script Editor widget・IME/2Window/Focus/Undo/DPI/
  pixel 受入 → sv#114 / sv#267（別 Gate）。
- 実 Luau.Analysis Native Bridge 結合・同一バイナリ・3OS
  配布 → ruviform#560/#558/#559 lane。
- `synvelis-cli` への Script Analyzer verb 配線 → 別スライス。
- `ruviform_ui::view::ComponentSpec` へのテキスト編集 kind
  追加が必要になった場合は Ruviform 側 Issue。
