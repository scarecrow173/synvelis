---
id: "IMPL-rcp03-render-policy"
type: "impl"
status: "completed"
title: "RCP-03 Render Policy・Save/Commit/Share Trigger・Async Job/Completeness — Headless スライス"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/292"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-render/src/policy/mod.rs"
      - type: "file"
        path: "crates/synvelis-render/src/policy/model.rs"
      - type: "file"
        path: "crates/synvelis-render/src/policy/trigger.rs"
      - type: "file"
        path: "crates/synvelis-render/src/render_job/mod.rs"
      - type: "file"
        path: "crates/synvelis-render/src/render_job/model.rs"
      - type: "file"
        path: "crates/synvelis-render/src/render_job/orchestrator.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/rcp03_render_policy.rs"
      - type: "file"
        path: "docs/impl/ir/rcp-03-render-policy.md"
      - type: "file"
        path: "docs/impl/exp/rcp-03-render-policy.md"
      - type: "file"
        path: "docs/test-specs/rcp-03-render-policy.md"
      - type: "file"
        path: "evidence/sv292/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv292/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-render/src/lib.rs"
    supersedes: []
    superseded_by: []
  verifies:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/adr/rcp-00-render-recipe.md"
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCP-03 Render Policy headless implementation record

## Summary

synvelis#292 RCP-03: Render Policy model・非排他
Save/Commit/Share/OnDemand trigger port・pinned target に対する
async render job orchestration・BestEffort/RequireComplete
Completeness verdict を `crates/synvelis-render` に閉じて実装する。
scope: `crates/synvelis-render/` + 本 doc chain +
`evidence/sv292/` + 生成 index。実 OS/VCS/async/RT/Gateway/GPUI
は対象外。

## Context

- canonical contract: `docs/specs/render-checkpoint.md` §6
  (Render Policy / Save / Commit / Share)・§Non-negotiable
  Invariants・§Test Plan/Failure Matrix・§Evidence Status
  Vocabulary、`docs/designs/rcp-render-recipe-design.md`、
  `docs/adr/rcp-00-render-recipe.md`。
- RCP-01 (`recipe.rs`/`dependency.rs`) の `resolve_recipe` /
  `RecipeKey` / `RecipeResolution` を消費。policy field は
  recipe 入力ではないため trigger/scope/completion/budget 変更は
  key を動かさない（RCP-POL-02）。
- RCP-02 (`artifact/`) の verified publish・CAS index・
  `attach_external_manifest`・`lookup` を job の publish 出口・
  completeness 入力・commit attach に再利用。新しい renderer/
  media writer/VCS writer は作らない。
- `synvelis-project`/`synvelis-player`/`synvelis-gateway` 等は
  lease 外 — typed port/contract のみ提供し、実投影は NotRun。

## Implementation

### Changes

- `policy/model.rs`: `RenderPolicy`（`schema_version`/非排他
  `triggers`/`scope`/`quality_profile`/`completion`/
  `resource_budget`/`retention`）、`PolicyPatch` 3 層
  （project declared → user local override → operation intent）
  解決 + per-field `FieldProvenance` + `resolution_digest`、
  `LocalGrants`（認可は policy ではなく環境供給 — shared
  project の宣言が他者の CPU/GPU/network を強制しない）。
- `policy/trigger.rs`: `SaveEvent`/`CommitEvent`/`ShareRequest`/
  `DemandProposal` typed port、`TriggerReport`（entry surface
  + Gateway action + grant の同一投影）、`JobIntent` 組立、
  `ExternalReceipt`（save/commit/share/attach の独立 stream —
  render job receipt と構造的に分離）。
- `render_job/model.rs`: `PinnedSnapshot`（immutable target）、
  `RenderJobKey`（locator+digest+sorted recipe keys+purpose+
  policy resolution digest+action+grant）、`RenderJobState`
  state machine、`JobReceipt`/`ReceiptKind` append-only
  receipt、`JobFailureReason` typed taxonomy（retryable vs
  gate-managed vs permanent）、`CompletenessReport`/`Gap`/
  `ShareGateVerdict`。
- `render_job/orchestrator.rs`: `JobOrchestrator` — trigger
  routing（save failure→inert external receipt、commit
  unsupported→typed Unsupported、OnDemand→authorizer gate）、
  recipe 解決を pinned snapshot に固定、key coalescing（trigger/
  manifest provenance union・saved_revision first-pin-wins）、
  preflight（budget/grant/remote consent hard reject・
  concurrency/storage pending gate）、`drive()` state machine、
  `RenderExecutor` 注入 render、RCP-02 `lookup` による Exact
  再利用、verified `publish_artifact` のみの publish、
  completeness 評価、share gate（BestEffort partial 開示 /
  RequireComplete block）、cancel/retry/resume/crash、
  commit locator への `attach_external_manifest`（失敗は
  `ExternalManifestAttachFailed` で記録・job receipt 不変）。
- `tests/rcp03_render_policy.rs`: RCP-POL-01..07 の 21
  headless fixture（FakeExecutor/`InMemoryArtifactStore`/
  fixture probes）。

### Decisions During Implementation

- headless slice のため `run()`/`drive()` は同期 scheduler
  等価物 — state machine と receipt は async driver と同一。
- `RenderJobKey` に `policy_resolution` digest を含める
  （Issue 指定の "policy versioned resolution"）— policy
  変更は新 job identity だが `RecipeKey` は不変、artifact は
  lookup reuse で二重 publish しない。
- `OnDemand` は routing 時に `DemandAuthorizer` を必須化 —
  authorizer 非設定なら `Denied`（fail closed）。
- retryable worker failure は `pending_reason` に記録して
  `Queue` に戻すが、`InsufficientStorage`/`QuotaExceeded` は
  gate-managed として `drive()` の環境再評価のみに限定 —
  `retry()` が quota/storage gate を bypass しない。
- terminal job への同一 key 重複 event は coalesce（新規
  work なし・receipt に `Coalesced` 記録）— 冪等・fail-closed。
  retryable は `retry()`、永久 failure は policy 変更での
  新 key が再試行経路。
- trigger routing 経路の `JobCostEstimate` は caller 宣言
  （default=0）— cost model は後続 job-gate slice。

## Related Experiments

- `docs/impl/exp/rcp-03-render-policy.md`

## Validation

- `mise run fmt-check` / `check` / `lint` / `test
  synvelis-render`: 全て Pass（test 83 件、うち
  `rcp03_render_policy` 21/21）。`check synvelis-project`・
  `check synvelis-player` も Pass。
- docs index 再生成（doc-maintenance `index apply`）+
  `doc_status lint --dir docs/test-specs`（範囲内 finding
  0）+ `audit_impl_record`（ir/exp 範囲内 finding 0）。
- review-protocol adversarial pass: attach 失敗の無記録
  破棄・receipt 遷移の逆行/欠落を修正（詳細は
  `evidence/sv292/verification.md`）。
- 証拠台帳: `evidence/sv292/acceptance-ledger.md`（RCP-POL-01
  ..07 → fixture 対応）、`evidence/sv292/verification.md`
  （実測コマンド/結果/NotRun）。

## Risks

- lease 外 crate（project declared preference 本体・実 Gateway
  投影・実 VCS provider）は typed port まで。実結線は後続
  slice の責務。
- headless fixture の PASS を実 OS/VCS/Job Gate の PASS に
  昇格しない。

## Follow-ups

- RCP-05 (実 VCS provider/#231)、RCP-06 (GPUI/sv#295)、
  RCP-07 (cross-gate) での実投影・実 store 検証。
