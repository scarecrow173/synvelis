---
id: "DESIGN-pcC4L1RealAbi01"
type: "design"
status: "approved"
title: "PC-C4 L1 real ABI binding design"
created: "2026-11-26"
updated: "2026-11-26"
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
    - "docs/specs/pc-c4-l1-real-abi-binding.md"
  implemented-by:
    - "docs/plans/pc-c4-l1-real-abi-binding.md"
  depends-on:
    - "docs/specs/pc-c4-l1-real-abi-binding.md"
    - "docs/designs/tim-03-l1-out-of-process-hosting.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-03-l1-out-of-process-hosting.md"
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

# PC-C4 L1 real ABI binding design

## 境界

Synvelis 側が所有するのは project/control surface の target capability、binding pin、
generation/frame-base gate、control ledger oracle、PCM oracle である。Ruviform が
所有する L1 process isolation、lookahead、plugin ABI adaptation、runner export は
pinned revision `20dc41f71c47674b3e51d1eb0921124b94f8bfaf` を消費するだけで変更しない。

## Production path

test は app project op fixture を作り、`compose_routing_engine_with_out_of_process` に
format 別 `OutOfProcessSpec` を渡す。in-process factory には `test_gain` を混ぜない。
project publish 後の auto attach/sync を使い、control report と
`with_control_dispatch(...binding...)` で binding を検証する。

## Frame base と generation

`AttachClockContext` は node control から mint された frame base だけを信頼する設計を
崩さない（外部 supplied `TimeBase` constructor は新設しない）。test は 44100/1 の
foreign base が frame-base check で reject されることも negative assertion として確認する。
binding の graph generation は active source map generation、capability generation は
`port_capability_generation(project)`、stable instance id は persisted plugin instance
identity と照合する。clip 追加で graph revision を進め、replacement generation で同じ
semantic processor が再度 bound され、旧 generation handle が空になることを見る。

## Control oracle と PCM oracle の分離

control ledger の `Dispatched{call_start,frame_offset}` は frame precision evidence、
PCM は value/domain evidence である。production L1 path から sample-exact PCM timing を
主張しない。`DispatchStage::Dispatched` は plan×span projection であり endpoint
acknowledgement ではないため、PCM oracle で実際の DSP law 適用を独立に確認する。
node transport は最初の write dispatch で Play anchor されるため、ledger の physical
projection は receipt 由来の transport anchor を加えて照合する。

node の live transport plan は単一で、後の plan install は先行 plan を replace する。
そのため automation write は fast write が dispatched observation へ達した後に stage し、
未配送 plan を潰さない。

## PCM oracle と L1 honesty

入力は L=0.25/R=-0.25 の固定 stereo、normalized write は 0.25、期待 plain gain は 0.5。
`input*0.25` は domain 混同 bug として fail、`input*1.0`→`input*0.5` の順序を要求する。
L1 lookahead は 2 blocks、latency contract bound は 1024 frames。初期 silence、遷移 block、
defined pass-through/substitution は fresh delivery と呼ばない。production observability
から upstream matched/substituted counter を読めないため、counter は捏造しない。

headless loop が realtime より速く lookahead window を退役させると、child scheduling
starvation を測るだけになる。そのため test は 1 block ずつ 20ms の wall-clock pacing で
駆動する。これは timing を成功へ偽装するものではなく、非同期 child に処理機会を与えて
DSP law を観測するための oracle 設計である。

## normalized/plain domain

ruviform `ParamValue` / Synvelis `lower_event` の wire-canonical domain は normalized
`[0,1]`。CLAP bridge は `ParamInfo::denormalize` で plain ABI value へ変換し、VST3 は
normalized ABI value を受け取った plugin 側が plain へ変換する。`lower_event` を plain
domain に書き換えるのではなく、最終的な fixture DSP gain が rf#630 contract どおり
`plain = normalized * 2` になることを PCM で検証する。
