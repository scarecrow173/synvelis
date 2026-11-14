# AUTX-03 synvelis-cli Headless 実行ファイル — 検証・Deferred 台帳 (synvelis#262)

Issue #262「[impl/cli][AUTX-03] synvelis-cli 独立 Headless 実行
ファイル・JSON/Exit Code」の **headless CLI adapter スライス**の
実測記録。GUI/Native/WebView/GPUI/GPU/windowing に依存しない
独立実行ファイル `synvelis-cli`（新規 crate）を追加し、
`synvelis-gateway` の `HeadlessSessionHost`/`LocalCliAdapter`/
`Gateway`/`CommitGate`/project store を consume する薄い adapter
として `action list|describe|prepare|invoke`・`project info`・
`render`・`action run --file`・`workflow run --file`・
`--help`/`--version` を公開した。versioned `CliEnvelope`
（schema_version=1）と決定的 exit code 写像（0,2〜10）を実装し、
file 分類を fail-closed に固定した。

**本スライスの達成範囲**は Issue 受入条件のうち headless 独立
実行ファイル・JSON/Exit 契約・file 分類 fail-closed・dry-run
非変更・非昇格の in-crate/実バイナリ検証まで。`.luau` Script の
実 VM 実行（synvelis#272）・`.yaml`/`.luau` Workflow の実 step
実行（synvelis#273）・Macro/Workflow coordinator 配線
（AUTX-04/05/06）・実 2-process lock・実 OS credential/session・
3 OS 実機受入は後続スライスに Deferred — 本台帳に
`Deferred`/`NotRun` として記録する。

## 環境・対象 Revision・実行コマンド

| 項目 | 値 |
|---|---|
| 対象 Repository | scarecrow173/synvelis |
| 対象 Revision | branch `scarecrow173/sv262-autx3`、base `2b7b67653d39a930ff9e0a204b4e75af5f6cabce`（`origin/main` HEAD at branch time = AUTX-05 merge #424）。検証は commit 前 working tree で実施 — PR head SHA は PR 本文に記録 |
| OS / Arch | Windows / x86_64（Git Bash 経由） |
| Rust toolchain | rustc/cargo `1.99.1`（環境導入済み。`mise.toml` は Rust を pin しない。CI pin は `mise.ci.toml`） |
| Node | `v26.12.1`（doc 検証 script 用） |
| 実行日 | 2026-11-14 |
| 作業 worktree | `C:\Users\akiyoshi\orca\workspaces\synvelis\sv262-autx3` |
| binary artifact | `synvelis-cli`（`cargo build --locked -p synvelis-cli`） |

### 実行した正規検証コマンド（実測）

| コマンド | exit | 結果 |
|---|---|---|
| `mise run fmt-check` | 0 | `cargo fmt --all -- --check` 成功 |
| `mise run check synvelis-cli` | 0 | `cargo check --locked --all-targets -p synvelis-cli` 成功 |
| `mise run lint synvelis-cli` | 0 | clippy `-D warnings` 成功（新規 warning なし。`result_large_err` の `#[allow]` は既存 crate 慣行に一致） |
| `mise run test synvelis-cli` | 0 | `cargo test --locked -p synvelis-cli`。lib unit 5 tests + `cli_e2e` 18 tests 全 Pass（計 23 passed / 0 failed） |
| `mise run check-cli` | 0 | 新設 task。`cargo check --locked -p synvelis-cli` 成功 |
| `mise run test-cli` | 0 | 新設 task。`cargo test --locked -p synvelis-cli` 成功 |
| `mise run build-cli` | 0 | 新設 task。`cargo build --locked -p synvelis-cli` 成功（Native/WebView/GUI feature なしで構築） |
| `mise run check-headless-deps` | 0 | `synvelis-cli` を含む 12 headless crates に GUI framework 依存なし |

補足: 開発中に `cargo check/clippy/test/build -p synvelis-cli`
（cargo 直接実行）と実バイナリ smoke（`--help`/`--version`/
`action list`/`action describe`/file 分類 matrix/workflow DSL
dry-run/`--format json`）を反復実施した — 補助証拠であり
canonical は上記 mise タスク。

## Case 台帳（実バイナリ E2E）

`crates/synvelis-cli/tests/cli_e2e.rs` が `synvelis-cli` 実
バイナリを subprocess 起動する Golden。Case ID は test 名
（review-protocol レビューで検出した 3 件の CONFIRMED 指摘 —
`DirectoryBundleStore::create` の dry-run FS 副作用、read-only
boot の live revision 欠落、prepare への top-level target 混入 —
を修正し、回帰 test 2 件を追加して 18 tests）。

| Case ID | 期待 | 観測 | 判定 |
|---|---|---|---|
| `help_and_version` | `--help`/`--version` が GUI 資産・Project・credential なしで exit 0 | exit 0・usage/version 出力 | Pass |
| `action_list_json_envelope` | `--format json action list` が `CliEnvelope` 1 文書・登録 capability 列挙 | `schema_version=1`・`synvelis.operation.prepare`/`synvelis.proposal.apply`/`synvelis.proposal.approve`/`synvelis.proposal.preview`/project mutation・query capability を列挙 | Pass |
| `action_describe_known_and_unknown` | 既知 id は descriptor match、未知 id は `cli.unsupported` exit 5 | 既知 `synvelis.context.query` が matches 返却、`bogus.cap` が exit 5 | Pass |
| `action_prepare_produces_typed_outcome` | `action prepare` が typed `GatewayOutcome` を envelope 化 | outcome/status が envelope に反映 | Pass |
| `action_prepare_target_stays_inside_operation` | `--target` が inner `operation` に属し、prepare capability（TargetContract::None）へ top-level target を出さない | exit 3 `denied`（unexpected-target exit 2 ではない） | Pass |
| `action_prepare_dry_run_pins_boot_revision` | `--dry-run` の expected-revision consistency が boot revision（1）を pin し 0 に落ちない | exit 3 `denied`（revision error ではない） | Pass |
| `action_invoke_context_query_round_trip` | `synvelis.context.query` invoke が Completed | exit 0・result に query 結果 | Pass |
| `action_invoke_unknown_capability_version` | 未知 capability/version が `cli.unsupported` exit 5 | exit 5・typed error | Pass |
| `project_info_reports_saved_state` | HEAD snapshot を持つ project で `project info` が revision/state を報告 | snapshot/revision が envelope result に出力 | Pass |
| `project_info_fail_closed_paths` | HEAD snapshot 不在・不正 path が setup failure | exit 9 typed error | Pass |
| `dry_run_leaves_project_untouched` | `--dry-run` が Project/FS を変更しない | 実行前後で project bundle 不変 | Pass |
| `file_classification_matrix` | 拡張子↔kind/backend 写像と fail-closed reject | `.luau`→backend unsupported exit 5、`.yaml` を action に→kind mismatch exit 2、`.rhai`→PoC backend unavailable exit 5、`.lua`→Lua 5.4 非対応 exit 5、未知拡張子→invalid input exit 2、`.yaml` workflow→backend unsupported exit 5 | Pass |
| `workflow_dry_run_validates_dsl_without_executing` | 有効 `.yaml` Workflow DSL が parse/validate summary を返す | id/version/step/input/output 数を result に報告・exit 0・実行なし | Pass |
| `workflow_dry_run_rejects_invalid_dsl` | malformed/非対応 version `.yaml` が typed reject | exit 2 invalid input | Pass |
| `no_script_command_or_flag` | `script run`・`--script` が存在しない | usage error exit 2 | Pass |
| `usage_errors_are_typed` | 未知 flag・必須値欠落・`--args`/`--args-file` 併用が typed reject | exit 2 typed error | Pass |
| `interactive_claim_fails_closed` | credential issuer/session 不在で interactive claim が Human 昇格しない | fail-closed typed error | Pass |
| `render_fails_closed_unsupported` | production media/job provider 不在で `render` が fail-closed | `cli.unsupported` exit 5 | Pass |

lib unit tests（`script.rs` 分類写像・version 判定等）5 tests も
全 Pass。

## 受入条件カバレッジ（本スライス範囲）

| Issue 受入条件 | 本スライスでの扱い | 証拠 |
|---|---|---|
| `synvelis-cli --help/--version/action list/action describe` が Window/GPU/WebView 起動・GUI asset 検証なしで Headless 動作し、Capability Version/Schema/Availability を現行 Gateway と一致して出力 | **実装+検証済み（Windows のみ）** | `help_and_version`/`action_list_json_envelope`/`action_describe_known_and_unknown` Pass。`check-headless-deps` で GUI 依存なし確認。3 OS 実機は NotRun |
| JSON/NDJSON/人間可読の stdout/stderr 分離、Versioned Schema、Exit 0/2〜10、Accepted/Indeterminate/Unsupported/Conflict/Denied/Cancelled を Golden Fixture で別々に判定 | **実装+検証済み（代表 status）** | `CliEnvelope` schema_version=1・`exitcode.rs` 単一写像表・`action_list_json_envelope`/`action_invoke_unknown_capability_version`/`file_classification_matrix`/`usage_errors_are_typed` Pass。全 status の個別 Golden は一部のみ — Accepted/Cancelled 等の発生経路は provider/coordinator 不在で NotRun |
| `action run --file`/`workflow run --file` の ScriptKind/Version 対応と Project/JSON/Exit 結果の Gateway/Plan 一致 | **部分実装（fail-closed 判定まで）** | file 分類 matrix・DSL dry-run validate は Pass。実 Script/Workflow 実行は #272/#273 Deferred |
| `.rhai` は PoC のみ、Lua 5.4 非サポート、`script run`/`--script` なし、Unknown extension/Version/ScriptKind mismatch を fail-closed で拒否 | **実装+検証済み** | `file_classification_matrix`/`no_script_command_or_flag`/`workflow_dry_run_rejects_invalid_dsl` Pass |
| `action prepare/invoke` の Project Create/Change/Save/Reload が専用 CLI で独立 Expected/Observed を示し、直接 Project Store 編集で CommitFence/Grant を迂回しない | **部分実装** | `action_prepare_produces_typed_outcome`/`action_invoke_context_query_round_trip`/`dry_run_leaves_project_untouched` Pass。実 project mutation commit 経路（`synvelis-project` port adapter 結合）は Deferred |
| `--dry-run` が Project/FS/Job を変更せず、実行時 Revision/Scope/Capability 変更を Stale/Denied と判定し、Secret/Token/Grant を stdout へ出さない | **実装+検証済み（dry-run 非変更）** | `dry_run_leaves_project_untouched` Pass。stale revision/secret 非流出の CLI 経由結合検証は一部 Deferred |
| `--yes`/偽 `principal`・無人 CI が Manual Human へ昇格せず、OS/Project Lock/2Process Race・Crash/Journal で既存 Gateway の正本を維持 | **部分実装** | `interactive_claim_fails_closed` Pass（AUTX-02 契約の consume）。実 2-process lock・crash/journal 結合は Deferred |
| Native/WebView なしの Rust workspace check/test と CLI バイナリ build・実バイナリ E2E | **実装+検証済み（Windows のみ）** | 上記 mise task 全 Pass + `cli_e2e` 16 tests。非対応 OS 実機は NotRun |

## OS capability 判定（Issue 検証方法の要求）

| OS | headless 実機検証 | 判定 | 理由 |
|---|---|---|---|
| Windows | 実バイナリ E2E 実施 | **Pass** | `cli_e2e` 16 tests + smoke 全 Pass（x86_64） |
| macOS | 未実施 | **NotRun** | 本 worktree は Windows。3 OS 実機受入は後続 Gate |
| Linux | 未実施 | **NotRun** | 同上 |

非対応環境で Human を推測許可しないことは AUTX-02 契約を
consume する形で維持: `interactive_claim_fails_closed` が
credential issuer/session 不在での fail-closed を実証。

## Deferred / NotRun 一覧

| 項目 | 判定 | 理由・後続 |
|---|---|---|
| `.luau` Action Script の実 VM 実行（`action run --file` backend `luau-vm`） | **Deferred** | synvelis#272。backend 未 install のため非 dry-run は `cli.unsupported` で fail-closed |
| `.yaml`/`.luau` Workflow の実 step 実行・coordinator 配線 | **Deferred** | synvelis#273・AUTX-04/05/06。`workflow run --file` は DSL validate/dry-run まで |
| 実 2-process project lock・OS credential/session・crash/journal 回復の CLI 経由結合 | **Deferred** | `synvelis-project` port adapter + 実行環境待ち。in-crate の `CommitGate` fence/journal 契約は既存 gw10/REC suite で非回帰 |
| 実 project mutation（Create/Change/Save/Reload）の CLI 経由 commit | **Deferred** | `synvelis-project` port adapter が必要。read/dry-run/prepare/invoke query 経路は実装済み |
| Proposal/Approval store の durable 化（prepare→apply の cross-invocation） | **Deferred** | `ProposalStore` は in-memory のみ（durable variant は gateway 未提供）。`action prepare` の proposal ref は同一 process 内でのみ有効 — 別 invocation の `proposal.apply` には到達しない。policy/journal は durable（`FilePolicyStore`/`CommitGate`） |
| media/job provider を伴う `render` 実実行 | **Deferred** | production `MediaPort`/`JobPort` provider 不在。test stub で偽装せず fail-closed |
| Accepted/Indeterminate/Cancelled 等の全 exit status 個別 Golden | **NotRun** | provider/coordinator 不在で発生経路を構成できない。写像表は `exitcode.rs` に集約済み |
| macOS/Linux 実機 headless 受入 | **NotRun** | 3 OS 横断は後続 Gate |
| `*-msrv`・`*-full`・実 GUI/実 2-process レーン | **NotRun** | task spec で対象外と明記 |

## 既知の制約・注意

- `action run --file`/`workflow run --file` の「受理」は
  fail-closed 判定（拡張子/kind/version/backend）までを意味し、
  実 Script/Workflow 実行の成功を示さない。実行経路は #272/#273。
- `CliEnvelope` schema_version=1。field/status 追加は version
  更新を伴う。
- `render` は常に fail-closed（provider 不在）。実 media/job
  provider 配線は別スライス。
- durable boot の policy/journal は project-local
  （`synvelis-cli.policy.log`/`synvelis-cli.commit.journal`）。
  read-only/dry-run は durable write を残さない。
