---
id: "IMPL-034XNDR6phv2BXm24gk8cN"
type: "impl"
status: "completed"
title: "CB-11 cross-gate headless fixture 被覆棚卸し・欠落 fixture 追加 (synvelis#288)"
created: "2026-11-15"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/288"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/cb11_cross_gate.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs"
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
    - "evidence/sv296/acceptance-ledger.md"
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

Issue #288 の headless fixture slice（slice 1: CB-01..06 基盤上の
cross-gate 棚卸し + slice 2: CB-07..10 merge 済み前提の runnable
subset 拡張）。Creative Branch の merge 済み 7 domain adapter
（CB-01..07）・orchestrated compare（CB-08）・gateway variant
actions（CB-10）・VCS-02 semantic diff・branch render-pin 解決・
RCP-07 case ledger をまたぐ cross-gate fixture を `synvelis-
project`/`synvelis-gateway` の公開 API のみで拡張し、Issue #288
の全 case-ID（`CB-E2E-01..09` + `CB-X-RCP-01`）を fixture /
oracle / runnability へ棚卸しした evidence ledger を
`evidence/sv288/` に整備した。production コード・既存 test・
canonical doc は一切変更していない。

## Context

Issue #288 は長期 test/integration tracker であり、概要が
「FixtureはDomain Adapterの実装を待たず並行作成する」と
規定する。slice 1 は merged foundations（CB-01..06）上の
runnable subset のみを対象とし、CB-07..10 を Blocked として
記録した。slice 2 は CB-07（#284）・CB-08（#285）・
CB-10（#287）・RCP-07 headless slice（#296）の merge を受け、
Issue 本文の Stage-3 規則（着手条件なし・adapter 実装を待た
ず fixture を並行作成）に従い CB-E2E-03・CB-E2E-04
orchestration 軸・CB-E2E-07 headless 半分・CB-X-RCP-01 紐付け
を runnable 化した。GPUI Difference Explorer（CB-09・#286）
と全外部 gate（実 signal/plugin/OS/consumer/実機 Gate A）は
引き続き ledger で Blocked/NotRun/Deferred として正直に記録
し、headless green を実機受入へ換算しない。

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
  - slice 2 拡張: `adapter_for` dispatch に merge 済み
    `ArrangementAdapter`（CB-07）を接続し、gate fixture に
    2 Track / 4 Clip の arrangement bed（out-of-range・
    absolute-anchored witness・clip-local note 付き）を追加。
  - `cb11_e2e_03_arrangement_composite_adopt_undo_scope_isolation`:
    mute+move+note-remove の composite candidate edit が単一
    adopt=1 CAS=1 undo unit に materialize され、undo/redo が
    bitwise 復帰し、scope 外 clip・他 domain・raw material が
    不変であることを検証。
  - `cb11_e2e_04_orchestrated_compare_*`: CB-08
    `ComparisonOrchestrator` の session を通し、scoped edit
    が自 session の Right-side stale dims + `ProjectRevision`
    のみで stale 化し sibling session は `ProjectRevision`
    のみ、shared-context edit が全 session を `SharedContext`
    stale にすること、stale session の再評価が pin-scoped
    semantic evidence を保ち aux layer を stale flag する
    こと、`Unaligned` policy・opaque plugin state・非有限
    metric・probe loss が `EvidenceReason`/`Unknown`/`Partial`
    で誠実に降格することを検証。
- `crates/synvelis-gateway/tests/cb11_ai_vcs_off.rs`（新規）:
  CB-10 merge 済み gateway 契約で CB-E2E-07 headless 半分を
  pin — grantless agent の variant op が prepare/apply 両段で
  `Denied`("no applicable authorization") fail-closed、
  `VariantPorts::standard(None)`（in-memory store +
  `NoopRuntimePublish`、durable/VCS backend 非搭載）rig で
  fork→edit→adopt（ApproveToApply + human receipt）→query→
  discard が独立動作、arrangement scope は wired adapter
  不在で `unsupported` fail-closed（src gap を正直に pin）。
- `docs/test-specs/cb-11-cross-gate.md`（slice 1 新規・slice 2
  更新）: Issue #288 の全 case-ID を Purpose/Feature/Rules/
  Examples/Guarantees/Non-goals/Risk 構造で仕様化し、oracle
  種別と runnability 分類を固定。slice 2 で CB-07..10 merge
  済み分を Runnable headless now へ再分類し、case↔test
  対応を更新。
- `evidence/sv288/{coverage,external-gates,verification}.md`
  （slice 1 新規・slice 2 更新）: `evidence/sv109` の 3-file
  構成を踏襲し、case→fixture→oracle→result 台帳・外部 gate
  判定表・実行記録を整備。slice 2 で stale `Blocked` 行を
  実走 evidence に基づき更新し、RCP-07 case-ID mapping・
  rf#628 closed 記録・gateway adapter gap を追記。

### Decisions During Implementation

- `midi_sources` は scope-owned content であり raw material
  witness から除外した（fixture bug — src defect ではない）。
  take fixture の `source_offset` は絶対 source frame として
  coverage `[40000, 70000)` を満たす値に修正。
- adopted 後の live branch は `BranchContentRef::Live` となり、
  旧 adopted branch は `Captured` に凍結される — receipt の
  provenance 検証は live 側の `last_adopt` を対象にした。
- `too_many_lines`/`similar_names`/`used_underscore_binding`
  lint は cb02..cb10 先例に倣い、逐次 fixture 構築・track×lane
  位置を示す命名の正当性を scoped `#[allow]` または改名で
  処理（lint 隠蔽ではなく fixture 構造由来の既知パターン）。
- slice 2: orchestrated compare の session は project
  revision を open 時に pin するため、session open 前に全
  fork/edit を完了させる構成にした。scoped candidate edit
  後の sibling session は `Current` ではなく
  `Stale{[ProjectRevision]}` のみ — これが domain 分離の
  正直な契約であり、「sibling が Current のまま」という
  当初仮説は実測で棄却した（exp E6）。
- slice 2: probe loss（`BlobRead::Absent`）は asset
  availability manifest 経由でのみ `unresolved`/`Partial`
  に効く — asset を参照しない take scope では `Complete`
  のままだったため、probe-loss assert は asset 保持の
  gate fixture audio scope へ移した（exp E7）。
- slice 2: `VariantPorts::standard` が `ArrangementAdapter`
  を wire していないのは CB-10 slice の意図的な未配線で
  あり、CB-07 merge 後も gateway 側は未更新 — src 変更は
  本 slice 範囲外のため、fail-closed `unsupported` を
  fixture で正直に pin し ledger/PR に gap として記録した
  （exp E8）。

## Related Experiments

### Adopted

<!-- 実験ログ採用なし。検証は exp メモの Expected vs Observed 記録に集約。 -->

### Rejected

<!-- なし。 -->

## Validation

- `mise run fmt-check`: Pass
- `mise run check synvelis-project`: Pass
- `mise run lint synvelis-project`: Pass
- `mise run test synvelis-project`: Pass（cb11 9 test 含む全 target green）
- `mise run check synvelis-gateway`: Pass
- `mise run lint synvelis-gateway`: Pass
- `mise run test synvelis-gateway`: Pass（cb11_ai_vcs_off 3 test 含む全 target green）
- `mise run test-rt`: 実行結果は `evidence/sv288/verification.md` に記録
- `mise run check-headless-deps`: 実行結果は `evidence/sv288/verification.md` に記録
- `mise exec -- prek run --all-files`: 実行結果は `evidence/sv288/verification.md` に記録
- impl-doc audit（`audit_impl_record.js`・`audit_experiment_log.js`）: 実行結果は `evidence/sv288/verification.md` に記録

## Risks

- headless fixture の green は Project 軸の CAS/Undo/Save/
  Reload/Semantic Diff/render-pin 解決・orchestrated compare
  の freshness/evidence 契約・gateway の grant/approval/
  VCS-off 契約のみを証明する。PCM 実測・実 RT publish・
  実 GUI/plugin/device/consumer 受入・実 AI/VCS backend は
  別 gate の責務であり、本 slice の Pass をそれらへ換算
  しない。
- CB-09 GPUI Difference Explorer（#286）は未 merge であり
  GUI 面の cross-gate は本 fixture の対象外。
- gateway `VariantPorts::standard` の Arrangement 未配線は
  fail-closed で安全側だが、CB-E2E-03 の gateway 経路は
  src 変更（adapter 登録）を要する follow-up。

## Follow-ups

- CB-09 GPUI Difference Explorer（#286）merge 後の GUI
  fixture — 実 window・実 plugin は外部 gate。
- gateway `VariantPorts::standard` への `ArrangementAdapter`
  登録（src 変更 — 別 slice/Issue で対応）。
- `CB-X-RCP-01` の recipe cache/invalidation・RCP-GATE 本体は
  #296/RCP-07 側の所有として参照のみ（case-ID mapping は
  `evidence/sv288/coverage.md` に記録済み）。
- 実 signal/clock（`scarecrow173/ruviform#628` — closed、
  deterministic runtime は存在するが CB PCM 軸接続は未評価）・
  実 Plugin ABI（`scarecrow173/ruviform#630`）・実 GUI runner
  （`scarecrow173/ruviform#631`）・AudioLens consumer
  （`scarecrow173/ruviform#294`/`#308`/`#309`）の外部 gate。
