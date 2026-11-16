---
id: "PLAN-034YDhXmRCMo80uy2OMsrP"
type: "plan"
status: "approved"
title: "issue-274-cli-script-analyze — 実装 plan"
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
    - "docs/designs/issue-274-cli-script-analyze.md"
  implemented-by: []
  depends-on:
    - "docs/designs/issue-274-cli-script-analyze.md"
    - "docs/specs/issue-274-cli-script-analyze.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-274-cli-script-analyze.md"
  derived-by:
    - "docs/tasks/issue-274-cli-script-analyze.md"
    - "docs/test-specs/issue-274-cli-script-analyze.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-274-cli-script-analyze — plan

## file map

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-cli/src/args.rs` | `Command::ScriptAnalyze{file,cursor}` 追加、`script`→`parse_script`（`analyze` のみ許可）、`cursor` flag 追加、module doc 更新、parse unit test |
| `crates/synvelis-cli/src/commands.rs` | `ScriptAnalyze` dispatch → `analyze::analyze_command`、`reject_dry_run` を `pub(crate)` 化 |
| `crates/synvelis-cli/src/analyze.rs` | 新規 — `classify_language`・pipeline・`AnalyzerError`→`CliError` 写像・envelope/human 描画 |
| `crates/synvelis-cli/src/lib.rs` | `pub mod analyze` + module map |
| `crates/synvelis-cli/src/main.rs` | HELP に `script analyze` 行追加・「no script run」記述更新 |
| `crates/synvelis-cli/tests/cli_analyze.rs` | 新規 — 実バイナリ e2e（happy/fail-closed/human 分離） |
| docs/`evidence` | 本 slug doc チェーン + `evidence/sv274/` ledger + README NR-03 行 |

`host.rs`/`script.rs`/`error.rs`/`exitcode.rs`/`envelope.rs`/
`Cargo.toml`/`Cargo.lock` は原則無変更（`read_source` 再利用、
既存語彙のみ使用、新規依存なし）。

## dependency graph

```
args.rs parse ──→ commands.rs dispatch ──→ analyze.rs
                                              ├─ script::read_source (既存)
                                              ├─ host::boot_discovery (既存)
                                              ├─ LuauStub/DefinitionInjection (script)
                                              └─ AnalyzerRegistry/LuauPortAnalyzer/
                                                 analyze_with_injection/verify_revision
                                                 (automation) → CliEnvelope
```

## risk register

| リスク | 対応 |
| --- | --- |
| `script` 名詞の他 verb reject が緩む | `parse_script` は `analyze` 以外を全て Usage reject — e2e + parse test で pin |
| `.rhai` で registry `Unavailable` が CLI で握り潰される | `analyzer_failure` 写像を 1 関数に集約、language/capability を message に保持 |
| `verify_revision` が内部で別 revision を返す異常 | `StaleResult`→`Internal`(10) — contract drift はユーザー入力の責ではない |
| `--cursor` 非数値・境界外 | 非数値は parse 時 Usage(2)、境界外は analyzer `InvalidRequest`→`InvalidInput`(2) |
| docs index 未同期 | 意図的 — Coordinator が統合時に再生成（PR 本文に明記） |

## verification matrix

| 検証 | コマンド |
| --- | --- |
| fmt | `mise run fmt-check` |
| cli check/lint/test/build | `mise run check-cli` / `lint-cli` / `test-cli` / `build-cli` |
| consume 側非回帰 | `mise run check synvelis-automation` / `check synvelis-script` |
| headless deps | `mise run check-headless-deps` |
| hooks | `mise exec -- prek run --all-files` |

NotRun（正直記録）: macOS/Linux build・3OS 実機（#267）、実
Luau.Analysis FFI（ruviform#560・rf#692）、GPUI editor/IME/DPI
（NR-01・#114/#267）、mount（NR-04）、DTCG token（NR-05）、
gpui-test（NR-06）、Rhai analyzer（NR-07）、`*-full`/`*-msrv`/
`gw10`/`test-rt`/`check-app-*` lane。

## rollback

単一 PR revert で `script analyze` verb が消えるだけ — 既存 verb・
分類行列・envelope 契約は不変。永続副作用なし（discovery boot は
in-memory seam のみ）。
