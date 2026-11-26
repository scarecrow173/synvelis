---
id: "IMPL-pcC4L1RealAbi01"
type: "impl"
status: "approved"
title: "PC-C4 L1 real ABI binding implementation record"
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
    - "docs/plans/pc-c4-l1-real-abi-binding.md"
  implemented-by: []
  depends-on:
    - "docs/designs/pc-c4-l1-real-abi-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c4-l1-real-abi-binding.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv101/pc-c4-l1-real-abi-binding.jsonl"
    - "evidence/sv101/pc-c4-l1-real-abi-binding-verification.md"
  defers: []
  deferred-by: []
---

# PC-C4 L1 real ABI binding implementation record

## 実装内容

- `crates/synvelis-app/tests/pc_c4_l1_real_abi_binding.rs` を新設し、VST3/CLAP の
  production L1 real ABI binding evidence を実装した。
- `mise.toml` に `test-pc-c4-l1` を追加した。runner は既存
  `synvelis-plugin-test-l1-runner` を build するだけで、runner crate 自体は変更していない。
- production path で binding pin、verified frame base、control ledger frame projection、
  normalized/plain PCM oracle、generation replacement/retirement を検証する。
- `evidence/sv101/pc-c4-l1-real-abi-binding.jsonl` に両 format の machine-readable
  evidence を記録した。

## 実測サマリ

`mise run test-pc-c4-l1`（Windows, exit 0）で 2/2 pass。両 format とも fast write
normalized 0.25 に対し PCM が `input*1.0` から `input*0.5` へ遷移し、`input*0.25`
block は 0 件。fast observation は `call_start=1280, frame_offset=132`、automation は
`call_start=5376, frame_offset=36`。graph generation 1→2、retired handles 0、
replacement handles 1。

## 判断記録

- L1 lookahead/pass-through/substitution を fresh delivery と呼ばない。PCM は
  domain/value oracle、frame precision は control ledger oracle と分離した。
- `DispatchStage::Dispatched` は plan×span projection であり endpoint acknowledgement
  ではないため、PCM oracle を独立証拠として必須にした。
- production observability から upstream matched/substituted counter は読めないため
  記録しない（捏造しない）。
- endpoint detach receipt/same-instance re-attach は upstream 契約がないため主張しない。
- automation は single live transport plan の replace semantics により、fast write
  dispatch 後に stage する順序へ限定した。複数 write batching/全 automation matrix は後続。
- transport anchor は receipt から照合した。PCM sample-exact timing は主張しない。
- lookahead 近傍の frame 900 では write が既に submit 済み window に入り plugin に
  適用されないケースがあったため、playback 中の write が lookahead window より先の
  frame に届くよう pacing と frame 選定を行った。

## 参照

- machine evidence: `evidence/sv101/pc-c4-l1-real-abi-binding.jsonl`
- verification: `evidence/sv101/pc-c4-l1-real-abi-binding-verification.md`

## 残件

PLUG-C4-01/02 real ABI 全 matrix、PLUG-C4-03/04/05、tempo/loop/automation matrix、
GUI、EXT-L3、fault/crash/hang、`test_stateful`/`test_fault`/`test_instrument`、
docs index 整備（Coordinator）。
