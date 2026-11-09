---
id: "PLAN-034UEufn0HEwKaQ2GRVCWB"
type: "plan"
status: "approved"
title: "VCS-02 Semantic Diff Engine implementation plan"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "DESIGN-034UEtYMoQUWsPmOBrl819"
  verified_by:
    - "TSPEC-034UEueLhP34NeCWclgoux"
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-02 Semantic Diff Engine implementation plan

## Scope

`crates/synvelis-project/src/diff/` 新設 + `lib.rs` export + `tests/vcs02_diff.rs` + docs chain。範囲外 crate・依存追加なし。

## File map

- `src/diff/mod.rs` — module doc・re-export。
- `src/diff/types.rs` — public 型一式。
- `src/diff/adapters.rs` — `DomainAdapter` + 7 adapter。
- `src/diff/engine.rs` — compare pipeline・matching・field diff・sort・cache。
- `src/lib.rs` — `pub mod diff;` + re-export。
- `tests/vcs02_diff.rs` — fixture tests。

## Tasks

1. types.rs: 全 public 型（serde derive、Ord where needed）。
2. engine.rs: matching helper・json_diff・policy・sort・equality・cache。
3. adapters.rs: per-domain adapter（field policy table 含む）。
4. lib.rs export。
5. tests/vcs02_diff.rs: fixture 構築（controller ops + 直接 Project 操作 + envelope）→ VCS-DIFF-01..08。
6. `mise run fmt` → `fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`/app-native lanes。
7. impl record + `git fetch origin main && git merge origin/main` → 最終 verify → push → PR → worker_done。

## Verification matrix

| 検証 | 手段 |
| --- | --- |
| semantic 分類 | unit/integration tests（fixture） |
| 決定性 | 同一入力 2 回 compare で `ChangeSet` 完全一致 |
| AB/BA/AA | 方向反転・AA Equal/partial |
| Unknown tri-state | missing asset/opaque/schema suppression/cross-kind collision |
| resource | budget/cancel |
| cache | `ChangeSetCache` hit で同一 `Arc`/再計算なし |

## Risk

- field policy の網羅漏れ → default `Modified` で安全側、fixture で確認。
- JSON diff の配列 wholesale → order 判定は policy が値を検査。
- 大規模 snapshot → budget で明示失敗、bounded memory。
