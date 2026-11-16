---
id: "IMPL-034YDj5h1A3CjgiMC9pF80"
type: "impl"
status: "completed"
title: "issue-274-cli-script-analyze"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/274"
  changes:
    added:
      - "crates/synvelis-cli/src/analyze.rs"
      - "crates/synvelis-cli/tests/cli_analyze.rs"
      - "docs/discovery/issue-274-cli-script-analyze.md"
      - "docs/specs/issue-274-cli-script-analyze.md"
      - "docs/designs/issue-274-cli-script-analyze.md"
      - "docs/plans/issue-274-cli-script-analyze.md"
      - "docs/tasks/issue-274-cli-script-analyze.md"
      - "docs/test-specs/issue-274-cli-script-analyze.md"
      - "docs/impl/ir/issue-274-cli-script-analyze.md"
      - "evidence/sv274/issue-274-cli-script-analyze-ledger.md"
    modified:
      - "crates/synvelis-cli/src/args.rs"
      - "crates/synvelis-cli/src/commands.rs"
      - "crates/synvelis-cli/src/lib.rs"
      - "crates/synvelis-cli/src/main.rs"
      - "evidence/sv274/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-274-cli-script-analyze.md"
    - "docs/designs/issue-274-cli-script-analyze.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/issue-262-run-file-real-vm.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/issue-274-cli-script-analyze.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# issue-274-cli-script-analyze

## Summary

`synvelis-cli script analyze --file <path> [--cursor <byte-offset>]`
を新設し、`boot_discovery()` ScriptStack → `LuauStub`/
`DefinitionInjection` → `AnalyzerRegistry{LuauPortAnalyzer}` →
`analyze_with_injection` → `verify_revision` → `CliEnvelope` の
headless Script Analyzer を実配線する。

## Context

`docs/tasks/issue-274-cli-script-analyze.md` / plan /
`docs/designs/issue-274-cli-script-analyze.md` に従う。
spec `docs/specs/issue-274-cli-script-analyze.md` の R1-R9 を
実装。Issue #274（OPEN 継続）の SV274-NR-03 headless 残件。

## Implementation

### Changes

- `crates/synvelis-cli/src/args.rs`: `Command::ScriptAnalyze
  {file, cursor}` 追加、`script` 名詞を `parse_script` に変更
  （`analyze` のみ許可、他 verb・裸 `script` は Usage reject
  で「analyze は存在するが実行面は存在しない」を文面で伝達）、
  `cursor` を `COMMAND_VALUE_FLAGS` に追加、module doc 更新、
  parse unit test 2 件追加/1 件拡張。
- `crates/synvelis-cli/src/analyze.rs`（新規）: `classify_language`
  （`.luau`→luau、`.rhai`→rhai id、`.lua`→Unsupported、その他→
  InvalidInput）、`analyze_command` pipeline（reject_dry_run →
  classify → `script::read_source` → `host::boot_discovery()` →
  `LuauStub::generate` → `DefinitionInjection::from_stub` →
  `AnalyzerRegistry`+`LuauPortAnalyzer` → `analyze_with_injection`
  （`SourceRevision(1)`・`SyntaxAndType`・cursor 時のみ
  completion）→ `verify_revision` → envelope）、
  `analyzer_error` typed 写像、envelope/human 描画。
- `crates/synvelis-cli/src/commands.rs`: `ScriptAnalyze` dispatch、
  `reject_dry_run` を `pub(crate)` 化。
- `crates/synvelis-cli/src/lib.rs`: `pub mod analyze` + module map。
- `crates/synvelis-cli/src/main.rs`: HELP に `script analyze` 行
  追加・「no `script run`」記述を analyze=static analysis の
  区別が分かる形に更新。
- `crates/synvelis-cli/tests/cli_analyze.rs`（新規）: 実バイナリ
  e2e 9 tests（happy path・diagnostics=completed data・
  completion・fail-closed 行列・実行面非回帰・human 分離）。
- `evidence/sv274/issue-274-cli-script-analyze-ledger.md` 新規 +
  `evidence/sv274/README.md` NR-03 行のみ更新。
- docs チェーン: discovery（resolved）→ spec（approved）→
  design（approved）→ plan（approved）→ task（in-progress→
  completed 相当）→ test-spec（approved）→ 本 ir。docs index
  は未同期（Coordinator が統合時に再生成）。

### Decisions During Implementation

- verb 命名は Spec 確定済み `script analyze --file`（design で
  再検討しない）。
- `SourceRevision(1)` 固定値 — 単発同期 invocation に stale
  domain は無い。
- `AnalyzerRegistry` は invocation ごと新規 + `LuauPortAnalyzer`
  単独登録（deterministic・永続状態なし、NR-07 への開放を
  登録追加だけで済ませる形）。
- `script::classify` の kind 軸は analyze に流用せず、module
  内の小さな `classify_language` を新設（実装時確定 — design
  どおり）。
- diagnostics は result データ — Error severity でも
  `completed`+exit 0。
- `AnalyzerError` は `#[non_exhaustive]` のため写像は wildcard
  arm を含む（`VersionMismatch`/`StaleResult`/`Internal`/未知
  variant → `cli.internal`）。
- human 描画は analyzer の 0-based position を 1-based
  `line:col` に変換（JSON は契約どおり 0-based のまま）。
- e2e の wrong-arity case は completion `detail` の
  `declare function` signature から arity を導出する 2 段構成
  （live registry の関数名/arity を fixture に固定しない）。

## Related Experiments

### Adopted

なし（代替実装の実験は行っていない — verb 命名は Spec で
確定済み、既存 seam のみ使用）。

### Rejected

なし。

## Validation

Windows 11 (x86_64)、mise 管理 toolchain。canonical = mise:

| コマンド | exit | 結果 |
|---|---|---|
| `mise run fmt-check` | 0 | Pass |
| `mise run check-cli` | 0 | Pass |
| `mise run lint-cli` | 0 | Pass（`-D warnings`、warning 0） |
| `mise run test-cli` | 0 | Pass — args unit + `cli_analyze` 9 + `cli_e2e` 26 全 Pass |
| `mise run build-cli` | 0 | Pass |
| `mise run check synvelis-automation` | 0 | Pass（consume 非回帰） |
| `mise run check synvelis-script` | 0 | Pass（consume 非回帰） |
| `mise run check-headless-deps` | 0 | Pass（12 headless crates に GUI deps なし） |
| `mise exec -- prek run --all-files` | 0 | Pass |
| `git diff --check` | 0 | Pass |

NotRun（正直記録 — 外部 gate 所有）: macOS/Linux build・3OS
実機・製品 E2E（synvelis#114/#267）、実 Luau.Analysis FFI
（ruviform#560・rf#692）、GPUI editor/IME/DPI（NR-01）、
mount（NR-04）、DTCG token（NR-05）、gpui-test（NR-06）、
Rhai/Yaml analyzer（NR-07）、`*-full`/`*-msrv`/`gw10`/`test-rt`/
`check-app-*` lane（Daily Full CI・GUI lane 別 Issue 委譲）。

## Risks

- 実 Luau.Analysis FFI・GPUI editor・Rhai analyzer・mount・
  token は外部 gate（ruviform#560・rf#692・synvelis#114/#267・
  NR-07/04/05/06）— 本 slice は port-level fallback のみ。
- `boot_discovery()` は engine stack 全構築で `action list` と
  同コスト — 低頻度 verb で許容。

## Follow-ups

- docs index は Coordinator が統合時に再生成。
- Issue #274 は OPEN 継続 — AC checkbox 更新なし。
