---
id: "EXP-autx04f01compositefwd001"
type: "experiment"
status: "completed"
title: "AUTX-04-F01 Composite validate 逐次 dry-run — scratch clone・watermark シード採番の試行"
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
  implemented-by:
    - "docs/impl/ir/autx-04-f01-composite-forward-ref.md"
  depends-on:
    - "docs/automation/design-atomic-macro.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-04-f01-composite-forward-ref.md"
    - "evidence/sv432/composite-forward-ref-ledger.md"
    - "docs/impl/exp/autx-04-atomic-macro.md"
  refines: []
  refined-by: []
  verifies: []
  verified-by: []
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
    adopted: []
    rejected: []
---

# AUTX-04-F01 Composite validate 逐次 dry-run — scratch clone・watermark シード採番の試行

## 問題設定

Issue #432（AUTX-04-F01）は `ProjectOp::Composite::validate`
の逐次 dry-run 化を求める。autx-04 exp E3 で確定した
blocker — validate が全 member を pre-state に対して検査する
ため、先行 member が mint する ID を後続 member が参照する
create-then-reference が commit 時に `NotFound` で失敗する —
を解消しつつ、member 毎の早期 validate 契約・API・非
composite 挙動・単一 undo を不変に保つ必要がある。

設計上の未確定点は 2 つあった:

1. dry-run で member apply が mint する ID を何に揃えるか —
   `validate(&self, project: &Project)` は controller の
   live `IdGenerator` にアクセスできない。
2. `validate` 内で `apply` を呼ぶことの安全性 — apply が
   project/generator 以外に副作用を持たないか、reject
   順序・語彙が変わらないか。

## 試行と結果

### E1: 事前走査で mint 予定 ID を収集する案 — 棄却

全 member をまず pre-state で validate し、create 系 op が
mint する ID を「出現順の watermark 相対 slot」として
事前計算し、後続 member の参照を書き換えてから逐次検査する
案。ID の書き換えは op の内部表現に踏み込む必要があり、
`ProjectOp` の全 variant の ID 保持 field を網羅列挙する
保守コストと、参照が ID 以外の形（routing port 等）で
現れる場合の取りこぼしリスクがある。`Composite::apply`
が既に逐次 `validate`+`apply` を行っている以上、validate
側も同じ逐次形に揃える方が責務境界として正しいため棄却。

### E2: scratch clone + watermark シード `CounterIdGenerator`

`Composite::apply` と同型に、`project.clone()` した scratch
上で member 毎に `op.validate(&scratch)?;` →
`op.apply(&mut scratch, &mut dry)?;` を実行する案を採用。
`dry` は `crate::id::CounterIdGenerator::new(
project.id_watermark)` — `Routing` variant preflight
（`ops.rs`）と `preview_routing`（`controller.rs`）が既に
使う「watermark シードで non-colliding な raw を発行し
live ID を消費しない」先例と同一。

検証した性質:

- **採番系列の一致**: commit 成功後は
  `next.id_watermark = generator.peek()` により
  `live.id_watermark` と generator 位置が一致する。
  この不変条件下では dry-run（watermark シード）と実
  apply（live generator）が同一の raw 系列を発行し、
  forward-reference を構成する側は `peek_raw_id()` で
  次回発行 raw を正確に予測できる
  （`tests/composite.rs` で実測）。
- **watermark > 全 live entity ID**:
  `check_id_watermark` が load 境界で保証する不変条件
  （`id_watermark` は全発行済み entity ID を上回る）により、
  dry-run が mint する ID は live に衝突しない。
- **副作用の局所性**: `Operation::apply` の signature は
  `(&self, &mut Project, &mut dyn IdGenerator)` のみで、
  `CounterIdGenerator` は validate 内ローカル — live
  project・live generator・revision・undo/redo のいずれにも
  触れない。`SetProjectPpqn` validate が既に
  `reproject_ppqn` を scratch clone 上で dry-run する前例
  がある。
- **reject 順序・語彙**: `ops.is_empty()`→
  `"empty composite"`、member 毎の nested 検査→
  `"nested composite"` を member の validate 前に維持し、
  語彙・優先順位を不変に保つ（`composite_rejects_empty_
  and_nested_with_stable_vocabulary` で固定）。

### E3: 予約窓での系列ずれ — fail-closed で許容

`draw_raw_id`（外部予約、例: variant engine の
`ReservingIdGenerator`）が commit を挟まずに連続すると、
generator 位置が `live.id_watermark` を上回る。この窓で
forward-reference composite を validate すると、dry-run
（watermark シード）と実 apply（先行 generator）の採番
系列がずれ、参照先 ID が一致せず validate が `NotFound`
で reject する。これは fail-closed であり誤 commit を
生まない。予約を跨ぐ batch の end-to-end（GW-04 側で
予約 ID を織り込んだ placeholder 解決）は gateway seam
の後続 slice の責務とし、本 slice では watermark シード
の既定契約として記録する。

## 結論

採用: `project.clone()` scratch + member 毎
`validate`+`apply` + `CounterIdGenerator::new(
project.id_watermark)`。棄却: 事前走査による mint ID 収集
（op 内部表現への侵襲・取りこぼしリスク）。create-
then-reference の project 側前提は headless で実 commit
まで検証済み。gateway 側の `bindings` 昇格・placeholder
rewrite は `synvelis-gateway` リースの後続 slice
（NotRun/Deferred — `evidence/sv432/` 台帳参照）。
