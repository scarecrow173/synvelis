# issue-274-cli-script-analyze — `script analyze --file` headless Script Analyzer 検証・Deferred 台帳 (synvelis#274)

Issue #274「[impl/cli][SV-SCRIPT-04] Script Editor」の
**headless CLI Script Analyzer slice**（SV274-NR-03）の実測記録。
`synvelis-cli script analyze --file <path> [--cursor <byte-offset>]`
を新設し、

`extension→language 分類 → script::read_source →
host::boot_discovery() ScriptStack → LuauStub::generate →
DefinitionInjection::from_stub → AnalyzerRegistry{LuauPortAnalyzer}
→ analyze_with_injection → verify_revision → CliEnvelope`

を実配線した。analyzer 成功は実行権限・Grant・approval・
project truth を生まない（AUTX-SCR-ANA-01 — Gateway wire
request・`host::handle`・claims を一切発行しない）。Issue #274
は OPEN 継続（auto-close しない・AC checkbox 未更新 — 本台帳が
tracking 権威）。実 GPUI editor・実 OS・実 FFI bridge・mount・
token・Rhai analyzer は別 Issue gate 所有。

## 環境・対象 Revision・実行コマンド

| 項目 | 値 |
|---|---|
| 対象 Repository | scarecrow173/synvelis |
| 対象 Revision | branch `feat/274-cli-script-analyze`、base `b5a8919`（`origin/main` HEAD = sv262 run-file 実 VM/実 authority 配線 #507 + index 再生成 #508。前提の analyzer 契約 `AnalyzerRegistry`/`LuauPortAnalyzer`/`analyze_with_injection`/`verify_revision`・`boot_discovery()` ScriptStack capture を含む）。検証は commit 前 working tree で実施 |
| OS / Arch | Windows 11 (x86_64) |
| Rust toolchain | mise 管理 toolchain（`mise.toml` 正本。CI pin は `mise.ci.toml` 1.99.0） |
| 実行日 | 2026-11-16 |
| 作業 worktree | `G:\orca\workspaces\synvelis\sv274-cli-analyze` |
| binary artifact | `synvelis-cli`（`mise run build-cli` = `cargo build --locked -p synvelis-cli`） |

### 実行した正規検証コマンド（実測）

| コマンド | exit | 結果 |
|---|---|---|
| `mise run fmt-check` | 0 | `cargo fmt --all -- --check` 成功 |
| `mise run check-cli` | 0 | `cargo check --locked --all-targets -p synvelis-cli` 成功 |
| `mise run lint-cli` | 0 | `cargo clippy --locked --all-targets -p synvelis-cli -- -D warnings` 成功（warning 0） |
| `mise run test-cli` | 0 | `cargo test --locked -p synvelis-cli`。args unit + lib unit + `cli_analyze` **9 tests 全 Pass** + `cli_e2e` **26 tests 全 Pass**（既存非回帰含む） |
| `mise run build-cli` | 0 | `cargo build --locked -p synvelis-cli` 成功（dev profile、GUI/Native/WebView feature なし） |
| `mise run check synvelis-automation` | 0 | consume 側非回帰 check 成功 |
| `mise run check synvelis-script` | 0 | consume 側非回帰 check 成功 |
| `mise run check-headless-deps` | 0 | `node tools/check-headless-deps.mjs` — 12 headless crates に GUI framework deps なし（analyzer verb が headless 性を維持する証拠） |
| `mise exec -- prek run --all-files` | 0 | whitespace/EOL/BOM/yaml/toml/actionlint 全 Pass |
| `git diff --check` | 0 | whitespace error なし |

## Case 台帳（実バイナリ E2E — `cli_analyze.rs` 9 tests）

| Case | 結果 | 内容 |
|---|---|---|
| `clean_luau_completes_without_project` | Pass | clean `.luau` → exit 0 `completed`、`language:"luau"`/`source_revision:1`/`diagnostics:[]`/`completions` キー無し、`analyzer_id:"synvelis-luau-port"`/`contract_version:1`/`completion:true`、`injection.package_name:"synvelis-script"`+`model_digest`/`stub_digest` 64hex。`--project` 無し・scratch dir に生成物ゼロ（`project_revision` キー無し） |
| `unknown_api_is_a_completed_diagnostic_not_a_failure` | Pass | `synvelisNope()` → exit 0 `completed`、`code:"unknown-api"`/`severity:"error"`/`kind:"type"`/`source_revision:1`/`range{start,end}` |
| `syntax_diagnostic_is_completed_data` | Pass | unterminated string → exit 0 `completed`、`code:"syntax-unterminated-string"`/`kind:"syntax"` |
| `wrong_arity_is_diagnosed_against_injected_signature` | Pass | completion `detail` の `declare function` signature から arity を導出し `arity+1` 引数で呼出 → exit 0 `completed`、`code:"wrong-arity"` |
| `cursor_completion_uses_live_registry_stub` | Pass | `--cursor <end>` → `completions` 非空・全 `kind:"generated-function"`・`source_revision:1`・`synvelis*` 生成名を含み hidden/retired 名を含まない。未知 prefix（`synvelisZz`）→ 空配列（キーは存在） |
| `cursor_out_of_range_is_invalid_input` | Pass | `--cursor 999999` → exit 2 `cli.invalid-input`（analyzer `InvalidRequest` → typed 写像） |
| `unsupported_and_invalid_inputs_fail_closed` | Pass | `.rhai`→exit 5 `cli.unsupported`（message に `rhai`）、`.lua`→exit 5、`.yaml`/`.yml`/`.txt`/noext→exit 2 `cli.invalid-input`、不存在 file→exit 2、非 UTF-8→exit 2 |
| `script_execution_surface_stays_rejected` | Pass | `script run`/裸 `script`/`script <other>`/`action run --file`（`--project` 無し）/`--dry-run`/`--file` 不在/未知 flag → 全て exit 2 `cli.usage`（AUTX-CLI-FILE-01 非回帰） |
| `human_format_routes_diagnostics_to_stdout` | Pass | `--format human`: diagnostics は stdout の `file:line:col severity code message` 行、stderr 空。clean→`no diagnostics` 行。失敗系→stderr `error:` のみ・stdout 空 |

## Unit test 追加（`args.rs` `#[cfg(test)]`）

| test | 内容 |
|---|---|
| `parses_script_analyze` | `script analyze --file x.luau` → `Command::ScriptAnalyze{cursor:None}`、`--cursor 42` → `Some(42)` |
| `rejects_bad_script_analyze_invocations` | `--file` 不在・未知 flag・非数値 cursor・余分 positional → parse error |
| `rejects_script_run_and_script_flag`（拡張） | `script`/`script other` も reject に追加（既存 `script run`/`--script` pin 維持） |

## AC 対応（本 slice 範囲 — Issue AC checkbox は未更新）

| Issue #274 受入行（headless slice が触れる範囲） | 本 slice での状態 |
|---|---|
| Headless CLIのScript AnalyzerはGUI不要で起動可能（SV274-NR-03） | **Pass（Windows headless）** — `boot_discovery()` ScriptStack のみで完結、`--project` 不要、GUI/FFI/外部プロセス不要 |
| Synvelis公開CapabilityのLuau補完/型診断/構文診断/Unknown API | **Pass（headless CLI）** — live `ScriptApiModel` 由来 `LuauStub` injection 経由で unknown-api/wrong-arity/構文診断・prefix 補完を実バイナリで確認。実 Editor 描画は SV274-NR-01 Deferred |
| Registryの更新・Hidden/Retiredの漏洩やstale補完がない | **Pass（headless）** — injection は boot 時 live model 由来 stub のみ（手書き/fixture コピーなし）。`SourceRevision(1)` echo を `verify_revision` で検査。Hidden/Retired/Disabled/非対応 version は stub に出ないため補完・診断対象に「存在」しない（`synvelis-script` 側 `luau.rs` test が pin、CLI e2e は `synvelis*` 生成名 + hidden/retired 非含有を確認） |
| 未提供AnalyzerはUnsupportedと表示し、Runtime自体の起動可否とは区別する | **Pass（headless CLI）** — `.rhai` は registry `AnalyzerError::Unavailable` → `cli.unsupported` exit 5（language 保持）。Runtime 起動可否とは別軸の typed 語彙 |
| Source revision 遅延応答 | **Pass（echo 検査）** — 単発同期 invocation のため `verify_revision` echo 確認のみ。遅延応答の consumer reject は sv#274 headless slice（`synvelis-automation` test）で検証済み |

## NotRun / Deferred 対応表（外部 gate — 本 slice では実施しない）

| Case | 内容 | 状態 | 所有 gate / 必要ファイル |
|---|---|---|---|
| SV274-NR-01 | 実 GPUI Script Editor（IME・2Window・Focus・Undo・DPI・pixel）・実 OS 受入 | **NotRun** | synvelis#114/#267（GUI-L2・実機 lane） |
| SV274-NR-02 | 実 Luau.Analysis Native Bridge 結合・mlua 同一バイナリ・3OS 配布・外部 luau-lsp 非依存の実配布検証 | **NotRun** | ruviform#560・rf#692 lane。`LuauPortAnalyzer` は port-level honest fallback でそのまま置換可能 |
| SV274-NR-03 | `synvelis-cli` への Script Analyzer verb 配線 | **Pass（本 slice）** | 本台帳 — Windows headless 実バイナリ E2E まで |
| SV274-NR-04 | composition root（`SynvelisApp`/`NativeShell`）mount・dock open 配線 | **NotRun** | `mount_script_editor_view` 呼出側配線 slice |
| SV274-NR-05 | `assets/themes/` DTCG token 追加 | **NotRun** | assets slice |
| SV274-NR-06 | `gpui-test` scenario | **NotRun** | `check-app-gpui`/`test-app-gpui` lane |
| SV274-NR-07 | Rhai/Yaml analyzer 実装 | **NotRun** | `AnalyzerRegistry` 登録追加で開放される設計（本 slice は typed `Unavailable` fail-closed のみ） |
| — | macOS/Linux build・3OS 実機・製品 E2E | **NotRun** | 本環境は Windows — Daily Full CI / #267 gate |
| — | `*-full`/`*-msrv`/`gw10`/`test-rt`/`check-app-*` lane | **NotRun** | Daily Full CI・GUI lane 別 Issue 委譲（POLICIES.md §5） |

## 補足

- `script analyze` は `action`/`workflow` の `FileClass` kind 軸を
  持たないため `script::classify` を流用せず、module 内の小さな
  `classify_language`（`.luau`→luau、`.rhai`→rhai、`.lua`→
  Unsupported、その他→InvalidInput）を新設した。
- `AnalyzerRegistry` は invocation ごとに新規構築 +
  `LuauPortAnalyzer` 単独登録（deterministic・永続状態なし）。
  将来の言語 analyzer 追加は登録追加のみで済む。
- `AnalyzerError` → `CliError` の写像は `analyze.rs` の
  `analyzer_error` 1 関数に集約: `Unavailable`/`Unsupported`→
  `cli.unsupported`(5)、`InvalidRequest`→`cli.invalid-input`(2)、
  `VersionMismatch`/`StaleResult`/`Internal`/その他→
  `cli.internal`(10)。
- docs index（`docs/**/README.md`）は Coordinator が統合時に
  `doc-maintenance` で再生成 — 本 slice では未同期のまま。
