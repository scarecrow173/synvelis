---
id: "PLAN-m4Vx8Kq2Bn7Rw5JtYeD9"
type: "plan"
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
    - "docs/designs/tim-03-l1-out-of-process-hosting.md"
  implemented-by:
    - "docs/impl/tim-03-l1-out-of-process-hosting.md"
  depends-on: []
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

# TIM-03 L1 out-of-process hosting production wiring — Plan

## Phase 1 — 調査と境界固定

- ruviform pin `20dc41f71c47674b3e51d1eb0921124b94f8bfaf` の
  `register_out_of_process`、`OutOfProcessSpec`、wire-v2 runner argv contract、
  upstream L1 oracle を確認する。
- 既存 `tim03_real_abi_process_oracle` が L0 negative boundary を所有し続けることを
  確認する。
- root `Cargo.toml` の変更を workspace member 追加と `ruviform-plugin-export`
  dependency 追加に限定する許可を Coordinator から得る。

## Phase 2 — seam と runner

- `ProductOutOfProcessPlugin`、`compose_routing_engine_with_out_of_process`、
  `PluginHostProcessors::product_with_out_of_process` を追加する。
- test-only runner crate と VST3/CLAP runner binary を追加する。
- dedicated mise task `test-tim03-l1` を追加し、runner build → oracle test の順序を
  固定する。

## Phase 3 — oracle と evidence

- `tim03_l1_process_oracle.rs` で VST3/CLAP を実行する。
- production activation と direct endpoint oracle を別 evidence として記録する。
- artifact hash、descriptor、runner resolution、latency、matched/substituted block数、
  未達項目を JSONL と verification markdown へ記録する。

## Phase 4 — 検証と PR

- `fmt-check`、`check`、`lint`、`test synvelis-app`、`test synvelis-player`、
  `test-rt`、`check-headless-deps`、`prek`、runner crate check、
  `test-tim03-l1` を実行する。
- docs index は編集せず、Coordinator 統合時の整備に委ねる。
- 論理 commit を作成し、branch へ push して日本語 PR を作る。

## リスクと対応

- runner argv 非対応: fixture path を argv で渡せないため、sibling artifact stagingを
  採用する。
- target directory の stale artifact: hash mismatch を fail-closed にする。
- L1 bus enumeration: `Unavailable` を unverified とし、empty bus reportと混同しない。
- production endpoint detach: generation teardown で行使されるが独立 oracle 断言は
  せず、direct endpoint detach を evidence にする。
