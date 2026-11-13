---
id: "TSPEC-034X3TcwIjZEAATs2dGBz9"
type: "test-spec"
status: "approved"
title: "Workspace UX 操作仕様の文書契約検証"
created: "2026-11-13"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/3"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/workspace-ux-scenarios.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/workspace-ux-scenarios-design.md"
  derived-by: []
  verifies:
    - "docs/specs/workspace-ux-scenarios.md"
  verified-by: []
  references:
    - "docs/test-specs/ws-a-e2e-workspace-foundation.md"
  defers: []
  deferred-by: []
---

# Workspace UX 操作仕様の文書契約検証

## Purpose

`docs/specs/workspace-ux-scenarios.md`（要件）と
`docs/designs/workspace-ux-scenarios-design.md`（UX 正本）が、
Issue #3 の受入条件を「後続の実装・検証が依拠できる契約」として
満たしていることを固定する。対象は**文書成果物の契約**であり、
製品コードの動作ではない。実装 Issue（#112/#113/#114/#5/#11/
#12）と Ruviform（#181/#182/#386/#388）が本契約を消費する間は
維持し、UX 正本の改訂（supersede）時に見直す。

## Feature

Workspace UX 操作仕様の文書契約 — Issue #3 §1〜§10 の UX 規約が
状態図・操作表・Decision Log・Owner 境界・シナリオ Expected
Oracle を備え、WUX-AC-01〜10 と WUX-SCN-A〜F で追跡可能である
こと。

## Rules

- **WUX-R-01 Docking 操作の一意性**: Docking 操作体系（タブ
  並べ替え・タブ化・分割・Window 分離・Window 間移動・再
  ドッキング・View 複製・Split 調整・View 閉鎖・Layout
  Undo/Redo）が操作 ID（WUX-DK-01〜10）で一意に列挙され、
  ドラッグとメニュー/コマンドの代替経路が同等操作を提供する。
- **WUX-R-02 ドラッグ安全性**: ドラッグ開始領域はタブ/専用
  ヘッダーのみ。Drop Preview は View 名・有効領域・確定結果を
  示し無効領域を適用不可と明示。Esc/失敗時はレイアウト不変・
  `ViewInstanceId`/重要 View State 保持で rollback。Project・
  Undo・Transport に影響しない。
- **WUX-R-03 閉鎖意味論**: View 閉鎖/サブ Window 閉鎖/メイン
  Window 閉鎖/最後の View 移動が区別され、Hidden（既定・復帰
  可能）と Discarded（明示・復帰不能）が分離される。サブ
  Window 閉鎖は Main Window へ自動再ドックせず detached pool
  保持。「閉じた Window を再表示」は Window 単位復帰。
- **WUX-R-04 Inspector 状態機械**: Follow/Explore/Pinned が
  明示遷移であり、Pinned は外部選択変更で Inspection Focus を
  上書きしない。Explore は Follow 元を保持するナビゲーション
  状態で戻る/進む履歴を持つ。Follow 元 View 喪失時は現対象を
  保持し明示操作を待つ。
- **WUX-R-05 Link Group 分離**: Link Group が同期するのは
  Linked Context（ハイライト・ナビゲーション文脈）のみ。
  Editing Selection の書換・編集コマンド連動・Group 外/
  Pinned/未対応 Mapping への勝手な同期を行わない。
- **WUX-R-06 入力境界**: 入力コンテキスト（View/TextInput/
  Ime/PluginGui/Modal）×コマンド種別の状態表でショートカット
  発火を決める。Delete は Active View の明示的選択のみ。IME/
  テキスト/Plugin GUI 中に DAW コマンドを誤発火させない。
  Context Menu は対象 ID を明示的にコマンドへ渡す。
- **WUX-R-07 Undo 3 系統分離**: Project Undo・Layout Undo・
  Text/IME Undo が別スタック。UI Close/Detach・失敗移動・
  Drop Preview・キャンセルは Project 変更も Undo 消費もしない。
  Window 閉鎖+View Hidden は Layout Undo 1 エントリ。
- **WUX-R-08 Layout 永続化**: Default/Saved/Last Session
  （+Workspace Preset は別 schema）を区別し Project Data と
  分離保存。モニター欠落・画面外座標は可視領域へ補正し、
  保存済み Layout を上書きしない。
- **WUX-R-09 シナリオ完全性**: WUX-SCN-A〜F が Issue #3 §8 の
  A〜F に 1:1 対応し、各々に手順・Expected Oracle・検証
  レーン（design-review/headless/gpui-test/real-os）がある。
- **WUX-R-10 Owner 境界と検証レーン分離**: Ruviform
  （Window/Dock/NativeSurface）・Synvelis（#112/#113/#114/
  #5/#11/#12）・headless 契約（WS-A/ED-A）の責務境界が明記
  され、real-os/Plugin GUI 未検証項目は NotRun/Deferred で
  あり設計評価を実機 Pass に換算しない。
- **WUX-R-11 文書チェーン完全性**: spec → design → plan →
  task → test-spec → impl(ir/exp) が存在し、`evidence/sv3/`
  に WUX-AC-* → WUX-SCN-* → 設計節 → Issue § の追跡表と検証
  記録がある。

## Examples

- **Dock 誤操作の復旧（WUX-SCN-D）**: Given Mixer が誤配置
  された When 専用 Layout Undo を実行しモニターを切断する
  Then 誤配置が安全に戻り、Project Undo/Transport に影響せず、
  Window/View を喪失せず可視領域へ補正復帰する。
- **Inspector 比較（WUX-SCN-C）**: Given Inspector A が Bass に
  Pinned、Inspector B が Follow のとき When Arrangement で
  Drums を選択する Then A は Bass を維持し B のみ Drums に
  追従し、A の Inspection Focus は不変。
- **入力衝突（WUX-SCN-F）**: Given テキスト欄・IME 変換中・
  Plugin GUI フォーカス中に Delete/Space/Undo が入力される
  Then DAW 側の編集/再生が意図せず発火せず、Undo は入力
  コンテキストに応じた系統へ配送される。
- **連携と編集分離（WUX-SCN-E）**: Given 同一 Link Group の
  Arrange/Mixer が別々の Editing Selection を持つとき When
  Track Context 連携が起きる Then ハイライトは連動するが
  Edit Command の作用先は変わらない。

## Guarantees

- spec の R1〜R8 と WUX-AC-01〜10 が design の規範節
  （§5〜§10）・WUX-SCN-A〜F（§11）・Decision Log（§12）・
  Owner 境界（§13）へ漏れなく写像される。
- 状態図（View/Window/Inspector/Dock Drag/Layout 復元）と
  遷移表・操作表が mermaid/表形式で存在し、内部 § 参照が
  改番後も正しい。
- 未決定事項（Drop 寸法・履歴深さ・保持期間等）は「分離/
  未決定」として明示され、確定済みであるかのように記述
  されない。
- 実装・実 OS 検証の完了を主張する記述が存在しない
  （NotRun/Deferred が明示される）。

## Non-goals

- 製品コードの動作検証（headless 契約は WS-A/ED-A の test-spec
  が担う。新規 UI 挙動の headless/gpui-test scenario 化は
  #112/#113/#114 の後続 slice）。
- 実 OS Window/Focus/IME/DPI/実 Plugin GUI の実測
  （#114/#176 lane、現時点 NotRun）。
- Inspector の情報階層・Provider・Probe 詳細（#5）、Tool/
  Selection/Snap/Gesture 詳細（#11/ED-A）、Recording 連携
  （#12）の個別仕様確定。
- `docs/mockups/synvelis-daw.html` の更新（旧 HTML Mock は
  設計参考であり正式仕様ではない）。

## Risk

本契約が破棄・矮小化されると、(1) #113/#114 が検証すべき
Oracle を失い実装と UX 仕様が乖離したまま進行する、(2)
Linked Context と Editing Selection の混同により Link Group
が編集コマンドを誤連動させる設計ミスを後続実装が再導入する、
(3) Layout Undo と Project Undo の混同で音楽的 Undo が破壊
される、(4) NotRun を Pass と誤記する運用違反が横行する。
これらは POLICIES.md §2/§3 と Issue #3 の明示禁止に反する
ため、本 test-spec は文書契約の存続中は維持する。
