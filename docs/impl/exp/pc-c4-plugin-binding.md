---
id: "EXP-cx1Q53tw8M19hBnYGHDWFo"
type: "experiment"
status: "completed"
title: "PC-C4 plugin binding experiment log"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/impl/ir/pc-c4-plugin-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments: []
---

# PC-C4 plugin binding — experiment log

## EXP-1: `instance` を `Option` にする vs fail-closed `Result`

### 問い

`plugin_parameter_instance_binding` が store の vouch 失敗
（欠落・PluginRef drift・範囲外 id）を `Err` で返すべきか、
`PluginParameterInstanceBinding.instance: Option` で返すべきか。

### 検討した選択肢

1. **`Err(Error::Validation)` で fail-closed** — mapping 関数単体
   としては一貫するが、`bind` が plugin identity 照合
   （`PluginMismatch`）を instance 照合より先に報告する既存契約を
   崩す。project の plugin rebind 時、semantic.plugin は live
   binding を指すため instance 解決（旧 `(processor, plugin)`
   pair に対する store lookup）が先に失敗し、`PluginMismatch`
   ではなく project error が報告される。
2. **`instance: Option<PluginInstanceId>`** — semantic 解決失敗は
   従来どおり `Err`、store vouch 失敗は `None`。`bind` は
   `PluginMismatch` → `InstanceMismatch` の順序を維持できる。
   `PluginInstanceIdentities::resolve` 自体が `Option` を返す
   store 契約とも一致する。
3. semantic と instance を別関数に分け、`bind` が 2 回呼ぶ —
   正本が分散し、呼び出し側が合成順序を再実装する余地が残る。

### 結果

選択肢 2 を採用。`pc_c4_02_instance_authority_bind_gate` が
`PluginMismatch` 優先・`InstanceMismatch` fail-closed・A→B→A
再発行の 3 経路を pin し、既存 `pc_c4_02_stage1` /
`pc_c4_02_dispatch_plugin_pin` の PluginMismatch 契約が非回帰で
あることを確認した。

### 却下理由（選択肢 1・3）

- 選択肢 1: typed error の報告順序という観測可能な契約を退行
  させる。store lookup 失敗と semantic 失敗を同一 `Err` に畳むと
  consumer が原因を区別できない。
- 選択肢 3: 「Mapping 正本を一箇所に置く」というタスク要件に反し、
  bind 以外の consumer が正しい合成を再発明するリスクがある。
