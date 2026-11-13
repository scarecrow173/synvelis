# POLICIES.md — synvelis

Status: Accepted  
Effective: 2026-11-13  
Scope: scarecrow173/synvelis / 三リポジトリ共通の検証方針

## 1. 方針の適用と責務

- 本文書はこのリポジトリの作業・検証・受入についての規範的なポリシーである。エージェントおよび人は、Issue起票/改訂、実装計画、実装、レビュー、受入判定でルートの `AGENTS.md` と本ファイルを参照する。
- 同名の `POLICIES.md` を Ruviform / AudioLens / Synvelis の**各リポジトリのルート**に置く。作業対象のローカル版を参照する。共通条項を変える際は3リポジトリへの影響を確認し、適切に同期する。
- 古いIssue/仕様・検証計画と矛盾する場合は、この現行方針と、明示的な最新の設計判断を照合し、差分を報告・是正する。古い記述を暗黙に優先したり、機能の未実測を合格扱いしたりしない。
- 共通・再利用可能な契約/実装はRuviformの責務、解析製品の意味論はAudioLens、DAW製品の意味論はSynvelisの責務。共通基盤変更は両Consumerの互換性・精度・性能・RTを独立評価する。

## 2. OS別検証レベル（2026-11-13決定）

本書では検証レベルを **OS-L1 / GUI-L2 / EXT-L3** と記述する。Plugin Hostの実行形態で用いる **L0/L1** とは別概念である。

| 検証レベル | 内容 |
| --- | --- |
| **OS-L1** | 対象OSでのネイティブBuild・Link、実行可能な単体/統合/Headless/RT/CLIテスト、利用するCargo Feature、Consumer固有の精度・整合性Oracle |
| **GUI-L2** | 実OSのWindow・ユーザー入力・Focus・表示/画素・必要なDPI/IME等、提供するGUI機能の実画面検証 |
| **EXT-L3** | 実際に使用する物理Audio/MIDI機器、実VST3/CLAP/AU等のPlugin/Editor、Native Dock、外部実環境と結合する機能の実測 |

| OS | OS-L1 | GUI-L2 | EXT-L3 |
| --- | --- | --- | --- |
| **Windows** | **必須** | **実際に提供・利用するGUI機能に限り必須** | **実際に提供・利用する機器・Plugin等に限り必須** |
| **macOS** | **必須** | 利用要件発生までDeferred | 利用要件発生までDeferred |
| **Linux** | **必須** | 利用要件発生までDeferred | 利用要件発生までDeferred |

- Windowsでも、**使わないDevice・Plugin形式・Host mode・特殊FS等の全組合せは必須にしない**。利用する能力だけをCase/Device/Format/環境ごとに指定する。
- Windowsで利用する機能のGUI-L2/EXT-L3がNotRun/Failなら**その機能は未受入**。Fake/Headless/Offscreen成功は実GUI、Generic/Floating Editorは実Native Dock、Mock Pluginは実Plugin動作の代用にならない。Native Dockの画素合成など、実利用機能の未解決不具合をPassにしない。
- macOS/LinuxはOS-L1までが現行必須。実GUI/物理機器/実PluginをそのOSで利用する必要が生じた時点で、対象OS・機能のGUI-L2/EXT-L3をRequiredへ昇格し、Owner、Issue、環境、受入Caseを定義する。
- 本方針は3OSを**設計対象から除外する判断ではない**。OS-L1がPassしても、未実測のCPUアーキテクチャ、OSバージョン、配布パッケージ、実デスクトップ環境を正式対応済みと主張しない。

## 3. 要求状態と検証結果の分離

- **要求状態**：`Required`（現行必須）、`Deferred`（利用条件成立まで保留）、`NotApplicable`（対象機能では不要）。
- **実行結果**：`Pass`、`Fail`、`NotRun`、`Unsupported`（根拠を伴う技術的非対応）。要求状態と実行結果は別軸で管理する。
- 実機がない、CI jobが未実行、テストがSkip/ignored、Featureが有効でない、といったケースは`NotRun`であり`Pass`でも`Unsupported`でもない。
- `Deferred`かつ`NotRun`であれば現在の必須受入を妨げない。一方、`Required`の`NotRun`/`Fail`は当該機能の受入未達である。環境不在を技術的非対応と誤認しない。
- 証拠には対象Repo/SHA（Consumerの場合Ruviformのresolved SHA）、OS/Arch、Feature/テスト名、期待結果・観測結果、実行環境、CI Run URL/実機ログ等を記録する。**Jobが定義されていることやgreenであることだけでは個別CaseのPassを示さない**。

## 4. 着手・CI・Issue・PR運用

- Headless/RT/Domainの実装Sliceの完了と、製品でその機能を**提供するための最終受入**を分離する。Windowsで必要な実GUI/実Plugin受入を、無関係なHeadless実装の着手Hard Blockerに一律設定しない。
- 原則、既存の**変更検知付きDaily Full CIと手動Full CI**を利用する。OS-L1に不足があれば既存Job・Feature・テスト・Run証拠を点検して最小限で補う。独立の重複WorkflowやPR必須Full CIを無条件に追加しない。
- 非デフォルトのCargo Featureについて、通常の`cargo test --workspace`だけで全組合せが検証されたとみなさない。必要Feature・OS・実走テストを明示する。
- Issueでは`### 受入条件`と`### 検証方法`にOS別のRequired/Deferredと証拠を記す。着手必須依存は`### 着手条件（Blocked by）`、統合・受入だけの依存は`### 統合・受入で関係する Issue（着手は止めない）`に分離する。
- 方針変更や新しいOS/Device/Plugin機能の利用決定でGateを変更する場合は、影響する3Repo、受入Case、Owner、Issue、既存CIとの関係を確認し、必要なファイルとIssueを更新する。片Consumerの成功を他ConsumerのPassへ転用しない。

## 5. ローカル検証と Daily CI の責務分離（2026-11-13決定）

- **通常のローカル検証は変更範囲に比例させる。** 実装・不具合修正では、変更したクレート/パッケージと直接影響する依存先・下流の型チェック、対応する単体/回帰テスト、軽量な整形/静的チェックを実施する。失敗した対象テストを未修正のまま「CIに委譲」して完了扱いにしない。
- **文書・Issue/ポリシーのみの変更**は、対応するMarkdown/構成ファイル/リンク等の軽量な衛生チェックを原則とし、Rust全Workspace Build・Testを要求しない。CI/workflowやビルド設定を変更した場合は、対応する設定検査（例: actionlint・workflow契約検査・TOML検査）を実施する。
- 日常作業に**全Workspaceの `check`/`lint`/`test`/`build`、全Feature Matrix、MSRV別再コンパイル、3OS横断、全Golden・長時間回帰・Cross-Consumer全検証を一律要求しない**。これらは既存の変更検知付きDaily Full CI（mainの対象SHA）と手動Full CIに委譲する。既存の全体検証用miseタスクは削除せず、任意の再現・切り分け入口として維持する。
- ローカルで対象クレートだけを検証する場合はリポジトリの`mise`タスクを用いる（例: `mise run check-local <package>`、`mise run test-local <package>`）。これらはCargoの`-p`による対象限定であり、依存クレートのコンパイルや必要なFeature指定を省略してよいという意味ではない。CI専用Rust pin/profileを日常ローカルで強制しない。
- 公開API/永続化形式/共通契約・依存グラフ・RT/精度・セキュリティ境界を変更する場合は、変更に直接影響する下流・Consumer・必要Featureをローカルでも検証し、広範囲の非回帰・3OS判定は対象SHAのCI証拠で確認する。高リスクな変更・リリース前・必要な原因切り分けでは手動Full CIを使用できる。
- **ローカル検証の成功は統合受入の成功ではない。** Daily CI未実行・進行中・未対象SHA・必要Case未実施を`Pass`にしない。PRがmainにマージされても、当該main SHAの必須CI結果が揃うまでは統合検証は`NotRun`（実行中ならPending）であり、最終受入は保留する。失敗時は既存のCI Tracker/Issueで追跡し、修正後に対象SHAを再検証する。
- Daily Full CIは既存の`ci-summary`等のfail-closed集約と対象SHA証拠を維持し、OS-L1のNative Build/Link・実走テスト・必要Feature/製品Oracleが不足する場合は既存レーンを補う。**CIに実装されていない検証を「委譲済み」と扱わない。** 新しい重複WorkflowやPR必須Full CIを無条件に増設しない。
- Windowsで提供・利用するGUI-L2/EXT-L3の実機受入は、Daily OS-L1 CIへの委譲では代替できない。macOS/LinuxのGUI-L2/EXT-L3は従来のDeferred方針を維持する。全体テストのローカル省略は、受入要件の削除や性能・精度・Consumer互換性の緩和ではない。

## 6. Synvelis固有の適用

- OS-L1ではProject/Track/Clip/Undo/Save、Playback/Export、録音Mock/Fault、RT NoAlloc/NoBlock、Plugin/GUI Driver-neutral契約の独立Oracleを保護する。
- Windowsで製品として実際に提供するDAW実GUI、録音/モニター/実MIDI、VST3/CLAP等のNative Plugin DockをGUI-L2/EXT-L3で検証する。実Native Dockに画素・Focus・入力の未解決不具合がある場合、当該提供機能を未受入として追跡する。
- 利用しないPlugin形式・機材は必須外とする。DAW固有のProject/編集操作/UX/Recording Workflowと製品受入はSynvelisのIssueで管理する。一方、ドメイン非依存で再利用可能なTime/Graph/Device/Plugin/GUI Port・実行契約・テスト基盤等の課題はRuviformのIssueとして起票し、共通基盤へ還元する。Ruviformの変更は両Consumerの精度・RT・互換性を独立に検証し、必要な導入・追従対応は各ConsumerのIssueで管理する。
