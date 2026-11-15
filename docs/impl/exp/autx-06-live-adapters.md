---
id: "EXP-034XLaLiveAdapters01"
type: "experiment"
status: "completed"
title: "AUTX-06 live adapters — attestation・reconcile・registry projection の試行"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/265"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/autx-06-live-adapters.md"
  depends-on:
    - "docs/automation/design-workflow-run.md"
    - "docs/impl/ir/autx-06-workflow-coordinator.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-06-live-adapters.md"
    - "evidence/sv265/autx-06-live-adapters-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-automation/src/live.rs"
    - "crates/synvelis-automation/src/live_registry.rs"
    - "crates/synvelis-automation/tests/live_adapters.rs"
    - "crates/synvelis-gateway/tests/autx06_live_adapters.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUTX-06 live adapters — attestation・reconcile・registry projection の試行

## 問題設定

前 slice の coordinator は `StepLane`/`JobLane` を Mock で
検証した。live adapter 接続では (a) run principal をどう
trusted caller へ attest するか（`StepCall` は credential を
運ばない）、(b) `DuplicateRequest` の `original` operation を
どの正本で照合するか（ledger/commit/job の 3 系統）、
(c) job observe/cancel を authority のどの面に限定するか、
(d) Gateway registry を automation `RegistrySnapshot` へ
どう「最新解決なし」で投影するか、の 4 点が未確定だった。

## 試行と結果

### E1: principal attestation — delegated のみ・human は fail-closed

候補は (i) `LocalCliAdapter::caller_delegated` で Agent
principal を agent-equivalent caller 化し Human も同経路へ
流す、(ii) Human principal は `Authentication` で拒否する。
(i) は unattended delegated が常に Agent である AUTX-02
契約に反し、verified-session を持たない human run を agent
scope へ silent downgrade する。(ii) を採用 — step call が
credential を運ばない現契約では fail-closed が唯一正直な
答えであり、human workflow の credential 委譲は別 Issue
の設計課題とした。

**結果**: 採用。`live_adapters.rs` の human principal test
で `Authentication` typed error・wire 未送出を確認。

### E2: `operation_result` の正本照合順序

`DuplicateRequest{original}` の照合先候補は keyed outcome
ledger・`CommitPort::reconcile`・`JobEngine::record` の 3 系統。
ledger は evict されうるため単独では不足。commit-bound
operation は gate の durable ledger が、admitted job submit
は book record がそれぞれ残る。3 段順序（outcome → commit →
job）で照合し、いずれにも無ければ `Unknown` — 成功・失敗を
捏造しない。

**結果**: 採用。`Gateway::operation_outcome` を gateway に
最小追加し、`live_adapters.rs` の duplicate/evict/unknown
test で `Settled`/`Pending`/`Unknown` の各分岐を確認。

### E3: job lane の到達面

候補は (i) `JobEngine::record`/`JobPort` を lane が直接読む、
(ii) `synvelis.job.get`/`synvelis.job.cancel` capability を
run scope で `Gateway::handle` へ送る。(i) は existence-
blind scope と disclosure 検査を迂回し第二正本化するため
却下。(ii) を採用 — observe/cancel の両方が wire 経路で、
phase/cancel の wire 名を `JobObservation`/`CancelResponse`
へ写像し、解釈不能は `Unknown`/`Unreachable` に落とす。

**結果**: 採用。`autx06_live_adapters.rs` の cancel
confirmed/unreachable test で provider verdict がそのまま
run settle に届くことを実測。

### E4: registry 投影 — 厳密版コピー・latest 解決なし

`RegistrySnapshot` は compile の正本入力であり、live capture
が "latest" を解決したり workflow-local schema を捏造すると
plan の capability ref が authority と乖離する。Gateway
`CapabilityRegistry::list`/`describe` と `SchemaRegistry`
の全宣言 schema を厳密 version のまま `SchemaNode` へ再帰
変換し、投影後 `validate()` を通す形を採用。converter は
descriptor の全 field（domain/target/effect/permission/
data-access/execution/availability/入出力 schema）を保持する。

**結果**: 採用。`live_adapters.rs` の converter test と
e2e の `capture_registry_snapshot` → `compile` 経路で
`synvelis.context.query@1.0` 等の厳密 ref 解決を確認。

### E5: e2e での gateway 共有 — `Arc<Mutex<Gateway>>` cell

rig は `Gateway` を値で所有し lane は `Arc<Mutex<Gateway>>`
を要求する。`mem::replace` で実 gateway を cell へ移し、
rig 側には呼ばれない placeholder を残す構成を採用
（`boot` は移動前に `rig.gw.registry()` を読むため順序を
固定）。lane の per-dispatch lock と test の trusted apply
が同一 authority を共有し、commit gate は `Arc<dyn
CommitPort>` として lane の reconcile にも供給した。

**結果**: 採用。`live_macro_submit_await_completes_and_
commits_once` で macro→prepare→submit→await→emit→
trusted apply→`CommitGate::commit`→`clip11_muted` を
単一 authority 上で実測。

## 結論

live adapter は coordinator の狭い 2 seam を実 authority
経路で満たし、第二正本・silent downgrade・verdict 捏造を
構造的に排除できた。`docs/test-specs/autx-06-live-adapters.md`
の case が unit/e2e で Pass。durable journal・実 OS・
GUI/MCP/CLI 実経路・実 JobPort は前 slice から継続して
NotRun/Deferred。
