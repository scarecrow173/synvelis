---
id: "TSPEC-034XO3CeEjpQyb1MNDayVm"
type: "test-spec"
status: "draft"
title: "CB-09 Difference Explorer headless view-model + driver-neutral ViewSpec"
created: "2026-11-15"
updated: "2026-11-18"
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
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/test-specs/rcp-06-render-gui.md"
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

# CB-09 Difference Explorer headless view-model + driver-neutral ViewSpec

## Purpose

Creative Branch の Difference Explorer が、pinned された
variant branch の比較結果を driver-neutral な headless
view-model と `ViewSpec` projection として提供し、Semantic /
Signal / Perceptual の 3 layer を型付き evidence として分離し、
stale・unavailable・unknown・unsupported を「差分なし」に
見せないことを保証する。この spec は synvelis#286 の
`CB-UX-01`〜`CB-UX-04`、`CB-CMP-01`〜`CB-CMP-03`、
`CB-UI-RCP-01` の headless 検証意図を固定する。実 GPUI mount・
実 plugin dock・PCM/RT signal 測定・OS/IME・CB-08 orchestrator
経由の end-to-end 受入は対象外であり、それらの保証が必要に
なった時点で別 test-spec が必要。

## Feature

CB-09 Difference Explorer — pinned variant branch 比較の
headless view-model と driver-neutral ViewSpec projection

## Rules

- Rule: Explorer は `VariantScope` を `VariantDiffDomain`
  （AudioClip/TakeUsage/MidiClip/InstrumentSlot/
  ProcessingRange/StripFields/Arrangement）へ写像し、
  adapter 未 merge の domain は
  `EvidenceState::Unsupported{DomainAdapterUnmerged}` として
  正直に報告する — fabricated row を生成しない
  （CB-UX-03）。列挙 domain は CB-07/#284 merge により
  全て adapter 済みであり、`DomainAdapterUnmerged` は
  将来の未 merge domain に対する typed verdict として
  残る（synvelis#286 で stale verdict を修正済み）。
- Rule: branch の Adopted/Auditioning/Editing/RT-Active/
  RT-Pending label は独立した軸であり、一つの branch が
  複数 label を同時に持ちうる。view-local の audition 選択が
  authority の adopted/editing/publish 状態を変更しない
  （CB-UX-02）。
- Rule: compare/adopt/adopt-preview は明示的な `BranchPin`
  （branch id + `BranchRevision`）を必須とし、implicit target・
  malformed payload・unknown action 名は `ImplicitTarget`/
  `Malformed` の型付き reject。pin が authority の現行
  revision と一致しない場合 `StaleRevision`、未知 branch は
  `UnknownBranch`、同一 branch 同士の compare は `SameBranch`
  で拒否される（CB-UX-01/04）。
- Rule: `Adopt`/`AdoptPreview` intent は apply 時点の
  `CompareAuthority.scope_digest` を CAS guard として同梱する
  — view-model は digest を計算・転記せず、client も assert
  しない（CB-UX-04）。
- Rule: pinned context（variant_set・scope・scope_digest・
  project_revision・両端 branch revision）が authority と
  乖離した場合、`ContextState::Stale{reasons}` または
  `Rejected{reason}` として報告し、全 authority action を
  blocked にする — stale context で action を実行可能に
  見せない（CB-CMP-01）。
- Rule: `Unknown`/`Unavailable`/`Unsupported` の evidence は
  equality を `None` とし行を生成しない — 「差分なし
  (Equal)」に昇格しない（CB-CMP-02）。
- Rule: Signal/Perceptual layer は Semantic と独立した
  `AuxSection` で、`EvidenceOrigin::Mock` の evidence は
  `ViewSpec` 上で provenance label（"mock"）を必ず露出する。
  headless/mock の成功は real 測定の受入を意味しない
  （CB-CMP-03）。
- Rule: capability evidence は editability と exact audition
  を分離し、plugin 不在は `MissingPlugin` として片方だけを
  `Unavailable` にできる（CB-UI-RCP-01）。
- Rule: `ViewSpec` projection は同一 `(view, state)` に対し
  deterministic で、component key は一意。executable な
  candidate のみが `Button` になり、blocked candidate は
  block reason を示す `Label` に留まる。
- Rule: `VariantDiffViewState` は view-local（target・
  audition・selected_row・layer visibility）のみを保持し、
  project/variant/diff の authority を持たない。永続化は
  tolerant decode で malformed payload を `None` に degrade
  し、snapshot 全体を reject しない。

## Examples

- Example: AudioClip scope の live vs candidate（clip mute
  差分）で `compare_snapshots` の `ChangeSet` を投影すると、
  `clip/modified` の content row が現れ、`VariantSet`/
  `VariantBranch` の bookkeeping 変更は content row に出ず
  `variants_bookkeeping` 件数として計上される。
- Example: adopted branch + audition 中 branch + editing 中
  branch + RT pending branch が混在しても、各 branch view の
  labels が独立に正しく立つ。
- Example: `compare` に `{left}` のみ・`adopt` に params 無し・
  未知 action 名・`{"row": "x"}` の select は全て型付き reject。
- Example: context の branch revision・scope_digest・
  project_revision のいずれかが authority とずれると
  `ContextState::Stale` となり spec 上の authority button が
  全て消える（view-local audition button のみ残る）。
- Example: signal layer が `Unavailable`・perceptual が
  `Unknown` のとき、両 section の equality は `None` で行は
  空 — mock の `+0.8 LU` 行は origin `Mock` として label に
  "mock" を含む。

## Guarantees

- 全 `VariantScope` variant が正しい `VariantDiffDomain` に
  写像され、unsupported domain は fabricated diff を出さない。
- typed action の decode→validate→intent 経路が fail-closed で、
  authority 実行は view-model の外に残る。
- `ViewSpec` が deterministic・stable key・executable-only
  control を満たし、Mock driver がそのまま消費できる。
- view-local state の serde roundtrip と malformed tolerance。

## Non-goals

- 実 GPUI/GPUI-component mount、実 window/OS/IME、実 plugin
  dock、PCM/RT signal 測定、real perceptual evaluation。
- CB-08 orchestrator からの end-to-end 駆動、adopt の実行
  （intent 発行まで）、3-way merge、signal/perceptual の
  実測定 adapter。
- `ViewSpec` の driver 側レンダリング実装（GPUI/Fake/Mock
  consumer は別 slice）。

## Risk

- `Variants` domain bookkeeping の filter を誤ると scoped
  content 差分が欠落するか、内部機構のノイズが content に
  混入する。
- `ViewSpec` に測定値らしき表示を出すと headless 成功が real
  受入に誤認される — provenance label の欠落はこのリスクを
  直接生む。
- context 検証を apply 時ではなく投影時だけにすると、stale
  context で action が executable に見える窓が生まれる。
