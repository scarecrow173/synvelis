# プロジェクト基盤とワークスペース — 実行証拠

この文書は、Synvelis の基盤・ワークスペース系スライスで実施された検証と、その結果・判断を記録する。対象は VS-A〜D Recording Foundation、WS-A Workspace 基盤、および文書化・設計契約 slice である。計画段階で未実施の検証は実績と混同しないよう明示する。

## VS-A Project skeleton と Project model（synvelis#13）

task1 では workspace skeleton として 3 クレート `synvelis-project`/`synvelis-recording`/`recording-poc` を作成し、依存方向を `recording-poc → synvelis-recording → synvelis-project` とした。`mise run check`/`build`/`test`/`fmt-check`/`lint` が pass し、`test-msrv`（rust 1.89.0）も全 suite pass した。`Cargo.lock` には ruviform `41772a7a` を記録した。

task2 では 12 種 typed ID、checked time 型、半開 Range、tempo DTO、project POD を実装した。`id_time_range.rs`/`tempo.rs` のテストで全 ID round-trip、checked 演算拒否、半開隣接 semantics を検証した。tempo 検算は 120 BPM/4/4/48 kHz の 4 小節 loop = 384,000 frames ↔ 15,360 ticks、PPQN 既定 960 である。`compile_fail` doctest で kind 分離を証明した。テストは `rsplit_once('-')`（複数ハイフン kind の reload 失敗）と `SourceState` 1 段階遷移（Imported→Saved skip）の実バグを検出・修正した。

task3 では `ProjectController`/`ProjectOp`/`DirectoryBundleStore` を実装した。`controller_store.rs` で stale-revision 拒否（zero mutation）、invalid op 原子性、undo/redo の revision minting、Saved 分離、save/reload round-trip、malformed JSON/unknown field/unsupported schema/壊れた参照の拒否、失敗 save の prior HEAD 保持、失敗 load の Live state 保持を検証した。

task5/6 では `recording-poc` の決定的シナリオ（固定 seed・BTreeMap 順序出力）を 2 回実行して同一 stdout を `poc_determinism` が検証した。task6 で clippy pedantic 17 件を修正し、`mise run fmt-check`/`check`/`lint`/`test`/`build` が全緑、`test-msrv` も pass した。`git merge origin/main` は already up to date、`--locked` build は clean である。

判断として、`DirectoryBundleStore` は PoC 形式で最終永続化形式ではない。undo 深さは無制限で policy cap は deferred、`SynvelisOp` は PoC-level の sum type であり、journal/recovery・VCS 統合は VS-B 以降に委譲された。

## VS-B Capture engine（synvelis#14）

`tests/vs_b.rs` の 15 件の受入テストが全 pass した。内訳は Normal tiling+PCM 検算、257-frame block 内 punch in=113、3 loop pass+wrap、部分終 pass、PunchInLoop、tempo 境界、負 pre-roll、epoch/input 断絶、queue full/free 枯渇、span overflow fail-stop、underrun、SPSC 1:1、回収照合である。

RT 証拠として `tests/vs_b_rt_alloc.rs` を追加し、`stats_alloc` で通常・queue full・gate closed の 3 パスの allocations/deallocations = 0 を計測して pass した。`recording-poc` の 2 回実行は stdout byte-identical で、`vsb_frames=1152000`、`vsb_recovered=8`、`vsb_checksum` は固定値であった。`mise run fmt-check`/`check`/`lint`/`test`/`build` は全緑である。実デバイス/backend（ruviform #187/#190）は Mock 証拠の対象外と明記した。

判断として、1 block > chunk_frames の複数 chunk 分割は VS-B では持ち越しで、preflight が `max_block_frames` で排除する契約である。`GateClosed` validity は契約値として残すが全区間閉塞 block は chunk を生成しない。`PCM_OVERFLOW` は `max_block_frames <= chunk_frames` 契約で到達不能な防御 bit である。実数値（chunk/pool/queue/span/lookahead 容量）は PoC 計測で確定する Deferred である。

## VS-C Staging・journal・recovery（synvelis#15）

`tests/vs_c.rs` の 16 件が全 pass した。内訳は layout/manifest、bit-exact PCM、multi-stream、punch loop、partial write、disk-full byte limit、sync 失敗の区別、journal torn tail/checksum/seq gap、last valid checkpoint 復旧、unverified tail 排除、multi-stream 障害分離、finalize 冪等、stall→drain、contract violation、failed chunk/journal append の validated prefix 保持である。

process 試験は `vsc_process.rs` の 2 件で、`--crash` で `process::exit(3)` した子 process を別 process で recover し、Partial ref・再 recover の `already_finalized`・`media_byte_len` 一致を確認した。通常 record → Complete も pass した。review で発見された durability 欠陥（`verify_chunk` が media/journal 成功前に cursor を commit し、checkpoint が未 journaled 値を frontier に記録）は journaled frontier 分離で修正し、回帰テスト 3 件（`vsc_failed_chunk_preserves_validated_prefix`/`vsc_failed_journal_append_preserves_prefix`/`vsc_multi_stream_fault_isolation`）を追加した。`mise run check`/`lint`/`fmt-check`/`test` は全緑である。実デバイス/実 OS crash・power-loss は未検証で、fsync/dir sync/atomic rename の保証範囲は manifest `capabilities` の観測値記録のみである。

判断として、`CaptureChunk` に `sequence`/`channels`/`sample_rate` が無いため writer の連続整合は header+span の連続性に依存する。`ByteLimit`/`ShortWrite` 後の media partial byte は orphan として保持するが正式 Media GC policy は対象外である。`GateClosed`/`Faulted` chunk は VS-B が filled に出さないため writer は `Complete` 以外を契約違反として reject する。`StagingArea::create` は manifest を authority とし、dir のみ残る partial create は manifest 未公開として冪等に完了する。

## VS-D Commit 冪等（synvelis#16）

`tests/vs_d.rs` の 22 件と `vsd_process.rs`（実 process 2 回起動で全 stdout byte 一致・`result=OK`）で VS-001〜015 を全網羅した。内容は normal commit verified、2ch×2track+clock evidence、punch mid-block placement、loop 3-pass、punch-in-loop、queue overflow no partial apply、writer/storage fault recoverable、journal truncation last checkpoint、crash before/after commit retry、undo/redo stable IDs、save/reload integrity、epoch discontinuity、revision conflict、commit path 非 RT、PCM 不混入である。

review（cooperative+adversarial）で P1×4・P2 複数を修正した。内訳は staging drift での AlreadyApplied 不可達、stale manifest rebuild、原子公開後 dir sync、FinalizeSession commit 可否、manifest 内検証強化、負値/反転 range 拒否、ref 帰属/format メタ照合である。`mise run fmt-check`/`check`/`lint`/`test`/`build`/`test-msrv` は全緑（`--locked` 含む）である。

判断として、Partial media の明示 commit・session 追記 commit・media GC は後続 Issue である。`FinalizeSession`（Closed→Finalized）は registry 未登録なら commit 可能であり、commit 済みは registry で拒否されるため二重登録にならない。`capture-fault.json` は session スコープの保守的分離で、per-stream の engine fault 帰属（shared latch の分解）は実デバイス層の課題として残る。staging recovery は media 側の回復であり Project 状態の復旧を意味しない。`payload_hash` は非鍵付き CRC ベースの accidental-corruption 検出であり、暗号的 integrity ではない。

## WS-A Workspace 基盤（synvelis#36）

`tests/ws_a.rs` の 8 件で WS-A-01..06 を個別検証した。内容は複数 view 別 window の共有正本/独立状態、link group 連携/Pinned/独立/no-reemit/mapping 不能、command 解決（明示 target/suppress/unresolved/no cross-window fallback/ViewBusy）、detach→reattach/rollback/close veto、dock 操作で revision/graph_builds/undo 履歴不変、snapshot 保存/復元/破損復帰である。`mise run fmt-check`/`check`/`lint`/`test` は全 PASS で、`Cargo.lock` の全 ruviform source は `94c5bf8` 単一 rev（#181/#182 祖先）に統一された。

`doc_status audit --type all` は 72 findings（67 blocking）を報告したが、全て既存 architecture/vs-c 文書の front-matter/traceability 起因であり、WS-A 成果物への finding は 0 である。`test-msrv` は rust 1.89 未導入で未実施と記録し、pinned 1.99.0 で全検証した。実 OS Window/WebView/GPU/IME/plugin GUI は ruviform #243 lane で未検証と正直に記録した。

task 別の作業内容は、task1 が `synvelis-workspace` crate 骨格と ruviform-ui git 依存・全 ruviform source を #181/#182 含有 rev へ更新、task2 が link.rs（LinkGroup/LinkEvent/SelectionMapping/LinkGroupManager）と session.rs（ProjectSession+per-view revision 配信+EngineStub）、task3 が command.rs（CommandKind/InputContext/CommandRouter/ResolveOutcome）、task4 が workspace.rs（WorkspaceManager/dock lifecycle/window 台帳）・host.rs（FakeWindowHost）・persist.rs（WorkspaceSnapshot+restore_or_default）、task5 が `tests/ws_a.rs` の WS-A-01..06、task6 が merge・再生成・最終検証・IR である。全 task は `done` である。

判断として、Inspector `Explore` 状態は #5 へ deferred（Follow/Pinned のみ実装）である。実機 Window/Focus/DPI は ruviform #243 lane、複数 project・remote asset DnD・tool system は対象外である。`revision_inbox` は PoC の配信ログで無制限に伸びるため実装系では event 化/上限化が必要である。`FocusState`（WindowId 含む）は persist 対象外（セッション固有）である。

## 文書化・設計契約 slice

### Workspace Model（synvelis#2）

Issue #2 の 9 受入条件を WSM-AC-01〜09 から WSM-T-01〜20 Case へ分解し、既存 fixture（ws_a/ed_a/adopt/preset/two_window/sv113_dock/dock_ops/close_veto/shell/ui_ops）を照合した。WSM-T-08（transport playing 中の dock/window churn）だけ直接 oracle が無く、`workspace_model.rs` に 1 test を追加した。初回 `assert_eq!(revision, rev0+1)` が FAIL（実際 +3、undo+redo roundtrip が各 +1 進める会計）し、`rev0+3`/`builds0+3` に修正して pass した。`mise run fmt-check`/`check`/`lint`/`test`（synvelis-workspace・synvelis-app、app は 38 test binaries 全件 pass）は全 exit 0 である。review-protocol で CONFIRMED 3 件を修正した（非存在 op 名 `syn.project.rename`→`syn.project.set_ppqn`、壊れた節参照、relation 種別）。

### Workspace UX Scenarios（synvelis#3）

UX 正本化の文書チェーンと `evidence/sv3/` 追跡表を作成した。review-protocol で C1〜C6 を CONFIRMED（全て P2/P3、P0/P1 なし）とし D21〜D24 に確定化した。repo 全体の既存 baseline 122 blocking findings・route_graph blocked は範囲外として区別記録した。実機 lane（#114/#176）は NotRun である。design-doc 生成時にテンプレート断片の重複出力と規範詳細セクションの誤ネストが判明し、`## 1.`〜`## 16.` のフラット番号体系へ再構成して内部節参照を全件追従修正した。

### Context Inspector（synvelis#5）

CI-01〜13 の被覆台帳を `evidence/sv5/coverage.md` に記録し、`Pass (headless)` 3 件・`Partial` 7 件・残り NotRun/Deferred とした。review で link event 粒度の誤記（「Clip/Lane/Source・Bus も link event で到達可能」→実際は Track 粒度のみ）、存在しないテスト名参照（`sf04_probe_off_is_true_observation_silence`→正: `sf04_s2_probe_trace_bridge.rs::sf04_s2_mode_off_maps_to_disabled_and_unrouted_record_returns_false`）、ファイル名誤記、「情報階級」→「情報階層」の誤記を修正した。`mise run fmt-check` は exit 0 である。製品 AC の実行結果は設計のみ項目を正直に NotRun とした。

### DAW Workflow（synvelis#8）

Gate A backbone GA-01〜08・WF-SCN-MIDI/AUDIO/MIX・Keyboard/Gesture/Tool 契約を文書鎖と `evidence/sv8/` で確定した。WF-AC-01〜10 が Issue #8 受入条件 7 項目とタスク固有 3 項目を網羅する。review-protocol 1 サイクルで CONFIRMED P0/P1 なし（P2 文書バグ: 節参照誤り・gesture 行ラベル崩れ・implemented-by 欠落等のみ修正）→ approved 化した。実機/実 GUI/実 Device は NotRun/Deferred である。Keyboard 割当→#112、実機→#114/#176 に委譲した。

### Product Value Core（synvelis#1）

製品価値・三層構造・中核体験（Where/What/Why/How）・Gate A/B/C/AI・Acceptance Matrix 6 行を文書鎖と `evidence/sv1/` で正本化した。`mise run fmt-check` は exit 0、`doc_status lint`/`audit` で新規 docs 起因の error は無く（spec の `traceability-missing-upstream` warning は Issue 起点 spec 共通の既存パターン）、index 全対象 dir は `in-sync`、review-protocol は CONFIRMED P0/P1 なしである。実 OS/RT/実 GUI 受入は範囲外で各 Owner Issue（#114/#109/#103 等）の証拠で別途判定（NotRun のまま）とした。途中証拠として `evidence/sv114/` の SVT-01〜09・SF-06/PC-C6 oracle を `Headless verified` の途中証拠として記録した。

### Project Model（synvelis#10）

`mise run test synvelis-project` で 41 test binary・460 test 全件 pass（exit 0、lib unittests・39 integration・2 doc-test）を実測し PM-T oracle を確認した。PM-AC-01〜13 を PM-T-01〜22 の headless case・設計レビュー・NotRun/Deferred（PM-T-23〜30）に写像した。review-protocol で CONFIRMED 3 件を修正した（`ClipPlayback`/`PlaybackMapping`/`Clip` の field 名を実装に合わせ訂正、clip-level gain/fade field が現行 schema に存在しないことを明記（gain は strip/send `ParameterTarget`、fade は `CompTransition`）、`Command` が `ProjectId` を持たない実装事実に合わせ訂正、`SplitClip` の `DerivedFrom` 欠落を schema 拡張として deferred 記録）。`mise run fmt-check` は exit 0 である。route_graph は pre-existing baseline で blocked である。

### Project/Asset/Persistence（synvelis#17）

Issue #17 受入条件 13 項目を `SV17-C01`〜`SV17-C13` へ写像し、根拠 fixture（`store_publish.rs`/`head_cas.rs`/`controller_store.rs`/`vcs01_snapshot.rs`/`vcs02_diff.rs`/`vcs05_restore.rs`/`alib02_digest.rs`/`plugin_state.rs`/`monitoring_persist.rs`/`import.rs`/`import_recovery.rs`/`vs_d.rs`/`recovery_review.rs`/`project_adapter.rs`/`x_e2e.rs`/`l2_e2e.rs` 等）・先行実測（`sv84`/`rcd-02`/`vcs-06`/`syn-io-02-os-l1` の SHA・日時付き ledger）・NotRun/Deferred に分類した。`mise run fmt-check` は Pass（win32、rustc 1.99.1）である。review-protocol は ACCEPT WITH NON-BLOCKING NOTES である。実装 crate の検証は対象外で、正規検証は fmt-check のみである。未実装として RCP 系（#290〜#297 全て OPEN）・Save Job scheduler・Autosave/Backup・Retention/GC UI・macOS/Linux 実機証拠・物理電断/power-loss を NotRun/Deferred と記録した。

### Execution Plan（synvelis#23）

Issue #23 の横断 DAG を lane node + Issue 番号併記の単一非巡回 DAG として固定し、Start/Integration/Acceptance の 3 相 edge・Owner/Gate/Acceptance Matrix・OS-L1/GUI-L2/EXT-L3・正規入口対応・候補 SHA tuple Registry（#262/#419・ruviform#294・#114/#109/#103）を `evidence/sv23/` 台帳 3 件に記録した。検証実行記録は `evidence/sv23/execution-plan-verification.md` を正本とし、実装/Gate 実行は本 slice 対象外として `NotRun`/`Deferred` を誠実に記録した。route_graph/lint/index/fmt-check/review-protocol の結果は verification.md 参照である。

### AI-Native Baseline（synvelis#44）

AIB-001〜007/AHG-001〜011・AIB-AC-01〜08 を headless fixture・#45/#46・#23 配下 Issue・実機 lane へ写像した。`mise run gw10` は exit 0（gw10 86 tests pass、strict report G0〜G6/GTV PASS、missing evidence: SYS-02-ux/SYS-03-ux/SYS-05-powerloss/CONSUMER-RUVIFORM/CONSUMER-AUDIOLENS、UNSUPPORTED: SYS-07-rt）である。`mise run test synvelis-gateway` は exit 0（406 tests pass）、`mise run fmt-check` は exit 0 である。route_graph.js は pre-existing baseline blocker（artifact-graph + broken-relation）を報告するが本 slice 起因ではないとして範囲外記録した。

## 採用/棄却・凍結・既知制限

VS-A では DirectoryBundleStore は PoC 形式で最終永続化形式ではなく、undo 深さは無制限（policy cap は deferred）、`SynvelisOp` は PoC-level の sum type である。VS-B では 1 block > chunk_frames の複数 chunk 分割を持ち越し、`GateClosed` validity は契約値として残すが全区間閉塞 block は chunk を生成せず、`PCM_OVERFLOW` は到達不能な防御 bit、実数値は PoC 計測で確定の Deferred である。VS-C では `CaptureChunk` に `sequence`/`channels`/`sample_rate` が無い制約、ByteLimit/ShortWrite 後の media partial byte は orphan 保持、`GateClosed`/`Faulted` chunk は契約違反 reject、`StagingArea::create` は冪等である。VS-D では Partial media の明示 commit・session 追記 commit・media GC は後続 Issue、`FinalizeSession` は registry 未登録なら commit 可能、`capture-fault.json` は session スコープの保守的分離、staging recovery は Project 状態復旧を意味しない、`payload_hash` は暗号的 integrity ではない。

WS-A では Inspector `Explore` 状態は #5 へ deferred、実機 Window/Focus/DPI は ruviform #243 lane、複数 project・remote asset DnD・tool system は対象外、`revision_inbox` は無制限に伸びるため実装系では event 化/上限化が必要、`FocusState` は persist 対象外である。Workspace Model では multi-window の意味論は headless oracle のみで検証し（`ShellWindowHost` は single-window のまま、製品配線は #113/#175）、実 OS 2 Window/Dock/IME/Keyboard/pixel 検証は #114、実 plugin editor/NativeSurfaceSlot は #176 系、macOS/Linux GUI lane は使用要件発生時（WSM-T-20）である。Context Inspector では固定 5 タブ・自動 Monitor・Explore 履歴永続化・DAW 語彙の Ruviform 持ち込みは不採用/保留であり、MIDI Note/Event Provider は #10/#12 model 確定待ち、Issue #5 本体の Close は下流未実装を隠さないため Coordinator/人間に委ねる。

Project Persistence では Save Job scheduler 本体・Autosave/Recovery Root 物理配置・Backup/Save As・Retention 期間・GC UI は #92 系/PAP arch §15 の Deferred、macOS/Linux の File Sync/DirSync/Atomic Publish/Indeterminate 実機証拠は NotRun、物理電断・真の power-loss・Network/Cloud FS は全 OS で NotRun/Deferred（injected/process-kill 相当の fixture のみが契約を pin する）である。Execution Plan では実 Window/実 Device/実 Plugin/実 Provider の L2/L3 実測は synvelis#114/#176・ruviform#313/#243/#251/#253/#388/#389/#532 が担い、AudioLens f64/長尺の非回帰実測は ruviform#294 candidate SHA Gate、synvelis#21（Media/Render）・synvelis#22（Target Architecture）並行 Worker の文書は参照のみで編集・作成しない。文書承認を実装完了・製品受入と混同しない規則を test-spec Guarantees に明記した。Product Value Core では UX 比較の数値閾値・比較対象 DAW 最終リスト・参加者条件は Baseline 計測後に別途合意（Deferred）、初期リリースでの B/C/AI 必須化は製品 Owner の別途決裁、Creative Branch の Feature Acceptance は #277/#288 が正本である。AI-Native Baseline では通常 DAW 利用への AI Grant/Provider 接続の強制はせず、headless Pass を製品/実機受入に繰り上げず、未決定事項は受入 Gate 化しない。実装コード・Cargo/mise/CI/Ruviform は本 slice で変更しない。後続 Owner は #45 型契約・#46 製品受入・#114 実機 lane・#23 実行計画接続である。

## 文書状態の食い違い

vs-c の task 6 件（crc32 journal codec / staging faultinj / staging writer / recovery scanner / poc acceptance tests / verification impl record）は全て front matter `status: "todo"` だが、対応する impl IR は `status: "completed"` で全テスト pass・検証緑を記録している。task status が更新されていないだけで実装は完了済みと読むべきである（synvelis#15）。vs-c-task5 の Affected Files は `recording-poc/tests/vsc_crash.rs` を挙げるが、IR が記録する実ファイルは `vsc_process.rs` である。task 記述と実装のファイル名が食い違う（synvelis#15）。vs-d-task6 の Done When は短い IR 名 `vs-d-recording-commit-e2e` を指すが、実際の IR 名は `vs-d-recording-commit-e2e-idempotency-undo-reload` である（synvelis#16）。

plan/task の status が `in-progress` のまま IR が `completed` の文書が複数ある（execution-plan・project-model・workspace-model・workspace-ux-scenarios・ai-native-baseline-task）。完了度は IR/evidence を正本とする（synvelis#23/#10/#2/#3/#44）。判定語彙の揺れとして `Pass(部分)`（synvelis#44）vs `Pass (headless)`+`Partial`（synvelis#5）vs 2 軸のみで中間状態なし（synvelis#23）がある。新規 evidence を書く際は採用する語彙を検証・受入方針と先行 slice に照合する。skill script のパス表記が `.agents/skills/`（多数派）と `.claude/skills/`（workspace-model IR）で混在する。実行環境で実在パスを確認する（synvelis#2）。vs-a 系 task の Affected Files 記述（例: `tests/{id_roundtrip,time_checked,range,tempo}.rs`）と IR が記録する実ファイル名（`id_time_range.rs`/`tempo.rs`/`controller_store.rs`/`recording.rs`）が一致しない。task は計画時点の概略であり実ファイルは IR を正本とする（synvelis#13）。`.md` 形式の experiment record は index 対象外のため、exp index の差分が出ないのは規約どおりである（index は `.jsonl` のみを管理する）。「index 未反映」を欠陥と誤認しない（synvelis#10/#17）。`new_experiment_log.js` が生成する `.jsonl` はタスク許可範囲外になりがちで、削除して repo 慣行の `.md` に置き換える判断が繰り返し行われている（synvelis#8/#10）。

## 出処

synvelis#1（Product Value Core）、synvelis#2（Workspace Model）、synvelis#3（Workspace UX Scenarios）、synvelis#5（Context Inspector）、synvelis#8（DAW Workflow）、synvelis#10（Project Model）、synvelis#13（VS-A Recording Foundation task1/2/3/5/6）、synvelis#14（VS-B Capture engine）、synvelis#15（VS-C Staging/journal/recovery）、synvelis#16（VS-D Commit 冪等）、synvelis#17（Project/Asset/Persistence）、synvelis#23（Execution Plan）、synvelis#36（WS-A Workspace 基盤）、synvelis#44（AI-Native Baseline）、synvelis#230（ui vendor 同期規約）。upstream 側の前提・委譲先として ruviform#187/#190/#243/#294/#313/#388/#389/#532 を含む。
