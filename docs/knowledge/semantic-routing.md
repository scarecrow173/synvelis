---
id: "KNOW-vFNnYDJyzN80X5gZ8HvqR9"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "architecture"
title: "セマンティックルーティング"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/108"
    - "https://github.com/scarecrow173/synvelis/issues/109"
    - "https://github.com/scarecrow173/synvelis/issues/230"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/ruviform/issues/306"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
  informs: []
  explains: []
  supersedes: []
---

# セマンティックルーティング

## 概要

Synvelis のオーディオルーティングは、永続化されるセマンティックモデルと実行時に生成される runtime モデルを分離する設計で進められた。Bus・ChannelStrip・ProcessorChain・SemanticPort・RoutingConnection・SendConfig といった安定セマンティック ID を永続モデルの正本とし、semantic→runtime lowering、activation、プローブ、Signal Flow 表示を別々の層と issue に分けて実装した。この分離により、編集の原子性、lowering の証拠、プローブの時計同期、UI の read model がそれぞれ独立した契約を持つ。検証は headless の独立 oracle と lane 分離 evidence ledger を軸に行い、外部 gate（実機・plugin・consumer）は NotRun/Deferred として明示する運用が一貫している。

## 詳細

### 永続モデルのセマンティック ID とスキーマ移行

SF-01（synvelis#104）で、永続モデルは安定セマンティック ID を導入した。対象は Bus・ChannelStrip・ProcessorChain・SemanticPort・RoutingConnection・SendConfig などである。Schema v5→v6 の順序付きマイグレーションと、厳格なルーティング不変条件 oracle を同時に導入し、複合ルーティング編集は 1 CAS / 1 Undo command に統合する設計とした。

ランタイム lowering・mute/solo 挙動・一時参照トランザクションは SF-01 の scope から意図的に除外し、SF-02/SF-03 へ繰り延べた。この scope 分離により、永続モデル層（synvelis-project）と実行層（synvelis-player）の責務を分けた。gateway 層の op_facts は fail-closed catch-all で routing ops を拒否し、gateway の routing capability model は下流 issue に明示的に委譲した。当時 routing op は session/controller 経路のみであった。

### ルーティング編集の原子性と一時参照

SF-02（synvelis#105）では、RoutingTransaction が TempBusId・TempSendId・TempConnectionId の一時参照で未確定 entity を表現する。RoutingEditIntent は staged preflight と atomic commit を持ち、複合編集を 1 revision / 1 undo に集約する。拒否時は live project・revision・undo 状態を一切変更しない fail-closed が契約である。

mute/solo の権威は strip 所有の control target に置き、独立した mute bus のような重複権威を作らない。検証はセマンティック検証と汎用グラフ検証の二層構成を採用した。channel/mute/solo のモデル変更は additive に行い、既存 v6 schema を破壊しない方針とした。

### semantic→runtime lowering と activation の証拠

SF-03（synvelis#106）では、semantic→runtime lowering の契約を RoutingBuildKey・SourceMap・RoutingFault taxonomy・activation receipt・last-good generation・stale publish 防止で構成した。RoutingEngine を generation スコープの receipt・source-map 証拠の唯一の所有者（authoritative owner）とした。

NoRebuild Equivalence が中核の最適化である。semantic 同値の新 revision（Project 名変更・ViewState 相当・undo/redo）には RoutingPrepare::Equivalent を返し、generation 消費・host connect・compile を一切発生させない。同一 revision の再 prepare、engine/source-format/capability generation 差、build mode/epoch/instance 差は常に通常 build とする。reconcile は revision のみの drift を fault とせず、semantic drift は従来どおり報告する。

processor control は世代スコープで engine が保持し、take_processor_control で one-shot 払い出しする。unknown/stale generation・processor・non-plugin 接続は None を返す。lease teardown closure は host lifecycle のみを担い、control 半体は含めない。ProcessorLease の drop が teardown を担い、ライフサイクルは Instantiate→Initialize→Activate→StartProcessing→attach、終了時は best-effort で StopProcessing→Deactivate→Destroy の逆順 cleanup である。StaticProcessorHost は制御払い出し None を返し、headless fixture を成立させる。

SessionRoutingEngine（synvelis-app）は workspace の EnginePort trait（on_project_revision/graph_builds/last_built_revision）の cloneable handle 実装であり、revision 通知→plan_playbacks→resolver::prepare→RoutingDeps→RoutingEngine::prepare→publish を駆動する。canonical_format probe は verified-open を resolver::prepare と共有し、二重の format 解決を避ける。clip 解決失敗は last_prepare_error に記録し、engine を untouched に保つ。

capability generation は ProcessorHost::capability_generation（default 0）として導入した。register_boundary/register_params の epoch bump と Scan descriptor+epoch の FNV-1a 畳み込みで決定的に変化させ、publish は project 側 sum + host generation の 2 半分を再検証する。equivalence は host_current を要求し、reconcile は CapabilityGeneration mismatch を報告して PluginCapabilityChanged を live 発火させる。これにより prepare→publish 間の factory drift・host 差し替え・semantic 同一 revision 下の drift を全て fail-closed で検出できる。

RoutingControlDispatch は bounded fast-control queue・catalog refresh 時の revalidate・queue drain 中の semantic-intent re-lowering を採用した。loop math を第二 accessor で再計算する案は、WrapSim の scheduling 意味論を重複させ staged/scheduled command 処理の不整合を招くため却下した。

検証面では、same-revision fixture が receipt・source-map・probe binding・signal_flow_view status の同一 committed revision を指すことを app/workspace/render consumer 横断で確認する。visual op は revision・engine・receipt を動かさず、semantic no-op は Equivalent だが projection は旧 map を stale と表示する。save は persistence evidence であり activation 証拠を生まない。controller bypass commit は reconcile で ProjectRevision mismatch+fault として表面化し、session apply で収束する。

### プローブ契約・時計 epoch・capture segment

SF-04（synvelis#107、ruviform#308/#309）では、probe 契約型（SignalBoundary・SemanticSignalEntityRef・SemanticProbePoint・ProbeSubscriptionKey・ProbeResolveState・SemanticProbeBinding・ProbeCaptureSegment 他）を synvelis-project が所有する。resolve_probe_point は fail-closed で Unavailable/Unsupported を正直に返し、PCM を捏造しない。

player 側 ProbeSession は ProbeBroker・ProbeCollector・binding ledger・segment ledger を持ち、requested/queued と RT-installed を区別する。generation 不一致は stale として拒否し、publish 成功時の on_publish rebind が RebindReceipt 経由で capture segment を cut する。auto-recapture は行わない。ProbeTraceBridge は ProbeEvent→ProbeObservation を fail-closed に写像する（generation 照合・clock provenance・status 写像）。ControlSession が ProbeTraceSession state を所有し drain を単一 owner にし、port は per-call &mut dyn ProbeTracePort で注入する。

時計の不連続は CaptureSegmentEnd::ClockEpoch で扱う。epoch 境界を跨ぐ capture segment を存在させない契約であり、drain 側の epoch 照合は lazy cut（triggering record 破棄・closing kind 除外・unwired clock no-op）で行う。RoutingEngine::seek が epoch を bump し、probe_time_context() accessor を公開する。Monitor/Capture probe が master PCM/CRC を変えない DSP 不変性を fixture で固定した。bounded fault（capacity・rebind channel-full）は fail-closed で原子性を保ち、次 publish で回復する。

probe subscriber/window ownership は session 層の lease（ProbePrincipal/ViewInstanceId）と capture owner で管理する。同一 (session, point, kind) への 2 view subscribe は provider request 1 回に dedup され、最後の release でのみ provider remove が走る。capture owner の lease 喪失時は owner が Session に退避し capture は継続する。port 失敗時は局所 lease/owner 状態を変更しない。ProbeCoverage は per-probe bounded CoverageEvent 系列（PROBE_COVERAGE_MAX_EVENTS=2048、loss floor 付き）である。

probe_compare は resolve→build side→budget→ObservationSpan→align_observation_points→contract 写像の順で AcousticObservation を構成する。seek/epoch 不連続・generation 境界・coverage gap は Unknown/Incomparable/Conditional として正直に報告し、aligned を捏造しない。syn.probe.compare app op は &self read-only で、engine 未接続時は {engine_attached:false, observation:null}、malformed request は INVALID_PARAMS を返す。probe op は drain・project revision・bindings・master PCM を変えないことが契約である。

loop 不連続は、observe_block/observe_node_block が WrapSim::walk_window の LoopWrap 点と SetLoop/ClearLoop apply（note_transport_command の noted 判定）を検出する。SessionRoutingEngine::run を per-block run_observed(1)+PumpPolicy emulation+PlayReport merge に再構成し、各 block 観測後に best-effort declare_probe_discontinuity を呼ぶ。callback 実行中の即時宣言は engine/bridge が mutable borrow 中のため却下し、run 末尾のみの宣言は後続 block が旧 epoch で record を emit するため却下した。宣言失敗（CommandChannelFull）は best-effort で飲み、跨ぎ compare は fail-closed に倒す。

ProbeCollector::drain には順序ギャップの構造的欠陥があった。close status が生成済み PCM chunk より先に drain されると chunk が orphan される。PendingClose による batch 末尾 close で修正した。BudgetExhausted は最終 chunk と CaptureStopped status が必ず同一 drain batch で生成されるため、telemetry-first 配信順序では close 前 chunk が孤立する。app 配線では syn.probe.control/syn.probe.state op を service.rs に登録し、ui_pump tail の drain hook が ControlSession::probe_drain を唯一経路として呼ぶ。

### Signal Flow read model と view state

SF-05（synvelis#108）では、Signal Flow は workspace read model で semantic state と visual state を分離する。visual-only 操作（scope/collapse/role filter 等）は project revision・project JSON・engine build・receipt を一切変更しない。

routing gesture は revision・capability generation・window を pin し、confirm は window→capability→CAS revision の順で検査する。stale gesture は retarget せず、input-context 遷移で gesture を drop する。ViewState は ViewInstanceId 単位で schema v2→v3 migrate・malformed degrade の safe recovery 付き永続化を行う。inspection は Follow/Explore/Pinned mode・session-only 履歴・FollowSource filter を提供し、link delivery は明示変換成功時のみ Signal Flow Follow focus を更新して selection/arrangement focus を変えない。

probe の view-scoped action（subscribe/unsubscribe/capture_for/abort_capture）は view_id→ViewInstanceId 解決で session authority action と分離する。op_view_close/close_vcs_view は当該 view の lease のみ解放し、provider 失敗時は view を残して close を retry 可能にする。syn.probe.state は holders/capture_owner/subscribers を投影し、Signal Flow tab の probe table は read-only 表示とする。TypeScript view model は wire projection を verbatim 保持し domain state を再計算しない。

wire 語彙は経路で異なる。Signal Flow は Debug 整形データ、probe serde は kebab-case、entity tag は PascalCase（"Track" 等）であり、lowercase 前提の fixture は誤りである。

### 独立 oracle と lane 分離 evidence

SF-06（synvelis#109）では、persisted domain に対する独立 oracle（AudibilityOracle・routing_invariant_violations）を採用した。mix topology の exhaustive mute×solo sweep（2^14）・chain topology・fault matrix（reject→state unchanged）・v5→v6 migration oracle を headless で実測する。Case-ID registry と lane 分離 evidence ledger（headless/native/GUI/device/plugin/RT/consumer を列で分離）を採用し、headless Pass を実機・device・consumer 受入へ昇格させない規則を spec/ADR に明記した。外部 gate は NotRun/Deferred/Unsupported を明示し、Pass 扱いしない。

SF-XR-01 CTL-01/CTL-02 cross-gate fixture は、strip/send 権威・serialization・undo・lowering・channel-map consent・atomic rejection を persisted 層と runtime 層を跨いで検証する。fixture 追加前に既存 coverage（ctl_01・ctl_02・sf_02・sf06_oracle・sf03_l1）を棚卸しして重複を回避する。

StagedFile 移行（synvelis#84、ruviform#294/#305/#306）では、synvelis-project の DirectoryBundleStore が Ruviform の StagedFile/PublishFailure を利用する。Snapshot は CreateNew、HEAD は replacement/CAS+HEAD.lock、durability は FileAndDirectorySync であり、collision・stale-lock breaking・fencing・orphan/temp recovery・2 プロセス CAS を fixture で検証した。「Publish ≠ Project Commit」の責務境界と、両 consumer（Synvelis・AudioLens）の独立 Golden を確認する。実装本体は先行 task で main 統合済みであり、証拠統合 slice は production code を追加しない。

### Ruviform との責務分離

Ruviform pin は main HEAD 追従で bump し（例 cc7d5296→0f5b5737 で ruviform#308/#309 を取込）、Cargo.lock 反映と check-ruviform-rev green を伴う。Ruviform 依存は既定ブランチ追従の git 依存で解決 revision は Cargo.lock に記録し、恒久的 SHA ハードコード・vendor コピーは禁止である。ui/vendor/ruviform は gitignore された生成物で、pin 更新・cargo update・ブランチ切替後は mise run sync-ui-deps で lock rev へ再同期する。check-ruviform-rev の REV 不一致（ui/vendor/ruviform/REV vs Cargo.lock 解決 rev）は stale vendor を検出する fail-closed である。

Ruviform 側の不具合・改善要望は Synvelis 側で恒久的重複実装せず、Ruviform Issue を確認・起票して相互参照する。

### 実装・検証で得られた教訓

fixture 設計では、SF-03 revision fixture で明示的 track→master 接続追加が duplicate-connection validation error に衝突するため既存 implicit routing を使う。bypass fixture を AddTrack だけで構成すると strip が無く rebuild が失敗するため、既存 lane/source に AddClip bypass semantics を使う。静的 processor host は instance を one-shot 消費するため、反復 semantic build には接続ごとに fresh processor を vend する test-local FreshGainHost が必要である。

UI 実装では、private inspection() helper が public inspection getter と衝突し vitest 4 件失敗したため sendInspection() に改名して解消した。private viewId field が get viewId() と Duplicate identifier 'viewId' を起こし tsc --noEmit で 4 件エラーになったため viewIdValue に改名した。TypeScript の promise 変数は互換性のある広い型（Promise<unknown> 等）への widening が tsc --noEmit で必要である。wire test は当初 lowercase entity tag を仮定したが、実際の serde 契約は PascalCase variant（"Track" 等）であり、fixture を契約に合わせて修正した。

設計段階レビューの効果も確認された。SF-04 slice5 の設計レビューで invariant 矛盾 2 件と unsubscribe atomicity の問題が実装前に検出され、後工程の手戻りを防いだ。

失敗の分類では、既存 flaky/環境起因の失敗をコード起因と混同しない。Windows WebDAV test の os error 10054（TransientIo vs InvalidRequest 分類差）は単独再実行・全量再実行・MSRV で pass し既存 flake と分類した。os error 112 は C: drive 一時枯渇（並行 worker build）で、容量回復後の再実行で全 pass しコード起因でないと記録した。

証拠の扱いでは、古い台帳を黙って正規化せず乖離として記録するのが正しい扱いである。SF-06 の先行 ledger は SF-03/SF-04 merge 前の stale な T2 NotRun 行を抱えていたため、新 coverage 文書を現行 tests/ 全件実測 green に基づく被覆の正本とし、重複追加せず stale 行を訂正した。

nearest-SHA CI 証拠は byte 同一範囲を限定して主張する条件付き採用である。dispatch-only CI で対象コード SHA の ci.yml run が 0 件のため、fixture binary 入力（store.rs+3 fixture+manifest+Cargo.lock）が同一の最近接 SHA run を採用し、対象 SHA 自体は NotRun pending として分離記録した。

task のチェックボックス完了は PR merge・issue close を独立に証明しない。front matter status・未チェックの Done When・IR の Validation 記録を突き合わせて「計画/実装済み/外部 gate 未実施」を区別する必要がある。実際に task front matter と IR 記録が一致しない文書が複数あり、「文書が一致しない」と記録してどちらかを正と断定しない扱いとした。

### 検証と evidence の運用規約

canonical 検証は mise run 経由で行う。対象は fmt-check・check・lint（clippy -D warnings）・test（cargo test --workspace --locked）・test-rt（rt_ filter）・test-msrv（rust 1.89）・build・check-app-native/lint-app-native/build-app-native・check-ui/test-ui・check-headless-deps である。補助の cargo test 単体実行は証拠として記録するが canonical 判定は mise lane とする。Cargo.lock は再現性のため追跡し、依存変更時は必要範囲で更新する。

headless pass を GUI・native plugin・device・RT・consumer 受入へ昇格させない。実 OS/GPUI lifecycle・実 2-window・実 VST3/AU/CLAP・実機 RT latency・AudioLens 統合・consumer cross-gate は外部 gate として NotRun/Deferred/Unsupported を明示する。

docs index は doc-maintenance script（doc_maintenance.js plan --type all → apply）で再生成し、slice ごとに docs chain（specs/designs/plans/tasks/test-specs/impl/ir/impl/exp）と各 README index を揃える。evidence は evidence/svNN/ 配下に被覆表・検証結果・外部 gate 判定を slice 単位で記録する。

dispatch-only CI 環境では対象 SHA の run が存在しないことがあるため、nearest-SHA 証拠は byte 同一範囲（対象 binary の入力ファイル集合を列挙）を限定して採用し、対象 SHA 自体は merge 後の covering run に委ねる。

fixture 設計の再利用パターンは、独立 oracle（本実装と別経路で期待値を計算）・exhaustive sweep（小さい state 空間の全列挙）・fault fixture（reject→state unchanged の原子性）・same-revision fixture（receipt/source-map/probe/view が同一 committed revision を指すことの横断検証）・lane 分離 ledger である。これらの検証骨格は SF-03〜SF-06 と StagedFile で共通のパターンとして機能した。

review-protocol で blocker 0 を確認してから commit/push/PR に進む流れが全 slice で共通であり、Conventional Commits で論理分割コミットし日本語 PR を作成する運用である。

## 適用対象

この知識は Synvelis のルーティング系サブシステム全体に影響する。synvelis-project は永続モデル・スキーマ v5→v6・ルーティングトランザクション・probe 契約型を所有し、synvelis-player は lowering・RoutingEngine・probe・PCM を担う。synvelis-workspace は session・Signal Flow read model・gesture・view state・probe 購読 lease を持ち、synvelis-app は composition root・SessionRoutingEngine・op 公開を担う。ui/ は TypeScript IPC と view model を担当する。永続化の下位基盤として Ruviform の StagedFile/PublishFailure と probe 時計契約（RebindReceipt・CapturePolicy・ProbeClockMark・ProbeTimeContext・ObservationCoverage）が使われる。

検証・受入工程では、mise run lane・evidence ledger・外部 gate の NotRun/Deferred 管理という運用規約が同じ知識体系に属する。

## 出処

SF-01〜SF-06（セマンティックルーティング・ルーティングトランザクション・lowering/activation・プローブ・Signal Flow・クロスゲート Oracle）と StagedFile/SFXR 系の永続化・検証に関する作業群からの抽出。出所 Issue は synvelis#84・#104・#105・#106・#107・#108・#109・#230、および ruviform#294・#305・#306・#308・#309 である。consumer 側の統合 gate は AudioLens#143 に委譲されている。
