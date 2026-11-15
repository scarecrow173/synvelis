---
id: "IMPL-034XNNwRQLH6IQ0GqHISl9"
type: "impl"
status: "in-progress"
title: "CB-09 Difference Explorer headless view-model + driver-neutral ViewSpec（synvelis#286）"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/286"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/variant_diff.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/cb09_diff_explorer.rs"
      - type: "file"
        path: "docs/impl/ir/cb-09-diff-explorer.md"
      - type: "file"
        path: "docs/test-specs/cb-09-diff-explorer.md"
      - type: "file"
        path: "docs/impl/exp/cb-09-diff-explorer.md"
      - type: "file"
        path: "evidence/sv286/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv286/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/view.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/state.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/creative-branch.md"
    - "docs/designs/creative-branch-design.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/test-specs/rcp-06-render-gui.md"
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
    - "docs/impl/ir/rcp-06-render-gui.md"
    - "evidence/sv286/"
  refines: []
  refined-by: []
  verifies:
    - "docs/test-specs/cb-09-diff-explorer.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# CB-09 Difference Explorer headless view-model + driver-neutral ViewSpec（synvelis#286）

## Summary

Creative Branch Difference Explorer の headless slice を
`synvelis-workspace` に実装した。`variant_diff.rs` が pinned
comparison context・branch authority・Semantic/Signal/
Perceptual の型付き evidence を入力に取り、deterministic な
view-model（`VariantDiffView`）と driver-neutral な
`ViewSpec` projection（`variant_diff_spec`）を生成する。
action は typed candidate として列挙され、wire decode
（`decode_variant_diff_action`）と authority 検証付き apply
（`apply_variant_diff_action`）を経て `VariantDiffIntent`
（Compare/AdoptPreview/Adopt）または view-local state 更新に
なる — view-model 自身は domain authority を実行しない。

## Context

synvelis#286（CB-09）。`docs/specs/creative-branch.md` と
`docs/designs/creative-branch-design.md` の Difference
Explorer 要件 — Semantic/Signal/Perceptual の 3 layer 分離、
pinned endpoint/context の明示、stale/unknown の正直な扱い、
typed evidence、authority 境界 — を headless view-model +
driver-neutral ViewSpec として実装する slice。mock/Fake
driver が GPUI 実装と並行開始できる契約を先に固定する。
実 GPUI mount・plugin・PCM/RT・OS 受入は別 gate（#288 系）
であり、本 slice の成功はそれらを意味しない。

## Implementation

### Changes

- `crates/synvelis-workspace/src/variant_diff.rs`（新規、
  2054 行）:
  - 入力契約: `VariantDiffInput`（`CompareContext` pin +
    `CompareAuthority` + `SemanticEvidence` + `AuxLayerEvidence`
    ×2 + `CapabilityEvidence`）。
  - `project_variant_diff` → `VariantDiffView`: domain 写像
    （`VariantDiffDomain::for_scope`）、`ContextState`
    （Current/Stale/Rejected）評価、branch view（独立 label
    軸: Adopted/Auditioning/Editing/RtActive/RtPending）、
    semantic section（`SemanticDiffEvidence` → `DiffRow`/
    `DiffFieldRow`/`IssueRow`）、aux section ×2、capability
    view、action candidates。
  - Semantic 投影は `compare_snapshots` の `ChangeSet` を
    `From<&ChangeSet>` で取り込み、`ComparisonDomain::
    Variants` の change を content row から除外して
    `variants_bookkeeping` として計上（summary/unresolved は
    全量保持）。Arrangement domain は adapter 未 merge のため
    evidence が供給されても `Unsupported` で row を生成しない。
  - Action: `VariantDiffViewAction`（SelectRow/SetAudition/
    SetTarget = view-local、Compare/AdoptPreview/Adopt =
    authority intent 候補）。`decode_variant_diff_action` は
    wire params → typed action（pin 必須・fail-closed）、
    `apply_variant_diff_action` は view state 更新または
    `VariantDiffIntent` を返し、StaleRevision/UnknownBranch/
    SameBranch/UnsupportedDomain 等を `VariantDiffReject` で
    型付き拒否。Adopt intent は apply 時の
    `authority.scope_digest` を CAS guard として同梱。
    apply は context の `Current` 判定も apply 時 authority に
    対して再導出する（投影時 verdict のままでは projection→
    apply 間の authority drift を取りこぼす — review 指摘）。
  - `variant_diff_spec` → `ViewSpec`: Panel/Label/Table/
    Button の deterministic projection。component key は
    `variant-diff.*` 名前空間で一意。executable candidate のみ
    Button、blocked は reason 付き Label。`EvidenceOrigin::
    Mock` は provenance label を露出。`VARIANT_DIFF_TARGET =
    "cb.variant-diff"`。
  - 投影は bounded: `MAX_FIELDS_PER_ROW=16`・
    `MAX_CONTENT_ROWS=256`・`MAX_ISSUE_ROWS=128`・
    `MAX_AUX_ROWS=256`。超過分は view model の
    `omitted_rows`/`omitted_issues` と spec の
    "showing X of Y" label で正直に計上し、driver の
    `MAX_TABLE_ROWS=4096` 上限を常に下回る。capability
    reason label は spec 側で 32 件まで表示し超過は
    omitted label で明示（view model は全件保持）。
    selected semantic row は field-level before/after を
    `variant-diff.semantic.detail.*` label として投影し、
    view-local selection を観測可能にする。
- `crates/synvelis-workspace/src/lib.rs`: `pub mod
  variant_diff` + 公開型/定数/projection の re-export。
- `crates/synvelis-workspace/src/view.rs`: `ViewType::
  VariantDiff` 追加（read-only・pinned variant branch を投影・
  typed authority request を emit し実行しない旨を doc 記載）。
- `crates/synvelis-workspace/src/state.rs`:
  `ViewState.variant_diff: Option<VariantDiffViewState>` を
  `#[serde(default, deserialize_with)]` で追加 — view-local
  のみ、malformed payload は `None` に degrade（既存の
  Signal Flow/Automation/Render/Script Editor/Control
  Relation と同じ tolerant per-view pattern）。
- `crates/synvelis-workspace/tests/cb09_diff_explorer.rs`
  （新規、33 test）: 全 scope→domain 写像、Arrangement
  Unsupported、real `compare_snapshots` ChangeSet 投影
  （Variants bookkeeping 分離）、独立 branch label、
  explicit-pin action decode/apply、全 reject 経路、stale/
  rejected context、unknown≠equal、missing-plugin capability
  分離、mock provenance label、ViewSpec determinism/key
  一意性/executable-only button、view-local action が
  authority を触らないこと、ViewState serde roundtrip +
  malformed tolerance、ViewType 登録、投影 cap/省略表示、
  pinned context identity の spec 表示、button params の
  decode roundtrip、apply 時 authority scope の domain 判定、
  editability/already-adopted による adopt gating、
  Available-without-payload/origin の Unknown 降格、
  scope mismatch の UnknownScope、audition decode の
  object/branch key 必須化、apply 時 authority に対する
  context 再検証（projection→apply drift）、selected row の
  field detail 投影、same-branch context の adopt candidate
  重複排除、capability reason 表示 cap、summary label の
  全 change kind 計上。

### Decisions During Implementation

- `ComparisonEquality` が `Copy` でないため `overall`/
  `equality` は clone して保持（diff evidence は owned）。
- `Variants` domain bookkeeping を content row から除外する
  判定は `ObjectKind::domain() == ComparisonDomain::Variants`
  — `ChangeSetSummary`・`unresolved`・`overall` はフィルタ前
  の全量を保持し、情報損失なしにノイズだけを除く
  （exp E1）。
- Adopt intent の `scope_digest` は input context ではなく
  apply 時の `CompareAuthority` から取る — intent が常に
  権威の現在値を運ぶ（exp E2）。
- `EvidenceOrigin::Mock` の aux evidence は spec 上で必ず
  provenance label を露出し、unavailable/unknown は行を
  生成しない（exp E3）。
- review-protocol round で、apply の context 検証を投影時
  verdict 参照から apply 時 authority に対する
  `context_state` 再導出へ修正 — projection→apply 間に
  project_revision/scope_digest/set/scope/branch revision が
  drift しても intent を mint しない（fail-closed）。
- 同 review で、selected row の field detail を spec に投影
  （選択状態を観測可能に）、same-branch context の adopt
  candidate 重複を排除、capability reason 表示に上限、
  summary label に全 ChangeKind 計上を追加。

## Related Experiments

### Adopted

- `docs/impl/exp/cb-09-diff-explorer.md` E1（Variants
  bookkeeping の content 除外 + 件数計上）— 採用。
- 同 E2（authority scope_digest を intent に同梱）— 採用。
- 同 E3（mock provenance label 必須・unknown は行なし）—
  採用。

### Rejected

- なし（実験は全て採用。代替案は各 E 節に記録）。

## Validation

- `cargo test -p synvelis-workspace --test cb09_diff_explorer`
  — 33 passed / 0 failed（Windows, rustc 1.99.x, exit 0）。
- `cargo check -p synvelis-workspace` — pass（missing-docs
  warning を doc comment 追加で解消済み）。
- 詳細な環境・コマンド・exit code・NotRun 一覧は
  `evidence/sv286/verification.md` を参照。

## Risks

- `variant_diff.rs` は 2054 行の単一モジュール — view-model・
  action・spec projection が同居するが、責務は型で分離済み。
  今後 signal/perceptual の実測定 adapter が入る際に aux
  section の投影が肥大化する可能性。
- `ViewSpec` の Button/Label 構成は driver 非依存の最小契約 —
  GPUI driver 側の実レンダリング・focus・keyboard 経路は未検証
  （本 slice の非目標）。
- `VariantDiffViewState` の serde 形状は v1 — 将来 field 追加時は
  `#[serde(default)]` で後方互換を維持する必要あり。

## Follow-ups

- synvelis#288 系: real signal/Plugin/OS/PCM/RT acceptance
  gate での Difference Explorer 実機検証。
- CB-08 orchestrator（#285）からの `VariantDiffInput` 供給
  経路の接続。
- GPUI/Fake driver consumer の mount slice（ViewSpec →
  実画面）。
