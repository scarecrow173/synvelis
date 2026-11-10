---
id: "PLAN-034VoSKvu6PtN0qTGN02l3"
type: "plan"
status: "completed"
title: "SF-03 slice8 — app/workspace same-Revision fixture plan"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice8-revision-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-03-slice8-revision-fixture-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice8-revision-fixture.md"
    - "docs/designs/sf-03-slice8-revision-fixture-design.md"
  derived-by:
    - "docs/tasks/sf-03-task1-revision-fixture.md"
    - "docs/test-specs/sf-03-slice8-revision-fixture.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice8-revision-fixture.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice8 — app/workspace same-Revision fixture plan

## 対象ファイル

| 変更 | ファイル |
| --- | --- |
| 新規 | `crates/synvelis-app/tests/sf03_revision.rs` — same-Revision・viewstate/no-op・bypass drift・save≠activation の 4 fixture |
| 新規 | `crates/synvelis-workspace/tests/sf03_revision.rs` — EnginePort/投影/evidence 同一 revision・transition stale・save revision 軸の 3 fixture |
| 新規 | docs チェーン（spec/design/plan/task/test-spec/impl） |
| 再生成 | `docs/{specs,designs,plans,tasks,test-specs}/README.md`（managed index）・`docs/impl/ir/README.md`（手動追記） |

## 順序

1. app fixture: `sf03_session_engine.rs`/`sf05_signal_flow.rs`/
   `sf04_probe_wiring.rs` の helper パターンで
   `app_with_engine`（fixture WAV・track strip・clip・chain
   processor・master 接続）を構築し、4 test を実装。
2. workspace fixture: `sf_05.rs` の session/routing helper で
   fixture を構築し、`evidence_at` で合成
   `SignalFlowRuntimeEvidence` を供給する 3 test を実装。
3. 各 test を補助証拠として個別実行し、assert を実測に合わせる
   （production 変更はしない — API 不足があれば ask）。
4. docs 仕上げ・impl record・index 確認。
5. `git fetch origin main && git merge origin/main` 後、
   `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
   `test-msrv`/`check-headless-deps`。

## リスク

- `signal_flow_view` の node `Resolved` は lowering が実際に
  semantic entity を lowering した場合のみ — fixture は
  strip/processor/master に限定し、playback/未 lowering entity
  は assert しない。
- semantic no-op 後の `stale = true` は現行 projection 仕様
  （`source_map_revision != live`）の固定であり、equivalence を
  新規証拠と見なす変更ではない。
- 合成 `RoutingSourceMap` は `NodeId` を名指しできない
  （workspace の依存に `ruviform-graph` が無い）ため
  `Unavailable`/`Unobserved` resolution のみ — `Resolved` の
  node 単位照合は app fixture の実 engine map が担保する。

## Task Breakdown

- task 1（`sf-03-task1-revision-fixture`）: app/workspace 両
  fixture・test-spec・impl record。

## Verification

- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
      `mise run test` / `mise run test-rt` / `mise run test-msrv` /
      `mise run check-headless-deps` が全て exit 0。
- [ ] `cargo test -p synvelis-app --test sf03_revision`・
      `cargo test -p synvelis-workspace --test sf03_revision` が
      green（補助証拠）。
- [ ] receipt/source_map/probe_binding/signal_flow_view が同一
      committed revision を指す・viewstate-only change が
      NoRebuild(Equivalent)・controller bypass→reconcile drift・
      save≠activation を headless に固定。

## Open Questions / Gaps

- probe の RT-confirmed mode・`BoundActive` への実機昇格は
  headless 範囲外（requested/pending の正直な投影までを固定）。
- SF-ACT-07/08/09・GUI gate・SF03-RCP-01/02 は本 slice 対象外。

## Self-Review

- spec Requirements は app 4 fixture + workspace 3 fixture に
  写像済み。placeholder なし。

## Review Handoff

- 前提: slice5–7（session engine・control・render 消費）が main
  に存在。公開面は全て既存 API。
- 未解決の残件（scope 外）: GUI/実機 wiring・SF-ACT-07/08/09・
  probe RT-confirmed 実機証拠。
