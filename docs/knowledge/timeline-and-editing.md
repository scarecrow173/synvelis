---
id: "KNOW-0UmMMhCVewYIfXkWxVv3WK"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "architecture"
title: "タイムラインと編集モデル"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/4"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/18"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/38"
    - "https://github.com/scarecrow173/synvelis/issues/42"
    - "https://github.com/scarecrow173/synvelis/issues/50"
    - "https://github.com/scarecrow173/synvelis/issues/52"
    - "https://github.com/scarecrow173/synvelis/issues/65"
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/128"
  informs: []
  explains: []
  supersedes: []
---

# タイムラインと編集モデル

## 概要

Synvelis のタイムライン編集・時間系・再生/配送系サブシステムの設計契約と実装で得られた知見をまとめる。対象は、設計契約の docs-only チェーン（Timeline Editing synvelis#11、Take Lane/Comp Editor synvelis#12 §14/§15、Signal Flow synvelis#4、Playback Graph synvelis#18、Musical Time synvelis#19、Transport Integration synvelis#20）、編集モデルの headless 実装（ED-A synvelis#38、ED-B synvelis#65、MD-A/MD-B synvelis#42/#50、MIDI Source synvelis#52）、時間・PPQN・SMF 実装（TIM-01 synvelis#125、TIM-02 synvelis#126）、process boundary・transport 配送・plugin state 実装（TIM-03 synvelis#127、TIM-04 synvelis#128）である。時間ドメインの型分離、有理 beat の正本化、gesture の fixation/preview 分離、fail-closed と honest reject の一貫した適用が、この領域を貫く設計原則である。

## 詳細

### 時間ドメインの分離と有理 beat 正本

Musical Tick、MIDI/SMF ネイティブ tick、有理 beat 位置、media time、device clock、engine frame、recording session frame、plugin frame、transport/plan 世代は、すべて別 newtype・別契約で扱い、暗黙変換を禁止する。ドメインを跨ぐ変換は Ruviform の `TickPosition::to_beat`/`from_beat`/`Rounding` のような明示的な checked API のみ経由する（synvelis#19）。

時間計算の正本は exact rational beat（`BeatPosition`）であり、integer tick のスケール係数乗算で代用しない。PPQN 変更時は musical site を beat 経由で厳密に再投影し、source-native tick・media・device・plugin frame・非永続 compile 出力は再投影対象から除外する（synvelis#125）。代替として「tick を単純に係数スケールする」方式は精度劣化と source-native 領域の破壊を招くため棄却され、有理 beat 正本化は TIM-01 全 slice の前提となった。`PpqnConversionPolicy` は `Exact` と明示 `Round` のみを持ち、暗黙の量子化・丸めは存在せず、inexact/overflow は reject する。

Signal Flow（synvelis#4）では、Issue の概念語彙と実装語彙が食い違う場合に実装側の語彙を正本とする方針を採用した（例: `Unbound`/`Partial` の概念語ではなく `PartiallyRebound`/`StaleGeneration`/`Stopped`）。Playback Graph（synvelis#18）では「headless で確認できたが要求された全ケースの独立 oracle がない」状態を `Pass (partial)` と表記し、独立 oracle を持つ完全 Pass と区別する。契約記述のみの項目は「設計記録」として test Pass にしない。

### ED-A 編集モデル：Selection・Gesture・Snap・Tool

synvelis#38 の ED-A は編集モデルを headless で固定した。Selection は Object / Time / Lane / Primary / Inspection / Focus / Linked の 7 概念に分離し、単一の `editing_selection` フィールドを廃止した。ObjectSelections は domain（Clip/Track 等）別 map + active domain で構成し、混在 selection を型で拒否する。

Gesture は `GestureFixation`（開始時に確定・不変）と `GesturePreview`（mutable）に分離する。frame ごとの Project 書込み、自動 rebase、target substitution はすべて禁止し、commit は `ProjectOp::Composite` 1 回で行って 1 gesture = 1 undo unit を保証する。cancel は preview を破棄するだけで Project は不変である。

Snap と Grid は分離概念である。`SnapUnit::Explicit`（例: 1/4 note）は zoom 非追従の明示 tick 値、`Auto` は文脈（zoom 等）依存であり、`GridConfig` は表示用で Snap とは独立する。Tool 解決は Smart 優先順 Handle > Body > LaneEmpty > Background で、同順位複数ヒットは `Ambiguous` を返して暗黙のタイブレークをしない。dedicated tool は固定解決である。

意図的な reject として、semantic `SplitClip` の source-offset 近似計算は `UnsupportedOperation` を返す（誤った近似より拒否）。Time Selection の暗黙 Ripple/delete は行わず、range 削除は明示 command `clips.delete-range` のみとする。Undo 配送は View→session+配信、Text/Ime→text_undos stub、Modal/PluginGui→Suppressed に分類し、View/Window Destroyed 時に in-flight gesture を cancel する hook を持つ。Workspace schema は 1→2 に bump し、schema 1 snapshot は読まない。

### media-domain 編集と comp の effective lowering

synvelis#65 の ED-B は gesture 位置を domain-tagged `EditTimePoint`（Musical / Absolute(media)）に一般化し、HitProbe・gesture origin・preview position・drag_update をこの型で統一した。probe domain と fixation domain の不一致は `pointer_down` 開始時に `MixedTimeDomains` で拒否し、途中で気づかない構造にした。混在 domain（Musical clip × Absolute selection 等）の結果は設計の結果表どおりに定義し、暗黙の domain 変換をしない。

media overlap 判定は anchor base への floor ではなく exact rational 比較を使う。`EndTrim` の snap は常に「動く端」基準（anchor start ではない）で、`media_rescale` は `Option` を返して silent saturation を避ける。media trim は playback `source_region` を変更しない（content-fixed trim は別 composite operation が必要）。ED-A API の EditTimePoint 化に伴い ED-A の既存 test も API 追従で更新したが、project crate と workspace schema は変更なし（`SnapUnit`/workspace schema 不変のため migration 不要）だった。

synvelis#50 の MD-B（MD-A synvelis#42 が前提）は、Comp の「effective（実効）投影」を player/render lowering が消費できる unified playback node model にする。node 種別は `Clip`/`CompSegment`/`CompOverlap` で、`SupersededByComp` により double-play を防止する。fade の domain は「side 自身の overhang span」で定義し、他 side の span を借用しない。lowering の skip はすべて explicit typed（silent skip 禁止）で、Loop/SimpleStretch 等の unsupported mapping は honest reject とする。load-time 再検証は `Project::validate_references` に集約し、`CompOp` は session dispatch 経由で適用する。旧 snapshot（v1/v2）の migration は regeneration tool を作らず honest reject（pre-release 方針）とした。

### MIDI Source と SMF provenance

synvelis#52 は project 所有の `MidiSource`/`MidiNote` に stable ID を持たせ、Linked Source 編集（複数 clip が同一 source を共有）を実現した。明示的 `DetachClipSource` op（fresh SourceId+NoteIds の deep copy）で detach し、暗黙 detach はしない。SMF-backed source の note 編集は全 op を拒否し（Detach のみ許可）、SMF provenance を壊さない。source/clip の extent・mapping は `Musical` extent + `Natural` mapping に制限し、unsupported 組合せは reject する。persistent schema は v4→v5 に bump し、note map key と id の一致、source-ID namespace の非交差、ID watermark の load 時検証を review で追加した。

synvelis#126 の TIM-02 は SMF Format 0/1 の deterministic parse/serialize・provenance 永続化（`Project.smf_documents`）・typed error・export + `ConversionReport` を `synvelis-project` に実装した。schema は v8→v9 で migration は identity step。`SmfDocument` は serde `deny_unknown_fields` 規約に従い、将来 field 追加時は schema 前進が必要とする。`paired_notes` の unclosed note は厳格 error とし、note 編集 op は SMF document に対して immutable guard、`RemoveMidiSource` は document 連動削除とする。`compile_midi_comp` は channel-aware chase に拡張され、`MidiControllerEvent.channel` を追加して chase キーは `(channel, kind)` となった。Export→Reimport golden で byte 一致を固定し、PPQN mismatch 変換は `ConversionReport` で報告する。

PPQN×SMF cross-golden fixture（`tim_02_ppqn_smf.rs`）は `src/` 無変更で完結する設計で、SMF-backed source を持つ project に `SetProjectPpqn`（Exact 960→480 / Round 960→1000）を適用しても source-native provenance・export@480 golden・re-import doc equality が保たれることを証明する。`SetProjectPpqn` が `smf_documents`/`midi_sources` に触れる不具合があれば fixture が検出する構造であり、実際には発生しなかった。

### PPQN 変更の command・preview・UI

synvelis#125 の TIM-01 は `SetProjectPpqn` で旧固定 960 stub を置換した。範囲は `1..=32767` で、`expected_revision` + `old_ppqn` の二重 CAS、open-recording 拒否、scratch clone による atomic validation を持つ。`reproject_ppqn_with` が site 列挙を一元化し、preview と mutating command が同一 walk を共有するため preview/実行の乖離を構造的に排除する。site audit で `monitoring.punch.punch_in`/`punch_out` の欠落を発見し `PpqnSite::single` として追加した（PR #385 で silent retime を修復）。

PPQN 自体は schema bump 不要（値域拡張のみ）と判断されたが、punch-mark 残件 slice では v5 snapshot migration fixture を使用した。schema version の記述は slice ごとに時点差・対象 crate 差（project vs workspace）があり、単一の「現在値」として統一すべきではない。`preview_ppqn` は非破壊 dry-run で `syn.project.preview_ppqn` op として公開し、`syn.project.new` の optional `ppqn` param も追加した（省略時 960、範囲外は project 層に委譲して reject）。

app 層 `syn.project.set_ppqn` は wire params を `SetProjectPpqn` に写像し、transport-stopped / open recording transaction / automation record_enabled の 3 前提条件を fail-closed で検査する。判定順は transport → open transaction → record_enabled（record_start が record_enabled を要求するため）で、全失敗は `invalid_params` に写像する。

UI slice では view 所有 `PpqnUi` read model を追加し、select→preview→confirm/cancel を view-local 状態機械（Idle→Selected→Previewed）で完結させる。confirm は preview 時 revision CAS のみ mint し、cancel は非破壊である。Snap/Grid `Explicit` 量子は PPQN 変更で tick 数を verbatim 維持する「tick 保存」ポリシーを採用し（beat 間隔のみ変化）、rebinding 案は lossy/persisted preset 所有権のため棄却した。`project_info.ppqn` を投影し、undo/redo・project new/open・外部 session 変化で pending gesture を `observe_current`/`committed` で再同期する。

### player MIDI 配送と process boundary

TIM-02 の player MIDI RT/Offline slice は `compile_midi_events`（channel 解決・`AllNotesOff`→CC123 展開）+ `ParamDispatch::dispatch_midi` + driver transport method（`set_loop`/`clear_loop`/`panic`/`resume_attached_transport`）を実装した。`dispatch_midi` は Play を要求しない（transport anchoring は driver が所有）。panic は `TransportCommand::Stop` + `resume_attached_transport` の 2 段で、panic-and-continue は upstream 未対応のため採用しなかった。リスク対応として、note channel zip の長さ不一致時は全 note fallback channel、`AllNotesOff` が plan 末尾でない手作り plan では note channel 集合を全走査してから展開する二段構成、panic 後 resume の position 乖離は `resume_attached_transport` が set_clock+Seek+Play で再 anchor、`install_plan` 世代衝突は `next_generation` を `dispatch()` と共有して一意採番とした。

TIM-03（synvelis#127）は `process_boundary.rs` に consumer 側 policy oracle を実装した。`ProcessCapability::evaluate` は offset 正規化 → segment 列挙 → legality → exact/degraded/offline/consent/deferral の順で判定し、degraded scan は upstream `find_legal_boundary` を mirror する。upstream の degraded wrap（undeliverable span + wrap 継続）に対し oracle は fail-closed（`DeferralExceeded`）を選択する設計判断を明記した。degenerate contract（`max_frames==0`）は `BlockContract::legal_span_len` の除算 panic を避けるため oracle が `Unsupported` で事前拒否する。

task2 で `AttachDelivery` 投影 + `BoundaryDeclaration`/`register_boundary` + `connect` 配線を行い、capability/precision/consent/budget → `AttachConfig::{policy,budget}` を instantiate 前に fail-closed 評価する。OfflineExact は consent を無視して certified exact-class を必須化し、未対応 variant は `RoutingFault` に決定的写像（`PluginUnavailable`/`OfflineCapabilityMismatch`）とする。宣言なし processor は従来どおり Exact + default budget で後方互換を保つ。

task3 で `ProcessBoundarySession`（per-request orchestration）+ `PrecisionProvenance`（StablePluginInstanceId + NodeId + capability generation + graph generation の 4 軸 pin）+ `BoundarySessionError` を実装した。transport/plan generation は upstream scheduler 所有の event-scoped 軸のため複製しない。request API は全て provenance 照合を staging/install の前に実行し、stale/mismatch は typed refusal で何も stage しない（誤 Param/MIDI 配送 0）。node 1 plan last-writer-wins 制約は doc comment に明記した。

session→driver/routing wiring では `BoundarySessionSpec`（capability/mode/consent/budget の Copy spec）を追加した。`ProcessorConnection` は render が exhaustive destructure するため field 追加不可であり、`ProcessorControl` + generation-retained state 経由で vend する。`TransportMirror` は scheduler の apply/do_wrap 則を投影し（command 成功時のみ fold、receipt で generation 補正）、out-of-band `control()` を見ない既知 limitation を文書化した。session identity は binding session と同一 mint とする。

loop boundary の自動導出は、`TransportMirror` の commanded state から loop wrap `BoundaryPoint` を再構成する案を棄却し、scheduler 等価の maintained scalar `WrapSim`（status・anchor frame/position・loop ticks・frame base）を採用した。mid-window `SetLoop`/`ClearLoop` を跨ぐと mirror 読み出しは `pos_at` 乖離するためである。`WrapSim::walk_window` は clone preview で read-only、advance は `observe_node_block` の pending 適用列で 1 窓 1 回とする。`position_base == frame_base`（1 tick == 1 engine frame）の frame-linear domain 前提、tempo map は musical 情報専用で wrap frame を動かさない不変条件を明記した。driver 側では node cursor を driver cursor と独立した `sim_window` ドメインで追跡し（seek quiesce で node cursor が driver cursor を最大 2 block 先行しうる）、preview は clone のみ、advance は `observe_attached_block` の 1 箇所に限定して二重計上を防ぐ。`staged_transport` は「queue に入ったものだけ」push し、offset 0 は head boundary で applied interior cuts から正規化除外される既存 oracle 規則に委ねて driver は `BoundaryPoint` をそのまま供給する。

### plugin state snapshot

TIM-04（synvelis#128）は capture 品質を `StateCaptureQuality`（Coordinated/Quiescent/BestEffort/Unsupported）で分類し、`exact_eligible` は Coordinated/Quiescent のみとする。Standard quality は degraded を記録して publish、Strict は store 接触前に fail-closed とする。store は content-addressed（`blobs/<digest>` + HEAD pointer）で、manifest publish の `Indeterminate` は HEAD readback + digest 照合で reconcile する（`ProjectStore` の effect 分類先例に倣う）。restore は blob SHA-256 を port 呼出前に検証し、copy-restore は品質/digest を維持する。`PluginStateReceipt` は File Save / Plugin State / RT Active を混同しない別証拠として記録する。`PluginStatePort` は `&mut` で capture/restore し、player adapter は `Rc<RefCell<..>>` 経由で non-RT のみ（RT callback から呼ばない契約）。`state_handle` の copy vend は generation counter で stale 拒否（既存機構に依存）。render 側は `PluginStateRecipe` offline gate を追加し、Strict + 非 exact evidence を preflight で拒否する。receipt は evidence を偽らない。

### docs-only 設計契約チェーンの共通運用

設計契約 slice（synvelis#11/#12/#4/#18/#19/#20）は `spec → design → plan → task → test-spec → impl/ir + impl/exp` の 7 文書 + `evidence/sv<N>/` 台帳で構成し、scaffold ツールで生成して `doc-maintenance` の `index apply` で index を再生成する（index の手編集は禁止）。受入条件は一意 ID（TE-01〜14、EX-001〜020、TT-001〜030、SV19-C01〜C08、SFD-R1〜R8、TL-AC-01〜12 等）で test-spec case・oracle・状態に写像し、evidence ledger で headless Pass / design-review Pass / 実機 Pass を必ず区別する。検証 lane は `doc-maintenance` の lint/audit、`audit_impl_record.js`、`audit_experiment_log.js`、`route_graph.js --focus <name> --json`、`review-protocol`（blocker 0）で、`mise run fmt-check` は docs-only でも必須とする。generator gate として plan は approved design を、task は approved/in-progress plan を、test-spec は解決可能な verifies relation を requires とし、artifact ID は `SPEC-`/`DESIGN-`/`PLAN-`/`TASK-`/`TSPEC-`/`IMPL-` + 22 文字 Base62（crypto random）とする。

graph router は pre-existing broken relation により `status: blocked`/`current/next: probe`/`condition: blocked`/`focus-required` を返すことが多い。これは slice 範囲外の既存問題であり、各 slice は無関係な relation を修復せず「blocked baseline finding」として evidence に記録する運用を採用した。

owner 分離では synvelis-* → ruviform-* の一方向依存を守り、DAW 固有型を generic runtime に逆流させない。Ruviform generic の不足は Synvelis 側で恒久重複実装せず upstream Issue（ruviform#184/#188/#189/#190/#196/#243/#253/#294/#307/#308/#309 等）へ起票する。docs-only slice は実装を行わず下流 Owner（synvelis#104〜109 playback/signal-flow、#112/#114 GUI/実機受入、#125〜128 TIM 実装、#129/#130 SMF SMPTE/MIDI2 deferred）へ契約を供給し、「実装 Issue への委譲」を plan の実装 Owner 写像表で明示する。headless 証拠を実機/GUI/plugin/OS 受入に繰り上げず、capability 別に Pass/Fail/NotRun/Unsupported/Deferred を強制分類し、`Pass (partial)` は独立 oracle を満たさない場合の誠実な表記とする。

review は cooperative（Reviewer A）+ adversarial（Reviewer B）+ Validator + Judge で行い、subagent 不使用時は分離 pass で実施する。Confirmed P0/P1 blocker を残さないことが完了条件である。canonical 検証は `mise run <task>` 経由（`fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`/`check-app-native`/`lint-app-native`/`build`/`check-headless-deps`）で、cargo 直接実行は補助証拠とする。Rust toolchain は mise 管理（1.99.0 pin / MSRV 1.89.0）。`origin/main` 取り込み後は merge → index 再生成 → 全 lane 再検証 → Conventional Commit → push → PR 作成が定型フローである。file ownership/lease の例として `synvelis-workspace/src/lib.rs` は sv107 lease のため `#[path]` 暫定 mount で coordinator 統合に handoff し、`ProcessorConnection` は render が exhaustive destructure するため `ProcessorControl` + `ProcessorHost::boundary_attach` 経路で拡張した。TDD 運用として fixture を先に書いて RED 確認後に実装 GREEN とする（TIM-01 app slice で `unknown_op` 11 件 RED を確認してから実装）。

### 検証手法のパターン

headless fixture の定石として、`FakeWindowHost` + semantic `HitProbe` 経路で GUI なしに pointer/gesture/selection を検証し（ED-A/ED-B 共通）、`ProjectController` + `CounterIdGenerator` + `InMemoryStore` で CAS/Undo/Save/Reload/migration を headless で固定する。`VerifiedOfflineNode` pattern を踏襲して RT/Offline parity を検証し（TIM-02）、`StatsAlloc` window で RT no-alloc を検証する（`rt_process_boundary_hot_path_alloc_free`: 64 callback で allocations=deallocations=0）。実 `PluginNode`/`GraphRuntime`/`ParamDispatch` fixture で 3 層（evaluator/scheduler/node）を検証し（TIM-03）、fake `PluginHost` + atomic instance counter で preflight 拒否時に plugin 未 instantiate を証明する。`SmfDocument` byte builder（vlq/ev/meta/tempo/timesig/trackname/eot/note_on/note_off/cc/mtrk/smf_file）で SMF fixture を構築し、Export→Reimport golden（byte 一致）+ `SmfDocument` `PartialEq` doc equality で provenance 保持を二重に固定する。

契約写像・台帳パターンとして、Issue 受入条件 → test-spec case → fixture/oracle → Pass/Fail/NotRun/Unsupported/Deferred の一意 ID 写像を evidence ledger に記録する。Owner 分離表（Synvelis DAW 意味論 vs Ruviform generic）を plan に固定し、どの実装 Owner がどの契約を消費するかを明示する。「実装は plan の File Map を参照、docs slice は実装に依存しない（設計契約は実装前に固定する）」という順序を明記する。

fail-closed / honest reject パターンとして、Offline Exact は fail-closed で、Realtime degraded は consent + capability 判定 + receipt 記録が必要（consent は Offline Exact 違反を救済しない、synvelis#20）。`InBlockTransportSupport` は capability が明示 `Verified` の場合のみ in-block 対応とし、API 存在だけでは sample accuracy を保証しない。plugin event offset は各物理 process 呼出し開始を起点とし（`frame_offset = event_engine_frame − process_start`）、logical transport span と physical plugin process call を分離する。stale provenance/mismatch は staging/install 前に typed refusal（`BoundarySessionError`）とし誤配送 0 を fixture で固定する。unsupported mapping/extent/operation は silent skip ではなく explicit typed reject（`UnsupportedOperation`、`SmfError::Unsupported`、`MixedTimeDomains` 等）とし、`media_rescale` は `Option` 返却で silent saturation を回避する。

状態機械・世代管理パターンとして、`PpqnUi` は Idle→Selected→Previewed の view-local 状態機械で confirm は preview 時 revision CAS のみ mint する。`ProcessBoundarySession` は open 時に `AttachClockContext::of` で verified clock context を取得し `attach_delivery` を再評価し、`plan_window` は純粋（staging なし）、`stage_window`/`stage_plan` は evaluate 成功後にのみ push/install、`dispatch_params`/`dispatch_midi` は provenance + node pin + `check_frame_base` を通してから `ParamDispatch` へ委譲する。`TransportMirror` の generation 補正源は `ParamDispatch::observe_cycle` の receipts（`AppliedReceipt::{transport_generation, plan_generation, clock_epoch}`）である。`NodeSurface.wrap_sim` は vend 時初期化、`observe_node_block` が pending 適用列で walk advance し、`boundary_loop_wraps` は surface/provenance ゲート + wrap_sim clone + scheduled/staged merge + walk_window で構成する。

schema/migration パターンとして、`SCHEMA_VERSION` bump は永続化 shape 変更時のみとし、PPQN 値域拡張（1..=32767）は bump なし、SMF `smf_documents` 追加は v8→v9、MIDI Source は v4→v5、ED-A workspace は 1→2（旧 schema 1 は読まない）。migration は identity step または明示的変換で、旧 snapshot の regeneration tool は作らず honest reject とする。`SmfDocument` は `deny_unknown_fields` で将来 field 追加時に schema 前進を強制し、`state_handle` の copy vend は generation counter で stale 拒否（既存機構に依存、重複実装しない）とする。

### 意図的な非目標と deferred

GUI 実装・実機/実 OS/実 device/実 plugin 受入は全 slice で NotRun/Deferred であり、headless 証拠を実機受入に繰り上げない。SMF SMPTE/Format 2（synvelis#129）、MIDI2/MPE（synvelis#130）、実 MIDI device backend（synvelis#253/ruviform#253）は deferred。「1/7 beat」専用 fixture・その他未構成 fixture の新設は Ruviform#189/TIM Owner の別 gate とする。

Timeline Editing（synvelis#11）は docs-only で実装は synvelis#112/#114・#10/#12 系・#9 import 系が担当し、Ruviform GPUI Driver・共通 Pointer/HitTest/Viewport/Gesture の独自再実装は禁止、Pointer/Selection の暗黙 Project/Graph Commit 化も禁止とする。数値プリセット（fade 既定・hit-test px 閾値・Snap Hysteresis）は #11 §9 PoC 後続決定、OS 別 Keyboard/Modifier/Touch 既定は §14.8 未確定事項である。Take Lane/Comp Editor（synvelis#12 §14/§15）も docs-only で実 GUI 実装は #112/#114 系、第二 undo/preview/audition authority を作らず upstream canonical contract を参照し、Creative Branch は #278/#279/#288、MIDI note-level comp/chase/serialization は §15.6〜§15.9 が担当する。Signal Flow（synvelis#4）は docs-only 設計正本で実装 Owner は synvelis#104〜109、legacy migration は empty `serde(default)` map では完了せず明示的移行が必要である。Playback Graph（synvelis#18）は docs-only で実装は synvelis#104〜109、#114 は GPUI 実 GUI、ruviform#294 は両 consumer 独立受入。Musical Time（synvelis#19）は docs/evidence-only で実装は TIM-01〜04（#125〜128）と Ruviform#189 に委譲。Transport Integration（synvelis#20）は docs-only で実装は #125〜128・#129/#130・Ruviform#190/#188/#186/#189/#307/#196 が担当する。

実装 slice の gap として、ED-A の Slip/Stretch/Fade commit 意味論、mixed-domain arithmetic、Magnetic mode、pixel/device 挙動、`clips.delete-range` の ripple は gap として記録された。ED-B の source-length validation は source store が実 duration を持たないため制限され、GUI 挙動は deferred。MIDI Source の一部 pre-existing validation gap は review で記録された。TIM-03 の `TimingPrecision` certified class 認定 provenance は未定義（mock では constructor 注入）で、`ProcessBoundarySession` の driver/routing_control 実 dispatch 経路への接続は後続 slice とする。TIM-04 の実 plugin/OS/GUI は対象外である。

## 適用対象

この知識は Synvelis のタイムライン編集・時間系・再生/配送系サブシステム全体に適用される。具体的には `synvelis-project`（編集モデル・PPQN 再投影・SMF・comp lowering・plugin state store）、`synvelis-app`（`syn.*` dispatch・`PpqnUi`・session 配線）、`synvelis-render`/`synvelis-player`（comp PCM・plugin state・process boundary session）、および headless 検証 fixture 層にまたがる。設計契約の正本は synvelis#11/#12/#4/#18/#19/#20 の docs-only チェーンが担い、実装 Owner は synvelis#104〜109・#112/#114・#125〜128・#129/#130 と Ruviform 側 Issue に分散する。外部 gate（実 GUI・実 device・実 OS・実 plugin 受入、SMF SMPTE/MIDI2、Creative Branch）は NotRun/Deferred として区別して扱う。

## 出処

synvelis#11（Timeline Editing 設計契約）、synvelis#12（Take Lane/Comp Editor §14/§15）、synvelis#4（Signal Flow 設計正本）、synvelis#18（Playback Graph）、synvelis#19（Musical Time）、synvelis#20（Transport Integration）、synvelis#38（ED-A 編集モデル headless 実装）、synvelis#42（MD-A）/synvelis#50（MD-B effective comp lowering）、synvelis#52（MIDI Source 編集）、synvelis#65（ED-B media-domain 編集）、synvelis#125（TIM-01 Configurable PPQN）、synvelis#126（TIM-02 SMF/player MIDI/cross-golden）、synvelis#127（TIM-03 process boundary/session/loop/driver）、synvelis#128（TIM-04 plugin state snapshot）から抽出した。連携先として synvelis#104〜109・#112/#114・#129/#130・#230・#253・#278/#279/#288 と ruviform#184/#188/#189/#190/#196/#243/#253/#294/#307/#308/#309 がある。
