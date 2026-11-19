# プロジェクトバージョン管理 — 実行証拠

この文書は、Synvelis の VCS（Semantic Visual Diff）に関して実施された検証と、その結果・下された判断を記録する。対象は VCS-01 Immutable Snapshot Resolver（synvelis#92）、VCS-02 Semantic Diff Engine（synvelis#93）、VCS-03 Git Read Provider（synvelis#94）、VCS-04 History/Visual Diff UI + backend wiring + native mount（synvelis#95）、VCS-05 Safe Apply/Restore + apply surface 拡張（synvelis#96）、VCS-06 fixture/fault matrix と証拠補完（synvelis#97）、親 Issue #7 の統合契約 docs-only slice（synvelis#7）、および関連修正 synvelis#335（unborn HEAD 誤分類）である。計画段階で未実施の検証は実績と混同しないよう明示する。

## VCS-01 Immutable Snapshot Resolver（synvelis#92）

検証構造は test-spec SNAP-FIX-01〜10 を vcs01_snapshot テスト群の 28 tests + digest FIPS 2 tests で網羅するもので、5 locator 系統の resolve、Saved 4-read window・Recovery 2-read 比較・staged index_generation 比較の race semantics、asset/opaque state の `Partial`/`Degraded` 記録、read-only oracle（controller revision/live/saved・store HEAD/bytes・VCS port call journal が resolve 前後で不変）を assert した。

結果は `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`/`check-app-native`/`lint-app-native`/`build-app-native` 全 Pass である。`lint` 初回は main 由来の既存警告 2 件を style commit で解消してから Pass となった。

NotRun/Unsupported の記録は、実 OS filesystem atomicity は headless `FakeStore` fixture のみで NotRun、実 Git remote/provider・実 index/worktree は `VcsBytesPort` 境界のみで Unsupported、GUI/実 plugin host probe は Unsupported（opaque state は `Resolved`→`Unknown`・`Unresolved`→`Missing` 固定）である。

## VCS-02 Semantic Diff Engine（synvelis#93）

検証構造は test-spec VCS-DIFF-01〜08 を vcs02_diff テスト群 21 tests で検証するもので、semantic 分類（rename/move/trim/duplicate/source swap/processor reorder・move/connection rewire・add・remove/automation/asset digest）、AA/AB/BA・determinism（同一入力 2 回 compare で `ChangeSet` 完全一致）、missing/unknown asset・opaque state、pre-v6 partial projection、cross-kind・owner・cross-project ambiguity、`ChangeSetCache` hit で同一 `Arc`/再計算なし、budget/cancel を網羅した。

結果は `mise run` 全 lane Pass である。`lineage` は常に `None`（schema extension まで deferred）、cross-kind/owner/cross-project は `AmbiguousIdentity` issue + `Unknown` に確定しない設計で、consumer は tri-state を区別して扱う必要がある。

## VCS-03 Git Read Provider と unborn HEAD 修正（synvelis#94、synvelis#335）

検証構造は受入 4 行を vcs03_git_provider テスト群 19 tests（後に unborn HEAD 回帰で 20）で検証するもので、実 git 2.54 fixture（temp worktree 構築、SHA-1/SHA-256 init・LFS pointer+手置き object・conflict/gitlink/partial stage を生成）を使った。結果は 19/19 pass で、fixed commit が branch/tag force-move 後も同一、merge 全 parent 保持、rename/delete、partial stage `PartialStage`、untracked/missing LFS `Missing`+`Degraded`（silent Equal なし）、index generation 追跡、repo/project identity 分離、SHA-1/SHA-256 区別、cancel 全 operation、repo oracle + session oracle（live revision/tracks 不変）を実測した。remote LFS fetch・submodule・rename similarity・first-parent opt-in は対象外である。

NotRun の記録は、実 OS permission matrix・real remote fetch・auth/credential 実経路・GC/objects 欠損の実 filesystem 条件・GUI/Visual Diff・RT 性能保証は headless fixture で検証不能のため Issue #97 へ `NotRun` として送るものである。

synvelis#335 の unborn HEAD 誤分類は、pre-fix で回帰 test が `Unavailable { detail: "TransientIo: git command failed" }` で決定的に失敗（Issue の観測 signature と一致）することを確認した後、`rev-parse --verify` から `--quiet` を除去する修正を行い、`vcs03_git_provider` 20/20 pass を得た。`mise run` fmt-check/check/lint/test/test-rt/check-app-native/lint-app-native/test-msrv/build 全 exit 0 で、git 系テスト群（ALIB-04 系）は無変更で全件 pass（blast radius ゼロの実測証拠）である。

## VCS-04 History UI・backend wiring・native mount（synvelis#95）

検証構造は、workspace の vcs04_history テスト群 13 test（A/B ChangeSet 共有・同一 cache Arc・Focus/Selection・Stale・read-only oracle・ViewSpec mount）+ app の vcs04_history テスト群 9 test（`syn.vcs.view`/`syn.vcs.visual` op・detach view・authority 不変）+ project crate 内 unit test 6 件（adapter 契約・foreign repository 拒否）である。op 面では `syn.vcs.view`（refresh/load_more/select_node/pin/compare/view_mode/clear）は read-only で `blobs: None` の `VcsSession` を構築し、`syn.vcs.visual{open,close}` は detached window として `view-{n}` を mint/解放する。select/invoke は read-only 契約で拒否される。

backend wiring は vcs_backend_wiring テスト群 2 test で検証した。`project_new_binds_real_backend_and_serves_committed_diff`（実 commit/index を跨ぐ committed/staged A/B diff）と `backend_rebinds_per_root_and_clears_on_non_repo`（非 repo `None`/`no-backend`・repo A→B re-bind の `repository_id` 変化・repo→非 repo clear・`project.open` re-bind）で、real git CLI fixture・git 不在時 honest skip である。

native mount は vcs04_native_mount テスト群 7 test（dock-domain 解決・`view-{n}` fail-closed・`ViewHost` mount・state publication・shell lifecycle・new-window dock saga・authority 不変）+ vcs04_mount_gpui テスト群 3 scenario（`GpuiGuiTest` mount・read-only・A/B/diff・pagination・update・detach stale・2 window remount・close・external gates Unsupported）で検証した。

PR #252 merge commit `cac19b2`（18 files/+5453）について、merge 時に IR が欠落していたため backfill + doc chain 閉鎖 slice で、merged tree（`origin/main` 取り込み後）上で canonical lane を再実測し全 Pass となった — workspace `vcs04_history` 13/13・app `vcs04_history` 9/9・`vcs06_ui_matrix` 4/4・project `vcs::history` unit 6 である。

環境事象として、初回試行は Windows 並列 link 時の一過性 `LNK1102`（メモリ不足）を観測し、再実行で exit 0（コード欠陥ではない）であった。doc lint で plan/test-spec の relation 系 blocking finding を解消し、残存は legacy `invalid-id-prefix` のみ（repo-wide migrate 対象として据え置き）である。

native mount の検証結果は、`mise run test synvelis-app` 全 suite Pass（`vcs04_history` 9/9・`vcs04_native_mount` 7/7・`view_mount` 27/27・two-window 8/8・`vcs06_ui_matrix` 4/4・`vcs_backend_wiring` 2/2 含む）、`check-app-gpui`/`lint-app-gpui`/`test-app-gpui` Pass（`vcs04_mount_gpui` 3/3 — default lane では 0 test の feature gate）、`check/lint/test synvelis-workspace` Pass（`vcs04_history` 13/13 含む）である。実 OS/GPUI window・実 Git・IME・pixel・実 Plugin dock は NotRun（#114 gate）である。

unborn HEAD の provider 分類バグ（当時未修正）は fixture を born-HEAD 状態に揃えて回避し、spec/design/test-spec/PR の既知制限・未対応の作業に再現手順つきで記録した（後に #335 で恒久修正）。

## VCS-05 Safe Apply/Restore（synvelis#96）

headless の検証構造は、vcs05_apply テスト群 13 tests（後に monitoring fixture 追加で 14）— apply 成功/1 undo・approval/revision/fingerprint/identity guard・fail-closed unsupported/missing/degraded・idempotent/ACK gap・replay — と vcs05_restore テスト群 7 tests — publish/replay・missing asset・degraded policy・existing destination・unsafe destinations・low disk・cancelled — である。結果は `mise run` fmt-check/check/lint/test/test-rt/test-msrv 全 Pass（Windows・rust 1.99.0、MSRV lane は rust 1.89.0）。実 OS FS durability・実 Provider/plugin E2E・GUI/RT は #97/#294 へ deferred である。

monitoring apply surface の検証は、`vcs05_apply` 14 tests・`vcs06_monitoring_matrix` 6 tests で、`mise run` fmt-check/check/lint/test/test-rt/test-msrv/build 全 Pass（`git merge origin/main` で `1093530` post-#332 取り込み・conflict なし）。ledger VCS-APPLY-07 行を `Unsupported` fail-closed から supported-apply 実測（Pass）へ更新し、Environment ブロックを実走環境（origin/main @ `1093530`・ruviform `fd1d4d5f`・schema v13・rustc 1.99.0/git 2.54.0.windows.1）で更新、fixture test 数を実数（vcs06_monitoring_matrix 6・合計 34）へ修正した。

plugin-instance apply の検証は、`vcs06_plugin_instance_matrix` 9 tests all Pass（apply 8 + restore-as-copy verbatim 1）。`mise run lint synvelis-project` 初回は `too_many_lines` + test 3 件の指摘で Fail → helper 分離と規約整合で修正後 Pass。`test-msrv` は NotRun（ポリシー文書 §5 により Daily Full CI 委譲）。review-protocol 実施 1 回で CONFIRMED P0/P1 なし・REJECTED 1 件（Processor change を伴わない `PluginInstance` `Added` は有効 live state では到達不能のため非該当）・P3 non-blocking のみ、verdict ACCEPT WITH NON-BLOCKING NOTES。ledger に VCS-APPLY-09 行と VCS-RESTORE-08 行を追加した。

## VCS-06 fixture/fault matrix と証拠補完（synvelis#97）

test-only slice 群（プロダクションコード変更なし・既存 fixture/helper と公開 API の外部観測のみを oracle にする）で matrix の未消化行を補完した: monitoring-domain matrix（diff 2・apply 1・restore 1）、UI headless matrix（4 tests）、real-git/FS fault（real-git 4 + Windows FS fault 5）、variant-domain matrix（3 tests）、evidence completion（RESTORE-08 fixture + COMPAT CI 記録）。per-domain matrix file の先例に倣い domain ごとに 1 file へ束ねる構成である。

monitoring matrix の検証は `vcs06_monitoring_matrix` 4 tests pass（後に apply surface 拡張で 6 tests に再構成）で、`mise run` fmt-check/check/lint/test/test-rt/test-msrv 全 Pass（`origin/main` @ `7822857` post-#321 取り込み後、200 suites 全緑・test-rt 191 suites・test-msrv 200 suites 緑）。review-protocol で CONFIRMED P1×2（partial/inverted punch・Recording domain 除外・AA-equal・異 project_id の coverage 欠損、clippy 4 件）・P3×2（`stable_id` assert 範囲、front-matter relation 不整合）を修正、REJECTED 2 件（placeholder fingerprint が `FingerprintMismatch` で fail する疑義 → 実測は `Unsupported` exact match で非該当、`snap_manual` digest 算出式の相違 → 一意・決定的であればよい）、verdict ACCEPT。ledger に VCS-DIFF-09/10・VCS-APPLY-07（当時 `Unsupported` 実測）・VCS-RESTORE-07 行を追加した。

UI headless matrix の検証は `vcs06_ui_matrix` 4 tests pass で、`mise run` fmt-check/check/lint/test/test-rt/test-msrv/check-app-native/lint-app-native 全 exit 0（`origin/main` @ `099e7ea` 取り込み・`mise run sync-ui-deps` で ruviform pin `b02942c2` へ vendor 同期）。ledger の VCS-UI-01..05 行を headless `Pass*`（UI-01/02/03/04）と実機 `NotRun`（UI-05 → #114）に分割記録した。

real-git/FS fault の検証は `vcs06_git_matrix` 7 tests・`vcs06_restore_matrix` 11 tests・`vcs06_apply_matrix` 10 tests 全 pass（`origin/main` @ `7c206289` base、git 2.54.0.windows.1）。real-git 行列の内訳は packed/GC/repack 後の fixed-revision read・pruned revision → `RevisionGone`・`file://` remote での force-push+fetch 後も pin 不変・linked worktree の index scope 分離を実測した。Windows FS fault 行列の内訳は restore staging manifest/HEAD と apply journal の share-mode hold・read-only ケースを dir 内ファイル対象で実測（`#[cfg(windows)]`、POSIX compile 除外）した。

CI run 43428177865（main `7c206289`、manual workflow_dispatch・actor `scarecrow173`）全 leg success を `gh` 実測で記録し COMPAT-03 verdict を最新 run の Pass に更新した。bare repo の `symbolic-ref HEAD` 不足による clone unborn・空 checkout を初回 test 実行で発見・修正した。

variant matrix の検証は `vcs06_variant_matrix` 3 tests pass（`origin/main` @ `3da82c5` post-#459、rustc 1.99.1）。`variant_sets` fork/adopt のみ差分が `Equal`/`Complete`・0 changes（digest 差と実差を assert し AA fast path でないことを確認）、mixed diff で variant 差分 silent drop、restore が `RestoreError::Validation`（`origin belongs to a different project`）fail-closed を実測 pin した。ledger に VCS-DIFF-11=`Blocked`・VCS-APPLY-10=`Blocked`・VCS-RESTORE-10=`Fail` を実測値で追加し Environment ブロックを schema v14・現行 ruviform pin へ更新。`mise run test-msrv` は NotRun（Daily CI 委譲）、`prek run --all-files` Pass。follow-up Issue #457/#458 を起票した。

variant matrix の apply 行は fixture で apply を実行せず honest `Blocked` である — diff が variant kind `SemanticChange` を emit しないため apply 対象の change object が存在しない。silent `Equal`/silent drop/`Validation` 拒否を観測値のまま pin し Pass に繰り上げなかった。

evidence completion の検証は `vcs06_plugin_instance_matrix` の RESTORE-08 test 1 件 pass で、resolved processor 2 件（`vendor.dist`/`1.0.0`/rev3・`vendor.eq`/`2.1.0`/rev7）+ unresolved 1 件の project を `restore_as_copy` し、publish 済み `SnapshotEnvelope` の `project.plugin_instances` が source と `PartialEq` 全 field 一致（watermark=3 実測 — test-spec 例示の watermark=2 を訂正）・fork `project_id` のみ再採番であることを pin した。

COMPAT-03/04 を required/observed 区別つきで COMPAT 証拠レポートに記録した（run 42938180676 head `13e65189` の leg outcome を URL 付き転記、#268/#306 は windows verify が VCS-06 非関連の webdav MockServer TransientIo で failure のため「測定済みだが非一様に green」、#323 は covering run 無し = NotRun pending）。`mise run` fmt-check/check/lint/test/test-rt/test-msrv/build 全 Pass（`origin/main` @ `4443007`）である。

## VCS Integration 統合契約（synvelis#7）

docs-only slice の検証は、`mise run fmt-check` exit 0、`doc_maintenance.js index plan|apply`（7 dir）全て適用・planHash 一致・index 手編集なし、`sync_impl_index.js --kind ir|exp` IN-SYNC、`doc_status.js lint` 新規文書の finding 0（repo 全体の pre-existing finding は範囲外として記録）、`audit_impl_record.js`/`audit_experiment_log.js` 本 slice 起因の error なし、`route_graph.js --focus` は `blocked`（`artifact-graph`、pre-existing repo 状態）だが taskGraph は本 task を `active` として解決、review-protocol blocker なしである。

docs 構成は spec/design/plan/task/test-spec/impl-ir/impl-exp の 7 文書を canonical chain で作成し、sv7 evidence 領域に Case 被覆台帳と検証記録を置いた。ledger は Issue #7 の 14 受入条件 → Case family 写像・責務 Owner 写像・mode/layer/fixture 凡例・全 Case ID の被覆表・明示的非 Pass 領域・verdict 分布集計を含む。検証記録はコマンド・OS・toolchain・exit code を記録し、docs-only slice のためプロダクションコード変更・CI 変更・本番 crate 変更は一切ないことを明記した。

8 view 採否の根拠は、Issue #7 §6 の HTML mockup が chat 添付で repo 未収録のため pixel レビューは実行不能であり、採否判定は Issue 本文の画面構成と既存 slice spec の確定済み view mode を根拠に記録した — SideBySide/ChangeFocus 採用・Overlay/Evolution Map/Rendered Audio Diff 延期・Clip Detail 条件付き採用（独立 mount は後続）・Plugin State 未採用・Audio 延期である。

## 文書状態の注意（食い違い・未更新箇所）

task/plan 文書の status・checkbox と IR `completed` の齟齬が複数ある。vcs-01 の task1（status `in-progress`・Done When/Verification 全て未チェック）と plan（Implementation Sequence が未チェックのまま、status `approved`）、vcs-05-plugin-instance-apply の task（`in-progress`・全 task 未チェック）と plan（`approved`・sequence 未チェック）、vcs-06-fixture-fault-matrix task（`in-progress`・「push → PR 作成」が未チェック）は、対応する IR が `completed` で実測済みだが task/plan 側が未更新のまま残っている。一方 vcs-04 history は merge 後の doc closeout slice で task `done` 化・relations 充填の先例があり、vcs-05-monitoring-apply-surface-task は `done` だが plan の sequence checkbox は未チェックのまま、という混在状態である。IR の実績を正とし、task/plan の未更新は文書運用の遅延として扱うのが妥当である。

`test-msrv` の実施方針が時期により揺れている。vcs-01〜vcs-06 の多くの slice（〜2026-11-13 頃）では `mise run test-msrv`（rust 1.89.0 で workspace test）を実走し Pass を記録するが、vcs-05-plugin-instance-apply（2026-11-14）と vcs-06-variant-matrix（2026-11-14/15）では「ポリシー文書 §5 により MSRV 別再コンパイルは Daily Full CI へ委譲」として NotRun 記録（variant-matrix は「MSRV レーンは通常開発では行わずデイリーCI（daily-ci → CI workflow）で行う」というユーザー指定を根拠に必須検証から除外）。後者が新しい時期の記述であり、ポリシー文書 §5 の現行運用を確認する必要がある。

vcs-06-fixture-fault-matrix task が `in-progress`（push/PR 未完）のままだが、後続 slice（monitoring matrix 等）がその ledger 行追加を前提に作業しており、evidence completion の IR は「headless fixture 行列は PR #268/#306/#323 で記録済み」と記述 — task 文書の status が実態（merge 済み）とずれている可能性がある。源文書からは task doc が closeout されなかったことが読み取れるが、実際の merge 状態は本グループの文書だけでは確定できない。

vcs-06-monitoring-matrix-slice の `defers` frontmatter に monitoring-apply-surface の spec 文書参照が含まれており、vcs-04 history IR の決定「自由記述 `defers` は `broken-relation-link` finding を生むため Follow-ups 節に記録」とは異なる扱いになっている — doc 側で既知の揺れとして記録されている範囲の差異である。

vcs-06-monitoring-matrix の ledger 記述で「VCS-APPLY-07 は `Unsupported` 実測」とあるが、後続の vcs-05-monitoring-apply-surface slice で同じ VCS-APPLY-07 行が supported-apply 実測（Pass）へ更新されている — 時系列の異なる 2 つの実測が同一 Case ID に対して記録されており、ledger の現行 verdict は後者（Pass）が正と解釈すべきである。

実 GUI/IME/2Window/pixel/plugin dock・実 remote/auth/GC・他 provider・OS matrix・consumer precision は全 slice を通じて `NotRun`/`Unsupported` のまま残る。これらは #114 gate・#97 REAL lanes・#219/#294 の責務であり、本グループの headless Pass を実機受入に換算しない運用が一貫している。設計実験ログは skim 対象であり、個別の実験結果は対応する IR の実測（test 数・lane 結果・ledger verdict）に集約されている。実験段階の検討事項と最終実装の差異は IR の「下された判断」節に記録される。

## 出処

synvelis#92（VCS-01 snapshot resolver）、synvelis#93（VCS-02 diff engine）、synvelis#94（VCS-03 Git read provider）、synvelis#95（VCS-04 History/Visual Diff UI・backend wiring・native mount）、synvelis#96（VCS-05 Safe Apply/Restore・apply surface 拡張）、synvelis#97（VCS-06 fixture/fault matrix・証拠補完）、synvelis#7（VCS Integration 統合契約）、synvelis#335（unborn HEAD 誤分類修正）からの抽出。
