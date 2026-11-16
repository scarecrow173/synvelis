---
id: "TSPEC-034YDhYCgjWoM4vF7KomTQ"
type: "test-spec"
status: "approved"
title: "issue-274-cli-script-analyze — test spec"
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
    - "docs/designs/issue-274-cli-script-analyze.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-03-headless-cli.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-274-cli-script-analyze.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-274-cli-script-analyze.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-274-cli-script-analyze — test spec

## Feature: script analyze --file headless Script Analyzer

### Rule: happy path（clean `.luau`）

- **Example**: 有効な `.luau` source を `script analyze --file
  <path> --format json`（`--project` 無し・temp dir 外）で実行 →
  exit 0、`status="completed"`、`result.diagnostics` 空配列、
  `result.language="luau"`、`result.source_revision=1`、
  `result.analyzer.analyzer_id="synvelis-luau-port"`、
  `result.injection.package_name="synvelis-script"`・
  `model_digest`/`stub_digest` が 64hex。

### Rule: diagnostics は result データ

- **Example**: `synvelisNope()` を呼ぶ `.luau` → exit 0・
  `completed`、`diagnostics[]` に `code="unknown-api"`・
  `severity="error"`・`kind="type"`・`source_revision=1`・
  `range.start/end` を持つ要素。構文エラー（unterminated
  string）→ `code="syntax-unterminated-string"`・`kind="syntax"`。
- **Example**: Hidden/Retired/Disabled/非対応 version に相当する
  `synvelis<HiddenName>` 呼出 → `unknown-api` が返り、当該名が
  stub に存在しないことを確認（live registry 由来のため
  fixture ではなく実名で pin — e2e では `synvelis` prefix の
  未知名で代替検証）。

### Rule: completion（`--cursor`）

- **Example**: `--cursor <offset>` 指定 → `result.completions`
  が配列で存在し、`label` が `synvelis` prefix の生成関数名を
  含む。`--cursor` 無し → `completions` キー自体が result に
  存在しない。cursor 境界外 → exit 2 `cli.invalid-input`。

### Rule: fail-closed 分類行列

- **Example**: `.rhai` → exit 5 `cli.unsupported`（message に
  `rhai` を保持）。`.lua` → exit 5。`.yaml`/`.yml`/未知拡張子/
  拡張子無し → exit 2 `cli.invalid-input`。非 UTF-8 file・
  不存在 file → exit 2 `cli.invalid-input`。

### Rule: 実行面非回帰（AUTX-CLI-FILE-01）

- **Example**: `script run --file x.luau`・裸 `script`・
  `script <other>`・`action run --script x.luau` → exit 2
  `cli.usage`。`script analyze --dry-run` → exit 2 `cli.usage`。
  `script analyze`（`--file` 不在）→ exit 2 `cli.usage`。
  `script analyze --file x.luau --bogus` → exit 2 `cli.usage`。

### Rule: human format stdout/stderr 分離

- **Example**: `--format human` で diagnostics 有り → stdout に
  `{file}:{line}:{col} error {code} {message}` 形の行、stderr は
  空（invocation 成功）。失敗系（不存在 file 等）→ stderr に
  `error:` 行、stdout に envelope/診断行なし。

## golden fixture 配置

`crates/synvelis-cli/tests/cli_analyze.rs` — 実バイナリ spawn
（`CARGO_BIN_EXE_synvelis-cli`）、`cli_e2e.rs` の
`cli()`/`stdout_json`/`scratch`/`write_file` パターン踏襲。
project fixture は不要（discovery boot — `--project` を渡さない
こと自体を pin する）。
