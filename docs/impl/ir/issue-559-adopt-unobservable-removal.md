---
id: "IMPL-034ZwRgI5iWfZe5yFp6VPB"
type: "impl"
status: "completed"
title: "Issue #559 impl IR — adopt unobservable-removal fail-closed 実装記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/559"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/specs/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/designs/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/plans/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/tasks/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/test-specs/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/impl/exp/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/impl/exp/issue-559-adopt-unobservable-removal-exp.jsonl"
      - type: "dir"
        path: "evidence/sv559/"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/apply/entity.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/error.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/variant.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb06_strip_fields.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb06_strip_gateway.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-559-adopt-unobservable-removal.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-559-adopt-unobservable-removal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/issue-557-adopt-facts.md"
    - "docs/impl/ir/issue-553-intent-facts.md"
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
    - "docs/impl/exp/issue-559-adopt-unobservable-removal.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-559-adopt-unobservable-removal.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-559-adopt-unobservable-removal.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-559-adopt-unobservable-removal.md"
    rejected: []
---

# Issue #559 impl IR — adopt unobservable-removal fail-closed 実装記録

## Summary

`VariantIntent::Adopt` が durable stored candidate payload
に含まれる観測不能 kind の `Removed` write を fail-closed
で拒絶する hardening を実装した。domain
`plan_adopt_writes` に `entity::unverifiable_removal_kind`
predicate（`ModulationSource | ModulationAssignment` のみ
真）を追加し、`Removed` write 構築前に typed
`VariantError::UnverifiableRemoval` を返す。gateway
`variant_reason` は `"unverifiable-removal"` に写像する。
`RoutingConnection` は sv#550 send teardown の internal-
structure 境界として意図的に対象外とし、観測可能な send
teardown の adopt 物質化は維持した。

## Context

synvelis#557 監査（PR #560）で特定された残存経路:
synvelis#550〜#553 の窓に永続化された candidate payload は
pre-553 wire `send.remove` cascade が生成した send-owned
`ModulationSource`/`ModulationAssignment` 削除を含み得る。
gateway の `ResourceKind`/`snapshot::ENUMERATED_KINDS` は
これらを独立列挙しないため、`derive_effects`/
`authorize_effects` は当該削除を観測できず、adopt は
typed 拒絶なく live に物質化していた
（再現: `evidence/sv559/reproduction.md`）。

## Implementation

### Changes

- `crates/synvelis-project/src/apply/entity.rs` —
  `unverifiable_removal_kind(ObjectKind) -> bool` を追加。
  `ModulationSource | ModulationAssignment` のみ真。
  `RoutingConnection` 除外の理由（send teardown の
  internal structure・`op_facts` RemoveSend arm と同じ
  分類）をコメントで明示。
- `crates/synvelis-project/src/variant/error.rs` —
  `VariantError::UnverifiableRemoval { detail: String }`
  を追加し `Display`（`"unverifiable removal: {detail}"`）
  を実装。
- `crates/synvelis-project/src/variant/engine.rs` —
  `plan_adopt_writes` の `origin_objects` loop で
  `removal_supported_kind` 通過後・`Removed` write 構築前に
  `unverifiable_removal_kind` を検査し typed error を返す。
  関数 doc に fail-closed 規則を追記。
- `crates/synvelis-gateway/src/variant.rs` —
  `variant_reason` の exhaustive match に
  `("unverifiable-removal", detail)` arm を追加。
- `crates/synvelis-project/tests/cb06_strip_fields.rs` —
  `cb_strip_559_adopt_unobservable_removal_rejected`:
  send-owned modulation closure を持つ fixture で
  fork→candidate `RemoveSend`→`try_adopt` が
  `UnverifiableRemoval` を返し live/revision/branch が
  不変であることを pin。
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs` —
  `cb06_strip_559_adopt_unobservable_removal_denied`:
  wire-legal `SetSendGainTarget(null)` で assignment 削除を
  含む candidate を永続化し、adopt prepare が
  `PreparationRejected`（`variant-unverifiable-removal`）
  で拒絶・live 非破壊であることを pin。
- doc chain 8 件（discovery/specs/designs/plans/tasks/
  test-specs/impl-ir/impl-exp+jsonl）+ `evidence/sv559/`
  （reproduction.md・verification.md）を新規作成。

### Decisions During Implementation

- **設置境界**: gateway prepare/effect 層ではなく domain
  `plan_adopt_writes`。`Removed` write 化直前に正確な
  `ObjectKind` を知る唯一の層であり、SEC-14 fail-closed
  （`Unsupported`/`AmbiguousLineage`）の既存所有者。
  gateway 側 filter は owner 境界下流での policy 重複。
- **reject 集合**: `ModulationSource | ModulationAssignment`
  のみ。`removal_supported_kind` ∧ 観測モデル外はこの 2 つと
  `RoutingConnection` の 3 つだが、後者は観測可能な owner
  （Send/Processor）削除に不可分な internal structure で
  あり、reject すると sv#550/#557 の受理済み send teardown
  契約を破壊する（`op_facts` RemoveSend arm と同じ分類）。
- **typed error**: 汎用 `Unsupported` ではなく
  `UnverifiableRemoval` — 「操作非対応」と「効果を観測・
  認可できない」を区別し、journal/gateway consumer が
  判別可能にする。
- **`AdoptIntent::declared_facts` 非変更**: sv#557 監査で
  inner op 非所持が確定しており伝播対象が存在しない。

## Related Experiments

### Adopted

- `docs/impl/exp/issue-559-adopt-unobservable-removal.md`
  — fail-closed 境界（domain planner）・reject 集合
  （2 kind のみ）の採用判断と再現確認を記録。

### Rejected

- gateway prepare/effect 層での kind filter — owner 境界
  下流の policy 重複になるため不採用（exp.jsonl EXP-2）。
- `ENUMERATED_KINDS` 外の全 removal-supported kind を
  reject — `RoutingConnection` を巻き込み合法 send
  teardown を破壊するため不採用（exp.jsonl EXP-3）。

## Validation

`evidence/sv559/verification.md` に全記録。Issue 指定の
必須コマンド全て exit 0:

- `mise run check/lint/test synvelis-project`
- `mise run check/lint/test synvelis-gateway`
- `mise run fmt-check` / `check-headless-deps` /
  `prek run --all-files`

`cb06_strip_fields` 25 件・`cb06_strip_gateway` 10 件が
全 Pass — 新規 559 pin・既存 550/553/557 pin の非回帰を
実測。

review-protocol を 1 回実施（Reviewer A cooperative・
Reviewer B adversarial の独立 2 系統 → 候補検証 →
判定）。CONFIRMED P0/P1 なし、verdict
`ACCEPT WITH NON-BLOCKING NOTES`。P2/P3 の内訳:
exp doc の混入破損（修正済み）・gateway pin の経路記述
誇張（修正済み）・`RoutingConnection` 単独削除 residual
の記述精緻化（design doc 修正済み）・`Added`/`Modified`
write の観測不能非対称（pre-existing・follow-up 記録）。
棄却した主な候補: 「`removal_supported_kind` ∧ 観測モデル
外は他 kind を含むのでは」という指摘は、同集合が
`{ModulationSource, ModulationAssignment,
RoutingConnection}` の 3 つに確定することを
`entity.rs`/`snapshot.rs`/`request.rs` の実コードで
検証して棄却。

## Risks

- `SetSendGainTarget` rebind（`release_send_level_target`）
  ・`RemoveTrack` 等の live/edit 経路 cascade が
  `ModulationAssignment` を削除するが `op_facts` が
  unverifiable を flag しない関連 gap を発見 — wire 合法
  op で assignment 削除入り candidate を生成し得る。
  本 slice 範囲外として `synvelis#561` に記録。
- `RoutingConnection` 単独削除（send 温存・Main route・
  Sidechain・chain-internal 全般）と観測不能 kind の
  `Added`/`Modified` write は kind-only 判定では証明
  不能な residual — design doc に明示的受理判定として
  記録。wire 生成不能のため trusted domain API 経路のみ。

## Follow-ups

- `synvelis#561` — wire 側 honest-deny 強化
  （`op_facts` coverage・`SetSendGainTarget` 系 cascade
  の unverifiable 分類・connection 系 op の扱い）。
- docs index（`docs/**/README.md`・`docs/impl/*/README.md`）
  の同期は Coordinator が統合時に再生成
  （`sync_impl_index` preview→apply は本 slice で実施済み
  の範囲に留める）。
- Issue #559 は open のまま — commit/PR 本文は
  `Refs #559` のみ使用（Closes/Fixes/Resolves 禁止）。

## 関連リンク

- `synvelis#559`（本 Issue — open 維持）/ `synvelis#557` /
  `synvelis#553` / `synvelis#550` / `synvelis#561` /
  `synvelis#283`（CB-06 tracker）
- `docs/discovery/issue-559-adopt-unobservable-removal.md`
- `docs/specs/issue-559-adopt-unobservable-removal.md`
- `docs/designs/issue-559-adopt-unobservable-removal.md`
- `docs/plans/issue-559-adopt-unobservable-removal.md`
- `docs/tasks/issue-559-adopt-unobservable-removal.md`
- `docs/test-specs/issue-559-adopt-unobservable-removal.md`
- `docs/impl/exp/issue-559-adopt-unobservable-removal.md`
- `docs/impl/exp/issue-559-adopt-unobservable-removal-exp.jsonl`
- `docs/impl/ir/issue-557-adopt-facts.md`
- `docs/impl/ir/issue-553-intent-facts.md`
- `docs/impl/ir/cb-06-track-bus-adapter.md`
- `evidence/sv559/`
- `crates/synvelis-project/src/variant/engine.rs`
- `crates/synvelis-project/src/apply/entity.rs`
- `crates/synvelis-project/src/variant/error.rs`
- `crates/synvelis-gateway/src/variant.rs`
- `crates/synvelis-project/tests/cb06_strip_fields.rs`
- `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
- `POLICIES.md`
