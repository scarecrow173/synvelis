---
id: "DISC-034YDfGylRNr58hWDe5lyH"
type: "discovery"
status: "resolved"
title: "issue-274-cli-script-analyze — script analyze --file headless Script Analyzer 配線の探索結果"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/sv-script-04-script-editor.md"
    - "docs/impl/ir/issue-262-run-file-real-vm.md"
    - "docs/automation/design-script.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-274-cli-script-analyze.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-274-cli-script-analyze — script analyze --file headless Script Analyzer 配線の探索結果

## Summary

synvelis#274 の headless 残件 `SV274-NR-03`（synvelis-cli への
Script Analyzer verb 配線）を、既存の公開契約だけで実装できるかを
探索した。`AnalyzerRegistry`/`LuauPortAnalyzer`/
`analyze_with_injection`/`verify_revision`（synvelis-automation）、
`DefinitionInjection`/`LuauStub`/`ScriptSource` 等
（synvelis-script）、`host::boot_discovery()` の `ScriptStack`
（synvelis-cli）は全て main（`b5a8919`）に merge 済みで、新規依存
なしに配線可能と確認した。verb 命名は `script analyze --file
<path> [--cursor <byte-offset>]` に確定。

## Exploration Goal

`synvelis-cli` に headless Script Analyzer の製品入口を追加し、
`boot_discovery()` の ScriptStack → `LuauStub::generate` →
`DefinitionInjection::from_stub` → `AnalyzerRegistry{LuauPortAnalyzer}`
→ `analyze_with_injection` → `verify_revision` → `CliEnvelope` の
実配線を行う。AUTX-CLI-FILE-01（`script run`/`--script` の独立実行面
禁止）と AUTX-SCR-ANA-01（analyzer 成功は実行権限を生まない）を
両立させる verb 形状の確定が本探索の焦点。

## Key Issues and Assumptions

- **verb 命名**: `action analyze`/`workflow analyze` は解析対象が
  artifact kind ではなく言語・ソース単位であるため誤 scope。
  `--analyze` flag は `run` の project-bound 実行経路と project-less
  検査を混線させる。`script analyze` は `--file` 軸に整合し、
  「実行面は存在しない」reject 規則を `analyze` のみ許可に変える
  ことで help/error 文面でも明示できる → 確定（confirmed）。
- **`.rhai` の扱い**: analyzer registry は luau のみ登録。
  `ScriptLanguageId("rhai")` をそのまま `analyze_with_injection` に
  渡せば `AnalyzerError::Unavailable` → `CliError::Unsupported`
  （exit 5）の typed fail-closed になる。CLI 層で言語別の事前検査を挟ま
  ない → confirmed。
- **`SourceRevision`**: 単発同期 invocation に stale domain は無い。
  固定値 `SourceRevision(1)` + `verify_revision` echo 確認で十分
  → confirmed（design で記録）。
- **`--cursor`**: byte offset をそのまま `Cursor{byte_offset}` に渡す。
  境界外・非 char boundary は analyzer の `InvalidRequest` →
  `CliError::InvalidInput`（exit 2）に委ねる → confirmed。
- **`--project`/`--dry-run`**: analyze は discovery boot
  （project/durable files を開かない）のため `--project` は不要・
  非要求。`--dry-run` は `action list` 系と同じ Usage reject
  （exit 2）→ confirmed。
- **envelope**: diagnostics は result データであり invocation 失敗
  ではない → Error severity があっても `completed`+exit 0。
  `AnalyzerError` は typed 語彙で `CliError` へ写像
  （Unavailable/Unsupported→5、InvalidRequest→2、
  VersionMismatch/StaleResult/Internal→10）→ confirmed。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| `script analyze --file`（採用） | `--file` 軸に整合、名詞 `script` が言語単位の検査対象を正直に表す、既存 reject 分岐の自然な拡張 | `script` 名詞に初めて verb を許す | yes |
| `action analyze` / `workflow analyze` | 既存名詞の延長 | 解析対象は kind ではなく言語・ソース。`.luau` は両 kind に跨り `--file` 分類規則と矛盾 | no |
| `--analyze` flag on `action run`/`workflow run` | 既存 verb の flag 追加のみ | `run` は project-bound 実行経路。project-less 検査を混ぜると `--file` の「実行する入力」意味が曖昧化。発見性も劣る | no |
| `script::classify` 再利用 | 既存分類器 | `FileClass` は action/workflow の kind 軸を持ち analyze に不適。`script::read_source` のみ再利用 | no |

## Tentative Conclusions and Hypotheses

- confirmed: 入力依存は全て `b5a8919`（origin/main tip）に merge
  済み。`synvelis-cli` 依存木は完備で新規依存追加は不要。
- confirmed: Hidden/Retired/Disabled/非対応 version の API は
  `LuauStub`（invocable entry のみ emit）に出ないため、unknown-api
  診断で「存在しない」ことを e2e pin できる。
- hypothesis: 実 Luau.Analysis FFI bridge（ruviform#560・rf#692
  lane）は `LuauPortAnalyzer` の `ScriptAnalyzer` port 契約で
  そのまま置換可能 — 本 slice では port-level fallback のまま。

## Open Questions

- なし（Coordinator Spec で verb 命名・envelope 形状・fail-closed
  語彙は確定済み。残る設計詳細は design doc で確定する）。

## Promotion Candidates

- [x] spec-doc needed: `docs/specs/issue-274-cli-script-analyze.md`
- [x] adr-doc needed: なし（verb 命名は Spec 確定の設計判断であり
  ADR 級の不可逆決定ではない）

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/274
- 契約正本: `docs/automation/design-script.md` §4/§5
  （AUTX-SCR-ANA-01・AUTX-CLI-FILE-01）
- consume 側: `crates/synvelis-automation/src/analyzer.rs`、
  `crates/synvelis-script/src/analyzer.rs`/`luau.rs`、
  `crates/synvelis-cli/src/host.rs`（`ScriptStack` capture）
- 配線正本（READ 参照）: `crates/synvelis-automation/tests/
  script_analyzer.rs`、`crates/synvelis-cli/tests/cli_e2e.rs`
- 前 slice 証拠: `evidence/sv274/README.md`（SV274-NR-03 行）

## Evidence

- 2026-11-16: `git fetch origin` → `origin/main` = `b5a8919`
  （PR #508 sv262 統合分 index 再生成）。前提契約の merge を
  `crates/synvelis-automation/src/analyzer.rs:39/83/103/139/243/
  261/285/732`、`crates/synvelis-script/src/analyzer.rs:36/97/116/
  123/253/265/376`、`crates/synvelis-script/src/luau.rs:105`、
  `crates/synvelis-cli/src/host.rs:103/111/305/373` で確認。
- 2026-11-16: `crates/synvelis-cli/Cargo.toml` — 依存は
  serde/serde_json + synvelis-{automation,script,script-luau,
  gateway,project} のみ。analyzer 配線に必要な crate は全て既存。
