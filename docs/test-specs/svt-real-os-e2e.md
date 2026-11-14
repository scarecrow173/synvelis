---
id: "TSPEC-034XaDSzD3ZeB9n3sazAZD"
type: "test-spec"
status: "approved"
title: "SV-GUI-05 Real OS E2E — SV-OS-01〜09 case freeze test spec (synvelis#114)"
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
    - "docs/specs/sv-gui-05-real-os-e2e.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/svt-product-e2e.md"
    - "evidence/sv114/sv-os-case-definitions.md"
    - "evidence/sv114/acceptance-ledger.md"
    - "evidence/sv114/svt-headless-l1.md"
    - "evidence/sv114/l1-contract-comparison.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-05-real-os-e2e.md"
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-05-real-os-e2e.md"
  verified-by: []
  references:
    - "evidence/sv114/sv-os-case-definitions.md"
    - "evidence/sv114/acceptance-ledger.md"
    - "evidence/sv114/svt-headless-l1.md"
    - "evidence/sv114/l1-contract-comparison.md"
  defers: []
  deferred-by: []
---

# SV-GUI-05 Real OS E2E — SV-OS-01〜09 case freeze test spec (synvelis#114)

## Purpose

Issue #114 の real-OS ケース（SV-OS-01〜09）と受入条件 8 行の
分類が、実行前に一意の契約として固定されていることを保証する
検証意図を記録する。実 OS/実 GUI/実 Plugin の実行結果そのもの
ではなく、**freeze された定義と ledger が後続の実行・受入判定
の基準として矛盾なく使えること**を保証する。実 OS レーンが
具体化して各 Case の実測証拠が別文書として積み上がった時点で、
本 test-spec の freeze 規則は実行証拠の schema 契約として残り、
case 定義の改訂が必要になれば新しい test-spec または明示的な
supersede で扱う。

## Feature

SV-GUI-05 Real OS E2E — case definition freeze and acceptance
ledger (synvelis#114)

## Rules

### Rule SVT-FRZ-01: Case freeze は Issue #114 正本の 9 Case を網羅する

`evidence/sv114/sv-os-case-definitions.md` と本書は
SV-OS-01（Project 起動・Close）〜SV-OS-09（Native Plugin
Dock VST3/CLAP 実 Plugin 画素/Input/rollback）を、Issue #114
「2026-11-10 実GUI自動入口・ケースとOwner補足」の Windows
先行ケースを正本として固定する。各 Case は証拠スキーマ
`Case × OS × driver/backend/GPU × plugin binary/format/SHA ×
Expected / Observed / Result(Pass/Fail/NotRun/Unsupported) /
Artifact / Owner` の全項目を持つ。

### Rule SVT-FRZ-02: Headless/Simulated・Real OS・Native Plugin は別評価

各 Case の評価は少なくとも Headless/Simulated（OS-L1 相当）、
Real OS（GUI-L2 相当）、Native Plugin（EXT-L3 相当）の列で
分離する。ある列の Pass/Fail/NotRun/Unsupported を別の列へ
転用・昇格・合算しない。Headless/Simulated 列は先行 merge 済み
証拠（`evidence/sv114/svt-headless-l1.md`・
`l1-contract-comparison.md`・`crates/synvelis-app/tests/
svt_product_e2e.rs`・`svt_product_e2e_gpui.rs`）への参照
pointer に留め、証拠内容を複写・再主張しない。

### Rule SVT-FRZ-03: freeze 時点の Real OS / Native Plugin は NotRun

本スライスは定義 freeze のみであり、いかなる実 OS Window/
Input/Pixel・実 plugin editor・実 device の実行も行わない。
全 Case の Real OS / Native Plugin 行は `NotRun` で固定する。
`Unsupported` は根拠を伴う技術的非対応に限り、環境不在を
`Unsupported` にしない（POLICIES.md §3）。`Pass` は Observed・
Artifact・実行環境・Run/実機 URL を伴う実測がある場合に限る。

### Rule SVT-FRZ-04: 受入条件 ledger は 3 分類のいずれかに留まる

`evidence/sv114/acceptance-ledger.md` は Issue #114 現行
受入条件 8 行を、(i) merged-evidence pointer、(ii) `NotRun`、
(iii) 外部 Owner（Ruviform #243/#313/#388/#389 fixture、
Ruviform #219/#294、Synvelis #28、Synvelis #176 の EXT-L3
実 Plugin Dock、macOS/Linux L1 CI、AUTX #267、SVT-10 の実
Backend 準備後の別受入）のいずれかに分類する。いずれの行も
本スライスで `Pass`/`accepted` にしない。CI Run URL 照合
（AC2 関連）は merge 後対象 SHA の CI 証拠として pending の
まま記録する（`l1-contract-comparison.md` §5）。

### Rule SVT-FRZ-05: 未使用 capability を一律要求しない

Windows で未使用の Device・Plugin 形式・Provider は受入を
要求しない（POLICIES.md §2・Issue #114 AC8）。必要な実走が
`NotRun`/`Fail` なら機能単位で未受入とし、OS/Owner/CI/実機
証拠を分離して記録する。

## Examples

- Example: SV-OS-01 を Windows 実機で後日実行する場合、
  `driver/backend/GPU`・実 OS Window の `Observed`・
  screenshot/log の `Artifact`・CI/実機 Run URL を埋めた行が
  新しい証拠文書に追加され、初めて `Result=Pass/Fail` を
  取り得る。freeze 行自体は `NotRun` のまま履歴として残る。
- Example: Headless L1 で SVT-01 が Pass していても、
  SV-OS-01 の Real OS 列へその結果をコピーしない。
  Headless/Simulated 列の pointer と Real OS 列の `NotRun`
  は独立に維持される。
- Example: `syn.app.info` が plugin surface unavailable を
  報告する現状をもって SV-OS-09 を `Unsupported` としない。
  plugin dock が製品機能として具体化するまで `NotRun`
  （Deferred）であり、#176 の EXT-L3 受入が別途必要。
- Example: AC2 の CI Run URL 照合は merge 後対象 SHA の CI
  完了を待つ pending 項目として ledger に残り、本スライスの
  merge をもって Pass 扱いしない。

## Guarantees

- SV-OS-01〜09 の定義・証拠スキーマ・評価列分離・Owner が
  文書として一意に存在し、後続の実行者が解釈を要しない。
- 受入条件 8 行それぞれの現在地（merged evidence / NotRun /
  外部 Owner / pending CI 照合）が ledger から読み取れる。
- Fake/Headless/Simulated の Pass が実 GUI・Native Dock・
  実 Plugin の Pass として記録される経路が、本 freeze 文書
  群の上では存在しない。

## Non-goals

- SV-OS-01〜09 の実 OS での実行・合否判定。
- 実 Plugin（VST3/CLAP/AU）の Dock・Editor・hang
  （PD-01〜07）の受入 — synvelis#176 の責務。
- CI Run URL の実収集と照合 — Coordinator の merge 後証拠
  工程。
- macOS/Linux の L2/L3 — 利用要件発生まで Deferred
  （POLICIES.md §2）。
- SVT-10（実 Backend E2E）— 実 Backend 準備後の別受入。

## Risk

- freeze がないまま実 OS レーンが個別に走ると、Case ID・
  証拠 schema・判定語彙がばらつき、後で台帳を再構成する
  コストが発生する。
- 分類を誤ると Headless L1 の Pass が製品受入
  （Product Accepted）へ誤昇格し、POLICIES.md §3 の
  Required/Deferred・Pass/Fail/NotRun/Unsupported 分離が
  崩れる。
- AC ledger が曖昧だと、外部 Owner（Ruviform fixture・
  #176・CI）待ちの項目と Synvelis 側で完結する項目の境界が
  不明になり、Issue #114 の Close 判定が不可能になる。
