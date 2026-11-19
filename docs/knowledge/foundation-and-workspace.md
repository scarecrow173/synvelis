---
id: "KNOW-hvbqpffNej7r6789GCJqNK"
type: "knowledge"
status: "draft"
lifecycle: "draft"
knowledge_kind: "architecture"
title: "プロジェクト基盤とワークスペース"
created: "2026-11-19"
updated: "2026-11-19"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/1"
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/3"
    - "https://github.com/scarecrow173/synvelis/issues/5"
    - "https://github.com/scarecrow173/synvelis/issues/8"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/13"
    - "https://github.com/scarecrow173/synvelis/issues/14"
    - "https://github.com/scarecrow173/synvelis/issues/15"
    - "https://github.com/scarecrow173/synvelis/issues/16"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/36"
    - "https://github.com/scarecrow173/synvelis/issues/44"
    - "https://github.com/scarecrow173/synvelis/issues/230"
  informs: []
  explains: []
  supersedes: []
---

# プロジェクト基盤とワークスペース

## 概要

Synvelis の基盤・ワークスペース系スライスから得られた設計上の決定と運用上の教訓をまとめる。対象は、文書と証拠の作法、Rust workspace 規約、Project/Asset/Persistence の契約、Recording Foundation（VS-A〜D）、Workspace Model/UX、Context Inspector、DAW ワークフロー、Execution Plan、AI-Native Baseline である（synvelis#1/#2/#3/#5/#8/#10/#13/#14/#15/#16/#17/#23/#36/#44）。

共通する原則は三つある。第一に、ProjectController、ProjectSession、LinkGroupManager、CommandRouter のような権威ある境界へ責務を集約し、暗黙の fallback や二重の正本を作らない。第二に、計画・未実施・NotRun・Deferred を実績と区別し、headless の成功を GUI-L2 や EXT-L3 の Pass に繰り上げない。第三に、文書 status ではなく実装記録と証拠台帳を完了度の正本とする。文書間の食い違いが複数観測されており、実装と照合しない記述はそのまま信用しない（synvelis#13/#15/#16/#23/#44）。

## 詳細

### 文書運用と証拠の作法

canonical 文書鎖は spec→design→plan→task→test-spec→impl record/experiment record の 7 文書と slice ごとの evidence 台帳で構成するのが本リポジトリの標準である。front matter の ID は `<PREFIX>-<22 文字 Base62>` の形式で採番し、文書種別ごとに SPEC-/DESIGN-/PLAN-/TASK-/TSPEC-/IMPL-/EXP- を使う（synvelis#2/#3/#5/#8/#10/#17/#23/#44）。

文書作業 slice の典型的なスコープは「文書鎖と evidence 台帳の整備」に限定される。`crates/`、`Cargo.toml`/`Cargo.lock`、`mise.toml`、CI、Ruviform を変更せず、実装は別 Issue へ委譲する。文書の役割は、どの実装 Issue や fixture がどの要件を満たすかの写像と、証拠語彙の固定に留める（synvelis#1/#3/#5/#8/#17/#23/#44）。docs-only slice は Issue 1 件につき task 1 件という粒度で運用され、実装系の細分化 task 群とは構造が異なる（synvelis#1/#2/#3/#5/#8/#10/#17/#23/#44）。

task 文書は固定テンプレート `## Work`/`## Classification`（`normal-plan-task`）/`## Upstream Documents`/`## Dependencies`/`## Affected Files`/`## Done When`/`## Verification` で構成され、`Done When` は検証可能な条件で書く。task の `relations` は `implements`・`depends-on`・`verified-by` を持ち、実装系 task でも `implemented-by` は空のまま残ることが多い。plan は承認済み計画であり実装完了を意味せず、IR が実績を記録するまで計画のままである。実装系 slice の task 分解は「土台→中核機構→統合 op→PoC 実演→受入 test→検証+IR」の 6 task 構成が VS-A〜D・WS-A で共通である（synvelis#13〜#16/#36）。

document index は生成物であり手編集しない。実装系の experiment record は `.jsonl` を対象とする index 規約がある一方、実験イベントを持たない文書作業では `.md` 形式の検討記録が repo 慣行として採用されている。許可範囲外に生成された `.jsonl` は削除する（synvelis#1/#8/#10/#44）。`route_graph.js` は repository 全体の pre-existing broken relation により fail-closed で `blocked` を返す baseline 状態が繰り返し観測されている。範囲外の修復はせず、宣言済み文書順を手動で踏襲する運用が各 slice で一貫している（synvelis#1/#2/#3/#5/#8/#10/#17/#23/#44）。

review-protocol は協調 Reviewer と敵対 Reviewer の独立 pass、Validator、Judge の順で進み、通常 1 サイクルで終える。CONFIRMED P0/P1 blocker が残らないことを、Worker 契約上の承認代替として扱う運用が確立している。人間承認を捏造したのではなく、coordinator 契約に基づく手続きであると明記される（synvelis#3/#8 ほか）。P2/P3 の指摘も必要なものは直し、確定化判断は Decision Log に記録する。実例として workspace-ux-scenarios の C1〜C6（Window 閉鎖 Undo 粒度、Follow 元 View 喪失時の Inspector 挙動、異 DPI ショートカット、空 Sub Window の自動閉鎖、全 View 閉鎖時のアプリ終了、復帰経路の曖昧さ）を D21〜D24 で確定化した（synvelis#3）。

証拠の判定語彙は検証・受入方針 §3 の `Pass`/`Fail`/`NotRun`/`Unsupported` と、OS 検証レベル OS-L1/GUI-L2/EXT-L3、要求状態 Required/Deferred/NotApplicable の組合せを正本とする（synvelis#23）。要求状態と実行結果は別軸・別欄で記録し、未実施入口は `NotRun`/`Unsupported`/`Deferred` として記録する。`Pass` は実測成功に限る。headless・Fake Driver・`gpui-test`/`webview`/Browser の成功を GUI-L2/EXT-L3 の Pass に繰り上げず、capability 分離を evidence 台帳に明記する（synvelis#23）。

判定語彙は slice 間で完全には統一されていない。ai-native-baseline は headless PASS を `Pass(部分)` と記録したが、execution-plan は中間状態を導入せず要求状態×実行結果の 2 軸と `NotRequired` のみを採用した。context-inspector は `Pass (headless)` と `NotRun`/`Partial` を併記した。新規台帳では採用語彙を先に確認する（synvelis#44 vs #23 vs #5）。

「fixture 設定済み・当該 slice 未実走」と「構成済みテストなし=NotRun」を厳密に区別する。先行 slice の実測は SHA・日時付きの証拠を参照し、再実行したかのように見せない。merge 後対象 SHA の CI 証拠を統合受入とし、nearest-SHA 証拠を対象 SHA 証拠と混同しない（synvelis#17）。

front matter の status は実態とずれることがある。実装記録が `completed` でも対応する plan/task が `in-progress` や `todo` のまま残る例が複数ある。文書の完了度は status ではなく実装記録本文と evidence で判断する（synvelis#13/#15/#16/#23/#36）。task の `Affected Files` は計画時点の概略であり、実際に触れたファイル名と一致しないことがある。正本は実装記録の「変更内容/検証」節である（synvelis#13/#15/#16）。

`mise run fmt-check` は docs-only slice の正規検証入口であり、docs slice では実装 crate の check/lint/test を対象外とする。対象 SHA の CI 証拠は merge 後 CI/Coordinator に委譲する。各 slice の共通終了手順は `git fetch origin && git merge origin/main`、生成物（docs index・Cargo.lock）の再生成、Conventional Commits、push/PR である。index 競合は merge 後の index 再生成で解消し、検証を流用できる（synvelis#1/#10/#17/#23）。`doc_status lint` の `traceability-missing-upstream` warning は Issue 起点 spec に共通の既存パターンとして記録対象であり、repo 全体の baseline finding と新規文書起因を区別する（synvelis#1/#3/#36）。

skill script の参照パスは `.agents/skills/` と `.claude/skills/` に混在する。環境差か移行途中かは不明だが、コマンド引用時は実在パスを確認する（synvelis#2）。新規製品トレードオフ、契約破壊、定量 SLO、OS サポート宣言は「提案/決定待ち」に分離し、人間承認を捏造しない（synvelis#3/#8）。

### Rust workspace と Project/Persistence の契約

Cargo workspace 規約は `resolver = "2"`、`edition = "2024"`、`rust-version = "1.89"`（MSRV。CI pin は mise.ci.toml の 1.99.0）、`publish = false`、`[workspace.lints]` は ruviform 規約に合わせる。`unsafe_code = "forbid"`、clippy `all`/`pedantic` は warn とし、`lint` の `-D warnings` で gate する（synvelis#13）。

Ruviform 依存は既定ブランチ追従の git 依存とし、特定 SHA の恒久的ハードコードと vendor コピーは禁止する。解決 revision は `Cargo.lock` に記録し、`Cargo.lock` は再現性のため追跡する。`ui/vendor/ruviform` は gitignore された生成物で、lock rev が変わる変更の取り込み後は `mise run sync-ui-deps` で再同期する。`check-ruviform-rev` の REV 不一致は stale vendor を検出する fail-closed であり、復旧は `sync-ui-deps` の再実行である（synvelis#13/#36/#230、リポジトリ規約）。

`unsafe_code = "forbid"` 下での有界バッファは `BoundedVec<T>` で実現する。構築時に 1 回だけ `Vec::with_capacity` する capacity 固定 Vec と head カーソルで RT の memmove を回避し、満杯時は `BoundedFull(T)` で payload を返す。`with_capacity(0)` は非割当てとして engine scratch の退避に利用できる（synvelis#14）。

ruviform-media-core の `Tick`/`TickDuration` は unchecked `Add`/`Sub` のみ公開するため、Synvelis 側の時間演算はすべて `checked_add`/`checked_sub` で行い、同演算子の直接利用は lint 対象である（synvelis#13、リポジトリ規約）。ruviform `TempoMap` は fields が private で serde もないため、Synvelis は `TempoMapData`/`TempoMapRef`/`ProjectPpqn`（既定 960）の DTO 層を持ち、`TempoMapBuilder` 経由で ruviform `TempoMap` を rebuild する（synvelis#13）。tempo 変換は ruviform `tick_to_media`（NearestEven）の丸め規則に依存するため、試験 tempo は整数 rate を選び丸め差を排除する（synvelis#14）。

`ProjectController::apply` は `O: Operation` に対して generic（controller 自体は non-generic）にすることで、`synvelis-recording` が独自 op 型を循環クレート依存なしに供給できる（synvelis#13）。apply の手順は expected-revision 検査、全検証、clone-mutate-once、live 差替え、undo への prior snapshot push、redo クリア、Live revision +1 の原子的手順である（synvelis#13）。`Command<O>` は `expected_revision` と `op` のみを持ち `ProjectId` を持たないのが実装事実であり、spec/design を実装に合わせて訂正した前例がある（synvelis#10）。

undo/redo は snapshot 復元方式で、復元時も常に新しい Live revision を mint し Saved state を触らない。revision 会計は「project mutation の回数」であり、view-local op（select/inspect/transport.play/dock churn）は寄与しない。一方、undo/redo は project history 操作なので revision と graph_builds を各 +1 進める。この会計を誤るとテストが FAIL する実例がある（synvelis#13/#2）。

永続化は `DirectoryBundleStore`（PoC 形式で最終形式ではない）が `snapshots/r<N>.json` を tmp+fsync+rename で書き、`HEAD` を最後に同じ atomic rename で publish する last-write HEAD 方式である。snapshot envelope は `schema_version` と全 persisted struct の `deny_unknown_fields`、canonical serde_json を持つ。load は HEAD→parse→schema version 検査→参照検証を経てから差替え、失敗時は Live state を保持する（synvelis#13/#17）。`HEAD.lock`+expected-head CAS は協調 `save_cas` caller のみを直列化する advisory 機構であり、`save()` 直接呼出・lock 非取得・外部プロセス変更は保証外である。検出は次回 CAS conflict と `head()` 再読込の正直報告に依存し、この境界を拡張しない（synvelis#17）。FS の Durable（publish `Ok` = durability+可視性）と Application Commit（HEAD repoint = commit 点）は別 receipt・別保証として記録し、片方の成功を他方の証明にしない（synvelis#17）。

typed stable ID は u128 newtype 12 種（Project/Track/Lane/Clip/Source/Asset/Take/RecordingSession/RecordingPass/CaptureStream/CaptureSegment/ClockEpoch）を macro 生成し、serde は `<kind>-<32hex>` の tagged 文字列である。`compile_fail` doctest で kind 間の型分離を証明する（synvelis#13）。ID テキスト parse は `rsplit_once('-')` を使う。`clock-epoch`/`recording-session` 等の複数ハイフン kind の round-trip に必須であり、単純 split で実際の PoC reload 失敗をテストが検出・修正した（synvelis#13）。`SourceState` は厳密な 1 段階前進遷移で、Imported→Saved の skip を拒否する。`Range<F>` は `[start,end)` の半開で inversion を構築時に拒否し、empty は許可する。serialize は raw `start`/`end` で、deserialization は `Project::validate_references` で再検証する（synvelis#13）。

### Recording Foundation の構造

VS-A は workspace skeleton と Project model を作る。3 クレート `synvelis-project`/`synvelis-recording`/`recording-poc` を作り、依存方向は `recording-poc → synvelis-recording → synvelis-project` である。Project model は checked time 型、半開 Range、tempo DTO、POD を持ち、`ProjectController`/`ProjectOp`/`DirectoryBundleStore` で stale-revision 拒否、invalid op 原子性、undo/redo、save/reload、malformed JSON/unknown field/unsupported schema/壊れた参照の拒否、失敗 save の prior HEAD 保持、失敗 load の Live state 保持を検証する（synvelis#13）。`recording-poc` は固定 ID generator seed・固定データ・BTreeMap 順序の安定出力で、2 回実行の stdout byte-identical を `poc_determinism`/`vsd_process`/`vsc_process` テストで検証する。タイムスタンプ・乱数・filesystem 順序を出力に含めない（synvelis#13〜#16）。

VS-B は capture engine の RT 契約を作る。`CaptureEngine::process` は非 alloc/lock/I/O/無界であり、`stats_alloc` を dev-dep にして専用 test binary（単一テスト・warm-up 済み）で allocations/deallocations = 0 を計測する。並列テスト汚染を避けるため専用バイナリが必須である（synvelis#14）。`FaultLatch` は `AtomicU32` の side channel（bit 定数 10 種）で、満杯 Queue 非依存に fault を伝播する。失敗 push 返却 chunk と部分書込み chunk は `retained` へ move して RT dealloc を回避する（synvelis#14）。Gate Scheduler は gate を pass 基準 frame offset の template 列へコンパイル（非 RT）し、tempo 境界で分割・Loop pass は同一 template の平行移動で展開する。RT 側 `evaluate_gate` は engine 所有 scratch へ `CapturePiece` を書き出し、scheduler は無状態化して生成カーソルを `GateSchedule` 側へ持たせて stream ごと独立化した（synvelis#14）。1 block > chunk_frames は preflight が `max_block_frames` で排除し、超過時は `PCM_OVERFLOW` で fail-stop する。VS-B では複数 chunk 分割は持ち越しである（synvelis#14）。VS-B は Mock-first 証明であり、ruviform #187（実 Device Input）/#190（Span Scheduler）は前提にしない。実数値（chunk/pool/queue/span/lookahead 容量）はテスト注入値で先行し、PoC 計測で確定する Deferred 方針である（synvelis#14）。

VS-C は staging・journal・recovery の durability を作る。Written / Validated / Durable Checkpoint を分離し、`media_len`（実 file 書込み cursor）とは別に `journaled_media_len`/`journaled_digest`/`journaled_source_frames` を journal append 成功時のみ commit する「journaled frontier」方式を取る。これにより書込み途中失敗の chunk は durable frontier を汚染しない。`verify_chunk` が media/journal 成功前に cursor を commit する実装欠陥は review で検出され、この分離で修正された（synvelis#15）。checkpoint は media `sync_data`→Checkpoint record→journal `sync_data`→dir sync の順である。dir sync capability は環境依存（Windows では unsupported 観測）として manifest に記録し、保証を過剰主張しない（synvelis#15）。

VS-C の journal は 16 byte header（magic/version/kind/seq/payload_len）+ payload + CRC32 の framed record codec で、Chunk/Checkpoint/Finalize の 3 kind を持つ。replay の `StopReason` は CleanEnd/TornTail/BadMagic/BadVersion/BadLength/BadChecksum/SequenceGap であり、破損以降の byte は変更・削除しない（synvelis#15）。CRC32 は in-tree IEEE 実装で新規外部依存を持たない（synvelis#15）。`finalized.json` が finalized の authority で、journal の Finalize record は診断用である。二重 finalize 防止は marker 存在確認が担い、Finalize record 追記失敗は notes 記録のみで ref 公開を取り消さない（synvelis#15）。`recover_finalize` は checkpoint を新→旧順に再検証し最初に通る frontier を採用するため、最新 checkpoint の不整合が先行する整合 checkpoint を覆い隠さない。journal tail が clean でない場合は Finalize record を追記しない（synvelis#15）。fault injection は `StorageWriter` trait 境界で本番 File 経路と同一コードを通す（FailWrite/ShortWrite/FailSync/ByteLimit/TruncateAfter/JournalCorrupt）。Process Kill 試験は子 process を N record 後に異常終了させる方式で、OS kill 非依存の決定性を確保する（synvelis#15）。

VS-D は commit 冪等と staging→project 反映を作る。authority は Project 永続化対象の `applied_commits` registry（`BTreeMap<RecordingSessionId, AppliedCommit{content_hash, revision}>`、serde default で旧 snapshot 読み込み維持）であり、staging の `commit.json` marker は補助記録である（synvelis#16）。`commit-manifest.json` で Stable ID を事前確保し、`draw_raw_id` で generator から raw u128 を引き `used_id_watermark` を巻き上げる。`prepare_manifest` は「probe payload（ID 文字列を除く構造 fingerprint）照合」の二段構成である（synvelis#16）。同一内容なら再採番せず再利用し、不整合なら予約 ID は一度も Project に入っていないため破棄して再構築する。payload 整合と Stable ID 再利用を分離することで、commit 後 retry（lane 非空化後）でも manifest を正しく再利用できる（synvelis#16）。clip 生成可否（空 lane 判定）は commit 時点の project 状態由来なので manifest fingerprint から外し、clip↔segment の値整合（tick_in/out・source_offset）だけを別チェックとした（synvelis#16）。

`RecordingOp::CommitRecording` は validate（schema/hash/session 状態・registry 未登録・全 ID 一意性・pass index/stream entry 一意性・参照整合・負値/反転/overflow しない範囲計算、部分適用を型で防止）→ apply（asset→source Verified→Committed→pass→segment→take→clip→session Finalized→registry→reseed を scratch clone 上で一括登録、失敗時は controller が partial apply を捨てる）の単一 mutation である（synvelis#16）。`StaleRevision` は最新 revision 取得で bounded retry（上限 8）の recoverable conflict である（synvelis#16）。RT/engine 側 fault は shared `FaultLatch` にしか残らず writer state に反映されないため、session スコープの `capture-fault.json` marker で運び全 stream を recoverable 分離する。per-stream の writer 個別失敗は stream 単位で分離済みであり、capture fault は take 全体を中断する session 単位の出来事である（synvelis#16）。commit 対象 0 の session（全 stream partial/faulted）は mutation を発行せず report のみ返す。Partial media の明示 commit・session 追記 commit・media GC は後続 Issue である。`commit_status` の IntegrityConflict 検出は content hash 比較のみで field 単位差分報告はしない。`payload_hash` は非鍵付き CRC ベースの accidental-corruption 検出であり、暗号的 integrity ではない（synvelis#16）。

### Workspace session・link・window の構造

`ProjectSession` が唯一の `ProjectController` を包摂し、二重 Store を禁止する。`resolve_target` は explicit targets→origin view の selection snapshot→origin view の現行 selection のみを読み、他 view/window の selection への暗黙 fallback は構造的に存在しない（synvelis#36）。`LinkGroupManager::deliver` は受信側のみで再発信せず、`(receiver, event_id)` dedup と Group 越境拒否を行う。mapping 不能な selection は publish 自体しない（synvelis#36）。

`WindowId` は host 発行かつ `LayoutSnapshot` に key がないため、永続化では window title を capture 順序で `windows[i]` と対応付ける。`detach_to_new_window` は create→register→move の順であり、未登録 window は begin_move 不可である。create 失敗は何も汚さず、move 失敗時は作った window を破棄して孤児を残さない（synvelis#36）。serde の落とし穴として、`SemanticTarget` は externally-tagged serde を使う。内部 tag は newtype variant の string payload と両立しない（テストで発見・修正）（synvelis#36）。`ShellWindowHost` は single-window（`create_window`→`BackendUnavailable`）のままであり、multi-window の意味論は `FakeWindowHost`/`HeadlessWindowHost`/`FakeDriver` の headless oracle のみで検証する。製品配線は #113/#175 の gate である（synvelis#36）。window-owned job cancel は `CallOutcome::Job` を返す op が無く headless fixture を作れないため NotRun とし、`UiRuntime` 契約（close/destroy は job を自動 cancel しない）を設計不変条件として記録する（synvelis#36/#2）。

WS-A の persist は `WorkspaceSnapshot` envelope に `LayoutSnapshot` + window titles（順序対応）+ view descriptors/state + link assignments + schema version 検証を持ち、破損 layout は `restore_or_default` の安全既定へ戻す。restore 後は `next_view` を復元済み最大 id に合わせ、post-restore spawn の `DuplicateView` 衝突を review で検出・修正した（synvelis#36）。undo/redo は `ProjectSession` 経由で `EngineStub` の graph build を呼び、project 変更→graph build の不変条件を保つ（synvelis#36）。

Context Inspector の `ContextInspectionTarget` は既存 entity 参照（`SemanticTarget`/`SignalFlowEntityRef`/`ControlEntityRef`/`SemanticProbePoint`）の tagged union であり、新 domain id を発明しない。`SemanticTarget` に写像不能な kind は fail-closed で Explore/明示 inspect 導線に限定する（synvelis#5）。link event が運べるのは `map_to_track` 適用後の `SemanticTarget::Track` のみで、Clip/Lane→owning Track、Source→`None` で event 不発である。Bus/Strip/Processor/Parameter/Connection/Send/Port/Probe は `EditingSelection`/`SemanticTarget` の対象外である（synvelis#5）。したがって Follow retarget は Track 粒度であり、「link で全 kind に到達できる」とは設計しない（synvelis#5）。Explore→Follow 復帰は `link.rs` の共有 semantics（Follow mode のみ retarget し Explore/Pinned を動かさない）を変更せず、Inspector 側のイベント消費規則として定義し、実装経路は #112/#108 に委譲する。「契約は固定・実装経路は後続」の分離パターンである（synvelis#5）。一括編集は絶対値・発行時全対象 snapshot・単一 controller preflight・all-or-nothing であり、相対変更・部分適用は後続である。Probe は `ProbeSubscriptionKey`/`ProbePrincipal` lease を再利用し、Off 既定・View 閉鎖で session hold を維持する（synvelis#5）。固定 5 タブ・自動 Monitor・Explore 履歴永続化・DAW 語彙の Ruviform 持ち込みは不採用/保留であり、既存 `InspectionMode`/`FollowSource`/explore stack・per-view `Option` state + tolerant decode の確立済み契約を再利用し、新しい並行機構を作らない（synvelis#5）。

Workspace UX 正本化は「既存 headless 契約を前提参照し UX 操作面のみ新規に書く」方針である。統合設計は論理構造・状態正本・Command 契約の正本であり、ユーザー操作 UX を混在させると責務境界が崩れるため、UX を統合設計へ追記する案は棄却した（synvelis#3）。DAW ワークフローは Gate A backbone を GA-01〜08（Track 作成→Import/Record→非破壊編集→Mixer/Transport→Save/Reload→Export→独立検証）に分解し、WF-SCN-MIDI/AUDIO/MIX が backbone を共有する。差分は素材取得と編集対象のみである。Signal Flow/Inspector/Version Diff は spine から外した文脈保持の任意 detour である（synvelis#8）。

Workspace UX の未決定事項（Drop zone 寸法・ドラッグ閾値・Split 最小サイズ・Explore 履歴深さ・閉鎖 Window 復元履歴保持期間・Layout Undo 履歴範囲）は実機/UI PoC・#5・#177 運用で確定する。DAW ワークフローの Keyboard 具体的キー割当は #112、Tool/Snap 数値・Take/Comp Editor UI は #11/#12、実機受入は #114（GUI-L2）/#176（EXT-L3）に委譲し、Oracle は文書上の期待結果として定義する（synvelis#3/#8）。

Execution Plan は全 workstream（RCP-00〜07/F01・F0/E2E-L/R/X・SF/CTL・Recording・TIM・GUI・GW・VCS/ALIB/OBS・CB/AUTX・CI）を lane node + Issue 番号併記の単一非巡回 DAG に統合する（synvelis#23）。全 Issue 個別 node 化は可読性を損なうため棄却した。edge は Start/Integration/Acceptance の 3 型で、Start は「契約/型/Fixture が利用可能」、Integration は「機能接続時の capability」、Acceptance は「最終受入 Gate の独立 Oracle/実機証拠」と定義する（synvelis#23）。参照先 Issue の全 Close を Start 条件にせず（例: #103 Oracle PASS は #133 の acceptance 条件であり start 条件ではない）、Gate node は Start edge を持たず Acceptance→Start の逆流を禁止する（synvelis#23）。新 CI Workflow・専用 Runner の追加は「新 CI Workflow を増設しない」要求に反するため棄却し、Daily（変更検知）+ 手動 Full CI のみを利用する（synvelis#23）。Windows では利用機能の L2/L3 のみ Required、macOS/Linux は L1 のみで L2/L3 は利用要件発生まで Deferred とし、Browser/Fake 成功を Native 合格にしない。RCP-F01・AUT-F・RCD-F・CTL-F・TIM-F 系は初期 Gate 非必須の後続として Open のまま管理する（synvelis#23）。

AI-Native Baseline は Issue #44 の AIB-001〜007/AHG-001〜011 をそのまま要件 ID に採用し、受入 8 件を AIB-AC-01〜08 に対応付ける（synvelis#44）。Owner 境界を Synvelis（DAW 意味論・AI ポリシー・Grant/Approval 権威・ProjectController・MCP Adapter）/Ruviform（generic Graph/Time/Audio/Plugin/Media/Offline/Job/Observability、汎用 Gateway/認証 Provider を Synvelis 専用として重複実装しない）/Agent Reasoning Runtime（交換可能）/#45（型契約）/#46（実装/製品受入）/#114（実機 lane）に固定する（synvelis#44）。未決定事項（Grant UI・Context 取得対象・Privacy/Retention・Provider 契約・Agent 自律性・具体 Budget・MCP Tool Schema/Transport・各受入試験の実証結果）は「独立した受入 Gate ではない」として Deferred Design Concerns に集約し、受入 Gate 化による実装の過剰拘束を避ける（synvelis#44）。合意済み事項と未決定事項の分離を spec/design/evidence で必須化する（AIB-T-R-05）（synvelis#44）。

Project/Asset/Persistence（synvelis#17）は新規 port/trait を導入しない契約写像 slice である。既存 `ProjectStore`/`ProjectController`/`CommitIdentity`/`MediaReferenceProbe` が契約を既にコード化しており、重複抽象化は YAGNI/DRY に反する（synvelis#17）。HEAD CAS の Ruviform 移譲は syn-io-01 境界 spec で Synvelis 責務と確定済みのため棄却し、PAP architecture 本文への契約追記も既存 refines/derives-from 関係を持つ多数文書の前提を揺らがせるため棄却した。写像は別文書 + case-ledger が適切である（synvelis#17）。`PROJ-RCP-01` は Project 側境界のみ固定し、Saved HEAD 確定が Render Job 完了を待たないこと、Artifact 反映が Project CAS/Undo/HEAD を暗黙書換えしないことを定める。Artifact attach 統合の実装・fixture は RCP 系（#290〜#297・全て OPEN）に委譲する（synvelis#17）。

Project Model（synvelis#10）は既存実装を正本として参照し再定義しない方針である。`synvelis-project` の module/test を contract の根拠とし、設計書は概念/スキーマ契約のみ固定する。物理 serializer・schema version=13 は既存実装の確立契約を参照する（synvelis#10）。新規 test を追加せず、既存 fixture が oracle を満たす範囲は実測、満たさない範囲は NotRun/Deferred で gap を隠さない（synvelis#10）。製品語彙（Configured/Active/Observed/Unknown）と実装語彙（PC-C3 の Configured/Prepared/Applied/Reported/Observed + Missing/Unsupported/Dropped/Expired）は 1:1 で対応しない。「Active」は実装側で Prepared/Applied 系に分解され、製品語彙は断定を禁じる側の語彙である。design の対応表で吸収し既存 docs を書き換えない（synvelis#1）。

### 検証手法と教訓

受入条件を Case ID（WSM-T-*/VS-001〜015/SV17-C01〜C13/PM-T-*/CI-01〜13/AIB-AC-01〜08/WF-AC-01〜10）へ分解し、既存 fixture への写像を case-ledger で台帳化する。coverage gap（例: WSM-T-08「再生中の churn」）だけ最小 test を追加し、大規模 fixture 追加や contract の hard-code 化を避ける（synvelis#2/#10/#17/#44）。

review-protocol が実バグを検出する実績がある。ID parse の `rsplit_once` 修正、SourceState 遷移 skip、VS-C の journaled frontier 前 commit（durability 欠陥）、VS-D の staging drift で AlreadyApplied 不可達・stale manifest deadlock・原子公開後 dir sync 欠落・FinalizeSession commit 可否・manifest 内重複/負値検証不足など P1/P2 を修正した。adversarial pass + 実コード照合の組合せが有効である（synvelis#13/#15/#16/#5）。検証時の教訓として、design の記述が実装とずれる例が複数あり（link event 粒度の誤記、`ClipPlayback`/`PlaybackMapping`/`Clip` の field 名、`Command` の `ProjectId` 有無、`SplitClip` の `DerivedFrom` 欠落、存在しないテスト名参照）、review で実コードと照合して修正した。文書は実装を正本として参照する姿勢が重要である（synvelis#5/#10）。

VS-D の `applied_commits` 追加は `#[serde(default)]` を付けて旧 snapshot の読み込みを維持する。`deny_unknown_fields` と併用可であり、RecordingRegistry への field 追加も同様に旧 VS-A 試験 snapshot が default で復元される（synvelis#16）。`mise run test-msrv`（rust 1.89 toolchain）は初回に toolchain download が必要で、環境に未導入の場合は未実施として正直に記録する（synvelis#13/#36）。clippy pedantic 対応では `too_many_lines` の allow を「variant ごとの flat match dispatch で分割するとかえって不明瞭」な箇所のみに限定し、17 件の pedantic/restriction 指摘（must_use・pass-by-value・identical match arms・clone_from・collapsible let-chains・doc backticks・then_some・unused self）を修正した実績がある（synvelis#13 task6）。x_e2e 系の既存 lint 失敗（`assert-is-empty`）は 1 行修正で解消できた。新規 slice が既存 lint 債務を踏んだ場合の最小修正例である（synvelis#36）。`mise.toml` の正規入口（check-ruviform-rev/check-headless-deps/check/lint/test/build/test-rt/gw10/check-app-native 系/check-ui 系/test-recording-real-*/ci-summary-gate 系/lint-actions/test-msrv 等）は OS Level・Gate/Feature・対象 SHA・結果状態へ対応づけて entry-gate 台帳に記録する運用である（synvelis#23）。

## 適用対象

この知識は、Synvelis の workspace 構成と Ruviform 依存運用、Project/Asset/Persistence の境界、Recording Foundation の実装と検証、Workspace Model/UX・Context Inspector の headless 契約、DAW ワークフローと Execution Plan の受入構造、AI-Native Baseline の Owner 境界に影響する。文書・evidence・review・CI 証拠の運用を扱う全スライスでも、完了度の判断と証拠語彙の選択に適用される。

## 出処

この知識は、Synvelis の基盤・ワークスペース系開発文書群から抽出した。主な出所は synvelis#1（Product Value Core）、synvelis#2（Workspace Model）、synvelis#3（Workspace UX Scenarios）、synvelis#5（Context Inspector）、synvelis#8（DAW Workflow）、synvelis#10（Project Model）、synvelis#13〜#16（VS-A〜D Recording Foundation）、synvelis#17（Project/Asset/Persistence）、synvelis#23（Execution Plan）、synvelis#36（WS-A Workspace 基盤）、synvelis#44（AI-Native Baseline）、および synvelis#230（ui vendor 同期規約）である。
