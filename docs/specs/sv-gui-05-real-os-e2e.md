---
id: "SPEC-034XaDS2SnpbaxNulU1BmU"
type: "spec"
status: "approved"
title: "SV-GUI-05 Real OS E2E — case definition freeze and acceptance ledger (synvelis#114)"
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
  implemented-by: []
  depends-on:
    - "docs/specs/svt-product-e2e.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/svt-real-os-e2e.md"
    - "docs/specs/ci-os-l1-native-gpui.md"
    - "evidence/sv114/sv-os-case-definitions.md"
    - "evidence/sv114/acceptance-ledger.md"
    - "evidence/sv114/svt-headless-l1.md"
    - "evidence/sv114/l1-contract-comparison.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/svt-real-os-e2e.md"
  references:
    - "evidence/sv114/svt-headless-l1.md"
    - "evidence/sv114/l1-contract-comparison.md"
    - "crates/synvelis-app/tests/svt_product_e2e.rs"
    - "crates/synvelis-app/tests/svt_product_e2e_gpui.rs"
  defers: []
  deferred-by: []
---

# SV-GUI-05 Real OS E2E — case definition freeze and acceptance ledger (synvelis#114)

## What Should Be Built

Issue #114 の残件 docs/evidence スライスとして、SV-GUI-05
「GPUI Native Product E2E」の **real-OS ケース定義の freeze** と
**受入条件の acceptance ledger 化** を行う。具体的には:

- Issue #114 本文「2026-11-10 実GUI自動入口・ケースとOwner補足」
  の Windows 先行ケース **SV-OS-01〜09** を、証拠スキーマ
  `Case × OS × driver/backend/GPU × plugin binary/format/SHA ×
  Expected / Observed / Result(Pass/Fail/NotRun/Unsupported) /
  Artifact / Owner` で凍結し、`evidence/sv114/
  sv-os-case-definitions.md` と `docs/test-specs/
  svt-real-os-e2e.md` に固定する。
- Issue #114 現行「受入条件」8 行を `evidence/sv114/
  acceptance-ledger.md` に写し取り、各行を merged-evidence
  pointer・`NotRun`・外部 Owner のいずれかへ正直に分類する。
- Headless/Simulated（OS-L1）・Real OS（GUI-L2）・Native Plugin
  （EXT-L3）を別評価列として分離し、先行スライスの Headless L1
  証拠（`evidence/sv114/svt-headless-l1.md`・
  `l1-contract-comparison.md`・`svt_product_e2e*.rs`）への
  参照 pointer のみを張る。

## Why Now

Issue #114 の OS-L1/Headless Slice（SVT-01〜09 の製品
State/Receipt 検証と Ruviform Fake/Fixture との L1 契約比較）は
merge 済みだが、real-OS ケースと受入条件の写像が未固定のまま
残っている。実 OS/実 GUI/実 Plugin レーンの実行環境・専用
mise 入口・CI Run URL 照合は未整備であり、本スライスはそれらを
実行せず、**実行可能な状態になる前にケース定義と受入分類を
凍結**して後続スライスの判定基準を固定する。

## Users and Value

- Synvelis の Acceptance Owner（Issue #114）と Coordinator:
  実 OS E2E 実施時に「何を・どの OS/driver/plugin で・どの
  証拠 schema で判定するか」が一意に定まる。
- Ruviform 側 Owner（#243/#313/#388/#389/#532）と Consumer
  横断 Gate（#294）: Synvelis 側の要求分類と外部依存が
  明示され、Fake/Headless Pass の誤昇格を防げる。

## Scope

### In Scope

- SV-OS-01〜09 のケース定義 freeze（名称・対象操作・関連
  SVT/SV-UI/Issue・OS/driver/plugin/SHA 次元・Expected の骨格）。
- 証拠スキーマと評価列（Headless/Simulated・Real OS・Native
  Plugin）の明示。
- Issue #114 受入条件 8 行の acceptance ledger 化
  （merged-evidence pointer / NotRun / 外部 Owner の分類）。
- spec anchor → test-spec → impl-ir → impl-exp の文書チェーンと
  管理 index の再生成。

### Out of Scope

- 実 OS Window/Input/Pixel・IME/DPI/Monitor の実行（GUI-L2）。
- 実 VST3/CLAP/AU Native Dock・実 Plugin editor・実
  Audio/MIDI device の実行（EXT-L3、synvelis#176 系）。
- 実 OS/実 GUI/実 Plugin 用 mise レーン入口の新設、CI
  workflow の変更。
- merge 後対象 SHA の CI Run URL 照合（Coordinator の証拠工程、
  `l1-contract-comparison.md` §5）。
- `crates/**` のコード・テスト変更、design/plan/task 文書
  （本スライスの lease 範囲外）。
- いかなる Case/AC 行の `Pass`/`accepted` 昇格。

## Requirements

- SV-OS-01〜09 は Issue #114「2026-11-10 実GUI自動入口・
  ケースとOwner補足」の Windows 先行ケースを正本として freeze
  する。
- 全 Case の Real OS / Native Plugin 評価は `NotRun` のまま
  freeze し、Headless/Simulated 列は先行証拠への pointer に
  留める。
- 受入条件 8 行はそれぞれ (i) merged-evidence pointer、
  (ii) `NotRun`、(iii) 外部 Owner のいずれかに分類し、本
  スライスで `Pass` にしない。
- CI Run URL 照合（AC2 関連）は pending のまま記録する。

## Acceptance Criteria

- [ ] `evidence/sv114/sv-os-case-definitions.md` と
  `docs/test-specs/svt-real-os-e2e.md` が SV-OS-01〜09 を同一
  証拠スキーマで凍結し、全 Real OS/Native Plugin 行が
  `NotRun` である。
- [ ] `evidence/sv114/acceptance-ledger.md` が受入条件 8 行を
  merged-evidence pointer / `NotRun` / 外部 Owner に分類し、
  参照先証拠の再主張・昇格をしていない。
- [ ] Headless L1 証拠（`svt-headless-l1.md`・
  `l1-contract-comparison.md`・`svt_product_e2e*.rs`）が
  pointer 参照のみで複写・改変されていない。
- [ ] `docs/specs/README.md`・`docs/test-specs/README.md`・
  `docs/impl/ir/README.md`・`docs/impl/exp/README.md` の
  managed index が `doc-maintenance` の `index apply` で
  再生成されている。

## Deferred Design Concerns

- 実 OS Scenario Runner / Artifact 採取の専用 opt-in mise 入口
  | 実行環境・host 入力・screenshot/video/receipt/log の
  記録形式が未定 | 実 OS/Plugin が具体化した時（#176・
  Ruviform #313/#314 と共に）| 定義なしに実行すると証拠
  schema が Case 定義と乖離する。
- SV-OS-09 の plugin binary/format/SHA・PD-01〜07 の具体
  Case 展開 | 対象 Plugin・Format が未確定 | #176 で plugin
  dock 要件が具体化した時 | 未確定のまま Pass 判定すると
  Fake/Generic Editor の成功を Native Dock Pass に誤読する
  恐れ。
- CI Run URL の Case 別照合 | merge 後対象 SHA の CI 証拠が
  未収集 | merge 後の対象 SHA の CI 完了時（Coordinator の
  証拠工程）| Job green のみでは個別 Case の Pass を示せない
  （POLICIES.md §3）。

## Sources and Relations

- Source: synvelis Issue #114（現行受入条件 8 行・
  2026-11-10 実GUI自動入口・ケースとOwner補足・AUTX 連携
  節）。
- Related: `docs/specs/svt-product-e2e.md`（先行 Headless L1
  スライスの spec）、`docs/test-specs/svt-real-os-e2e.md`
  （本 spec の検証意図）、`evidence/sv114/` の先行証拠、
  `docs/specs/ci-os-l1-native-gpui.md`（3OS CI OS-L1 の
  別 Issue #377 正本）。
