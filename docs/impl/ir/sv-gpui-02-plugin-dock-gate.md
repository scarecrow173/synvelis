---
id: "IMPL-034XPOTEa44xCPXxCek7Hb"
type: "impl"
status: "completed"
title: "SV-GPUI-02 Plugin Dock Gate — PD-01〜07 freeze + headless contract (synvelis#176)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/176"
  changes:
    added:
      - type: "file"
        path: "docs/specs/sv-gpui-02-plugin-dock-gate.md"
      - type: "file"
        path: "docs/test-specs/pd-01-07-plugin-dock.md"
      - type: "file"
        path: "evidence/sv176/pd-01-07-case-definitions.md"
      - type: "file"
        path: "evidence/sv176/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv176/pd-headless-contract.md"
      - type: "file"
        path: "crates/synvelis-app/tests/plugin_dock_contract.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/plugin_dock_contract_gpui.rs"
      - type: "file"
        path: "docs/impl/ir/sv-gpui-02-plugin-dock-gate.md"
      - type: "file"
        path: "docs/impl/exp/sv-gpui-02-plugin-dock-gate.md"
      - type: "file"
        path: "docs/impl/exp/sv-gpui-02-plugin-dock-gate.jsonl"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/specs/sv-gpui-02-plugin-dock-gate.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pd-01-07-plugin-dock.md"
    - "evidence/sv176/pd-01-07-case-definitions.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
    - "docs/impl/exp/sv-gpui-02-plugin-dock-gate.md"
    - "docs/impl/exp/sv-gpui-02-plugin-dock-gate.jsonl"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gpui-02-plugin-dock-gate.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv176/pd-01-07-case-definitions.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
    - "crates/synvelis-app/tests/plugin_dock_contract.rs"
    - "crates/synvelis-app/tests/plugin_dock_contract_gpui.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sv-gpui-02-plugin-dock-gate.md"
      - "docs/impl/exp/sv-gpui-02-plugin-dock-gate.jsonl"
    rejected: []
---

# SV-GPUI-02 Plugin Dock Gate — PD-01〜07 freeze + headless contract (synvelis#176)

## Summary

Issue #176（SV-GPUI-02 Plugin Dock gate）の残件
docs/evidence スライスを実施した。(a) PD-01〜07 の
plugin-dock 受入ケースを Issue 本文「2026-11-07 横断
レビュー確定仕様 v2」を正本に証拠スキーマ付きで freeze、
(b) 現行受入条件 9 行を merged-evidence pointer /
`NotRun` / 外部 Owner に分類する acceptance ledger を作成、
(c) optional impl leg として `rf631.plugin-dock` 語彙の
headless view-mount contract test を `synvelis-app` に
Fake lane（常時）と GPUI-simulated lane（`gpui-test`
feature）の 2 file で追加した。実 OS/実 Plugin/実 Native
Dock の実行は一切行わず、全 Real OS/Real Plugin 行は
`NotRun` のまま。Issue #176 は Close しない。

## Context

- Task: synvelis#176 の gate-freeze slice（Orca dispatched
  worker task — spec/test-spec/evidence/impl docs + optional
  headless contract test）。
- Spec: `docs/specs/sv-gpui-02-plugin-dock-gate.md`（本
  freeze slice の spec anchor — SV-GPUI-02 全体の spec では
  ない）。
- Test-spec: `docs/test-specs/pd-01-07-plugin-dock.md`
  （Rule PD-FRZ-01〜07）。
- Policy: `POLICIES.md` §2（OS-L1 必須・GUI-L2/EXT-L3 は
  提供機能のみ Required・未使用は要求しない）・§3
  （Pass/Fail/NotRun/Unsupported/Blocked の分離・Fake/
  Headless は実機証拠に昇格しない）。
- Prior art: `docs/specs/sv-gui-05-real-os-e2e.md`・
  `evidence/sv114/`（SV-GUI-05 の同型 freeze slice）。
- Upstream vocabulary: `ruviform-ui-test`（Cargo.lock pin
  `ac10a2f7`）の `PLUGIN_DOCK_SCENARIO`・
  `PLUGIN_DOCK_SLOT_KEY`・`plugin_dock_view_spec`・
  `plugin_dock_spec`・`PluginArtifactEvidence`・
  `run_scenario`/`SemanticGuiTest` port。8 case 語彙の正本は
  `ruviform-plugin-dock-test`（参照のみ・依存ではない）。

## Implementation

### Changes

- `docs/specs/sv-gpui-02-plugin-dock-gate.md` — freeze
  slice の spec anchor（scope・requirements・AC・deferred
  concerns・relations）。
- `docs/test-specs/pd-01-07-plugin-dock.md` — Rule
  PD-FRZ-01〜07（case 網羅・評価列分離・Real 系 NotRun・
  ledger 3 分類・未使用非要求・artifact dims/rf631 語彙・
  Fake 非昇格）。
- `evidence/sv176/pd-01-07-case-definitions.md` — PD-01〜07
  の case 定義 freeze（証拠 schema・lane status summary・
  rf631 語彙と synvelis runnability・Non-claims）。
- `evidence/sv176/acceptance-ledger.md` — Issue #176 受入
  条件 9 行の verbatim 転記 + 3 分類 + 外部 Owner 一覧。
- `crates/synvelis-app/tests/plugin_dock_contract.rs` —
  headless Fake lane: spec 形状・artifact dims・required
  capabilities・`view-mount` Pass（honest report）・
  `plugin_dock_spec` spec-helper capability gate
  `Unsupported`（body 未実行）の 5 test。
- `crates/synvelis-app/tests/plugin_dock_contract_gpui.rs` —
  `gpui-test` feature lane: 同一 driver-neutral body を
  `GpuiGuiTest` で実行する 2 test（`#![cfg(feature =
  "gpui-test")]`）。
- `docs/impl/exp/sv-gpui-02-plugin-dock-gate.md` +
  `.jsonl` — 実験記録（narrative + CLI event log）。
- managed index 4 file（specs/test-specs/impl-ir/impl-exp
  README）を `doc_maintenance`/`sync_impl_index` で再生成。

### Decisions During Implementation

- **impl leg 実施**: 依存は全て既存（`ruviform-ui-test`・
  `ruviform-plugin-host` dev-deps、`ruviform-ui-gpui` は
  `native`/`gpui-test` feature）のため依存追加なしで実施
  可能と判断。`PluginHostProbe` 系 case の実走に必要な
  `ruviform-plugin-test-vst3`/`ruviform-plugin-dock-test`
  は Ask-first のため追加せず `NotRun`/Deferred とした。
- **2 file 構成**: `svt_product_e2e.rs`/`scenario_gpui.rs`
  の先例どおり、headless（常時 compile）と GPUI-simulated
  （feature-gated）を別 file に分け、scenario body は
  `SemanticGuiTest` port のみを使う driver-neutral 関数で
  共有した。mount 手順のみ adapter 別（Fake
  `mount_view` / GPUI `ViewHost`+`create_view_window`）。
- **`view-mount` spec の直接 mount**: 製品 PluginDock view
  は未具現化のため、共有 `plugin_dock_view_spec()` を
  `ViewHost` に直接 mount する upstream `view-mount` case
  と同じ構成で contract conformance を検証した。これは
  製品 composition の証拠ではなく semantic mount の契約
  証拠であり、その区別を test 名・assert・report detail・
  evidence doc の全てで固定した。
- **canonical impl 名**: scaffold 生成の長い slug 名を
  `sv-gpui-02-plugin-dock-gate.{md,jsonl}` へ rename し、
  spec/test-spec relations を合わせた（SV-GUI-05 と同じ
  判断）。
- **graph router blocked**: `route_graph.js` は repo 全体の
  既存 broken `defers` relation で fail-closed。main 由来の
  既知状態であり本 slice と無関係 — coordinator 指定の文書鎖を
  手動で踏襲し、PR に記録する。
- **review 修正**: review-protocol の独立 reviewer 2 名
  （cooperative / adversarial）の confirmed 指摘を修正 —
  upstream probe case が `plugin_dock_spec` を使わない
  （`required={}` の `probe_spec`）事実誤認の訂正、ledger
  行 4/8 の `NotRun` 再分類、行 5/7 の証拠位置 repoint、
  PD-03 Result 語彙修正（`Deferred` は要求状態の別軸）、
  front matter relations への GPUI lane 追記。詳細は
  exp A-7。

## Related Experiments

### Adopted

- `docs/impl/exp/sv-gpui-02-plugin-dock-gate.jsonl` — 本
  slice の実装中イベント（freeze 判断・impl leg 判定・
  検証結果）。
- `docs/impl/exp/sv-gpui-02-plugin-dock-gate.md` — 試行の
  narrative 記録（E1〜E4・採用判断 A-1〜A-7）。

### Rejected

- なし。

## Validation

全て Windows worktree（`G:\orca\workspaces\synvelis\
sv176-pd-gate-freeze`）・locked deps・ruviform pin `ac10a2f7`
で実行。exit code は全て 0。

- `mise run fmt` + `mise run fmt-check` — pass（rustfmt
  差分を `fmt` で適用後 check green）。
- `mise run check synvelis-app` — pass。
- `mise run lint synvelis-app` — pass（`-D warnings`；初回
  `clippy::doc_markdown` 1 件を doc comment の backtick 修正で
  解消）。
- `mise run test synvelis-app` — pass（全 test file green;
  `plugin_dock_contract` 5/5 含む）。
- `mise run test synvelis-app --test plugin_dock_contract` —
  5 passed / 0 failed。
- `mise run test-app-gpui --test plugin_dock_contract_gpui` —
  2 passed / 0 failed。
- `mise run test-app-gpui`（full `gpui-test` lane）— pass。
- `mise run check-app-gpui` — pass。
- `mise run lint-app-gpui` — pass。
- `mise run check-headless-deps` — pass（"no GUI framework
  deps in 12 headless crates"）。
- `mise exec -- prek run --all-files` — pass。
- `doc_maintenance.js index plan/apply`（docs/specs・
  docs/test-specs）+ `sync_impl_index.js --kind ir/exp
  --apply` — 全 index in-sync を再 plan で確認。
- `audit_impl_record.js --json` / `audit_experiment_log.js
  --json` / `audit_docs.js --type spec|test-spec` — 本 slice
  file に error なし。spec の `traceability-missing-upstream`
  warning は先行 slice（sv-gui-05）と同じ Issue 由来 spec の
  既知 pattern。`issue-147-sccache.jsonl` の既存 invalid
  event・repo 全体の broken `defers` は main 由来の既知状態
  で本 slice と無関係。
- review-protocol self-review（reviewer 2 名・validator/
  judge は親 agent が実施）後の修正分を再検証:
  `mise run test synvelis-app --test plugin_dock_contract`
  5/5・`--features gpui-test --test
  plugin_dock_contract_gpui` 2/2（test 名変更
  `*_spec_gate_is_unsupported_before_the_body_runs` 後も
  green）。doc 変更分は audit 再実行で error なしを確認。

## Risks

- semantic `view-mount` Pass が Native Dock Pass と誤読
  される危険 — test-spec PD-FRZ-07・evidence Non-claims・
  report detail の三重で抑止済みだが、後続 slice での
  誤昇格は引き続き review 対象。
- `rf631.plugin-dock` 語彙は upstream 進行中（#630/#631・
  #532 未解消）— pin 更新で export 形状が変わる可能性。
  `Cargo.lock` pin `ac10a2f7` 時点の形状に test を固定
  しており、pin 更新時は再検証が必要。
- graph router の既存 broken `defers` は main 由来 — 本
  slice で修復せず、PR/ledger に既知 blocker として記録。

## Follow-ups

- `PluginHostProbe` 系 case（`artifact`/`host-lifecycle`/
  `session-lifecycle`/`host-terminate`/`open-or-degrade`）の
  synvelis 側実走 — `ruviform-plugin-test-vst3`・
  `ruviform-plugin-dock-test` 依存追加の承認（Ask-first）後。
- 製品 PluginDock view 実装・`bind_native_slot`/
  `SurfaceSlotId` 実 binding・`ui_ops` 登録（別 slice・
  `crates/synvelis-app/src/**`）。
- PD-01〜07 実 OS/実 Plugin 受入 — `scarecrow173/ruviform
  #630`/#631 実環境 + `#532` DComp 解消後、`evidence/sv176/`
  の freeze schema に実測行を追加。
- merge 後対象 SHA の CI Run URL 照合（Coordinator の証拠
  工程）。
