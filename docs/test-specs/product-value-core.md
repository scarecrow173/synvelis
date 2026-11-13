---
id: "TSPEC-034X3jTF3hBjJmyjdZVSsr"
type: "test-spec"
status: "approved"
title: "Synvelis 製品価値と中核体験 — Acceptance Matrix・UX評価テスト仕様"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/1"
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
    - "docs/specs/product-value-core.md"
    - "docs/designs/product-value-core-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/product-value-core-task.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/product-value-core.md"
    - "docs/designs/product-value-core-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Synvelis 製品価値と中核体験 — Acceptance Matrix・UX評価テスト仕様

## Purpose

Issue #1 の 6 検証観点を「製品定義としての合意・検証計画の整備」
条件として検証可能にするためのテスト仕様。各 Rule は Acceptance
Matrix の行（AM-1〜AM-6）と証拠正直性の不変条件に対応する。
本 spec が保証するのは**定義文書の完全性と評価手順の存在**であり、
実装の完成や実 OS/RT/実 GUI の製品受入ではない — それらは Owner
Issue（#114/#109/#103/#8/#23 等）の証拠で別途判定する。

## Feature: Product Value Definition Document Set

synvelis#1 の製品価値・中核体験・Gate A/B/C/AI・UX 評価計画が
`docs/` canonical 文書として正本化され、Acceptance Matrix が
評価可能な形で存在すること。

## Rules

### Rule AM-1: 基本制作（Gate A）の評価定義が存在する

代表タスク A（MIDI フレーズ→Audio トラック→音源/FX/Mixer→
Save/Reload→再生→WAV Export→別 Reader 再読込）について、
Signal Flow/AI/VCS なしでの完了を合格基準とし、GUI-L2 実機・
OS-L1 headless・EXT-L3 Export/再読込の検証手段と Owner
（#8/#23/#114）が Acceptance Matrix に記録されている。

### Rule AM-2: 構造探索（Gate B）の評価定義が存在する

代表タスク B（意図しない音量変化の調査）について、選択トラックの
上流/下流・制御元/影響先の探索、設定への移動・検証・制作復帰を
合格基準とし、既知の接続・観測事実・仮説/因果の区別と
Unknown/Unobserved の非断定を要求として記録している。Owner は
#4/#108/#109。

### Rule AM-3: 文脈維持の評価定義が存在する

画面移動（Arrangement/Mixer/Signal Flow/Inspector、2 Window
含む）で Editing Selection・Inspection Focus・View 状態が維持
され、元の制作作業へ復帰できることを合格基準とする。Owner は
#2/#3/#114。

### Rule AM-4: 調整への接続（Gate C）の評価定義が存在する

代表タスク C（Filter Cutoff の制御元⇄対象の双方向探索→設定調整
→Undo→制作復帰）について、操作数・誤操作・作用 Target の観測
方法と Owner（#6/#102/#103）を記録している。

### Rule AM-5: 情報の正確性の評価定義が存在する

表示内容が実際に取得可能な設定・状態と一致し、Configured/
Active/Observed/Unknown・欠測・未対応・未検証を誤って同一結果に
しないことを合格基準とし、Mock/Native GUI/RT/Plugin の Oracle
を別所有者・結果状態で識別する規則を記録している。

### Rule AM-6: 競合比較の評価定義が存在する

同一初期条件の代表タスク A〜C を Synvelis と既存 DAW（初期候補:
REAPER・Bitwig Studio）で比較する手順・計測項目（達成率/時間/
操作数/誤操作/理解度/探索コスト）・判定方法を定義し、数値閾値は
Baseline 計測・基準値合意の後に決定し、実測前に性能優位を宣言
しないことを記録している。ユーザーテスト未実施は `NotRun`。

### Rule EV-1: 証拠状態の正直性

`Not started / Headless verified / Real GUI verified /
Platform verified / Product accepted` を Gate・行ごとに独立記録
し、headless E2E・GPUI-simulated・Mock・契約 fixture を実 UI
操作の完了と混同しない。`NotRun`/`Unsupported`/`Unknown` を
`Pass` にしない。

### Rule EV-2: 定義と受入の分離

本書群の承認は製品定義の合意であり、実装の完成・実 OS/RT/実
GUI/実 Plugin の製品受入を意味しない。各 Gate の製品受入は
Owner Issue の証拠で別途判定される。

### Rule EV-3: Creative Branch の独立性

Creative Branch（#277〜#288）は別の Feature Acceptance であり、
未完成を Gate A のハードブロッカーにしない。Adopt は宣言 Scope
の正式 Project 適用であり、3-way Merge/Cherry-pick・Mix/Blend・
必須 VCS/AI と区別する。

## Examples

- **EX-1（AM-2/AM-5）:** Bass トラックの音量低下調査で、Sidechain
  接続が「設定として既知の事実」として表示されても、それが音量
  低下の「実原因」とは断定表示されない。観測不能な第三者 plugin
  内部は `Unknown`/`Unsupported` として扱う。
- **EX-2（AM-6）:** UX 比較で Synvelis が REAPER より高速だった
  場合でも、Baseline 未計測・閾値未合意の段階では「優位」と
  記録せず、計測値と条件のみを `evidence/` に残す。
- **EX-3（EV-1）:** SVT-01〜09 の headless pass（`evidence/sv114/`）
  は `Headless verified` の証拠であり、`Real GUI verified` や
  `Product accepted` を主張しない。
- **EX-4（AM-3）:** Signal Flow で対象を調査した後 Mixer へ戻った
  とき、Editing Selection・Inspection Focus・View 状態が遷移前と
  一致し、元の制作作業を再開できる。

## Guarantees

- Acceptance Matrix の全 6 行が測定方法・合格基準・検証手段・
  現状の達成度を持つ。
- 各行に Owner Issue への相互参照がある。
- 「現状の達成度」が headless 証拠と `NotRun`/`Unsupported` を
  正直に区別する。
- UX 比較計画が数値閾値の恣意的設定を禁じ、Baseline 先行の手順
  を持つ。

## Non-goals

- 実 OS/RT/実 GUI/実 Plugin の製品受入の検証（Owner Issue の責務）。
- 個別機能の実装正しさ（#8/#23/#4/#6/#102/#103/#108/#109 の
  各 test-spec が担う）。
- UX 比較の実測実行（Baseline 計測は別途実施）。
- Creative Branch の機能 Acceptance（#277〜#288 の責務）。

## Risk

- 本 spec が「定義の存在」だけを保証し、製品価値の実現を保証
  しない — これは意図的な境界であり、実現の検証は AM 行の Owner
  Issue と UX 比較計画（AM-6）に委譲される。
- 閾値未設定のまま比較議論が進むリスク — §9.4 の Baseline 先行
  手順と Deferred 記録で管理する。
