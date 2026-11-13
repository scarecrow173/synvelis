---
id: "SPEC-034X3kmamOZgcqrHR75ac9"
type: "spec"
status: "approved"
title: "Workspace UX 操作 Scenario・状態図・UX 仕様の正本化 (Issue #3)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/3"
    - "https://github.com/scarecrow173/synvelis/issues/2"
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
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/designs/workspace-ux-scenarios-design.md"
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
    - "docs/mockups/synvelis-daw.html"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/workspace-docking-state-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/5"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/113"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/181"
    - "https://github.com/scarecrow173/ruviform/issues/182"
    - "https://github.com/scarecrow173/ruviform/issues/386"
    - "https://github.com/scarecrow173/ruviform/issues/388"
  defers: []
  deferred-by: []
---

# Workspace UX 操作 Scenario・状態図・UX 仕様の正本化 (Issue #3)

## What Should Be Built

Issue #3 が要求する **Workspace 共通インタラクション仕様の正本** を
文書として確定する。対象は Docking 操作体系、Window/View の開閉・
復帰、Context Inspector の Follow / Explore / Pinned、Link Group と
Linked Context、Focus / Keyboard / Context Menu / IME / Plugin GUI の
入力境界、Default / Saved / Last Session Layout とモニター欠落時の
復帰、そして代表シナリオ A〜F の操作手順と Expected Oracle である。

本 spec が定義するのは「UX 仕様文書が満たすべき要件と受入条件」で
あり、UX 挙動そのものの規範本文は
`docs/designs/workspace-ux-scenarios-design.md` が正本となる。
実装・実 OS 検証は行わない（#112 画面、#113 Dock 実装 [closed]、
#114 実機 E2E、#5 Inspector 詳細、#11 Tool/Editing 詳細、
Ruviform #181/#182/#386/#388 が各 Owner）。

## Why Now

- Issue #3 は 2026-11-07 横断レビュー確定仕様 v2 で「製品 UX の正本」
  と位置づけられ、#113（実装 Owner）・#114（実機受入 Owner）がこの
  仕様を前提に進んでいる。headless 契約（WS-A #36 / ED-A #38）は
  実装済みだが、**ユーザーが見る操作規則の正本**が未確定のままでは
  GUI 受入の Oracle を固定できない。
- #113 は closed で Dock の実装経路が存在する一方、Drop Preview・
  キャンセル・メニュー代替経路・閉鎖復帰といった **操作面の規範**
  は文書化されていない。本タスクはその正本化のみを行う。

## Users and Value

- **DAW ユーザー**: ドッキング可能な複数 Window / 複数モニター環境で
  「Create → Understand → Refine」を予測可能に行える。誤操作・
  文脈喪失・入力衝突が仕様レベルで排除される。
- **実装・受入担当（#112/#113/#114/#5/#11）**: 操作規則・状態遷移・
  失敗時契約の単一参照先ができ、シナリオ A〜F を共通規則で検証
  できる。
- **Ruviform 連携**: 製品固有の選択・調査・編集規則を Ruviform に
  持ち込まない境界が明文化され、基盤側 Issue への要求が切り分け
  られる。

## Scope

### In Scope

- Issue #3 §1〜§10 の UX 規約を、状態図・操作表・Decision Log・
  Owner 境界・シナリオ Expected Oracle を備えた正本文書として
  `docs/` に確定すること（文書成果物のみ）。
- 受入項目ごとのシナリオ ID（`WUX-AC-*` / `WUX-SCN-*`）による追跡
  可能性と、`evidence/sv3/` への仕様根拠・既存 docs 整合確認の記録。
- Mock / Native / headless / real-OS の各検証レーンへの対応付け
  （設計評価を実 OS Window の PASS に換算しない）。

### Out of Scope

- `crates/` を含む一切のコード実装・画面実装・実 Window 検証。
- Ruviform の Window/Dock/NativeSurface 契約の変更・実装。
- Issue #5（Inspector 詳細）・#11（Tool/Editing 詳細）・#12
  （Recording）の個別仕様の確定 — 本仕様は境界と参照のみ定義する。
- `docs/mockups/synvelis-daw.html` の更新（旧 HTML Mock は設計参考
  であり正式仕様ではない。更新モックアップが必要になった場合は
  別 Issue/タスク）。
- 新しい製品トレードオフ・契約破壊・定量 SLO・OS サポート宣言の
  最終決定（「提案/決定待ち」として分離し、人間承認を捏造しない）。

## Requirements

- **R1 正本化**: Docking 操作体系（ドラッグ・Drop Preview・
  キャンセル・失敗復帰・メニュー等の代替経路）を操作仕様として
  一意に読める形で定義する。
- **R2 閉鎖意味論**: View 閉鎖 / サブ Window 閉鎖 / メイン Window
  閉鎖 / 最後の View 移動の 4 操作を区別し、View 破棄と非表示・
  復帰可能性の違いを明文化する。
- **R3 Inspector 状態機械**: Follow / Explore / Pinned の状態遷移・
  履歴（戻る/進む）・外部選択変更時の挙動を状態図で定義する。
- **R4 Link Group**: Link Group と Linked Context の UI 表示・
  設定・同期範囲を定義し、Editing Selection との分離を不変条件と
  して明記する。
- **R5 入力境界**: Focus / Keyboard / Context Menu / IME / Plugin
  GUI の入力境界とコマンド解決優先順位を定義する。Tool 操作
  （Smart/Dedicated・Gesture）との境界も含む。
- **R6 Layout 永続化**: Default / Saved / Last Session Layout の
  区別、保存先の分離、モニター欠落・画面外座標からの復帰動作を
  定義する。
- **R7 シナリオ**: Issue #3 §8 のシナリオ A〜F を `WUX-SCN-A`〜
  `WUX-SCN-F` として手順・Expected Oracle・検証レーン付きで
  定義し、受入項目へ逆引きできるようにする。
- **R8 境界明記**: #112/#113/#114・#5/#11/#12・Ruviform 各 Issue
  との責務境界と、未実装・未検証項目の NotRun/Deferred 記録を
  文書に含める。

## Acceptance Criteria

Issue #3 §9 の受入条件を文書成果物の検収条件へ写像する。
「検証する」は本タスクでは「仕様文書が当該項目を検証可能な形で
規定し、検証レーンと Oracle を明示する」を意味し、実 OS での
実測 Pass を意味しない。

- [ ] **WUX-AC-01**: Docking の操作体系・Drop Preview/キャンセル・
  ドラッグ以外の代替経路が操作仕様として一意に記述されている
  （対応: Issue §9-1、R1）。
- [ ] **WUX-AC-02**: サブ Window 閉鎖時の View 復帰 UX と
  View 破棄/非表示の違いが明文化されている（Issue §9-2、R2）。
- [ ] **WUX-AC-03**: Follow / Explore / Pinned の状態遷移・履歴・
  選択変更時の挙動が状態図と遷移表で検証可能に規定されている
  （Issue §9-3、R3）。
- [ ] **WUX-AC-04**: Link Group と Linked Context の UI 表示・
  設定・同期範囲が規定されている（Issue §9-4、R4）。
- [ ] **WUX-AC-05**: Focus / Keyboard / Context Menu、IME /
  Plugin GUI との入力境界が規定されている（Issue §9-5、R5）。
- [ ] **WUX-AC-06**: Default / Saved / Last Session Layout と
  モニター欠落時の復帰動作が規定されている（Issue §9-6、R6）。
- [ ] **WUX-AC-07**: シナリオ A〜F が同一の操作規則で記述され、
  各シナリオに Expected Oracle と検証レーン（design-review /
  headless / gpui-test / real-OS Gate）が対応付けられている
  （Issue §9-7、R7）。
- [ ] **WUX-AC-08**: Ruviform 汎用機構との境界が明記され、製品固有
  規則を Ruviform へ持ち込まない方針と不足時の upstream 参照先が
  記録されている（Issue §9-8、R8）。
- [ ] **WUX-AC-09**: spec → design → plan → task → test-spec →
  impl の文書チェーンが `docs/` に存在し、`evidence/sv3/` に
  受入項目→シナリオ ID→仕様節の追跡表と検証記録がある。
- [ ] **WUX-AC-10**: 実 OS Window / 実 Plugin GUI で未検証の項目は
  `NotRun`/`Deferred` として記録され、設計評価を実機 Pass に
  換算していない（POLICIES.md §2/§3 準拠）。

## Deferred Design Concerns

- Split 比率・最小サイズ・ドラッグ開始閾値・Drop zone の具体的
  寸法 | Issue #3 §10 未決定 | 実機/UI PoC（#113/#114 lane）で
  確定 | 現状は相対配置と優先順位のみ規定。
- OS 間の Window ドラッグ・Focus・Native Plugin GUI 互換性 |
  同上 | #114/#176 実機 lane | OS 別差異が未規定のまま残る。
- 「閉じた Window を再表示」UI の具体表現・復元履歴の保持期間 |
  同上 | UX 検証（後続 slice）| 閉鎖→復帰の導線が曖昧になり得る。
- Explore 履歴の深さ・保持期間・Follow 元選択変更時の通知表現 |
  #5（Inspector 詳細設計）| #5 確定後 | 本仕様は遷移規則のみ規定。
- Link Group の複数所属・タイムライン連携・詳細設定 UI | #3 §10・
  #11 | 同 | v1 は単一 Group 所属を前提とする。
- レイアウト Undo の履歴範囲・Workspace プリセット細部 |
  SV-GPUI-03（#177）実装済み範囲と要整合 | preset 運用確定後 |
  Default/Saved/Last Session の 3 概念は本仕様で固定。

## Sources and Relations

- Issue #3 本文（§1 承認済み UX 方向性・§2〜§7 操作仕様案・
  §8 シナリオ A〜F・§9 受入条件・§10 未決定事項・2026-11-10
  実行判断: Worker が設計ドラフト・review-protocol・PR まで担当、
  新規トレードオフのみ「提案/決定待ち」に分離）。
- `docs/designs/workspace-docking-state-architecture.md`（統合設計
  正本: 論理構造・状態正本・Command 契約・ライフサイクル）。
- `docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md`
  / `docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md`
  （headless で固定済みの契約: View/Selection/LinkGroup/Command/
  Dock・Tool/Gesture/Transaction）。
- `docs/specs/sv-gpui-03-product-theme-workspace-presets.md`・
  `docs/designs/sv177-product-theme-workspace-preset-design.md`
  （Workspace Preset schema・monitor 補正の既存契約）。
- `docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md`
  ・関連 design（#113 系の実装済み dock op 経路）。
- Ruviform #181（Multi-Window Host）・#182（Docking 基盤）・
  #386（Backend-neutral GUI Host Contract）・#388（NativeSurfaceSlot）。
- `evidence/sv3/workspace-ux-scenarios-traceability.md`（受入項目→
  シナリオ→仕様節の追跡と既存 docs 整合確認）。
