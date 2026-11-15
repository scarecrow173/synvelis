---
id: "EXP-cb10-gateway-contract"
title: "CB-10 gateway typed contract 実験メモ (synvelis#287)"
created: "2026-11-15"
updated: "2026-11-15"
---

# 実験記録: CB-10 Creative Branch gateway typed contract slice（synvelis#287）

## E1 variant mutation を `ProjectOp` に昇格させるか、別 intent として運ぶか

**仮説**: `synvelis.branch.*` の mutation を既存 `ProjectOp` 表に
載せれば、prepare/effects/authorization をそのまま再利用できる。

- 調査: `Operation::apply` の signature は
  `(&mut Project, &mut dyn IdGenerator)` のみ。fork/edit/adopt は
  `DomainAdapter`・`BranchContentStore`・`SnapshotResolver`・
  `AdoptSources`・`RuntimePublish` を必要とし、`ProjectOp` では
  運べない。`VariantOp` は `synvelis-project` 内で `pub(crate)`。
- 案 A（`ProjectOp` 昇格）: port を trait object として
  `ProjectOp` variant に持たせると wire serde・batch member・
  preview 意味論すべてが崩れる。却下。
- 案 B（`PreparedMutation::Variant` + `GateDeps.variant`）:
  lowering は named-op 表を通し、prepare は scratch
  `ProjectController` で domain engine を実走、commit は既存
  fence 内で `variant::execute` に dispatch。採用。

**観察**: 案 B で authorization/effects/CAS/journal/settle を
verbatim 再利用でき、`cb10_variant_actions` 14 件で契約を確認。

## E2 prepare 時の効果導出を宣言で済ませるか、simulation で導出するか

**仮説**: variant intent の effect bound は descriptor の
`EffectSet` 宣言で十分であり、before/after diff を取る必要はない。

- 調査: effect-aware authorization（SEC-14 unverifiable fail-
  closed、scope coverage、create axis）は derived effects を要求
  する。宣言のみでは `adopt` が live scope を書き換える事実を
  grant scope 照合に反映できない。
- 案 A（宣言のみ）: authorization が実 post-state を見ない。
  却下。
- 案 B（scratch simulation → resource map diff）: live を触らず
  real post-state から effects を導出。採用。

**観察**: `ScratchStore` は shared store へ read-through・write
は local に留め、simulation が committed payload を orphan
させないことを確認。`adopt` の
`expected_project_revision` は scratch の commit counter が live
と異なるため scratch tip に pin（project-rev CAS は commit fence
で再検査）。

## E3 adopt の approval ticket を wire args に載せるか

**仮説**: `synvelis.branch.adopt` の args に approval ticket を
含めれば domain contract を直接満たせる。

- 調査: approval authority は gateway の approval receipt が所有
  し、caller が ticket を自己申告すると「approval を推測しない」
  不変条件を破る。
- 案 A（args に ticket）: agent が任意 ticket を捏造可能。却下。
- 案 B（commit gate が consumed receipt id を注入）: lowering は
  `SIMULATION_TICKET` placeholder、commit は実 receipt id を
  `ApplyApproval.ticket` に代入。receipt 不在なら domain が
  `Unapproved` で fail-closed。採用。

**観察**: `DelegatedApply`（receipt 無し）では prepare/apply が
通っても fenced commit が `unapproved` で determined reject、
`ApproveToApply` + human approve では consumed receipt で commit
成功。`adopt_without_approval_fails_closed`・
`adopt_with_consumed_receipt_commits` で確認。
