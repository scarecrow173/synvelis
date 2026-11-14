---
id: "SPEC-hfPJc5Twxe1eCRZ0UTi4xk"
type: "spec"
status: "approved"
title: "SF-06 cross-gate headless fixture 被覆棚卸し・欠落 fixture 追加・evidence 台帳 (synvelis#109)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/sf06_send_path_pcm.rs"
      - type: "file"
        path: "evidence/sv109/coverage.md"
      - type: "file"
        path: "evidence/sv109/verification.md"
      - type: "file"
        path: "evidence/sv109/external-gates.md"
      - type: "file"
        path: "docs/specs/sf06-cross-gate.md"
      - type: "file"
        path: "docs/test-specs/sf06-cross-gate.md"
      - type: "file"
        path: "docs/impl/ir/sf06-cross-gate.md"
      - type: "file"
        path: "docs/impl/exp/sf06-cross-gate.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sf-06-cross-gate-oracle.md"
    - "docs/impl/ir/sf-06-cross-gate-headless-oracle.md"
    - "evidence/reports/sf-06-cross-gate-ledger.md"
  refines:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  deferred-by: []
---

# SF-06 cross-gate headless fixture 被覆棚卸し (synvelis#109)

## What Should Be Built

Issue #109（Routing/Plan/Probe/GUI Cross Gate — Fault Matrix・OS/RT・
AudioLens 互換）の **headless fixture slice**。先行 slice
（`sf-06-cross-gate-oracle`）が Domain Oracle・fault fixture・
v5→v6 migration を `synvelis-project` に樋けたのに対し、本 slice は
**SF-01〜SF-05 の全 headless テスト被覆を SF-MODEL/VALID/ACT/PROBE
case 行列へ棚卸しし、被覆が実測されていない経路だけを fixture として
追加し、結果を `evidence/sv109/` の台帳へ記録する**。

本 slice が証明するのは headless/domain 層の契約のみである。
GUI-L2・EXT-L3・実 plugin・実 device・consumer 受入を headless
結果から昇格させない（`POLICIES.md` の Required/Deferred/
NotApplicable と Pass/Fail/NotRun/Unsupported の分離を維持する）。

## Scope

### 棚卸し対象（監査のみ・変更しない）

- `crates/synvelis-project/tests/` — SF-MODEL/SF-VALID の正本
  （`routing_sf01.rs`・`sf_02.rs`・`sf06_oracle.rs`・
  `sf04_project_probe.rs`・`sf_xr_01.rs` 等）
- `crates/synvelis-player/tests/` — SF-ACT/SF-PROBE の lowering・
  activation・fault・probe 系（`sf03_l*.rs`・`sf04_s*.rs`・
  `l1_e2e.rs`・`sf_xr_01.rs`・RT 系 `*_rt.rs`）
- `crates/synvelis-render/tests/` — offline render/PCM 系
  （`sf03_render.rs`・`l2_e2e.rs`・`x_e2e.rs` 等）

### 追加する fixture

被覆監査で「lowering/source-map 層では検証済みだが PCM 実測が
欠落」と判定した経路に限り、`tests/` 配下へ追加する。本 slice で
追加したのは `crates/synvelis-player/tests/sf06_send_path_pcm.rs`
の 3 テスト:

- `sf06_send_pcm_pre_vs_post_tap` — Pre/Post send tap が fader≠1
  で異なる信号を運び、send gain・bus→master 集約が解析 oracle と
  bit 一致すること（SF-ACT-02/SF-VALID-03/04）
- `sf06_graph_edit_pcm_undo_save_reload` — `RoutingEditIntent` の
  bus+send+return 配線が 1 revision/1 undo で commit され、
  save→load 往復と undo が PCM を bit-for-bit 保存/復元すること
  （SF-MODEL/SF-ACT-02）
- `sf06_mute_solo_pcm_through_sends` — mute/solo が main 経路と
  send 経路の双方へ PCM レベルで効くこと（測定 0 ≠ 未観測）
  （SF-VALID-03/04）

### Evidence 台帳

`evidence/sv109/` に case 行列・検証記録・外部 Gate 判定を置く。
各 case に Case ID・SHA・OS/toolchain・Expected/Observed・
Pass/Fail/NotRun/Unsupported を記録し、SF06-RCP-01・EX-001〜020・
Gate A/B/C・Windows L2/L3・Ruviform #219/#294 提出分は
NotRun/Deferred として対応表に残す。

## Acceptance Criteria

- [ ] SF-MODEL/VALID/ACT/PROBE の全 case について、既存 fixture
      または本 slice 追加 fixture への写像が
      `evidence/sv109/coverage.md` に記録されている。
- [ ] 追加 fixture は `crates/*/tests/` のみで、`src/` 本番コード・
      manifest・lockfile を変更していない。
- [ ] 各 case の実行結果が Case ID・SHA・OS・Expected/Observed・
      Pass/Fail/NotRun/Unsupported で `evidence/sv109/` に記録
      されている。
- [ ] SF06-RCP-01・EX-001〜020・Gate A/B/C・Windows L2/L3・
      Ruviform #219/#294 提出分が NotRun/Deferred として対応表に
      記録されている。
- [ ] headless PASS を GUI/device/plugin/consumer 受入へ昇格する
      記述がない。#114 用の Case 対応表が test-spec に記録されて
      いる。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`（対象 package）が
      green。

## Non-Goals

- 製品 GPUI Editor（#108）・Native Dock（#176）の実装。
- Ruviform Generic Graph/Probe/Clock/OS Input Harness の再実装。
- 実 GUI・実 device・実 plugin・consumer（AudioLens）受入の実行
  （全て外部 Gate として NotRun/Deferred 記録）。
- `docs/specs/signal-flow-design.md` 等、並行 Worker が占有する
  signal-flow 設計 docs の編集。
- `src/` 本番コードの変更（fixture/test のみ）。
