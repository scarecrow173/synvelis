---
id: "KNOW-xvWO88I0UCWGh887XfiAYx"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "architecture"
title: "パラメータコントロール"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/synvelis/issues/40"
    - "https://github.com/scarecrow173/synvelis/issues/43"
    - "https://github.com/scarecrow173/synvelis/issues/64"
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/99"
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/230"
    - "https://github.com/scarecrow173/ruviform/issues/219"
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/307"
    - "https://github.com/scarecrow173/ruviform/issues/313"
    - "https://github.com/scarecrow173/ruviform/issues/314"
  informs: []
  explains: []
  supersedes: []
---

# パラメータコントロール

## 概要

Synvelis のパラメータ制御系（自動化・パラメータ dispatch・録音・trace/observation・UI）の設計知見をまとめる。PC-A（制御モデル基盤、synvelis#40）→ PC-B（dispatch / note-scope / RT / offline、synvelis#43）→ PC-B 残件 driver 配線（synvelis#64）→ PC-C1（録音トランザクション、synvelis#98）→ PC-C2（relation index、synvelis#99）→ PC-C3（control trace、synvelis#100）→ PC-C4（plugin binding / observation / app composition、synvelis#101）→ PC-C5（UI slice、synvelis#102）→ PC-C6（独立 oracle / 受入台帳、synvelis#103）という一連の slice 群から抽出した。

この領域を貫く原則は三つある。真実の所在を単一権威に寄せること、推測で補わず fail-closed の typed rejection または absence evidence に落とすこと、headless 検証の成功を GUI・実 plugin・実 OS・実 RT の受入 Pass に読み替えないことである。

## 詳細

### 制御モデルの単一権威とレイヤー配置

authored モデルは `Project::control`（`ControlModel`）一箇所に置き、lane view / signal flow view / inspector breakdown 等の投影は read-only の派生ビューとする。mutable な真実を複数箇所に複製しない設計が全 slice を通じた基調である。モジュール配置も分離されており、モデルは synvelis-project、セッション挙動（mode・gesture・recording transaction・observation）は synvelis-workspace の `ControlSession`、dispatch/実行は synvelis-player に置く。プロジェクトを純粋モデルに保ち、plugin bridge 等の実行依存を project 側へ持ち込まない。

`BeatPoint`/`BeatRange` は Synvelis 側が所有する persisted wire DTO で、有理数 beat 座標を canonical state とする。ruviform の `BeatPosition`/`TickPosition` は serde を持たないため永続化形式を自前で定義し、runtime 変換は `to_beat`/`from_beat` 経由で行う。`f64` 変換は lossy であることが文書化されている。時間演算は checked 演算のみとし、ruviform-media-core の `Tick`/`TickDuration` が unchecked `Add`/`Sub` のみ公開するため、Synvelis 側で直接演算子を使わない規約が lint 対象として維持されている。

conflict policy は Reject（既定）と明示的 Replace の 2 系統で、暗黙の lane 優先順位や silent 上書きを行わない。同一 target・同一 eval 時刻の重複は `AutomationConflict` として reject する（conflict 意味論の合意は synvelis#6）。`detect_conflicts` は O(regions²) であり、大規模化には index が必要という既知の制約が記録されている。

capability 判定は upstream（ruviform の `ParamInfo`/`ParamFlags`）を authoritative とし、write means を Synvelis 側で重複定義しない。`check_param_write` が authoritative な判定経路である。`TimedEvent` 非対応 capability は `BlockBoundary` への明示的 lowering に落とし、`Lowered` enum で劣化を表面化する（reject を隠さない）。

録音 commit は既存の `ProjectOp::Composite` → session = 1 undo の原子経路に乗せ、新規 undo 機構を作らない。loop wrap は `SealReason::LoopWrap` で seal し、silent 上書きしない。制御 graph compile は cycle を reject し、評価は prepared compile（`EvalPlan`）と live eval を分離する。`ControlSession` は mode（Off/Read/Touch/Latch/Write）・`WriteArm`・gesture・recording transaction・observation・stamp・投影を workspace 側に持つ session 挙動の正本である。

スキーマ版は意図的な破壊的 bump が記録されている（v1→v2）。PC-A/PC-B 時点では移行経路なしのポリシーで、`load()` は `UnsupportedSchema` で strict gate した。`SCHEMA_VERSION` リテラルを pin する test は版 bump ごとに更新が必要である。

### note scope・prepared eval・RT dispatch

PC-B では `Note` エンティティを追加した。note scope は `resolve_scope_pos` の Note arm で clip arm を合成した上で domain 検査する。note-scoped target は note→clip 参照を pin し、`RemoveClip` は note を cascade 削除、dangling owner は post-apply oracle で reject する（clip-scoped target と同じ規則）。`SplitClip` は分割点を跨ぐ note を reject し、分割点以降の note は re-parent/re-base して絶対 eval domain を保存する。

`EvalPlan` は compile 時に owner scope/base/kind/merged sorted series/assignment/depth-source index を確定し、caller 所有の `EvalScratch` バッファを使う。`eval_rt` の成功経路は alloc-free であり、error 経路（`format!`/`NotFound(String)`）は ruviform RT test 規約に倣い契約外とする。`computed` flag は呼出ごとに reset して stale cache を防ぐ。

dispatch の境界は synvelis-player（実行クレートが plugin bridge を所有）で、project は純粋モデルに留める。`ParamDispatch` は `PluginHost`→attach→`PluginNode`→`install_plan`→endpoint `ParamEvent` の経路で typed rejection token（`param_unknown`/`param_unresolved`/`param_mismatch`/`queue_full` 等）を伝播する。

`PluginNode::offline_support()` の既定 `Unknown` は ruviform 側の意図的契約（opaque plugin state のため node 単体では判定不能）であり、instance 単位の `Verified` 宣言を `NodeFactory` wrapper で行うのが正しい拡張点である。consumer 側の恒久 workaround は行わず、upstream issue も不要と結論された。

`EvalPlan::is_stale` は `ControlGraph::compile(project).map_or(true, |p| p != *self)` の recompile-compare で、compile 失敗は stale 扱いになる。コストは check ごとに O(compile) で、最適化は deferred とされた。driver 配線では `activate_with_attached` が publish 前に binding 一意性を検査し、reject 時は last-good plan を保持する。`anchor_attached_transport` は `set_clock`→`Seek`→`Play` を FIFO で送り、fresh node が position 0 から開始しないようにする。seek 時の quiesce は attach 済み node がある場合 2 pass（32-control/callback drain 上限 × 2 = 64-channel 網羅）で、rewind window の event を 1 回だけ再配送する tradeoff が文書化されている。`rt.process` 失敗時は observe を skip し、scheduler_gen gate で reject された `install_plan` が `Dispatched` を偽造 mint しない。record write 経路は removed/unbound write lane を早期 reject し、`build_commit_ops` は全 segment lane の liveness を `take()` 前に検証して失敗時は transaction を復元する（旧実装は silent total loss だった）。`install_clock_anchor` は playback epoch を anchor frame epoch に同期して `EpochMismatch` を防ぐ。workspace session 側では、empty Off-mode gesture は `GestureEnd::Returned` で返し seed 評価値を BaseEdit として commit しない修正が入った。

### 録音トランザクションの 3 相 commit

PC-C1 の録音 commit は非破壊の `prepare_record_commit` → fixed `expected_revision` での CAS `apply` → 成功時のみ `finish_record_commit` の 3 相で構成される。Conflict は全 point を保持し、回復経路は明示的 rebase のみである。write gate は stop/seek/mode/arm/project-change で seal されるが pause では seal されない。seal は write gate のみ閉じ、commit/abort/inspect は可能なままである。

session 内 idempotency は `RecordingTransactionId`（session ごと単調増加）+ FNV-1a fingerprint で実現する。同一 id+fingerprint → `AlreadyApplied`、同一 id で payload 相違 → `IntegrityConflict`、未知 id → `UnknownTransaction` である。applied registry は in-memory・上限 64 で、crash 復旧・永続 registry は synvelis#12 へ deferred された。`preflight_record_finish` は receipt registry と open tx の id/fingerprint を apply 前に検証する。レビューで「apply 後照合」による二重書き込み hole が発見され、apply 前照合へ修正された。point budget は `RECORD_POINT_BUDGET = 262_144`、`SealReason` 拡張と `fixed_lanes`/epoch/`fixed_revision` を持つ。空 commit は receipt を残さず retry は `UnknownTransaction` を返すが無害と記録されている。

### relation index と graph 走査

PC-C2 の `RelationIndex` は `Project::control` 上の revision タグ付き派生双方向 index で、第二の control 正本を作らない。query は `Project`/eval/compile/undo を一切 mutate しない read-only 保証を持ち、serde なし（永続 schema 変更なし）、新規依存なし（BTreeMap/BTreeSet/VecDeque のみ）である。

cycle 検出の SCC は再帰なしの iterative Kosaraju（明示 stack）で深い graph でも安全に動く。SCC は disabled 以外の辺——dangling `Unresolved` endpoint を含む——を対象とし、compile が dangling endpoint を cycle 検出の node として扱うのと同じ規則である。`Disabled` 辺は compile と同様に除外する。`Both` direction は connected component 内の無向到達性で、`visited_edges` dedup を行い格納 direction は保存する。

query 意味論は、revision 不一致 → `Stale`（edge なし）、未知 anchor → `Unavailable`、`max_depth==0` → `Complete` 空、`max_edges` 超過 → `Truncated` である。`RelationReply.project_revision` は index 世代を返し、consumer が staleness を検出できる。

### control trace の stage 記録

PC-C3 の `ControlTrace` は 7 つの独立 stage slot（configured/prepared/scheduled/applied/dispatched/plugin_reported/observed）を持つ。`TraceSlot` は `Present`/`Missing`/`Unsupported`/`Dropped`/`Expired` で、欠落 stage を合成して埋めない。

freshness は key↔ctx の generation/epoch/transport/loop/position 一致を先に評価し、次に stamp age を見る。future/uncomputable は `Estimated` provenance で `Current` にはしない。`freshness()` は best-channel 集約で個別 `*_freshness()` accessor もある。`is_coherent` は slot evidence の generation と key の一致を検査し、history は bounded である。`DispatchEvidence`/`DropReason` は bridge 型の semantic mirror として project 内に定義し、project が ruviform-audio-plugin-bridge に依存しない構造を保つ。

TRACE-C3-05 の `DispatchTraceAdapter`（player 側 RT trace adapter）は `ParamDispatch` を値で所有し、「唯一の report-channel drain owner」を型レベルで保証する。`observe_cycle` が唯一の drain 経路である。bounded 条件は history 256・`max_targets` 1024・`plan_gen_window` 64。`plugin_reported` stamp は常に `Estimated` provenance で `Current` を名乗らない（意図的な under-report）。ledger revision ↔ dispatch ordinal の 1:1 仮定は upstream `record_plan` 規約への依存として記録されている。trace projection のレビューでは、`AppliedReceipt`/`CommandId` が外部から構築不能だったため `ReceiptEvidence` 投影型を導入し、freshness の `Unavailable` tie-break が `ClockUntrusted` を隠す問題を Present-slot 優先で修正した。

### plugin binding・instance identity・observation・app composition

plugin identity の pin は connect 時の `ProcessorBinding::Resolved(PluginRef)` を唯一の正しい evidence とする。live project の再読は rebind 後の旧 generation instance を誤 pin し得るため使わない。bind は stage-1 `semantic_plugin_binding` を解決し `PluginRef` を比較し、不一致は `PluginMismatch` で fail-closed である。旧 generation の last-good binding はその instance に対して有効なまま、新 bind は generation 遷移まで typed mismatch で fail する。

`plugin_parameter_instance_binding` は store vouch 失敗（欠落・PluginRef drift・範囲外 id）を `instance: Option` の `None` で返し `Err` にしない。semantic 解決失敗は従来どおり `Err` で、`bind` の `PluginMismatch` → `InstanceMismatch` の報告順序を維持する。`instance` を `Err` にする案は typed error の報告順序という観測可能契約を退行させ、semantic/instance を別関数に分ける案は正本分散を招くため、いずれも却下された（experiment 記録）。

frame base gate は `ParamDispatch::new` が `Result` を返し plan staging 前に `control.check_frame_base` を実行する。capability 比較は `f64::to_bits` の bit-exact（epsilon なし）で、batch dispatch は全 (binding, event) pair を staging 前に検証し atomic reject する。`eval_event` は event ごとの plan freshness 再検査をせず、caller が orchestration 境界で `check_plan` を gate する設計である。

persisted `StablePluginInstanceId`/`PluginInstanceIdentities` が schema v13 で authored instance の権威となった。identity store の watermark は単調増加で `Remove`→`Insert` でも id を再利用せず、undo は復元する。`ensure` 意味論は、同一 `PluginRef`（`capability_revision` 含む）なら id 維持、差異があれば新 id である。v12→v13 migration は `Resolved` processor を `ProcessorId` 順・1-based の deterministic id で backfill し、`Resolved`⇔entry の bijection oracle を `validate_references` が load と apply の両方で reject する。runtime は不正な persisted state を「修復」せず、resolve 失敗は `without_control` fail-closed である。

pinned ruviform に attached instance の param 列挙 API がないため、`ParamCatalog` は caller 登録テーブルを `PluginHostProcessors` が集約する vend 方式を採用した。未登録 plugin は空 table を vend し bind が typed に失敗する（値を捏造しない）。`SessionEngineConfig`/`ProcessorConnection` への field 追加は既存 literal/destructuring を壊すため setter 方式を採用した。

`TransportMirror`（driver.rs を `pub(crate)` 化）を `NodeSurface` ごとに mirror し、command は scheduler が受理した後にのみ記録する（mirror が scheduler reject を捏造しない）。`note_transport_command` は `media_time_ticks` で node frame base に変換し、`Pause` には `note_pause` を追加して command 網羅を完成した。`observe_block` は runtime 所有の transport state から span を内部導出し、呼出ごとに span を捏造していた旧 `syn.control.observe` の問題を修正した。generation 不一致は fail-closed である。

`observe_block` の `call_start` は node cursor domain であり engine cursor とは別物である。routing path は `SetClock`/`DeviceRestart` を送らないため `call_start` は `ClockEpochId::new(0)` の node-cursor domain とした。engine は `rt.process` 成功ごとに post-process の `active_generation` を読み `observe_gen`/`observe_head` を自前追跡し、generation 変化で head=0 に戻す（観測しない block でも head は進める）。`run_observed`/`seek_observed` は opt-in observer variant で、`run`/`seek` は no-op observer に delegate し挙動不変である。

scheduler receipt は drain-once である。`observe_cycle` 呼出ごとに report channel を drain するため、run 中の auto-observation が `AppliedReceipt` を消費し、後続の明示 `syn.control.observe` の `receipts` は空になり得る。receipt の authoritative consumer の役割は `note_receipts` による transport/plan generation 補正であり、明示 op の `receipts` は「その call が drain した分」を正直に返す意味に整理された。receipt の peek 型化は単一 drain owner 契約違反・二重適用リスクで却下、response merge は window 帰属の偽装・ui_pump 先行 drain で効果薄として却下された（experiment 記録）。

`RefCell` borrow 競合対策として、observer callback は `inner.borrow_mut()` 内で workspace を feed せず、engine が `DispatchCycles` を buffer し app が `ui_pump` 周期で drain する。publish は pending cycle を clear して stale generation の evidence を流さない。`attach_routing_engine` は engine↔drain の host-cell pairing を規約ではなく attach 時の強制にした（レビュー指摘）。

`syn.control.fast_control/status/observe` op surface は typed evidence のみを返し、値を捏造しない。status は unattached/drain-not-run を正直に示す（`active_generation` 0→null、`ReportDrainReport.attached:false`）。observe の span/call window/`plan_generation`/`transport_generation` は全て caller evidence で op は surface generation 一致のみ gate し、不一致は誤配送ではなく `Scheduled` 残留の fail-safe である。receipts は structured JSON で返し caller の generation 学習経路とし、`position` は surface が検証した `frame_base` で stamp し caller は base を主張しない。

library は `std::env` に触れない契約を維持し、`SYNVELIS_PROJECT_DIR` の env 読みは process boundary の bin が行い `Option<PathBuf>` を渡す。`compose_product_routing(app, project_dir)` の `Option` 契約が gate 意味（`None`→stub 維持）を in-process testable にする。webview bin は実 WebView2 backend を要求し headless spawn 不能のため fixture は helper を直接呼び、env 読み 1 行は compile + review で担保した。native bin は意図的に無変更（回帰リスク回避）で、重複は Deferred Design Concern に記録された。ruviform-plugin-core を synvelis-app の `[dependencies]` へ昇格した（`ParamWriteMeans` blocker、coordinator 承認）。`ui_pump` が plan generation を deterministic に進める（fast=1、automation=2、observe pump +1 の規則が op surface の evidence と整合）。

### UI slice の read model と gesture

PC-C5 の relation inspector は `relation_ui` read model で、publish ごとに `RelationIndex` を構築する（query あたり O(N) だが非 RT で許容）。既定 query は depth-1 `Both` で全 row が anchor に接するため `neighbor()` 写像が一意である。`include_disabled` 既定 true で `Disabled` 辺を正直に表示し、`RelationNav` は LIFO stack、`is_stale` が revision drift を検出する。既存の `signal_flow_view`/`lane_view` は置き換えず併存する。

control edge gesture は別型 `control_edge.rs` として実装した。既存 `ControlsUi` の `CommandIntent` は 2 endpoint connect を表現できず、`gesture.rs` の Audio Routing は別 domain であるためである。fixation は Begin 時に view・typed endpoint・`ProjectRevision`・window epoch を捕捉し、commit は 2 段 check（`is_transferring`/`owner_of` の window epoch、次に `session.apply` の CAS `StaleRevision`）で stale を drop する。stale gesture は破棄するが非 stale の project error では gesture を保持する。session-only field は serde skip で preset 非永続を維持し、`follow_source: None` は legacy 挙動（後方互換）である。

recording status read model は pure read fn で 5 状態 DTO（`Conflict`/`Prepare`/`Pending`/`Applied`/`Unknown`）を返す。`Prepare` は caller 保持の `PreparedRecordingCommit` 投影（永続的な「Prepared」txn 状態は存在しない）を `preflight_record_finish` + `expected_revision==live` で検証する。`Conflict` は prepare が最初に検査するのと同じ evidence（`tx.state==Conflict` または `fixed_revision != live`。revision は単調なので drift ⇒ 次回 commit は conflict）で予測し、`Retry` action を出さない（先に re-pin が必要）。health row は open txn の `fixed_targets` に限定する（session は target を列挙しない）。

### 独立 oracle と受入台帳

PC-C6 の oracle fixture の方法論は、契約文書（test-spec の Rule）から oracle を独立に再実装し、impl の出力と field 単位で比較するものであり、impl 内部を呼ばない。oracle と impl が食い違った場合、impl を oracle に合わせて変更せず、spec/design/test-spec の解釈へ立ち返る。これが oracle の存在意義であり contract drift を検出する仕組みである。fixture 値は先行 test と意図的に異なる値を使い独立性を証明する。`Instant` の timing は evidence 記録のみで pass/fail に使わず sanity bound のみとする。

`stats_alloc` は process-global のため RT noalloc test は独立 binary（pc_c6_oracle_pl_rt.rs）か単一 test への集約（pc_b_rt.rs が等価 assert を同一 test に fold）で並列 cross-talk を回避する。warm-up は測定 window の外、判定は alloc/dealloc==0 のみで timing 閾値は設けない（baseline 未確定のため）。

受入台帳は Issue 固定 Test Matrix 全 53 行（PC-C1-R・PC-C2-REL・PC-C3-TR・PC-C4-PL・PC-C5-UI・PC-C6-COMP + PC-X1〜X6 + GUI-PC-01〜04）を「headless 実施可能か」「既存 fixture が被覆するか」の 2 軸で棚卸しした。結論は「headless 実施可能部分は全て既存 fixture で被覆済み、新規 test 追加 0 件」である。物理的に headless 不可能な部分（実 OS/GUI/plugin/device/RT/consumer）は fixture 追加ではなく `NotRun`/external owner 記録が正しい処理と判断された。部分被覆は完全被覆と見せかけず `◐`/部分被覆として明示する（PC-C1-R08、PC-C5-UI01、PC-C3-TR07、PC-X2、PC-C6-COMP04 等）。`Unsupported` は 0 行で、技術的非対応の証拠がない行を捏造しない。`Pass-measured (headless)` 語彙を「当該 SHA で `mise run test` green」に限定して導入した。

台帳監査で P2 文書問題を発見・修正した。行数分類の誤り、同名 test pointer の曖昧さ（`pc_b.rs` が player/workspace で重複し workspace 側 `pc_b_02_*` を見落としかけた）、note-scope 被覆の過少計上、検証不能な正確 test 数の記載である。`pc_c6_oracle_pl.rs` の test 関数コメント番号（PL03/PL04/PL06）が test-spec の Rule semantic と一部入れ替わっていることも発見した。機能的欠落ではないため ledger 側を Issue 受入条件（test-spec semantic）で写像し差異を明記し、fixture 本体のコメント修正は編集範囲外として記録に留めた。ledger が `synvelis-workspace/tests/` の fixture を引用するため、引用の正直性のため `mise run test synvelis-workspace` を補助証拠として実行した。`check`/`lint` の workspace は本 slice が同 crate を変更していないため未実行と明記した。

### 横断する設計規範と検証手法

headless 検証の定型は、fixture test（`crates/*/tests/pc_*`）+ `mise run` 全 lane + review-protocol 2 pass + IR への表形式 evidence 記録であり、全 slice 共通の完了形である。fail-closed typed error は徹底され、stale generation・欠落 identity・foreign plugin・disabled/unresolved relation・invalid frame base・unavailable report・invalid transport command は全て推測 fallback せず typed rejection または absence evidence にする。`without_control`・`Stale`/`Unavailable`/`Truncated`・`Scheduled` 残留等の語彙が一貫している。

真実の所在を単一権威へ寄せる一連の移行が行われた。authored state は `Project`、semantic plugin binding は stage-1 project 権威、instance 真実は connect 時 binding + persisted identity store、runtime transport 真実は `TransportMirror`、observed evidence は実 dispatch/report channel である。buffer + deterministic drain は ownership/borrow 競合を解消しつつデータを捏造しない手法として機能し、engine が cycle を buffer し app/ui_pump が drain、generation swap で stale buffer を clear、receipt は drain-once で「その call が drain した分」のみ返す。

schema migration は明示的互換境界として扱う。PC-A/PC-B は意図的破壊的 bump（移行なし）、PC-C4-05 は v12→v13 の `migrate_v12_plugin_instances` MigrationStep を追加し defaults + deterministic backfill で互換を確保した。版 bump はポリシー転換点として記録する。

coordinator 承認・外部 gate の正直な記録も一貫している。GUI・実 plugin・OS・wire-schema L1・AudioLens・consumer・実 RT/device test は明示的に外部 gate/NotRun として残し、headless green を製品受入に見せない。pin 更新待ちの upstream 不足（attached instance param 列挙、L1 wire report verb、bridge `offline_support` verification 等）は ruviform#253/#307/#313/#314/#219/#294 等の関連 Issue として記録された。

canonical 文書鎖は spec→design→plan→task→test-spec→impl(ir/exp) で構築される。task の `Done When` は検証可能な条件、IR の「検証」節は `mise run` コマンドと結果を表で記録する定型が守られた。review-protocol（協調 Reviewer + 敵対 Reviewer の独立 pass）で CONFIRMED P0/P1 blocker が残らないことを Worker 契約上の承認代替とする運用が全 slice で一貫しており、敵対レビューは実際に複数の実害バグを検出した。範囲拡大（workspace への test 追加、apply/entity.rs の同期処理、synvelis-app への ruviform-plugin-core 依存昇格など）は coordinator への ask 承認を経て条件付きで許可され、承認条件は Decision/記録に残す。

canonical 検証入口は `mise run` レーン（`fmt-check`/`check`/`lint`/`test` に `test-rt`/`test-msrv`/`check-app-native`/`lint-app-native`/`build`/`check-headless-deps`/`check-ruviform-rev` を必要に応じて追加）で、直接 `cargo` 実行は補助証拠として「補助」と明記される。evidence の判定語彙は「Pass (headless)」と「NotRun / external owner」を厳密に分け、headless・fake driver の成功を GUI・実 plugin・実 OS・実 RT の受入 Pass に繰り上げない。PC-C6 では「`mise run test <package>` が当該 SHA で green だった」ことのみに限定した `Pass-measured (headless)` という語彙を導入した。

実験記録（impl/exp）は「問い → 検討した選択肢 → 結果 → 却下理由」の構造で、設計分岐の理由を後から追跡可能にする。`.md` 形式の experiment record が使われる。doc graph router（route_graph.js）は repository 全体の pre-existing broken relation で fail-closed `blocked` を返す baseline 状態が続き、文書鎖は宣言済み順を手動で踏襲する運用である。docs index は生成物であり手編集しない。

`check-ruviform-rev` の REV 不一致は gitignore された `ui/vendor/ruviform` の stale 検出で fail-closed し、復旧は `mise run sync-ui-deps` の再実行である（synvelis#230）。Ruviform の恒久的ハードコード・vendor コピーは禁止で、解決 rev は `Cargo.lock` が正本である。

環境面の運用知見として、Windows 環境では `CARGO_BUILD_JOBS=4` で page-file 起因のビルド失敗を緩和する。`test-msrv` が disk 枯渇（`os error 112`）で失敗した例では `mbx gc` で 257 GiB を解放して再実行 pass した。既知の環境 flake として `synvelis-library` の webdav `TransientIo`、`status_mapping_table` の status-mapping 不一致、`monitor_arm` の rlib format エラーが繰り返し観測されており、いずれも対象 slice の変更範囲外・並列負荷起因と判断され、単独・再実行で pass する記録が残る。`mise run fmt-check` が対象外の pre-existing rustfmt drift（`crates/synvelis-library/tests/vcs03_git_provider.rs`）で fail した例では、本 slice 起因でないと切り分けて記録する運用が取られた。

### 既知の制限と文書の不整合

L1 wire report verb の実値は pin `b02942c2` が未対応で NotRun である。attached instance の param 列挙 API も pinned ruviform に無く caller 登録 catalog で代替した。これらは pin 更新待ちの upstream 不足として ruviform 側 Issue に紐付けられている（synvelis#101、ruviform#253/#307/#313/#314）。GUI・実 plugin・実 OS・wire-schema L1・AudioLens・consumer・実 RT/device の検証は全 slice で明示的に外部 gate/NotRun であり、headless green を製品受入に読み替えない。

`RelationIndex::build` は publish ごとの O(N) 再構築で、RT 経路ではなく UI read model 用途で許容と記録された。`detect_conflicts` は O(regions²) で大規模化に index が必要である。recording の applied registry は in-memory cap 64 で crash 復旧・永続 registry は synvelis#12 へ deferred、ledger revision ↔ dispatch ordinal の 1:1 仮定は upstream `record_plan` 規約依存である。`param_defunct`・`observe` の plan 鮮度は orchestration 境界の caller gate に依存し `eval_event` 単体では再検査しない設計で、`EvalPlan::is_stale` の recompile-compare は check ごと O(compile) で最適化 deferred である。

文書の不整合も記録されている。front matter の status は実態とずれることがあり、task 文書が `in-progress` のまま対応 IR が `completed` の例、plan が `approved` のまま実装完了済みの例が複数ある。PC-C3 RT trace adapter では対応 task 文書が `in-progress` で steps 2–6 が未チェックのままだが、実装記録は `completed` で 6 test pass・rt alloc 0/0 を記録しており、後続 slice も trace adapter の存在を前提に進んでいる。task status の更新漏れと見られるが、文書上は未解決の不整合として記録する。PC-C6 でも `pc-c6-task1-oracle-rel-tr`・`pc-c6-task1-oracle-rows` の task 文書が `in-progress` のままだが対応 IR は `completed` で全 test pass を記録しており、`Done When` 全チェック済み・Evidence 節ありの task が status 未更新の例である。ほぼ全 plan が `approved` のまま（PC-A plan のみ `completed`）、PC-C4-06 plan の Verification チェックボックスは未チェックのまま task/IR が完了、webview composition task は `Done When` 全チェック済みだが Verification 節が未チェックという status 運用の不統一もある。完了度は status ではなく IR 本文と検証記録で判断する必要がある。

schema 版の記述齟齬も未解決のまま残る。PC-B 残件（synvelis#64）の実装記録はベース `origin/main = 26ad0b8` を「`SCHEMA_VERSION=3`、PC-B #43 = b609b39、ED-B #65 = 26ad0b8 着地済み」と記述するが、PC-B（synvelis#43）の記録は schema 3→4 bump を報告している。同一ベースで schema 3 と 4 が矛盾して見える可能性があるが、記録の文脈（#64 の作業基点と PC-B merge の前後関係）が文書からは一意に確定できず、推測で解消しない。Ruviform pin SHA の多様性（94c5bf8→fdd65da9、258df5ae、b02942c2、0f5b5737、fd1d4d5f、9865cd57 等）は branch-tracking git 依存の時系列差であり矛盾ではないが、文書をまたいで同一値を期待すると誤読する。各値は当該 slice 時点の履歴的事実である。

## 適用対象

この知識は Synvelis のパラメータ制御・自動化・録音・observation 系サブシステム全体に適用される。具体的には synvelis-project（`ControlModel`・`BeatPoint`/`BeatRange`・`EvalPlan`・`ControlTrace`・`RelationIndex`・schema migration・`StablePluginInstanceId`）、synvelis-workspace（`ControlSession`・recording transaction・`relation_ui` read model・gesture fixation）、synvelis-player（`ParamDispatch`・`DispatchTraceAdapter`・`PluginNode`・driver 配線・`TransportMirror`）、synvelis-app（`syn.control.*` op surface・`compose_product_routing`・`ui_pump` drain・`PluginReportDrain`）、synvelis-library（`std::env` 非接触契約）にまたがる。Ruviform 側の契約（`ParamInfo`/`ParamFlags`・`offline_support`・`record_plan` 規約・wire report verb・attached instance param 列挙）への依存と upstream 不足（ruviform#219/#253/#294/#307/#313/#314）も本知識の適用範囲に含まれる。

## 出処

synvelis#40（PC-A 制御モデル基盤）、synvelis#43（PC-B dispatch / note-scope / RT / offline）、synvelis#64（PC-B 残件 driver 配線）、synvelis#98（PC-C1 録音トランザクション）、synvelis#99（PC-C2 relation index）、synvelis#100（PC-C3 control trace）、synvelis#101（PC-C4 plugin binding / observation / app composition）、synvelis#102（PC-C5 UI slice）、synvelis#103（PC-C6 独立 oracle / 受入台帳）から抽出した。関連 Issue として synvelis#6（conflict 意味論の合意）、synvelis#12（applied registry の永続化 deferred）、synvelis#28・synvelis#114・synvelis#176（外部 gate/consumer）、synvelis#230（`check-ruviform-rev` の stale vendor 検出）、ruviform#219・ruviform#253・ruviform#294・ruviform#307・ruviform#313・ruviform#314（upstream 不足の pin 更新待ち）がある。
