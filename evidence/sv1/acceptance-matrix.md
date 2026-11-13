# Acceptance Matrix 根拠 — synvelis#1 製品価値・中核体験

Date: 2026-11-14. 対象: `docs/specs/product-value-core.md` §11 の
Acceptance Matrix 6 行。各行の測定方法・合格基準・検証手段・
現状の達成度の根拠を記録する。canonical な判定規則は spec §4
（Gate 定義）と `docs/designs/product-value-core-design.md`
§2（証拠状態機械）を正本とする。

**重要:** 本ファイルの「現状の達成度」は文書・headless 証拠の
棚卸しであり、実 OS/RT/実 GUI/実 Plugin の製品受入を意味しない。
`NotRun`/`Unsupported`/`Unknown` を `Pass` にしない
（POLICIES.md §3）。

## AM-1 基本制作（Gate A）

- **Issue 根拠:** synvelis#1 §4 シナリオ A・2026-11-04 Gate 合意
  （A = AI/SF/VCS なしで成立する通常 DAW、必須）。
- **測定方法の根拠:** 代表タスク A の各ステップ（MIDI フレーズ→
  Audio トラック→音源/FX/Mixer→Save/Reload→再生→WAV
  Export→別 Reader 再読込）を実 GUI で実行し、操作数・誤操作・
  回復を記録。数値閾値は Baseline 後に別途合意（#1「恣意的に
  設定しない」）。
- **検証手段:** GUI-L2 実機 E2E = synvelis#114（SV-GUI-05 製品
  E2E）、OS-L1 headless 補助 = #8/#23 系の実装/受入、EXT-L3 =
  実 Export/再読込。
- **現状の達成度:** 定義は本書で合意。headless の製品
  State/Receipt 証拠あり — `evidence/sv114/svt-headless-l1.md`
  （SVT-01〜09: project create/select/fader/undo/transport/
  viz/2window/save-reload/IPC）。実 GUI 操作・実録音・実
  Export/再読込の製品受入は **NotRun**（#114・#115〜#118 系の
  Owner 証拠待ち）。

## AM-2 構造探索（Gate B）

- **Issue 根拠:** synvelis#1 §4 シナリオ B・Gate B 定義
  （取得可能な関係を探索し設定へ移動・検証・復帰。
  既知/観測/仮説を区別）。
- **測定方法の根拠:** Bass トラックの意図しない音量低下を題材に、
  Inspector/Signal Flow で Bus 出力・Sidechain・Automation の
  取得可能な関係を探索→設定移動→調整→制作 View 復帰。
- **検証手段:** headless oracle = SF-05（#108）signal flow
  hierarchy、SF-06（#109）cross-gate oracle、PC-C2 relation
  index（`docs/specs/pc-c2-control-relation-index.md`）、
  PC-C5-01 inspector 投影（`docs/specs/pc-c5-01-relation-
  inspector.md`）。GUI-L2 実シナリオ = #108/#109。
- **現状の達成度:** SF/PC 系の headless oracle・fixture 証拠
  あり。実 GUI での探索シナリオ受入は **NotRun**。

## AM-3 文脈維持

- **Issue 根拠:** synvelis#1 受入条件「文脈維持」+ Owner 指定
  #2/#3/#114。
- **測定方法の根拠:** 探索中の Editing Selection・Inspector
  Follow/Explore/Pinned・戻り先を 2 Window 含む代表操作で確認。
- **検証手段:** 設計 = #2/#3
  （`docs/designs/workspace-docking-state-architecture.md` が
  Selection/Focus/Link Group/Command Routing を所有）、
  headless = ws-a foundation・sv-gui-04 two-window/close-veto・
  PC-C5-02/03 gesture/stale、実機 = #114 GUI-L2。
- **現状の達成度:** 設計と headless 契約は存在
  （`docs/specs/ws-a-workspace-foundation-view-selection-
  linkgroup-command-dock.md`、`docs/specs/sv-gui-04-*.md`）。
  実機 multi-window・実 focus の受入は **NotRun**（GUI-L2）。

## AM-4 調整への接続（Gate C）

- **Issue 根拠:** synvelis#1 §4 シナリオ C・Gate C 定義（制御元→
  対象・対象→制御元の双方向探索→調整→Undo→制作復帰）。
- **測定方法の根拠:** Filter Cutoff の制御元探索を題材に、
  クリック/操作数・誤操作・作用 Target を観測できる評価手順。
  Owner #6/#102/#103。
- **検証手段:** headless = PC-C5 relation inspector・
  PC-C6 REL/TR/UI oracle（`docs/specs/pc-c6-oracle-rows.md` 等）、
  GUI-L2 = #102/#103 の実シナリオ。
- **現状の達成度:** PC-C5/C6 の headless oracle 証拠あり。
  実 GUI での双方向探索→調整→Undo 評価は **NotRun**。

## AM-5 情報の正確性

- **Issue 根拠:** synvelis#1 受入条件「情報の正確性」+ Owner
  #5/#109/#103。
- **測定方法の根拠:** 表示内容と実際に取得可能な設定/状態の照合。
  Configured/Active/Observed/Unknown・欠測/未対応/未検証を誤って
  同一結果にしない。Mock/Native GUI/RT/Plugin の Oracle を別
  所有者・結果状態で識別。
- **検証手段:** PC-C3 ControlTrace slot
  （`docs/specs/pc-c3-control-trace-projection.md`:
  Configured/Prepared/Applied/Reported/Observed + Missing/
  Unsupported/Dropped/Expired）、SF-04 probe adapter、PC-C6
  oracle、`syn.app.info` の plugin unavailable honest reporting
  （`evidence/sv114/svt-headless-l1.md`）。
- **現状の達成度:** 状態区別の headless 検証済み。実表示と実
  状態の一致（GUI-L2）・実機器/Plugin での一致（EXT-L3）は
  **NotRun**。

## AM-6 競合比較

- **Issue 根拠:** synvelis#1 §7・§8「競合比較」+「数値目標は恣意的
  に設定せず、試験内容・基準値を定めてから決定」。
- **測定方法の根拠:** 同一初期条件の代表タスク A〜C を Synvelis
  と既存 DAW で実施。計測項目 = 達成率/所要時間/操作数/誤操作・
  回復/主観的理解度/探索コスト。環境・参加者条件を記録。
- **検証手段:** spec §9 の UX 比較計画 + ユーザー調査（未実施）。
  判定は Baseline（既存 DAW 側）計測→Synvelis 側計測→閾値の
  別途合意の順。
- **現状の達成度:** 手順・評価基準・計測項目を定義済み。
  比較対象 DAW の最終リスト・参加者・閾値は未決定
  （Deferred）。Baseline 未計測・ユーザーテスト未実施 →
  **NotRun**。実測前に性能優位を宣言しない。

## 証拠状態の現在値（2026-11-14 時点）

| Gate | 状態 | 根拠 |
| --- | --- | --- |
| A — Core DAW Creation | Headless verified（途中証拠） | SVT-01〜09 等。Real GUI/Platform/Product accepted は未到達 |
| B — Structural Investigation | Headless verified（途中証拠） | SF-05/SF-06・PC-C2/C5 oracle。実 GUI シナリオ未到達 |
| C — Bidirectional Control | Headless verified（途中証拠） | PC-C5/C6 oracle。実 GUI 評価未到達 |
| AI — Gateway Integration | Not started（定義のみ） | #44/#46・GW-01〜10 が Owner。Gate A の依存にしない |
| AM-6 競合比較 | Not started | Baseline 未計測 |
