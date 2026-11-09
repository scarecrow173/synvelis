---
id: "TSPEC-034Um6SOlStZLqEVmi7IEk"
type: "test-spec"
status: "approved"
title: "SF-04 slice1 受入テスト仕様 — semantic probe 解決・lifecycle・segment cut"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source: []
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
    - "docs/designs/sf-04-slice1-semantic-probe-adapter-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/designs/sf-04-slice1-semantic-probe-adapter-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice1 受入テスト仕様 — semantic probe 解決・lifecycle・segment cut

## Purpose

SF-04 slice1（Issue #107 の project/player 基盤）が、semantic probe point を
fail-closed に解決し、requested/queued と RT-installed を区別し、generation
境界で capture を切断することを保証する受入仕様。UI/実測 slice でも同じ
契約を検証に使う。

## Feature

Semantic probe point の解決と binding lifecycle（headless）。

## Rules

- Rule: `SemanticProbePoint` は entity + `SignalBoundary` + optional port で
  一意に表され、永続化してよいのは semantic 意図のみ。runtime `ProbeId` は
  binding の runtime 状態にのみ存在する。
- Rule: 解決は active `RoutingSourceMap` + published `GraphSpec` に対して
  行われ、物理 slot が無い位置は `Unavailable`/`Unsupported`/`Unresolved`
  を返す。PCM・ゼロ値の代替観測を生成しない。
- Rule: `Off`/`Monitor`/`Capture` の要求は `Requested`→`QueuedForBinding`→
  （RT status）`BoundActive` の順を踏み、RT status 無しに installed と
  みなさない。generation 不一致の status は `StaleGeneration`。
- Rule: generation 交代（publish）で in-flight capture は
  `RebindReceipt.capture_stopped` + `CaptureStopped{GenerationBoundary}`
  status により segment が切断され、新 generation へ自動継続しない。
- Rule: probe mode の既定は `Off`（binding 未作成）。

## Examples

- Example: strip `Output`/`PostFader` boundary は fader post-out port に、
  `PreFader` は pre_out port に、`Input`/`PreInsert` は entry（chain head
  input または fader in）に解決される。strip `Sidechain` は `Unsupported`。
- Example: 存在しない entity・不整合な明示 port は `Unresolved`。
- Example: mute/disabled で lowering が `NotLowered` を記録した port は
  `Unavailable`。
- Example: install 後・RT drain 前の binding は `QueuedForBinding`。
  `Installed` status（generation 一致）drain 後は `BoundActive`。
- Example: capture 中に publish → segment が generation 境界で閉じ、
  `segments` に 1 件の完了 segment が残り、自動で新 segment は始まらない。
- Example: `remove` → binding `Stopped`、RT `Removed` status で確定。

## Guarantees

- `resolve_probe_point` は pure で、同一 `(project, map, spec, point)` に
  同一結果を返す。
- `ProbeSession::request` は同一 `ProbeSubscriptionKey` に冪等。
- drain された status の generation が binding の
  `active_plan_generation` と一致しない場合 `BoundActive` にしない。
- capture segment は RT status（`CaptureStopped`/`RebindDropped`/`Removed`）
  でのみ close され、frame_range/discontinuities/completeness が記録される。

## Non-goals

- UI・複数 window ownership・実デバイス計測・clock/format 比較診断・
  capture PCM の永続化/retention。
- ruviform 側の Probe/Broker/Rebind/Clock 機構自体の再検証（ruviform の
  test suite が担保）。

- What is lost or breaks if these guarantees are dropped: semantic point が
  実在しない slot を指したまま「観測中」を装う、旧 generation の status を
  新 generation の証拠に見せる、graph swap で capture が黙って継続する —
  いずれも SF-PROBE-01/03/04 の直接違反。
