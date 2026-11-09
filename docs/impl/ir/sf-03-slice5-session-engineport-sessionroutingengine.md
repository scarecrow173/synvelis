---
id: "IMPL-034UjyiVpFVP7RCwlZ2neB"
type: "impl"
status: "completed"
title: "SF-03 slice5 実装記録 — session EnginePort・SessionRoutingEngine 接続"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/src/engine.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/sf03_session_engine.rs"
      - type: "file"
        path: "docs/specs/sf-03-slice5-session-engine-connect.md"
      - type: "file"
        path: "docs/designs/sf-03-slice5-session-engine-connect-design.md"
      - type: "file"
        path: "docs/plans/sf-03-slice5-session-engine-connect.md"
      - type: "file"
        path: "docs/test-specs/sf-03-slice5-session-engine-connect.md"
      - type: "file"
        path: "docs/tasks/sf-03-slice5-session-engine-port.md"
      - type: "file"
        path: "docs/tasks/sf-03-slice5-session-routing-engine.md"
      - type: "file"
        path: "docs/tasks/sf-03-slice5-acceptance-tests.md"
      - type: "file"
        path: "docs/impl/ir/sf-03-slice5-session-engineport-sessionroutingengine.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/session.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/ed_a.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/sv48_viz.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/ws_a.rs"
      - type: "file"
        path: "crates/synvelis-player/src/resolver.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
    - "docs/tasks/sf-03-slice5-acceptance-tests.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-03-slice5-session-engine-connect.md"
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sf-03-slice5-session-engine-connect.md"
  verified-by: []
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-03 slice5 実装記録 — session EnginePort・SessionRoutingEngine 接続

## Summary

`ProjectSession` の engine 保持を `EngineStub` から `EnginePort`
trait 化し、`synvelis-app` に `RoutingEngine` を内包する
`SessionRoutingEngine`（cloneable `Rc<RefCell>` handle）を実装して
session→routing の headless 接続経路を完成させた。player には
`resolver::canonical_format` probe を追加し、`prepare` と同一の
verified-open を共有する。e2e
（`crates/synvelis-app/tests/sf03_session_engine.rs`、5 件）で
publish→`Active` receipt→last-good 維持→NoRebuild equivalence→
stale 拒否/reconcile drift を session 経路で実測した。

## Context

Issue #106 SF-03 slice 5。slice 1–4（PR #191/#192/#195/#197）で
`RoutingEngine` 本体・fault 分類・`ProcessorHost` 境界・
NoRebuild equivalence が完成済みで、本 slice は app/session 層への
接続のみ。spec `sf-03-slice5-session-engine-connect.md`・design・
plan・tasks・test-spec に従う。

## Implementation

### Changes

- `crates/synvelis-workspace/src/session.rs`: `EnginePort` trait
  （`on_project_revision(&Project, ProjectRevision)`・
  `graph_builds`・`last_built_revision`）を追加。`EngineStub` は
  既定実装として trait を実装し pub field 観測を維持。
  `ProjectSession.engine` を `Box<dyn EnginePort>` 化、
  `with_engine`/`set_engine` で注入可能に。`apply_internal`/`undo`/
  `redo` は commit 成功後に `controller.live()` と revision を渡す。
- `crates/synvelis-workspace/src/lib.rs`: `EnginePort` re-export。
- workspace tests（`ed_a`/`sv48_viz`/`ws_a`）: `.graph_builds`
  field → `.graph_builds()` メソッドへ機械的追従。
- `crates/synvelis-player/src/resolver.rs`: verified-open を
  `open_verified` に抽出し `prepare`/`canonical_format` で共有。
  `canonical_format` は prefetch 資源を取らない format probe。
- `crates/synvelis-player/src/lib.rs`: `canonical_format` re-export
  追加は不要（`resolver` は pub mod のため module path で到達可）—
  実際には `pub use resolver::PreparedClip` のみで変更なし。
- `crates/synvelis-app/src/engine.rs`（新規）: `SessionEngineConfig`・
  `SessionEngineError`・`SessionRoutingEngine`。revision 通知ごとに
  per-build `canonical_of` cache + `plan_playbacks` →
  `resolver::prepare`（plan 順 `PreparedClip`）→
  `RoutingCompileRequest`/`RoutingDeps` → `prepare`→`publish`。
  `Equivalent`/`Err` は engine receipt に委譲。adapter 側 clip
  解決失敗は `last_prepare_error` に記録し engine untouched。
- `crates/synvelis-app/src/app.rs`: `attach_routing_engine` で
  現在 session に engine を接続する最小 composition 経路。
- `crates/synvelis-app/src/lib.rs`・`Cargo.toml`: `pub mod engine`・
  re-export・`synvelis-player`/`ruviform-audio`/
  `ruviform-audio-core`/`ruviform-media-reader` 依存追加。
- `Cargo.lock`: 依存追加に伴う更新（新規外部 package なし、
  既 lock 内の ruviform git 依存のみ）。

### Decisions During Implementation

- engine 実装の置き場所: `synvelis-workspace`→`synvelis-player` の
  依存を避けるため trait は workspace、`RoutingEngine` 内包実装は
  composition root の `synvelis-app` に配置（design Approaches B）。
- `SessionRoutingEngine` を `Rc<RefCell<Inner>>` の cloneable
  handle とし、session（`Box<dyn EnginePort>`）と test が同一
  engine を観測できるようにした。`!Send` は session の単一
  thread 前提と整合。
- `source_format_generation` は「その build が consult した
  canonical format map」の差分で +1 — key 契約に忠実で、未参照
  source の変化は build key に影響しない。
- clip 解決失敗（probe 成功後の `resolver::prepare` 失敗）は
  `deps.clips` を完成できないため engine を呼ばず
  `last_prepare_error` に記録 — live plan/receipt を維持。
  この経路は `Prefetch::manual` 失敗時のみ到達し得る防御的経路で、
  e2e では `MissingSource`（blob 削除→probe 失敗→skip→engine
  `Failed`）で last-good を実測した。

## Related Experiments

なし。

## Validation

- `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` / `mise run test-rt` /
  `mise run check-headless-deps`: 全て exit 0（Windows、
  toolchain 1.99.0）。
- `cargo test -p synvelis-app --test sf03_session_engine`:
  5/5 pass（補助証拠、canonical は `mise run test`）。
- e2e 実測: `session.apply`→`Published`→`run`→`Active`+
  `last_good_generation`、unresolved binding→`Failed`+
  `PluginUnavailable`+last-good 継続、rename→`Equivalent`
  （generation/receipt 不変・`graph_builds` は増加）、stale
  revision→controller 拒否で `graph_builds` 不変、controller
  bypass→reconcile `ProjectRevision` mismatch→次 apply で
  republish→`Active` 復帰、blob 削除→`Failed`+`MissingSource`+
  last-good。

## Risks

- `Equivalent` 判定は engine 内で行われるため、adapter は
  equivalence 可能性があっても `plan_playbacks`+`resolver::prepare`
  を実行してから `Equivalent` を受け取る — no-op revision ごとに
  media resolve+prefetch 確保のコストがかかる（非 RT・破棄は
  clean）。engine 側に equivalence 事前 probe API を置く最適化は
  follow-up 候補。
- `Rc<RefCell>` handle は `!Send` — GUI/実機で engine を別 thread
  に置く場合は構造変更が必要（現状 session は単一 thread）。
- `SessionEngineConfig` の既定値は test 実測値 — GUI/実機の
  config 供給規則は scope 外。

## Follow-ups

- GUI/実機 slice: media_root/engine config の project 単位決定、
  `AudioRuntimeSlot` への実接続、`syn.*` op 面、実機検証。
- SF-ACT-07 preview policy・SF-ACT-08 Ruviform/AudioLens 連携。
- `Equivalent` 時の clip prepare スキップ最適化（engine 側
  equivalence probe 公開の検討）。
- `PluginHostProcessors` を session 経路へ注入する結合
  （`set_processor_host` は公開済み、実 plugin chain の session
  e2e は GUI/実機 slice で）。
