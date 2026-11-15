---
id: "IMPL-034XNDR6phv2BXm24gk8cN"
type: "impl"
status: "completed"
title: "CB-11 cross-gate headless fixture 被覆棚卸し・欠落 fixture 追加 (synvelis#288)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/288"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/cb11_cross_gate.rs"
      - type: "file"
        path: "docs/test-specs/cb-11-cross-gate.md"
      - type: "file"
        path: "docs/impl/ir/cb-11-cross-gate.md"
      - type: "file"
        path: "docs/impl/exp/cb-11-cross-gate.md"
      - type: "file"
        path: "evidence/sv288/coverage.md"
      - type: "file"
        path: "evidence/sv288/external-gates.md"
      - type: "file"
        path: "evidence/sv288/verification.md"
    modified: []
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
    - "docs/test-specs/cb-11-cross-gate.md"
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
    - "docs/test-specs/vcs-02-semantic-diff-fixtures.md"
    - "docs/test-specs/sf06-cross-gate.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/cb-11-cross-gate.md"
    - "evidence/sv288/coverage.md"
    - "evidence/sv288/external-gates.md"
    - "evidence/sv288/verification.md"
    - "evidence/sv109/coverage.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/cb-11-cross-gate.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# CB-11 cross-gate headless fixture 被覆棚卸し 実装記録

## Summary

Issue #288 の headless fixture slice。Creative Branch の
merge 済み 7 domain adapter（CB-01..06）と VCS-02 semantic
diff・branch render-pin 解決をまたぐ cross-gate fixture を
`synvelis-project` の公開 API のみで追加し、Issue #288 の
全 case-ID（`CB-E2E-01..09` + `CB-X-RCP-01`）を fixture /
oracle / runnability へ棚卸しした evidence ledger を
`evidence/sv288/` に整備した。production コード・既存 test・
canonical doc は一切変更していない。

## Context

Issue #288 は長期 test/integration tracker であり、概要が
「FixtureはDomain Adapterの実装を待たず並行作成する」と
規定する。本 slice は design doc §9.2 の SV-CB DAG が並行
開始ノードとして stage する headless slice であり、
merged foundations 上の runnable subset のみを対象とする。
unmerged 前提（CB-07 arrangement・CB-08 orchestrated compare・
CB-09 GPUI・CB-10 gateway actions）と外部 gate（実 signal/
plugin/OS/consumer）は ledger で Blocked/NotRun/Deferred と
して正直に記録し、headless green を実機受入へ換算しない。

## Implementation

### Changes

- `crates/synvelis-project/tests/cb11_cross_gate.rs`（新規）:
  - `Gate` harness が `ProjectController`・
    `InMemoryBranchContentStore`・`SnapshotResolver`・
    `FakeRuntimePublish` を所有し、fork/edit/adopt/candidate/
    snapshot/render-pin 操作を公開 API 経由で提供。
  - 5 domain disjoint scope fixture（AudioClip・MidiClip・
    InstrumentSlot・ProcessingRange・StripFields）と、
    recording registry を手動 seed する TakeUsage/Comp fixture。
  - `cb11_e2e_01_*`: fork→scope edit→candidate isolation→
    `compare_snapshots`→`adopt_branch_adapted`→1 CAS=1 undo→
    undo で pre-adopt 完全復帰→save/reload で variant set・
    lineage・content ref・digest が verbatim 保持、を
    5 domain 跨ぎと Take/Comp で検証。
  - `cb11_e2e_04_*`: scoped adoption が scoped semantic
    change のみを報告し raw material churn を報告しないことを
    検証。
  - `cb11_x_rcp_01_*`: `resolve_render_project` の render pin が
    scope-owned edit で `scope_digest` のみ、shared-context
    edit で `shared_context_digest` のみを動かすことを検証。
  - `cb11_e2e_05_*`: stale-scope adopt が `VariantError::
    StaleScope` で fail-closed し、live state・revision・
    sibling candidate を汚染しないことを検証。
- `docs/test-specs/cb-11-cross-gate.md`（新規）: Issue #288 の
  全 case-ID を Purpose/Feature/Rules/Examples/Guarantees/
  Non-goals/Risk 構造で仕様化し、oracle 種別と runnability
  分類を固定。
- `evidence/sv288/{coverage,external-gates,verification}.md`
  （新規）: `evidence/sv109` の 3-file 構成を踏襲し、case→
  fixture→oracle→result 台帳・外部 gate 判定表・実行記録を
  整備。

### Decisions During Implementation

- `midi_sources` は scope-owned content であり raw material
  witness から除外した（fixture bug — src defect ではない）。
  take fixture の `source_offset` は絶対 source frame として
  coverage `[40000, 70000)` を満たす値に修正。
- adopted 後の live branch は `BranchContentRef::Live` となり、
  旧 adopted branch は `Captured` に凍結される — receipt の
  provenance 検証は live 側の `last_adopt` を対象にした。
- `too_many_lines` lint は cb02/cb03 先例に倣い、逐次 fixture
  構築の正当性を示す scoped `#[allow]` を付与（lint 隠蔽では
  なく fixture 構造由来の既知パターン）。

## Related Experiments

### Adopted

<!-- 実験ログ採用なし。検証は exp メモの Expected vs Observed 記録に集約。 -->

### Rejected

<!-- なし。 -->

## Validation

- `mise run fmt-check`: Pass
- `mise run check synvelis-project`: Pass
- `mise run lint synvelis-project`: Pass
- `mise run test synvelis-project`: Pass（cb11 6 test 含む全 target green）
- `mise run test-rt`: 実行結果は `evidence/sv288/verification.md` に記録
- `mise run check-headless-deps`: 実行結果は `evidence/sv288/verification.md` に記録
- `mise exec -- prek run --all-files`: 実行結果は `evidence/sv288/verification.md` に記録

## Risks

- headless fixture の green は Project 軸の CAS/Undo/Save/
  Reload/Semantic Diff/render-pin 解決のみを証明する。PCM 実測・
  実 RT publish・実 GUI/plugin/device/consumer 受入は別 gate
  の責務であり、本 slice の Pass をそれらへ換算しない。
- CB-07..CB-10 の unmerged domain は本 fixture の対象外であり、
  merge 後に cross-gate coverage を拡張する必要がある。

## Follow-ups

- CB-07 Arrangement adapter（#284）・CB-08 orchestrated
  compare（#285）・CB-09 GPUI（#286）・CB-10 gateway（#287）
  merge 後の fixture 拡張。
- `CB-X-RCP-01` の recipe cache/invalidation・RCP-GATE 本体は
  #296/RCP-07 側の所有として参照のみ。
- 実 signal/clock（`scarecrow173/ruviform#628`）・実 Plugin
  ABI（`scarecrow173/ruviform#630`）・実 GUI runner
  （`scarecrow173/ruviform#631`）・AudioLens consumer
  （`scarecrow173/ruviform#294`/`#308`/`#309`）の外部 gate。
