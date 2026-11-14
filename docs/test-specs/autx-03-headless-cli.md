---
id: "TSPEC-034XgJPQyivnWHKo5099nI"
type: "test-spec"
status: "approved"
title: "AUTX-03 synvelis-cli Headless 実行ファイル・JSON/Exit Code 検証仕様 (synvelis#262)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/262"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/autx-03-headless-cli.md"
  depends-on:
    - "docs/specs/command-gateway.md"
    - "docs/test-specs/command-gateway.md"
    - "docs/test-specs/autx-02-cli-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/autx-03-headless-cli.md"
    - "docs/automation/design-trust-boundary.md"
    - "docs/automation/design-workflow-run.md"
    - "evidence/sv262/autx-03-headless-cli-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/designs/command-gateway-design.md"
  verified-by: []
  references:
    - "docs/impl/ir/autx-03-headless-cli.md"
    - "docs/impl/exp/autx-03-headless-cli.md"
    - "evidence/sv262/autx-03-headless-cli-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/272"
    - "https://github.com/scarecrow173/synvelis/issues/273"
  deferred-by: []
---

# AUTX-03 synvelis-cli Headless 実行ファイル・JSON/Exit Code 検証仕様 (synvelis#262)

## Purpose

Issue #262 の headless CLI スライスが守るべき不変条件を、
`crates/synvelis-cli/tests/cli_e2e.rs` の実バイナリ fixture 群として
固定する。「GUI/Native/WebView/GPUI/GPU/windowing に依存しない独立
実行ファイルである」「実行権限の正本は `synvelis-gateway` の単一
`Gateway`/`CommitGate` であり CLI は第二の実行・認可・commit 経路を
持たない」「CLI 引数は Human 権限を mint しない」「file 拡張子・
ScriptKind・Language・Backend・Version の不一致は推測実行せず
fail-closed で typed reject する」「stdout の機械可読出力は versioned
`CliEnvelope` 1 文書で、人間可読出力と混在しない」という契約を、
将来の変更が静かに緩めないよう検証仕様として残す。実装は
`docs/impl/ir/autx-03-headless-cli.md`、試行は
`docs/impl/exp/autx-03-headless-cli.md`、実測記録は
`evidence/sv262/` が正本。

## Feature

`synvelis-cli` が GUI 非依存の独立バイナリとして Capability
Gateway の read/prepare/invoke と headless Project 操作を公開し、
`action list|describe|prepare|invoke`・`project info`・`render`・
`action run --file`・`workflow run --file`・`--help`/`--version` を
提供すること。全ての応答は `CliEnvelope {schema_version,status,
operation_id?,project_revision?,result?,error?,references?}` の
versioned 契約で返り、exit code は決定的な写像表に従うこと。

## Rules

### AUTX-03-T-R-01 Headless 独立性

- `synvelis-cli` は Native/WebView/GPUI/GPU/windowing 依存を持たず、
  `tools/check-headless-deps.mjs` の `HEADLESS_CRATES` 検査対象である。
- `--help`/`--version`/`action list`/`action describe` は Project
  path・credential・TTY 状態なしで完了する（Window/GPU/WebView 起動・
  GUI asset 検証を要求しない）。
- Capability の id/version/schema/availability は Gateway
  `CapabilityRegistry` の登録内容と一致し、CLI 独自の第二
  Registry を持たない。

### AUTX-03-T-R-02 Gateway 権威の単一経路

- `action prepare`/`action invoke` は `HeadlessSessionHost` の
  `handle_claim` → seal 発行済み `CallerIdentity` → `Gateway::handle`
  の単一経路のみを通る。CLI は capability 実行ロジック・認可評価・
  commit 判定を複写しない。
- durable boot は project-local の `FilePolicyStore`
  （`synvelis-cli.policy.log`）と `CommitGate`
  （`synvelis-cli.commit.journal`）を使用し、read-only/dry-run boot は
  `InMemoryPolicyStore` と commit 境界なしで FS に durable 変更を
  残さない。
- `--dry-run` は Project/FS/Job を変更しない（scratch/preparation
  preview に限定）。

### AUTX-03-T-R-03 Caller 権限の fail-closed

- `--yes`・principal 引数・credential 引数・env・TTY 状態・JSON
  引数は Human 権限を mint しない。`CliSessionClaim::Unattended` は
  常に delegated `Agent` caller（`OriginKind::LocalCli`）に解決する。
- interactive session claim は検証可能な credential issuer/session が
  存在しない環境で fail-closed に失敗する（`cli.authentication`
  系の typed error、exit 3）。
- bearer credential・session secret・grant token を stdout の
  envelope/result に出力しない。

### AUTX-03-T-R-04 file 分類と fail-closed reject

- 拡張子↔kind/language/backend の写像は確定的:
  `.luau` → Action/Workflow Script（backend `luau-vm`）、
  `.yaml`/`.yml` → Workflow DSL のみ（backend `synvelis-automation`）、
  `.rhai` → Action/Workflow Script PoC のみ（backend `rhai`、
  production backend 未 install）、`.lua`（Lua 5.4）・未知拡張子は
  reject。
- command↔拡張子の不一致（例: `action run --file x.yaml`）、未知・
  未対応 version、metadata/language/kind 不一致、install されていない
  backend は全て typed reject であり、推測実行しない。
- `script run` command・`--script` alias は存在しない。
- `.yaml` Workflow DSL の `--dry-run` は parse/validate と summary
  metadata の報告までであり、step 実行・Project 変更を行わない。

### AUTX-03-T-R-05 出力・Exit Code 契約

- `--format json`/`ndjson` は stdout に `CliEnvelope` 1 文書のみを
  出力し、人間可読 text・診断・進捗を stdout に混在させない
  （診断は stderr）。
- exit code 写像は決定的: `0` Completed/proposal-ready/approval-required、
  `2` InvalidInput、`3` Denied/Authentication/Authorization/Approval、
  `4` Conflict/StaleRevision/Idempotency、`5` Unsupported、
  `6` PreparationRejected/ExecutionFailure、`7` Accepted/Indeterminate、
  `8` ConfirmedCancelled、`9` Unavailable/SetupFailure、
  `10` InternalError。`Accepted`/`Running` は exit 0 にしない。
- usage/parse error も typed `CliError` として envelope/error
  契約で返り、exit 2。

## Examples

- Given `synvelis-cli --help` / `--version`, When 実バイナリを起動
  する, Then GUI 資産・Project・credential なしで exit 0 と
  versioned 出力を返す（`help_and_version`）。
- Given `synvelis-cli action list --format json`, When 実行する,
  Then `CliEnvelope` 1 文書が stdout に出て `result` が
  `synvelis.operation.prepare`・`synvelis.proposal.apply` 等の
  登録 capability を列挙する（`action_list_json_envelope`）。
- Given `action describe synvelis.context.query`（version 省略）, When
  実行する, Then 登録済み全 version の descriptor match が返る
  （`action_describe_known_and_unknown`）。
- Given `action invoke` で未知 capability id または未登録 version,
  When 実行する, Then `cli.unsupported` typed error・exit 5。
- Given `action run --file` に `.yaml`（kind 不一致）・`.lua`
  （Lua 5.4 非対応）・未知拡張子・install されていない `.luau`/
  `.rhai` backend, When 実行する, Then file 分類 matrix の通り
  exit 2/5 の typed reject で推測実行しない
  （`file_classification_matrix`）。
- Given `--session-credential` のみで interactive session を要求,
  When 実行する, Then Human 昇格せず fail-closed（`interactive_
  claim_fails_closed`）。
- Given `--yes`・偽 `--principal`・env を付与した無人呼出し, When
  mutating operation を実行する, Then caller は delegated Agent の
  ままで Human 権限経路に到達しない。
- Given `--dry-run` 付きの prepare/invoke, When 実行する, Then
  Project snapshot・policy log・journal が変化しない
  （`dry_run_leaves_project_untouched`）。
- Given 有効な Workflow DSL `.yaml`, When `workflow run --file
  --dry-run` する, Then parse/validate 済み summary（id/version/
  step/input/output 数）を返し exit 0、実行はしない
  （`workflow_dry_run_validates_dsl_without_executing`）。
- Given malformed/非対応 version の Workflow DSL `.yaml`, When
  `--dry-run` する, Then typed invalid-input・exit 2
  （`workflow_dry_run_rejects_invalid_dsl`）。
- Given `script run` または `--script` flag, When parse する,
  Then usage error として拒否される（`no_script_command_or_flag`）。
- Given Project bundle が HEAD snapshot を持たない, When
  `--project` 付き command を実行する, Then setup failure・exit 9
  （`project_info_fail_closed_paths`）。

## Guarantees

- `synvelis-cli` は単一 `synvelis-gateway` composition の薄い adapter
  であり、capability 実行・認可・commit・policy の第二実装を持たない。
- 全応答は versioned `CliEnvelope` で機械可読に返り、exit code は
  上記写像表どおり決定的である。
- CLI 引数・TTY・env・`--yes` は権限を mint せず、unattended は常に
  delegated Agent、interactive は検証済み credential のみ Human。
- file 分類・backend・version の不一致は推測実行せず typed reject
  される。`.rhai` は PoC 受理のみで production 実行経路を持たない。
- `--dry-run` と read-only boot は durable state を変更しない。

## Non-goals

- `.luau` Action/Workflow Script の実 VM 実行・`.yaml`/`.luau`
  Workflow の実 step 実行 — synvelis#272/#273 の後続スライス。
- Macro/Workflow の `validate|preview|run|status|cancel` coordinator
  配線 — AUTX-04/05/06 の後続スライス。
- 実 OS credential/session store・実 2-process lock・実
  journal/crash-recovery の CLI 経由結合検証 — 後続スライス。
- 3 OS 実機の headless 受入・非対応 OS 実機検証 — Gate/後続スライス。
- media/job provider の production 実装 — 存在しない capability は
  fail-closed で不在のまま（test stub で偽装しない）。

## Risk

- `action run --file`/`workflow run --file` の実行経路が未配線の
  まま「受理された」ことを実行成功と誤読される危険がある。
  本 test-spec は「受理 = fail-closed 判定まで」を保証し、実行は
  明示的に Non-goal とする。後続スライスが実行経路を足す際は
  file 分類・version・backend 判定の契約を再利用し、緩めないこと。
- `CliEnvelope` schema_version=1 の field 追加・status 追加は
  downstream consumer を壊しうる。version を上げる場合は移行契約が
  必要。
- exit code 写像は process contract であり、将来 status 追加時に
  写像表の更新を忘れると exit code が静かにずれる。写像は
  `exitcode.rs` の単一表に集約し、e2e が代表 status を固定する。
