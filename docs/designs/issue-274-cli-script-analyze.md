---
id: "DESIGN-034YDh5yvBwPjPPKMUXsgY"
type: "design"
status: "approved"
title: "issue-274-cli-script-analyze — script analyze --file の設計"
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
  implements:
    - "docs/specs/issue-274-cli-script-analyze.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-274-cli-script-analyze.md"
    - "docs/impl/ir/sv-script-04-script-editor.md"
    - "docs/impl/ir/issue-262-run-file-real-vm.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/autx-03-headless-cli.md"
    - "docs/automation/design-script.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-274-cli-script-analyze.md"
  derived-by:
    - "docs/plans/issue-274-cli-script-analyze.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-274-cli-script-analyze — script analyze --file の設計

## Context

`synvelis-cli` は headless thin adapter（AUTX-03）。analyzer 側の
公開契約（`AnalyzerRegistry`/`LuauPortAnalyzer`/
`analyze_with_injection`/`verify_revision`、
`DefinitionInjection`/`LuauStub`/`ScriptSource` 系）は sv#274
headless slice・sv#271/sv#273 で main 着地済み。`boot_discovery()`
は `ScriptStack`（snapshot→`surface_for_snapshot`→
`ScriptApiModel::build`→`BindingSet::generate`）を project/durable
files 無しで capture 済み。本 slice はこの既存 seam の上に
CLI verb を 1 本足すだけで、新しい authority・registry・VM を
作らない。

## Scope

### In Scope

- `args.rs`: `Command::ScriptAnalyze { file, cursor }`、
  `script` 名詞の `analyze` のみ許可する `parse_script`、
  `cursor` を `COMMAND_VALUE_FLAGS` に追加。
- `commands.rs`: `ScriptAnalyze` dispatch、`reject_dry_run` の
  crate 公開。
- `analyze.rs`（新規 module）: 分類・pipeline・envelope 写像・
  human 描画。
- `lib.rs`/`main.rs`: module export・HELP 文更新。
- `tests/cli_analyze.rs`（新規）: 実バイナリ e2e。
- docs チェーン・`evidence/sv274/` ledger。

### Out of Scope

Spec の Out of Scope と同一（実行面・他 crate・実機 lane・
docs index）。

## Components and Boundaries

- **args.rs（parse 層）**: `script` positionals の第 2 要素が
  `analyze` のときのみ `Command::ScriptAnalyze` を生成。
  `script run`/`script <other>`/裸 `script` は Usage reject
  （「analyze は存在するが実行面は存在しない」ことを文面で伝える）。
  `--cursor` は `usize`（byte offset）として parse — 非数値は
  Usage。`--file`/`--cursor` 以外の flag は `take_flags` で reject。
- **analyze.rs（pipeline 層）**: `script::read_source`（既存
  helper — canonicalize・is_file・UTF-8 検証）を再利用し、
  `script::classify` は流用しない（kind 軸を持つため）。
  analyze 専用の小さな `classify_language(file) ->
  Result<ScriptLanguageId, CliError>` を module 内に置く。
- **host.rs**: 変更なし — `boot_discovery()` の `Booted.script`
  （`ScriptStack`）を consume するのみ。
- **error/envelope/exitcode**: 既存語彙をそのまま使う。
  `AnalyzerError`→`CliError` の typed 写像は analyze.rs 内の
  1 関数に集約（`Unavailable`/`Unsupported`→`CliError::
  Unsupported`、`InvalidRequest`→`CliError::InvalidInput`、
  `VersionMismatch`/`StaleResult`/`Internal`/その他→
  `CliError::Internal`）。

## Data and Control Flow

```
script analyze --file <path> [--cursor <n>]
  → args::parse → Command::ScriptAnalyze{file, cursor}
  → commands::run → analyze::analyze_command
    1. reject_dry_run("script analyze")        → Usage(2)
    2. classify_language(file)                → ScriptLanguageId
       .luau→luau() / .rhai→"rhai" / .lua→Unsupported(5)
       .yaml|.yml|other|none→InvalidInput(2)
    3. script::read_source(file)               → UTF-8 text or InvalidInput(2)
    4. host::boot_discovery()                  → Booted (ScriptStack)
    5. LuauStub::generate(&booted.script.model)→ Internal(10) on err
    6. DefinitionInjection::from_stub(&stub)
    7. AnalyzerRegistry::new() + register(LuauPortAnalyzer)
                                                → Internal(10) on err
    8. analyze_with_injection(registry,
         ScriptSource{text, language, SourceRevision(1)},
         &injection,
         WantedCapabilities{diagnostics: Some(SyntaxAndType),
                            completion: cursor.is_some()},
         cursor.map(|b| Cursor{byte_offset: b}))
                                                → analyzer_failure (typed)
    9. verify_revision(&result, SourceRevision(1)) → Internal(10)
   10. envelope: completed/0 + result payload / human lines
```

`SourceRevision(1)` は単発同期 invocation の固定値 — CLI は
編集 session を持たず stale domain が存在しない。`verify_revision`
は echo 確認（analyzer が要求 revision をそのまま返すことの
契約検査）であり、遅延応答 reject は consumer 側の責務として
sv#274 headless slice で検証済み。

`AnalyzerRegistry` は invocation ごとに新規構築 + `LuauPortAnalyzer`
単独登録（deterministic・永続状態なし）。将来の言語 analyzer 追加は
登録追加のみで済む形（NR-07 への開放、YAGNI で抽象化を増やさない）。

## Envelope / human 出力

- `result` JSON（machine mode）:
  `file`（引数 path の display）、`language`、`source_revision`、
  `analyzer`{analyzer_id, language, impl_version,
  contract_version}（`registry.descriptor_for` が Some のとき）、
  `injection`{package_name, model_digest, stub_digest}、
  `diagnostics[]`{severity, kind, code, message,
  range{start,end}, source_revision}、`completions`（cursor 指定
  時のみ — label/kind/detail?/insert_text?/replace_range/
  source_revision）。severity/kind/completion-kind は lowercase
  文字列写像（non_exhaustive の未知 variant は "other"）。
- human mode: 診断 1 件 = `{file}:{line+1}:{col+1} {severity}
  {code} {message}`（analyzer の 0-based position を 1-based 表示
  に変換 — コンパイラ診断の慣例。JSON は契約どおり 0-based の
  まま）。completion 1 件 = `{file}:{line+1}:{col+1} completion
  {label}`（detail あれば ` — {detail}`）。diagnostics 空なら
  `{file}: no diagnostics` を stdout へ。invocation 失敗は
  `CliError` 経路の stderr 規則を維持。
- diagnostics は result データ — Error severity があっても
  `completed`+exit 0（lint 的な非ゼロ終了は contract 外）。

## Non-Functional Constraints

- headless 維持: `check-headless-deps` が GUI/GPU/WebView crate
  非含有を静的検査 — `analyze.rs` は synvelis-automation/script/
  gateway の既存 import のみ使用、新規依存なし。
- AUTX-SCR-ANA-01: `host::handle`/`caller_for`/`claim_for`/
  wire request を呼ばない（code review + `--project` 無し temp
  dir 外 e2e で pin）。
- Hidden/Retired 非漏洩: injection は live `ScriptApiModel` 由来
  `LuauStub` のみ（手書き surface/fixture コピー禁止 — sv#262 と
  同じ canonical 経路）。

## Risks and Trade-offs

| リスク | 対応 |
| --- | --- |
| `boot_discovery()` が重い（engine stack 全構築） | `action list` と同コスト。analyze は低頻度 verb で許容。script stack capture は既に boot の一部 |
| `.rhai` で stub 生成まで走る無駄 | registry resolve が `Unavailable` で fail-closed するため結果は正直。pipeline を言語で分岐させると経路が 2 本になり複雑化 — 単一経路を優先 |
| `--cursor` が UTF-8 boundary 外 | analyzer `InvalidRequest` → `InvalidInput`(2) に委譲。CLI 側で事前検証しない（単一権威） |
| 大きな source の parse コスト | `LuauPortAnalyzer` は決定的な線形 scan。`max_source_bytes=0`（無制限）— 本 slice では制限を設けない（contract 既定に従う） |

## Deferred Design Concerns

- 実 Luau.Analysis FFI bridge — ruviform#560・rf#692 lane。
  `ScriptAnalyzer` port 契約はそのまま使える。
- Rhai/Yaml analyzer（NR-07）— registry 登録追加で開放。
- 非同期/stale-revision 実運用 — 単発同期のため対象外。

## Verification Notes

- [x] `mise run fmt-check` / `check-cli` / `lint-cli` /
  `test-cli`（real-binary e2e 含む）/ `build-cli`
- [x] `mise run check synvelis-automation` / `check
  synvelis-script`（consume 側非回帰）
- [x] `mise run check-headless-deps`
- [x] `mise exec -- prek run --all-files`
- [ ] macOS/Linux build・3OS 実機・製品 E2E（#267）— NotRun
