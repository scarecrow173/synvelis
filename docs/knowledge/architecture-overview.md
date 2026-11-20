---
id: "KNOW-AmJQHxhnW2zRPGvDY1wqqG"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "architecture"
title: "システムアーキテクチャ概要"
created: "2026-11-20"
updated: "2026-11-20"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/597"
    - "docs/adr/l1-player-slice-boundaries.md"
    - "docs/adr/l2-render-slice-boundaries.md"
    - "docs/adr/vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md"
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
    - "docs/adr/gw-01-gateway-contract-crate-boundary.md"
    - "docs/adr/sv175-native-shell-driver-neutral-gpui.md"
    - "docs/adr/sv-gui-02-syn-binding-ipc-surface-notify-outbox-bounded-wire-dto.md"
    - "docs/adr/x-recorded-media-canonicalization-boundary.md"
  informs: []
  explains: []
  supersedes: []
---

# システムアーキテクチャ概要

## 概要

Synvelis は Ruviform を共通基盤とする Rust 製 DAW であり、単一の Cargo workspace（`crates/` 以下 15 クレート）と、`ui/` 配下の Vite + SolidJS フロントエンドで構成される。アーキテクチャの骨格は三つの規則で説明できる。第一に、永続化されるドメイン状態は `synvelis-project` の `Project` 集約に閉じ込められ、すべての変更は唯一の書き込み経路である `ProjectController` を通る。第二に、セッション状態、コマンド実行、UI 配線は `synvelis-workspace`・`synvelis-gateway`・`synvelis-app` がそれぞれ別の境界として受け持ち、ドメインの真実を複製しない。第三に、プロジェクトの型はオーディオランタイムの境界を越えず、再生・エクスポート・録音はいずれも lowering によって `ruviform_graph` の汎用グラフへ落としてから実行する。依存方向は常にドメイン側から基盤側へ向かい、`synvelis-app` が各サービスを組み立てる唯一の合成ルートとなる。

## 詳細

### レイヤ構成と依存方向

workspace の crate は、おおむね五つの層に分けられる。依存は下位層へだけ向かい、上位層が下位層から参照されることはない。

```text
┌─ 入口・合成 ───────────────────────────────────────────────┐
│ synvelis-app (bin: synvelis / synvelis-native)             │
│ synvelis-cli   mcp-poc   recording-poc                     │
└──────────────┬─────────────────────────────────────────────┘
               │
┌─ セッション・コマンド境界 ────────────────────────────────┐
│ synvelis-workspace   synvelis-gateway                     │
│ synvelis-automation  synvelis-script  synvelis-script-luau│
└──────────────┬─────────────────────────────────────────────┘
               │
┌─ ドメインサービス ────────────────────────────────────────┐
│ synvelis-player   synvelis-render   synvelis-recording    │
│ synvelis-library                                           │
└──────────────┬─────────────────────────────────────────────┘
               │
┌─ ドメインコア ────────────────────────────────────────────┐
│ synvelis-project   synvelis-theme                          │
└──────────────┬─────────────────────────────────────────────┘
               │
┌─ 共通基盤（git 依存: ruviform）───────────────────────────┐
│ graph / audio* / media-* / io-* / rt / exec / plugin-* /   │
│ ui* / viz / observability / script*                        │
└─────────────────────────────────────────────────────────────┘
```

Ruviform は別リポジトリの git 依存として解決され、メディア・グラフ・RT・I/O・UI・プラグインの汎用契約を供給する。Synvelis 側の crate が担うのは DAW 固有の意味論（プロジェクトモデル、lowering、ゲート、永続化、製品としての合成）であり、ドメイン非依存の責務は Ruviform に残す分界が `AGENTS.md` と ADR の各境界文書で固定されている。実際の依存の向きはルート `Cargo.toml` と各 crate の `Cargo.toml` が正本である。

### crate 一覧と責務

| crate | 責務 |
| --- | --- |
| `synvelis-project` | 永続ドメインの全値型（`Project` 集約、安定 ID、時間・範囲・テンポ、ルーティング、コントロール、バリアント、録音レジストリ POD）、`Operation`/`Command` 契約、revision・undo、永続化ポート `ProjectStore`。唯一の書き込み点 `ProjectController` を持つ |
| `synvelis-library` | アセット Provider ポートと実装（`LocalFolderProvider`、Git、WebDAV）、検証付きインポート（staged→verified→published→committed）、`GitVcsProvider` による VCS 読み取り |
| `synvelis-recording` | 録音の RT キャプチャ経路、SPSC ハンドオフ、staging writer とジャーナル、復旧、プロジェクトへの単一コミット。永続プロジェクト状態は持たない |
| `synvelis-player` | 再生エンジン。プロジェクトの lowering（`GraphSpec`+`PlaybackPlan`）、ソース解決（検証済み blob→demux→prefetch）、`ClipPlayer` プロセッサ、`HeadlessDriver`/`RoutingEngine` の駆動、モニター、パラメータ dispatch |
| `synvelis-render` | オフラインエクスポート。スナップショット凍結、メモリ内デコード、独立したプロセッサ群、`ruviform_audio::render` による逐次駆動、WAV 出力と再読込検証、アトミック公開 |
| `synvelis-gateway` | コマンドゲートウェイ。wire JSON の parse→bind→invoke、capability レジストリ、ポリシー評価 seam、Prepare/Proposal エンジン、書き込み唯一経路 `CommitGate`、監査用ジャーナル |
| `synvelis-script` | スクリプト基盤のポート側。レジストリスナップショットから `ScriptApiModel`・`BindingSet` を生成し、`InvocationDraft`（権限情報を持てない wire 前身）を介してゲートウェイへ提出する |
| `synvelis-script-luau` | `synvelis-script` のポートを `ruviform_script_luau::LuauBackend` に適合させるアダプタ。mlua は Ruviform 内に閉じる |
| `synvelis-automation` | ワークフローの決定的実行。YAML→`PlanCompiler`→`ExecutablePlan`、ステップレーン seam、実行 FSM、run ジャーナル |
| `synvelis-workspace` | セッション基盤。`WorkspaceManager`（dock・view・focus・link）と唯一の `ProjectSession`、編集モデル（selection/tool/snap/gesture/edit）、`*_ui` 読み取りモデル、workspace 固有スナップショット |
| `synvelis-app` | 製品の合成ルート兼 GUI ホスト境界。`SynvelisApp` が `WorkspaceManager`・エンジン・レコーダ・観測を束ね、`AppService` が `syn.*` op 面を公開し、`NativeShell` がドライバ中立のイベントルーティングを担う |
| `synvelis-theme` | 製品テーマ資産（DTCG 文書・workspace レイアウト）の検証と `ruviform-ui-theme` への適合 |
| `synvelis-cli` | ヘッドレス CLI バイナリ。action/workflow/script-analyze/project/session の各コマンドをゲートウェイ経由で駆動する |
| `mcp-poc` | MCP トランスポートの PoC。permit 発行と `McpHost` 経由のツール公開 |
| `recording-poc` | 録音パイプラインの決定的ハーネス（VS-B ADR がキャパシティ決定を測定に委ねた根拠） |

### 起動から主要フローまでの制御線

アプリの組み立ては `synvelis-app` の `SynvelisApp`（`crates/synvelis-app/src/app.rs`）に集約される。ここで `WorkspaceManager<ShellWindowHost>`、その内部の唯一の `ProjectSession`/`ProjectController`、エンジンスロット、レコーダ、観測基盤、UI 読み取り状態が合成される。`AppService`（`service.rs`）はこの合成物を `ruviform_ui::Service` 契約の `syn.*` op 面として公開するだけの薄い dispatcher であり、`NativeShell`（`shell.rs`）は `ruviform_ui` の `WindowDriver`/`DriverEvent` 契約を実装するドライバ中立ホストである。

起動レーンは三系統ある。webview レーン（`src/bin/synvelis.rs`、feature `webview`）は、ロギング install→`detect_backend`→`SynvelisApp::new`→`SYNVELIS_PROJECT_DIR` があれば boot project と `compose_product_routing`（`SessionRoutingEngine`、48 kHz/stereo/256 フレーム）→`check_bundle` でフロントエンド資産を検証→`NativeShell`→`run_multi` の順で、`rvf://app` に埋め込み UI、`rvf://data` に `DataRegistry` を配線する。native レーン（`src/bin/synvelis-native.rs`、feature `native`）は `GpuiDriver` を組み立て、`ViewHost`+`ViewBinding` によるマウント済みビューを `NativeShell` の同じハンドラへ流す。ヘッドレスレーン（既定 feature）は `ruviform-ui-test` の `FakeDriver` で同一経路を検証に使う。webview・native はいずれも非既定 feature で、`cargo check --workspace` が GUI 依存なしに通るよう `tools/check-headless-deps.mjs` がガードする。

UI からドメインへの往路はレーンを問わず `rvf.ipc.v1` の intent に統一される。webview 側は `ui/src` の ViewModel→`__rvf.call`→`window.ipc.postMessage` でホストへ入り、`NativeShell::handle`→`UiRuntime`→`AppService::call`→`ui_ops.rs` の op 実装→`WorkspaceManager` 系の変更と進む。native 側は `ViewBinding` が `rvf.ui.command.v1` intent を発行し、同じ `NativeShell` の `on_view_command` 検証（origin view と generation の fail-closed 照合）を経て同じ op 面へ合流する。復路は `Service::call` が「1 intent→1 result」しか返せないため、継続状態は bounded outbox（cap 4096）経由の `Event::Notify`（`syn.control.projection`、`syn.control.receipt`、`syn.viz.frame` 等）でプッシュされる。合成・配線の詳細は `docs/knowledge/gui-app-shell.md` を参照。

### プロジェクトの変更と永続化の単一経路

ドメインの書き込み経路は一点に絞られる。`synvelis-project` の `Operation` は「`validate(&Project)` → `apply(&mut Project, &mut dyn IdGenerator)`」の副作用を持たない全関数で、`Command` が `expected_revision` を添える楽観的排他を担う。`ProjectController`（`controller.rs`）は期待 revision の照合→検証→クローンへの変更→ライブ状態との差し替え→undo スタックへの旧状態 push、の順で処理し、失敗時はライブ状態を一切変更しない。undo/redo は逆操作の再生ではなく `Project` 全体のスナップショット復元であり、単調増加する revision と `saved` 基準は巻き戻らない。

永続化は `ProjectStore` ポートの本番実装 `DirectoryBundleStore`（`store.rs`）が担う。ディスク上は `<root>/snapshots/r<rev>.json` と `HEAD`/`HEAD.lock` の束で、`SnapshotEnvelope` が `schema_version`・`project_id`・`project_revision` を持つ。保存は `StagedFile`→`SealedFile` の分類付き公開（`PublishMode::CreateNew` のイミュータブル snapshot と `HEAD` のアトミック置換）で、`PublishEffect::Indeterminate` を含む結果を正直に返す。読み込みは `HEAD`→envelope→順序付き v→v+1 マイグレーション（`migration.rs`）→`validate_references`→`id_watermark` 検査→ジェネレータ再シードの順で、失敗時は既存状態を保持する。

エンティティ ID は `id.rs` のタグ付き `u128` newtype で、`"<kind>-<32hex>"` 文字列として直列化される。`WELL_KNOWN_BASE`（`1<<96`）以上は bootstrap 予約域としてジェネレータが払い出さず、プロジェクトの `id_watermark` が保存・復元時にジェネレータを再シードする。`ProjectId`・`TrackId`・`LaneId`・`ClipId`・`SourceId`・`ProcessorId`・`SemanticPortId`・`VariantSetId` など、種別は文字列の kind 部分で読み込み時に検査される。

### 再生: 保存プロジェクトからサンプルまで

再生は「lowering→resolve→activate→drive」の四段で組み立てられる。第一段の lowering（`synvelis-player/src/lowering.rs`、`routing_lower.rs`）は、clip/lane/comp/transition/tempo map を `GraphSpec`+`PlaybackPlan`（あるいはセマンティック経路の `lower_semantic*`）に変換し、アクティブ comp の範囲上書き、トランジションのフェード窓、時間領域（`MusicalTick`/media anchor/engine frame）の変換を一度だけの丸めで確定させる。レート不一致や未対応の loop/stretch は暗黙のリサンプルではなく計画上の明示的 skip として記録される。

第二段の resolve（`resolver.rs`）は、再生可能ゲート（`SourceState >= Verified`、登録済み asset、検証済み blob digest の `open_blob_verified`）を通したうえで、`ruviform_io_symphonia` の demux→`CodecId::Pcm` 要求→`ruviform_media_reader::SourceReader`（canonical decode）を開き、`PrefetchHandle`/`PrefetchProducer`/`PrefetchConsumer` の三役を持つ `PreparedClip` を生む。第三段の activate は validate→compile→publish の last-good 方式で、失敗しても既存の公開プランを保つ。第四段の drive は `HeadlessDriver`/`RoutingEngine`（`driver.rs`、`routing_engine.rs`）が行い、ブロックごとに prefetch を pump→共有アトミック時計へカーソル書込み→`rt.process`→sink tap から planar PCM を取得する。`ClipPlayer` は各ノードに付く `GraphProcessor` で、`ClipWindow` への位置写像と consumer からの充填だけを担い、`Project`/`Clip`/`Track` の型を参照しない——プロジェクト型がグラフ境界を越えないという L1 ADR の規則の帰結である。

`RoutingEngine` は同じ Ruviform プリミティブでセマンティックルーティングのプランを動かす新版経路であり、`publish` が revision・routing fingerprint・capability generation・source map・単調 generation を再検証する。モニター用途の `MonitorEngine` は `EngineBlocks` 経由でどちらのエンジンも包め、実際のマスタータップ PCM を消費する。`HeadlessDriver` は物理デバイスを開かず、実機バックエンドはこの層の上の統合として扱われる（入力側は `real-device` feature の `ruviform-audio-device` のみ）。

### エクスポート: オフラインレンダ

エクスポートは再生と抽象を共有するが別パイプラインであり、プロセッサや駆動ループをライブ経路と共有しない（L2 ADR）。`synvelis-render` の `RenderSnapshot::freeze` がライブ `Project` の複製と revision・ソース digest を凍結し、`prepare.rs` が必要なクリップ窓だけを `DecodedWindow`（canonical planar f32、prefetch なし）へデコードする。プロセッサは `ClipRenderer`・`GainProc`・`OnePoleProc`・`DelayProc` の新規インスタンスで、`OfflineRenderSupport` を正直に申告し、未対応ノードは Ruviform の offline 能力検査が fail-closed に止める。`PreparedRender::compile` の後、`RenderTransport` が `ruviform_media_core` の `TransportScheduler` でプレーロールを処理し、逐次駆動は `ruviform_audio::render` に委譲する。

出力は `ruviform-io-wav` のエンコードを `StagedFile` へ書き、`verify.rs` の `reopen_check` が独立した読み込み経路で再検証（geometry・フレーム数・必要に応じ全サンプル比較）してから `PublishMode` でアトミック公開する。`ExportJob` は `ruviform-exec` の `Progress`/`CancellationToken` を持ち、`ExportOutcome::Completed` は再読込検証と公開の両方が成功したときだけ成立する。RCP-03 の `JobOrchestrator` は pinned target・dedup・budget/grant/consent ゲートを持つ非 RT のジョブ管理で、offline 失敗を realtime へ暗黙フォールバックしない。

### 録音: 入力からプロジェクトコミットまで

録音は RT キャプチャ経路と非 RT 永続化経路を `ruviform-rt` の SPSC で切り離す構造である（VS-B ADR）。RT 側は `CaptureEngine::process`（`engine.rs`）で、事前確保した `CaptureChunk` プールと `BoundedVec` スクラッチだけを使い、ゲート評価（`GateSchedule` の bounded lookahead）→チャンク取得→PCM+span 書込み→filled キューへの `try_push` を、alloc・lock・OS I/O・panic なしに行う。`try_push` が満杯で返すペイロードは drop せず `retained` スロットへ退避し、故障はデータキューと別系統のアトミック `FaultLatch` で単方向に記録される。

非 RT 側は `StreamWriter` が filled チャンクを順に検証→raw f32le PCM 追記→ジャーナル追記→フリープール返却と進み、チェックポイントは media sync→journal sync→dir sync の順序で `DurableFrontier` を確定する。staging 配置は `<staging_root>/<session-tag>/manifest.json` + `streams/stream-NNNN.{pcm,journal,finalized.json}` で、dir-sync 非対応（Windows 等）は `DirSyncCapability::Unsupported` として隠さず記録される。復旧はジャーナルリプレイの検証済みプレフィクスまでを採用し、未検証テールを復元済みと偽らない。コミットは `RecordingOp::CommitRecording` が `RecordingCommitManifest`（安定 ID 予約済み・`applied_commits` による冪等）の単一プロジェクト変更として適用され、その後の canonical publish（生 PCM→canonical F32 WAV→sha256 blob）で、録音物はインポート物と同一の検証済みソース経路で再生可能になる——録音専用の再生経路は存在しない。

### コマンド実行: ゲートウェイとプロポーザル

`docs/knowledge/automation-and-command-gateway.md` が詳述するコマンド面の構造を、ここではシステム上の位置として要約する。呼び出し側（CLI/MCP/スクリプト/ワークフロー/UI intent）は wire JSON と、wire には乗らない信頼済み `CallerIdentity` を持ち、`Gateway::handle` の parse→bind→invoke を通る。`CallerIdentity` は `pub(crate)` の `AdapterSeal` でしか mint できず、発行者は `FixtureAdapter`・`McpHost`・`LocalCliAdapter` の三者だけである。`WireRequest::parse` は `deny_unknown_fields` と禁止キー走査で、自称の権限キー（`principal`/`auth`/`token` 等）と暗黙ターゲットキー（`selection`/`focus` 等）を拒否する。

invoke 段では capability の厳密バージョン解決、`Availability`、スキーマ互換、`PolicyEvaluator` seam による段階別判定、冪等レジャー照合を経て `CapabilityOperation::execute` へ渡る。クエリ系は読み取りだけを行い、書き込み系は `PrepareEngine` が `PinnedSnapshot` 上の private clone で `ProjectOp` へ lowering して `DerivedEffect` を差分から導き、`ProposalEngine` の `Proposal → Approval → Apply` 状態機械を通る。実際の書き込みは `CommitGate` が `MutexFence` 内で「再認可→`ProjectController` への変更→durable ジャーナル」の順に行い、ジャーナル記録失敗は gate を diverged として以後の変更を poison する——`Accepted` は完了ではなく、未確定は `Indeterminate` として成功に昇格しない。

`synvelis-automation` はこのゲートウェイを呼ぶだけの決定的プラン実行層で、`PlanCompiler` が capability スナップショットにピン留めした `ExecutablePlan` を作り、`GatewayStepLane` が delegated identity で `Gateway::handle` へ流す。不明・未確定の結果は run を park させ、成功への繰り上げを行わない。`synvelis-script` はゲートウェイのレジストリ真実だけを消費する読み取り側で、`InvocationDraft`→`verify_wire`→`submit` の三段で同じパイプラインへ入る。`synvelis-script-luau` はセッションごとに新規 Luau 状態を立て、host function を注入しない——スクリプトは操作を直接呼ばず、`ScriptActionSpec`/`ScriptWorkflowSpec` という成果物を出力し、それを automation がプランへ落とす。

### 横断的な不変条件

コード全体に共通する規則を挙げる。いずれも実装に散らばる局所対応ではなく、型・ポート・feature・lint の形で強制される。

- **単一ライタと単調 revision。** プロジェクトの変更は `ProjectController` の `expected_revision` CAS＋validate-then-clone-swap に限定され、undo はスナップショット復元、revision と `saved` 基準は巻き戻らない。`ProjectSession` の `EnginePort::on_project_revision` は revision を進めるコミットでのみ発火し、view 系変更では再構築されない。
- **時間演算はすべてチェック付き。** `ruviform-media-core` の `Tick`/`TickDuration` は unchecked `Add`/`Sub` のみ公開であり、Synvelis 側の時間演算は `checked_add`/`checked_sub` に統一される（`AGENTS.md` の lint 対象、`synvelis-project/src/time.rs`）。
- **RT 経路の無割当・無ロック・無 OS I/O・無 panic。** 事前確保プール、payload 返却の `try_push`、アトミック fault latch、bounded lookahead で構成され、`rt_` 接頭辞テストが計数アロケータで零割当を検証する。workspace 全体は `unsafe_code = "forbid"`。
- **fail-closed と正直な結果報告。** 型付きエラーと `Indeterminate`/`Unsupported`/`Unresolved` を成功へ昇格させない。能力・durability・digest・時計精度の「わからない」は隠さず記録され、`syn.app.capabilities` も未実装の領域を `unavailable` と返す。
- **呼び出し者の偽造不能。** `CallerIdentity`/`OperationRequest` は私有フィールドを持ち `Deserialize` できず、wire の権限自称キーは parse で拒否される。ローカル無人実行は常に `Agent`+`LocalCli` で、`Human` は store 検証済みクレデンシャルか `Control` チャネル permit のみ。
- **エンジン適用は last-good と generation 検査。** activate/publish は revision・fingerprint・capability generation・source map・単調 generation を再検証し、失敗時は既存プランを保持する。
- **ドライバ・バックエンド中立。** UI は `WindowDriver`/`Service`/`ViewHost` のポートで抽象され、OS window・webview・GPUI は `synvelis-app` の feature 付き bin にのみ現れる。オーディオも headless エンジン＋sink tap が中心で、物理 I/O はオプションの `ruviform-audio-device` に隔離される。
- **永続・ランタイム ID の分離。** `RoutingSourceMap` が安定なセマンティック ID を世代付きで `ruviform_graph` の `NodeId`/`PortId`/`ConnectionId` に写像し、ランタイム ID は永続化されない。
- **境界の有界性。** outbox cap、subscription/projection の上限、`McpBounds::max_message_bytes`、ジョブ・スクリプト・リクエストの予算など、各境界に上限が設けられる。

### サブシステム別 knowledge 文書への導線

全体像の入口としてこの文書を置き、詳細は既存のドメイン別 knowledge 文書に委ねる。

| 領域 | 文書 |
| --- | --- |
| プロジェクト基盤・workspace・編集モデル | `docs/knowledge/foundation-and-workspace.md` |
| GUI シェル・合成・IPC | `docs/knowledge/gui-app-shell.md` |
| タイムラインと編集モデル | `docs/knowledge/timeline-and-editing.md` |
| セマンティックルーティング | `docs/knowledge/semantic-routing.md` |
| ミキサー・Strip・バリアント | `docs/knowledge/mixer-strip-and-variants.md` |
| パラメータコントロール | `docs/knowledge/parameter-control.md` |
| メディア I/O とレンダリング | `docs/knowledge/media-io-and-render.md` |
| Ruviform I/O 境界・StagedFile | `docs/knowledge/ruviform-io-boundary.md` |
| 録音基盤 | `docs/knowledge/recording.md` |
| プロジェクトバージョン管理 | `docs/knowledge/project-version-control.md` |
| アセットライブラリ・Asset Provider | `docs/knowledge/asset-library.md` |
| オートメーション・コマンドゲートウェイ | `docs/knowledge/automation-and-command-gateway.md` |
| スクリプト基盤・機能スライス | `docs/knowledge/scripting-and-feature-slices.md` |
| CLI・配線・本番統合スライス | `docs/knowledge/app-integration-slices.md` |
| CI と検証プロセス | `docs/knowledge/ci-and-verification-process.md` |
