---
id: "SPEC-sv518bC3dE5fG7hI9jK0l"
type: "spec"
status: "approved"
title: "Issue #518 spec — production job system 配線の受入仕様"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/518"
    - "docs/discovery/issue-518-job-system.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/adr/issue-518-job-system.md"
    - "docs/designs/issue-518-job-system.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-518-job-system.md"
  derived-by:
    - "docs/adr/issue-518-job-system.md"
    - "docs/designs/issue-518-job-system.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-518-job-system.md"
  references:
    - "evidence/sv518/verification.md"
  defers: []
  deferred-by: []
---

# Issue #518 spec — production job system 配線

## 目的

Issue #518 の残件 — (a) 非 RT bounded job scheduler/executor 経路、
(b) declared render preference/immutable intent の project 永続化、
(c) Recipe/scope/実測由来の cost estimate モデル、(d) gateway
`RenderDispatch` port の本番 adapter — を配線し、既存の
pol01〜pol06 trigger/pin/completeness/失敗行列契約を保存する。

## 受入条件

- AC-1: job が非同期 scheduler/executor へ投入され、RT スレッドを
  blocking しない経路が配線される（`run()`/`drive()` の同期等価物
  のみに依存しない）。pol01/pol06 の coalesce・冪等・失敗行列契約が
  非同期でもテストで pin される。
- AC-2: declared render preference/immutable intent が project
  モデルへ永続化され、Save/Reload で保持される（schema migration
  が必要ならその扱いを設計に記録）。
- AC-3: cost estimate が Recipe/scope/実測由来のモデルで生成され、
  budget/storage-quota 評価が caller-declared のみに依存しない
  （または併用境界が設計で明記される）。
- AC-4: `mise run test synvelis-render` / `mise run test
  synvelis-project`（変更が及ぶクレートの正規検証タスク）が pass
  し、`rcp03_render_policy` 等の既存 suite に回帰がない。
- AC-5: headless/fixture lane を本 Issue の Pass 判定とし、実 RT
  blocking 実測・実 provider/実 OS 受入は別 gate（#296/#294）とし
  て NotRun 記録する。

## 非目的

- 実 render executor 本体・実 RT 計測・実 OS/GUI 受入
  （#296/#294/#114 系）。
- pol01〜pol06 の trigger/pin/completeness/失敗行列契約の変更。
- `synvelis-gateway` の変更・synvelis-gateway→synvelis-render
  依存追加。
- ruviform JobExecutor 直接採用（ruviform#744 merge + pin bump が
  前提 — 本 slice は OfflineJob+bounded queue 経路）。
- Remote Render Worker（#297）・GPUI Policy UI（#295）。
- docs index（`docs/**/README.md`）の編集・再生成。

## 制約

- `JobOrchestrator` は単一 writer — scheduler は orchestrator を
  所有する 1 worker thread（`OfflineJob`）+ bounded command
  channel（`sync_channel`、満杯 → typed 応答、caller non-blocking）。
- command は demand 粒度の `drive_step` 間で interleave — cancel/
  retry は最大 1 demand render の遅延で反映される。
- cost estimate は model floor と caller-declared の要素別 max —
  caller の指定は締める方向にのみ有効で、model を下回れない。
- RenderDispatch adapter の `Dispatched`/`Pending`/`Unavailable`/
  `Failed` は determined rejection を維持 — render success を
  捏造しない。
- headless host の port 既定は fail-closed（settings probe 不在 →
  typed reject、executor 不在 → typed failure）。
- `Refs #518` のみ使用（`Closes`/`Fixes`/`Resolves` 禁止）。
