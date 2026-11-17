---
id: "IMPL-034YqsnyJeTYCw8WID3wqz"
type: "impl"
status: "completed"
title: "AUTX-08 Leg C live-adapter leg — AI surface same-wire contract test (synvelis#267)"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/267"
  changes:
    added:
      - type: "file"
        path: "evidence/sv267/live-adapter-leg.md"
      - type: "file"
        path: "docs/specs/autx-08-live-adapter-leg.md"
      - type: "file"
        path: "docs/designs/autx-08-live-adapter-leg.md"
      - type: "file"
        path: "docs/plans/autx-08-live-adapter-leg.md"
      - type: "file"
        path: "docs/tasks/autx-08-live-adapter-leg.md"
      - type: "file"
        path: "docs/test-specs/autx-08-live-adapter-leg.md"
      - type: "file"
        path: "docs/discovery/autx-08-live-adapter-leg.md"
      - type: "file"
        path: "docs/impl/ir/autx-08-live-adapter-leg.md"
      - type: "file"
        path: "docs/impl/exp/autx-08-live-adapter-leg.md"
      - type: "file"
        path: "docs/impl/exp/autx-08-live-adapter-leg-exp.jsonl"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx08_e2e.rs"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/test-specs/autx-08-common-e2e.md"
      - type: "file"
        path: "evidence/sv267/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv267/case-definitions.md"
      - type: "file"
        path: "evidence/sv267/verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/autx-08-live-adapter-leg.md"
    - "docs/plans/autx-08-live-adapter-leg.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/autx-08-headless-common-e2e-slice-same-wire-3-surface-contract-test-synvelis-267.md"
    - "docs/impl/ir/autx-06-live-adapters.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/autx-08-live-adapter-leg.md"
    - "docs/designs/autx-08-live-adapter-leg.md"
    - "docs/test-specs/autx-08-live-adapter-leg.md"
    - "docs/test-specs/autx-08-common-e2e.md"
    - "docs/impl/exp/autx-08-live-adapter-leg.md"
    - "evidence/sv267/live-adapter-leg.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/autx-08-live-adapter-leg.md"
  verified-by: []
  references:
    - "crates/synvelis-workspace/tests/autx08_e2e.rs"
    - "crates/synvelis-workspace/tests/rig/mod.rs"
    - "crates/synvelis-automation/src/live.rs"
    - "crates/synvelis-automation/src/coordinator.rs"
    - "crates/synvelis-gateway/tests/autx06_live_adapters.rs"
    - "crates/synvelis-automation/tests/live_adapters.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-08-live-adapter-leg.md"
      - "docs/impl/exp/autx-08-live-adapter-leg-exp.jsonl"
    rejected: []
---

# AUTX-08 Leg C live-adapter leg — AI surface same-wire contract test (synvelis#267)

## Summary

Issue #267 AUTX-08 T-08-1 の AI lane headless 部を閉じた。
`crates/synvelis-workspace/tests/autx08_e2e.rs` に Leg C
section を追加し、production `GatewayStepLane`
（`synvelis_automation::live`、AUTX-06 merge 済み）を
`rig::compose()` の fresh `GatewayStack` 上に
`GatewayStepLane::new` で組み立て、Leg B と同一論理操作
（`synvelis.operation.prepare@1.0` + `midi.note.add`
on `source10`・`expected_revision:1`・`t1`/`g1`・agent
principal）の `StepCall` に対し 4 assertion —
(i) `GatewayOutcome` parity、(ii) `SnapshotExpired`
`ErrorEnvelope` parity、(iii) fail-closed denial parity、
(iv) `DuplicateRequest{original}` parity +
`operation_result` → `OperationObservation::Settled`
reconcile — を固定した。`cargo test -p
synvelis-workspace --locked --test autx08_e2e` で
8/8 pass（新規 4 + Leg B 非回帰）。

## Context

`docs/automation/tasks.md` §AUTX-08（T-08-1〜T-08-3、
Issue #267）。前提 slice: Leg B
（`autx-08-headless-common-e2e-slice` — GUI/CLI/MCP
3-surface same-wire contract test + `compose()`/
`GatewayStack` 抽出）、AUTX-06 live adapters
（`GatewayStepLane`/`GatewayJobLane`）、AUTX-07 catalog
lowering seam、AUTX-02/03（`LocalCliAdapter`/
`HeadlessSessionHost`）。境界: `step_wire` は
adapter-internal private fn で wire byte は観測不能 —
公開 `StepLane` surface（`invoke`/`operation_result`）の
outcome/error/`DuplicateRequest` parity が証明面
（`docs/test-specs/autx-08-common-e2e.md` Rule
AUTX-E2E-SAME-01 の MCP leg convention と同型）。
real-OS/EXT-L3・live catalog・dispatch-hop・job lane
は範囲外 — headless Pass を実機 Pass に昇格しない。

## Implementation

### Changes

- `crates/synvelis-workspace/Cargo.toml`:
  `[dev-dependencies]` に `synvelis-automation = { path =
  "../synvelis-automation" }` を追加（Coordinator 承認済み
  — acyclic・headless-safe・外部依存は serde/serde_json/
  sha2 のみ・`synvelis-gateway`/`synvelis-cli` 同一先例。
  sv293 Worker が `[dependencies]` に追加する
  `ruviform-graph`/`ruviform-audio-core` とは別 section で
  競合しない）。`Cargo.lock` は `synvelis-workspace`
  deps 欄への `synvelis-automation` 1 行追加のみ。
- `crates/synvelis-workspace/tests/autx08_e2e.rs`:
  - module doc を「Leg B: one wire, three surfaces; Leg
    C: the AI/live-adapter surface」へ更新し、Leg C の
    観測面（公開 `StepLane` seam、wire byte 非 assert）
    を明記。
  - import 追加: `std::sync::{Arc, Mutex}`、
    `synvelis_automation::coordinator::{OperationObservation,
    StepCall, StepLane}`、`synvelis_automation::{
    GatewayStepLane, VersionedRef}`。
  - `live_lane(with_authority)`: `compose(None)` の fresh
    `GatewayStack` 上に `GatewayStepLane::new` を組立 —
    `s.gw` を `Arc<Mutex<..>>`、`LocalCliAdapter::new
    (CLI_ADAPTER_RAW, MemoryCliSessionStore, s.clock)`、
    `s.ports.clone()` を `Arc<dyn CommitPort>`、
    `s.jobs`（`None`）。`with_authority` 時のみ
    `issue_agent_authority`。
  - `live_step_call()`: `StepCall` を Leg B 論理操作で
    構築（`PREPARE_CAPABILITY` 1.0・`note_add_args()`
    verbatim・`expected_revision: Some(1)`・
    `idempotency_key "autx08-live-1"`・agent principal・
    `project().to_tagged()`・`t1`/`g1`）。
  - `autx08_live_adapter_lands_same_outcome`: `invoke`
    の `GatewayOutcome` が CLI/GUI outcome および MCP
    `structuredContent` decode と一致。
  - `autx08_live_adapter_unavailable_revision_fails_
    identically`: `expected_revision: Some(99)` で
    `SnapshotExpired` envelope が CLI/GUI と一致、
    MCP `ExternalError` 投影とも一致。
  - `autx08_live_adapter_without_authority_fails_closed`:
    `task`/`grant: None` の delegated agent `StepCall`
    が CLI leg denial と同一 envelope、MCP 投影とも一致。
  - `autx08_live_adapter_duplicate_reconciles_original`:
    同一 `StepCall` 再 `invoke` が `DuplicateRequest
    {original}`、`operation_result(&original)` が
    `Settled(Completed)`、独立 compose した CLI leg
    replay が指名する `original` と一致。
- `evidence/sv267/acceptance-ledger.md`: T-08-1 行の
  AI lane を実測に基づき `Pass (headless)` へ更新、
  NotRun/Deferred 一覧を Leg C 実施分だけ更新。
- `evidence/sv267/case-definitions.md`: live-adapter
  lane 行を追記（AUTX-E2E-SAME-01 の AI leg・
  AUTX-E2E-IDEM-01 の AI leg）。
- `evidence/sv267/verification.md`: Leg C 実測値を追記。
- `evidence/sv267/live-adapter-leg.md`（新規）: Leg C
  専用 evidence — 構成・4 assertion・実測コマンド・
  NotRun/Deferred 分離。
- `docs/test-specs/autx-08-common-e2e.md`: Non-goal
  「live lane の `step_wire` の等価は #264 接続後の別検証」
  に対し、本 slice が公開 `StepLane` surface parity を
  headless で固定した旨の note を追記のみ（front-matter
  ・Non-goal 文・構成は不変）。
- doc-chain 8 文書 + `docs/impl/exp/
  autx-08-live-adapter-leg-exp.jsonl`（新規）。

### Decisions During Implementation

- **配置は `autx08_e2e.rs` 拡張を採用**: sibling
  `autx08_live_e2e.rs` 案は Leg B helper
  （`note_add_args`/`issue_agent_authority`/`gui_leg`/
  `cli_leg`/`mcp_prepare_call`/`wire`/`expected`/`complete`/
  `mcp_structured`）の複製または `pub(crate)` 化が必要で、
  「同一論理操作」の保証が 2 file に分散し fixture 意味論の
  drift リスクがあった。拡張なら parity 比較が同一 file
  内で完結し、既存 leg・`boot()`・fixture を一切変更
  しない。判断理由は `docs/discovery/
  autx-08-live-adapter-leg.md` Alternatives に記録。
- **観測面は公開 `StepLane` surface のみ**: `step_wire`
  は private（live.rs L227）で wire byte assert は
  不可能かつ adapter src は READ-ONLY。`invoke`/
  `operation_result` が産出する `GatewayOutcome`/
  `ErrorEnvelope`/`DuplicateRequest{original}`/
  `OperationObservation` の parity で証明 — MCP leg の
  adapter-internal wire を outcome parity で証明した
  先例と同型。
- **`live_lane` は `compose(None)` のみ使用**:
  `GatewayJobLane` job 観測は本 slice 対象外のため
  `jobs: None` で十分。`operation_result` の stage-1
  （`gateway.operation_outcome`）で prepare outcome が
  settle し `committer.reconcile`/`jobs.record` には
  到達しない — 実測で `Settled(Completed)` を確認。
- **`committer` は `s.ports.clone()` の unsize coerce**:
  `GatePorts` は `CommitPort` impl 済み（rig/mod.rs
  L234）で `Arc<dyn CommitPort>` へそのまま渡せる。
  別途 committer を組み立てる必要はなかった。
- **fail-closed は `task`/`grant: None` + authority 未発行
  の組合せ**: `attest` は `Agent` principal を
  `caller_delegated` で mint するが、task/grant が wire
  に乗らなければ `GrantPolicy` が deny する — CLI leg の
  unattended claim denial と同一 typed envelope。
  `live_lane(false)`（`issue_agent_authority` なし）で
  `StepCall.task/grant = None` を検証。
- **review-protocol self-review で確認した境界**:
  `Human` principal の `attest` fail-closed
  （`GatewayError::Authentication`）は adapter contract
  の別経路であり本 leg の対象外 — delegated agent のみ
  を検証。`wf-req-*` request_id は surface 由来メタと
  して同一性対象外（Leg B convention）。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-08-live-adapter-leg.md` /
  `docs/impl/exp/autx-08-live-adapter-leg-exp.jsonl`:
  composition 可能性（`GatewayStepLane::new` over
  `rig::compose` stack）、file 配置の比較（拡張 vs
  sibling）、`operation_result` reconcile の stage 確認、
  および検証実測を記録した実験ログを採用した。

### Rejected

- `autx08_live_e2e.rs` sibling 新設: helper 複製・
  `pub(crate)` 化・lease 外変更リスク・parity 保証の
  分散により不採用（discovery Alternatives 参照）。
- wire byte 一致 assert: `step_wire` private・adapter
  src READ-ONLY のため不可能 — 公開 surface parity で
  代替。
- `GatewayJobLane` job 観測 leg の同時実装: #265
  coordinator 接続が前提であり本 slice 対象外
  （Deferred に記録）。

## Validation

- `cargo test -p synvelis-workspace --locked --test
  autx08_e2e`: 8/8 pass（`autx08_live_adapter_lands_
  same_outcome`・`autx08_live_adapter_unavailable_
  revision_fails_identically`・`autx08_live_adapter_
  without_authority_fails_closed`・`autx08_live_adapter_
  duplicate_reconciles_original` + Leg B 4 test 非回帰）。
- canonical lane（`mise run fmt-check` / `check
  synvelis-workspace` / `lint synvelis-workspace` /
  `test synvelis-workspace` / `check-headless-deps` /
  `test-rt` / `prek run --all-files`）の実測値は
  `evidence/sv267/verification.md`・`evidence/sv267/
  live-adapter-leg.md` と PR 本文に記録。Windows
  worktree、rustc/cargo 1.99.1（mise 管理）。
- `audit_impl_record.js`・`audit_experiment_log.js`・
  `doc_status.js lint`: docs/impl 配下の新規 doc で
  blocking finding 0 を確認。
- `*-msrv` 系はユーザー指定により未実行（NotRun）。
  merge 後 main SHA の必須 CI 証拠・real-OS
  GUI-L2/EXT-L3 は Coordinator/後続 slice の責務。

## Risks

- headless 等価は実 OS での同値を保証しない。実
  window・実 credential store・実 transport・実 provider・
  multi-process durable commit でのみ現れる差異は本
  slice の対象外（`docs/test-specs/autx-08-live-adapter-
  leg.md` Non-goals、`evidence/sv267/acceptance-ledger.md`
  NotRun/Deferred）。
- 公開 `StepLane` surface parity は `step_wire` の内部
  lowering 詳細（request-id 形式・consistency 写像の
  edge case）までは保証しない — adapter contract test
  群（`synvelis-automation/tests/`・`synvelis-gateway/
  tests/`）が担保し、本 slice は surface 間 verdict
  parity のみを固定する。
- `autx08_e2e.rs` の file 肥大化（~730 行）— Leg C 以降
  の leg 追加時は sibling 分割を再検討する余地があるが、
  本 slice では parity 保証の集約を優先した。

## Follow-ups

- #267 受入 lane: `evidence/sv267/` の real-os/ext-l3
  行を Required/Deferred/NotRun で埋める（本 slice は
  headless のみ）。
- #264 live adapter mount wiring・live-registry→
  `syn.autx.catalog` producer・`lower`→gateway
  dispatch-hop consumer 接続後、live catalog 実供給・
  dispatch-hop 経由の等価を再検証。
- #265 workflow coordinator live run 接続後、
  `GatewayJobLane` の job 観測 parity・honest-cancel
  semantics（`CancelRequested` 非昇格）を same-wire で
  固定。
- `synvelis-cli` `build_request`/`claim_for` 出力と
  `lower`/`step_wire` wire の field cross-check —
  別 crate での cross-check または共有 wire-builder
  抽出が必要（`synvelis-workspace` は `synvelis-cli`
  に依存しない）。
- MCP `proposal.apply` idempotency 引数 surface・control
  channel human authority 経路（GW-08 adapter contract）。
- docs index（`docs/**/README.md`）の再生成は
  Coordinator が統合時に実施。Issue #267 の Close・
  merge は Coordinator が行う（PR は `Refs #267`）。
