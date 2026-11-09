---
id: "PLAN-034Uj7LMOlnyb7SKUO8Ajd"
type: "plan"
status: "done"
title: "SF-03 slice5 実装計画 — session EnginePort・app RoutingEngine 接続・headless e2e"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
  derived-by:
    - "docs/tasks/sf-03-slice5-session-engine-port.md"
    - "docs/tasks/sf-03-slice5-session-routing-engine.md"
    - "docs/tasks/sf-03-slice5-acceptance-tests.md"
    - "docs/test-specs/sf-03-slice5-session-engine-connect.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice5-session-engine-connect.md"
  references:
    - "docs/plans/sf-03-slice4-act-acceptance.md"
  defers: []
  deferred-by: []
---

# SF-03 slice5 実装計画 — session EnginePort・app RoutingEngine 接続・headless e2e

## 対象ファイル

| 変更 | ファイル |
| --- | --- |
| 変更 | `crates/synvelis-workspace/src/session.rs` — `EnginePort` trait・`Box<dyn EnginePort>`・通知に `&Project` |
| 変更 | `crates/synvelis-workspace/src/lib.rs` — `EnginePort` re-export |
| 変更 | `crates/synvelis-workspace/tests/{ed_a,sv48_viz,ws_a}.rs` — `.graph_builds` → `.graph_builds()` |
| 変更 | `crates/synvelis-player/src/resolver.rs`・`lib.rs` — `canonical_format` probe 追加 |
| 新規 | `crates/synvelis-app/src/engine.rs` — `SessionRoutingEngine`・`SessionEngineConfig`・`SessionEngineError` |
| 変更 | `crates/synvelis-app/src/{lib,app}.rs`・`Cargo.toml` — module export・`attach_routing_engine`・deps |
| 新規 | `crates/synvelis-app/tests/sf03_session_engine.rs` — e2e |
| 新規 | docs チェーン（spec/design/plan/tasks×3/test-spec/impl） |

## 順序

1. workspace: `EnginePort` trait + `EngineStub` impl + `ProjectSession`
   の engine 差し替え・通知変更、既存 test 追従。
2. player: `resolver::canonical_format` probe（`prepare` と verified
   open を共有）。
3. app: `engine.rs`（config/error/handle/`EnginePort` impl/
   公開面）、`app.rs` の `attach_routing_engine`、`lib.rs` export、
   `Cargo.toml` deps。
4. `sf03_session_engine.rs` e2e（publish→Active→last-good→
   equivalence→stale）。
5. docs 仕上げ・impl record・index 確認。
6. `git fetch origin main && git merge origin/main` 後、
   `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
   `check-headless-deps`。

## リスク

- `deps.clips` と lowering 内 `plan_playbacks` の不整合 →
  adapter が先に `plan_playbacks` を呼び同じ `canonical_of`/engine
  config で `PreparedClip` を作る。engine 内再計算は決定的。
- `canonical_of` probe を `resolver::prepare` で代用すると prefetch
  資源を無駄に掴む → 専用 probe を追加し verified-open を共有。
- `Rc<RefCell>` handle の二重 borrow → `on_project_revision` 中に
  engine 公開面を呼ばない（単一 thread・逐次呼出し）。
- `EnginePort` 化で `engine()` の戻り型が変わり既存 caller が壊れる
  → `&dyn EnginePort` 化し accessor を trait に集約、test は
  `.graph_builds()` 追従のみ。
- app への player/media-reader/audio-core 依存追加で headless deps
  違反 → `check-headless-deps` で検証、GUI 系は import しない。

## Task Breakdown

- task 1（`sf-03-slice5-session-engine-port`）: workspace `EnginePort`・
  `ProjectSession` 接続・既存 test 追従。
- task 2（`sf-03-slice5-session-routing-engine`）: player
  `canonical_format`・app `SessionRoutingEngine`・attach 経路。
- task 3（`sf-03-slice5-acceptance-tests`）: e2e test・test-spec・
  impl record。

## Verification

- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
      `mise run test` / `mise run test-rt` /
      `mise run check-headless-deps` が全て exit 0。
- [ ] `cargo test -p synvelis-app --test sf03_session_engine` が green
      （補助証拠）。
- [ ] session.apply→publish→run→`Active` receipt、失敗→
      `Failed`+`last_good_generation`+旧 plan 継続、rename→
      `Equivalent`（generation 不変）、stale revision 拒否・
      reconcile mismatch を e2e で実測。

## Open Questions / Gaps

- `SessionEngineConfig` の既定値（sample_rate/channels/block_frames/
  budget/prefetch）は test で実測して決める。GUI/実機の config 供給は
  scope 外。
- `syn.*` service op 追加は GUI slice へ deferred（spec 参照）。

## Self-Review

- spec R1–R7 は task 1（port）・task 2（adapter+probe）・task 3
  （e2e）に写像済み。
- placeholder なし。ファイルパスは既存 crate 構成に一致。

## Review Handoff

- 前提: slice 1–4（PR #191/#192/#195/#197）が main に存在。
- `EnginePort`/`SessionRoutingEngine` の API 詳細は design の
  Components and Boundaries を正本とする。
- 未解決の残件（scope 外）: GUI/実機 engine wiring・`syn.*` op・
  SF-ACT-07/08・実機検証。
