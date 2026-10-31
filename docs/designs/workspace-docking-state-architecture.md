# Synvelis Workspace / Docking / Multi-Window 統合設計

- **文書状態:** 初期設計ベースライン（基本方針を承認済み、詳細API・OS挙動は提案/PoC待ち）
- **対象:** SynvelisのWorkspace / Docking / Selection / Focus / Command Routing、およびRuviformへの要求
- **前提:** [製品価値と中核体験 Issue #1](https://github.com/scarecrow173/synvelis/issues/1)
- **Ruviform側の依存課題:** [Multi-Window Host #181](https://github.com/scarecrow173/ruviform/issues/181) → [Docking基盤 #182](https://github.com/scarecrow173/ruviform/issues/182)
- **参考:** [Ruviform GUIホスト #156](https://github.com/scarecrow173/ruviform/issues/156)、[現行GUIホスト設計](https://github.com/scarecrow173/ruviform/blob/main/docs/designs/issue-156-gui-host-foundation.md)、[UI方針ADR](https://github.com/scarecrow173/ruviform/blob/main/docs/adr/ui-architecture-webview-hybrid.md)

> 本書は**製品の合意済み方向性と、その実現に向けた設計提案**を整理する。コード・API・テストが既に実装されたことを意味しない。

## 1. 目的・確定した要求

Synvelisは汎用DAWとして、通常のArrangement / Mixer操作を妨げずに「Create → Understand → Refine」をつなぐ。Viewをタブ化・分割・独立Windowへ切り離し・再ドッキングし、複数モニターへ自由に配置できることは**初期アーキテクチャから対応すべき必須要件**であり、後付けの拡張機能ではない。

- **Shared Project, Independent Views:** 同一Project Sessionの制作データとTransportを共有しつつ、Viewごとの編集選択・調査対象・表示範囲を独立して扱う。
- **Explicit Synchronization:** Link Groupで意味的な選択/ナビゲーションを必要な範囲だけ連携する。編集コマンドを他Viewへ自動複製しない。
- **Context Continuity:** Viewの切り離し/移動/復元によって論理上のView IDと持続すべき状態が失われない。
- **Safe Commands:** フォーカス、調査対象、編集選択、実際のコマンド対象を混同しない。曖昧な編集操作は他Windowの過去の選択へ転送しない。
- **RT Separation:** UI/Window/Dock操作は非RT経路で行い、RT処理を待たせたり、Windowの移動だけでAudio Graphを再生成したりしない。

### 非目標

- 全画面をノードベース化すること。
- 単一のグローバルEditing Selectionを全Windowへ強制すること。
- DOM/JSオブジェクトそのものをWebView間で移送すること。
- プラグイン独自GUIを通常のDockable Viewと同じ所有・キーボード契約に強制すること。
- 初期から複数Projectを単一Windowに混在させること、遠隔共同編集すること。
- 現時点で全OSの実機挙動を検証済みとみなすこと。

## 2. 論理構造と責務

```text
Application
  ├─ Project Session (Synvelisの制作データ正本 + コントローラー)
  ├─ Workspace Session (Window群・Dock Layout・View Registry・Link Group)
  │    ├─ Window A [native window + WebView] → Dock Tree A
  │    │    ├─ Arrangement (View A1)
  │    │    └─ Signal Flow (View A2)
  │    └─ Window B [native window + WebView] → Dock Tree B
  │         ├─ Mixer (View B1)
  │         └─ Inspector (View B2)
  └─ Audio Engine (別のRT実行契約。UI Windowに所有させない)
```

| 要素 | 定義 | 主たる責務 |
| --- | --- | --- |
| `ProjectSession` | 1つの制作プロジェクトに対応する共有セッション | Project Model、編集コマンドとUndo/Redo、Transport、Engineとの連携 |
| `WindowId` | 実行中のネイティブWindowの識別子 | OS Windowのイベント発信元を区別（永続IDとは分ける） |
| `DockTree` | 各Windowに属する分割・タブ配置 | 単一Window内のレイアウト。全WindowでViewの所属一意性を検証 |
| `ViewInstanceId` | Viewの論理インスタンスの安定したID | 同種のViewを複数持ち、再ドッキングしても論理状態を引き継ぐ |
| `ViewType` | Arrangement、Mixer、Signal Flow、Inspector等 | View Factoryへ表示構築を要求する |
| `WorkspaceSession` | Window・Dock・Viewの集合 | Layout、リンク構成、起動時復元、移動・閉鎖ポリシー |
| `LinkGroup` | View間のナビゲーション連携単位 | 対象種類に応じたSelection Mapping、購読と発信元識別 |
| `CommandRouter` | 入力イベントの意味付けと配送 | Origin/Focus/Command種別/Targetを解決しProject Controllerへ渡す |

**重要:** `WindowId` と `ViewInstanceId` は別物。Windowは複数Viewを含み、Viewは移動によって所属Windowが変わる。DOM/ネイティブWindowの寿命をProjectまたは論理Viewの寿命と一致させない。

## 3. Dockingデータモデル（論理スキーマ案）

```text
WorkspaceLayout
  schema_version
  windows: [WindowLayout]
  views: [ViewDescriptor]
  link_groups: [LinkDescriptor]

WindowLayout
  layout_window_key       # 永続化用。OS/実行時WindowIdとは別
  placement               # monitor hint, bounds in logical units, state
  root: DockNode?         # windowのDock Tree

DockNode =
  Split { axis, ratio, first: DockNode, second: DockNode }
  | Tabs { views: [ViewInstanceId], active: ViewInstanceId }

ViewDescriptor
  view_id, view_type, persisted_view_state

LinkDescriptor
  group_id, members: [ViewInstanceId], topics
```

### 構造不変条件

1. 生存中のViewは、全Windowを通じて**1つのTabsに高々1回**現れる。表示前の未配置ViewはRegistryで明示的に管理し、孤児状態を隠さない。
2. 1つのWindowは1つのDock Treeルートを持ち得る（空Windowは明示的状態）。`Tabs` のアクティブViewは所属View集合に含まれる。
3. `Split` の子の数・分割比、重複ID、循環、空Tabs、無効な参照は操作時・復元時に検証または正規化する。
4. Viewの複製は**新しいView IDの生成**であり、Window間移動では**既存View IDを維持**する。
5. Windowを閉じてもView StateやProject Modelは自動で破棄しない。破棄/再配置/終了のポリシーはアプリ側で決める。
6. Monitorは安定した識別子と仮定しない。保存したWindow座標が不可視の場合は利用可能な画面内へ安全に復帰する。

### Docking操作

`Split` / `Tabify` / `Reorder` / `MoveToWindow` / `Detach` / `Reattach` / `ResizeSplit` / `CloseView` を**検証可能なレイアウト操作**として定義する（名前は仮）。ドラッグ中のプレビュー等はTransient UIであり、ドラッグ確定時に操作を適用する。Drop先の検出・指差しUIと、正当なDock Treeへの変換を分離する。

- レイアウト保存/復元はバージョン付きとし、読み込みエラーでは安全な標準レイアウトを利用する。
- Layout操作はProjectの音楽的Undo/Redoに混入させない。
- 同種Viewを複数開くことを想定し、View Instanceが何を編集・観測するかはSynvelisの設定として保持する。

## 4. 状態の正本と更新経路

| 状態 | 正本 | 保存・共有 |
| --- | --- | --- |
| **Project State** | `ProjectController` が管理するProject Model | 全Window共有。プロジェクトとして保存 |
| **Engine State** | Ruviform経由のRT実行系 | Projectの変更を非RT→RT境界で反映。UIが直接所有しない |
| **Workspace Layout** | `WorkspaceManager` | Window/Dock/Link設定。ユーザー環境設定として保存可能 |
| **View State** | `ViewStateRegistry`（論理View単位） | 編集選択、調査対象、Zoom、Scroll、表示モード。必要部分を永続化 |
| **Host Focus** | OS・ネイティブGUIホスト | 現在のWindow/入力先。終了・復元で推測しない |
| **Transient State** | 各WebView / UI component | Hover、ドラッグ、IME、開いたメニュー。通常は永続化しない |

UIはProject Modelのコピーを**表示・編集用の投影（read model）**として保持してよいが、Window別に独立した変更可能なProjectの正本を作らない。

- Project変更はOriginとTargetを持つコマンドとして中央のControllerへ送る。Controllerが順序付け、妥当性検証、Undo/Redo、revision発行を担う。
- Viewは必要な状態を購読して更新する。通知は重複配信・遅延・取りこぼしを考慮し、revisionによる再同期手段を設ける。
- メーター等の高頻度情報は購読者に必要な頻度で配信し、巨大な描画データを全Windowへ無条件broadcastしない。
- IPCの呼出相関キーはWebView間で衝突し得る。Ruviformホスト側では**発信元Window/WebView + request ID**の組を識別できるようにする（#181）。

## 5. Selection / Focus / Link Group

### 5.1 独立して扱う5概念

- **Native Window Focus:** OSが現在アクティブとするWindow。
- **Input Focus:** そのWindow/WebView内で実際にキー入力を受けるコントロール（IME・プラグイン固有GUIを含む）。
- **Active View:** Window内の操作対象View。
- **Editing Selection:** 各Viewが持つ編集用選択（複数可。種類とprimary itemを保持）。
- **Inspection Focus:** 関係を調査している対象。Editing Selectionとは別に保持する。

**フォーカス移動 ≠ 選択変更 ≠ 調査対象変更 ≠ 編集対象変更**を原則とする。

### 5.2 Link Group

- 各Viewは0個または1つの連携Groupへ所属することを初期モデルとし、Groupは`topics`（例: track selection / navigation）を指定する。
- 連携イベントは`source_view_id`、`event_id`、`topic`、`semantic_target`を持つ。中央のManagerがGroupに一度だけ適用・配信し、受信側は再発信しない。
- 選択内容の変換（例: ArrangementのBass Clip → MixerのBass Track）は**明示したSelection Mapping**を使う。変換できない対象は強引に同期しない。
- Zoom/Scrollは原則独立。Inspection Focusの追従・固定はViewごとに扱い、ユーザーが固定した対象を別ViewのSelectionで上書きしない。
- Link Groupは**編集コマンドのbroadcast手段ではない**。Project Stateの変更はGroup所属と無関係に全Viewへ反映される。

### 5.3 Inspectorの追従

`follow` は所属する選択/リンク規則に従って調査対象を更新する。`pinned` は明示した対象を保持する。別ノードの調査は編集選択を自動更新しない。Inspector内で明示的にパラメータ操作を行う場合、対象IDはその操作から直接指定する。

## 6. Command Routing契約（概念API）

```text
CommandContext {
  command_id
  project_session_id
  origin_window_id
  origin_view_id
  input_context         # focused control / IME / plugin / modal / view
  command_kind          # application / project / transport / view / editing
  explicit_target_ids?  # context menu, parameter control等
  selection_snapshot?   # keybinding解決時の有効な選択
  expected_revision?    # 競合が問題になる操作のみ
}
```

対象は**発行時に確定**する。非同期処理の開始後に「現在の選択」を改めて読んで対象を変更しない。Project Controllerが実行直前に対象の存在・権限・revision等を再検証する。

### キー入力の優先順位

1. OS/プラグイン固有の入力境界、モーダル、IME変換、文字入力欄を優先する。
2. 現在のActive Viewで適用可能なView/Editing Commandを解決する。
3. 入力と衝突しない場合に限りTransport/Project/Application Commandを解決する。
4. 解決できない破壊的コマンドは**何もしない**。別Windowの古いSelectionへフォールバックしない。

| 状況 | 結果 |
| --- | --- |
| ArrangementでClipを選択しDelete | そのViewの選択Clipを削除 |
| MIDI EditorでNoteを選択しDelete | そのViewの選択Noteを削除 |
| Inspector検索欄でDelete/Space | 文字入力が優先。DAW編集や再生は実行しない |
| InspectorでParameterを直接変更 | 明示されたParameter IDを変更。別Viewの選択は無関係 |
| Signal Flowで調査だけしてDelete | 調査対象の削除には使わない |
| 通常ViewでCtrl/Cmd+Z | ProjectのUndo（実装時に所有scopeを確定） |
| テキスト入力中のCtrl/Cmd+Z | テキスト編集のUndo |
| プラグインGUIでSpace/Delete | プラグインの入力をホストが無条件に奪わない |
| 右クリックから対象編集 | Context Menuの明示的Targetを使う |

既存RuviformのPlugin GUIとWindowフォーカス契約を尊重し、プラグイン内部へのキー処理を推定して奪わない。ネイティブWindowとWebView間でのショートカット配送はOS別に検証する。

## 7. ライフサイクル・失敗時の扱い

### 7.1 Detach / Reattach

1. 移動要求時に`view_id`、現在の所属、保持すべき`ViewState`を確定する。
2. 移動先Windowが必要なら生成し、WebViewのReady/表示準備を待つ。
3. 旧表示からの新規編集コマンドを抑止するため、Viewの操作所有権に移行状態/世代を設ける。
4. Layoutの所有先を**一回の論理コミット**で切り替え、新しい表示でViewを再構築し、状態を復元する。
5. 準備・移動・復元失敗時は元の所属へロールバックし、View IDと制作状態を失わない。

ドラッグUIでのポインター位置やDOMの内部状態は移動しない。表示側の復元には安定IDとモデル投影を使う。

### 7.2 Close

- サブWindowの`CloseRequested`は**Project終了やJob一括取消を意味しない**。SynvelisのWorkspace ManagerがViewの再配置・破棄・確認を判断し、WindowのDestroy可否を返す。
- 最後のWindow/アプリ終了では、未保存Projectの確認、Audio/Job停止等を含む**別のApplication Shutdown**を行う。
- モーダル表示中やドラッグ中のClose、ネイティブクラッシュ、WebView再初期化失敗では、残ったWindowとProjectを維持し、可能な限りレイアウトを復旧する。
- 原則としてサブWindowが閉じても、当該Window内のViewをユーザーへ知らせずに喪失させない。具体的な閉鎖時UIポリシーは別途UX検証する。

### 7.3 Persistence / Monitor再構成

Workspace LayoutとView/Link設定は、初期段階ではユーザー環境設定としてProject本体と分離して保存する設計を推奨する。将来Project別レイアウト保存は可能にしても、音楽データの保存/Undoと混同しない。

復元時はバージョン・参照整合性・Window bounds・モニターの有無を検証する。利用できないモニターのWindowは表示可能な領域へ戻し、復元不能なViewは安全にデフォルト配置へ回す。

## 8. リポジトリ境界と基盤APIの要求

| 機能/契約 | Ruviform（#181・#182） | Synvelis |
| --- | --- | --- |
| 複数Window・WebViewのホスト | `WindowId`、Window作成/閉鎖、Focus/DPI/Monitorのイベントとライフサイクル | Window生成方針、主要Window、Workspaceへの紐付け |
| Window-aware IPC | 発信元/送信先の区別、応答の相関、失われたWindowへの安全な配送 | DAW用のIntent/Command/State subscriptions |
| 汎用Dock Tree | Split/Tabs、所属一意性、レイアウト変更/検証、スナップショット | View Factory、Dock可能な対象と初期プリセット |
| Window間View移動 | Hostの準備・表示・破棄との調停、汎用的な移動/失敗契約 | View Stateの保存/復元、編集所有権の移行 |
| Project/Engine | 既存のRT/GUI分離契約（DAW固有型を知らない） | Project Model、Audio Graph反映、Transport/Undo |
| Selection/Focus | OS Window/WebViewの入力元とFocus境界 | Active View、Editing Selection、Inspection Focus |
| Link Group | 汎用イベント配送上の必要な機構に限る | Semantic Selection Mapping、追従/固定の規則 |
| Command Router | UI発信元を区別できる汎用Intent境界 | DAWコマンドの優先順位・ターゲット決定・妥当性検証 |
| レイアウト永続化 | 汎用Dockスナップショット・検証 | 設定の保存先、View State、プロジェクト別運用方針 |

### 現行Ruviformとの差分（2026-11-01確認）

- `ruviform-ui-webview::host::run` は現状単一Window・単一WebViewを生成する。
- `ruviform-ui::HostEvent` は`Focused(bool)`、`CloseRequested`等を提供するが、Window発信元を個別に保持しない。
- `HostEvent::CloseRequested` を受けると現行Hostは終了し、`UiRuntime` もJobを取消する。複数WindowではWindow Closeとアプリ終了を分離する必要がある。
- `rvf.ipc.v1` とWebViewの`__rvf.call`は現行WebView内で要求IDを採番する。複数WebViewの識別・配送を拡張する必要がある。
- Issue #156のGUI基盤設計は複数WindowをDeferredとしていたため、[Ruviform #181](https://github.com/scarecrow173/ruviform/issues/181)と[#182](https://github.com/scarecrow173/ruviform/issues/182)でその範囲を明示的に拡張する。
- これらは**不足が確認された契約**であり、提案APIの名称・配置・互換性対応は各Issueで決定する。

## 9. 検証計画

| レイヤー | テスト/PoC | 合格条件 |
| --- | --- | --- |
| Ruviform headless | Window ID、Close承認/取消、IPC相関、Dock Tree演算、不正復元 | 発信元分離・整合性・ロールバックが成立 |
| Ruviform実機 | 2 Window以上、各WebView、複数モニター、Focus/DPI/Close | 別Windowを閉じても残りが操作でき、既存GUIデモが機能 |
| Synvelis状態管理 | Viewを2つ開き別Selection/Inspectionを保持 | Projectは1つの正本、各Viewの表示状態は独立 |
| Synvelis Link Group | Clip→Trackの意味的選択同期、固定Inspector | 変換対象以外を誤同期せず、通知ループが起きない |
| Synvelis Command | Delete/Undo/Space、IME/文字入力/プラグインGUI | 誤ったWindow/View/Targetで編集・再生しない |
| 移動・復元 | Detach → Reattach、Window作成失敗、モニター切断 | View ID/必要なStateを維持し、失敗時もViewを失わない |
| RT境界 | 再生中のWindow移動・Dock変更 | UI操作だけでRT側を停止/ブロック/不要に再構築しない |

**注意:** 以上は試験計画であり、現在のSynvelisに動作するDAW実装や検証済みのマルチウィンドウGUIがあることを意味しない。Windows/macOS/Linuxの実機検証状況は個別に記録する。

## 10. 作業分解と依存関係

1. **Ruviform #181:** Multi-Window Hostの発信元・IPC・Focus/Closeライフサイクル契約とPoC。
2. **Ruviform #182:** 汎用Dock Tree、Detach/Reattach、Snapshotと安全な復元（#181に依存）。
3. **Synvelis（統合Issue）:** Project/Workspace/View State、Link Group、Selection/Focus/Command Routing、アプリ固有View Factory、Ruviform連携。
4. **Synvelis UX検証:** 通常制作、Bass→Bus→Compressor調査、MIDI Editor、複数Mixer、複数モニター、プラグインGUIの操作シナリオで検証。

設計書を正本とする対象は**Synvelis側の統合設計**。Ruviformの公開API・クレート境界・Windows/macOS/Linux実機契約の正本は、Ruviform側の設計書/ADR/実装で別途確定する。

## 11. 未決定・PoCで確定する項目

- Docking UIの実装技術とWebView間ドラッグ制約、代替操作。
- Window別WebViewプロセス/Context・DPI混在・モニター識別と復元形式。
- View閉鎖時の具体的なUX、View複製/破棄ポリシー。
- Link Groupの複数Group所属やタイムライン連動の詳細規則。
- プラグインGUIのネイティブ親子関係・キーボード入力のOS/規格差。
- 実装上の具体的な型名、永続化フォーマット、互換性と移行方法。

上記は確定事項を曖昧にするためではなく、**方針と実装方式を分離し、検証できる境界で決定するため**に残す。
