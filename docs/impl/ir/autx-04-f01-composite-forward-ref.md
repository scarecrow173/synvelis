---
id: "IMPL-034XihWxPpf0evq4eGpxAa"
type: "impl"
status: "completed"
title: "AUTX-04-F01 Composite forward reference sequential validate"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/432"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/composite.rs"
      - type: "file"
        path: "docs/impl/exp/autx-04-f01-composite-forward-ref.md"
      - type: "file"
        path: "docs/test-specs/autx-04-f01-composite-forward-ref.md"
      - type: "file"
        path: "evidence/sv432/composite-forward-ref-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
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
    - "docs/impl/exp/autx-04-f01-composite-forward-ref.md"
    - "docs/test-specs/autx-04-f01-composite-forward-ref.md"
    - "evidence/sv432/composite-forward-ref-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/autx-04-f01-composite-forward-ref.md"
  references:
    - "docs/automation/design-atomic-macro.md"
    - "crates/synvelis-project/src/ops.rs"
    - "crates/synvelis-project/src/controller.rs"
    - "crates/synvelis-project/src/id.rs"
    - "crates/synvelis-gateway/src/prepare_engine.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-04-f01-composite-forward-ref.md"
    rejected: []
---

# AUTX-04-F01 Composite forward reference sequential validate

## Summary

Issue #432（AUTX-04-F01）の headless 実装スライスとして、
`ProjectOp::Composite::validate` を逐次 dry-run 化した。
従来は全 member を pre-state の `project` に対して validate
していたため、先行 member が生成する ID を後続 member が
参照する create-then-reference（`OutputBinding`→placeholder
ID の解決先）が `NotFound` で reject されていた。変更後は
scratch clone 上で member 毎に `validate`+`apply` を逐次実行
し、`Composite::apply` と同じ member 順・同じ失敗伝播で
composite 内の前方参照を許可する。`validate` のシグネチャ・
`Operation`/`ProjectOp` の API・非 composite arm の挙動・
単一 undo セマンティクスは不変。

## Context

AUTX-04（synvelis#263、PR #429 統合済み）で
`synvelis.operation.batch`→`ProjectOp::Composite` lower と
非空 `bindings` の typed `PreparationRejected` fallback が
入ったが、`Composite::validate` が pre-state 一括検査のまま
では、GW-04 の scratch-ID/rewrite seam が placeholder を実
ID へ解決しても commit 時に `NotFound` で失敗する
（`docs/automation/design-atomic-macro.md` §3、
`docs/impl/exp/autx-04-atomic-macro.md` E3 で blocker 確定済み）。

`Composite::apply`（`ops.rs`）は既に member 毎に evolving
scratch で `validate`+`apply` する逐次実装であり、
`ProjectController::apply` の commit 経路
（`op.validate(&live)` → clone → `apply(&mut next, gen)` →
`next.validate_references()`）は変更していない。

## Implementation

### Changes

- `crates/synvelis-project/src/ops.rs` —
  `Self::Composite { ops }` validate arm を逐次 dry-run へ
  変更。`ops.is_empty()`→`Validation("empty composite")` と
  nested `Composite`→`Validation("nested composite")` の
  reject 順序・語彙は不変。その後
  `let mut scratch = project.clone();` と
  `let mut dry = crate::id::CounterIdGenerator::new(
  project.id_watermark);` を用意し、member 毎に
  `op.validate(&scratch)?; op.apply(&mut scratch, &mut dry)?;`
  を実行して scratch を破棄する。dry 採番は
  `ops.rs` の `Routing` variant preflight と
  `controller.rs` `preview_routing` と同じ「watermark
  シードで non-colliding な raw を発行し live ID を消費
  しない」セマンティクス。
- `crates/synvelis-project/tests/composite.rs`（新規）—
  6 integration tests。create-then-reference の forward
  reference が `ProjectController::apply` で commit まで通り
  単一 undo/redo で戻ること、asset→source の別 kind でも
  成立すること、member 早期 validate・mid-member 失敗の
  atomicity・empty/nested の語彙・stale revision の各
  回帰契約を固定。
- `docs/impl/exp/autx-04-f01-composite-forward-ref.md`（新規）
  — dry-run 化の設計判断と採番シードの検討を記録。
- `docs/test-specs/autx-04-f01-composite-forward-ref.md`
  （新規）— Composite sequential validate が保証する不変条件
  を固定。
- `evidence/sv432/composite-forward-ref-ledger.md`（新規）
  — Case ID 付き結果台帳。gateway 側 bindings 昇格試験・
  実 OS/実 Plugin/実 RT は NotRun/Deferred として記録。

### Decisions During Implementation

- **dry 採番シードは `project.id_watermark`**:
  `validate(&self, project: &Project)` は controller の
  live generator にアクセスできないため、既存先例
  （`Routing` preflight・`preview_routing`）と同じく
  永続化 watermark をシードに使う。commit 後は
  `next.id_watermark = generator.peek()` により
  `live.id_watermark` と generator 位置が一致するため、
  forward-reference を構成する側は `peek_raw_id()` で
  次回発行 raw を予測できる（`draw_raw_id` による予約が
  残る窓では generator が watermark を先行し得るが、
  その場合は validate が fail-closed で reject するのみで
  部分 commit は起きない）。
- **member 毎の reject 順序を維持**: nested 検査は各
  member の validate 前に置き、空 composite の判定を
  先頭に残すことで、従来のエラー優先順位・語彙を変えない。
- **`op.apply` の失敗も validate から伝播**: dry-run が
  `Composite::apply` と同じ member 順・同じ `?` 伝播を
  踏襲するため、apply 段階でしか顕在化しない失敗も
  validate で早期に同じ error として返る。commit 経路の
  atomicity（失敗時 `next` 破棄）は不変。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-04-f01-composite-forward-ref.md` —
  scratch clone + watermark シード dry generator による
  逐次 validate+apply の採用。却下した代替案（事前走査・
  placeholder 特別扱い）は exp に記録。

### Rejected

- なし。

## Validation

- `mise run fmt-check` — Pass。
- `mise run check synvelis-project` — Pass。
- `mise run lint synvelis-project` — Pass（clippy
  `-D warnings`、`--all-targets`）。
- `mise run test synvelis-project` — Pass（43 test
  binaries・0 failure。新規 `tests/composite.rs` 6 件含む）。
- `mise run check synvelis-workspace` — Pass
  （`synvelis-project` 直接依存の依存先整合）。
- `mise run check synvelis-render` — Pass（同上）。
- `*-msrv` レーン・実 OS/実 Plugin/実 RT 経路は対象外
  （NotRun）。

## Risks

- dry-run は member apply を scratch 上で実際に実行するため、
  composite サイズに比例した clone+apply コストが validate
  に加わる。`Composite::apply` 側も同じ逐次処理を行うため
  オーダーは変わらないが、大規模 composite では validate が
  2 回分の適用コストを持つ。
- `draw_raw_id` 予約直後の窓では generator 位置が
  `live.id_watermark` を上回り、watermark シードの dry-run
  と実 apply の採番系列がずれ得る。この場合 forward-
  reference composite は validate で fail-closed reject
  される（誤 commit はしない）。予約を跨ぐ batch の
  end-to-end は GW-04 側 seam の後続 slice で扱う。

## Follow-ups

- GW-04 scratch-ID/rewrite seam（placeholder → 実 ID）と
  `synvelis.operation.batch` の非空 `bindings` 昇格 —
  `crates/synvelis-gateway`（sv#265 リース）側の後続 slice。
  本 slice は project 側前提（forward-reference composite が
  `ProjectController::apply` で実 commit されること）のみを
  headless で証明した。
- Issue #432 の Close 判定は Coordinator が行う
  （`Refs #432`、PR では `Closes` を使わない）。
