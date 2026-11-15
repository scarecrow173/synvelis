---
id: "TSPEC-285CB08cmpOrch0001"
type: "test-spec"
status: "draft"
title: "CB-08 Comparison Orchestrator — pinned context・layered evidence・staleness・derived cache の headless 契約"
created: "2026-11-16"
updated: "2026-11-16"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/285"
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
  related:
    - "evidence/sv285/coverage.md"
    - "evidence/sv285/external-gates.md"
    - "evidence/sv285/verification.md"
    - "docs/impl/ir/cb-08-compare-orchestrator.md"
    - "docs/impl/exp/cb-08-compare-orchestrator.md"
    - "crates/synvelis-project/tests/cb08_compare_orchestrator.rs"
    - "docs/test-specs/cb-01-variant-set.md"
    - "docs/test-specs/cb-02-audio-clip-adapter.md"
    - "docs/test-specs/cb-03-midi-clip-adapter.md"
    - "docs/test-specs/cb-04-instrument-adapter.md"
    - "docs/test-specs/cb-05-fx-range-adapter.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/test-specs/cb-07-arrangement-adapter.md"
    - "docs/test-specs/vcs-02-semantic-diff-fixtures.md"
    - "docs/test-specs/cb-09-diff-explorer.md"
    - "docs/test-specs/cb-11-cross-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/creative-branch-design.md"
  derived-by: []
  verifies:
    - "docs/specs/creative-branch.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# CB-08 Comparison Orchestrator — pinned context・layered evidence・staleness・derived cache の headless 契約

## Purpose

Issue #285（CB-08 Comparison Orchestrator）の headless slice が
固定すべき契約を定義する。`ComparisonOrchestrator` は read-only・
non-persistent な `ComparisonSession` を開き、両 branch endpoint
（`BranchRevisionRef`）と共有評価 context（`EvaluationContext`）を
pin した上で、Semantic / Signal / Perceptual の 3 層を**別データ**
として返す。未対応・利用不可・陳腐化・unknown な evidence を
equality や success に見せかけないこと、derived cache が project
truth を装わないこと、context が動けば旧 evidence が current を
自称しないことがこの spec の中核である。

headless Pass は実 signal render・PCM delta・試聴/知覚受入・
外部 gate 完了を意味しない — それらは `evidence/sv285/
external-gates.md` で NotRun/Deferred/Blocked として記録する。

## Feature

Creative Branch Comparison Orchestrator の pinned session・
layered result・typed staleness・bounded derived cache の
headless 契約

## Rules

- Rule: `open_session` は left/right の `BranchRevisionRef`
  （variant set・branch・branch revision・canonical scope digest）
  を CAS 検証し、異なる variant set の endpoint 組合せ・未知
  branch・revision/digest 不一致を typed error
  （`CompareError`）で fail-closed に拒否する。比較は
  `Project`・branch content・controller revision・undo/redo を
  一切変更しない。
- Rule: `EvaluationContext` は session open 時に固定され、
  live project revision・両側 `BranchRenderPin`・tempo/ppqn・
  clock-epoch time mapping・plugin-instance・source material・
  remainder の各 digest を保持する。`freshness` は pinned
  context と現 authority を再計算比較し、drift を
  `StaleDimension`（`ProjectRevision`/`BranchRevision`/
  `ScopeContent`/`BranchPayload`/`Tempo`/`TimeMapping`/
  `Plugin`/`Source`/`SharedContext`）として分類して返す —
  opaque な「何か変わった」ではなく次元別に報告する。
- Rule: `ComparisonResult` の 3 層は独立である。
  Semantic 層は pin された 2 つの composed view 上の
  `ChangeSet` で常に生成され、`overall`/`completeness`/
  `unresolved` が不確実性を内包する。Signal 層は provider seam
  （`SignalComparisonProvider`）経由でのみ evidence を取り、
  provider 非供給・`Unaligned` policy・plugin opaque state
  非 `Known`・非有限 metric を typed gap
  （`Unavailable`/`Unknown`）へ落とす — fabricated PCM・
  measured-exact の主張をしない。Perceptual 層は headless では
  常に `Unavailable{RequiresAudition}` であり、試聴受入を
  捏造しない。
- Rule: signal evidence の `origin` は `Pipeline`/`Derived`/
  `Mock` を区別し、`Mock` provider 出力が実 pipeline evidence
  と混同されない。stale context 下の aux evidence は
  `stale: true` を立て、pin-scoped であって current ではない
  ことを示す — semantic 層の pin-scoped 結果は stale でも
  消さないが「現在の測定」とは表示しない。
- Rule: semantic `ChangeSet` は bounded `ChangeSetCache`
  （`ComparisonKey` digest 駆動 LRU）を経由し、cache hit/miss
  を `CacheStatus` で観測可能にする。cache は derived であり
  project/media truth ではない — いかなる content/context
  変化も key digest を変え、偽の再利用を起こさない。
- Rule: 全 7 `VariantScope`（`AudioClip`/`TakeUsage`/
  `MidiClip`/`InstrumentSlot`/`ProcessingRange`/
  `StripFields`/`Arrangement`）で session が開き、pinned
  revision・固定 context・semantic diff が返る。scope が
  support しない evidence は equality ではなく typed reason
  で返す。
- Rule: 新規 CI workflow を作らない。検証は既存 `mise.toml`
  task に委譲し、実 signal・PCM・試聴・plugin GUI・consumer
  gate は `evidence/sv285/external-gates.md` で
  NotRun/Deferred/Blocked として記録する。

## Examples

- Example: `gate_fixture` の 6 scope（`AudioClip`・`MidiClip`・
  `InstrumentSlot`・`ProcessingRange`・`StripFields`・
  `Arrangement`）と take fixture の `TakeUsage` で、fork 直後の
  adopted/candidate を pin して `open_session` すると、
  `session.left`/`session.right` が pin した revision・set・
  branch を保持し、`pinned_context` が project revision・
  両側 render pin・shared-context digest を固定する
  （`cb08_cmp_01_six_gate_scopes_pin_context_and_semantic_diff`・
  `cb08_cmp_01_take_usage_scope_pins_and_clean_equal`）。
  candidate を scope 内 edit すると新 session の semantic
  `overall` が `Different` になり、diff に raw-material kind
  （Asset/MediaSource/Take/Recording*/ClockEpoch/SmfDocument/
  AppliedCommit/AppliedImport）が出ない。
- Example: session open 後に candidate branch を edit すると
  `freshness` が `Stale` を返し、`BranchRevision{Right}`・
  `ScopeContent{Right}`・`BranchPayload{Right}`・
  `ProjectRevision` のみが理由に列挙され、shared-context 系
  dimension は列挙されない。live の scope 外 edit
  （strip fader target）は `SharedContext` のみ、tempo event
  追加は `Tempo`、asset 登録は `Source`、processor 挿入は
  `Plugin`+`SharedContext` を動かす
  （`cb08_cmp_02_branch_edit_marks_session_and_result_stale`・
  `cb08_cmp_02_shared_context_edit_marks_stale_without_scope_move`・
  `cb08_cmp_02_tempo_and_source_moves_classify_independently`・
  `cb08_cmp_02_plugin_insert_marks_plugin_dimension_stale`）。
- Example: stale 化した session を `evaluate` すると semantic
  層は pin-scoped `ChangeSet` を返しつつ `freshness`/`signal.
  stale`/`perceptual.stale` が立ち、provider なしでは signal
  が `Unavailable{NoProvider}`、perceptual が
  `Unavailable{RequiresAudition}` のまま — 旧 evidence が
  current を自称しない
  （`cb08_cmp_02_branch_edit_marks_session_and_result_stale`・
  `cb08_cmp_signal_unaligned_policy_and_stale_flag`）。
- Example: 同一 pin の `evaluate` を繰り返すと 2 回目以降が
  `CacheStatus::Hit` で同一 `key` digest を返し、pin 不変の
  別 session でも hit する。candidate をさらに edit すると
  composed-view digest が変わり `Miss` + 別 key になる —
  cache は derived であって truth ではない
  （`cb08_cmp_cache_hit_reuse_and_content_invalidation`）。
- Example: pin 後に candidate が動くと `open_session` は
  `StaleRevision{Right}`、forged scope digest は `StaleScope`、
  未知 branch は `UnknownBranch`、別 set の endpoint 組合せは
  `ScopeMismatch` で拒否される。session open・evaluate は
  live revision・`Project`・branch content を変えない
  （`cb08_cmp_open_rejects_stale_revision_and_scope_digest`・
  `cb08_cmp_open_rejects_unknown_and_cross_set_endpoints`・
  `cb08_cmp_open_is_read_only_and_session_is_derived`）。
- Example: plugin を持たない project では `Mock` provider の
  report が `origin: Mock` 付きで signal 層に流れ、processor
  を持つ project では opaque plugin state が `Known` でない
  ため `Unavailable{PluginStateUnknown}` に降格し、非有限
  metric は `Unknown{EvidenceQuality}` になる。
  `AlignmentPolicy::Unaligned` は provider の有無に関わらず
  `Unavailable{AlignmentPolicy}` で signal evidence を拒否
  する（`cb08_cmp_signal_provider_gates_plugin_state_and_quality`・
  `cb08_cmp_signal_unaligned_policy_and_stale_flag`）。
- Example: adopt 済み endpoint（`BranchContentRef::Live`）を
  pin して自分自身と比較しても session が開き、changes は空・
  freshness は `Current` — orchestrator は Captured/Live 両方の
  content ref を扱う
  （`cb08_cmp_adopted_branch_compares_against_candidate`）。
