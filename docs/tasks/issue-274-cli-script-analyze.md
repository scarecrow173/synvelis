---
id: "TASK-034YDhY9pqCERJWLjMaGyX"
type: "task"
status: "completed"
title: "issue-274-cli-script-analyze — task 分解"
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
    - "docs/plans/issue-274-cli-script-analyze.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-274-cli-script-analyze.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-274-cli-script-analyze.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-274-cli-script-analyze.md"
  references: []
  defers: []
  deferred-by: []
---

# issue-274-cli-script-analyze — tasks

| # | task | 状態 |
| --- | --- | --- |
| T1 | `args.rs`: `Command::ScriptAnalyze`・`parse_script`（analyze のみ許可）・`--cursor`・doc/test | done |
| T2 | `analyze.rs` 新規: `classify_language`・pipeline・error 写像・envelope/human | done |
| T3 | `commands.rs`/`lib.rs`/`main.rs`: dispatch・module export・HELP | done |
| T4 | `tests/cli_analyze.rs` 実バイナリ e2e（happy/fail-closed/human） | done |
| T5 | 検証（fmt/check/lint/test/build-cli/automation+script check/headless-deps/prek）+ evidence ledger + docs chain 完了 | done |

## 検証済み事項（Windows headless）

実装・検証ともに完了（PR #509、branch
`feat/274-cli-script-analyze`、base `b5a8919`）。canonical =
mise、全て exit 0:

- `mise run fmt-check` / `check-cli` / `lint-cli`（`-D warnings`
  warning 0）/ `test-cli`（args unit + `cli_analyze` 9 +
  `cli_e2e` 26 全 Pass）/ `build-cli`
- `mise run check synvelis-automation` / `check synvelis-script`
  （consume 側非回帰）
- `mise run check-headless-deps`（12 headless crates に GUI
  deps なし）
- `mise exec -- prek run --all-files` / `git diff --check`
- `git merge origin/main` → Already up to date（Cargo.lock
  差分なし）

実測の内訳は `evidence/sv274/issue-274-cli-script-analyze-ledger.md`
を参照。

## NotRun / Deferred（外部 gate）

- 実 GPUI editor/IME/2Window/Focus/Undo/DPI/pixel・実 OS 受入 —
  synvelis#114/#267（NR-01）。
- 実 Luau.Analysis Native Bridge・mlua 同一バイナリ・3OS 配布 —
  ruviform#560・rf#692 lane（NR-02）。`LuauPortAnalyzer` は
  port-level honest fallback。
- composition root mount（NR-04）・DTCG token（NR-05）・
  `gpui-test` scenario（NR-06）・Rhai/Yaml analyzer（NR-07）。
- macOS/Linux build・3OS 実機・`*-full`/`*-msrv`/`gw10`/`test-rt`/
  `check-app-*` lane — Daily Full CI・GUI lane 別 Issue 委譲。
