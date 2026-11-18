---
id: "EXP-561OpFactsUnverifiable08"
type: "experiment"
status: "completed"
title: "Issue #561 impl EXP — op_facts unverifiable cascade 実験記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/561"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-557-adopt-facts.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #561 impl EXP — op_facts unverifiable cascade 実験記録

## 実験サマリ

設計上の代替案を検討し、採否を記録する。実装は
`RemoveSend` arm の確立パターン踏襲で確定した。
詳細なイベントログは `issue-561-opfacts-unverifiable-cascade-
exp.jsonl` を参照。

## 採否記録（exp.jsonl と対応）

| EXP | 仮説 | 結果 | 要点 |
| --- | --- | --- | --- |
| EXP-1 | `SetSendGainTarget` の unsupported は send closure 全体を検査すべき | rejected | release が消すのは旧 target に触れる assignment のみ。send-owned source は release で削除されない（独立 entity として生存）— closure 全体検査は過剰拒否になる |
| EXP-2 | `monitoring.tracks` decl・strip chain/ports も unsupported トリガに含めるべき | **採用（decl のみ）** | 初版は decl を「strictly-1:1 companion で受理内」と判断したが、review で `smf_documents` との非整合が指摘された — decl は optional で独立 content を持つ keyed companion であり同一修正の SMF 判断と矛盾。`contains_key` 検査を追加。strip chain/ports は必須内部構造として引き続き受理内 |
| EXP-3 | `RemoveSend`/`RemoveTrack`/`RemoveMidiSource` を単一 helper に統合すべき | rejected | `smf_documents` 検査は control-closure とは別種の cascade — 表面的類似の誤抽象化を避け、`ControlOwner` 版 helper は RemoveSend/RemoveTrack のみに限定 |
| EXP-4 | `RemoveSend` arm を helper 抽出で書き換えると observable 挙動が変わる恐れ | rejected | 同一収集・判定・文言を維持する semantics-preserving 抽出。cb06 sv#550/sv#553 pin が非回帰を実測 |
| EXP-5 | optional domain pin test は不要（gateway pin のみで十分） | rejected | domain の発火条件は `ops.rs` の既存動作であり mirror の正本。domain pin で発火条件自体を固定し gateway mirror の乖離を防ぐ価値があるため採用 |
| EXP-6 | wire pin test を `cb06_strip_gateway.rs` に追加すべき | rejected | 同ファイルは sv#559 占有 — `tests/prepare.rs` にファイルローカル fixture で再構成（共有 `rig` fixture も非接触） |
| EXP-7 | Composite member を pinned base のまま評価し続けるべき | rejected | `Composite::apply` は逐次だが `op_facts` は全 member を base に対して評価 — batch 内 seal→release（`set-gain-target` pair、`AddSend`+`RemoveSend`）で unverifiable cascade を見逃す under-report。review で実シナリオを指摘され、`lower_bound_batch` と同型の逐次 scratch 評価に修正 |
| EXP-8 | `SetProcessorBinding` の plugin-instance 退役 cascade も本 slice で修正すべき | rejected | main 既存の別クラス（persisted plugin identity で routing topology ではない）。本 Issue の wire cascade 範囲外として IR 残件に記録し別 Issue 検討に委ねる |

## 関連リンク

- IR: `docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md`
- 検証: `evidence/sv561/verification.md`・`evidence/sv561/reproduction.md`
