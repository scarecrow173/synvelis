---
id: "IMPL-z2Qw8Lm5Vx7Kn4RtYeB6"
type: "impl"
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
    - "docs/plans/tim-03-l1-out-of-process-hosting.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-l1-out-of-process-hosting.md"
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
  references:
    - "evidence/sv127/l1-process-oracle.jsonl"
  defers: []
  deferred-by: []
---

# TIM-03 L1 out-of-process hosting production wiring — Implementation Record

## 実装内容

- `crates/synvelis-player/src/host.rs`
  - `PluginHostProcessors::product_with_out_of_process` を追加。
  - in-process factory 登録と L1 `OutOfProcessSpec` 登録を同一 composition-root
    constructor で扱えるようにした。
- `crates/synvelis-app/src/composition.rs`
  - `ProductOutOfProcessPlugin` を追加。
  - `compose_routing_engine_with_out_of_process` を追加。
  - 既存 `compose_routing_engine` は empty L1 route table で新 helper へ委譲し、
    in-process API の意味を変えていない。
- `crates/synvelis-plugin-test-l1-runner/`
  - `synvelis-plugin-test-vst3-l1-runner` と
    `synvelis-plugin-test-clap-l1-runner` を追加。
  - runner は fixture cdylib を `SYNVELIS_TIM03_L1_PLUGIN_DIR` または executable
    sibling から解決し、production loader bridge 経由で scan した factories を
    `ruviform_plugin_export::run` へ渡す。
- `mise.toml`
  - `test-tim03-l1` を追加。runner build 後に oracle test を実行する。
- `crates/synvelis-app/tests/tim03_l1_process_oracle.rs`
  - VST3/CLAP の production activation と direct endpoint process oracle を追加。
- `Cargo.toml` / `Cargo.lock`
  - Coordinator 許可の限定変更として workspace member と
    `ruviform-plugin-export` dependency を追加し、lockfile を必要範囲で更新した。

## 実測サマリ

`mise run test-tim03-l1`（Windows, exit 0）で 2 tests passed。

| format | production activation | endpoint attach/detach | matched blocks | substituted blocks | latency |
| --- | --- | --- | --- | --- | --- |
| CLAP | Active generation 1 | pass | 14 | 8 | 1024 frames |
| VST3 | Active generation 1 | pass | 15 | 7 | 1024 frames |

両 format とも `BusReport::Unavailable` であり、bus enumeration は unverified。
matched block の samples は deterministic input 履歴に対する `0.5 * input` membership
を満たした。substituted/pass-through block は delivery evidence に数えていない。

## 判断記録

- upstream runner binary が dependency edge 経由で downstream に提供される前提は
  採用しなかった。Synvelis 側 runner crate を所有する。
- runner へ独自 argv を渡す設計は、`ruviform-plugin-export` が argv を defined usage
  rejection にするため不成立だった。初回実装は `SpawnFailed{cause:Transport}` で
  検出し、sibling artifact staging へ修正した。
- production endpoint detach は generation/application teardown で行使されるが、
  独立観測可能な oracle としては断言しない。direct endpoint の drop 後 teardown を
  この slice の detach evidence とする。
- `test_gain` 以外の fixture、tempo/loop/automation 精度、crash/hang isolation は
  この slice では未検証。

## 参照

- machine-readable evidence: `evidence/sv127/l1-process-oracle.jsonl`
- verification evidence: `evidence/sv127/l1-process-oracle-verification.md`
