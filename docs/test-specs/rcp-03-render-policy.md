---
id: "TSPEC-Rcp03RenderPolicyTestX"
type: "test-spec"
status: "approved"
title: "RCP-03 Render Policy / Save・Commit・Share Trigger / Async Job orchestration / Completeness verdict fixture tests"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/292"
  verifies:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/adr/rcp-00-render-recipe.md"
  supersedes: []
  superseded_by: []
---

# RCP-03 Render Policy fixture tests

## Purpose

synvelis#292 RCP-03 スライスの headless fixture が保証すべき
振る舞いを固定する。非排他 Trigger 集合・provenance 付き
Effective Policy 解決・Save/Commit/Share/OnDemand の typed
trigger port・pin された snapshot に対する async Job
orchestration（coalesce/idempotent retry/cancel/resume）・
BestEffort/RequireComplete の Completeness verdict・
Failure Matrix（cancel/retry/crash/quota/budget/permission/
remote consent）を `crates/synvelis-render/tests/rcp03_policy.rs`
で検証する。実 OS/filesystem・実 VCS provider・実 async job
system・実 RT lane・実 Gateway/Workflow/Script 投影・GPUI は
本 spec の対象外（`evidence/sv292/` で NotRun/Unsupported
記録）。

## Feature

`synvelis-render::policy` が Render Policy model と
Save/Commit/Share/OnDemand の typed trigger port を提供し、
`synvelis-render::render_job` が pinned target に対する
非 RT render job の状態機械・receipt・Completeness 判定を
提供する。publish 出口は RCP-02 の `publish_artifact` /
`attach_external_manifest` / `lookup` をそのまま消費し、
renderer・compiler・plugin host・media writer・VCS writer は
実装しない。

## Rules

- RCP-POL-01: `Manual`/`OnSave`/`OnCommit`/`OnShare`/
  `OnDemand` は非排他。Manual は常時有効。混合 Trigger・
  重複 Event は同一 PinnedTarget + resolved recipe set +
  purpose + policy resolution で Job coalesce し、二重
  Artifact Publish も二重 render work もしない。
- RCP-POL-02: Trigger/Scope/UI preference・budget・local
  override・completion の変更は `RecipeKey` を不変に保つ。
  音質に影響する RenderSettings（renderer/temporal/output
  入力）の変更のみ key を更新する — policy field は
  `resolve_recipe` の入力ではない。
- RCP-POL-03: Save failure では OnSave job を起動しない。
  Save success は durable receipt が返した固定 snapshot のみ
  pin し、live HEAD を先回りしない。Project が R1→R2 へ
  進んでも R1 pin の job completion は R1 の snapshot
  digest/revision に紐付いたまま。
- RCP-POL-04: Commit API 非対応は typed `Unsupported`。
  Commit 成功時は immutable locator(SHA 相当)を保持したまま
  `attach_external_manifest` で external attach する。
  provider 失敗/push 失敗は render job receipt と別 stream
  の `ExternalReceipt` に記録し混同しない。
- RCP-POL-05: `BestEffort` は save/commit を unblock し
  Incomplete を明示 mark、Share は Partial/欠落の明示を
  条件に許可。`RequireComplete` は Share/public 前に
  fail-closed で完全性を確認し、不足なら Block + 理由 +
  retry を返す。不完全を Complete と表示しない。
  missing/invalid/stale/unsupported/plugin-missing/denied/
  incomplete を typed gap として区別する。
- RCP-POL-06: Cancel/Retry/Crash/Quota/CPU budget/Permission/
  Remote Consent の Failure Matrix で、無許可実行・二重
  work・誤成功・RT blocking がない。budget/grant/consent は
  job 発行前に検証し、offline 失敗を realtime へ黙って
  fallback しない。
- RCP-POL-07: GUI/Headless CLI/Workflow/Script は同一の
  Gateway Action/Grant に投影される typed port/contract まで
  を本スライスで検証する。entry surface は receipt provenance
  であり job key には含めない — 同一 action+grant+target は
  surface を跨いで coalesce し、Target Revision と Job
  Receipt が一致する。実 Gateway/Workflow/Script 投影は
  NotRun。

## Examples

- `rcp_pol_01_mixed_triggers_coalesce_single_job`: 同一
  snapshot/recipe/purpose への Manual+OnSave+OnDemand(approved)
  event が 1 job に coalesce し artifact は 1 件。
- `rcp_pol_02_policy_fields_do_not_move_recipe_key`: trigger/
  scope/completion/budget/local override を変えても同一
  `RecipeKey`、renderer precision 変更で key 更新。
- `rcp_pol_03_save_failure_no_job_and_r1_pin_survives_r2`:
  save failure→job なし、save success→R1 pin、R2 編集後も
  completion receipt は R1 digest を指す。
- `rcp_pol_04_commit_unsupported_and_external_manifest`:
  commit API 非対応→typed Unsupported、commit 成功→SHA 不変
  external manifest attach、provider/push 失敗 receipt が
  render job receipt と別 stream。
- `rcp_pol_05_best_effort_vs_require_complete_share_gate`:
  missing/invalid/plugin-missing artifact で BestEffort=
  AllowedPartial(mark 付き)、RequireComplete=Blocked。
- `rcp_pol_06_failure_matrix_no_unauthorized_or_duplicate`:
  cancel/retry/crash/quota/budget/permission/remote consent
  の各 fault で typed 結果・二重 work なし・誤成功なし。
- `rcp_pol_07_entry_surfaces_share_projection`: Gui/Script
  entry が同一 action+grant で coalesce、receipt の target
  revision が一致。
