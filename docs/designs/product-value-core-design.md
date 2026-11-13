---
id: "DESIGN-034X3jMjITd6SCoqrUyFnH"
type: "design"
status: "approved"
title: "Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価設計"
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
  implements:
    - "docs/specs/product-value-core.md"
  implemented-by: []
  depends-on:
    - "docs/specs/product-value-core.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/product-value-core-plan.md"
    - "docs/tasks/product-value-core-task.md"
    - "docs/test-specs/product-value-core.md"
    - "docs/designs/workspace-docking-state-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/product-value-core.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/product-value-core.md"
  references: []
  defers: []
  deferred-by: []
---

# Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価設計

## Context

Issue #1 は製品価値・中核体験・Gate A/B/C/AI・UX 評価計画の**定義**
を対象とする。個別機能の実装方式・データモデル・OS 対応は対象外
であり、本設計は「定義をどう構造化し、どの証拠をどの段階に
対応づけ、誰が受入を所有するか」を固定する。実装 Owner は #8/
#23・#4/#6・#102/#103・#108/#109・#114・#2/#3・#44/#46 であり、
本設計はそれらへの相互参照と判定規則を与える。

## Scope

### In Scope

- Gate A/B/C/AI の構造化定義と証拠状態機械（§1〜§3）。
- Create → Understand → Refine の UX フローと説明可能性の境界
  （§4〜§5）。
- Acceptance Matrix 6 項目の Owner 境界・代表 Normal/Failure
  シナリオ・Expected Oracle の対応（§6〜§8）。
- Decision Log（採用・非採用・根拠）（§9）。
- 既存 `docs/` との重複・矛盾の調査結果と解消方針。

### Out of Scope

- Signal Flow / Inspector の画面設計・操作仕様（#4/#5/#108/
  #102 の設計 Issue）。
- Project Model・Control Graph・Automation/Modulation のデータ
  モデル（#17〜#21 系）。
- 実装計画のタスク分解（#23）と個別 PR の受入。
- OS/RT/実 GUI/実 Plugin の受入判定そのもの — 本設計は判定規則
  を与えるだけで、判定は Owner Issue の証拠で行う。

## Components and Boundaries

| 構成要素 | 責務 | 正本 |
| --- | --- | --- |
| 製品価値定義（三層構造・中核体験・UX 原則） | 何を価値とするか | `docs/specs/product-value-core.md` |
| Gate A/B/C/AI 定義 | 段階的受入の目的・合格条件・証拠区分 | 本書 §1〜§3 + spec §4 |
| Acceptance Matrix | 6 検証観点の測定・判定・現状 | spec §11 + `evidence/sv1/acceptance-matrix.md` |
| UX 比較計画 | 既存 DAW との同一タスク比較手順・評価基準 | spec §9 |
| 実装・検証計画 | Gate の実装スライスと受入 Owner | #23（Execution Plan） |
| 個別 UX/設計 | Workspace/Inspector/Signal Flow/Automation の操作設計 | #2/#3/#4/#5/#6/#8 |
| 製品 E2E・実機受入 | GUI-L2/EXT-L3 の実機証拠 | #114・#109・#103・#176 系 |

境界規則: 本設計が「定義の構造化」を所有し、実装の「どう作るか」
は各 Owner Issue に委譲する。設計合意は実装・実機受入の完了を
意味しない。

## Gate 定義の構造化

### 1. Gate 間の依存関係

```text
Gate A (Core DAW Creation)  … 必須。AI/SF/VCS なしで成立
   │  B/C/AI は A を無条件にブロックしない
   ├─ Gate B (Structural Investigation) … 最初の差別化検証
   │     B の共通探索契約を C が再利用
   ├─ Gate C (Bidirectional Control)    … 段階的拡張
   └─ AI Gate (Gateway Integration)     … 独立受入軸。既定 OFF
```

- **A は製品価値の最低成立条件。** B/C/AI の未完成は A の受入を
  妨げない。
- **B は差別化体験の実証 Gate。** 「関係を根拠つきで探索し、
  設定へ移動・検証・制作復帰できる」ことを製品として証明する。
- **C は B の探索契約の双方向・制御関係への拡張。** A を
  ブロックしない。
- **AI は明示的に有効化したときだけの受入軸。** AI Off 時の通常
  DAW 非回帰が AI Gate の一部であり、AI を使わず A が成立する
  ことが前提。

### 2. 証拠状態機械

各 Gate・各 Acceptance Matrix 行は独立に次の状態を取る:

```text
Not started → Headless verified → Real GUI verified → Platform verified → Product accepted
```

| 状態 | 意味 | 例 |
| --- | --- | --- |
| Not started | 証拠なし・未着手 | AM-6 UX 比較（Baseline 未計測） |
| Headless verified | OS-L1 の headless/契約/fixture/oracle で検証済み | SVT-01〜09（`evidence/sv114/`）、SF-06 oracle、PC-C6 fixture |
| Real GUI verified | GUI-L2: 実 OS Window/Input/Focus/表示の検証済み | 未到達（Ruviform #310〜#314 系 gate） |
| Platform verified | EXT-L3: 実機器・実 Plugin・複数 OS の検証済み | 未到達 |
| Product accepted | Owner Issue で製品受入完了 | 未到達 |

昇格規則: 下位状態の証拠は上位状態を自動的に満たさない。headless
E2E・GPUI-simulated・Mock・契約 fixture は途中証拠であり、実 UI
操作の完了ではない。`NotRun`/`Unsupported`/`Unknown` を `Pass`
に読み替えない（POLICIES.md §3）。

### 3. Owner 境界

| Gate / 領域 | 定義 Owner | 実装・検証 Owner（Issue） |
| --- | --- | --- |
| 製品価値・中核体験・Gate 定義・Acceptance Matrix | 本書（#1） | — |
| Gate A 標準制作ワークフロー | #8 | #23（実行計画）・#114（製品 E2E）・#115〜#118（Recording 系） |
| Gate B Signal Flow | #4 | #106（SF-03 map）・#108（SF-05 Editor）・#109（SF-06 oracle） |
| Gate C 制御関係 | #6 | #102（PC-C5 UI）・#103（PC-C6 oracle） |
| 文脈維持（Workspace/Docking/Selection） | #2・#3 | ws-a 系・sv-gui-04 系・#114 |
| 情報の正確性（状態語彙・Oracle） | #5・#109・#103 | PC-C3 trace・SF-04 probe・PC-C6 oracle |
| AI Gateway | #44・#46 | GW-01〜10（#46）・#60 系 |
| Creative Branch | #277（CB-00） | #278〜#288・Ruviform #562 |
| Consumer 互換・Ruviform 追従 | #28 | Ruviform #294 cross-consumer gate |

## 中核体験の UX フロー

```text
        ┌────────────────────────────────────────────┐
        │            Create（通常制作）               │
        │  Arrangement / Mixer / Plugin / 録音 / 書出 │
        └───────┬───────────────────────▲────────────┘
                │ 必要時に離脱              │ 調整後に復帰
                ▼                       │
        ┌────────────────────────────────────────────┐
        │  Understand（構造の理解）                    │
        │  選択対象 → Where/What/Why/How を確認        │
        └───────┬───────────────────────▲────────────┘
                │                         │
                ▼                         │
        ┌────────────────────────────────────────────┐
        │  Refine（判断・調整）                       │
        │  関連設定へ移動 → 調整 → 結果確認 → 制作へ戻る│
        └────────────────────────────────────────────┘
```

不変条件:

- 各フェーズは強制モードではなく、文脈（選択対象・Inspector
  対象・View 状態）を維持したまま遷移する（Context Continuity）。
- Understand は双方向にたどれる（Bidirectional Navigation）。
- Refine の調整は Undo 可能で、作用 Target が明示される。
- 表示は取得可能な範囲を明示し、未観測を断定しない
  （Information Integrity）。

## 説明可能性の境界と状態語彙

製品レベルの説明枠組み Where / What / Why / How を、実装側の
証拠状態へ対応づける。製品語彙は「設定として既知の事実・実測値・
推定・取得不能」、実装側の代表語彙は PC-C3 ControlTrace の
`Configured / Prepared / Applied / Reported / Observed` slot と
SF-04 probe の観測値である。

| 製品語彙 | 意味 | 対応する実装証拠 | 断定してはいけないこと |
| --- | --- | --- | --- |
| 設定として既知の事実 | Project/設定モデル上の接続・パラメータ・ルーティング | `Configured`、semantic routing model（SF-01/02） | 「この接続が音量低下の原因である」 |
| 実測値 | probe/RT/出力から観測した値 | `Observed`、SF-04 probe trace | 「この値が意図した設定の結果である」（因果は別証拠） |
| 推定 | 観測からの仮説・相関 | `Reported`（plugin 可視報告）等の間接証拠 | 「確定した原因」 |
| 取得不能 | ホストが観測できない領域（第三者 plugin 内部等） | `Unsupported`/`Missing`/`Unknown` slot | 存在しないものを推測で埋める |

原則: **接続が見えることと原因が証明できることは別**。Gate B/C
の合格条件は「取得可能な関係を区別つきで提示し、設定へ移動・
検証・復帰できること」であり、万能な因果推定を要求しない。

## Acceptance Matrix の Owner 対応

| AM | 測定の実施主体 | 補助証拠（現状） | 製品受入の Owner |
| --- | --- | --- | --- |
| AM-1 基本制作 | #114 実 GUI E2E + #8/#23 計画 | SVT-01〜09 headless（`evidence/sv114/`）、録音 Mock/Fault 系 | #23 Gate 判定・#114 |
| AM-2 構造探索 | #108/#109（SF-05/06）+ #4 定義 | SF-05 headless hierarchy、SF-06 oracle、PC-C2 relation index | #109 Cross Gate・#4 |
| AM-3 文脈維持 | #2/#3 設計 + #114 実機 | ws-a foundation、sv-gui-04 two-window/close-veto、PC-C5-02/03 gesture/stale | #2/#3・#114 |
| AM-4 調整への接続 | #102/#103（PC-C5/C6）+ #6 定義 | PC-C5 relation inspector、PC-C6 REL/TR/UI oracle | #103 横断 Gate・#6 |
| AM-5 情報の正確性 | #109/#103 oracle + #5 Inspector | PC-C3 trace slot、SF-04 probe、`syn.app.info` honest reporting | #109・#103 |
| AM-6 競合比較 | 製品評価計画（本書 §9）→ ユーザー調査 | なし（Baseline 未計測） | 製品 Owner（閾値は別途合意） |

## 代表 Normal / Failure シナリオと Expected Oracle

評価タスクの Expected Oracle は `docs/test-specs/product-value-core.md`
の Feature/Rule を正本とする。ここでは Gate 判定に直結する代表
ケースを列挙する。

### Normal

- **N-A1（Gate A）:** AI/SF/VCS を一切開かず、MIDI フレーズ作成→
  Audio トラック追加→FX/Mixer→Save/Reload→再生→WAV Export→
  別 Reader 再読込が完了する。Expected Oracle: 各ステップの
  receipt/状態が実 Service で観測でき、Export PCM が独立 Reader
  で検証できる。
- **N-B1（Gate B）:** Bass トラックで意図しない音量低下に気付き、
  Inspector/Signal Flow で Bus 出力・Sidechain・Automation の
  **取得可能な**関係を確認し、設定へ移動・調整・制作 View へ復帰
  する。Expected Oracle: 既知接続と観測事実と仮説が区別表示され、
  未観測の因果を断定しない。
- **N-C1（Gate C）:** Filter Cutoff を選択し、Automation/LFO の
  制御元→対象と対象→制御元を双方向にたどり、調整→Undo→制作復帰
  する。Expected Oracle: 双方向の関係が一貫し、Undo で元に戻る。
- **N-CTX（文脈維持）:** Mixer/Inspector/Signal Flow を移動しても
  Editing Selection・Inspection Focus・View 状態が維持され、
  2 Window 環境でも対象を見失わない。Expected Oracle: 移動前後で
  対象 ID・選択・View state が一致する。

### Failure / 境界

- **F-1（情報の正確性）:** 第三者プラグイン内部の変化など観測
  不能な事象で、UI が原因を断定表示しない。Expected Oracle:
  `Unknown`/`Unsupported`/`Missing` が `Observed` と区別される。
- **F-2（文脈喪失）:** View の Detach/Close/再アタッチや Window
  移動で、View ID・編集選択・Project 状態が失われない
  （失敗時はロールバック）。Expected Oracle: 移動失敗時も元の
  所属・状態が保持される。
- **F-3（誤対象操作）:** Inspector 検索欄やプラグイン GUI 入力中に
  Delete/Space 等が DAW 編集・再生を誤発火しない。Expected Oracle:
  入力コンテキストが優先され、曖昧な破壊的操作は何もしない
  （workspace-docking-state-architecture §6 の優先順位）。
- **F-4（Gate 誤昇格）:** headless/GPUI-simulated の pass を実 GUI
  pass と読み替えない。Expected Oracle: 証拠状態が
  `Headless verified` のまま `Real GUI verified` に昇格しない。

## Decision Log

| 決定 | 採否 | 根拠 |
| --- | --- | --- |
| 三層構造 Create/Understand/Refine を製品価値の正本とする | 採用 | Issue #1 §2 の合意。②表示充実≠③価値の区別を明文化 |
| 中核体験を強制モードでなく連続体験とする | 採用 | Issue #1 §3。文脈維持が AM-3 の合格条件 |
| Gate A 必須・B 差別化優先・C/AI 独立 | 採用 | Issue #1 2026-11-04 合意。B/C/AI が A を無条件ブロックしない |
| AI Gate を独立軸・既定 OFF・AI Off で A 成立 | 採用 | 同上。AI を通常 DAW の必須依存にしない |
| 証拠状態 5 段階を Gate/行ごとに独立記録 | 採用 | POLICIES.md §2/§3 と整合。headless pass の昇格を防ぐ |
| 数値閾値を Baseline 計測後に決定 | 採用 | Issue #1「恣意的に設定しない」明示。実測前の優位宣言を禁止 |
| 比較対象 DAW 初期候補 = REAPER/Bitwig Studio | 採用（候補） | Issue #1 §7 の既存言及に基づく。最終リストは評価レビューで決定 |
| 全画面ノード化 | 非採用 | Issue #1 §1「ノード化しない」明示 |
| 因果の完全推定保証 | 非採用 | Issue #1 §3/§9。観測不能領域を説明可能とみなさない |
| Creative Branch を Gate A のブロッカーにする | 非採用 | Issue #1 2026-11-11 追加。別 Feature Acceptance |
| 初期リリースでの B/C/AI 必須化の自動決定 | 非採用（保留） | Issue #1「今回のゲート定義から自動決定しない」。製品 Owner の別途決裁 |

## 既存 docs との重複・矛盾の調査

調査結果の正本は `evidence/sv1/issue-1-scope-map.md`。要約:

- `docs/designs/workspace-docking-state-architecture.md` は #1 を
  前提として Workspace/Docking を設計しており、重複ではなく下流
  の実装設計。矛盾なし（AM-3/文脈維持の設計詳細を担う）。
- `docs/specs/ws-a-*`、`sv-gui-*`、`sf-*`、`pc-c5-*`、`pc-c6-*` は
  個別機能の spec であり、本書はそれらを束ねる製品定義。用語の
  対応（Configured/Active/Observed/Unknown ↔ PC-C3 slot）は §5 で
  明示した。
- 矛盾・重複の解消が必要な既存記述は確認されなかった。解消方針:
  製品価値・Gate・Acceptance Matrix の正本を本書チェーンに一本化
  し、下流 docs は本書を `related`/`source` として参照する運用と
  する（各下流 docs の更新は本タスクの編集範囲外）。

## Risks and Trade-offs

- **リスク:** 本書が「定義の合意」と「製品受入」を混同される。
  **対策:** 証拠状態機械と Owner 境界を明示し、Acceptance Matrix
  の「現状の達成度」列で NotRun を正直に記録する。
- **リスク:** UX 比較の閾値が恣意的に設定される。**対策:** §9.4
  で Baseline 先行・閾値の別途合意を手順として固定。
- **トレードオフ:** Creative Branch を別 Acceptance とすることで
  Gate A の判定が単純になる一方、CB 完成を製品価値に含める判断は
  製品 Owner の将来決裁に残る（Deferred Design Concerns）。

## Deferred Design Concerns

- UX 比較の数値閾値・比較対象 DAW 最終リスト・参加者条件 —
  Baseline 計測と評価レビュー後（再評価契機: AM-6 初回計画）。
- Creative Branch の Feature Acceptance 詳細 — #277/#288 が正本。
- 初期リリースでの B/C/AI 必須化 — 製品 Owner の別途決裁。

## Verification Notes

- [x] spec の Acceptance Criteria（PV-01〜09・AC 全項目）を本文書
  が満たすことをレビューで確認。
- [x] Acceptance Matrix の 6 項目が測定方法・合格基準・検証手段・
  現状の達成度を持つことを確認。
- [x] 実装 Owner Issue への相互参照が張られていることを確認。
- [x] 設計合意のみで OS/RT/実 GUI 受入を済ませない記述があること
  を確認。
