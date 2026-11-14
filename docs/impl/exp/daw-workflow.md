---
id: "EXP-daw-workflow-docchain"
type: "experiment"
status: "completed"
title: "DAW ワークフロー UX 正本化 — 既存契約再利用と Gate A backbone 分解アプローチ"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/8"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/daw-workflow.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/daw-workflow-design.md"
    - "docs/specs/daw-workflow.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/daw-workflow-task.md"
  defers: []
  deferred-by: []
---

# DAW ワークフロー UX 正本化 — 既存契約再利用と Gate A backbone 分解アプローチ

## 背景と仮説

Issue #8 の UX 正本化は「#10/#11/#12/#18/#20/#21 の確定済み
headless 契約を前提参照し、ワークフロー横断の UX 規範
（Gate A backbone・3 WF-SCN・Keyboard/Gesture/Tool・非依存
境界）のみを新規に書く」アプローチが最も整合的と仮説を立てた。
対抗案は各 Owner 文書へワークフロー記述を分散追記する案
だったが、Issue #8 の成果物は「制作ワークフローとしての
受入 Oracle」であり、Owner 文書の責務（モデル・Tool・
Recording・Render の個別契約）を越えるため棄却した。

## 観察・試行

- **既存契約の棚卸し**: `docs/specs/product-value-core.md`
  （Gate A/B/C・AM-1 Owner）、`workspace-ux-scenarios`
  （操作分類・入力境界）、`project-model`（stable ID・
  単一 writer・Save/Reload）、`timeline-editing`
  （Selection/Tool/Gesture/Undo）、`recording-take-comping`
  （Session/Take/Recovery）、`playback-graph`/
  `transport-integration`/`media-render`（実行・時間・
  Import/Export）を読み、Gate A 最小タスクの各工程が
  既存正本で賄えることを確認。新規に定義すべきは
  「工程の束ね方・Oracle・横断安全契約」のみと判断した。
- **Gate A backbone の固定**: Issue #8 の必須最小タスク
  （Track 作成→Import/Record→非破壊編集→Mixer/Transport→
  Save/Reload→Export→独立検証）を GA-01〜08 に分解し、
  各工程に独立 Oracle（ファイル/hash/WAV decode/等価性）
  を割当てた。WF-SCN-MIDI/AUDIO/MIX はこの backbone を
  共有し、差分は素材取得（MIDI 入力 vs Audio Import/録音）
  と編集対象（Note vs Clip/Take）に限定される構造とした。
- **detour 契約**: WF-SCN-MIX の Signal Flow/Inspector/
  Version Diff は「Understand/Refine への文脈保持の任意
  detour」として spine から分離。これにより「診断/グラフを
  強制せずに完結」の受入条件を工程表レベルで保証した。
- **review-protocol**: spec+design に 2 pass（協調:
  カバレッジ・一貫性 / 敵対的: 反例・過剰主張）を実施。
  CONFIRMED 問題は全て P2 の文書バグ（design §2.2/§3 の
  節参照誤り、§7.3 gesture 行のラベル崩れ、spec の
  implemented-by 欠落・制作領域表欠落・P0/P1/P2 表欠落）
  で、P0/P1 blocker なし。修正後に plan-doc ゲート用に
  spec/design を approved 化した（タスク契約の承認代替）。
- **生成物命名**: impl-doc script が `issue-8-daw-ux.md`/
  `.jsonl` を生成したが、タスク許可範囲は
  `docs/impl/{ir,exp}/daw-workflow.md`。ir は許可パスへ
  移動し内容を記入、exp は repo 慣行（sv3 先例）の `.md`
  `type: "experiment"` で許可パスに作成。script 生成の
  `.jsonl` は許可範囲外のため削除した。

## 結果

- 文書チェーン 7 件 + evidence/sv8/ 2 件を作成。WF-AC-01〜10
  が Issue #8 受入条件 7 項目 + タスク固有 3 項目を網羅。
- review-protocol: 1 サイクル、CONFIRMED P0/P1 なし、
  blocker 残存なし → 承認代替として扱い plan/task/test-spec
  を approved/done で作成。
- doc_status lint で新規文書由来の blocking finding を
  解消（index 再生成・relation 双方向化・derives-from 追加）。
- 実機/実 GUI/実 Device 検証は NotRun/Deferred として
  evidence に記録。設計評価を実機 Pass に換算していない。

## 残課題

- Keyboard 具体的キー割当 → #112。
- Tool/Snap 数値・Take/Comp Editor UI → #11/#12。
- Remote Browser 詳細 → #9。
- Signal Flow/Inspector/Version Diff detour 詳細 → #4/#5/#7。
- 実機受入 → #114（GUI-L2）・#176（EXT-L3）。
