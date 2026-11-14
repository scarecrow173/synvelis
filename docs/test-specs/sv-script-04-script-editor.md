---
id: "TSPEC-034XnRBxxGiEcCQWGPb3oW"
type: "test-spec"
status: "approved"
title: "sv-script-04-script-editor"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/sv-script-04-script-editor.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/266"
    - "https://github.com/scarecrow173/synvelis/issues/267"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/267"
  deferred-by: []
---

# sv-script-04-script-editor

## Purpose

SV-SCRIPT-04（Issue #274）の headless-first スライスが守るべき
契約を固定する: Analyzer Registry が `LanguageId`/`Version`/
`Capabilities` で analyzer を fail-closed に解決し、Luau
port-level adapter が `DefinitionInjection` 経由の型定義だけを
根拠に構文/型診断・補完を返し、editor document model が
`SourceRevision` と model/stub digest で遅延非同期結果を
consumer-side に reject し、driver-neutral `ViewSpec` が
GPUI/OS/Native 型を露出せずに editor 状態を投影すること。
実 GPUI/実 OS/実 Luau.Analysis FFI の受入は本 spec の対象外
であり、fake/headless の PASS を実機 PASS と読み替えない。

## Feature

Script Editor の Analyzer 統合と driver-neutral 投影:
`AnalyzerRegistry` が登録済み `ScriptAnalyzer` を言語・
contract version・要求 capability で解決し、未登録言語は
`Unavailable`・未 advertise capability は `Unsupported`・
version 不一致は `VersionMismatch` の typed error で拒否する。
`LuauPortAnalyzer` が注入された `TypeDefinition` から
`declare function` 署名を読み、unknown API・arity 不一致の
型診断と prefix 補完を `source_revision` 付きで返す。
`ScriptEditorDocument` が request 時 revision で pending を
key 化し、遅延到着・digest drift の結果を stale として破棄する。
`script_editor_screen_spec` が document/diagnostics/
completions/analyzer status を既存 `ComponentSpec` kind へ
投影し、非 executable な action に button を生成しない。

## Rules

- Rule: registry 解決は唯一の権威であり、analyzer 未登録言語は
  `AnalyzerError::Unavailable`、contract version 不一致は
  `VersionMismatch`、要求 capability が advertise 範囲外なら
  `Unsupported` を返す — silent skip・成功扱い・fallback はない。
- Rule: Luau adapter の診断・補完は `request.type_definitions`
  （`DefinitionInjection` 由来）だけを API 知識の根拠とし、
  stub に現れない名前（Hidden/Retired/非 Invocable）は
  unknown-api 診断か補完欠落として扱われ、存在を漏洩しない。
- Rule: 全 `Diagnostic`/`Completion` は request の
  `source.revision` を `source_revision` に持ち、consumer は
  `verify_result_revision` 相当で現行 revision と照合して
  からのみ受理する — 遅延結果は `StaleResult` で破棄する。
- Rule: `ScriptApiModel`/`LuauStub` の model/stub digest が
  request 時点と現行で drift した場合、その revision の結果は
  stale として受理しない。
- Rule: analyzer の成否は実行権限・grant・project truth を
  生成しない。editor state は view-local であり、第二の
  capability/Gateway/Project 正本を持たない。
- Rule: `ViewSpec` は `ruviform_ui::view` の型のみで構成され、
  GPUI/OS/Native FFI/VM 型を公開シグネチャに露出しない。
  非 executable な action には button を生成しない
  fail-closed spec とし、理由は表示語彙で残す。
- Rule: intent decode は `SV_SCRIPT_TARGET` の単一経路で、
  未知 action・未知 target・型不一致 params は `None` で
  fail-closed に落とす。
- Rule: theme の新規 tone は kebab-case 安定 path を持ち、
  意味の異なる状態（Unavailable/Unsupported/stale/dirty/
  severity）に `ok`/`warn`/`err` を流用しない。

## Examples

- Example: Luau analyzer 登録済み registry で
  `synvelisNope(...)` 呼出しは `unknown-api` 診断、
  `synvelisPlay(project)` の arity 不一致は `wrong-arity`
  診断、`"synvelis"` prefix の cursor は公開 API 名のみの
  completion を返す（Hidden/Retired/版違いは含まれない）。
- Example: request revision 5 の結果が document revision 6
  到着後に `apply_result` されると stale として reject され、
  status が stale/dirty を表示する。
- Example: Rhai/YAML 等 analyzer 未登録言語への `analyze`
  要求は `Unavailable`、advertise されていない
  `SyntaxAndType` 要求は `Unsupported` の typed verdict に
  なり、analyzer status 表示が runtime 起動可否と区別される。
- Example: model/stub digest が差し替わった後に旧 injection で
  計算された結果は stale reject され、新 digest の request
  のみ受理される。
- Example: `script_editor_screen_spec` は空状態で Analyze
  button を出さず、document 読込後は diagnostics/completions
  table・analyzer status label・Analyze button を出し、
  table select が `SV_SCRIPT_TARGET` の view-local intent に
  decode される。

## Guarantees

- registry/analyzer/document/ViewSpec の全失敗経路が typed
  error または fail-closed `None`/verdict で観測可能であり、
  panic・silent 成功・部分投影はない。
- headless fixture（fake driver + port-level analyzer）で
  diagnostics/completion/Unknown API verdict/stale reject が
  ViewSpec 投影として観測できる。
- `synvelis-cli`/`synvelis-gateway`/`synvelis-script` の公開
  契約に変更を加えず、依存方向は既存 DAG を維持する
  （workspace→script の path 依存追加のみ、cycle なし）。

## Non-goals

- 実 GPUI 描画・実テキスト編集 widget・IME/2Window/Focus/
  Undo/DPI/pixel — sv#114/#267 の実 OS Gate。
- 実 Luau.Analysis FFI bridge・mlua 同一バイナリ・3OS 配布 —
  ruviform#560/#558/#559 の lane gate。
- `synvelis-cli` への Script Analyzer verb 配線 — 別 lease。
- Rhai 高機能 analyzer・外部 luau-lsp 常駐・Lua 5.4・
  全面 IDE/Debugger — Issue #274 の対象外と同じ。

## Risk

- port-level adapter は deterministic fixture であり、実
  Luau.Analysis の診断品質・網羅性を保証しない。契約層の
  fail-closed 性だけが本 spec の保証範囲。
- `ComponentSpec` に editor kind が無いため source 本文の
  リッチ投影は行わず、メタ/一覧投影に留まる。実編集 UX の
  受入は後続 Gate に依存する。
