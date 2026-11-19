# パラメータコントロール — 実行証拠

この文書は、Synvelis のパラメータ制御系で実際に実行されたテスト・計測・lane の記録をまとめる。対象は PC-A（制御モデル基盤、synvelis#40）から PC-C6（独立 oracle / 受入台帳、synvelis#103）までの slice 群と、その関連 Issue（synvelis#6・#12・#23・#28・#64・#106・#112・#114・#122・#176・#230、ruviform#28・#185・#186・#188・#189・#196・#197・#198・#219・#253・#294・#307・#313・#314）である。canonical lane は `mise run <task>` であり、直接 `cargo` 実行は補助証拠として区別される。headless・fake driver の成功を GUI・実 plugin・実 OS・実 RT の受入 Pass に昇格させていない。

## PC-A 制御モデル基盤（synvelis#40）

- `completed`。制御モデル・ID・scope・target/lane/source/assignment entity、schema v2、ProjectOps と conflict 検出、評価・DAG/cycle reject・capability check・dispatch、workspace `ControlSession` を実装した。
- 検証: PC-A-01〜08 の headless acceptance test（pc_a 系 9 test）と全 `mise` lane pass を記録。`test-msrv` は当時 rust 1.89 未導入で未実施と明記された。ruviform crates は 94c5bf8→fdd65da9 に再解決した。

## PC-B note-scope・prepared eval・dispatch（synvelis#43）

- `completed`。`Note` entity・note-relative 解決/記録・schema v4・prepared `EvalPlan`/`eval_rt`・RT allocation check・player dispatch の typed rejection 伝播・playback/render parity を実装した。
- 検証: `pc_b` test 10 件 + `pc_b_rt` 1 件。全 `mise` lane pass。レビューで複数の実害バグを検出・修正（後述の「発見・修正された不具合」節）。

## PC-B 残件 driver 配線（synvelis#64）

- `completed` 相当。`activate_with_attached`・`anchor_attached_transport`・`observe_attached_block`・seek quiesce 2-pass・`EvalPlan::is_stale`・PC-A 由来修正（empty gesture・removed write lane・clock anchor epoch）を実装した。
- 検証: 35 test binary 0 failures。`is_stale` の Err-arm 反転 bug を発見・修正。`offline_support` が意図的 upstream 契約と確認され workaround 不採用。

## PC-C1 録音トランザクション（synvelis#98）

- `completed`。非破壊 prepare → CAS apply → finish の 3 相 commit、`RecordingTransactionId` + FNV-1a fingerprint の idempotency、`preflight_record_finish` の apply 前照合を実装した。
- 検証: `pc_c1` test 12 件（REC-C1-01〜06）全 pass、全 `mise` lane + `test-msrv` pass。レビューで preflight-after-apply の二重書き込み hole を発見し apply 前照合へ修正。applied registry cap 64 は既知制約として記録。

## PC-C2 relation index（synvelis#99）

- `completed`。revision タグ付き派生双方向 index `RelationIndex` を実装した。
- 検証: `pc_c2_relation` test 5/5 pass。fixture で 400 edges の index build ≈1.97ms、`Both` query 240 relations ≈0.38ms（debug・single-shot）を計測。巨大 graph は当該時点では NotRun で synvelis#103 へ委譲された。

## PC-C3 control trace（synvelis#100）

- `completed`。7 stage slot の `ControlTrace`、`TraceSlot` 語彙、freshness 判定、`ReceiptEvidence` 投影型を実装した。`trace_c3` fixture test 6 件 pass、レビュー 2 pass で blocker なし。
- TRACE-C3-05 RT trace adapter は `completed`（IR 基準）。`trace_c3_05` 5 件 + `trace_c3_05_rt` 1 件 = 6 件 pass、rt alloc 0/0 を `stats_alloc` で実測。単一 drain owner・bounded history 256・max_targets 1024・plan_gen_window 64 を確認した。ただし対応 task 文書は `in-progress` のまま残り、status 不整合として後述する。

## PC-C4 plugin binding・observation・app composition（synvelis#101）

- `pc_c4_binding` 7/7 pass（instance authority・bind gate の新規 identity test 2 件含む）。`fmt-check` は対象外の pre-existing rustfmt drift（`crates/synvelis-library/tests/vcs03_git_provider.rs`）で fail、`check-ruviform-rev` は stale vendor 検出（synvelis#230 既知）で fail と記録。本 slice 起因ではないと切り分けた。
- PC-C4-03 param observation: `pc_c4_03_observation` 8/8 pass。L1 `Unavailable` の実値は pin `b02942c2` が wire report verb を持たず NotRun、注入 seam で deterministic unavailable/error を検証した。
- PC-C4-04 app composition: fixture 4 test pass。catalog vend・`SessionRoutingEngine` の dispatch 所有・`ui_pump` tick 駆動・`feed_control_cycle` の Reported-only `feed_plugin_report`（Estimated stamp）を確認した。
- PC-C4-05 persisted instance: `plugin_instance_persist` 13/13 + `pc_c4_05_persisted_instance` 3/3 pass。全 `mise` lane + `test-msrv` pass。`test-msrv` は初回 2 件 flake（`monitor_arm` rlib format error、webdav `TransientIo`）で cargo artifact/並列負荷起因と判断し再実行 pass。merge で ruviform rev `9865cd57` を取り込み `sync-ui-deps` 実行。TDD red→green で実施した。
- PC-C4-06 control op surface: fixture 4 test green、正規 9 lane 全 pass（`test-msrv`/`build` 含む）。`plan_generation` deterministic 規則（fast=1、automation=2、observe pump +1）を確認した。
- PC-C4-07 transport mirror + observe_block: `pc_c4_07_transport_observe` 10/10 + `capability_rebind` 2/2 pass。node cursor domain と engine cursor の乖離・generation swap 後の `[0,256)` restart・span 捏造修正を test で pin した。
- drain composition: fixture 4 test pass、正規 9 lane pass。レビューで drain/engine pairing を規約から attach 強制へ修正。`PluginReportDrain` newtype（`Rc<RefCell<PluginHost>>` clone）で app が `PluginHost` を名指さない seam を実現した。
- observe engine wiring: fixture 4 test pass、正規 lane pass。`status_observe_section_tracks_node_cursor_domain` test が 2 block 後の `[256,512)`・generation swap 後の `[0,256)` restart・engine cursor との乖離を pin。drain-once 意味変更に伴い `pc_c4_06_control_op_surface` の `observe_reports_stages_and_feeds_only_reported` を更新し、applied generation の receipt 証拠は buffered cycle で assert、明示 op の `receipts` は drain-once で空を明示 assert した。
- webview composition: fixture 3 test pass（gate 閉 `None`→stub 維持・gate 開 `Some(dir)`→attach→publish→非 stub `syn.control.status`・recreate detach）。`SetProjectName` revision が空 project でも generation 1 の control surface を publish することを実測確認。`synvelis-native.rs` は `git diff` で無変更を確認した。

## PC-C5 UI slice（synvelis#102）

- PC-C5-01 relation inspector: `rel_c5` 11 fixture test pass。全 canonical lane（`test-msrv`/`check-app-native`/`lint-app-native` 含む）pass。`RelationIndex::build` が publish ごと O(N) である点は非 RT 許容として記録。
- PC-C5-02/03 gesture/stale/inspector: `uic5` 12 fixture pass。`synvelis-library` webdav `status_mapping_table` の flake を 1 件観測するも無関係と判断し単独・再実行 pass。全 lane pass。
- PC-C5-04 recording status: `uic5_04` 10 fixture pass。`test-msrv` 初回が disk 枯渇 `os error 112` で fail → `mbx gc` で 257 GiB 解放 → 再実行 pass。lint で doc backtick/`too_many_lines`/`&Option<T>`/`assert_is_empty` の指摘を修正した。

## PC-C6 独立 oracle・受入台帳（synvelis#103）

- oracle REL/TR headless: `pc_c6_oracle_rel` 7 件 + `pc_c6_oracle_tr` 8 件 pass。REL07 巨大 graph 実測: 32,200 nodes / 32,000 edges で `RelationIndex::build` 566.6ms vs `OracleIndex::build` 198.9ms（debug）、8 query + widest は sub-ms、deterministic。レビュー 2 pass blocker なし。
- oracle rows: `pc_c6_oracle_pl` 8 + `pc_c6_oracle_pl_rt` 1 + `pc_c6_oracle_rec` 8 + `pc_c6_oracle_ui` 7 全 pass。RT noalloc は warm-up 8×256 frames 後に 64×256 frames window で alloc/dealloc=0 を実測。REL/TR regression も pass、6 lane（`test-msrv` 含む）pass。Ruviform lock SHA `0f5b5737`。
- TR ledger oracle: `pc_c6_oracle_tr_dispatch` 6 test（tr01/03/04/05/07/08）pass。TR08 の metrics は手計算で照合: `events_recorded=14`、`history_lost=10`、`targets_lost=2`、`expired_correlations=1`、`dropped_observations=1`、`latency_samples=0`。全 9 lane pass（stable + MSRV 1.89.0）。
- TR02/TR06 realpath: 8 test pass。TR02 は 7 次元 mismatch + 複合 `ProjectRevision` 優先、TR06 は `Silence` vs `Missing`/`Disabled`/`Lost`/`NoEvidence` の sibling 分離を実 path で検証し、全 TR row の realpath 被覆を完成した。
- acceptance ledger: 53 行 audit で headless 実施可能部分は既存 fixture で全被覆・新規 test 0 件を結論。`Pass-measured (headless)` 語彙を「当該 SHA で `mise run test` green」に限定導入。`Unsupported` 0 行。監査で P2 文書問題 4 件（行数分類・同名 test pointer 曖昧・note-scope 被覆過少・test 数未検証）を修正。全検証 lane pass、`evidence/sv103` に 4 file 作成した。

## 発見・修正された不具合（review/実験由来）

- PC-B レビュー: partial staging（`QueueFull` 時の orphan plan）、`u128→u64` 変換の truncate（`param_target_out_of_range` guard 追加）、shared param の `param_in_use` guard（誤帰属防止）、observe が caller-supplied plan を使う問題（内部保持 `last_plan` へ）、`param_defunct` の unreachable（plugin param 再列挙で実際に defunct するよう pass-through 追加）、`SplitClip` の note 処理、`AddNote` の domain 検証、RT 測定 scope の問題——全て修正済み（synvelis#43）。
- PC-B 残件レビュー: `EvalPlan::is_stale` の Err-arm 反転（compile 失敗を non-stale と誤判定する bug）、`build_commit_ops` の `take()`-before-validate による silent total loss、quiesce 1-pass では 64-entry channel を drain しきれない問題（2-pass 化）、observe が project-then-drain の順で false `Dispatched` を mint する問題——全て修正済み（synvelis#64）。
- PC-C1 レビュー: `preflight_record_finish` が apply 後照合で二重書き込み hole を持っていたため apply 前照合へ修正（synvelis#98）。
- PC-C3 レビュー: `AppliedReceipt`/`CommandId` が外部構築不能 → `ReceiptEvidence` 投影型導入、freshness `Unavailable` tie-break が `ClockUntrusted` を隠蔽 → Present-slot 優先へ修正、clippy `match_same_arms` 解消（synvelis#100）。
- PC-C4 レビュー/実験: drain/engine pairing を規約から `attach_routing_engine` の強制へ、`observe_cycle` drain-once による明示 op `receipts` 空の意味整理（test 更新）、`call_start` の node cursor domain 確定（engine cursor 誤用を却下）、`instance: Option` 採用で `PluginMismatch`→`InstanceMismatch` 報告順序維持、env 読みを process boundary に限定——全て記録・修正済み（synvelis#101）。
- PC-C6 監査: acceptance ledger の P2 文書問題 4 件修正、`pc_c6_oracle_pl.rs` のコメント番号と test-spec Rule semantic の入れ替わりを発見・記録（fixture 本体は編集範囲外として未修正）（synvelis#103）。

## 既知の制限・NotRun・スコープ外

- L1 wire report verb の実値は pin `b02942c2` が未対応で NotRun。attached instance の param 列挙 API も pinned ruviform に無く caller 登録 catalog で代替。これらは pin 更新待ちの upstream 不足として ruviform 側 Issue に紐付け（synvelis#101、ruviform#253/#307/#313/#314）。
- GUI・実 plugin・実 OS・wire-schema L1・AudioLens・consumer・実 RT/device の検証は全 slice で明示的に外部 gate/NotRun。headless green を製品受入に読み替えない（synvelis#102/#103、ruviform#219/#294、synvelis#28/#114/#176）。
- `RelationIndex::build` は publish ごとの O(N) 再構築。RT 経路ではなく UI read model 用途で許容と記録（synvelis#102）。`detect_conflicts` は O(regions²) で大規模化に index が必要（synvelis#40）。
- recording の applied registry は in-memory cap 64 で crash 復旧・永続 registry は synvelis#12 へ deferred。ledger revision ↔ dispatch ordinal の 1:1 仮定は upstream `record_plan` 規約依存（synvelis#98/#100）。
- `param_defunct`・`observe` の plan 鮮度は orchestration 境界の caller gate に依存し `eval_event` 単体では再検査しない設計（synvelis#101）。`EvalPlan::is_stale` の recompile-compare は check ごと O(compile) で最適化 deferred（synvelis#64）。
- 環境 flake（webdav `TransientIo`/`status_mapping_table`、`monitor_arm` rlib、pre-existing rustfmt drift、stale vendor）は全て対象 slice 範囲外・既知 baseline として記録され修復対象外（synvelis#43/#64/#101/#102/#230）。
- PC-C2 時点の巨大 graph index 計測は NotRun で PC-C6 の REL07（32,200 nodes/32,000 edges、build 566.6ms）へ委譲された（synvelis#99→#103）。

## 文書の不整合・status drift

- PC-C3 RT trace adapter: 対応 task 文書は `in-progress` で steps 2–6 が未チェックのままだが、実装記録（IR）は `completed` で 6 test pass・rt alloc 0/0 を記録。後続の PC-C4/C5/C6 slice も trace adapter の存在を前提に進んでおり、task status の更新漏れと見られるが、文書上は未解決の不整合として記録（synvelis#100）。
- PC-C6 task status: `pc-c6-task1-oracle-rel-tr`・`pc-c6-task1-oracle-rows` の task 文書が `in-progress` のままだが、対応 IR は `completed` で全 test pass を記録。`Done When` 全チェック済み・Evidence 節ありの task が status 未更新の例（synvelis#103）。
- plan/task の status 運用の不統一: ほぼ全 plan が `approved` のまま（PC-A plan のみ `completed`）、PC-C4-06 plan の Verification チェックボックスは未チェックのまま task/IR が完了、webview composition task は `Done When` 全チェック済みだが Verification 節が未チェック。完了度は status でなく IR/検証記録で判断する必要がある（synvelis#101/#103 ほか）。
- schema 版の記述齟齬（未解決）: PC-B 残件（synvelis#64）の実装記録はベース `origin/main = 26ad0b8` を「`SCHEMA_VERSION=3`、PC-B #43 = b609b39、ED-B #65 = 26ad0b8 着地済み」と記述するが、PC-B（synvelis#43）の記録は schema 3→4 bump を報告。同一ベースで schema 3 と 4 が矛盾して見える可能性があるが、記録の文脈が文書からは一意に確定できず推測で解消しない（synvelis#43/#64）。
- Ruviform pin SHA の多様性: slice ごとに観測される pin（94c5bf8→fdd65da9、258df5ae、b02942c2、0f5b5737、fd1d4d5f、9865cd57 等）は branch-tracking git 依存の時系列差であり矛盾ではないが、文書をまたいで同一値を期待すると誤読する。各値は当該 slice 時点の履歴的事実（synvelis#40〜#103、AGENTS.md 規約）。
- PL コメント番号の写像ずれ: `pc_c6_oracle_pl.rs` の PL03/PL04/PL06 コメントが test-spec の Rule(PLxx) semantic と一部入れ替わっている（QueueFull/atomic batch・GraphSwap/Reload・plan staleness の対応がずれる）。fixture は PL01〜PL08 全行を被覆しており機能的欠落ではないが、行番号だけの写像は誤対応になる。ledger は test-spec semantic で写像し差異を明記、fixture 本体の修正は未実施（synvelis#103）。
- `pc_b.rs` の同名ファイル衝突: `pc_b.rs` が player/workspace 両 crate に存在し、台帳監査で workspace 側 `pc_b_02_*`（note-scope fixture）を一時見落とすほど曖昧。同名 test pointer の写像には crate 修飾が必要（synvelis#103）。

## 出処

synvelis#40、synvelis#43、synvelis#64、synvelis#98、synvelis#99、synvelis#100、synvelis#101、synvelis#102、synvelis#103。関連: synvelis#6、synvelis#12、synvelis#28、synvelis#114、synvelis#176、synvelis#230、ruviform#219、ruviform#253、ruviform#294、ruviform#307、ruviform#313、ruviform#314。
