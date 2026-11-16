---
id: "EXP-034YqslY9xK2jHmNvQpRt"
type: "experiment"
status: "completed"
title: "AUTX-08 Leg C live-adapter leg の試行 (synvelis#267)"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/267"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx08_e2e.rs"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/autx-08-live-adapter-leg.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/autx-08-live-adapter-leg.md"
    - "docs/test-specs/autx-08-live-adapter-leg.md"
    - "docs/impl/exp/autx-08-live-adapter-leg-exp.jsonl"
    - "evidence/sv267/"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-automation/src/live.rs"
    - "crates/synvelis-automation/src/coordinator.rs"
    - "crates/synvelis-workspace/tests/autx08_e2e.rs"
    - "crates/synvelis-workspace/tests/rig/mod.rs"
    - "crates/synvelis-gateway/tests/autx06_live_adapters.rs"
    - "crates/synvelis-automation/tests/live_adapters.rs"
  defers: []
  deferred-by: []
---

# AUTX-08 Leg C live-adapter leg の試行

## 問題設定

Issue #267 AUTX-08 T-08-1 の AI lane。Leg B
（`autx08_e2e.rs`、GUI/CLI/MCP 3 surface same-wire
contract test）は merge 済みだが、`evidence/sv267/
acceptance-ledger.md` で AI lane（production
`GatewayStepLane`/`GatewayJobLane` の live adapter
surface）は Deferred のまま残っていた。

核心は 2 点: (a) `step_wire`（`StepCall` → wire JSON
producer）は adapter-internal private fn（live.rs L227）
で wire byte を公開 surface から観測できない — どの
観測面で Leg B との等価を証明するか。(b) production
lane を `rig::compose()` の `GatewayStack` 上にどう
組み立てるか（`gw` は値所有・lane は `Arc<Mutex<
Gateway>>` 要求・`ports` は `CommitPort` impl 済み・
`jobs` は `Option`）。加えて test を既存 file に足すか
sibling にするかの配置判断があった。

## 試行と結果

### E1: composition 可能性の確認

`crates/synvelis-automation/src/live.rs`（`GatewayStepLane`
L294/L330/L338）、`coordinator.rs`（`StepCall` L74・
`StepLane` L129・`OperationObservation` L101）、
`lib.rs`（`pub mod coordinator` L30・`pub use live::
{GatewayJobLane, GatewayStepLane}` L64・`pub use refs::
{Version, VersionedRef}` L71）を確認。公開 surface のみで
`invoke`/`operation_result` が `GatewayOutcome`/
`ErrorEnvelope`/`DuplicateRequest{original}`/
`OperationObservation` を返すことを確認した。

composition 先例は `crates/synvelis-gateway/tests/
autx06_live_adapters.rs`（L213 `cli_adapter`・L259
`GatewayStepLane::new`）と `crates/synvelis-automation/
tests/live_adapters.rs`（L101 `cli()`・L117 `step_call()`・
L206 `step_lane()`）。本 slice は workspace crate の test
であり `rig::compose()` の `GatewayStack`（`gw`/`ports`/
`jobs`/`clock`/`store`/`source`/`gate`）を使うのが Leg B
と同一 fixture 意味論を保つ最短経路と判断。

`synvelis-workspace` → `synvelis-automation` の dev-dep は
acyclic（automation は workspace に依存しない）・
headless-safe（`tools/check-headless-deps.mjs`
HEADLESS_CRATES 両者登録済み）・外部依存は serde/
serde_json/sha2 のみ。`synvelis-gateway`/`synvelis-cli`
が同一 dev-dep を持つ先例あり。Coordinator 承認済み。

### E2: `live_lane`/`live_step_call` helper の実装と初回実行

`autx08_e2e.rs` に Leg C section を追加。
`live_lane(with_authority)`: `compose(None)` で fresh
`GatewayStack` を得て `s.gw` を `Arc::new(Mutex::new(..))`
に包み、`LocalCliAdapter::new(AdapterId::new(
CLI_ADAPTER_RAW), Box::new(MemoryCliSessionStore::new()),
s.clock.clone())` を `Arc` 化、`s.ports.clone()` を
`Arc<dyn CommitPort>` へ unsize coerce、`s.jobs`
（`compose(None)` では `None`）をそのまま渡して
`GatewayStepLane::new` を呼ぶ。`with_authority` 時のみ
`issue_agent_authority(&s.ports)`。

`live_step_call()`: `StepCall` を Leg B 論理操作で構築
— `capability: VersionedRef::new(PREPARE_CAPABILITY, 1,
0)`、`args: note_add_args()`（`{"operation": {...}}`
shape verbatim）、`target: None`、`expected_revision:
Some(1)`、`idempotency_key: IdempotencyKey::new(
"autx08-live-1")`、`policy_epoch: None`、`principal:
agent_principal()`、`project_id: project().to_tagged()`、
`task: Some(task_ref("t1"))`、`grant: Some(grant_ref(
"g1"))`。

### E3: 4 assertion の設計と parity 比較面

4 test を既存 Leg B leg helper（`gui_leg`/`cli_leg`/
`mcp_prepare_call`/`wire`/`expected`/`complete`/
`mcp_structured`）との組合せで構成:

1. `autx08_live_adapter_lands_same_outcome` — `invoke`
   の `GatewayOutcome` が CLI/GUI outcome と一致、
   MCP `structuredContent` decode とも一致。
2. `autx08_live_adapter_unavailable_revision_fails_
   identically` — `expected_revision: Some(99)` で
   `SnapshotExpired` envelope が CLI/GUI と一致、
   MCP `ExternalError` 投影とも一致。
3. `autx08_live_adapter_without_authority_fails_closed`
   — `task`/`grant: None` + authority 未発行で
   delegated agent `StepCall` が CLI leg denial と
   同一 envelope、MCP 投影とも一致。
4. `autx08_live_adapter_duplicate_reconciles_original`
   — 同一 `StepCall` 再 `invoke` が `DuplicateRequest
   {original}`、`operation_result(&original)` が
   `Settled(Completed)`、独立 compose した CLI leg
   replay が指名する `original` と一致。

`operation_result` の reconcile は stage-1
（`gateway.operation_outcome`）で prepare outcome が
settle するため `jobs: None` で影響しないことを実測で
確認。

### E4: file 配置の比較

`autx08_live_e2e.rs` sibling 新設 vs `autx08_e2e.rs`
拡張を比較。sibling は Leg B helper の複製または
`pub(crate)` 化が必要で、「同一論理操作」の保証が
2 file に分散し fixture 意味論の drift リスクがある。
拡張なら parity 比較が同一 file 内で完結し、既存 leg・
`boot()`・fixture を一切変更しない。拡張を採用
（~730 行への肥大化は許容 — parity 保証の集約を優先）。

### E5: 検証

Windows worktree（rustc/cargo 1.99.1、mise 管理）で実施:
`cargo test -p synvelis-workspace --locked --test
autx08_e2e` 8/8 pass、`mise run fmt-check` /
`check synvelis-workspace` / `lint synvelis-workspace` /
`test synvelis-workspace` / `check-headless-deps` /
`test-rt` / `prek run --all-files` 全て exit 0。
`Cargo.lock` 差分は `synvelis-workspace` deps 欄への
`synvelis-automation` 1 行追加のみ。`audit_impl_record.js`/
`audit_experiment_log.js`/`doc_status.js lint` で
blocking finding 0。

## 採用した判断

- **配置は `autx08_e2e.rs` 拡張**: sibling 案は helper
  複製・`pub(crate)` 化・lease 外変更リスク・parity
  保証の分散により不採用。判断理由は `docs/discovery/
  autx-08-live-adapter-leg.md` Alternatives に記録。
- **観測面は公開 `StepLane` surface のみ**: `step_wire`
  private・adapter src READ-ONLY のため wire byte assert
  は不可能。`invoke`/`operation_result` の
  outcome/error/`DuplicateRequest`/`OperationObservation`
  parity で証明 — MCP leg の adapter-internal wire を
  outcome parity で証明した先例と同型。
- **`live_lane` は `compose(None)` のみ**: `GatewayJobLane`
  job 観測は本 slice 対象外。`committer` は `s.ports.
  clone()` の unsize coerce で十分（`GatePorts` は
  `CommitPort` impl 済み）。
- **fail-closed は `task`/`grant: None` + authority
  未発行**: `attest` が mint するのは delegated agent
  caller のみで、task/grant が wire に乗らなければ
  `GrantPolicy` が deny — CLI leg の unattended claim
  denial と同一 typed envelope。`Human` principal の
  `attest` fail-closed（`GatewayError::Authentication`）
  は adapter contract の別経路で本 leg 対象外。

## 結論

production `GatewayStepLane` が `rig::compose()` stack
上で Leg B 3 surface と同一の typed
`GatewayOutcome`/`ErrorEnvelope`/`DuplicateRequest`
verdict を返すことを headless で固定した。`step_wire`
内部 lowering 詳細・`GatewayJobLane` job 観測・live
catalog・dispatch-hop・CLI binary cross-check・real-OS/
EXT-L3 は #264/#265/後続 slice の責務として Deferred
のまま。`docs/test-specs/autx-08-common-e2e.md` には
公開 `StepLane` surface parity を固定した旨の note を
追記した。
