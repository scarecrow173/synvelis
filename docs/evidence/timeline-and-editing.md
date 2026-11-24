# タイムラインと編集モデル — 実行証拠

この文書は、Synvelis のタイムライン編集・時間系・再生/配送系で実際に実行された検証の記録をまとめる。対象は編集モデル headless 実装 slice（ED-A synvelis#38、ED-B synvelis#65、MD-B synvelis#50、MIDI Source synvelis#52）、時間/PPQN/SMF 実装（TIM-01 synvelis#125、TIM-02 synvelis#126）、process boundary・transport 配送・plugin state 実装（TIM-03 synvelis#127、TIM-04 synvelis#128）、および設計契約 docs-only slice（synvelis#11・#12・#4・#18・#19・#20）の文書検証である。canonical lane は `mise run <task>` であり、cargo 直接実行は補助証拠として区別される。実 GUI・実 OS・実 device・実 plugin の受入は全 slice で NotRun/Deferred と明示されており、headless/fake の成功を実機 Pass に昇格させていない。

## ED-A 編集モデル headless PoC（synvelis#38）

- 検証構造: 5 契約 ED-A-01..05 を `tests/ed_a.rs` の 6 test で検証。`FakeWindowHost` + semantic `HitProbe` 経路で pointer/gesture/selection を GUI なしに固定した。
- 結果: 全 `mise run` lane（fmt-check/check/lint/test/test-rt/test-msrv）PASS。ruviform rev `94c5bf8` 不変。workspace schema 1→2。`ed_a` 6 件 + `ws_a` 8 件を含む full workspace run で green。
- 未検証: 実 GUI/OS/device/plugin は明示的に未検証（headless のみ）。

## ED-B media-domain 編集（synvelis#65）

- 検証構造: ED-B-01..07 を `tests/ed_b.rs` の 20 test で検証。
- 結果: 全 lane green。review で CONFIRMED P1/P2 を修正: StartTrim の二重 floor、EndTrim の snap 基準（動く端）、marquee の exact rational overlap、media_rescale の saturation。既存 `pc_b_rt` の `too_many_lines` lint は `#[expect]` で対処。
- 制限: source-length validation・GUI 挙動は制限/deferred として記録。

## MD-B effective comp lowering（synvelis#50）

- 検証構造: player 4 + render 2 + project 3 + workspace 2 の追加 test。
- 結果: 全 lane green。v1/v2 snapshot は honest reject（migration tool なし）。

## MIDI Source 編集（synvelis#52）

- 検証構造: `tests/midi_source.rs` 9 件追加、36 スイート全緑。schema v4→v5。
- review 修正: P1 — `sources`∩`midi_sources` 非交差 oracle 欠如、clip-local notes の map-key↔id 未検査、ID watermark load 未検査。P2 — Loop/Stretch/Absolute extent を Musical+Natural に制限。pre-existing gap も記録。

## TIM-01 configurable PPQN（synvelis#125）

- 初期 slice: `tim_01_ppqn.rs` 14 tests、全 lane（fmt-check/check/lint/test/test-rt/test-msrv）green。PPQN 自体は schema bump なし。
- app slice: `tim01_ppqn_app.rs` 11 tests。TDD で `unknown_op` 11 件 RED を確認後に実装 GREEN。前提条件 3 件・undo/redo・save/reload・engine revision 通知を fixture で証明。
- punch-mark 残件 slice: `tim_01_ppqn.rs` 25 tests。6 PPQN 行列 Save/Reload/Undo/Redo・実 v5 snapshot 非破壊 migration・分母上限・同 tick site・保存対象外安全性・異 Division SMF import を検証。preview site_count 11→13（punch_in/punch_out 追加）。実 v5 snapshot は byte-identical migration。lint 初回失敗（`tim_01_ppqn.rs` の `doc_markdown`/`if-let`）を修正して green。
- preview slice: `tim_01_ppqn.rs` 20 tests + `tim01_ppqn_preview.rs` 6 tests。
- UI slice: `tim01_ppqn_ui.rs` 11 tests。select→preview→confirm/cancel・undo/redo 再同期・Snap/Grid tick verbatim・範囲外 reject を fixture で証明。
- flake: app workspace test で `synvelis-library` WebDAV fixture が Windows socket error 10053 で flake。単独再実行 42/42 pass、workspace 再実行で green → infrastructure flake として記録。

## TIM-02 SMF / player MIDI / cross-golden（synvelis#126）

- SMF core: `tim_02_smf.rs` 28 tests + `tim_02_midi_phrase.rs` 2 tests。schema v8→v9。138 test binary 全 ok、test-rt 129 ok、MSRV 138 ok。Format0/1 parse・複数 track/channel・PPQN mismatch 変換・tempo/CC64/event order・NoteOff→On・malformed/overflow/unsupported・Export→Reimport golden・chase/panic・immutable guard・schema v9 roundtrip を網羅。
- cross-golden: `tim_02_ppqn_smf.rs` 2 tests（Exact chain + Round chain）。`src/` 無変更で完結。`ConversionReport` 全 exact・`rounded_count=0`・`max_error=0`・`unsupported_events` 空。tempo/CC64 provenance が export→reparse→reimport で保持。
- player MIDI RT/Offline: `tim02_midi_playback` 10 tests + `tim02_midi_rt` 1 test。merged tree `ff33ffb` で全 lane 再実行 全 Pass。初回 run は `tim03_session_wiring` binary の alloc 失敗 abort（0xc0000409）と rustc LLVM OOM（exit 101）で transient 環境要因 → 単独再実行・全量再実行で非再現を確認。
- app op 配線 slice（`tim-02-app-ops-smf`）: `syn.project.import_smf`/`export_smf` を service op table→dispatch→handler→binding で配線（TIM-01 `set_ppqn` 前例）。`syn.app.capabilities` に `midi` 節を additive 追加（SMF=available・WinMM=implemented+実 port NotRun・macOS/Linux=unsupported・SMPTE/Format2/MIDI2=unsupported+reason）。`tim02_smf_app.rs` 14 tests（import/export/undo-redo/save-reload/typed error/capabilities/midi clip 証拠/muted clip）。clip→MIDI は plan（`plan_midi_clips`→`MidiClipPlan`）→compile（`compile_midi_comp`/`compile_midi_events`）→`MidiClipEvidence` まで。**plugin node への MIDI 配送は未配線** — `RoutingControlDispatch` midi seam + `routing_engine` plan merge（instrument=MIDI/processor=param の single-plan split）が必要で、Coordinator 確定により follow-up issue へ defer。`SkippedPlayback::MidiClip` 追加に伴い `routing_engine.rs`（`skipped_fault`→`None`、意図的 silence）と `export.rs`（`UnrenderableClip`）に compile-forced ripple を発生させた（lease 外、PR に明記）。外部 gate（実 OS backend・RF #629/#630 L3・AL f64 oracle）は全て NotRun/別 gate。旧物理 MIDI 3 ignored は歴史的 NotRun を維持。全 lane 実走結果は PR 本文 Verification 節に記録。
- verification slice: doc chain 閉鎖、status を実測整合（approved/done）、findingsBefore 644→639。

## TIM-03 process boundary / session / loop / driver（synvelis#127）

- task1: `tim03_process_boundary` 17 tests + `tim03_process_boundary_rt` 1 test（alloc=0）。`AttachDelivery` 投影。
- task2: `tim03_attach_wiring` 10 tests。Realtime Exact default・consent 付き Degraded clamp・OfflineExact certified gate・宣言 scoping・undeclared processor 後方互換を固定。
- task3: `tim03_request_plan` 10 tests。window/boundary 行列・同時境界・min/max legality・capability 3 分岐・stale provenance 全軸・foreign dispatch・誤配送 0・valid provenance 配送を固定。
- session→driver/routing wiring: `tim03_session_wiring` 10 tests。`tim03_request_plan` 回帰 + 新規 wiring + 既存 driver/routing test が pass。
- loop boundary: `tim03_loop_boundary` 7 tests + WrapSim unit 9 tests。導出 offset が実 scheduler の wrap frame（plugin `TransportInfo` position fold）と一致。E1–E9 fixture matrix を網羅。
- driver loop boundary: `tim03_driver_loop_boundary` 6 tests（DB-01..06）。quiesce 経路・seek・panic・resume を実測。当初 `[0, wrap_offset]` を期待したが offset 0 は oracle が applied set から除外する正規化のため `[wrap_offset]` のみを assert するよう修正。

## TIM-04 plugin state snapshot（synvelis#128）

- 検証構造: project `plugin_state` 14 tests + player `tim04_plugin_state` 9 tests + render `tim04_plugin_state` 5 tests。146 test binaries 0 failed。
- 結果: 全品質（Coordinated/Quiescent/BestEffort/Unsupported）と失敗 kind（Crash/Timeout/Permission/Corrupt/Unavailable）の typed 記録・Strict fail-closed・indeterminate reconcile・copy restore 品質維持を fixture で検証。

## docs-only 設計契約 slice の実績

- 共通構造: 各 slice は `spec → design → plan → task → test-spec → impl/ir + impl/exp` の 7 文書 + slice 別 evidence 台帳で構成され、受入条件は一意 ID で test-spec case・oracle・状態に写像される。検証 lane は文書系（fmt-check・doc-status audit/lint・impl audit・index apply・review-protocol）である。
- synvelis#11 Timeline Editing: 7 文書 + slice 別 evidence 台帳。TE-01〜14 × 既存被覆を `Pass (headless)`/`Partial`/`NotRun`/`Deferred`/`Unsupported` で分類。`mise run fmt-check` exit 0、index in-sync、review-protocol Confirmed P0/P1 = 0。Rust build/test は NotRun/NotApplicable。
- synvelis#12 §14/§15 Take Lane/Comp Editor: 8 新規ファイル + slice 別 evidence 台帳。REQ-TLC-01〜07・TL-AC-01〜12・契約 C1〜C7・owner-matrix AC 10 件全てを test-spec case へ対応付け。fmt-check・prek・doc-status audit/lint・impl audit・index apply 全て exit 0。新規 broken relation なし（orphan-navigation warning のみ、repo 全体の既存慣行と同型）。cargo 系・実 GUI/実 OS は NotRun/NotApplicable。
- synvelis#4 Signal Flow: 7 文書 + slice 別 evidence 台帳。SFD-R1〜R8 をカバーし Issue #4 受入 6 件に scenario ID 割当。evidence 台帳に設計根拠・代替比較・SF 系文書との整合確認を記録。`crates/`・manifest/lockfile/CI に差分なし。
- synvelis#18 Playback Graph: 7 文書 + slice 別 evidence 台帳。EX-001〜020 を既存 Synvelis/Ruviform headless 証拠へ写像。`Pass (partial)`・capability 別 NotRun/Unsupported・EX-017「設計記録」表記を使用。task/plan status は in-progress（後述の矛盾節）。
- synvelis#19 Musical Time: 7 文書 + slice 別 evidence 台帳。SV19-C01〜C08 を case ledger に写像し根拠 fixture・Expected/Observed・環境・SHA・Pass/Fail/NotRun/Unsupported/Deferred・丸め誤差を記録。fixture 帰属・型名・automation `BeatPoint` 分類・overflow fixture 過信表現を review で修正。
- synvelis#20 Transport Integration: 7 文書 + slice 別 evidence 台帳。TT-001〜030 を一意 ID・Owner・fixture/oracle・状態で写像。TT-003 SMF SMPTE division は #129 で Unsupported、plugin event offset 起点・`InBlockTransportSupport` Verified のみ・Offline Exact fail-closed・Realtime Degraded consent 必須を契約化。task/plan status は in-progress（後述）。

## 矛盾・異常・注意点

- **status 不整合**: Playback Graph・Transport Integration・TIM-03 loop boundary・TIM-01 PPQN の plan/task 文書は `in-progress` だが、対応する impl/exp 記録は `completed` で検証実績を記述している。Timeline Editing・Playback Graph の plan 文書の「全検証 lane 実行」checkbox も unchecked のまま。plan/task の status と実装記録の完了記述が食い違うため、plan status を実装完了の根拠にしてはいけない。
- **SCHEMA_VERSION の記述差**: musical-time 記録は現行 `SCHEMA_VERSION=13` と記述、初期 PPQN 記録は schema 9 を参照、SMF core は v8→v9、MIDI Source は v4→v5、ED-A workspace は 1→2。これらは slice ごとの時点差・対象 crate 差（project vs workspace）によるもので、単一の「現在値」として統一すべきではない。
- **graph router blocked**: 複数 slice で `route_graph.js` が pre-existing broken relation により `blocked`/`focus-required` を返す。範囲外として修復せず記録。
- **flaky/環境**: `synvelis-library` WebDAV `same_origin_redirect_is_followed` が Windows socket error 10053 で flake（TIM-01/TIM-02 で観測、単独再実行で pass）。`tim03_session_wiring` binary の alloc 失敗 abort（0xc0000409）と rustc LLVM OOM（exit 101）は transient memory 圧力で非再現。C: ドライブ temp full で link 失敗 → mbx sweep が必要な場合あり。
- **lint 修正**: TIM-01 残件 slice で `tim_01_ppqn.rs` の `doc_markdown`/`if-let` lint を修正。ED-B で `pc_b_rt` の `too_many_lines` を `#[expect]` で対処。
- **test 修正**: TIM-03 driver loop boundary test を `[0, wrap_offset]` → `[wrap_offset]` に修正（offset 0 は oracle が applied set から除外する正規化のため）。
- **lease/ownership**: `synvelis-workspace/src/lib.rs` は sv107 lease で `#[path]` 暫定 mount。`ProcessorConnection` は render が exhaustive destructure するため field 追加不可 → `ProcessorControl` + `boundary_attach` 経路で拡張。
- **review 修正の実績**: ED-B（StartTrim 二重 floor・EndTrim snap 基準・marquee exact overlap・media_rescale saturation）、MIDI Source（source-ID namespace・note map-key/id・ID watermark・extent/mapping 制限）、Take/Comp docs（AC-47 status・hit-test tier・内部参照・audition tool dependency・test case 欠落・tool vocabulary・index 変更・verification checkbox・"Collapsed default" rationale）、musical-time（fixture 帰属・helper/test 分類・型名・automation `BeatPoint` 分類・overflow fixture 過信表現）、TIM-03 driver（offset 0 期待値）。

## 出処

synvelis#38、synvelis#50、synvelis#52、synvelis#65、synvelis#125、synvelis#126、synvelis#127、synvelis#128（実装 slice の検証実績）。MD-A は synvelis#42 が前提。docs-only 設計契約 slice の検証は synvelis#11、synvelis#12、synvelis#4、synvelis#18、synvelis#19、synvelis#20。関連: synvelis#104〜109、synvelis#112、synvelis#114、synvelis#129、synvelis#130、synvelis#230、synvelis#253、synvelis#278、synvelis#279、synvelis#288、ruviform#184、ruviform#188、ruviform#189、ruviform#190、ruviform#196、ruviform#243、ruviform#253、ruviform#294、ruviform#307、ruviform#308、ruviform#309。
