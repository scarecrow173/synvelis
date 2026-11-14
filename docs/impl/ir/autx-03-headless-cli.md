---
id: "IMPL-034XgKVz7slU1QHG15t6tc"
type: "impl"
status: "completed"
title: "AUTX-03 synvelis-cli 独立 Headless 実行ファイル・JSON/Exit Code (synvelis#262)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/262"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-cli/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-cli/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/main.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/args.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/commands.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/envelope.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/error.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/exitcode.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/host.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/script.rs"
      - type: "file"
        path: "crates/synvelis-cli/tests/cli_e2e.rs"
      - type: "file"
        path: "docs/test-specs/autx-03-headless-cli.md"
      - type: "file"
        path: "docs/impl/ir/autx-03-headless-cli.md"
      - type: "file"
        path: "docs/impl/exp/autx-03-headless-cli.md"
      - type: "file"
        path: "evidence/sv262/autx-03-headless-cli-ledger.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "tools/check-headless-deps.mjs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/command-gateway.md"
    - "docs/test-specs/command-gateway.md"
    - "docs/test-specs/autx-02-cli-adapter.md"
    - "docs/test-specs/autx-03-headless-cli.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-02-cli-adapter.md"
    - "docs/impl/ir/autx-05-workflow-dsl.md"
    - "docs/automation/design-trust-boundary.md"
    - "docs/automation/design-workflow-run.md"
    - "docs/automation/spec.md"
    - "evidence/sv261/autx-02-cli-adapter-ledger.md"
    - "evidence/sv264/autx-05-workflow-dsl-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/test-specs/autx-03-headless-cli.md"
  derived-by: []
  verifies:
    - "docs/test-specs/autx-03-headless-cli.md"
  verified-by: []
  references:
    - "docs/test-specs/autx-03-headless-cli.md"
    - "docs/impl/exp/autx-03-headless-cli.md"
    - "evidence/sv262/autx-03-headless-cli-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/272"
    - "https://github.com/scarecrow173/synvelis/issues/273"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-03-headless-cli.md"
    rejected: []
---

# AUTX-03 synvelis-cli 独立 Headless 実行ファイル・JSON/Exit Code (synvelis#262)

## Summary

GUI/Native/WebView/GPUI/GPU/windowing に依存しない独立実行ファイル
`synvelis-cli` を新設した。薄い adapter/composition root として
`synvelis-gateway` の `HeadlessSessionHost`・`LocalCliAdapter`・
`Gateway`・`CommitGate`・`DirectoryBundleStore`/`ProjectController`
を consume し、capability 実行・認可・commit・policy の第二実装を
持たない。`action list|describe|prepare|invoke`・`project info`・
`render`・`action run --file`・`workflow run --file`・
`--help`/`--version` を提供し、versioned `CliEnvelope`
（schema_version=1）と決定的 exit code 写像（0,2〜10）を実装した。
file 分類は拡張子↔kind/language/backend を fail-closed に判定し、
`.luau`/`.yaml`/`.yml`/`.rhai` の受理と `.lua`（Lua 5.4）・未知
拡張子・kind/version/backend 不一致の typed reject を実バイナリ
E2E で固定した。

## Context

synvelis#262（AUTX-03）の headless CLI スライス。Coordinator task
spec の編集範囲は `crates/synvelis-cli/`（新規 crate 全体）・
`mise.toml`（CLI タスク追加）・`tools/check-headless-deps.mjs`
（`HEADLESS_CRATES` 追加）・root `Cargo.toml`（members 追加）・
`docs/impl/{ir,exp}/autx-03-headless-cli.md`・
`docs/test-specs/autx-03-headless-cli.md`・`evidence/sv262/`・
生成 index・`Cargo.lock` に限定される。AUTX-02（PR #423）で
`LocalCliAdapter`/`CliSessionStore`/`HeadlessSessionHost` が、
AUTX-05（PR #424）で YAML Workflow DSL v1 と `synvelis-automation`
の型付き Plan 契約が統合済みであり、本スライスはそれらを依存と
して consume するだけで gateway/automation 本体は編集しない。

本スライスの達成範囲は Issue 受入条件のうちコマンド surface・
JSON/Exit 契約・fail-closed file 分類・headless 独立性・dry-run
非変更性・非昇格の in-crate/実バイナリ検証まで。`.luau` Script の
実 VM 実行（#272）・YAML/Luau Workflow の実 step 実行（#273）・
Macro/Workflow coordinator 配線（AUTX-04/05/06）・実 2-process
lock・実 OS credential/session・3 OS 実機受入は後続スライスに
Deferred。

## Implementation

### Changes

- `crates/synvelis-cli/`（新規 crate、`publish = false`、workspace
  lints/edition/rust-version 継承、GUI 依存なし）:
  - `src/args.rs`: hand-rolled parser（CLI framework 非依存）。
    global flag `--format human|json|ndjson`・`--project`・
    `--principal`・`--session-credential`・`--yes`・`--dry-run`・
    `--help`/`-h`・`--version`/`-V`。command 別 flag 表で未知 flag・
    必須値欠落・`--args`/`--args-file` 併用を typed reject。
    `script` command・`--script` flag は parse 段階で拒否。
    operation 参照は `id@major.minor` 形式を要求。
  - `src/envelope.rs`: `CliEnvelope`（`schema_version=1`、status・
    operation_id・project_revision・result・error・references）と
    `to_line()`（json/ndjson 共通の 1 行 JSON）。
  - `src/error.rs`: `CliError` typed error（invalid-input /
    authentication / unsupported / unavailable / internal 等）と
    display 文言。
  - `src/exitcode.rs`: `ExitCode` enum（0,2〜10）と
    `GatewayOutcome`/`GatewayError`/`CliError` からの決定的写像。
    `Completed` のみ 0、`Accepted`/`Indeterminate` は 7。
  - `src/host.rs`: composition root。`--project` 指定時に
    `DirectoryBundleStore` から HEAD snapshot を load し、
    `ProjectController`・`MemorySnapshotSource`・`ProjectOracle`・
    policy clock を構成。read-only/dry-run は `InMemoryPolicyStore`
    + commit 境界なし、durable は `FilePolicyStore`
    （`synvelis-cli.policy.log`）+ `CommitGate`
    （`synvelis-cli.commit.journal`）。Gateway 構成を先に行い
    `synvelis.proposal.apply` descriptor を解決してから
    `CommitGate::open` する順序を固定。同一 oracle/clock を engine
    と commit 評価で共有。project は canonicalize 済み dir で、
    HEAD snapshot 不在は setup failure（exit 9）。
  - `src/script.rs`: file 分類。拡張子→(ScriptKind, Language,
    BackendRequirement) の確定的写像、`.yaml` Workflow DSL の
    `synvelis-automation` parse/validate（dry-run summary）、
    未対応 backend/version/kind mismatch の typed reject。
  - `src/commands.rs`: command dispatch。`action list`（registry
    descriptor 列挙）・`action describe`（id/version filter）・
    `action prepare`/`invoke`（`HeadlessSessionHost::handle_claim`
    に `CliSessionClaim::Unattended` を渡し `Gateway::handle` の
    outcome を envelope 化）・`project info`（snapshot/revision/
    dirty state）・`render`（production provider 不在のため
    fail-closed `cli.unsupported`）・`action run --file`/
    `workflow run --file`（file 分類 → dry-run では DSL validate、
    非 dry-run は backend 未 install で fail-closed）。
  - `src/main.rs`: parse → help/version short-circuit → dispatch →
    human/JSON/NDJSON emit。human は stdout、診断は stderr。
    JSON/NDJSON は `CliEnvelope` 1 文書のみ stdout。
  - `tests/cli_e2e.rs`: 実バイナリ subprocess E2E 18 tests
    （help/version・JSON envelope・action list/describe・
    prepare/invoke outcome・project info・file 分類 matrix・
    workflow DSL dry-run・no-script・usage error・dry-run 非変更・
    interactive claim fail-closed・render fail-closed・
    prepare target/revision 回帰）。
- workspace 配線: root `Cargo.toml` members に
  `crates/synvelis-cli` 追加、`tools/check-headless-deps.mjs` の
  `HEADLESS_CRATES` に `synvelis-cli` 追加、`mise.toml` に
  `check-cli`/`lint-cli`/`test-cli`/`build-cli` タスク追加
  （`build-cli` = `cargo build --locked -p synvelis-cli`）。
  `Cargo.lock` は cargo 解決で再生成。

### Decisions During Implementation

- **第二実行経路を作らない**: `action prepare`/`invoke` は全て
  `HeadlessSessionHost::handle_claim` → `Gateway::handle` の単一
  経路。CLI は request 組立・envelope/exit 変換のみを担い、
  capability 実行・Grant/Approval/Commit 評価を複写しない。
  `render` は production `MediaPort`/`JobPort` provider が存在
  しないため test stub で偽装せず fail-closed `cli.unsupported`
  を返す設計とした。
- **durable boot の構成順序**: `CommitGate::open` は registered
  `synvelis.proposal.apply` descriptor を要求するため、Gateway
  構成（engine/registry/proposal store）を先に完了させ、
  snapshot descriptor 解決後に gate を open する。project snapshot
  は gate open 前に load し identity/revision を確定する。
- **file 分類は推測しない**: 拡張子→kind/language/backend は
  `script.rs` の確定的表のみ。`.rhai` は PoC 受理（backend
  `rhai` は未 install のため非 dry-run は `cli.unsupported`）、
  `.lua`（Lua 5.4）・未知拡張子・command↔拡張子不一致・未知
  version は typed reject。`script run`/`--script` は存在しない。
- **exit code は単一写像表**: `exitcode.rs` が
  `GatewayOutcome`/`GatewayError`/`CliError` → `ExitCode` を一元
  管理。`Accepted`/`Running`/`Indeterminate` は完了ではないため
  exit 0 にせず 7。usage error は exit 2 の typed `CliError`。
- **`--format` pre-scan の prefix bug を修正**: global flag の
  pre-scan が `--` prefix を strip せず `format` を誤認し、
  `--format json` が human 出力に化ける不具合を parser 修正で
  解消（strip 後に flag 名比較）。
- **hand-rolled parser**: clap/any CLI framework を追加せず、
  command 別 flag 表 + global flag pre-scan の最小実装。外部依存
  追加は workspace.dependencies 変更を伴うため行わなかった。
- **review-protocol レビューでの 3 修正**: (a) `DirectoryBundleStore::
  ::create` は `<root>/snapshots/` を無条件作成するため、HEAD/
  snapshots の事前 probe（`require_saved_bundle`）を
  `boot_project`/`project_info` に追加し、dry-run/read-only や
  非 bundle dir への FS 副作用を排除。(b) `live_revision` が
  read-only boot（`NoAdminPort`）で None を返し consistency 既定が
  `expected-revision: 0` に落ちる問題を、boot binding revision への
  fallback で修正。(c) `action prepare` が `flags.target` を wire
  top-level にも複写し `TargetContract::None` の prepare capability
  で `UnexpectedTarget` になる問題を、top-level target を direct
  invoke のみに限定して修正（prepare は `operation.target` 内に
  正しく配置済み）。併せて human mode の gateway error が
  stdout/stderr 両方に出る重複を stderr のみに整理。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-03-headless-cli.md` — host 構成順序・file
  分類写像・envelope/exit 契約・dry-run 非変更の各試行結果を採用。

### Rejected

- 「`action run --file` が file を読んで直接 ScriptKind を推測」
  する設計 — 拡張子/metadata/command の不一致を推測実行する経路を
  作るため棄却（fail-closed 表に集約）。
- CLI 独自の簡易 outcome→exit 変換 — Gateway outcome/error の
  写像を CLI ごとに分散させると契約が乖離するため、
  `exitcode.rs` 単一表に集約。

## Validation

実測（全て `mise` canonical task、exit 0）:

- `mise run fmt-check` — `cargo fmt --all -- --check` 成功。
- `mise run check synvelis-cli` — `cargo check --locked
  --all-targets -p synvelis-cli` 成功。
- `mise run lint synvelis-cli` — clippy `-D warnings` 成功
  （28 件の初期指摘を全て修正、新規 warning なし）。
- `mise run test synvelis-cli` — `cargo test --locked
  -p synvelis-cli` 成功。lib unit tests + `cli_e2e` 18 tests
  全 Pass（実バイナリ subprocess）。
- `mise run check-cli` / `mise run test-cli` / `mise run
  build-cli` — 新設 task 経由で `cargo check/test/build
  --locked -p synvelis-cli` 成功。`build-cli` は
  `cargo build --locked -p synvelis-cli` を包む。
- `mise run check-headless-deps` — `synvelis-cli` を含む 12
  headless crates に GUI framework 依存なしを確認。

実バイナリ smoke（補助証拠）: `--help`/`--version`/
`action list`/`action describe`/未知 capability/file 分類 matrix/
workflow DSL dry-run/`--format json` envelope を直接実行で確認。
詳細は `evidence/sv262/autx-03-headless-cli-ledger.md`。

## Risks

- `action run --file`/`workflow run --file` の実行経路が未配線の
  まま「受理」が実行成功と誤読される危険。受理は fail-closed
  判定までであり、実 VM/Workflow 実行は #272/#273 に Deferred
  （test-spec Non-goals に明記）。
- `CliEnvelope` schema_version=1 の下位互換性。field/status 追加は
  version 更新を伴う必要がある。
- `MemorySnapshotSource` は process 内 snapshot を保持するため、
  durable commit 後の revision 反映は同一 process 内でのみ有効。
  別 process の commit は次回 boot で load される（2-process
  検証は Deferred）。
- `ProposalStore`/approval は in-memory のみ — `action prepare`
  の proposal ref は同一 process 内でのみ有効で、別 invocation の
  `proposal.apply` には到達しない（durable store は gateway 未
  提供、Deferred）。
- `render` は provider 不在で常に fail-closed。実 media/job
  provider 配線は別スライス。

## Follow-ups

- synvelis#272: `.luau` Action Script の VM 実行経路（`action run
  --file` の実 backend）。
- synvelis#273: `.yaml`/`.luau` Workflow の実 step 実行
  （`workflow run --file` の coordinator 配線、AUTX-04/05/06）。
- 実 2-process lock・実 OS credential/session・3 OS 実機 headless
  受入 — Gate/後続スライス。
- `synvelis-project` port adapter が必要な実 project mutation
  経路の結合検証。
