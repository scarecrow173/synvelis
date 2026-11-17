---
id: "ADR-sv518gH8iJ0kL6nO8pQ"
type: "adr"
status: "accepted"
title: "Issue #518 — job system 配線の方式決定"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/designs/issue-518-job-system.md"
    - "docs/specs/issue-518-job-system.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-518-job-system.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #518 ADR — job system 配線の方式決定

## D1: executor primitive = `OfflineJob` + bounded queue（JobExecutor 不採用）

`JobOrchestrator` は単一 writer で全 job 状態を共有する。per-job
executor ではなく「orchestrator を所有する 1 worker thread +
bounded command channel」が正しい粒度。`OfflineJob` は worker
lifecycle/cancel を提供し、`sync_channel` が queue bound を担う
（満杯 → typed、caller non-blocking）。JobExecutor 直接採用は
ruviform#744 merge + pin bump が前提で承認範囲外 — AC1 は
OfflineJob+quota 経路で実現可能（探索結論どおり）。

## D2: actor scheduler + `drive_step` quantum

`drive()`/`run()` を `drive_step`（demand 1 件分）の loop に
再構成し、worker は command drain → 1 quantum → command drain を
繰り返す。cancel/retry/grant 更新の latency ≤ 1 demand render —
同期等価物と同一の状態遷移を共有し、既存契約を壊さない。

## D3: cost estimate = model floor と caller-declared の要素別 max

caller-declared のみ信頼すると 0 を積む stub が budget/quota を
潜る。model（Recipe output contract・scope frames・calibration
seam）を算出し `max(caller, model)` で統合 — caller は締める方向
にのみ効き、model を下回れない。既存 test が caller estimate に
依存する契約を保存する。

## D4: project `render_policy` は opaque `serde_json::Value`

project は render 型を名指せない（render→project 依存で循環）。
declared preference は `Option<serde_json::Value>` で保持し、
解釈は render `PolicyPatch` の serde が担う。`#[serde(default)]`
+ identity migration（v14→v15）で旧 snapshot を保持。

## D5: adapter は `synvelis-workspace` に配置

`synvelis-gateway→synvelis-render` 依存は禁止（sv#517 制約）。
`RenderDispatch` port は gateway が定義し、impl は gateway+render
両方を名指せる唯一の headless-clean crate `synvelis-workspace` に
置く。cli/mcp-poc は `JobSystem::headless().dispatch()` を注入。

## D6: headless port 既定は fail-closed

`UnavailableExecutor`/`UnavailableSettings`/`NoDeclaredPolicies`/
`NoScopeDemands` は typed reject/failure を返す — render success
を捏造しない。実供給は各 owning gate（#296/#294/AC2 消費側）に委ねる。
