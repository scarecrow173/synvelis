---
id: "EXP-FTOy7bEUZK7ZwyBLcl5Bp"
type: "experiment"
status: "completed"
title: "issue-274-cli-script-analyze — 代替実装の実験なし記録"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/274"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/issue-274-cli-script-analyze.md"
  depends-on:
    - "docs/designs/issue-274-cli-script-analyze.md"
    - "docs/specs/issue-274-cli-script-analyze.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/issue-274-cli-script-analyze.md"
    - "evidence/sv274/issue-274-cli-script-analyze-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/exp/issue-274-cli-script-analyze-exp.jsonl"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# issue-274-cli-script-analyze — 代替実装の実験なし記録

## 問題設定

Issue `scarecrow173/synvelis#274` の headless CLI slice
（SV274-NR-03）: `synvelis-cli script analyze --file` の verb
配線。doc chain の必須項として experiment log を要求されるが、
本 slice では**代替実装のコード実験は行っていない**。理由は
下記「採用した判断」のとおり、設計上の不確実点が全て Spec /
discovery 段階で解消済みであり、実装は merge 済みの既存 seam
をそのまま consume するだけだったため。

## 試行と結果

実験イベントなし。`docs/impl/exp/issue-274-cli-script-analyze-exp.jsonl`
には start/summary の 2 event のみを記録し、「no experiments」
の意図を明示した。

### 設計段階で解消済みだった論点（実験不要の根拠）

- **verb 命名**: `script analyze --file` は Spec で確定済み
  （`action analyze`/`workflow analyze`/`--analyze` flag の
  却下理由は spec「実装指針」節 — design で再検討しない
  取り決め）。
- **pipeline 経路**: `boot_discovery()` ScriptStack →
  `LuauStub`/`DefinitionInjection` → `AnalyzerRegistry` →
  `analyze_with_injection` → `verify_revision` は全て
  `b5a8919`（origin/main）に merge 済みの公開契約で、
  消費側の選択肢が存在しない。
- **extension 分類**: `script::classify` の kind 軸を流用
  しない判断は design で確定（analyze 専用の小さな
  `classify_language`）。
- **error 写像・envelope 形状**: `AnalyzerError`→`CliError`
  の typed 写像と result payload 形状は design で確定し、
  e2e golden で pin。

## 採用した判断

- **実験を捏造しない**: 「試したが採用しなかった代替実装」が
  存在しないのに event を並べることは ledger の正直性に反する
  ため、jsonl は start/summary のみとし、却下案は spec/design
  文書側の記録に委ねた。
- **既存 seam のみ使用**: 新規依存・他 crate 変更・第二
  registry/authority の追加は行わず、`synvelis-cli` 内の
  parse/dispatch/envelope 層のみを拡張した。

## 結論

代替実装の実験は行われなかった — 全ての設計判断は Spec 確定
または design 確定であり、実装は検証済みの既存契約の配線のみ。
実装・検証の記録は `docs/impl/ir/issue-274-cli-script-analyze.md`
と `evidence/sv274/issue-274-cli-script-analyze-ledger.md` を
参照。
