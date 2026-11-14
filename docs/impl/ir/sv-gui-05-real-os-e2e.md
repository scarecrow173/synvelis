---
id: "IMPL-034XaE8xDAN85GPhpcnHbe"
type: "impl"
status: "completed"
title: "SV-GUI-05 real OS E2E case definition freeze and acceptance ledger (synvelis#114)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  changes:
    added:
      - type: "file"
        path: "docs/specs/sv-gui-05-real-os-e2e.md"
      - type: "file"
        path: "docs/test-specs/svt-real-os-e2e.md"
      - type: "file"
        path: "docs/impl/ir/sv-gui-05-real-os-e2e.md"
      - type: "file"
        path: "docs/impl/exp/sv-gui-05-real-os-e2e.md"
      - type: "file"
        path: "evidence/sv114/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv114/sv-os-case-definitions.md"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gui-05-real-os-e2e.md"
    - "docs/test-specs/svt-real-os-e2e.md"
    - "docs/specs/svt-product-e2e.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/svt-product-e2e.md"
    - "docs/impl/exp/svt-product-e2e.md"
    - "evidence/sv114/svt-headless-l1.md"
    - "evidence/sv114/l1-contract-comparison.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/test-specs/svt-real-os-e2e.md"
  derived-by: []
  verifies:
    - "docs/test-specs/svt-real-os-e2e.md"
  verified-by: []
  references:
    - "docs/specs/sv-gui-05-real-os-e2e.md"
    - "docs/test-specs/svt-real-os-e2e.md"
    - "docs/impl/exp/sv-gui-05-real-os-e2e.md"
    - "evidence/sv114/acceptance-ledger.md"
    - "evidence/sv114/sv-os-case-definitions.md"
    - "evidence/sv114/svt-headless-l1.md"
    - "evidence/sv114/l1-contract-comparison.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sv-gui-05-real-os-e2e.md"
    rejected: []
---

# SV-GUI-05 real OS E2E case definition freeze and acceptance ledger — Issue #114

## Summary

Issue #114 の残件 docs/evidence スライスとして、SV-OS-01〜09 の
real-OS ケース定義を Issue 本文「2026-11-10 実GUI自動入口・
ケースとOwner補足」を正本に freeze し、現行受入条件 8 行を
merged-evidence pointer / `NotRun` / 外部 Owner に分類する
acceptance ledger を `evidence/sv114/` に固定した。spec anchor・
test-spec・ir・exp の文書鎖と managed index を整備した。
実 OS/実 GUI/実 Plugin レーンは一切実行せず、全 Case・全 AC
行を `NotRun`/pointer/外部 Owner のまま維持し、`Pass` への
昇格はない。**定義 freeze と ledger のみ**であり、実装コード・
Fixture 追加・実行結果の主張は対象外。

## Context

- 対象 Issue: synvelis #114（OPEN・docs/evidence slice）
- ブランチ: worktree `scarecrow173/sv114-real-os-defs`
- 先行スライス（merge 済み・READ-ONLY 参照）:
  `evidence/sv114/svt-headless-l1.md`・
  `evidence/sv114/l1-contract-comparison.md`・
  `crates/synvelis-app/tests/svt_product_e2e.rs`・
  `svt_product_e2e_gpui.rs`（SVT-01〜09 Headless L1 証拠）
- 上流正本: Issue #114 本文（現行受入条件 8 行・2026-11-10
  ケース補足・AUTX #267 連携節）、`POLICIES.md` §2/§3/§5、
  `docs/specs/svt-product-e2e.md`
- 並行占有: `crates/**`・`mise.toml`・CI workflow・他 docs は
  他 Worker / lease 範囲のため未変更

## Implementation

### Changes

- `docs/specs/sv-gui-05-real-os-e2e.md`: SV-GUI-05 real-OS
  E2E 定義 freeze スライスの spec anchor（目的・scope・
  non-goals・受入条件・Deferred Design Concerns）。
- `docs/test-specs/svt-real-os-e2e.md`: Rule SVT-FRZ-01〜05
  （case freeze 網羅・評価列分離・freeze 時点 NotRun・ledger
  3 分類・未使用 capability 非要求）と Examples/Guarantees/
  Non-goals/Risk。
- `docs/impl/ir/sv-gui-05-real-os-e2e.md`: 本記録。
- `docs/impl/exp/sv-gui-05-real-os-e2e.md`: 実験記録
  （E1〜E3・採用判断 A-1〜A-5）。
- `evidence/sv114/sv-os-case-definitions.md`: SV-OS-01〜09 の
  freeze 定義（証拠スキーマ・評価列・lane status summary・
  case 別 scope/Expected/Result）。
- `evidence/sv114/acceptance-ledger.md`: 受入条件 8 行の
  verbatim 転記 + 3 分類 ledger + pending/後続工程。
- `docs/specs/README.md`・`docs/test-specs/README.md`・
  `docs/impl/ir/README.md`: managed index 再生成（生成物。
  `index apply` / `sync_impl_index` 経由・手編集なし）。
  `docs/impl/exp/README.md` は `.jsonl` event log のみを
  index する規約のため `.md` 追加では不変（`index plan` で
  `in-sync` を確認）。

### Decisions During Implementation

- **D-1 定義 freeze のみ**: Issue #114 の残件性質に従い、
  実 OS/実 GUI/実 Plugin の実行・検証・合格主張を行わず、
  case 定義と ledger 分類のみを成果物とした。`crates/**`・
  `mise.toml`・CI・既存証拠ファイルは未変更。
- **D-2 文書鎖は spec→test-spec→ir→exp の 4 本**: task spec
  の lease 指定どおり。design/plan/task 文書は範囲外のため
  作成しない。`route_graph.js` は repo 全体の既存 broken
  `defers` relation（main 由来・本 slice と無関係）で
  fail-closed `blocked` を返すため、先行 slice
  （`svt-product-e2e`）と同様に指定鎖を手動で踏襲した。
- **D-3 AC 分類の固定**: AC1・AC3 を merged-evidence
  pointer、AC2・AC5・AC6・AC7・AC8 を `NotRun`（AC4 は
  外部 Owner + `NotRun`）に分類。いずれの行も `Pass`/
  `accepted` にせず、CI Run URL 照合は pending のまま記録。
- **D-4 pointer は参照のみ**: 先行 headless 証拠
  （`svt-headless-l1.md`・`l1-contract-comparison.md`・
  `svt_product_e2e*.rs`）は path 参照に留め、内容の複写・
  改変・再主張をしなかった。
- **D-5 SV-OS-09 は NotRun/Deferred**: plugin surface が
  製品層で unavailable の現状を `Unsupported` にせず、
  #176 具現化まで `NotRun`（Deferred）とした
  （POLICIES.md §3: 環境・機能不在は技術的非対応ではない）。
- **D-6 index 再生成**: `doc_maintenance.js index apply`・
  `sync_impl_index.js --kind ir/exp --apply` で生成、
  手編集しない。

## Related Experiments

### Adopted

- `docs/impl/exp/sv-gui-05-real-os-e2e.md`（E1〜E3: case 正本
  特定・AC 分類・文書鎖/index 整備）
- `docs/impl/exp/svt-product-e2e.md`（先行 slice の実験記録・
  route_graph blocked の既知状況を参照）
- `evidence/sv114/svt-headless-l1.md`・
  `l1-contract-comparison.md`（merge 済み証拠・pointer 参照）

### Rejected

- なし

## Validation

文書・証拠のみのスライスのため cargo の check/lint/test/
build レーンと実 OS/実 GUI/実 Plugin レーンは実行しない
（POLICIES.md §5 比例原則・task spec 指定）。実行した検証
（実測・Windows・mise 管理 toolchain）:

- `mise run fmt-check` → exit 0
- `doc_status.js lint`（repo-wide）→ 新規ファイル由来の
  finding は `docs/specs/sv-gui-05-real-os-e2e.md` の
  `traceability-missing-upstream` warning 1 件のみ
  （先行 spec `svt-product-e2e.md` と同じ既存慣行。
  upstream doc が存在しない issue-anchor spec の既知分類）
- `doc_status.js lint --type test-spec` → 新規 test-spec に
  finding なし
- `audit_impl_record.js --json` → 新規 ir に finding なし
- `audit_experiment_log.js --json` → `.jsonl` のみ対象・
  変更なし
- `doc_maintenance.js index plan` → apply 前に
  specs/test-specs/ir が `sync`、exp が `in-sync`;
  `index apply` で 3 README を書込み、再 plan で全
  `in-sync`
- `sync_impl_index.js --kind ir/exp` → ともに `in-sync`
- `review-protocol` → coordinator 役の自己レビューで
  blocker なし（PR 本文に記録）

ローカル検証 = 変更範囲（docs/evidence のみ）の検証。統合検証は
merge 後の対象 SHA の必須 CI 結果が証拠（Coordinator 確認）。

## Risks

- **R-IR-1 status の誤昇格**: Headless L1 の Pass を実
  OS/Native Dock Pass に転用しない。評価列分離と 3 分類を
  test-spec Rule SVT-FRZ-02/04 で固定。
- **R-IR-2 freeze 後の定義乖離**: SV-OS-09 の plugin
  binary/format/SHA・PD-01〜07 は #176 で具体化するまで
  未確定。`NotRun`/Deferred を維持し、具体化時に case 定義を
  改訂する。
- **R-IR-3 CI 照合の pending 忘れ**: AC2/AC7 の Run URL
  照合は Coordinator の merge 後証拠工程。本 slice の merge
  をもって Pass 扱いしないことを ledger に明記。
- **R-IR-4 並行 Worker 衝突**: sv#114 の他スライス・AUTX
  #267・他 Issue が並行編集中。本 slice は許可範囲
  （新規 6 ファイル + 生成 index）のみを変更。

## Follow-ups

- 実 OS Scenario Runner / opt-in mise 入口・Artifact 採取の
  整備 → 実 OS/Plugin 具体化時（synvelis#176・
  ruviform#313/#314/#388/#389/#532）
- merge 後対象 SHA の CI Run URL・Case 別照合 → Coordinator
  証拠工程（AC2/AC7）
- SV-OS-09 plugin binary/format/SHA・PD-01〜07 展開 →
  synvelis#176
- SVT-10 実 Backend E2E → 実 Backend 準備後の別受入
- AUTX #267 AUTX-GUI-01〜06 との同一 Case ID 接続照合 →
  #267 実施時
- Ruviform #219/#294・Synvelis #28 の候補 SHA 証拠 →
  cross-consumer gate（AC4）
