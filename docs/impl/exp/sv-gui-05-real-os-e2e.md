---
id: "EXP-duLW760ydoj1PyLsU5aC0B"
type: "experiment"
status: "completed"
title: "SV-GUI-05 real OS E2E case definition freeze and acceptance ledger の試行 (synvelis#114)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/sv-gui-05-real-os-e2e.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-05-real-os-e2e.md"
    - "docs/test-specs/svt-real-os-e2e.md"
    - "evidence/sv114/sv-os-case-definitions.md"
    - "evidence/sv114/acceptance-ledger.md"
    - "evidence/sv114/svt-headless-l1.md"
    - "evidence/sv114/l1-contract-comparison.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/svt-real-os-e2e.md"
    - "docs/impl/ir/sv-gui-05-real-os-e2e.md"
    - "evidence/sv114/sv-os-case-definitions.md"
    - "evidence/sv114/acceptance-ledger.md"
  defers: []
  deferred-by: []
---

# SV-GUI-05 real OS E2E case definition freeze の試行

## 問題設定

Issue #114 の残件 docs/evidence スライスとして、(a) SV-OS-01〜09
の real-OS ケース定義を Issue 本文「2026-11-10 実GUI自動入口・
ケースとOwner補足」を正本に freeze し、(b) 受入条件 8 行を
merged-evidence pointer / `NotRun` / 外部 Owner に分類する
acceptance ledger を作る必要があった。実 OS/実 GUI/実 Plugin
レーンは一切実行せず、全 Case を `NotRun` のまま定義だけを
固定することが試行の核心だった。

## 試行と結果

### E1: ケース正本の特定と freeze

Issue #114 本文を `gh issue view` で取得し、「2026-11-10
実GUI自動入口・ケースとOwner補足」の Windows 先行ケース
SV-OS-01〜09 を正本として採用した。各 Case に証拠スキーマ
`Case × OS × driver/backend/GPU × plugin binary/format/SHA ×
Expected / Observed / Result / Artifact / Owner` の全項目と、
Headless/Simulated・Real OS・Native Plugin の別評価列を割り当てた。

- SV-OS-07（Asset Browser Pin/Cancel）と SV-OS-08（Signal
  Flow/Automation UI）は SVT-01〜09 の headless 被覆表に
  独立行を持たないため、Headless/Simulated 列は「専用行なし」
  または関連 oracle への pointer として正直に記録し、
  実在しない証拠を捏造しなかった。
- SV-OS-09 は `syn.app.info` が plugin surface unavailable を
  報告する現行製品状態（`l1-contract-comparison.md` §3）を
  受け、Native Plugin 列を `NotRun`（Deferred・#176 で具現化）
  とし、`Unsupported` にもしなかった（環境・機能未具現化は
  技術的非対応ではない — POLICIES.md §3）。

### E2: 受入条件 8 行の分類

Issue #114 現行「受入条件」8 行を verbatim で転記し、各行を
3 分類に写像した。

- AC1・AC3 は先行 merge 済み証拠（`svt-headless-l1.md`・
  `l1-contract-comparison.md`・`svt_product_e2e*.rs`）への
  pointer とし、証拠内容の複写・再主張を行わなかった。
- AC2・AC7 は merge 後対象 SHA の CI Run URL/Case 別照合が
  Coordinator の証拠工程（`l1-contract-comparison.md` §5）
  であるため `NotRun`（pending）に固定した。
- AC4 は Ruviform #219/#294・Synvelis #28 の候補 SHA 証拠が
  前提のため外部 Owner + `NotRun`。
- AC5・AC6・AC8 は実行自体が `NotRun`/Deferred であり、
  分離規則の freeze として本 ledger + case definitions に
  記録した。

### E3: 文書鎖と index

- spec anchor（`docs/specs/sv-gui-05-real-os-e2e.md`）→
  test-spec（`docs/test-specs/svt-real-os-e2e.md`、Rule
  SVT-FRZ-01〜05）→ impl-ir → impl-exp の 4 本鎖を canonical
  front matter で作成。design/plan/task は lease 範囲外の
  ため作成しない（task spec の指示どおり）。
- `doc-driven-dev-graph` の `route_graph.js` は repo 全体の
  既存 broken `defers` relation（本 slice と無関係・main 由来）
  で fail-closed `blocked` を返す。先行 slice
  （`docs/impl/exp/svt-product-e2e.md`）と同じ状況であり、
  coordinator 指定の文書鎖を手動で踏襲した。
- `docs/{specs,test-specs,impl/ir,impl/exp}/README.md` は
  `doc_maintenance.js index apply` / `sync_impl_index.js` で
  再生成（手編集しない）。

## 採用した判断

- **A-1 定義 freeze のみ**: 実 OS/GUI/Plugin の実行・合格
  主張を一切行わず、case 定義と ledger 分類だけを成果物と
  した。
- **A-2 3 分類の固定**: 各 AC を merged-evidence pointer /
  `NotRun` / 外部 Owner に一意分類し、`Pass` への昇格経路を
  残さなかった。
- **A-3 pointer は参照のみ**: 先行 headless 証拠は path/リンク
  参照に留め、内容の複写・改変・再主張をしなかった
  （READ-ONLY 証拠の保持）。
- **A-4 impl canonical 名**: task 指定 path
  `docs/impl/ir/sv-gui-05-real-os-e2e.md`・
  `docs/impl/exp/sv-gui-05-real-os-e2e.md` を採用
  （scaffold 生成 slug 名は canonical 名へ rename）。
- **A-5 CI 照合は pending のまま**: AC2/AC7 の CI Run URL
  照合を本スライスで済ませたことにせず、Coordinator の
  merge 後証拠工程へ明示的に残した。

## 結論

SV-OS-01〜09 の real-OS ケース定義と Issue #114 受入条件
8 行の分類が `evidence/sv114/` に freeze された。全 Real
OS/Native Plugin 行は `NotRun`、AC の `Pass` は 0 行。
実行に必要な環境・mise 入口・CI 証拠は後続工程に残り、
本スライスは Issue #114 を Close しない部分対応である。
