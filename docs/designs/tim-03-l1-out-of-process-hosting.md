---
id: "DESIGN-b6Kd2Wn8Qm4Xr7TyZuC5"
type: "design"
status: "approved"
title: "TIM-03 L1 out-of-process hosting production wiring"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim-03-l1-out-of-process-hosting.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-real-abi-process-oracle.md"
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
---

# TIM-03 L1 out-of-process hosting production wiring — Design

## 概要

設計は 3 層に分ける。

1. route registration seam: production composition が typed な `OutOfProcessSpec`
   を受け取り、`PluginHost::register_out_of_process` へ渡す。
2. test-only runner provisioning: upstream runner binary が dependency edge 経由で
   提供されると仮定せず、Synvelis workspace が runner executable を build する。
3. limited process oracle: production activation と direct endpoint process を分け、
   defined-output membership だけを正直に評価する。

## route registration seam

既存 `compose_routing_engine` は in-process factory 登録の意味を変えない。新規
`compose_routing_engine_with_out_of_process` は `Vec<ProductPlugin>` に加えて
`Vec<ProductOutOfProcessPlugin>` を受け取る。`ProductOutOfProcessPlugin` は
`OutOfProcessSpec` と静態 `ParamInfo` table を束ねるだけで、descriptor や
lookahead を Synvelis 側で再解釈しない。

`PluginHostProcessors::product_with_out_of_process` は、まず in-process factory を
登録し、次に L1 spec を `register_out_of_process` へそのまま登録する。duplicate
plugin id は `HostError` として composition caller へ返し、engine attach 前に失敗
させる。capability generation は既存どおり `HostCommand::Scan` 結果を fold する。
L1 route も scan に現れるため、route 追加・削除は drift detection から隠れない。

## runner provisioning

`crates/synvelis-plugin-test-l1-runner` は product runtime の依存ではない test-only
workspace member である。VST3/CLAP それぞれの runner binary が production host-side
loader（`Vst3Library` / `ClapLibrary`）で fixture cdylib を dlopen し、scan した
factory を `ruviform_plugin_export::run` へ渡す。

`ruviform-plugin-export` の wire-v2 以降の argv contract は `<runner>` のみで、
consumer 独自 argv を許さない。そのため fixture path は argv ではなく、
`SYNVELIS_TIM03_L1_PLUGIN_DIR` または runner executable と同じ directory に置かれた
un-hashed cdylib から解決する。oracle test は resolved artifact を runner directory
へ un-hashed name で stage する。既存 file の hash が resolved artifact と異なる
場合は stale target directory として fail し、黙って上書きしない。

## process oracle の境界

direct endpoint oracle は L1 route を `PluginHost` へ登録し、Scan → Instantiate →
Initialize → Activate → StartProcessing → `prepare_endpoint` を行う。512 frames、
lookahead 2 のため latency は 1024 frames と期待する。最初の 2 block は定義済み
silence とし、その後の block は host audio counter で matched/substituted を分類
する。

- `passthrough_submits` delta が 0 の block だけを matched delivery とする。
- matched block では `out = input * gain` の履歴集合に各 sample が属することを
  bit-exact に確認する。
- substituted/pass-through block は定義済み動作だが fresh child delivery の証拠に
  数えない。
- `BusReport::Unavailable` は bus layout unverified として記録し、`Reported([])`
  の意味に使わない。

production oracle は `compose_routing_engine_with_out_of_process` を通し、実 project
mutation → routing build → activation receipt `Active` を確認する。さらに同じ
production session で `RemoveProcessor` を commit し、replacement generation の RT
activation と retired generation の `plugin_state_handles` 空化を観測する。これは
generation retirement / lease teardown の independent evidence である。endpoint
object detach 自体は upstream API 上 ownership `Drop` であり detach receipt がない
ため、この slice では direct endpoint の drop → Stop/Deactivate/Destroy を明示 detach
evidence とし、production endpoint object detach を same-instance re-attach oracle
とは主張しない。

## 不変条件

- production crate は runner crate へ依存しない。
- runner は test-only で、GUI/product binary から参照しない。
- L1 lookahead を precision exact へ格上げしない。
- L0 negative boundary と L1 positive boundary を同じ test case/evidence に混ぜない。
