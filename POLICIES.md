# POLICIES.md — synvelis

Status: Accepted  
Effective: 2026-11-13（OS別検証・CI）、2026-11-15（仮想優先受入）  
Scope: scarecrow173/synvelis / 三リポジトリ共通の検証方針

## 1. 方針の適用と責務

- 本文書はこのリポジトリの作業・検証・受入についての規範的なポリシーである。エージェントおよび人は、Issue起票/改訂、実装計画、実装、レビュー、受入判定でルートの `AGENTS.md` と本ファイルを参照する。
- 同名の `POLICIES.md` を Ruviform / AudioLens / Synvelis の**各リポジトリのルート**に置く。作業対象のローカル版を参照する。共通条項を変える際は3リポジトリへの影響を確認し、適切に同期する。
- 古いIssue/仕様・検証計画と矛盾する場合は、この現行方針と明示的な最新の設計判断を照合し、差分を報告・是正する。古い記述を暗黙に優先したり、機能の未実測を合格扱いしたりしない。
- 共通・再利用可能な契約/実装はRuviformの責務、解析製品の意味論はAudioLens、DAW製品の意味論はSynvelisの責務。共通基盤変更は両Consumerの互換性・精度・性能・RTを独立評価する。

## 2. OS別検証レベル（2026-11-13決定・2026-11-15仮想優先改訂）

本書では検証レベルを **OS-L1 / GUI-L2 / EXT-L3** と記述する。Plugin Hostの実行形態で用いる **L0/L1** とは別概念である。

| 検証レベル | 内容 |
| --- | --- |
| **OS-L1** | 対象OSでのネイティブBuild・Link、実行可能な単体/統合/Headless/RT/CLIテスト、利用するCargo Feature、Consumer固有の精度・整合性Oracle |
| **GUI-L2** | 実OSのWindow・ユーザー入力・Focus・表示/画素・必要なDPI/IME等、提供するGUI機能の実画面検証 |
| **EXT-L3** | 提供する外部接続能力について、本番OS Audio/MIDI API・実Plugin ABI/Editor・Native Dock・外部Provider Protocol等を通す結合検証。初回から仮想OS Endpoint・正規ABIのTest Pluginを標準利用し、物理機材・市販Pluginは通常必須としない |

| OS | OS-L1 | GUI-L2 | EXT-L3 |
| --- | --- | --- | --- |
| **Windows** | **必須** | **実際に提供・利用するGUI機能に限り必須** | **実際に提供・利用するOS API・Plugin形式・Provider等に限り必須（仮想優先）** |
| **macOS** | **必須** | 利用要件発生までDeferred | 利用要件発生までDeferred |
| **Linux** | **必須** | 利用要件発生までDeferred | 利用要件発生までDeferred |

- Windowsでも、**使わないDevice・Plugin形式・Host mode・特殊FS等の全組合せは必須にしない**。利用する能力だけをCase/Device/Format/環境ごとに指定する。
- Windowsで利用する機能のGUI-L2/EXT-L3がNotRun/Failなら**その機能は未受入**。Fake/Headless/Offscreen成功は実GUIの代用にならず、Generic/Floating Editorは実Native Dockの代用にならない。Mock Pluginは実Plugin ABI動作の代用にならないが、**正規ABIを実装した実バイナリのTest Plugin**はその対象形式のHost/Process/Editor結合検証に利用できる。Native Dockの画素合成など、実利用機能の未解決不具合をPassにしない。
- macOS/LinuxはOS-L1までが現行必須。実GUI/本番OS Device API/実Plugin ABI等をそのOSで利用する必要が生じた時点で、対象OS・機能のGUI-L2/EXT-L3をRequiredへ昇格し、Owner、Issue、環境、受入Caseを定義する。物理機材・市販Pluginの通常必須化は行わない。
- 本方針は3OSを**設計対象から除外する判断ではない**。OS-L1がPassしても、未実測のCPUアーキテクチャ、OSバージョン、配布パッケージ、実デスクトップ環境を正式対応済みと主張しない。

### 2.1 仮想優先の受入と共通テスト基盤（2026-11-15決定）

- **初回から仮想優先:** 物理Audio/MIDI機材や市販VST3/CLAP Pluginの入手・接続を初回受入や通常回帰の必須条件にしない。決定的Test Backendで基礎精度を検証し、Windowsで実際に提供する外部機能は本番OS API/実Plugin ABIを通す仮想環境でGUI-L2/EXT-L3の必要Caseを検証する。テスト環境が未構築ならRequired Caseは`NotRun`であり、開発実装Sliceの完了と製品受入を分離する。
- **OS-L1とOS API L3を分離:** Ruviform製Test Audio/MIDI Backend、Virtual Clock、生成PCM/MIDI、Fault InjectionはOS-L1の契約・精度試験である。WindowsのWASAPI/WinMM等を通すL3では、OSが実際に列挙・Openできる仮想Endpointから**本番Backend**へ入力し、Format/Callback/Timestamp/Recoveryを独立Oracleで判定する。後段へ直接注入したMockを実OS APIのPassとしない。
- **実Plugin ABIとMockを分離:** 対象VST3/CLAP等の正規ABIを備えた実バイナリTest Pluginを、本番Hostの探索/Load/Process/State/Unload経路で実行する。独立Validator/Reference Hostや仕様由来の期待値でABIと精度を検証し、HostとTest Pluginが同じ内部Adapterを使う自己検証だけに依存しない。Fake PluginはOS-L1契約試験に限る。
- **実GUIとNative Dockを分離:** Windows GUI-L2では実OS Window/入力/Focus/画素/Receiptと製品状態を検証する。Native Plugin Dockの受入では、実ABI Test PluginのNative Editor（実Child HWND等）と本番Window合成を通し、画素/Resize/Focus/Crash/RTを検証する。Fake/Offscreen/Generic/FloatingをNative DockのPassに読み替えず、既知の不具合を残す。
- **物理適合性の限界:** 仮想EndpointやTest PluginのPassは、特定の物理機材のDriver、ADC/DACアナログ遅延、物理Direct Monitoring、特定市販Pluginの互換性を保証しない。これらは原則として必須Gateから外し、個別機材の正式サポートを宣言する場合や報告された不具合の原因調査で必要になったときに対象Caseを定義する。問題は可能な範囲で共通仮想Fixtureへ回帰再現を還元し、再現不可なら未保証範囲を記録する。
- **共通化とDogfooding:** 共通Test Runtime、仮想Endpoint制御、Test Plugin、実GUI Runner/Console自体もRuviformの再利用可能なAudio/MIDI/Clock/GUI/Plugin/Test契約を利用して構築し、汎用的な改善をRuviformへ還元する。OS固有Driver/Kernel/Plugin SDK/GPUI具象型はAdapter内部に閉じる。Test-onlyバイナリは製品配布に含めない。AudioLensのf64/長尺/CLIとSynvelisのPCM/DAW/RT/Project/Plugin/GUIは各Consumerの**独立Oracle**で回帰検証し、一方のPassを他方へ転用しない。
- **外部Provider:** 実Git/LFS/WebDAV等を提供する場合は本番Protocol・認証・永続化境界を通す制御可能なTest Serverを使用できる。単なるMockでは実Network/ProtocolのL3を満たさない。外部事業者固有の互換性は別途サポート条件を定義する。

## 3. 要求状態と検証結果の分離

- **要求状態**：`Required`（現行必須）、`Deferred`（利用条件成立まで保留）、`NotApplicable`（対象機能では不要）。
- **実行結果**：`Pass`、`Fail`、`NotRun`、`Unsupported`（根拠を伴う技術的非対応）。要求状態と実行結果は別軸で管理する。
- Requiredの実OS Window/仮想Endpoint/実ABI Test Plugin環境がない、CI jobが未実行、テストがSkip/ignored、Featureが有効でない、といったケースは`NotRun`であり`Pass`でも`Unsupported`でもない。物理機材がなくても、仮想方式で受入条件を満たしたCaseを機材不在のみで未達にしない。
- `Deferred`かつ`NotRun`であれば現在の必須受入を妨げない。一方、`Required`の`NotRun`/`Fail`は当該機能の受入未達である。環境不在を技術的非対応と誤認しない。
- 証拠には対象Repo/SHA（Consumerの場合Ruviformのresolved SHA）、OS/Arch/OS版、Feature/Case ID/テスト名、要求状態と実行結果、環境種別（deterministic backend / OS仮想Endpoint / 実ABI Test Plugin / 実Window / 物理機材等）、Driver・Endpoint・Plugin・FixtureのVersion/Hash、独立Oracle、期待結果・観測結果、CI Run URL/実OSログ等を記録する。**Jobが定義されていることやgreenであることだけでは個別CaseのPassを示さない**。

## 4. 着手・CI・Issue・PR運用

- Headless/RT/Domainの実装Sliceの完了と、製品でその機能を**提供するための最終受入**を分離する。Windowsで必要な実GUI/実Plugin受入を、無関係なHeadless実装の着手Hard Blockerに一律設定しない。
- 原則、既存の**変更検知付きDaily Full CIと手動Full CI**を利用する。OS-L1に不足があれば既存Job・Feature・テスト・Run証拠を点検して最小限で補う。独立の重複WorkflowやPR必須Full CIを無条件に追加しない。
- 非デフォルトのCargo Featureについて、通常の`cargo test --workspace`だけで全組合せが検証されたとみなさない。必要Feature・OS・実走テストを明示する。
- Issueでは`### 受入条件`と`### 検証方法`にOS別のRequired/Deferredと証拠を記す。着手必須依存は`### 着手条件（Blocked by）`、統合・受入だけの依存は`### 統合・受入で関係する Issue（着手は止めない）`に分離する。
- 方針変更や新しいOS/Device/Plugin機能の利用決定でGateを変更する場合は、影響する3Repo、受入Case、Owner、Issue、既存CIとの関係を確認し、必要なファイルとIssueを更新する。片Consumerの成功を他ConsumerのPassへ転用しない。

## 5. ローカル検証と Daily CI の責務分離（2026-11-13決定）

- **通常のローカル検証は変更範囲に比例させる。** 実装・不具合修正では、変更したクレート/パッケージと直接影響する依存先・下流の型チェック、対応する単体/回帰テスト、軽量な整形/静的チェックを実施する。失敗した対象テストを未修正のまま「CIに委譲」して完了扱いにしない。
- **文書・Issue/ポリシーのみの変更**は、対応するMarkdown/構成ファイル/リンク等の軽量な衛生チェックを原則とし、Rust全Workspace Build・Testを要求しない。CI/workflowやビルド設定を変更した場合は、対応する設定検査（例: actionlint・workflow契約検査・TOML検査）を実施する。
- 日常作業に**全Workspaceの `check-full`/`lint-full`/`test-full`/`build-full`、全Feature Matrix、MSRV別再コンパイル、3OS横断、全Golden・長時間回帰・Cross-Consumer全検証を一律要求しない**。これらは既存の変更検知付きDaily Full CI（mainの対象SHA）と手動Full CIに委譲する。`*-full` はローカルで任意に全体検証・原因切り分けを行う入口としても維持する。
- 通常のmiseタスク`check`/`lint`/`test`/`build`は対象Cargo packageを必須引数として受け取り、未指定なら失敗する（例: `mise run check <package>`、`mise run test <package>`）。対象限定はCargoの`-p`で実現し、依存クレートのコンパイルや必要なFeature指定を省略してよいという意味ではない。CI専用Rust pin/profileを日常ローカルで強制しない。
- **`*-full` は検証範囲の名前**であり、実行環境の名前ではない。`check-full`/`lint-full`/`test-full`/`build-full`は現在のOSにおけるWorkspaceの標準Feature範囲を検証し、3OS全体や非既定Featureを包含しない。`verify-full`はリポジトリごとに定める標準の全体検証を束ねる。3リポジトリ共通で`MISE_ENV=ci`をCIの実行モードとし、`mise.ci.toml`の固定Rustを使用する。`Cargo.toml`の`[profile.ci]`は`dev`を継承し、`debug = "line-tables-only"`・`incremental = false`、外部依存の`[profile.ci.package."*"]`は`debug = "none"`とする。CIの`*-full`は`--profile ci`かつ`CARGO_INCREMENTAL=0`で実行し、通常のローカル`*-full`は環境のRustと標準dev/test Profileを使う。CI Profileの指定は検証範囲（全OS/全Feature等）の拡大を意味しない。
- 公開API/永続化形式/共通契約・依存グラフ・RT/精度・セキュリティ境界を変更する場合は、変更に直接影響する下流・Consumer・必要Featureをローカルでも検証し、広範囲の非回帰・3OS判定は対象SHAのCI証拠で確認する。高リスクな変更・リリース前・必要な原因切り分けでは手動Full CIを使用できる。
- **ローカル検証の成功は統合受入の成功ではない。** Daily CI未実行・進行中・未対象SHA・必要Case未実施を`Pass`にしない。PRがmainにマージされても、当該main SHAの必須CI結果が揃うまでは統合検証は`NotRun`（実行中ならPending）であり、最終受入は保留する。失敗時は既存のCI Tracker/Issueで追跡し、修正後に対象SHAを再検証する。
- Daily Full CIは既存の`ci-summary`等のfail-closed集約と対象SHA証拠を維持し、OS-L1のNative Build/Link・実走テスト・必要Feature/製品Oracleが不足する場合は既存レーンを補う。**CIに実装されていない検証を「委譲済み」と扱わない。** 新しい重複WorkflowやPR必須Full CIを無条件に増設しない。
- Windowsで提供・利用するGUI-L2/EXT-L3の実OS Window・本番OS API・実Plugin ABI受入は、Daily OS-L1 CIのFake/Headless成功で代替できない。Windows専用の仮想Endpoint/実Window/実ABI Test Pluginの手動・opt-in・定期実行レーンを使用できるが、**RequiredのCaseがNotRun/Failなら未受入**とする。macOS/LinuxのGUI-L2/EXT-L3はDeferred方針を維持し、ローカル全体テスト省略によって精度・性能・Consumer互換性を緩めない。

## 6. 形式的検証原則（Property-driven Formal Verification with Explicit Verification Boundaries）（2026-11-25決定）

### 6.1 共通原則

- Property First：製品が保証すべき性質を起点とする。
- Risk Driven：破壊時のユーザー影響、障害の重大性、検出困難性に基づく優先順位。
- Assurance Driven：必要な保証強度を決めてから検証方法を選ぶ。
- Tool Fit：問いに適した手法を選び、特定の手法・順序を強制しない。
- Minimal Sufficient Boundary：必要十分な検証境界を設定する。
- True to the Code：実装と検証対象の対応を維持する。
- Able to Fail：意図的な契約違反を検出できることを確認する。
- Cheap to Rerun：関連変更ごとに再検証可能なコストを設計する。
- Honest Assurance：保証範囲、仮定、未証明領域、結果を正確に扱う。
- Continuous Synchronization：製品変更に検証と証跡を追従させる。

### 6.2 検証粒度・境界

Propertyを単位に必要なproof・モデル・テストを配置する。関数、ファイル、crate、公開API単位に機械的に割り当てない。Primitive/Function/Component/Protocol/Integration/Systemは設計上の参考であり固定階層ではない。小ささではなく必要な保証に対する適切さ、原因特定可能性、計算コストで評価する。

各Propertyについてリスク、必要保証、対象実装・モデル、入力領域・状態空間、含まれる処理と依存、外部契約・仮定、Bound、Stub/Mock/抽象化と妥当性、除外、残留リスク・補完検証、予算・実行契機・再実行方法、証跡を記録する。Harness分割（実行単位）、Input Domain分割（入力領域）、Boundary分割（実装・依存範囲）を区別する。入力分割は元領域を覆うかを評価し、境界変更は除外した契約と保証の差を記録する。

### 6.3 手法選定・契約分解

固定的な proptest → Kani → TLA+ → Lean の昇格路を廃止する。生成入力・Round-tripはproptest、Rust有界網羅・算術・境界はKani、状態機械・順序・並行性はTLA+、数学的定理はLean、並行実行はloom、異常入力探索はFuzzing、接続・実装の振る舞いは通常テスト等を選択・併用する。Property Test/Fuzzingと形式的証明、モデル証明と実装証明を同一視しない。

上位Propertyを独立した下位契約へ分解する。上位の形式的保証が必要なら、下位契約、呼出側の事前条件充足、事後条件から上位契約を導く接続条件を検証する。Function Contract、Verified Stubbing、Assume-Guarantee Reasoning等は必要に応じて採用。局所proofと統合テストだけで上位契約を証明済みとしない。

### 6.4 健全性・検証可能性

製品契約・保証範囲を暗黙に弱めることを禁止する。無関係処理の分離、固定fixture、必要なシンボリック軸を優先する。Stub/Mockは仮定、根拠、保証への影響を記録し、検証済み契約を優先する。cfg(kani)等による製品実装置換は原則回避。assumeは正当な事前条件・明示的な限定領域に限り、充足可能性と到達可能性を評価する。Bound縮小は保証差を記録。Unwinding Assertions無効化は原則禁止、例外は独立した妥当性根拠とレビューを要し、健全性が示せない結果を通常のProof Passに含めない。proptest移管は保証種別変更として記録する。反例を消す目的でProperty・モデルを弱めない。製品契約変更は別途承認を要する。

Verifiabilityを設計品質として扱い、必要に応じFunctional Core / Imperative Shell、明示的状態・不変条件、型制約、副作用分離、決定的実行、契約指向API、独立Oracleを採用する。ツール都合の不自然な分割は避け、精度・RT・公開契約・Consumer互換性を維持する。

### 6.5 コスト・証跡・同期

実行時間、ピークメモリ、関連検証群の合計コスト、選択実行、再現性、保守コスト、保証価値を評価する。固定の一律上限ではなく重要性と実測に基づき予算を定義する。超過時はCompile/Symex/Solver/Memory等を分類し、粒度・境界・手法を再評価してからRunner・時間予算を検討する。本質的に高コストで必要な検証は個別予算・実行契機を設ける。

要求状態（Required/Deferred/N/A等）、verdict（Pass/Fail/NotRun/Unsupported等）、保証種別（Example Tested/Property Tested/Bounded Exhaustive/Model Checked/Theorem Proved等）、対象（実装/モデル/抽象化）、同期状態、失敗原因（Product Defect/Model Error/Resource Limit/Environment等）を独立管理する。既存語彙・機械可読形式の互換性を維持し不足情報を補う。Timeout/OOM/環境不足/exit 0/CI greenをProof Passとしない。対象SHA、ツール版、実行条件、保証範囲、Mutation証跡を記録し、対応確認・再検証未完了時に検証済みSync Commitを更新しない。

### 6.6 開発工程・管理場所・責務

要求でProperty・リスク、仕様で必要保証、実装計画前の設計で粒度・境界・仮定・手法・予算を決定する。必要に応じ設計モデルを事前検証し、製品実装のproofは実装後に実走する。変更時の影響判定、再検証、検出能力確認、証跡同期を継続する。全開発で形式的証明を強制しない。

POLICIES.mdを必須原則の正本とし、AGENTS.mdは導線、既存verifyスキルは対象固有の検証・保守手順、docs/specs/formal-verification.mdはProperty・検証仕様、src/tests/モデルは検証実装、miseは正規実行入口、evidenceは証跡とする。規則を重複させない。専用DSL・固定階層・独自フレームワークは当面不要。

RFは共通契約・実装と再利用可能な検証支援、AL/SVは製品固有のProperty、共通契約の事前条件充足、独立Oracleによる統合検証を担当。実証された汎用実装のみRFへ還元し、片Consumerの保証を他方へ転用しない。

既存L0〜L3・仮想優先・通常開発とDaily Full CIの責務分離を維持する。形式的検証は別の保証面でありOS/GUI/ABI検証の代替ではない。CI必須化はリスク・コスト・環境に応じ個別判断し、Requiredな証明はNotRunで受入しない。

## 7. Synvelis固有の適用

- OS-L1ではProject/Track/Clip/Undo/Save、Playback/Export、録音Mock/Fault、RT NoAlloc/NoBlock、Plugin/GUI Driver-neutral契約の独立Oracleを保護する。
- Windowsで製品として実際に提供するDAW実GUI、録音/モニター/OS MIDI、VST3/CLAP等のNative Plugin DockをGUI-L2/EXT-L3で検証する。初回からRuviformの実OS仮想Audio/MIDI Endpointと正規ABIのTest Plugin/Native Editorを使用できるが、実Native Dockに画素・Focus・入力の未解決不具合がある場合、当該提供機能を未受入として追跡する。
- Ruviform共通Test Runtime/仮想Endpoint/Test Plugin/実GUI Runnerを使い、Recording PCM/RT/Project、MIDI、Plugin State、DAW GUI/UXはSynvelis独自Oracleで受入する。Test RuntimeのPassを製品Passへ自動転用しない。
- 利用しないPlugin形式・機材は必須外とする。DAW固有のProject/編集操作/UX/Recording Workflowと製品受入はSynvelisのIssueで管理する。一方、ドメイン非依存で再利用可能なTime/Graph/Device/Plugin/GUI Port・実行契約・テスト基盤等の課題はRuviformのIssueとして起票し、共通基盤へ還元する。Ruviformの変更は両Consumerの精度・RT・互換性を独立に検証し、必要な導入・追従対応は各ConsumerのIssueで管理する。
