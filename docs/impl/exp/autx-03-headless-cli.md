---
id: "EXP-034XgM2vNq8wRt5YuHb3Kd"
type: "experiment"
status: "completed"
title: "AUTX-03 synvelis-cli headless composition・file 分類・envelope/exit 契約の試行 (synvelis#262)"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-03-headless-cli.md"
    - "docs/impl/ir/autx-03-headless-cli.md"
    - "docs/automation/design-trust-boundary.md"
    - "docs/automation/design-workflow-run.md"
    - "evidence/sv262/autx-03-headless-cli-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/autx-03-headless-cli.md"
    - "docs/impl/ir/autx-03-headless-cli.md"
    - "evidence/sv262/autx-03-headless-cli-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/272"
    - "https://github.com/scarecrow173/synvelis/issues/273"
  deferred-by: []
---

# AUTX-03 synvelis-cli headless composition・file 分類・envelope/exit 契約の試行 (synvelis#262)

## 問題設定

synvelis#262 の headless CLI スライスで、新設 `synvelis-cli` crate が
(a) `synvelis-gateway` の既存 composition（`HeadlessSessionHost`・
`LocalCliAdapter`・`Gateway`・`CommitGate`・project store）を
どう組み立てるか、(b) `--project` の有無で durable/read-only を
どう分岐するか、(c) file 拡張子・ScriptKind・backend・version の
不一致をどう fail-closed に判定するか、(d) `GatewayOutcome`/
`GatewayError`/`CliError` をどう versioned `CliEnvelope` と決定的
exit code に写像するか、の 4 点を試行した。

## 試行と結果

### E1: host composition — 単一 Gateway 経路 vs CLI 独自 dispatch

**仮説**: CLI が capability descriptor を直接読み、prepare/invoke を
CLI 側で組み立てて `Gateway::handle` に渡す薄い経路で十分ではないか。

**結果**: 部分的に採用。`action list`/`describe` は registry の
read-only 投影として CLI が直接参照するが、`prepare`/`invoke` は
必ず `HeadlessSessionHost::handle_claim` → seal 発行済み
`CallerIdentity` → `Gateway::handle` の単一経路を通す。CLI は
request 引数の組立と outcome→envelope 変換のみを担い、capability
実行・Grant/Approval/Commit 評価を複写しない（第二実行経路を
作らない契約）。`render` は production `MediaPort`/`JobPort`
provider が存在しないため、test stub で capability を偽装登録せず
常に fail-closed `cli.unsupported`（exit 5）を返す。

### E2: durable vs read-only boot — `CommitGate::open` の順序

**仮説**: project snapshot load → store → `CommitGate::open` →
Gateway 構成の順で素直に書けるのではないか。

**結果**: 順序を修正して採用。`CommitGate::open` は registered
`synvelis.proposal.apply` descriptor を要求するため、Gateway
構成（engine/registry/proposal store/snapshot descriptor）を先に
完了させてから gate を open する必要があった。また project
snapshot は gate open 前に load して identity/revision を確定し、
同一 `ProjectOracle`/policy clock を engine と commit 評価で共有
する。`--dry-run`/read-only は `InMemoryPolicyStore` + commit 境界
なしで durable write を残さない。project dir は canonicalize し、
HEAD snapshot 不在は setup failure（exit 9）に fail-closed。

### E3: file 分類 — 確定的写像 vs metadata 推測

**仮説**: 拡張子だけでなく file 内 metadata を読んで ScriptKind/
Language/Backend を推測し、寛容に受理できるのではないか。

**結果**: 棄却。拡張子→(ScriptKind, Language, BackendRequirement)
の確定的写像のみを採用。`.luau` → `luau-vm`、`.yaml`/`.yml` →
`synvelis-automation`（Workflow DSL のみ）、`.rhai` → `rhai`
PoC（backend 未 install）、`.lua`（Lua 5.4）・未知拡張子は reject。
command↔拡張子不一致（`action run --file x.yaml` 等）・未知/
未対応 version・metadata/language/kind 不一致・未 install
backend は全て typed reject で推測実行しない。`script run`/
`--script` は parse 段階で拒否。`.yaml` Workflow DSL の
`--dry-run` は `synvelis-automation` の parse/validate と summary
metadata 報告までで、step 実行・Project 変更を行わない。

### E4: envelope/exit — outcome 写像の一元化 vs 分散

**仮説**: 各 command が個別に exit code を決めれば十分ではないか。

**結果**: 棄却。`exitcode.rs` に `GatewayOutcome`/`GatewayError`/
`CliError` → `ExitCode` の単一写像表を集約した。`Completed` のみ
exit 0、`Accepted`/`Indeterminate`/`Running` は完了ではないため
exit 7、`InvalidInput`=2・`Denied/Auth`=3・`Conflict`=4・
`Unsupported`=5・`PreparationRejected`=6・`ConfirmedCancelled`=8・
`Unavailable`=9・`InternalError`=10 を決定的に写像。usage/parse
error も typed `CliError` として envelope 契約で返り exit 2。
`--format json`/`ndjson` は stdout に `CliEnvelope` 1 文書のみ、
診断は stderr に分離する。

### E5: 実測 — `tests/cli_e2e.rs` 18 tests + smoke

実バイナリ subprocess E2E を実装し全件 Pass（`mise run test
synvelis-cli` exit 0）。help/version・JSON envelope・action
list/describe・prepare/invoke outcome・project info・file 分類
matrix・workflow DSL dry-run・no-script・usage error・dry-run
非変更・interactive claim fail-closed・render fail-closed を網羅。
開発中に `--format json` が human 出力に化ける不具合を検出し、
global flag pre-scan が `--` prefix を strip していなかった
parser bug を修正（実装側の緩和ではなく root cause 修正）。

review-protocol レビュー（cooperative + adversarial pass）で
CONFIRMED 3 件を検出・修正: (a) `DirectoryBundleStore::create` の
`snapshots/` 無条件作成が dry-run/read-only・非 bundle dir に FS
副作用を残す → HEAD/snapshots 事前 probe で排除;
(b) read-only boot の `live_revision` 欠落が dry-run consistency を
`expected-revision: 0` に落とす → boot binding revision へ
fallback;(c) prepare への top-level `target` 複写が
`TargetContract::None` で `UnexpectedTarget` → direct invoke のみ
へ限定。回帰 test 2 件（`action_prepare_target_stays_inside_
operation`・`action_prepare_dry_run_pins_boot_revision`）を追加。

## 結論

4 つの設計判断全てが実バイナリ E2E で裏付けられ、Issue 受入条件の
headless 独立性・JSON/Exit 契約・fail-closed file 分類・非昇格を
本スライス範囲で実装できた。`.luau` Script 実 VM 実行（#272）・
YAML/Luau Workflow 実 step 実行（#273）・実 2-process lock・実 OS
credential/session・3 OS 実機受入は後続スライスに Deferred
（`evidence/sv262/autx-03-headless-cli-ledger.md` 参照）。
