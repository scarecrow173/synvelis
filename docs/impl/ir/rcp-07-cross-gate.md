---
id: "IMPL-034XN97v9lRgAb0lHXs0nv"
type: "impl"
status: "completed"
title: "RCP-07 cross-gate fixture/ledger slice"
created: "2026-11-15"
updated: "2026-11-15"
owners:
  - "scarecrow173"
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-render/tests/rcp07_gate.rs"
      - type: "file"
        path: "docs/test-specs/rcp-07-cross-gate.md"
      - type: "file"
        path: "evidence/sv296/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv296/verification.md"
      - type: "file"
        path: "docs/impl/ir/rcp-07-cross-gate.md"
      - type: "file"
        path: "docs/impl/exp/rcp-07-cross-gate.md"
    modified: []
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
  implements:
    - "docs/test-specs/rcp-07-cross-gate.md"
  implemented-by: []
  depends-on:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
    - "docs/test-specs/rcp-07-cross-gate.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/rcp-00-render-recipe.md"
    - "docs/impl/exp/rcp-07-cross-gate.md"
    - "evidence/sv296/acceptance-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcp-render-recipe-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcp-07-cross-gate.md"
    - "evidence/sv296/acceptance-ledger.md"
  references:
    - "docs/test-specs/rcp-07-cross-gate.md"
    - "docs/impl/exp/rcp-07-cross-gate.md"
    - "evidence/sv296/acceptance-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/rcp-07-cross-gate.md"
    rejected: []
---

# RCP-07 cross-gate fixture/ledger slice

## Summary

synvelis#296（RCP-GATE-01..12 cross-gate tracker）の
fixture/ledger slice を実装した。merge 済み RCP-01..05 基盤を
またぐ 7 つの headless cross-gate fixture
（`crates/synvelis-render/tests/rcp07_gate.rs`）と、全
case-ID の runnability を正直に記録する acceptance ledger・
verification 記録・test-spec・本 IR/exp を追加した。
production code（`crates/**/src/**`）・既存 test・canonical
doc は一切変更していない。実 Plugin/実 OS/実 GUI/RT/
Consumer lane は NotRun/Deferred/Blocked として ledger に
記録し、headless Pass を昇格していない。

## Context

Issue #296 は Render Checkpoint 機構を Recipe・Cache・
Branch/Adopt・Policy/Job・Missing Plugin・GPUI・VCS・RT・
OS/plugin・Consumer 境界で横断検証する長期 tracker。
本 slice の責務は「case-ID inventory + Case→fixture→oracle→
result ledger + merged foundations 上の runnable headless
subset」であり、実機/consumer gate 自体は NON-GOAL。
merged foundations: RCP-01（#290）・RCP-02（#291）・
RCP-03（#292）・RCP-04（#293 partial plumbing）・RCP-06
（#295）。RCP-05（#294）は branch-pin plumbing のみ merge
済みで専用 test-spec 未整備のため、pin 可能な範囲のみ被覆。

## Implementation

### Changes

- `crates/synvelis-render/tests/rcp07_gate.rs`（新規・
  ~1700 行）: 7 test。GATE-01 recipe→cache identity・
  false-valid 0、GATE-02 closure-scoped invalidation、
  GATE-03 verified→fault/GC/旧 Verified 保護・raw 不変、
  GATE-04 policy 認可→job 副作用 0・coalesce・cancel、
  GATE-05 branch edit→key 分離→adopt 単一 CAS→独立 oracle
  key 一致、GATE-06 store lookup verdict→resolver candidate
  の Exact substitution/fail-closed、GATE-07 job key/recipe
  key 分離と旧 key verified 保持。全て公開 API のみ。
- `docs/test-specs/rcp-07-cross-gate.md`（新規）: GATE-01..12
  inventory の oracle 種別・runnability 分類と、cross-gate
  fixture が pin する rule（RCP-GATE-XG-01..09）を固定。
  `verifies`: render-checkpoint spec + rcp-render-recipe-design。
- `evidence/sv296/acceptance-ledger.md`（新規）: 全 12 case
  の status・所有 gate・fixture 対応・NotRun/Deferred/
  Blocked/Unsupported 一覧・判定規則（実測なし Pass=0）。
- `evidence/sv296/verification.md`（新規）: 実行コマンド・
  OS・toolchain・SHA・exit code の実測記録。
- `docs/impl/exp/rcp-07-cross-gate.md`（新規）: 採用/却下した
  設計判断の experiment log。
- managed index 再生成: `docs/test-specs/README.md`・
  `docs/impl/ir/README.md`・`docs/impl/exp/README.md`
  （doc-maintenance `index apply`・sync_impl_index）。

### Decisions During Implementation

- **app test を追加しない判断**: `synvelis-app` は
  `synvelis-render` 型を名指せない依存境界があり、
  resolver→projection の両端は RCP-04（`rcp_04.rs`）と
  RCP-06（`rcp06_render*.rs`）が既に pin 済み。store→
  resolver 接合部は依存方向上 `synvelis-render` test が
  正位置であり、app test の追加は coverage に対し fixture
  複製コストが見合わないと判断（test-spec Non-goals・PR
  本文に記録）。
- **`RecipeBranchPin` は hashed identity**: 当初
  「adopt 後 live key == candidate branch key」を仮定したが、
  `recipe.rs` の doc（pin は hashed recipe 内の唯一の branch
  data）と実測で、branch pin 有無で key が分離されることを
  確認。oracle を「独立構成した muted live project の
  resolve_live key」とする形に修正し、branch/live key 分離
  そのものを GATE-05 の pin 対象として明示。
- **`handle` は同期 drive**: `JobOrchestrator::handle` は
  non-coalesced submit を即 `drive` するため、queued cancel
  の検証は `submit`（non-driving）経路を使用。
- **on-demand authorizer**: `TriggerEvent::Demand` は
  authorizer `Denied` で `TriggerOutcome::Denied`・job/
  executor/artifact 0 を確認（無許可副作用 0）。
- **GATE-08..10 は fixture を作らない**: 実行しない lane を
  stub/`#[ignore]` で疑似配置すると「実行していない証拠を
  fixture で偽装」するため、ledger の分類記録のみに留めた
  （Issue #296 対象外規定）。

## Related Experiments

### Adopted

- `docs/impl/exp/rcp-07-cross-gate.md` — 本 slice の設計判断
  （app test 非追加・branch pin hashed・handle 同期 drive・
  GATE-08..10 stub 非作成）を記録。

### Rejected

- `synvelis-app/tests/rcp07_gate.rs` の新設 — 依存境界と
  既存 fixture 被覆から coverage/cost 不均衡（上記 Decisions）。
- `#[ignore]`/stub による GATE-08..10 疑似配置 — 実行証拠の
  捏造になるため却下。
- 既存 rcp01..06 fixture helper の共有化 — test file 間の
  helper 共有機構は存在せず、contract 独立性のため各 file が
  自前 helper を持つ慣行に従い複製。

## Validation

- `mise exec -- cargo test -p synvelis-render --test
  rcp07_gate`: 7/7 green（補助証拠・反復用）。
- `mise run check synvelis-render`: Pass（all-targets）。
- `mise run fmt-check` / `lint synvelis-render` /
  `test synvelis-render` / `test synvelis-player` / `test-rt` /
  `check-headless-deps` / `prek run --all-files` /
  doc index plan/apply / impl audit: 実測結果は
  `evidence/sv296/verification.md` を正本とする。

## Risks

- headless fixture の Pass を実機/consumer 受入へ誤昇格する
  リスク → ledger の判定規則・status 語彙・本 IR/exp の明記で
  抑止。実機 lane は全て NotRun/Deferred/Blocked。
- RCP-05（#294）未整備部を推測で fixture 化するリスク →
  merge 済み plumbing で pin 可能な範囲のみに限定し、残りは
  Blocked として #294 へ委譲。
- `synvelis-app` coverage gap → 両端既存 fixture 被覆と依存
  境界を根拠に proportionate と判断。将来必要なら別 slice。

## Follow-ups

- synvelis#294（RCP-05）完了後: late attach・2人協業・
  non-RT renderer・Undo/Reload・old commit pin の完全 fixture
  を GATE-05 へ追加。
- `scarecrow173/ruviform#628/#629/#630/#631`・synvelis#114/
  #176・`scarecrow173/ruviform#294`・`scarecrow173/
  audio-lens#127`・synvelis#23 の外部 gate 到達後: GATE-03
  実 PCM readback・GATE-05/07/08 実 plugin/OS/RT・GATE-09
  3OS plugin matrix・GATE-10 consumer を別 slice/別 repo で
  実施。
- GATE-11: merge 後対象 SHA の必須 CI 証拠は Coordinator が
  統合検証として確認（本 slice は per-PR Full CI を追加しない）。
