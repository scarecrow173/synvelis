---
id: "KNOW-K2td8JsVHGxMSFUpb26BQv"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "architecture"
title: "録音基盤"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/117"
    - "https://github.com/scarecrow173/synvelis/issues/118"
    - "https://github.com/scarecrow173/synvelis/issues/120"
  informs: []
  explains: []
  supersedes: []
---

# 録音基盤

## 概要

Synvelis の録音・Take・Comping 系サブシステムの基盤設計と、その検証で得られた知見をまとめる。基本契約は synvelis#12 が定義し、実装は RCD-01（Device Recording・Input Adapter・実入力、synvelis#115）、RCD-02（Durability・Recovery・Publish・SHA-256、synvelis#116）、RCD-03（Monitoring・Punch・Latency、synvelis#117）、RCD-04（Comp Transition・MIDI Phrase・Recovery・Product E2E、synvelis#118）に分割され、派生要件として RCD-F02（Advanced Monitoring・Overdub・Pre-Punch Handles、synvelis#120）が ADR として固定された。headless・fake 環境での検証を実機・実 GUI の証拠と混同しない状態語彙と、fault 注入・決定的 fixture・複数実装経路の照合という検証手法が、この領域の設計を貫く。

## 詳細

### 基本契約と受入条件の管理

synvelis#12 は Recording/Take/Comping の基本契約として、identity/lifecycle、frame/time、RT/handoff、writer/journal/recovery、monitor/latency、comp/editor、transaction の 7 領域（C1〜C7）を定義した。69 の受入条件（AC-01〜69）は Issue 側で採番され、spec・owner-matrix・test-spec の各文書で同一 ID を維持する docs-only slice として固定された。実装の Owner は synvelis#115〜#118 であり、synvelis#12 自体は「設計合意の記録であり実装完了ではない」と明示され、close は Coordinator の判断に委ねられた。

Owner Matrix は 69 AC を Owner Issue・fixture・独立 oracle・OS Gate・状態に対応付けた。集計は Pass (headless) 44、Partial 22、Deferred 2、Pass (design) 1、NotRun 0、Unsupported 0 である。AC-28/AC-29（Take Lane/Comp Editor UI）は UI 未実装のため Deferred（GUI Gate は synvelis#114/synvelis#118 側）と分類され、AC-69（API 適合チェック process AC）は本 chain が証拠となるため Pass (design) とされた。

状態語彙は `Pass (headless)`/`Pass (design)`/`Partial`/`NotRun`/`Deferred`/`Unsupported` を使い分ける。headless・unit の成功を実機・GUI・実 Device・実 Plugin の Pass に換算しない規範が全 slice で繰り返し明記されており、「fake port・fixture の検証は契約証拠であって実機証拠ではない」という区別が本領域の最重要の横断原則である。

### 録音パイプラインと fault の意味論

録音パイプラインの基本形は、fake Ruviform device/input port → `InputAdapter`（callback buffer を録音入力 block へ変換）→ `StreamWriter`（staging media + journal 書込み）→ finalize/recovery/commit である。fake device が実 production 経路を駆動するため、callback→adapter→writer→staging→journal の全経路を headless で検証できる。gate 開閉区間のみが journal cover され、PCM が fixture と一致することが確認された。

fault 注入は input drop・epoch 変更・sample-rate 変更・device disconnect・queue-full・filled-queue を網羅し、fail-stop + durable capture-fault marker + verified prefix 保存が意味論として固定された。faulted session の finalize は durable frontier `Complete` + session-scoped capture-fault marker で recoverable commit に分類される。この期待値は、以前の `Partial` 期待を先例 fixture（`fake_input_drop_fails_stop_and_preserves_verified_prefix`）と照合して訂正したものである。

stamp fallback には重要な設計変更があった。独立した block ごとの timestamp→frame 写像は実 WASAPI で sub-millisecond の jitter を起こし `FAULT_INPUT_DISCONTINUITY` を誘発したため、1 つの seed + 累積 frame-count 配置 + jitter/drift 検証方式へ変更された。実機でしか観測できない jitter が仮想 oracle の写像方式を否定した例である。

実 device pump は専用 scoped thread + sleep cadence を採用した。busy-wait（`spin_loop`/`yield_now`）は WASAPI worker を枯渇させるため却下されている。

仮想 L1 oracle は「独立 generator + emission-log bit pattern」を oracle とし、generator 再計算による自己参照的テストを避けた。これにより production source 無変更で stereo・multichannel・2 mono tap・同一 clock 複数 device・discontinuity prefix・missing-finalized mixed-session の 7 test が通った。

`PreparedRecordingPlan` は非 `Clone` であり `MonitorProbe<'a>` が `&plan` を借用するため、capture 側と monitor 側で同一 `RecordingSpec` から `preflight` を 2 回呼び、別 instance の plan を得る方式が採用された。borrow conflict の回避であると同時に、実系でも monitor session が録音側と同一の prepared plan を参照する意味論を維持する。`PreparedRecordingPlan::sample_rate` は `preflight` で populate され、engine sample rate との一致検証と frame↔tick 写像の基準として凍結される。

### durability・recovery・publish と blob identity

durability の証拠は media/journal/dir の sync stage と durable frontier を記録し、recovery は明示的な review action（Hold/Adopt/Discard）を要求する。参照 media の discard は fail-closed な media-reference probing（`MediaReferenceProbe`/`ProjectMediaProbe`/`CompositeProbe`）で防ぎ、commit receipt は状態を区別し、indeterminate publication を冪等に reconcile する。

publish 失敗の判定は `PublishEffect` の申告を信用せず、「destination の verified bytes を観測」する方式を採用した。retry は確認済み `MissingMedia` に限定される。`StagedFile`/`SealedFile::publish(PublishOptions)` + `PublishMode::CreateNew` への移行で manifest publication の TOCTOU を閉じた（synvelis#84 の project 側 HEAD CAS gate と連携）。非 `Sync` な publish error を `io::Error` へ変換する際は、kind/message 写像で分類を保存する形で対処した。

SHA-256 canonical blob は `blobs/sha256/<64hex>` + `sha256:<64hex>` identity を採用し、legacy CRC32 receipt は readable のまま書き換えない。CRC32 fold は integrity/idempotency digest として残り、SHA-256 は content identity として分離された（synvelis#86 の ALIB blob 方式と整合）。

unsynced-write 損失は「media/journal を最後の sync frontier まで切り詰める」決定的 fixture でモデル化し、対称・非対称損失の両方で Complete 昇格が防がれることを実 FS probe（Windows/NTFS）で確認した。真の power-loss は NotRun のまま残されている。

residual closeout は新規 fixture を作らず、既存 evidence ledger + test 名存在確認で AC を照合する方式を採用した。この過程で test-spec index の重複行 anomaly が発見された。managed area は index 再生成で修正できたが、unmanaged tail の stale `proposed` 行は generator が verbatim 保持するため残存し、手編集せず発見として記録された。AC-3 は ALIB-02 と project HEAD-CAS の責務を跨ぐため、RCD-02 所有分のみ Pass とし、cross-cutting 部分は gate 所有の NotRun/Deferred のまま残した。AC 単位の Pass/NotRun が Issue 責務境界で分割される例である。

### monitor gate の権威と latency 判定

monitor gate の権威は recording 側の `GateScheduler`/`evaluate_gate`/`decide` に集約される。player/app 層は第二の gate 実装を持たず、`MonitorProbe` が plan の scheduler を共有参照して segment を消費するだけの設計が全 slice で一貫している。`decide` は純粋関数であり、Monitor 状態は raw capture（Capture Gate/Writer）に一切触れない。Monitor Off で raw capture を止めず、Monitor On だけで録音を開始しない。

`MonitorSession`/`MonitorEngine` は `MonitorProbe` + `TrackMonitorContext` + `MonitorAdapter`（PCM gain-ramp 混合）を合成し、`HeadlessDriver`/`RoutingEngine` を 1 block ずつ実 run 経路で駆動して各 block の実 master tap PCM（`PlayReport.pcm`）を session へ供給する。engine frame と session frame は 1:1 で、attach 時 cursor を frame offset とし、以後のずれ（seek 等）は fail-closed（無音 + `faulted`）となる。monitor 出力は engine 通常出力・raw capture・export を変更しない独立 bus で、native placement を保ち、latency/PDC 補正・リサンプル・オフセットを placement に適用しない。`ramp_frames=0` では route 遷移が即時で PCM は bit 一致する。

`MonitorAdapter` の gain ramp は DSP crossfade に限定し、gate の sample 精度の代替にしない。route `Input` で input PCM 未供給なら `Muted` へ fail-closed し、`input_missing`/`input_unavailable` notice を立てて PCM を捏造しない。

`LatencyReport::resolve` は round-trip 実測があればその frames のみを合計とし、input/output/reported を再加算しない（二重加算禁止）。`reported` は `reported_verified == true` のみ算入し、`Measured` は算入要素の Clock Epoch が一意に確定した場合のみ維持、未確定・混在時は `Reported` へ降格する。合算は checked_add で行い、overflow 時は `Unknown` に fail-closed する。

`MonitorRoute::Overdub` は RCD-F02 の ADR で route variant として追加する設計が採用された（route+flag 並列表現・集合型再設計は却下）。Hardware Direct Monitor は外部事実として入力に取り、DAW の Mute/Solo や route 判定でその値を変更しない。`Unknown` は `hardware_direct_unknown` notice を常時立て、Software Input 可聴 + 外部監視観測/申告時のみ `double_monitoring` を立てる。

app UX は `syn.monitoring.set`/`punch`/`session` の session-scope 宣言（loop/count-in/pre-roll）を `syn.monitoring.session` start params 拡張で受け、新 op は追加しない。punch マークの正本は authored `project.monitoring.punch` のみで、session-scope の別 punch 宣言は持たない（単一正本）。`loop_start < transport_start` は負 SessionFrame で scheduler が fault するため app 境界で早期拒否し、gate 区間形状の検証は `preflight` に委譲して二重実装しない。

persistence は schema v11→v12 で `Project::monitoring`（`MonitoringData{tracks, punch}`）を追加した。project 側 POD 型は recording 型を参照しない別型として定義し、app 境界で写像する（依存方向は `project ← recording`）。session 観測値（`input_output_ready`/`hardware_direct`/稼働 session/latency 注入）は `MonitorUxState` に残し永続化しない。`SetTrackMonitor` は patch 型ではなく full-decl upsert、`RemoveTrack` は decl cascade 除去、`RemoveLane` は宣言参照中 lane を拒否し、`validate_references` が load 経路の最終防衛線となる。`session_json` の phase は live project punch ではなく `plan.gate` 凍結値と比較する。稼働中の `syn.project.undo` で project punch が変わっても表示 phase が gate と乖離しないためで、この乖離は review で発見・修正された。

VCS semantic diff は `ObjectKind::TrackMonitorDecl`（track-scoped keyed-synthetic view）と `ObjectKind::PunchMarks`（project singleton、`{None,None}`=宣言なしの合成 presence）を `ComparisonDomain::Recording` 配下に追加し、`DIFF_POLICY_VERSION` を 1→2 へ bump した。monitor-only 差分が v1 の `Equal` から `Different` になるため必須の変更である。`monitoring.tracks` は `diff_map` で処理するが `raw_kind_map` には登録しない。`TrackId` 借用の keyed-synthetic であり、全 decl が `Track` との `AmbiguousIdentity` 化を防ぐためである。`punch_range` は transition を生き残った宣言側優先で range を返す。初版は常に `after` を見て `Removed` で range `None` を返す誤りがあり、review で修正された。

monitor fixture は `DeviceRig`（fake device→`InputSession`/`InputAdapter`→`StreamWriter`→`StagingArea`）と `MonitorProbe` を同一 block 駆動で合成し、monitor 状態が raw capture bytes・journal span・fault bits・verified prefix・commit payload を一切変更しないことを固定した。punch 中 device loss は fail-stop + durable capture-fault + verified prefix 保存、新 session での clean recovery、interleave session 間の fault 非漏洩も検証された。production `src/` 無変更で「既存の monitor/capture 境界が正しい意味論を既に所有している」ことを fixture が証明した。

`MonitorEngine` の pump cadence は呼出し内 index ではなく `blocks_run` counter で `index % n == 0` と同一になるよう `Every(1)`/`Every(0)` を切替え、`run(1,...)` 反復で pump が毎 block 化するのを防ぐ。

### comp transition・MIDI phrase・product E2E

crate fixture slice は `compile_midi_comp`（決定的 playback plan コンパイラ）を `synvelis-project` に追加した。MIDI phrase の境界安全性は plan 層の純粋投影として実装され、onset が window 内のみ採用・window 越え note に仮想 note-off・境界は off→chase→on の rank 順・region 先頭で controller kind union chase・phrase 内 CC は mapped tick emit・末尾 `AllNotesOff` を含む。tick 写像は `delta * ppqn / division` が厳密な場合のみ許可し、native tick を requantize しない。selection は stable ID を保持する document 型だが、永続 comp model 登録は synvelis#126 の event model 確定まで deferred とされた。

controller chase は「その時点までに宣言された kind の union」を各 region 先頭で snapshot し、未宣言 kind は default 値で chase して region 間の state 漏洩を防ぐ。selection 検証→controller 入力検証→region ごとの emit→全 event の決定的 sort（tick, action rank, region, note, source_tick）という単一 pass で構成され、失敗時は plan を返さない all-or-nothing である。region id 一意性・同一 source の phrase 窓非重複（onset が 2 region に採用され `(source_id,note_id)` 重複発音になるのを構造的に防止）・`MidiDivision::Ppqn(0)` の fail-closed（`% 0` panic 防止）は review-protocol で検出した契約 gap の修正である。

RT↔Offline PCM 比較は、当初 test 内に lowering 規則を複写した offline renderer を置いたが、これは「実 engine vs project 層 comp 評価」の drift gap を検出できないため、後続 slice（rt-engine-comp-pcm）で実装経路 3 系統照合へ置き換えられた。複写 renderer は依存方向（`recording → project` のみ、player は上位）の都合での暫定であり、「実 RT engine の PCM 一致を主張するものではない」と明記されていた。

RT engine comp PCM slice は `HeadlessDriver::run`（legacy `lower()`）・`RoutingEngine`（semantic `lower_semantic`）・`synvelis_render::export`（`lower_semantic_offline`→`ClipRenderer`→F32 WAV reopen）の 3 実装経路で全 sample bit-exact 照合し、plan-derived anchor（`expected_at` = Σ window 写像 × `FadeWindow::gain_at`）で 13 probe frame を固定した。test 内複写 renderer は作らず、`export()` を offline 参照とする oracle 方針を採用した。ただし共有する `plan_playbacks`/`FadeWindow`/`TickTimeMapping` 自体の欠陥は構造上検出できないという限界があり、plan-derived anchor が placement/envelope の絶対値を固定して緩和している（mapping 解釈ミス自体は domain test の守備範囲）。Loop/SimpleStretch では legacy 経路の explicit-skip 沈黙再生と semantic/export の build 全体拒否という意図された fail-closed 挙動差を記録した。これは欠陥ではない。

product E2E は `UiRuntime::dispatch_op` → `Service::call` → `AppService` → `SynvelisApp` の実 `syn.*` dispatch を駆動し、fixture 構築のみ `app_mut()` canonical apply を使う「検証対象=dispatch・構築=canonical mutation 経路」の境界を明確にした。第二の project/recording/comp モデルは作らず、crate 層 fixture の domain 契約を再利用した。

MIDI phrase boundary fixture は `compile_midi_comp` → `compile_midi_events` → `ParamDispatch::dispatch_midi` → `HeadlessDriver`/fixture plugin に通し、期待 wire 列を手導出して完全一致 + 配送列への ledger 走査（全 note-on が一意の note-off と対・終端 active 空・重複発音なし）を独立検査した。canonical wire 値（CC は u32 スケール、pitch bend は符号付き canonical）への変換を期待値 builder も経由させる必要があった。初回 raw 7-bit 期待で失敗し、canonical 値に修正している。

comp kind の VCS apply は `ApplyError::Unsupported` で fail-closed を pin し、whitelist 追加は別 slice の設計事項とした。`Comp.transition_ids` は親の ordered-list membership 差分として Skip 分類されるため、transition 増減は `CompTransition` object 自身の change のみで表現される。

RCD-04 residual closeout は docs+evidence のみで、受入条件 4 行を merge 済み実装（PR #219/#300/#319/#374）の実在 test 名・merge 記録・canonical lane 実測に 1:1 対応付け、外部 gate（実 GUI=synvelis#114・AudioLens native f64=`ruviform`#294・MIDI 永続化=synvelis#126・実 device/実 RT=実機試験・comp apply whitelist=別 slice Deferred）を NotRun/Deferred のまま正直に棚卸しした。merge 済み 3 task + 3 impl の status/checkbox 簿記 drift を事実整合で修正した（作業記録本文は書き換えず）。

### overdub・pre-punch handle の設計決定

synvelis#120 は「ADR 合意を canonical 文書チェーン + evidence として固定する docs-only slice」であり、実装・実 OS・実 Device・実 Plugin の受入は主張しない。実装は ADR merge 後の後続 slice が Owner である。

MADR 4.0.0 形式の ADR で 5 領域の決定を固定した。

- D1: `MonitorRoute::Overdub` variant 追加。route+flag 並列表現・集合型再設計は却下。
- D2: 既存 `MonitorNotices` 3 variant 再利用 + `double_monitoring` 発火条件を input-leg 可聴（`Input`|`Overdub`）へ拡張。新 notice variant 追加は却下。
- D3: journal kind=4 `HandleGrant` record + 既存 `JournalSpan` の effective_ranges 包含分類。`JournalSpan` material field・VERSION bump・別 journal/media file は却下し、旧 reader は fail-closed prefix 切り詰めとする。
- D4: 既存 project undo + RCD-02 verified prefix 整合。handle 専用 undo/recovery 新設・take 同一 commit 単位は却下。
- D5: monitor PCM 非永続化 + session-scoped consent grant のみ。project-level consent 永続化・無期限 staging 保持は却下。

UX Matrix は `decide` 既存判定表を baseline 転記し、Overdub 次元を `MonitorRoute::Overdub` で表現する。handle/take 分類の曖昧さは effective ranges の構造的 disjoint を plan 凍結時に保証する設計で解消した。kind=4 journal の旧 reader 互換は fail-closed（prefix 切り詰め）を仕様として明記した。handle capacity の具体的既定値は実装 slice の fixture で確定し、本 slice は必須属性契約のみを固定した。

### fixture 構築と検証の定石

`DeviceRig`/`open_session` 系 fixture は `build_payload` が staging `capture_stream_id` を `Project.recording.streams` に登録済みであることを要求する。fixture は `CaptureStreamId::new(500)` のような明示 ID を project 側に mirror 登録して commit 契約と一致させる。`commit_session` の registry 解決に必要な tagged id（`RecordingSessionId`/`CaptureStreamId`/`LaneId`）を staging manifest に mirror するパターンは RCD-04 E2E でも再利用された。

comp 遷移 fixture の構築パターン（recording ops→take→TakeGroup→Comp→Transition）は既存の `md_b.rs` パターンを踏襲し、`x_e2e.rs`/`sf03_l6.rs` の export+expect_pcm・`RoutingCompileRequest` helpers を再利用して新規実装を最小化した。`synvelis-render/tests/` に test を置く判断は、render→player 既存依存で 3 経路すべてを駆動でき manifest 変更が不要という理由で coordinator が決定し、`synvelis-player` への dev-dep 追加は否認された。

`undo()`/`redo()` は履歴を戻すのではなく新 revision を発行する既存契約のため、「revision 番号の復元」ではなく「+1 step・対象内容の完全一致」で pin するのが正しい検証形である。comp registry JSON が切替前後で byte 同一・raw take/source/asset が全操作で不変であることを comp 切替 fixture で確認した。

`SharedService`（`Rc<RefCell<AppService>>` への delegating `Service` impl）で `UiRuntime` に `AppService` を再登録する harness パターンは、`UiRuntime` が登録済み service を呼出し側へ返さない制約を回避しつつ `syn.*` dispatch を実経路で駆動する定石として機能した。長時間 borrow は `app_mut()` と競合し得るため、call 完了後のみ fixture 変異を行う運用が前提である。

VCS diff の `overall` は tri-state（fake asset digest で `unknown` を返し得る）であり、entity-level `changes`/`summary.total` を主張し、`overall` は `different` でないことのみ確認する扱いが E2E で採用された。asset 非依存の MIDI fixture は真の `equal` を確認できた。

`Project::monitoring` は `#[serde(default)]` で pre-v12 snapshot 互換だが、v12 snapshot を v11 build で読むことは `deny_unknown_fields` で拒否される。意図的な前方非互換として contract_change に記録済みである。

### 簿記 drift と検証 lane の運用

plan/task の status=in-progress のまま IR が completed という齟齬が本領域では常態化していた。特に RCD-04 の merge 済み 3 task + 3 impl は frontmatter/checkbox が stale で、residual closeout が事実整合の修正を行った。正しい対応は「事実整合の status 修正 + 当時の作業記録本文は書き換えない + 生成物は手編集しない」で統一されている。

canonical 検証は `mise run <task>` 経由（`fmt-check`/`check`/`lint`/`test`/`test-rt`/`check-app-native`/`lint-app-native`/`test-msrv`/`build`）で行い、直接 `cargo test` は補助証拠として使う。canonical=mise と明記する規約である。環境は Windows 11 x64・rust 1.99.0 pinned（mise.toml/mise.ci.toml）、MSRV は rust 1.89.0。`test-msrv` の実行有無は slice ごとの指示に依存し、Daily CI 委譲として NotRun にした slice と `mise exec rust@1.89.0` で実際に Pass させた slice が混在する。

review-protocol（worker 契約に基づく承認代替）は Reviewer A（spec 適合）/Reviewer B（敵対的正当性）の分離 pass で Issue Candidate を洗い、確認済み指摘を修正する運用である。`session_json` phase の live punch 参照→`plan.gate` 凍結値修正、literal schema pin 2 件の v12 bump 対応、`punch_range` の `Removed` range 欠落修正、MIDI wire 値の canonical 変換修正など、複数 slice で実際に欠陥を検出・修正している。

docs chain は spec→design→plan→task→test-spec→実装記録（+実験記録）の 6〜7 文書構成で、docs 配下の README 管理 index は `index plan`/`apply`・`sync_impl_index` で再生成する。index を slice が編集しない判断（Coordinator が統合時に再生成する前提）も記録されており、index 未同期は許容済み状態として扱う運用が両方存在する。graph router `route_graph.js` は repository-wide pre-existing broken relation で `blocked` を返すことがあり、slice 起因でないため無関係な broken relation は修復せず範囲外とする先例が確立している。

flake 観測と切り分けの記録もある。`synvelis-library/tests/webdav.rs::browse_cursor_pages_listing` の `os error 10053`（socket abort）と `synvelis-player/tests/sf03_l2.rs` の `memory allocation failed`/`STATUS_STACK_BUFFER_OVERRUN` はいずれも大規模並列 test 実行時の一過性で、単独・workspace 再実行で緑となり変更と無関係と記録された。`test-msrv` の `crate synvelis_gateway required to be available in rlib format` は同一 target dir への rust 1.89 build と 1.99 fixture test の並行 artifact race で、単独再実行で Pass した。

実 device coverage は clean run と fault run を分離して記録する運用が確立された。clean run（10/10 pass・1,323,000 frames・0 fault）を実機 Pass の根拠とし、discontinuity/device-loss を観測した run は fault 種別ごとに NotRun/blocked として verified-prefix 証拠付きで残す。「1 回の成功 run」をもって全条件 Pass とせず、観測された fault を抑制・再分類しない誠実な記録が実機検証の型になっている。

その他の記録上の注意点として、monitor-session-ux slice の experiment log は未作成だが、これは「アプローチが先行 slice で確立済みのため exp は optional」と判断した意図的な省略であり欠落ではない。RCD-01 real-input coverage は「記録された 1 run で 10/10 pass（clean・0 fault）」と「別 run で `FAULT_EPOCH_DISCONTINUITY`・device disappearance・continuity fault を観測し NotRun/blocked 分類」が併存する。clean run の成功と fault 観測は矛盾ではなく、実機の非決定性を正直に分離記録したものである。RCD-04 comp fixture の renderer 複写は暫定であり、削除/統合判断は別 slice の follow-up として残っている。

## 適用対象

この知識は Synvelis の録音・Take・Comping 系サブシステム全体に適用される。具体的には `synvelis-recording`（録音パイプライン・fault 意味論・monitor gate/persistence・comp fixture）、`synvelis-project`（`compile_midi_comp`・`Project::monitoring`・VCS diff 分類）、`synvelis-app`（`syn.*` dispatch・`SharedService` harness・session-scope 宣言）、`synvelis-render`/`synvelis-player`（comp PCM 3 経路照合・blob identity）にまたがる。外部 gate（実 GUI=synvelis#114、AudioLens native f64=ruviform#294、MIDI 永続化=synvelis#126、実 device/実 RT=実機試験、comp apply whitelist=別 slice）に依存する領域は NotRun/Deferred として区別して扱う。

## 出処

synvelis#12（Recording/Take/Comping 基本契約・69 AC 管理）、synvelis#115（RCD-01 Device Recording・Input Adapter・実入力）、synvelis#116（RCD-02 Durability・Recovery・Publish・SHA-256）、synvelis#117（RCD-03 Monitoring・Punch・Latency）、synvelis#118（RCD-04 Comp Transition・MIDI Phrase・Recovery・Product E2E）、synvelis#120（RCD-F02 Advanced Monitoring・Overdub・Pre-Punch Handles、docs-only ADR）から抽出した。連携先として synvelis#84（project HEAD CAS gate）・synvelis#86（ALIB blob）・synvelis#114（GUI Gate）・synvelis#126（MIDI 永続化）・ruviform#294（AudioLens native f64）・ruviform#251/#565/#628/#629（実 device 周辺）がある。
