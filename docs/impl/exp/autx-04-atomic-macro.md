---
id: "EXP-autx04atomicmacro001"
type: "experiment"
status: "completed"
title: "AUTX-04 Atomic Macro — 式 AST・binding placeholder・Composite validate seam の試行"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/263"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/autx-04-atomic-macro.md"
  depends-on:
    - "docs/automation/design-atomic-macro.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-04-atomic-macro.md"
    - "evidence/sv263/autx-04-atomic-macro-ledger.md"
  refines: []
  refined-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/automation/design-atomic-macro.md"
    - "crates/synvelis-project/src/ops.rs"
    - "crates/synvelis-project/src/controller.rs"
    - "crates/synvelis-gateway/src/prepare_engine.rs"
    - "crates/synvelis-gateway/src/prepare.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUTX-04 Atomic Macro — 式 AST・binding placeholder・Composite validate seam の試行

## 問題設定

Issue #263（AUTX-04）の headless slice は、versioned
`AtomicMacroDefinition`（Recipe）を runtime input + pinned
revision と共に `synvelis.operation.batch` の `ops[]` へ
lowering し、既存 GW-04/05/06 経路（`ProjectOp::Composite` →
1 CAS / 1 proposal / 1 approval / 1 commit / 1 undo）に載せる
ことが契約（`docs/automation/design-atomic-macro.md`）。
設計上の未確定点は 3 つあった:

1. `$param.` / `$output.` 参照を既存 `Expr` AST にどう載せるか
   — workflow compiler は fail-closed でなければならない。
2. create-then-reference（新規生成 ID を後続 member が参照）
   の ID 解決をどこに置くか — ID 推測・静的ハードコードは禁止。
3. `ProjectOp::Composite` の validate/apply 意味論が逐次参照を
   通すか — 通さない場合の fallback は何か。

## 試行と結果

### E1: 式 AST — 新 variant 追加 vs workflow 側 error のみ

`Expr` に `Param { path }` / `Output { name }` を追加し、
workflow `PlanCompiler::type_of` では `MacroOnlyExpr` で
fail-closed reject、macro 側 `type_of`/`eval_expr` のみが
受理する二層構造を採用。代替案（macro 専用 AST を別型で
持つ）は definition schema・frontend lowering・型照合を
二重化するため棄却。`$param`/`$output` を workflow YAML で
書いても lowering は通るが compile で typed reject になる
ことを fixture で固定した。

### E2: output binding placeholder — 予約 ID レンジ

`$output.<name>` は compile 時に「予約レンジ内の決定的
tagged ID」を emit する。`OUTPUT_BINDING_ID_BASE =
(1<<96)+(1<<32)`、slot は `output_bindings` 宣言順。
`kind-<32hex>` 形式を保つため capability の `id`-kind
schema・target 契約を静的に通過でき、literal/param 値が
予約レンジに入る場合は `ReservedIdLiteral` で拒否
（placeholder との衝突を型で排除）。実 ID の推測は一切
行わない — placeholder は GW-04 が scratch 空間で実 ID へ
rewrite する前提のマーカーである。

### E3: Composite validate seam — 調査でブロッカー確定

`ProjectOp::Composite::apply` は member 毎に evolving
scratch で `validate`+`apply` する逐次対応済みだが、
`Composite::validate`（`synvelis-project/src/ops.rs`）は
全 member を **pre-state** に対して validate し、
`ProjectController::apply` が apply 前にこれを呼ぶ。
`AddLane::validate` は `tracks.contains_key` を検査する
ため、先行 member が作る track を参照する後続 member は
commit 時に `NotFound` で必ず失敗する — prepare 側の
scratch 解決を完璧に実装しても commit が通らない。

`synvelis-project` は別 Worker のリース（sv#290）のため
Coordinator へ範囲拡張を照会したところ否決。
`design-atomic-macro.md` §3 の fallback が確定:
**binding 使用 batch は prepare で typed Unsupported**、
`Composite::validate` の逐次 dry-run 化は Coordinator が
sv#290 完了後の後続 slice として記録する。

### E4: gateway 側 fallback の最小実装

`prepare_engine.rs` の batch input schema に optional
`bindings`（array, max 64, any_obj items）を追加し、
`lower_named` の batch arm 先頭で非空 `bindings` を
`GatewayError::PreparationRejected`（reason に Workflow
分解への誘導を明記）で reject。`outcome.rs` への新 variant
追加はリース外のため既存 typed channel を再利用。
`bindings: []`・欠落は従来通り member lowering へ進む
ことを in-file unit test で固定（schema 受理・空は通過・
非配列は SchemaViolation・非空は PreparationRejected）。

### E5: 実装中に検出した test/実装の齟齬（記録）

- static check が `bind_parameters` より先に走るため、
  param fixture は「宣言型が使用先 field schema と適合」
  でなければ bind 系 error に到達しない — fixture を
  string param に修正。
- workflow 側 `$output` reject は macro 解決後でないと
  `UndefinedMacro` が先に発火する — registry 付き経路で
  検証するよう修正。
- `Id` static type は `String` schema と非互換
  （`type_compatible` 厳密）— 予約レンジ literal の reject
  は target 位置（kind 適合）に置いて eval 層で検証。

## 結論

採用: `Expr` variant 追加 + 予約レンジ placeholder +
`MacroRegistry`/`compile_with_macros` + batch `bindings`
typed reject。棄却: macro 専用 AST 分離、実 ID 予測採番、
`Composite::validate` の本 slice での変更（リース外・
coordinator 判断）。create-then-reference の end-to-end
（scratch rewrite → commit）は `synvelis-project` の
`Composite::validate` 逐次化を待つ後続 slice — 本 slice
では compile 側の placeholder/bindings 契約と gateway の
typed reject までを保証する。
