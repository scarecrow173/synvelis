---
id: "EXP-565ProcessorBindingCascade08"
type: "experiment"
status: "completed"
title: "Issue #565 impl EXP — 退役 cascade 監査の実験記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/565"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/issue-565-processor-binding-cascade.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-565-processor-binding-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-561-opfacts-unverifiable-cascade.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-565-processor-binding-cascade.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "synvelis#128"
  deferred-by: []
---

# Issue #565 impl EXP — 退役 cascade 監査の実験記録

## 実験サマリ

監査 slice のため「実装の代替案」ではなく「着地形態の
代替案」を検討した。store cascade は不変条件健全のため
動作変更なし、契約明記 + test pin で着地する方針を
exp.jsonl と対応する表で記録する。

## 採否記録（exp.jsonl と対応）

| EXP | 仮説 | 結果 | 要点 |
| --- | --- | --- | --- |
| EXP-1 | store cascade 自体に `unsupported`/op_facts 検査を追加し fail-closed にすべき | rejected | 外部から persisted `PluginInstanceId` を参照する経路（wire `ResourceRef`・op target/precondition・VCS `Removed` 適用）が構造的に存在せず dangling 実害なし。`unsupported` は op を無効化する過剰拒否で既存受理の回帰になる。insert-processor/send 系の「internal structure」posture コメント明記で着地 |
| EXP-2 | `PluginStateProbe` の staleness を本 slice で修正すべき | rejected | in-tree 実装は fixture のみで実害実測なし。`dependency.rs` は lease 外であり port 契約の世代区別は TIM-04（`synvelis#128`）の設計に属する。defer 判定を IR 残件に記録 |
| EXP-3 | `ResourceKind`/`ENUMERATED_KINDS` に `PluginInstance` kind を追加し名指し可能にすべき | rejected | wire schema・snapshot kind・VCS object model 全体に波及する拡張。dangling 実害がない現状では過剰設計であり、「名指せない」こと自体が外部参照不能＝dangling なしの根拠 |
| EXP-4 | wire pin test を `cb06_strip_gateway.rs` に追加すべき | rejected | 同ファイルは sv#559 占有。`tests/prepare.rs` にファイルローカル fixture（`sv565_processor_controller`）で再構成し共有 `rig` fixture と占有パスを非接触に維持 |
| EXP-5 | `ops.rs` apply arm のコメントも変更すべき | rejected | apply arm 内コメントは既に「post-apply oracle re-checks the bijection」まで正確に明記済み。op 定義側 doc comment に退役 id 非再発行の 1 句を補強するのみで十分 |

## 関連リンク

- IR: `docs/impl/ir/issue-565-processor-binding-cascade.md`
- 検証: `evidence/sv565/verification.md`・`evidence/sv565/audit.md`
- 発見元: `docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md`（残件節が本 Issue の起票根拠、EXP-8 で defer 判定済み）
