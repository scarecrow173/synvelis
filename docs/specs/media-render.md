---
id: "SPEC-034X7uv8zUJ4bkVbH4LxYj"
type: "spec"
status: "approved"
title: "Media / Import / Export / Offline Render / UI / Diagnostics — Media & Render Lifecycle 設計契約 (synvelis#21)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/21"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
    - "docs/specs/x-recorded-verified-source-playback-export.md"
    - "docs/specs/issue-86-verified-import-wiring.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/recording-take-comping.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/workspace-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "https://github.com/scarecrow173/synvelis/issues/21"
  derived-by:
    - "docs/designs/media-render-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/media-render.md"
  references: []
  defers: []
  deferred-by: []
---

# Media / Import / Export / Offline Render / UI / Diagnostics — Media & Render Lifecycle 設計契約 (synvelis#21)

## What Should Be Built

Issue #21 の canonical 設計契約: Synvelis の DAW 意味論
（Media Capability/Identity/Provenance、Source/Asset/Project
Commit、Import Job、RenderSpec/Offline Render Recipe、
Master/Stem/Freeze/Bounce、Job Ownership、Inspector/Signal
Flow/Diagnostics の製品意味）と、Ruviform の再利用 generic
primitive（Generic Reader/Decoder/Encoder/Muxer、Offline
Driver、StagedFile、Probe/Dock/Clock/UI Host）の Owner 分離を
固定し、MR-RCP-01 と MR-001〜028 の 29 受入ケースを既存
Synvelis/Ruviform fixture・Issue・Receipt へ写像する。

上流 baseline は `docs/designs/
media-import-export-offline-render-ui-diagnostics-architecture.md`
（v0.6 Approved・14 項目合意済み）と `docs/designs/
synvelis-driven-ruviform-target-architecture-v07.md`
（v0.7.1）。本 slice は **docs/ 設計契約 + evidence 台帳のみ**。
実装は #23（Execution Plan）配下の各実装 Issue（#24/#25/#26 は
統合済み、#84/#86/#116/#118/#127/#128、RCP 系 #289〜#297、
asset-library 系 #9/#85〜#91）が担う。設計完了を実装・実
OS・実 Plugin・実機受入とみなさない。

## Why Now

- v0.6 baseline（2026-11-01 合意）は 14 項目の設計判断と
  MR-001〜028 マトリクスを持つが、canonical
  spec/design/plan/task/test-spec/impl の 7 文書鎖と
  evidence 台帳が未整備。
- E2E-L1（#24）・E2E-L2（#25）・E2E-X（#26）は CLOSED で
  headless の Local Import→Playback→WAV Export→Reopen が
  main に存在する。一方で Remote Import（#9/#85〜#91）、
  Stem/Freeze/Bounce、RCP 系 Render Checkpoint（#289〜#297）、
  Unified Job Orchestrator、実 Plugin/OS 受入は未完了であり、
  「何が実装済みで何が未実装か」を一枚の契約表に固定する
  必要がある。
- #127（TIM-03 process boundary）・#128（TIM-04 plugin state
  snapshot）・#116（RCD-02 recording durability）・#86
  （ALIB-02 verified import）は本 Issue の横断受入条件と
  直結しており、責務境界を Issue/Fixture レベルで照合する
  必要がある。

## Users and Value

- 実装者（#23 配下の各 Issue Owner）: Media/Import/Render/
  Export/Job/Diagnostics の判断基準と Owner 境界。
- Reviewer: 「設計合意」「headless fixture 成功」「製品受入」
  「実機受入」を区別できる evidence 語彙。
- Ruviform Consumer 調整: Generic I/O・Codec・Offline Driver・
  Probe・UI Host の責務と Synvelis 側 policy の分界。

## Scope

### In Scope

- Media Capability/Identity/Provenance（Container/Codec/Native
  PCM・Declared/Decoded/Verified/Inferred/Unavailable）。
- Media Resolver（SourceId→SourceVersion→AssetId/ContentRef→
  Verified Blob→Decoder/Reader→nonRT Prefetch→RT Engine Frame）。
- Asset Library 隔離（Metadata-first・Pinned Revision Copy・
  Catalog/Preview Cache ≠ Project Media）。
- Safe Import Job（Pinned Fetch→Staging→Verify→Durable
  Materialize→ExpectedRevision Project Commit・Cancel/Conflict/
  Orphan）。
- Unified non-RT Job Orchestrator と Window/View から独立した
  Job Ownership。
- Immutable RenderSpec/Render Snapshot と Preflight。
- Stateful-safe Offline Render（Sequential Stateful Graph・
  Preroll/PDC/Tail・並列独立解析との分離）。
- Real Output Pipeline（Format/SRC/Dither→Encode→Drain→Mux
  Finalize→Validate→Safe Publish→Receipt）。
- Master/Stem/External Export と Bounce/Freeze/Render-in-Place
  の分離。
- Bounded UI Binary Data Transfer と世代付き Projection。
- Diagnostics Evidence（Configured/Prepared/Applied/Observed/
  Inferred/Unavailable・Clock/Frame/Revision/Provenance/Loss）。
- Ruviform generic / Synvelis DAW policy の Owner 境界。
- Plugin Process precision capability（#127）と Plugin State
  品質 Receipt（#128）の本 Issue への写像。
- Verified Media / Recording Raw Verified Prefix / Probe
  Capture の分離（#116/#86）。
- MR-RCP-01 と MR-001〜028 の case 写像・Owner・Fixture・
  Revision・Pass/Fail/Unsupported/NotRun の台帳化。

### Out of Scope

- 実装コード（`crates/`、`Cargo.toml`、`Cargo.lock`、`mise.toml`、
  CI、Ruviform）。実装 Owner は #23 配下の各 Issue。
- GUI 実装・実 device/plugin・実 OS 受入（GUI-L2/EXT-L3）。
- Ruviform Generic Reader/Codec/StagedFile/Media Core の
  重複実装。
- Asset Library 文書（`docs/specs/alib-*.md` 等）の
  編集・作成 — #9 系は別 Worker が担当（参照のみ）。
- 設計だけによる実装・実 PCM・実 Plugin・実機 PASS の宣言。

## Requirements

- **MR-R-01 Media Capability/Identity/Provenance**: Container・
  Codec・Native PCM Layout（Interleaved/Planar・Sample Format・
  Channel Layout・Sample Rate）を別概念として管理し、
  Probe可能/Demux可能/Decode可能/Encode可能/Mux可能/Random
  Seek可能/Preview可能を分離する。`Declared`/`Decoded・
  Verified`/`Inferred`/`Unavailable` を区別し、未知値を確定値で
  埋めない。Native PCM/Original File を破壊せず、変換履歴
  （Import/Proxy/Render の SRC・Channel Map・Scale・Clipping/
  Dither）を保持する。
- **MR-R-02 Media Resolver**: `SourceId → Source Version →
  AssetId / ContentRef → Verified Blob → Decoder/Reader →
  nonRT Prefetch → RT Engine Frame` の解決経路を共有 Port
  とする。Blob 欠落は Missing、Decoded Frame を偽造して
  Verified と扱わない。Blob dedup は論理 ID/Provenance の
  統合を意味しない。Source/Asset 更新時に旧 Prefetch を新
  世代へ混入させない。
- **MR-R-03 Asset Library 隔離**: Catalog 正本は Provider
  Server、Cached Metadata は Observation。Metadata-first
  Page/Filter/Sort/Search で `IndexCoverage`/`IndexedAt`/
  `ProviderRevision`/`Offline・Stale・Unavailable`/
  `ResultCompleteness` を表示し、未検索 Repository の未検出を
  「存在しない」としない。Preview は明示要求で、Preview/Peak/
  Thumbnail Cache は Evict 可能で正式 Project Media と別。
  Pinned Revision の Copy into Project が既定。
- **MR-R-04 Safe Import**: `Pinned Fetch → Staging →
  Size/Hash/Format Verify → Durable Materialize →
  ExpectedRevision Project Commit` を別段階の非 RT Job とし、
  Blob Verify/耐久化と Project Controller Commit を分離する。
  Commit 失敗時の Verified Orphan は Recoverable。Cancel/Retry
  は段階ごとに意味を持ち、Commit 後の Undo は Project 変更で
  あって物理 Media 即削除ではない。未検証 Clip を Project 正本
  として先行公開しない（Pending Ghost/Job として明示）。
- **MR-R-05 Unified non-RT Job Orchestrator**: Import/Render/
  Bounce/Freeze/Analysis/Recovery/Catalog Preview の Owner・
  InputSnapshot・Progress/Phase・Cancel・Result/Receipt を
  統合する。`Queued → Preflight → Prepared → Running →
  Draining/Finalizing → Validating → Publishing → Completed`
  に `Cancelling`/`Cancelled`/`Failed`/`Recoverable`/`Degraded`
  を明示 outcome とする。Cancel 受理≠取消完了、Finalize/
  Publish は短い非中断相になりうる。Failed write は
  `Completed` receipt を出さない。Job Owner は Window/View の
  寿命から分離し、Window に紐付く短命 Preview と
  Application/Project Session に紐付く長時間 Job を混同
  しない。アプリ終了時の Cancel/Wait/Recoverable は明示
  Policy であり、バックグラウンド永続 Job を無条件保証
  しない。
- **MR-R-06 Immutable Render Snapshot**: `RenderSpec` が
  Project Snapshot Revision・TempoMap Revision・Asset/Source
  Versions・Plugin Instance/State Capture Evidence・
  Graph/Automation/Transport Configuration・Range Anchor・
  Channel/Output Format・Render Mode・Tail/Preroll/Latency
  Policy を固定する。**Project Live ≠ Saved ≠ Active Plan ≠
  Render Source Snapshot**。Render 中に Project を編集しても
  実行中 Spec を自動変更しない。欠落 Plugin/Asset・未対応
  Output Format・Render 不可能な External Sidechain/
  Realtime-only Plugin は Preflight で明示 Reject または明示
  Policy の Degraded。Source Missing の無断ゼロ埋め成功を
  既定で禁止し、明示 silence substitute は Receipt に記録
  する。
- **MR-R-07 Stateful-safe Offline**: Graph/Time/Event の評価
  意味論を Live と共用し、Plugin Instance は独立とする。
  `OfflineDriver::run_parallel` の結果一致は Chunk 独立が
  前提であり、Delay/Reverb/Compressor/Instrument 等の状態を
  持つ DSP に任意時間 Range の分割並列を無条件適用しない。
  初期実装は時間方向 Sequential Execution + 明示可能な
  独立単位に限った並列化。Stateful Graph の並列独立解析・
  cache 作業と stateful render を混同しない。
- **MR-R-08 Range/Preroll/PDC/Tail**: Render Range は
  Beat/Media/Clip-local Anchor を明示し非 RT で Event/Frame
  に変換、半開 `[start,end)` で同 Frame Event 二重評価なし。
  Lookbehind が必要な Plugin/Clip Stretch/Stateful
  Instrument は Preroll/Warm-up を準備し、先頭を無条件ゼロ
  状態で開始して正しい結果と断定しない。Graph PDC と
  Export 上の補償・波形 Leading Silence は分離し、Master/
  Stem の時間 Alignment は共通 Frame 原点と実 Output Tap に
  基づく。Tail は Fixed Time・Until Silence（閾値/保持時間/
  最大長）・User Defined・Unsupported/Unknown の Policy を
  持ち、無限 Tail は Max Duration/明示停止を要求。無断
  Trim・Length 不一致を成功扱いしない。Loop の「反復して
  書き出し」と「単一 Region のみ」を Render Spec で区別。
- **MR-R-09 Real Output Pipeline**: Format negotiation で
  Container/Codec/Sample Rate/Bit Depth/Channels/Metadata/
  Seekability を検証。PCM→整数 PCM は Clipping/Level・
  Rounding・Dither/Noise Shape・Channel Map・SRC・Endianness・
  Metadata を規則化し、Dither は最終量子化の単一段階のみ。
  L/R/多 Channel・Master/Control Room・Pre/Post Fader・
  Pre/Post FX の Export Tap を明示し、Monitor-only Plugin を
  暗黙混入させない。Encoder Drain→Muxer Finalize→Close/
  Sync→Output 検証→Path Publish を分離し、Staging/Temporary
  へ書いて検証後に Destination へ安全 Publish。同名
  Destination は Overwrite Policy と既存 File 保護/Backup を
  事前確認。Remote FS・異ボリューム・部分 Finalization は
  Capability/Recovery として管理。未完成出力を成功扱い
  しない。
- **MR-R-10 Export vs Project-derived**: Master/Stem/External
  Export は原則 Project 非変更。Bounce/Freeze/Render-in-Place
  は Verified Derived Media を明示 Project Command で Commit
  し、Original Source/Plugin/Automation/参照を保全する
  （非破壊 Lineage）。Freeze 中の Plugin/Parameter UI は
  「凍結状態」を明示し、変更時は Unfreeze/Re-freeze を要求。
  Verified Blob 完成と Project 参照 Commit 成功は別 receipt。
  Export ファイルは Project Asset ではなく、明示的な
  「Import rendered result」等があるときのみ Commit。
- **MR-R-11 Window/View/Job Lifetime**: Window Close・
  Inspector/Browser Selection・Workspace 変更・View 移動が
  Record/Export/Project Job を暗黙 Cancel/変更しない。
  現行 `UiRuntime` の CloseRequested→Job Cancel 要求を
  長時間 Job に適用しない設計境界を固定する。View/Inspection/
  Selection State と Project Command Target を分離し、
  Window Crash/Monitor Disconnect は Authoritative Status を
  変えず View を復元して Job へ再接続できる設計を目指す
  （アプリプロセス終了後の実 Job 継続は別 Capability）。
- **MR-R-12 Bounded UI Binary**: Waveform/Spectrum/Preview
  等の大容量データは有界 Pull/Range 配信とし、JSON Push しない。
  各 DataSource Key に Selection/Revision/Source
  Version/Graph Generation を含め、古いデータの誤表示を
  避ける。Render/Probe/Preview Job が UI 描画を block しない。
- **MR-R-13 Diagnostics Evidence**: Configured/Prepared/
  Applied/Observed/Inferred/Unavailable を区別し、Clock/
  Frame/Revision/Provenance/Loss を付す。Probe は
  Off（既定）/Monitor/Capture で、全論理 Node に能力宣言を
  持たせるが全 PCM を常時保存しない。存在しない Buffer や
  非公開 Plugin 内部を観測できると偽らない。Probe の
  Session/Controller は Inspector View 寿命と独立し、2 つの
  Inspector が同じ Probe を開いても Capture を重複生成
  しない。Signal 比較は同じ実行 Frame/Clock/Graph
  Generation と PDC/経路 Latency を考慮し、時間の揃わない
  測定値をそのまま引き算しない。Diagnostics Budget 超過は
  Capture Loss/Unavailable として可視化し、**録音 Writer の
  Fail-stop 要求を Probe にも、Probe の Drop 許容を録音にも
  適用しない**。ある点で初めて異常が観測されたことと原因
  Node 特定は別であり、原因候補は測定・コンパイル
  Evidence に基づき Confidence を示す。
- **MR-R-14 Owner 境界**: Media Codec/Container/Native PCM・
  Generic Reader/Prefetch/Encoder/Muxer/Offline Driver/
  Execution Graph/DSP・StagedFile/Filesystem・Generic
  nonRT Job/Cancellation/Bounded Data Transfer・Multi-
  window/Dock/Probe/Clock は Ruviform。Project AssetId/
  SourceId/Clip/Provenance/Retention・Catalog Provider/Remote
  Revision・Import Commit/Render Recipe/Job Orchestrator/
  Stem/Freeze/Bounce・Workspace/Inspector/Signal Flow の製品
  意味は Synvelis。既存 crate 構成の温存を目的化せず、
  generic runtime に不足があれば Synvelis 側で恒久的重複
  実装せず Ruviform Issue へ起票する。
- **MR-R-15 Plugin Process precision（#127）**: Realtime と
  Offline の Plugin Process min/max frames・Transport Event・
  Automation precision を Capability 別に
  `ExactWithinVerifiedCapability`/`FrameAccurateEvents`/
  `BlockBoundaryOnly`/`DegradedWithConsent`/
  `Unsupported`/`Unknown` で判定する（TIM-03 契約を正本
  として参照）。Offline Exact 要求時は Capability 不一致を
  `OfflineExact`/`OfflineUncertified` で fail-closed とし、
  consent で救済しない。実 Plugin の規格対応を実測保証と
  みなさず、RT degraded は `DegradedConsent` 明示同意のみ。
- **MR-R-16 Plugin State 品質 Receipt（#128）**: Plugin State
  Capture の `Coordinated`/`Quiescent`/`BestEffort`/
  `Unsupported` 品質、Project Snapshot/Asset/Plugin
  version/Clock/Graph runtime 間の対応証拠を Render Recipe
  へ固定する（TIM-04 契約を正本として参照）。`Save
  Completed`・`Plugin State Exact`・`RT Active` は別
  Receipt であり、`Standard` save は品質を可視化した条件
  付き保存を許容し `Strict`/Exact 要求は
  `BestEffort`/`Unsupported`/失敗を fail-closed で拒否する。
  条件付き保存を完璧な再現保証と偽らない。
- **MR-R-17 Verified Media / Recording Raw / Probe 分離
  （#116/#86）**: Raw Recording Writer/Journal の Verified
  Prefix（CRC32・durable frontier）と Asset SHA-256 CAS・
  Project Save・Recovery を別正本として区別する。Probe の
  Bounded/lossy capture を Lossless Raw/Offline Render
  output の Source-of-truth にしない。Finalized 以外は絶対に
  再生 source にならず、canonical publication は非 RT・
  finalize 後・commit 前の工程とする（X 契約を正本として
  参照）。
- **MR-R-18 Render Checkpoint 連携（MR-RCP-01）**: Snapshot
  pin 付き Offline Render output を RCP-02 independent
  readback で検証し、lossy probe・partial artifact・unknown
  source・Plugin state を Exact Cache に流用しない。既存
  Offline Render/Verified Output/Safe Publish は実 PCM 生成を
  Own、RCP-01 は DAW Graph からの canonical recipe、RCP-02
  は不変 Artifact Index/GC、RCP-03 は Trigger/Async Job を
  Own する。Source/Plugin State/Tempo/Port/Sidechain 全
  closure を固定し、Ready な Render でも Codec/Channel/
  Frame/Tail/Format/Hash/independent readback を満たさ
  なければ Exact に昇格させない。Plugin BestEffort State や
  非決定性では再生成同一性を保証しない。Job Completion は
  Save/VCS Commit/RT Active と独立。Manual/OnSave/OnCommit/
  OnShare は任意組合せで、Trigger だけの Policy 変更では
  音響 RecipeKey 不変。OnCommit は VCS Write 未実装なら
  Unsupported、Share の RequireComplete のみ公開操作を
  保留する。
- **MR-R-19 Evidence Honesty**: 各 MR ケースに実行 Owner・
  コマンド/Fixture・Revision・Pass/Fail/Unsupported/NotRun
  と検証証拠を付す。未実装・未実 OS 実測を完了と主張
  しない。headless Pass を実機/GUI/Plugin/OS 受入に繰り
  上げず、設計レビュー Pass と実測 Pass を区別する。
  「Pass(部分)」は headless で関連契約が確認できるが当該
  MR 全体の独立 oracle を満たさないことを示す。

## Acceptance Criteria

Issue #21 の受入条件を MR-AC-01〜10 として転記・対応付ける。
各 MR-001〜028/MR-RCP-01 の現在の被覆状況は
`evidence/sv21/media-render-case-ledger.md` を正本とし、本
spec は要求のみを定義する。

- [ ] **MR-AC-01** `MR-RCP-01`: Snapshot pin 付き Offline
  Render output を RCP-02 independent readback で検証し、
  lossy probe/partial artifact/unknown source や Plugin
  state を Exact Cache に流用しない設計・写像が存在する。
- [ ] **MR-AC-02** MR-001〜004/011〜014/018〜020（MR-A）:
  Local Audio Import→Verified Blob→Clip Playback→実 Master
  PCM WAV Export→Reopen が、入力／出力の PCM・Sample
  Frame・Provenance・独立 Decode で検証される設計・写像が
  存在する。
- [ ] **MR-AC-03** MR-005〜010（MR-B）: Remote Asset を
  メタデータ優先で検索・選択取得し、未選択 Asset の全量
  同期を回避する。Pinned Import/Cancel/Stale/Conflict/
  Permission の各状態を区別する設計・写像が存在する。
- [ ] **MR-AC-04** MR-015〜022（MR-C）: Immutable
  RenderSpec から Sequential Stateful Graph を用いて
  Preroll/PDC/Tail→Master/Stem/Freeze/Bounce→Finalize/
  検証・Commit までを再現し、並列独立解析と stateful
  render を混同しない設計・写像が存在する。
- [ ] **MR-AC-05** MR-023〜028（MR-D）: Window Close と
  Application/Project Job 所有を分離し、Dock/Inspector・
  Probe 状態・2 地点比較・Provenance・安全な UI Data
  Transfer を検証する設計・写像が存在する。
- [ ] **MR-AC-06** Realtime と Offline の Plugin Process
  precision/Transport Event の能力を Exact/Degraded/
  Unsupported/Unknown 等で区別し、Offline Exact 要求を
  黙って近似しない（#127）— TIM-03 契約への写像が存在
  する。
- [ ] **MR-AC-07** Plugin State の保存品質と Project Save
  Completed/RT Active を別の Receipt で記録し、Render
  Recipe が対応 Revision/Plugin Version/Clock/Graph 状態
  を追跡できる（#128）— TIM-04 契約への写像が存在する。
- [ ] **MR-AC-08** Raw Recording Writer/Journal の
  Verified Prefix と Asset SHA-256 CAS/Project Save/
  Recovery を区別し、lossy Probe Capture を lossless
  Raw/Offline 出力として扱わない（#116/#86）— VS-C/X/
  ALIB-02 契約への写像が存在する。
- [ ] **MR-AC-09** Media/IO の共通処理は Ruviform、DAW
  特有の Source/Render/Job/Commit 意味論は Synvelis の
  Owner であることを API/Fixture/Issue 関係で確認する
  設計・写像が存在する。
- [ ] **MR-AC-10** 各 MR ケースに実行 Owner・コマンド/
  Fixture・Revision・Pass/Fail/Unsupported/NotRun と
  検証証拠を付し、未実装・未実 OS 実測を完了と主張
  しない — `evidence/sv21/` 台帳が存在する。

## Deferred Design Concerns

- 実 PCM WAV Writer Backend の対応範囲（RIFF/RF64 等）・
  FLAC/MP3 等の Encode 優先度・許容 Format/Bit Depth/
  Metadata と実機 I/O | v0.6 §15 未確定 | Ruviform #191 系・
  実装 Issue | Backend 未実装を成功と誤読する。
- Tail Silence 検出の数値閾値・最大長・Plugin Warm-up/
  Offline Native support・Stem isolation 既定 Policy |
  未確定 | 実装 Issue + PoC | 無断 Trim・Length 不一致を
  成功扱いする誤り。
- Project Render Snapshot と Plugin State Capture の厳密
  同期/回復手順・Output overwrite durability・File/Network
  FS capability | 未確定 | #128 系・Ruviform I/O | BestEffort
  State の再現性を過大表示する誤り。
- Browser 各 Provider の具体 API・認証 UI・Security・
  Indexing・Remote Preview byte budget・Cache eviction |
  #9/#85〜#91 | asset-library 系 Issue | Preview Cache を
  Project 正本と混同する誤り。
- Window Close/App Exit 時の Job Survival・再接続・UI
  WebView/Native GPU backend・Probe budget/Diagnostics
  payload | 未確定 | #114・ruviform#181/#182/#185・#294 | Window
  Close で長時間 Job を暗黙停止する誤り。
- Performance Budget/Resource Admission・Audio RT vs
  nonRT Import/Render 同時実行の OS 別検証 | 未確定 |
  実機 lane | 実測なしに SLO を主張する誤り。
- 具体的 Rust API/Crate placement と Ruviform 既存 ADR/
  Consumer の移行計画 | v0.7.1 §10 未確定 | #23・各実装
  Issue | 設計契約を実装済み API と誤認する誤り。

## Sources and Relations

- Issue #21 本文（14 項目合意・MR-001〜028・MR-RCP-01・
  受入条件・Owner 境界・検証方法）。
- `docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md`
  （v0.6 Approved baseline — 14 判断・MR matrix・failure
  matrix・責務 matrix の上流正本）。
- `docs/designs/synvelis-driven-ruviform-target-architecture-v07.md`
  （v0.7.1 — Ruviform 汎用能力導出・M0〜M7・TA-001〜012）。
- `docs/designs/project-asset-persistence-architecture.md`
  （#17 — Project/Asset/Persistence の永続化契約）。
- 確定済み縦断契約: `docs/specs/l1-*`（#24）・`l2-*`（#25）・
  `x-*`（#26）・`issue-86-*`（#86）・`alib-*`（#9/#85〜#91）・
  `vs-c-*`/`rcd-02-*`/`recording-take-comping`（#12/#116/#118）・
  `tim03-*`/`tim-02-*`/`tim-04-*`（#127/#128/#20）・
  `playback-graph`（#18）・`project-persistence`（#17）・
  `workspace-model`（#2）・`sv-gui-04-*`・`sf-04-*`（#107 系
  probe）・`syn-obs-01-*`・`syn-rf305-*`（#82/#84）。
- RCP 系: synvelis #289（RCP-00 設計正本）〜#297（RCP-F01）、
  `PROJ-RCP-01`（`docs/specs/project-persistence.md`）。
- Ruviform 依存: rf#191（WAV Backend・closed）・rf#197
  （Reader・closed）・rf#198（Sequential Offline・closed）・
  rf#181/#182/#184/#185/#186/#187/#188/#189/#190/#196/#282/
  #308/#309（closed）・rf#253/#275/#294（open）。
- 関連 Issue: synvelis #2/#4/#5/#6/#8/#9/#10/#11/#12/#17/#18/
  #19/#20/#21/#22/#23/#24/#25/#26/#82/#84/#85〜#91/#92〜#97/
  #104〜#120/#127/#128/#145〜#167/#176/#184/#219/#230/#251/
  #253/#277〜#297/#307/#308/#309/#313/#314/#330/#365/#368。
