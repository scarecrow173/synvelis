---
id: "IMPL-285CB08cmpOrch0001"
type: "impl"
status: "completed"
title: "CB-08 Comparison Orchestrator headless slice — pinned session・layered result・staleness・derived cache (synvelis#285)"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/285"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/variant/compare.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb08_compare_orchestrator.rs"
      - type: "file"
        path: "docs/test-specs/cb-08-compare-orchestrator.md"
      - type: "file"
        path: "docs/impl/ir/cb-08-compare-orchestrator.md"
      - type: "file"
        path: "docs/impl/exp/cb-08-compare-orchestrator.md"
      - type: "file"
        path: "evidence/sv285/coverage.md"
      - type: "file"
        path: "evidence/sv285/external-gates.md"
      - type: "file"
        path: "evidence/sv285/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/variant/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/src/diff/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/src/snapshot/mod.rs"
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
    - "docs/test-specs/cb-08-compare-orchestrator.md"
  implemented-by: []
  depends-on:
    - "docs/specs/creative-branch.md"
    - "docs/designs/creative-branch-design.md"
    - "docs/test-specs/cb-01-variant-set.md"
    - "docs/test-specs/cb-02-audio-clip-adapter.md"
    - "docs/test-specs/cb-03-midi-clip-adapter.md"
    - "docs/test-specs/cb-04-instrument-adapter.md"
    - "docs/test-specs/cb-05-fx-range-adapter.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/test-specs/cb-07-arrangement-adapter.md"
    - "docs/test-specs/vcs-02-semantic-diff-fixtures.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/cb-08-compare-orchestrator.md"
    - "evidence/sv285/coverage.md"
    - "evidence/sv285/external-gates.md"
    - "evidence/sv285/verification.md"
    - "docs/test-specs/cb-09-diff-explorer.md"
    - "docs/test-specs/cb-11-cross-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/creative-branch-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/cb-08-compare-orchestrator.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/cb-08-compare-orchestrator.md"
    rejected: []
---

# CB-08 Comparison Orchestrator headless slice 実装記録

## Summary

Issue #285 の headless slice。`synvelis-project` に
`ComparisonOrchestrator` を新設し、pinned `BranchRevisionRef`
endpoint・固定 `EvaluationContext`・Semantic/Signal/Perceptual
3 層の layered `ComparisonResult`・typed `StaleDimension`
分類・bounded derived `ChangeSetCache` を実装した。
`synvelis-project` の公開 API のみを使う統合 test
`cb08_compare_orchestrator.rs`（13 test）で CB-CMP-01..03 の
headless 契約と 7 scope 全件の pin/diff/staleness/cache 挙動を
検証した。実 signal render・PCM delta・試聴/知覚受入・外部
gate は本 slice の対象外であり、`evidence/sv285/
external-gates.md` で NotRun/Deferred/Blocked として記録する。

## Context

Issue #285 は CB-08「Comparison Orchestrator —
Semantic/Signal/Perceptual Diff・Context 固定・Cache」。
design §7.1 は 3 層を別データとし相互代替を禁じ、pin された
context のいかなる構成要素の変化も evidence を stale に
すると規定する。CB-09（Difference Explorer）・CB-10
（action/CLI/Gateway surface）・CB-11（cross-gate）は本
orchestrator の consumer であり、本 slice は headless の
orchestration 契約のみを所有する — 実 renderer/probe・PCM・
A/B 試聴・plugin GUI・consumer 受入は別 Issue/gate の責務。

## Implementation

### Changes

- `crates/synvelis-project/src/variant/compare.rs`（新規）:
  - `BranchRevisionRef` — variant set/branch/revision/canonical
    scope digest を caller が assert する pin。`open_session`
    で fail-closed 検証（`CompareError::StaleRevision`/
    `StaleScope`/`UnknownSet`/`UnknownBranch`/`ScopeMismatch`）。
  - `EvaluationContext` — project revision・両側
    `BranchRenderPin`・tempo/time-mapping/plugin/source/
    remainder の digest bundle。`freshness` が各構成要素を
    現 authority と再計算比較し `StaleDimension` へ分類。
  - `ComparisonSession` — pin・context・宣言 policy
    （`SessionOptions`: domains/budget/`AlignmentPolicy`/
    `MonitoringPolicy`）・両側 composed view の immutable
    snapshot を保持する read-only・non-persistent derived
    state。
  - `ComparisonOrchestrator` — `pin_branch`/`open_session`/
    `freshness`/`evaluate`。両 endpoint を
    `resolve_render_project` で composed view へ解決し、
    `SnapshotResolver` で pin した view を capture、
    `ChangeSetCache` 経由で semantic `ChangeSet` を生成。
  - `ComparisonResult` — `freshness` + `SemanticLayerResult`
    （`ChangeSet`・`CacheStatus`・`ComparisonKey` digest）+
    `SignalLayerResult`（provider seam・origin/alignment/
    metrics/provenance/stale）+ `PerceptualLayerResult`
    （常に `Unavailable{RequiresAudition}`）。
  - `SignalComparisonProvider` seam — 外部 renderer/probe
    実装が差し込む契約。orchestrator は report を gate し、
    plugin opaque state 非 `Known`・非有限 metric・
    `Unaligned` policy を typed gap へ降格する。
- `crates/synvelis-project/src/snapshot/mod.rs`:
  `SnapshotResolver::capture_project` を追加 — composed
  `Project` 値（branch render view）を controller 無しで
  `capture_live` と同一の envelope/digest/manifest 経路で
  capture する公開 API。
- `crates/synvelis-project/src/diff/engine.rs`:
  `ChangeSetCache::contains` を追加 — hit/miss を
  non-mutating・LRU 非更新で観測する probe（key digest は
  `get_or_compare` と同一規則）。
- `crates/synvelis-project/src/variant/mod.rs`:
  `pub mod compare` 登録と公開型の re-export。
- `crates/synvelis-project/tests/cb08_compare_orchestrator.rs`
  （新規・13 test）: 公開 API のみで 7 scope fixture・
  pin/CAS/stale/cache/layer honesty/provider gate/read-only/
  adopt 後 Live endpoint を検証。
- `docs/test-specs/cb-08-compare-orchestrator.md`・
  `docs/impl/{ir,exp}/cb-08-compare-orchestrator.md`・
  `evidence/sv285/{coverage,external-gates,verification}.md`
  （新規）: test-spec・実装記録・実験メモ・evidence ledger。

### Decisions During Implementation

- `EvaluationContext` の shared-context digest を
  `BranchRenderPin.shared_context_digest` とは別に、tempo
  （`Project` singleton）・time mapping（`ClockEpoch`）・
  plugin（`PluginInstance`）・source（`Asset`/`MediaSource`/
  `MidiSource`/`MidiNote`/`SmfDocument`）・remainder
  （その他全 shared object）の 5 digest へ分解し、
  `StaleDimension` が次元別に分類できるようにした。
  `remainder_digest` の導入により、4 named 次元のいずれにも
  該当しない shared drift のみが `SharedContext` を発火する
  （分類 bug の修正 — 初版は最終 stale reason のみを見て
  誤分類し得た）。
- Signal 層は実 renderer を持たないため provider seam のみ。
  provider report は「主張」として扱い、plugin-state gate・
  metric 有限性 gate を orchestrator 側で再検証する —
  `LayerState::Available` を provider が申告しても opaque
  plugin state があれば `Unavailable{PluginStateUnknown}` に
  降格する。
- `PerceptualLayerResult` は設計上 interactive audition のみ
  が根拠を持つため、headless では state を
  `Unavailable{RequiresAudition}` に固定し、stale flag のみ
  が評価ごとに変わる。
- semantic 層の cache は `ChangeSetCache`（bounded LRU・
  `ComparisonKey` digest）を再利用し、hit/miss 観測用に
  `contains` probe を追加した — 新規 cache 機構は作らない。
- `capture_project` は composed view を live controller 無しで
  snapshot 化する最小の公開経路。`capture_live` の
  stale-revision 検査のみを除き、同一の digest/manifest
  経路を通す。

## Related Experiments

### Adopted

- `docs/impl/exp/cb-08-compare-orchestrator.md` E1:
  shared-context digest の次元分解（remainder 導入）—
  `SharedContext` の誤分類を防ぐ採用案。
- 同 E2: `ChangeSetCache::contains` probe 追加 —
  hit/miss 観測を LRU 非破壊で行う採用案。
- 同 E3: signal provider seam の gate 配置（orchestrator 側
  再検証）— provider 申告を信用しない採用案。

### Rejected

<!-- なし。 -->

## Validation

- `mise run fmt-check`: Pass
- `mise run check synvelis-project`: Pass
- `mise run lint synvelis-project`: Pass
- `mise run test synvelis-project`: Pass（cb08 13 test 含む
  全 target green）
- `mise run test-rt`: 実行結果は `evidence/sv285/verification.md`
  に記録
- `mise run check-headless-deps`: 実行結果は
  `evidence/sv285/verification.md` に記録
- `mise exec -- prek run --all-files`: 実行結果は
  `evidence/sv285/verification.md` に記録

## Risks

- headless green は orchestration 契約（pin・context 固定・
  layer 分離・staleness 分類・derived cache）のみを証明する。
  実 signal render・PCM delta・試聴/知覚受入・plugin GUI・
  consumer 受入は別 gate の責務であり、本 slice の Pass を
  それらへ換算しない。
- `SignalComparisonProvider` の実装は本 slice に存在しない
  — signal 層は常に `Unavailable{NoProvider}`（または gate
  された provider report）であり、実測 evidence の供給は
  CB-09/CB-10・RCP 系の前提 merge 後の責務。
- `MonitoringPolicy` は audition path の宣言 policy であり、
  headless では比較 signal content を変更しない — 実 monitor
  gain の聴取検証は外部 gate。

## Follow-ups

- CB-09 Difference Explorer（#286）・CB-10 action/CLI/
  Gateway surface（#287）・CB-11 cross-gate（#288）からの
  consumer 接続 — `ComparisonSession`/`ComparisonResult` は
  非 persistent derived state として設計済み。
- 実 signal/PCM/probe provider（Ruviform render/probe・
  #308/#309 系）・試聴 A/B・loudness-match 非破壊性の
  実機/GUI gate — `evidence/sv285/external-gates.md` 参照。
- `AlignmentPolicy::Mapped` の実 time-mapping provider と
  PDC/latency 分類の実測対応 — headless は policy 宣言と
  refusal のみを pin。
