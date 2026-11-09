---
id: "SPEC-034Ux7RxHxcEmVfIrLzcfM"
type: "spec"
status: "approved"
title: "SF-03 semantic→Ruviform runtime 写像 — 世代証拠・SourceMap・失敗不変条件の統合契約"
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
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sf-03-map-current-state-inventory.md"
    - "https://github.com/scarecrow173/synvelis/issues/106"
  derived-by:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
    - "docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md"
    - "docs/plans/sf-03-map-semantic-runtime-mapping-docs.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-map-failure-invariant-verification.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/108"
    - "https://github.com/scarecrow173/synvelis/issues/101"
  defers:
    - "SF-ACT-05〜09 の実機部分（RT latency/underrun・実 device epoch restart・実 OS/実デバイス/VST3 検証）"
    - "SF-ACT-06 残件（automation→TimedParamEvent lowering 配線・Fast Control active-generation gate・GUI）"
    - "SF-ACT-07 Partial Preview vs Export fail-closed の mode 別 policy 分岐"
    - "SF-ACT-08 Ruviform RT/render/AudioLens 側の受入証拠"
    - "SF-ACT-09 device config-only change の compile 判定・性能 baseline"
  deferred-by: []
---

# SF-03 semantic→Ruviform runtime 写像 — 世代証拠・SourceMap・失敗不変条件の統合契約

## What Should Be Built

Issue #106 の semantic→Ruviform runtime 写像について、**設計文書とし
て**全受入行の不変条件・失敗写像・検証計画を確定する文書チェーン
（spec/design/ADR/plan/task/test-spec）を `docs/` に整備する。
実装は本 slice の対象外 — 既に main に統合済みの SF-03 slice1〜5
実装を「確定済み契約」として扱い、本チェーンはその上位の統合設計
権威として、consumer 横断の世代証拠契約と失敗不変条件を一箇所に
固定する。

具体的に確定する内容:

- Semantic Routing → Typed Graph Lowering → Runtime SourceMap →
  Plan Generation の責務境界（crate/型の owner 固定）。
- `Configured→Prepared→Published→Active` の世代証拠分離と、
  停止中/frames=0・nonRT publish success・後続 process callback
  の区別（TRACE-C3 の stage/freshness 契約との整合）。
- Last-good/旧 Plan 継続・Publish success≠RT 駆動の誤表示防止・
  Pending 明示の設計。
- PDC/lowering 意味論・gesture CAS・Plan publish の失敗不変条件。
- Probe/Controls/PDC/PCM render/Undo/Save が同じ
  `ProjectRevision`/plan generation を指す consumer 契約。
- SF-ACT-01/02/03/04 の各行の設計根拠、実装順序、検証計画、
  fixture 一覧（headless 優先・実 OS/device/VST3 は NotRun）。

## Why Now

SF-03 slice1〜5（PR #191/#192/#195/#197/#198）で lowering・
SourceMap・activation・fault 分類・plugin host・NoRebuild・
session 接続が実装済み。TRACE-C3（#100）・SF-04（#107）・
SF-05（#108）・PC-C6（#103）も main 統合済みで、これらは全て
`ProjectRevision`/`graph_generation`/`plan_generation` を相関
キーに使う。Issue #106 は open のまま残り、受入行の根拠を
単一の設計権威として確定しないと、後続の実機/GUI slice が
各所で独自の「active」判定を再実装する危険がある。

## Users and Value

- 後続の実装 slice（SF-ACT-05〜09 残件・実機検証・GUI 統合）
  の実装者が、失敗不変条件と世代証拠の authoritative owner を
  一箇所で参照できる。
- #109（SF-06 oracle）・#114（GUI gate）の受入担当が、headless
  で検証済みの契約と NotRun 項目を区別して評価できる。
- consumer 実装者（probe/control/UI/render）が、同一
  Revision/Generation を指す契約を破らない。

## Scope

### In Scope

- `docs/` 配下の設計文書チェーン:
  - `docs/discovery/sf-03-map-current-state-inventory.md`
  - `docs/specs/sf-03-map-semantic-runtime-contract.md`（本書）
  - `docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md`
  - `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
  - `docs/test-specs/sf-03-map-failure-invariant-verification.md`
  - `docs/plans/sf-03-map-semantic-runtime-mapping-docs.md`
  - `docs/tasks/sf-03-map-task{1,2}-*.md`
- managed index 更新（`docs/*/README.md`）。
- review-protocol による文書レビューと blocker 解消。

### Out of Scope

- `crates/` 以下のコード変更、manifest/lockfile/CI 設定。
- Issue #106 の残り実装 slice（SF-ACT-05〜09 の実機部分・
  GUI・実 OS/device/VST3 検証）。
- 既存 sf-03-slice 文書の supersede/書き換え（本チェーンは
  上位統合設計として追加し、slice 文書は実装記録として残す）。
- Ruviform 側の変更（不足は基盤 Issue 起票として記録）。

## Requirements

### MAP-R1 世代証拠の分離と Pending 明示

- `ProjectRevision`（Configured）・plan `generation`
  （Prepared/Published）・RT 観測 `active_generation`（Active）・
  probe/dispatch の observed 証拠を別々の証拠として保持し、
  相互に推論・合成しない。
- `GraphRuntime::publish` 成功は `Published` まで。`Active` は
  RT `process(frames>0)` が新世代を観測した場合のみ。停止中・
  frames=0・経過時間だけでは成功/失敗を断定しない
  （`PublishedAwaitingCallback` 相当の Pending を明示）。
- `last_good_generation` は最後に RT 観測された Active 世代で
  あり、失敗経路で後退しない。Project revision は post-commit
  の activation 失敗でも巻き戻さない。
- UI/consumer は `Configured R_new / Active R_old` を区別して
  表示できる証拠（receipt + generation）を持つ。

### MAP-R2 SourceMap の単一権威と世代スコープ

- `RoutingSourceMap` は lowering と同時に生成され
  `PreparedRouting` に同梱され、publish 時に build key/
  generation/map の内部整合（`BadSourceMap`）を検査される。
- semantic ID→runtime ID の写像は generation-scoped であり、
  plan swap/plugin reload/port reorder で失効する。Project へ
  runtime ID を永続化しない。
- probe 解決・control dispatch・PCM render binding・UI evidence
  は同じ SourceMap/generation を読み、独自の写像を持たない。

### MAP-R3 Lowering/PDC の意味論

- Semantic→Typed Graph lowering は決定的順序で、strip ごとの
  main-in→insert chain 順→fader/pan→post-out、Pre/Post send
  tap、sidechain、bus/return、mute/solo（`audible_strips`
  authority による edge prune）を保持する。
- `BuildMode::Realtime`/`Offline` は同じ `lower_semantic` を
  通り、offline は `OfflineCapabilityMismatch` gate を追加する。
  PDC の機構は Ruviform typed runtime / synvelis-render が所有し、
  lowering は PDC を直接符号化しない — lowered graph の topology
  が同一であることが RT/Offline 同意味の担保点。
- 未解決の source/port/plugin binding は `MappingResolution` の
  `Unavailable`/`Unsupported`/`NotLowered` または deterministic
  失敗であり、fake silence・silent success にしない。

### MAP-R4 Gesture CAS と speculative plan の publish 0

- gesture の speculative candidate 検証は commit 前の予備検査に
  留め、`ProjectController` CAS commit 後の committed snapshot
  から BuildKey・SourceMap/Plan を再照合または再 compile する。
- publish は live project を直前再検証し、revision/fingerprint/
  capability/epoch の不一致は `StaleProjectSnapshot`/
  `SpeculativeMismatch`/`PluginCapabilityChanged`/
  `DeviceEpochChanged` で publish 0。`reconcile` は publish 前・
  retry 前の drift 検出 gate。
- 連続編集（rapid R1/R2/R3）では未 Active の pending 世代は
  次 publish で置き換えられ、last-good は RT 観測まで後退しない。
  既 publish 世代以下の generation は `Superseded` で拒否。

### MAP-R5 Plan publish の失敗不変条件

- `RoutingFault` 18 分類（Issue #106 §5）に全失敗経路を決定的に
  写像し、未分類 fallback を作らない。
- publish QueueFull・retire overflow・generation mismatch・
  stale command/event・device restart・plugin missing/reload の
  各経路で last-good plan が維持され、old event の wrong-target
  配送が 0 である（generation-tagged dispatch/reject）。
- retry は常に fresh prepare（新 BuildKey/SourceMap/generation）
  から開始し、失敗した `PreparedRouting`・旧 generation・旧
  source map を再利用しない。

### MAP-R6 Consumer 同一 Revision 契約

- Probe（`resolve_probe_point`+`SemanticProbeBinding`）・
  Controls/ParamDispatch（世代スコープ払い出し）・PDC/render・
  PCM render（playback bindings）・Undo/Save は、同じ
  `ProjectRevision` と plan `generation` を相関キーとして指し、
  独自の「active」判定を持たない。
- TRACE-C3 `ControlTraceKey` の `graph_generation`/
  `control_plan_generation`/`clock_epoch`/`evaluation_position` と
  `ObservationFreshness` は、SF-03 の receipt/SourceMap と同一
  世代名前空間で照合され、stale 証拠は `Stale`/`Unavailable` に
  分類され誤 Live を出さない。

### MAP-R7 SF-ACT-01/02/03/04 の設計確定

- SF-ACT-01（Legacy direct→master vs 新 Routing Golden PCM/加算
  順序/Insert/Comp・SourceMap・Missing Source 明示）、
  SF-ACT-02（Pre/Post/Sidechain/Mute/Solo/Return/Bus/Processor
  順序・PDC・RT/Offline 同意味 lowering）、
  SF-ACT-03（Gesture Candidate≠Committed Snapshot・CAS stale・
  Plugin Cap drift・temporary ID mismatch で Speculative Plan
  publish 0）、
  SF-ACT-04（Configured→Prepared→Published→Active 証拠分離・
  Last-good/EngineStub false positives 無し）の各行について、
  設計文書が不変条件・失敗写像・検証 fixture を対応付ける。

### MAP-R8 実装順序・検証計画・fixture 一覧

- 残存実装の順序（headless 契約 → 実機/GUI gate）と、各段の
  検証計画・fixture 一覧を設計文書に記録する。実 OS/device/VST3・
  RT 実測・AudioLens 実機非回帰は NotRun 項目として分離する。
- Ruviform generic の不足は Synvelis 側で恒久的に重複実装せず、
  基盤 Issue 起票として記録する方針を明記する。

## Acceptance Criteria

- [ ] 設計文書として全行の不変条件・失敗写像・検証計画を記述
      （実装は対象外）。
- [ ] `docs/` に spec/design/ADR/plan/task/test-spec の文書
      チェーンが存在し、doc-status audit の blocker がない。
- [ ] SF-ACT-01/02/03/04 の各行に設計根拠・失敗写像・検証
      fixture の対応がある。
- [ ] `mise run fmt-check` と doc_status/doc_maintenance lint が
      exit 0（docs のみの変更。workspace test は任意）。
- [ ] 実 OS/device/VST3・実機検証は NotRun/Unsupported として
      正直に記録され、成功と誤表示しない。

## Deferred Design Concerns

- SF-ACT-05 の実機部分（RT latency/underrun・実 device epoch
  restart・queue-full/partial stale command の実機検証）|
  headless では検証不能 | 実機タスク着手時 | Last-good/世代契約の
  実機確認が残る。
- SF-ACT-06 残件（automation→`TimedParamEvent` lowering の engine
  配線・Fast Control の active-generation gate・GUI fast-control）
  | engine 配線は consumer 接続 slice の責務 | GUI/制御統合 slice
  着手時 | 古い generation への dispatch 誤配送リスクは
  generation-tagged 契約で headless 担保済み。
- SF-ACT-07 Partial Preview vs Export fail-closed の mode 別
  policy 分岐 | policy 未規定 | policy 決定 Issue | 現行は両 mode
  fail-closed で安全側。
- SF-ACT-08 Ruviform RT/render/AudioLens 側の受入 | upstream・
  別 consumer の責務 | 各 upstream Issue gate | 実機非回帰証拠は
  別経路。
- SF-ACT-09 device config-only change の compile 判定・性能
  baseline | fingerprint/semantic_eq の拡張は設計変更を伴う |
  性能評価 slice | 無駄 compile と必要 compile の境界が未確定。

## Sources and Relations

- Issue #106（本 spec の source）、#104/#105（SF-01/02 正本）、
  #100（TRACE-C3）、#103（PC-C6 oracle）、#107（SF-04 probe）、
  #108（SF-05 signal flow）、#101（PC-C4 plugin param binding —
  capability gate/世代スコープ control の consumer 側実装）。
- 前提文書: `docs/specs/sf-03-slice1..5`（実装済み契約）、
  `docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md`、
  `docs/specs/pc-c3-control-trace-projection.md`、
  `docs/specs/sf-04-slice{1,2}-*.md`、
  `docs/specs/sf-05-signal-flow-headless-hierarchy.md`、
  `docs/specs/pc-c6-oracle-rel-tr.md`。
- 発見文書: `docs/discovery/sf-03-map-current-state-inventory.md`。
