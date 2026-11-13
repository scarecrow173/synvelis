---
id: "PLAN-jelz45WhvUPsZIVEaQyYpJ"
type: "plan"
status: "approved"
title: "PC-C4 plugin binding plan"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/pc-c4-plugin-binding-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-plugin-binding.md"
    - "docs/designs/pc-c4-plugin-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-plugin-binding.md"
    - "docs/designs/pc-c4-plugin-binding-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-plugin-binding.md"
  verified-by:
    - "docs/test-specs/pc-c4-plugin-binding.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4 plugin binding plan

## ファイルマップ

### synvelis-project

- `src/host_param.rs`: `PluginParameterInstanceBinding` struct と
  `plugin_parameter_instance_binding(project, target)` を追加。
  stage-1 `semantic_plugin_binding` + persisted
  `PluginInstanceIdentities::resolve` の合成。
- `src/lib.rs`: 新規型・関数を export。
- `tests/ctl_02.rs`: canonical mapping の解決成功・
  `instance: None`（store 欠落 / plugin drift / 別 plugin
  再発行）ケースを追加。

### synvelis-player

- `src/binding.rs`: `PluginBindingSession::bind` が
  `plugin_parameter_instance_binding` を使い、plugin 照合の後に
  instance authority を照合（`None`/不一致 →
  `InstanceMismatch`）。
- `tests/pc_c4_binding.rs`: fixture の `project_with` が
  `plugin_instances.ensure` で store を populate。instance pin を
  persisted id と一致させる。2 processor fixture で「同 raw
  ParamId・別 instance」の誤配送なしを維持。新規テストで
  instance authority の typed reject（store 欠落・id 再発行・
  A→B→A drift）を pin。
- `tests/pc_c6_oracle_pl.rs`: 同様に fixture を persisted id へ
  整合。pl04 reload は「同一 PluginRef の再 attach = 同一
  instance id・新世代」で model 化し、旧 record は世代軸で
  reject されることを pin。
- `tests/pc_c4_binding_rt.rs`（新規）: session-bound dispatch
  （bind → eval_event → dispatch staging → `process` 配送）の
  RT alloc-free/non-blocking fixture。

### 証拠・文書

- `evidence/sv101/`: 検証ログ・外部 gate（実 VST3/OS = NotRun）
  の記録。
- `docs/impl/ir|exp/pc-c4-plugin-binding.md`: 実装記録・実験記録。
- 各 `docs/<type>/README.md` index を doc-maintenance で再生成。

## 順序

1. project: canonical mapping + export + テスト（TDD: 赤確認）。
2. player: `bind` 照合 + fixture 整合 + instance reject テスト。
3. RT fixture 追加。
4. `mise run fmt-check` → `check|lint|test`（両 package）→
   `test-rt`。
5. impl 記録・evidence・index 再生成・review-protocol。

## リスク

- fixture の instance pin 変更は広範囲に波及する — `open` helper
  を persisted id 解決へ寄せ、2-instance scenario は別
  `ProcessorId` + store entry で model 化して誤配送なし契約を
  維持する。
- `bind` の照合順序を誤ると `PluginMismatch` の報告が後退する
  — plugin 照合を instance 照合より先に行う順序を test で pin。
