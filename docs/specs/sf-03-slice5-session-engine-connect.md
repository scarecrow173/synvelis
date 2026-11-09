---
id: "SPEC-034Uj7sqrin4g45fPU9uJM"
type: "spec"
status: "approved"
title: "SF-03 slice5 — app/session 層への RoutingEngine 接続（headless・EngineStub 置き換え）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/sf-03-slice5-session-engine-connect-design.md"
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
  defers: []
  deferred-by: []
---

# SF-03 slice5 — app/session 層への RoutingEngine 接続（headless・EngineStub 置き換え）

## What Should Be Built

synvelis の project/session 層が保持する engine 接続を、revision ごとに
build counter を増やすだけの `EngineStub` から、slice 1–4 で実装済みの
`RoutingEngine`（semantic lowering → typed graph compile → publish →
RT activation receipt → last-good → reconcile）へ接続する経路に置き換える。

session は committed project revision のたびに engine 境界へ
`(&Project, ProjectRevision)` を通知し、engine 側は `ProcessorHost`
注入境界を通じて lowered routing の activation / last-good 契約に乗る。
headless の app/session 経路で publish → activation receipt →
last-good 維持 → stale build 拒否が観測できること。

## Why Now

slice 1–4 で `RoutingEngine` 本体・fault 分類・plugin host 境界・
NoRebuild equivalence は完成したが、session 層は依然 `EngineStub` を
engine として保持し、app 側の `AudioRuntimeSlot` は常に
`unavailable` を返す。Issue #106 の残件である「app/session 層への
接続」を本 slice で消化し、GUI 統合 slice が乗る土台を作る。

## Users and Value

- headless app/session テスト: publish→activation→last-good→stale
  reject を session 経路で実測できる。
- 後続の GUI/実機 slice: `ProcessorHost`（`PluginHostProcessors`）を
  注入するだけで実 plugin chain を session 経路に載せられる。

## Scope

### In Scope

- `synvelis-workspace` の `ProjectSession` engine 保持を
  `EnginePort` trait 化し、`EngineStub` を既定実装として維持。
- `synvelis-app` に `SessionRoutingEngine`（`RoutingEngine` を内包し
  `EnginePort` を実装する handle）を追加し、`ProcessorHost` 注入・
  `run`/`receipt`/`reconcile`/`rt_status`/`take_processor_control`
  の session 向け公開面を提供。
- `plan_playbacks` の playback 順に `PreparedClip` を組み立てる
  `RoutingDeps` 供給と、canonical format probe（player 側の小さい
  公開 API 追加）の実装。
- headless e2e テスト: `session.apply` → publish → `run` で
  activation receipt、失敗変更で last-good 維持、semantic no-op で
  NoRebuild equivalence、revision ずれで stale 拒否。
- docs チェーン（spec/design/plan/tasks/test-spec/impl）。

### Out of Scope

- GUI 表示・`ui` 系・`synvelis-native` bin の GPUI 描画経路、
  `AudioRuntimeSlot` の実デバイス接続、実機検証。
- SF-ACT-07 preview policy・SF-ACT-08 Ruviform/AudioLens 連携。
- `RoutingEngine` 本体・`ProcessorHost`/plugin host・project 側の
  既存契約の再設計（既存 API の利用と小さい公開追加のみ）。
- view/layout/focus/transport-only 操作の engine 通知抑制は既存の
  「committed revision のみ通知」契約で担保し、session 側の
  新規抑制機構は追加しない。

## Requirements

- R1: `ProjectSession` は engine を `EnginePort` として保持し、
  `apply`/`undo`/`redo` で commit が成功した revision についてのみ
  `(&Project, revision)` を通知する。失敗した command・view/layout
  系の変更では通知しない。
- R2: `EngineStub` は `EnginePort` の既定実装として残り、既存の
  `graph_builds`/`last_built_revision` 観測契約を維持する。
  `EngineStub` を authoritative engine とする経路を残さない
  （注入可能だが app の canonical session では `SessionRoutingEngine`
  を接続する）。
- R3: `SessionRoutingEngine` は revision 通知ごとに
  `RoutingCompileRequest`（request id・expected revision・mode・
  budget・instance/epoch）と `RoutingDeps`（`canonical_of`・
  `plan_playbacks` 順の `PreparedClip`・generation）を組み立て、
  `RoutingEngine::prepare` → `publish` を駆動する。`Equivalent`
  時は publish しない。
- R4: engine 側の失敗（prepare/publish fault）は `RoutingEngine`
  の receipt/last-good に記録され、session の project commit を
  巻き戻さない。adapter 側の clip 解決失敗は adapter の
  `last_prepare_error` として記録し、engine の live plan を
  置き換えない。
- R5: `ProcessorHost` は `SessionRoutingEngine::set_processor_host`
  で注入し、generation-scoped lease/teardown・control 払い出しは
  engine 既存契約に委譲する。headless 既定は `StaticProcessorHost`。
- R6: activation（`Active` receipt）は RT `run` が generation を
  observe した場合のみ。publish 成功を activation と見なさない。
- R7: app の既定/headless 経路は GUI 依存を引き込まない
  （`check-headless-deps` green）。`SessionRoutingEngine` は
  `synvelis-app` の非 feature モジュールとする。

## Acceptance Criteria

- [ ] `session.apply` 経由の project 変更で `RoutingEngine` の
  prepare→publish が走り、`run` 後に `Active` receipt と
  last-good generation が観測できる。
- [ ] 失敗する変更（例: unresolved processor binding）で receipt が
  `Failed`+`last_good_generation` を示し、直前の live plan が
  `run` で処理を継続する。
- [ ] semantic fingerprint が同値の変更（project rename 等）で
  `RoutingPrepare::Equivalent` となり generation/compile が
  発生せず receipt が維持される。
- [ ] `session.apply` の stale revision 拒否・controller 直接変更を
  経た revision ずれが engine 側 stale 判定（reconcile の
  `StaleProjectSnapshot`/`ProjectRevision` mismatch）として
  観測できる。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-headless-deps` が全て exit 0。

## Deferred Design Concerns

- GUI/実機での engine config・media_root 決定と
  `AudioRuntimeSlot` への実接続 | GUI 統合 slice の責務 | 実機
  検証タスク着手時 | session 接続があっても実デバイス再生は
  別経路で実装が必要。
- app service (`syn.*`) への routing 操作 op 追加 | headless e2e は
  session 直結で十分、op 面は GUI slice で設計 | GUI slice 着手時 |
  service 経由の activation 観測が後続で必要。
- project 切替時の engine 世代/last-good の扱い | session ごとに
  engine を再 attach する運用で担保 | project open/new の engine
  wiring slice | 世代衝突や last-good 漏洩の可能性。

## Sources and Relations

- Issue #106 SF-03 slice 5 dispatch（app/session 層への
  RoutingEngine 接続、headless・GUI 非対象）。
- slice 1–4 の spec/design/plan/test-spec（semantic lowering・
  fault taxonomy・plugin host・NoRebuild equivalence）。
- `RoutingEngine`/`ProcessorHost`/`plan_playbacks`/`resolver` の
  実装（`crates/synvelis-player`）。
