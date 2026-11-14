# SV-GUI-03 slice6 Keyboard/Gesture key-assignment contract — Case Ledger

Issue #112 docs-only スライス(sv-gui-03-06)の case ledger。
daw-workflow(Issue #8)が #112 へ委譲した項目 → 本スライスの
契約セクション → KG-AC → 受入状況を対応付ける。

## 受入状況の語彙(POLICIES.md §2/§3)

- **Pass**: 契約文書が当該項目を検証可能な形で規定し、
  文書レベル検証を通過。
- **NotRun**: 実機/実 GUI/実 OS での検証が必要だが本スライス
  では未実施。
- **Deferred**: 後続 slice/Issue へ明示的に委譲。
- **Unsupported**: 対象環境・デバイスが存在しない。

## 委譲項目 → 契約セクション → 受入状況

| # | daw-workflow 委譲項目 | 契約セクション | KG-AC | 受入状況 | 備考 |
| --- | --- | --- | --- | --- | --- |
| D-1 | §7.2 最小キーボード操作集合の具体的キー割当(修飾キー含む) | design §3.2 KG-KEY-01〜15 | KG-AC-01 | **Pass**(文書) | 全操作に contract command id+キーを割当。Export は「既定バインドなし+理由」(KG-D-3) |
| D-2 | §12 D-3・§13.2 の OS 別既定バインド・OS 差異正規化 | design §3.3(KG-D-1/KG-D-2) | KG-AC-02 | **Pass**(文書)/**NotRun**(実機) | `Mod` 論理修飾キーで Win/Linux=Ctrl・macOS=Cmd を正規化。実 OS 既定バインド確認は #114 |
| D-3 | K-3 の具体化: `InputContext` 別 dispatch/suppression | design §4.1 発火行列・§4.2 pipeline・§4.4 ゲート配置 | KG-AC-03 | **Pass**(文書) | `captures_input`・kind 別 suppression・`dispatch_undo` 配送と整合。View/Application ゲート配置は Deferred |
| D-4 | K-2(Preview→Atomic Commit・Esc 階層取消)のキー操作適用 | design §5.1 | KG-AC-04 | **Pass**(文書) | keymap は Preview を開始せず、Preview 中は編集系を抑制、Esc=最内側取消 |
| D-5 | K-4(View ごと独立 Tool 状態・Window/Dock 不変)のキー操作適用 | design §5.2 | KG-AC-04 | **Pass**(文書) | `tool.activate` は origin_view 単位、active view 未解決は Unresolved |
| D-6 | キー競合の解決ポリシー(重複バインド・コンテキスト間衝突) | design §5.4 KG-C-1〜6 | KG-AC-04 | **Pass**(文書) | lane 内一意性・context lane 分離・chord 非導入・OS 予約回避・リマップ競合拒否 |
| D-7 | auto-repeat(キーリピート)方針 | design §5.3(KG-D-5) | KG-AC-04 | **Pass**(文書) | scroll/undo のみ repeat 許容、他は edge-triggered |
| D-8 | spec「Deferred Design Concerns」のコンテキストメニュー完全網羅 | design §6 CM-1〜6・§6.2 対応表 | KG-AC-05 | **Pass**(文書) | contract command→メニュー面対応表+網羅規則。各面の最終文言/構造は View slice へ Deferred |
| D-9 | headless 受入ケースの定義 | test-spec TS-DOC/TS-EX | KG-AC-06 | **Pass**(文書) | TS-R-01〜07(文書検証)・TS-EX-01〜11(headless 契約試験仕様)を定義 |
| D-10 | 実 GUI key dispatch・実 OS/GUI ゲートの正直な記録 | impl ir/exp・本台帳・verification.md | KG-AC-07 | **Deferred** | #114 GUI-L2・#176 EXT-L3。headless 成功を実機 Pass に昇格しない |

## 検証レーン別受入状況(POLICIES.md)

| レーン | 対象 | 状況 | 根拠 |
| --- | --- | --- | --- |
| OS-L1(文書契約) | spec/design/plan/task/test-spec/ir/exp の整合・index 再生成・lint/audit/fmt | **Pass** | `evidence/sv112/verification.md` にコマンド・exit code を記録 |
| headless 契約試験(TS-EX) | keymap テーブル一意性・OS 正規化・suppression 行列・競合拒否・メニュー網羅 | **Deferred** | 実装 slice で実走。契約試験の仕様のみ test-spec で定義 |
| GUI-L2(実機) | 実 Window/実キー入力/IME/Focus/DPI での dispatch・既定バインド | **NotRun** | #114 lane。docs-only スライス対象外 |
| EXT-L3(実機) | 実 Plugin GUI でのキー所有・ホスト規約 | **NotRun** | #176 lane。実機・実プラグインが必要 |

## 既知の差異・残課題

- **doc-driven-dev-graph router**: 既存の無関係な broken
  relation(automation/recording/MIDI/VCS/impl 文書からの
  未解決 `relations.defers`)で `probe` node が blocked を
  返す。本スライスの文書は宣言済み順で手動継続。router 結果を
  偽装せず、既存 graph 債務として記録(範囲外・修復しない)。
- **`Ime` Undo 配送の不整合**: workspace-ux §9.1 発火表は
  Ime Undo を「発火しない」とするが、現行 `dispatch_undo` は
  Ime を text lane へ配送。契約は現行コード挙動に合わせて
  §4.1 に記述し、実機検証・文書照合は #114 へ委譲
  (design R-3・test-spec Risk)。
- **`[contract]` command id**: `transport.toggle`/
  `transport.stop-return`/`transport.record`/`project.save`/
  `project.export`/`edit.delete`/`edit.copy`/`edit.paste`/
  `view.zoom-in`/`view.zoom-out`/`view.scroll-*` は
  `spec_for` 未登録の契約先行 id。登録は実装 slice の責務。
- **View/Application suppression 配置**: `dispatch` は
  View/Application kind を抑制しないため keymap 層ゲートが
  必要。配置(keymap 層 vs `dispatch` 拡張)は実装 slice の
  選択に委譲し、契約は振る舞いのみ固定(KG-D-4)。
