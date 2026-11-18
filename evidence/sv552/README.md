# evidence/sv552 — SetSendGainTarget seal 解放（synvelis#552）

## 概要

`ProjectOp::SetSendGainTarget` の Some→Some rebind /
Some→None unconfigure で、旧 `SendLevel` seal を
apply 内で解放し singleton 不変条件を保つ修正の
headless 検証証拠。

## 検証結果サマリ

- `mise run test synvelis-project`: Pass
  - `ctl_02::send_gain_rebind_releases_the_previous_seal`
  - `ctl_02::send_gain_unconfigure_then_rebind_leaves_no_orphan_seal`
  - `ctl_02::send_gain_rebind_via_routing_intent_releases_the_seal`
  - `cb06_strip_fields::cb_strip_550_sends_ops_mask_gated`
    （rebind 節を positive candidate edit 検証へ置換）
- `mise run check synvelis-project`: Pass
- `mise run lint synvelis-project`: Pass
- `mise run fmt-check`: Pass
- `mise exec -- prek run --all-files`: Pass

## NotRun / Unsupported

- 実 PCM/plugin・実 GUI・RT lane: 本 slice は domain
  semantics の headless 修正であり対象外。
- gateway wire positive 実測の追加（`cb06_strip_gateway.rs`
  への rebind 試験）: `synvelis#553` 占有のため本 slice
  では実施しない。
