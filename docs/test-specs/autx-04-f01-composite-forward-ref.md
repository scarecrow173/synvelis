---
id: "TSPEC-034XijnFYzanHYxsFsEIA0"
type: "test-spec"
status: "approved"
title: "AUTX-04-F01 Composite sequential validate — create-then-reference invariants"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/432"
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
    - "docs/automation/design-atomic-macro.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-04-f01-composite-forward-ref.md"
    - "docs/impl/exp/autx-04-f01-composite-forward-ref.md"
    - "evidence/sv432/composite-forward-ref-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/automation/design-atomic-macro.md"
  derived-by: []
  verifies:
    - "docs/automation/design-atomic-macro.md"
    - "docs/specs/command-gateway.md"
  verified-by: []
  references:
    - "crates/synvelis-project/tests/composite.rs"
    - "crates/synvelis-project/src/ops.rs"
    - "evidence/sv432/composite-forward-ref-ledger.md"
  defers: []
  deferred-by: []
---

# AUTX-04-F01 Composite sequential validate — create-then-reference invariants

## Purpose

`ProjectOp::Composite::validate` の逐次 dry-run 化
（synvelis#432 / AUTX-04-F01）が保証すべき不変条件を固定
する。`docs/automation/design-atomic-macro.md` §3 の
create-then-reference 契約の project 側前提 — composite 内で
先行 member が mint する ID を後続 member が参照でき、
単一 CAS/単一 Undo のまま commit まで通ること — を守る。
GW-04 scratch-ID/rewrite seam・gateway `bindings` 昇格・
実 OS/実 Plugin 結合は対象外（後続 slice・NotRun）。

## Feature

`ProjectController::apply` に渡される `ProjectOp::Composite`
の validate/apply 意味論。member は宣言順に適用され、
member i は member 0..i-1 を適用済みの状態で検査される。
validate は live state を一切変更しない dry-run であり、
composite 全体は 1 回の CAS・1 entry の undo を持つ。

## Rules

- **AUTX4F1-TS-R01 sequential member state**: member i の
  validate は、member 0..i-1 が適用された後の状態に対して
  行われる。先行 member が mint した ID（track/lane/
  asset/source 等、ID 種別を問わない）を後続 member が
  参照する forward-reference は受理されなければならない。
- **AUTX4F1-TS-R02 deterministic dry-run issuance**: validate
  の dry-run は `project.id_watermark` シードの
  `CounterIdGenerator` で採番し、live ID を消費しない。
  commit 後の `id_watermark == generator position` 不変条件
  が成立する限り、dry-run と実 apply の採番系列は一致し、
  参照先 ID の予測（`peek_raw_id`）が validate・apply の
  双方で成立する。
- **AUTX4F1-TS-R03 early validate order**: member は宣言順に
  検査され、member i が失敗した場合 member i+1 以降は
  validate も apply もされない。composite 内の失敗は全体を
  reject し、live project・revision・undo/redo stack を
  一切変更しない。
- **AUTX4F1-TS-R04 stable rejection vocabulary**: 空
  composite は `Validation("empty composite")`、nested
  `Composite` は `Validation("nested composite")` で
  reject され、従来の語彙・優先順位を維持する。
- **AUTX4F1-TS-R05 single commit semantics**: composite は
  `ProjectController::apply` の単一 CAS・単一 undo entry
  として commit され、1 回の `undo` で全 member の効果が
  戻り、`redo` で復元される。`StaleRevision` は validate
  前に reject される。
- **AUTX4F1-TS-R06 non-composite unchanged**: `Composite`
  以外の `ProjectOp` arm の validate/apply 挙動・
  `Operation`/`ProjectOp` の API・`ProjectController::apply`
  の commit 経路（validate → clone → apply →
  `validate_references`）は変更されない。

## Examples

- **EX-01**: `[AddTrack, AddLane{track_id: predicted},
  AddLane{track_id: predicted}]` — `predicted` は commit 前に
  `peek_raw_id()` で得た deterministic raw を `TrackId::new`
  で包んだ値。composite が 1 CAS で commit され、新 track が
  2 lane を持ち、1 回の undo で track+lane がまとめて
  戻る（`composite_create_then_reference_commits_as_
  one_undo_step`）。
- **EX-02**: `[RegisterAsset, RegisterSource{asset_id:
  predicted}]` — track/lane 以外の kind でも create-
  then-reference が成立する（`composite_forward_reference_
  works_for_asset_source_pair`）。
- **EX-03**: `[AddLane{missing}, AddTrack]` — member 0 の
  `NotFound` で即 reject され、member 1 は評価されず、
  live/revision/undo は不変
  （`composite_member_failure_never_reaches_later_members`）。
- **EX-04**: `[AddTrack, AddLane{missing}]` — member 0 が
  dry-run で mint しても member 1 の失敗で全体 reject、
  部分 commit・undo entry なし
  （`composite_mid_member_failure_discards_the_whole_dry_run`）。
- **EX-05**: `Composite{ops: []}` → `"empty composite"`、
  `Composite{ops: [Composite{...}]}` → `"nested composite"`
  の語彙が不変（`composite_rejects_empty_and_nested_with_
  stable_vocabulary`）。stale `expected_revision` は
  `StaleRevision` で validate 前に reject
  （`composite_stale_revision_is_rejected_before_validation`）。

## Guarantees

- forward-reference composite は `ProjectController::apply`
  で実 commit され、commit 後に参照関係が正本
  （`tracks`/`lanes`/`assets`/`sources` 等）へ反映される。
- validate は純粋な dry-run であり、失敗時に live project・
  live revision・undo/redo stack・ID generator のいずれも
  変更しない。
- dry-run と実 apply の採番系列が一致する限り、validate を
  通過した composite が apply 段階で ID 不整合により
  失敗することはない（`Composite::apply` の逐次
  validate+apply と同一系列）。
- 既存の全 `ProjectOp` 契約・全既存テスト（43 binaries）
  が非回帰であること。

## Non-goals

- GW-04 scratch-ID/rewrite seam・placeholder → 実 ID の
  rewrite・`synvelis.operation.batch` 非空 `bindings` の
  昇格 — `crates/synvelis-gateway`（sv#265 リース）の
  後続 slice。headless の Domain Oracle PASS を gateway
  経路の実 commit PASS に昇格しない。
- `draw_raw_id` 予約窓を跨ぐ採番系列の一致保証 — watermark
  シードの dry-run が fail-closed reject する既定契約
  として許容（誤 commit なし）。
- 実 OS・実 Plugin・実 RT・`*-msrv` レーン — 対象外
  （NotRun）。
- `Composite::apply`・`ProjectController::apply` commit
  経路・`validate_references` の変更。

## Risk

- validate が member apply を scratch 上で実際に実行する
  ため、composite サイズに比例したコストが validate に
  加わる。逐次化を戻す・制限する変更が入ると
  create-then-reference が再び `NotFound` で失敗し、
  gateway `bindings` 経路が成立しなくなる。
- member の早期 validate 順序・空/nested の語彙・単一
  undo が崩れると、batch の all-or-nothing 契約
  （`design-atomic-macro.md` §2）と GW-06 commit の
  原子性保証が失われる。
