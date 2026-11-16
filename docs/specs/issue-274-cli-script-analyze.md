---
id: "SPEC-034YDgnDTjpDlwq3cW0ik4"
type: "spec"
status: "approved"
title: "issue-274-cli-script-analyze — synvelis-cli script analyze --file headless Script Analyzer"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/274"
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
    - "docs/impl/ir/sv-script-04-script-editor.md"
    - "docs/impl/ir/issue-262-run-file-real-vm.md"
    - "docs/specs/autx-03-headless-cli.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/automation/design-script.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-274-cli-script-analyze.md"
  derived-by:
    - "docs/designs/issue-274-cli-script-analyze.md"
    - "docs/plans/issue-274-cli-script-analyze.md"
    - "docs/test-specs/issue-274-cli-script-analyze.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-274-cli-script-analyze.md"
  references: []
  defers: []
  deferred-by: []
---

# issue-274-cli-script-analyze — synvelis-cli script analyze --file headless Script Analyzer

## What Should Be Built

`synvelis-cli script analyze --file <path> [--cursor <byte-offset>]`
を新設し、headless Script Analyzer を実配線する。パイプラインは

`extension→language 分類 → script::read_source →
host::boot_discovery() ScriptStack → LuauStub::generate →
DefinitionInjection::from_stub → AnalyzerRegistry{LuauPortAnalyzer} →
analyze_with_injection → verify_revision → CliEnvelope`

で、GUI・project・Gateway wire request・claims を一切必要としない
（AUTX-SCR-ANA-01 — analyzer 成功は実行権限・Grant・project truth
を生まない）。Issue #274 の headless 残件 `SV274-NR-03` を充足する
slice であり、Issue 全体は OPEN 継続（実 GPUI editor・実 OS・実
FFI bridge・mount・token・Rhai analyzer は別 gate 所有）。

## Why Now

sv#274 headless slice（PR 済）で `AnalyzerRegistry`/
`LuauPortAnalyzer`/`analyze_with_injection`/`verify_revision` と
`boot_discovery()` の `ScriptStack` capture が main に着地した。
残るは CLI への verb 配線のみで、依存は全て merge 済み・新規依存
追加は不要。Issue #274 の「Headless CLIのScript AnalyzerはGUI不要で
起動可能」（SV274-NR-03）の唯一の残件。

## Users and Value

- ユーザー/自動化: GUI 無し環境（CI・ヘッドレスシェル）で script
  source の構文・型・Unknown API 診断と生成 API 補完を取得できる。
- 製品: 実 GPUI editor（#114/#267 gate）と同一の analyzer 契約・
  同一の live registry 由来 stub を共有し、headless/実機で診断が
  一致する基盤を提供する。

## Scope

### In Scope

- `script analyze --file <path> [--cursor <byte-offset>]` verb の
  parse・dispatch・pipeline 実装（`crates/synvelis-cli/` のみ）。
- extension→language 分類（`.luau`/`.rhai` は analyzer へ、
  `.lua` は Unsupported、`.yaml`/未知拡張子/拡張子無しは
  InvalidInput）。
- `CliEnvelope` への typed 写像（completed/failed + 決定的 exit
  code）と `--format human` の診断行出力。
- `script run`・`script` の他 verb・裸 `script`・`--script` flag の
  fail-closed reject 非回帰（AUTX-CLI-FILE-01）。
- Windows headless 実バイナリ E2E（`tests/cli_analyze.rs` 新規）。
- `evidence/sv274/` ledger 追加と `README.md` の NR-03 行更新、
  slug `issue-274-cli-script-analyze` の doc チェーン。

### Out of Scope

- `script run`/`--script` 実行面・`action analyze`/`workflow
  analyze` 別名・`--analyze` flag（verb 命名は本 Spec で確定済み）。
- Gateway wire request・`host::handle`/claims/grants/proposals/
  approvals・project truth への接続。
- `.rhai`/`.yaml` DSL 向け analyzer 実装（NR-07 — typed
  `Unavailable`/`InvalidInput` の fail-closed のみ）。
- 実 Luau.Analysis Native Bridge 結合・FFI・外部 luau-lsp
  （ruviform#560・rf#692 lane — `LuauPortAnalyzer` はそのまま置換
  可能な port-level fallback）。
- 実 GPUI Script Editor・IME・2Window・Focus・Undo・DPI・pixel・
  実 OS 受入（synvelis#114/#267 — NR-01）、composition root mount
  （NR-04）、DTCG token（NR-05）、`gpui-test` scenario（NR-06）。
- 非同期結果の stale-revision 実運用（CLI は単発同期 —
  `verify_revision` echo 確認のみ）。
- diagnostics を invocation 失敗として扱う exit 規則の変更
  （completed+0 で diagnostics を返す契約）。
- 新規依存追加・他 crate の変更・docs index（`docs/*/README.md`）
  の編集/再生成。
- Issue #274 の close・AC checkbox 更新。

## Requirements

- R1: `script analyze --file <path>` を新設。`--file` 必須
  （不在→Usage exit 2）、`--cursor <byte-offset>` は指定時のみ
  completion を要求。`--dry-run` は Usage reject。`--project` は
  不要・要求しない（discovery boot — project/durable files を
  開かない・作らない）。
- R2: pipeline は `boot_discovery()` の `ScriptStack` →
  `LuauStub::generate(&booted.script.model)` →
  `DefinitionInjection::from_stub` → `AnalyzerRegistry::new()` +
  `LuauPortAnalyzer` 登録（失敗は composition invariant →
  `CliError::Internal`）→ `analyze_with_injection`
  （`ScriptSource{text, language, revision: SourceRevision(1)}`、
  `wanted: {diagnostics: Some(SyntaxAndType), completion:
  cursor.is_some()}`）→ `verify_revision` → envelope。
  `SourceRevision(1)` は単発同期 invocation の固定値。
- R3: Gateway wire request・`host::handle`/`caller_for`/claims は
  発行しない（AUTX-SCR-ANA-01）。boot は ScriptStack capture の
  ためだけに使う。
- R4: extension 分類は analyze 専用の小さな分類（`script::classify`
  の kind 軸は流用しない）。`.luau`→`ScriptLanguageId::luau()`、
  `.rhai`→`ScriptLanguageId("rhai")`（registry が
  `AnalyzerError::Unavailable` で fail-closed）、`.lua`→
  `CliError::Unsupported`、`.yaml`/`.yml`/未知拡張子/拡張子無し→
  `CliError::InvalidInput`。
- R5: 解析成功は diagnostics を含め `completed`+exit 0
  （Error severity の存在は invocation の失敗ではない）。
  `AnalyzerError` の typed 写像: `Unavailable`/
  `Unsupported{capability}`→`CliError::Unsupported`（exit 5、
  language・capability を保持）、`InvalidRequest`→
  `CliError::InvalidInput`（exit 2）、`VersionMismatch`/
  `StaleResult`/`Internal`→`CliError::Internal`（exit 10）。
- R6: `result` payload は analyzer descriptor（analyzer_id・
  language・impl_version・contract_version）・`source_revision`・
  `diagnostics[]`（severity/kind/code/message/range/
  source_revision）・`completions`（cursor 指定時のみ）・
  injection 識別（package_name・model_digest・stub_digest）を
  正直に含める。`--format human` は `file:line:col severity code
  message` 系の行を stdout（result データ）へ、invocation 失敗は
  stderr 従来規則を維持。
- R7: injection は boot 時の live `ScriptApiModel` 由来 `LuauStub`
  のみ（手書き surface/fixture コピー禁止）。Hidden/Retired/
  Disabled/非対応 version の API が補完候補・診断対象に「存在」
  として出ないことを e2e で確認（stub の `declare function` に
  出ない名前を呼ぶ source → unknown-api が返る）。
- R8: 実バイナリ E2E で happy path・fail-closed 行列・
  `--format human` の stdout/stderr 分離を pin する。
- R9: `evidence/sv274/` に本 slice slug の ledger を追加し、
  AC→Case→test→結果と NotRun 表を POLICIES.md 語彙で記録。
  `README.md` は NR-03 行のみ更新。Issue #274 は OPEN 継続・
  PR は `Refs #274`。

## Acceptance Criteria

- [ ] `script analyze --file <clean .luau>` が `--project` 無し・
  temp dir 外で `completed`+exit 0、diagnostics 空を返す。
- [ ] unknown API 呼出を含む `.luau` が `completed`+exit 0 で
  typed `unknown-api` diagnostic（severity/kind/code/message/
  range/source_revision）を返す。
- [ ] `--cursor <offset>` 指定で completions が非空、live
  registry 由来の生成関数名（`synvelis*`）を含み、Hidden/
  Retired/Disabled/非対応 version 名を含まない。
- [ ] `.rhai`→exit 5 typed `cli.unsupported`（language 保持）、
  `.lua`→exit 5、`.yaml`/未知拡張子/拡張子無し/非 UTF-8/不存在
  file→exit 2 typed。
- [ ] `script run`/`script`/`script <other>`/`--script`→exit 2
  `cli.usage` 非回帰。`--dry-run`→exit 2、`--file` 不在→exit 2。
- [ ] `--format human` で診断行が stdout、invocation 失敗が
  stderr に出る（stdout/stderr 分離）。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`build-cli`
  （synvelis-cli）・`check synvelis-automation`/`check
  synvelis-script`・`check-headless-deps`・`prek run
  --all-files` が Pass。未実行項目は NotRun/Deferred として
  所有 gate を明記する。

## Deferred Design Concerns

- 実 Luau.Analysis Native Bridge 結合・mlua 同一バイナリ・3OS
  配布 — ruviform#560・rf#692 lane gate。`LuauPortAnalyzer` の
  port 契約で置換可能。
- Rhai/Yaml analyzer 実装（NR-07）— `AnalyzerRegistry` 登録の
  追加で開放される設計。
- 実 GPUI editor/IME/2Window/Focus/Undo/DPI/pixel・実 OS 受入
  （NR-01 — synvelis#114/#267）、composition root mount（NR-04）、
  DTCG token（NR-05）、`gpui-test` scenario（NR-06）。
- macOS/Linux build・3OS 実機・`*-full`/`*-msrv`/`gw10`/`test-rt`/
  `check-app-*` lane — Daily Full CI・GUI lane 別 Issue 委譲。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/274
  （OPEN 継続 — 本 slice は `SV274-NR-03` のみ。AC checkbox は
  更新しない）
- 契約正本: `docs/automation/design-script.md` §4/§5
  （AUTX-SCR-ANA-01・AUTX-CLI-FILE-01）、`evidence/sv274/README.md`
  §3 SV274-NR-03
- consume 契約: `crates/synvelis-automation/src/analyzer.rs`、
  `crates/synvelis-script/src/analyzer.rs`/`luau.rs`、
  `crates/synvelis-cli/src/host.rs`（`ScriptStack`）
- 上流: `docs/discovery/issue-274-cli-script-analyze.md`
