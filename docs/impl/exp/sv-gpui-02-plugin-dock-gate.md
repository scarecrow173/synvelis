---
id: "EXP-bn9kpUxq3WgaxDAkwZMp"
type: "experiment"
status: "completed"
title: "SV-GPUI-02 Plugin Dock Gate — PD-01〜07 freeze + headless contract の試行 (synvelis#176)"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/176"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/sv-gpui-02-plugin-dock-gate.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gpui-02-plugin-dock-gate.md"
    - "docs/test-specs/pd-01-07-plugin-dock.md"
    - "evidence/sv176/pd-01-07-case-definitions.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
    - "docs/impl/exp/sv-gpui-02-plugin-dock-gate.jsonl"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/pd-01-07-plugin-dock.md"
    - "docs/impl/ir/sv-gpui-02-plugin-dock-gate.md"
    - "evidence/sv176/pd-01-07-case-definitions.md"
    - "evidence/sv176/acceptance-ledger.md"
  defers: []
  deferred-by: []
---

# SV-GPUI-02 Plugin Dock Gate — PD-01〜07 freeze + headless contract の試行

## 問題設定

Issue #176 の残件 docs/evidence スライスとして、(a)
PD-01〜07 の plugin-dock 受入ケースを Issue 本文
「2026-11-07 横断レビュー確定仕様 v2」の実機製品ケース定義を
正本に freeze し、(b) 現行受入条件 9 行を merged-evidence
pointer / `NotRun` / 外部 Owner に分類する acceptance ledger
を作り、(c) optional impl leg として `rf631.plugin-dock` の
headless view-mount contract test を synvelis-app に追加する
必要があった。実 OS/実 Plugin/実 Native Dock レーンは一切
実行せず、全 Real OS/Real Plugin 行を `NotRun` のまま定義
だけを固定することが試行の核心だった。

## 試行と結果

### E1: ケース正本の特定と freeze

Issue #176 本文を `gh issue view` で取得し、「2026-11-07
横断レビュー確定仕様 v2」の実機製品ケース PD-01〜07 を
正本として採用した。各 Case に証拠スキーマ `Case × OS ×
plugin format × driver/backend/GPU × plugin artifact
identity（`PluginArtifactEvidence` 6 dims: path・SHA-256・
descriptor plugin_id・name・vendor・version）× host/repo
SHA × Expected / Observed / Result / Artifact / Owner` の
全項目と、Headless/Simulated・Real OS・Real Plugin ABI/
Native Dock の別評価列を割り当てた。

- `rf631.plugin-dock` の 8 case 語彙
  （`view-mount`/`artifact`/`host-lifecycle`/`editor-attach`/
  `session-lifecycle`/`host-terminate`/`open-or-degrade`/
  `dock-pixels`、`ruviform-ui-test` export・pin `ac10a2f7`）
  を証拠行の共有語彙として採用し、synvelis 側 runnability
  を正直に記録した。`PluginHostProbe` 系 5 case は
  `ruviform-plugin-test-vst3`/`ruviform-plugin-dock-test`
  の依存追加（Ask-first・本 slice lease 外）が必要なため
  `NotRun`/Deferred、`editor-attach` は upstream
  `createView` null の技術的 `Unsupported`、`dock-pixels` は
  `VisualCapture` 非存在で `NotRun`。
- PD-03（macOS AU）は POLICIES.md §2 により L2/L3 Deferred
  のため Real OS/Real Plugin 列を Deferred/NotRun とした。
- Headless/Simulated 列は merged 証拠
  （`evidence/sv114/` 各 file・`l1-contract-comparison.md`
  §3 の plugin surface `Unsupported` 記録）への pointer に
  留め、内容の複写・再主張をしなかった。

### E2: 受入条件 9 行の分類

Issue #176 現行「受入条件」9 行を verbatim で転記し、各行を
3 分類に写像した。

- 行 3・5・6・9 は先行 merge 済み証拠
  （`l1-contract-comparison.md`・`svt-headless-l1.md`・
  CI lane）への pointer または分類規則の freeze 記録とし、
  証拠内容の複写・再主張を行わなかった。
- 行 4・8 の merge 後対象 SHA CI Run URL 照合は
  AC の中核証拠が merge 後にしか存在しないため
  `NotRun`（pending）に分類し、Coordinator の証拠工程に
  固定した。
- 行 1・7 は `scarecrow173/ruviform#630`（実ABI Test Native
  Editor）・`#631`（実GPUI Window Runner）・`#532`（既知
  DComp Fail — 修正・実走まで未受入）待ちの `NotRun`、
  行 2 は製品 PluginDock view slice（別タスク）待ちの
  `NotRun` とした。
- いずれの行も本 slice で `Pass`/`accepted` にしなかった。

### E3: optional impl leg の実施判定と実装

`crates/synvelis-app/Cargo.toml` を確認し、`ruviform-ui-test`
・`ruviform-plugin-host` が dev-dependency、
`ruviform-ui-gpui` が `native`/`gpui-test` feature 経由で
既に利用可能であることを確認した。依存追加なしで impl leg
が実施可能と判断し、2 file を追加した。

- `tests/plugin_dock_contract.rs`（headless・常時 compile）:
  `plugin_dock_view_spec` の spec 形状（`plugin.dock` Panel +
  `plugin.editor` NativeSlot 1 個）、`PluginArtifactEvidence`
  の 6 dims JSON、`plugin_dock_spec` の required
  `{NativePluginDock, VisualCapture}`、Fake adapter での
  `view-mount` `Pass`（`Fake` + `{SemanticAction,
  MultiWindow}` のみ記録）、および `plugin_dock_spec`
  spec-helper の capability gate が body 未実行の
  pre-flight `Unsupported` を返すことを検証。
- `tests/plugin_dock_contract_gpui.rs`（`#![cfg(feature =
  "gpui-test")]`）: 同一の driver-neutral body を
  `GpuiGuiTest`（`GpuiSimulated`）で実行し、production
  composition 形（`ViewHost` + `create_view_window`）での
  mount と同じ capability gate を検証。
- Fake/GPUI で scenario body を共有するため、body は
  `SemanticGuiTest` port のみを使う driver-neutral 関数とし、
  mount 手順のみ adapter 別に分けた（`scenario_gpui.rs` の
  `mount_gpui` と同じ手順）。

### E4: 文書鎖と index

- spec anchor（`docs/specs/sv-gpui-02-plugin-dock-gate.md`）→
  test-spec（`docs/test-specs/pd-01-07-plugin-dock.md`、Rule
  PD-FRZ-01〜07）→ impl-ir → impl-exp の鎖を canonical
  front matter で作成。design/plan/task は lease 範囲外の
  ため作成しない。
- `doc-driven-dev-graph` の `route_graph.js` は repo 全体の
  既存 broken `defers` relation（本 slice と無関係・main
  由来）で fail-closed `blocked` を返す。先行 slice
  （SV-GUI-05）と同じ状況であり、coordinator 指定の文書鎖を
  手動で踏襲した。
- scaffold 生成の長い slug 名は canonical 名
  `docs/impl/ir/sv-gpui-02-plugin-dock-gate.md`・
  `docs/impl/exp/sv-gpui-02-plugin-dock-gate.{md,jsonl}` へ
  rename し、spec/test-spec relations を canonical 名に
  合わせた。
- `docs/{specs,test-specs,impl/ir,impl/exp}/README.md` は
  `doc_maintenance.js index apply` / `sync_impl_index.js`
  で再生成（手編集しない）。

## 採用した判断

- **A-1 定義 freeze のみ**: 実 OS/Plugin の実行・合格主張を
  一切行わず、case 定義と ledger 分類を成果物とした。
- **A-2 3 分類の固定**: 各 AC を merged-evidence pointer /
  `NotRun` / 外部 Owner に一意分類し、`Pass` への昇格経路を
  残さなかった。
- **A-3 pointer は参照のみ**: 先行 headless 証拠は path/
  リンク参照に留め、内容の複写・改変・再主張をしなかった。
- **A-4 impl canonical 名**: scaffold 生成 slug 名は
  canonical 名へ rename（先行 slice と同じ判断）。
- **A-5 impl leg は headless contract のみ**: 製品
  PluginDock view・`bind_native_slot`・実 plugin binary・
  依存追加は行わず、共有 `plugin_dock_view_spec` の semantic
  mount conformance と capability gate の正直さのみを
  検証した。`view-mount` の `Pass` は dock/window/pixel
  証拠ではないことを test の assert と report detail の両方
  で固定した。
- **A-6 依存追加なし**: `PluginHostProbe` 系 case の実走に
  必要な crate は Ask-first 事項のため追加せず、
  `NotRun`/Deferred として freeze に記録した。
- **A-7 review 修正（confirmed P1/P2/P3）**: 2 名の独立
  reviewer（cooperative / adversarial）の指摘を検証し、
  confirmed 分を修正した。
  - adversarial P1: upstream `rf631.plugin-dock` probe case
    は `required={}` の `probe_spec`（artifact presence を
    body 内 gate・`PluginHostProbe` driver が honest
    capability を申告）を使い、`plugin_dock_spec` helper は
    upstream case が使用しない。当初の「probe case が
    `plugin_dock_spec` の required を要求する」記述は誤り
    のため、case-definitions・pd-headless-contract・両
    test の doc/test 名・spec AC・test-spec・IR/exp を
    spec-helper contract の記述に修正した。
  - adversarial P2/P3: ledger 行 4・8 を merged-evidence
    pointer から `NotRun`（pending CI 照合）へ再分類、
    dangling section 参照の修正、spec AC/Sources への
    GPUI lane file 名追記。
  - cooperative P1: ledger 行 5・7 の証拠位置引用が誤り
    （`l1-contract-comparison.md` §3 に DComp/#532 記録は
    なく、`pd-01-07-case-definitions.md` Non-claims にも
    #532 未解消記録はない）→ `pd-headless-contract.md`
    Non-claims 節へ repoint。
  - cooperative P2: PD-03 が Result 欄に `Deferred` を記載
    （Result 語彙は Pass/Fail/NotRun/Unsupported の 4 値、
    `Deferred` は要求状態の別軸）→ Result=`NotRun`・
    要求状態=Deferred に修正。
  - cooperative P3: spec/test-spec front matter の
    related/references に `plugin_dock_contract_gpui.rs`
    を追加、PD-01 lane cell の gate 内訳を正確化、
    PD-01 Related の #175 表記を「native app shell
    composition root」に修正。
  - 非 blocking と判定: ledger header の「exactly one of」
    3 分類に対し行 3/4/8 が複合ラベルを持つ点は
    `evidence/sv114/acceptance-ledger.md` と同じ確立済み
    house style であり、分類サマリが各行を正直に注記
    しているため変更しない。

## 結論

PD-01〜07 の case 定義と Issue #176 受入条件 9 行の分類が
`evidence/sv176/` に freeze され、`rf631.plugin-dock` の
headless view-mount contract が `Fake`/`GpuiSimulated` 両
adapter で検証された。全 Real OS/Real Plugin/Native Dock
行は `NotRun`、AC の `Pass` は 0 行。実行に必要な環境
（ruviform#630/#631/#532）・依存追加・製品 view 実装・
CI 証拠照合は後続工程に残り、本スライスは Issue #176 を
Close しない部分対応である。
