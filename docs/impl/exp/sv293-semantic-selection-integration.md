---
id: "EXP-5pWM3x9cayvrWvjFallCgj"
type: "experiment"
status: "completed"
title: "sv293 semantic→selection integration 実装試行 (synvelis#293 deferred residual slice)"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/293"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/sv293-semantic-selection-integration.md"
  depends-on:
    - "docs/designs/sv293-semantic-selection-integration.md"
    - "docs/discovery/sv293-semantic-selection-integration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv293-semantic-selection-integration.md"
    - "docs/impl/ir/sv293-semantic-selection-integration.md"
    - "evidence/sv293/acceptance-ledger.md"
    - "evidence/sv293/verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv293-semantic-selection-integration.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/exp/sv293-semantic-selection-integration-exp.jsonl"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
---

# sv293 semantic→selection integration 実装試行

詳細な試行ログは `sv293-semantic-selection-integration-exp.jsonl`
を参照。

## 要約

1. **依存スコープの確認**: RCP 契約型が `ruviform_graph`/
   `ruviform_audio_core` 型であることを確認し、
   `synvelis-workspace` への workspace 継承依存 2 件を
   Coordinator 承認のもと追加。
2. **producer 初版の自己レビュー**: `strip_plumbing` 未宣言・
   artifact format 固定・re-export 崩れ・dead field を検出し、
   module を全面書き直し。
3. **compile 修正**: `RoutingCompileRequest` の import 経路を
   `synvelis_project` re-export へ修正、`NodeBuild::into_node`
   追加。`cargo check -p synvelis-workspace` exit 0。
4. **oracle test 組立**: `ProjectController` + `ProjectOp` で
   semantic routing model を構築する fixture を設計。
   `StaticProcessorHost::connect` の consume 意味論を発見し、
   drive 毎の host 再武装を test helper に組み込んだ。
5. **EdgeKind::Return の向き修正**: 当初 destination strip 判定
   だったが、「Return-bus feed」は return bus → mix 方向
   （source strip が Return bus 所有）と解釈を修正
   （`docs/test-specs/sf06-cross-gate.md` の記述と整合）。
6. **全 9 test green**: `cargo test -p synvelis-workspace
   --test rcp_selection_integration` で 9/9 Pass。
