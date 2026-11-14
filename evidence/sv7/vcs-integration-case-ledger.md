# VCS Integration — Case Ledger (synvelis#7)

Issue #7 の統合契約に対する受入 Case 被覆台帳。VCS-SNAP /
VCS-DIFF / VCS-GIT / VCS-UI / VCS-APPLY / VCS-RESTORE /
VCS-COMPAT の各 Case ID について、責務 Owner・依存・
fixture/test・execution mode・verification layer・
requirement state・result state・evidence path を記録する。

**本 slice は docs/ 設計契約 + evidence 台帳のみ**であり、
実装・GUI・実 device/plugin/OS/Provider/RT/consumer 受入は
対象外。verdict の正本は `evidence/reports/
vcs-06-fixture-matrix-ledger.md`（fixture/headless 実測）と
`evidence/reports/vcs-06-compat-evidence.md`（CI 実測）であり、
本台帳はそれを Issue #7 の Case ID へ写像したものである。
写像時に verdict を変更・昇格しない。

## Status 語彙

| 軸 | 値 | 意味 |
|---|---|---|
| 要求状態 | `Required` | 現行必須 |
| 要求状態 | `Deferred` | 利用条件成立まで保留（実機/GUI/Provider lane） |
| 要求状態 | `NotApplicable` | 対象機能では不要 |
| 実行結果 | `Pass` | 実行した検証が成功（mode/layer を明記） |
| 実行結果 | `Pass*` | persisted/headless 層の Pass のみ（実機受入を意味しない） |
| 実行結果 | `Fail` | 検証が失敗 |
| 実行結果 | `NotRun` | 未実装または未検証 |
| 実行結果 | `Unsupported` | harness/環境が技術的に非対応（根拠を伴う） |
| 実行結果 | `Blocked` | 実装欠落により実行不能 |

headless/fixture/CI の Pass を実 GUI・実 OS durability・実
provider/remote・plugin host・RT・consumer 受入に繰り上げない。
`Pass*` は「stated lane のみ Pass」を意味する。

## Issue #7 受入条件 → Case family 写像

| # | Issue #7 受入条件（要約） | 主な Case family / 記録先 |
|---|---|---|
| AC-1 | 全 Case ID を Owner/fixture 台帳に照合し未実行・非対応も記録 | 本 ledger 全行 |
| AC-2 | 各 VCS Slice の責務・先行関係と矛盾しない | `docs/designs/vcs-integration-design.md` 責務対応表 |
| AC-3 | VCS 非接続でも DAW 制作と Live vs Saved Diff が独立動作 | VCS-UI lifecycle 行（`no_backend_reports_status_*`）+ VCS-COMPAT-01 |
| AC-4 | Immutable Snapshot・Schema/Asset 欠落報告・安定 ID | VCS-SNAP-01〜06、VCS-DIFF-08 |
| AC-5 | Track Rename/Clip Move/FX Reorder/Edge Rewire/Parameter の意味的差分 | VCS-DIFF-01〜07 |
| AC-6 | 不透明 Plugin 状態/Asset 欠落を「変更なし」と誤表示しない | VCS-DIFF-08、VCS-SNAP-06、VCS-UI-04 |
| AC-7 | Merge 親を含む History・任意 A/B・対象別履歴検索 | VCS-GIT-01、VCS-UI-01 |
| AC-8 | 5 view の Visual Diff が同一 ChangeSet に整合 | VCS-UI-01/02（共有 ChangeSet 契約）+ design §ChangeSet 共有不変条件 |
| AC-9 | Source PCM vs Rendered Output の区別・条件・失敗/未取得表示 | design §8 view 判定（Audio = 延期）+ VCS-DIFF determinism 行。実装は Deferred |
| AC-10 | History 閲覧中も Project/Transport/AudioGraph 不変 | VCS-UI lifecycle/read-only 行（`vcs_ops_never_touch_project_engine_or_transport`） |
| AC-11 | Commit 対象の Staged/Saved/Live 差・欠落 Asset/未登録警告 | VCS-SNAP-04、VCS-GIT-02/03、VCS-UI-04 |
| AC-12 | Apply は検証付き Undo 可能 Project Command、Restore は失敗時に現作業を守る | VCS-APPLY-01〜08、VCS-RESTORE-01〜09 |
| AC-13 | VCS 処理は RT スレッドに流れず非同期実行可能 | design §Non-Functional（non-RT 契約）。RT 実測 lane は VCS-*-REAL-* = NotRun |
| AC-14 | mockup HTML 8 画面・主要インタラクションの採否記録 | design §Issue #7 §6 Visual Diff 8 view 採否判定 |

## 責務 Owner 写像

| Case family | 責務 spec（正本） | Owner Issue |
|---|---|---|
| VCS-SNAP-* | `docs/specs/vcs-01-immutable-snapshot-resolver.md` | #92 |
| VCS-DIFF-* | `docs/specs/vcs-02-semantic-diff-changeset.md`（+ `vcs-06-monitoring-matrix` 拡張） | #93（拡張行は #96/#97 slice） |
| VCS-GIT-* | `docs/specs/vcs-03-git-read-provider.md`、`vcs-03-unborn-head-classification.md` | #94 |
| VCS-UI-* | `docs/specs/vcs-04-history-visual-diff-ui.md`、`vcs-04-backend-wiring.md`、`vcs-04-native-mount.md` | #95（実 GUI は #114） |
| VCS-APPLY-* / VCS-RESTORE-* | `docs/specs/vcs-05-apply-restore-headless.md`、`vcs-05-monitoring-apply-surface.md` | #96 |
| VCS-COMPAT-* | `docs/specs/vcs-06-*.md` 群 | #97 |

## mode / layer 凡例

- `mode`: `headless`（in-process fixture/temp FS）/ `real-git`
  （実 temp git repo を git CLI で操作、read-only oracle 付き）/
  `ci`（GitHub Actions run）/ `gui`（実 GPUI/OS window）。
- `layer`: L1 unit/contract・L2 component・L3 in-process E2E・
  L4 FS/process fault・L5 system/OS。
- Environment 正本: `vcs-06-fixture-matrix-ledger.md` の
  Environment ブロック（Windows/NTFS・rustc 1.99.0・
  git 2.54.0.windows.1・schema v13・Ruviform SHA `fd1d4d5f`）。

## Fixture 凡例（実在 file）

- `crates/synvelis-project/tests/`: `vcs01_snapshot.rs`・
  `vcs02_diff.rs`・`vcs05_apply.rs`・`vcs05_restore.rs`・
  `vcs06_diff_matrix.rs`・`vcs06_apply_matrix.rs`・
  `vcs06_restore_matrix.rs`・`vcs06_monitoring_matrix.rs`・
  `vcs06_plugin_instance_matrix.rs`
- `crates/synvelis-library/tests/`: `vcs03_git_provider.rs`・
  `vcs06_git_matrix.rs`
- `crates/synvelis-app/tests/`: `vcs04_history.rs`・
  `vcs04_native_mount.rs`・`vcs04_mount_gpui.rs`・
  `vcs06_ui_matrix.rs`
- `crates/synvelis-workspace/tests/`: `vcs04_history.rs`
- 証拠文書: `evidence/reports/vcs-06-fixture-matrix-ledger.md`・
  `evidence/reports/vcs-06-compat-evidence.md`

## VCS-SNAP-01..06 — immutable snapshot resolver（Owner #92）

Lane: headless capture/resolve のみ。verdict 正本:
`vcs-06-fixture-matrix-ledger.md` §VCS-SNAP。

| Case | 期待（要約） | Fixture | Mode/Layer | Req | Result | Evidence |
|---|---|---|---|---|---|---|
| VCS-SNAP-01 Captured Live | live capture が revision+digest を pin、編集/undo/save で不変 | `vcs01_snapshot.rs::live_capture_is_immutable_across_edits_undo_and_save` | headless/L3 | Required | Pass* | ledger SNAP-01 |
| VCS-SNAP-02 Saved HEAD | saved resolve が HEAD pin、window 内 HEAD 移動 → HeadConflict | `vcs01_snapshot.rs::saved_head_movement_inside_window_is_head_conflict` 他 | headless/L3 | Required | Pass* | ledger SNAP-02 |
| VCS-SNAP-03 Recovery | recovery checkpoint が saved HEAD から分離 | `vcs01_snapshot.rs::recovery_checkpoint_is_isolated_from_saved_head` | headless/L3 | Required | Pass* | ledger SNAP-03 |
| VCS-SNAP-04 VCS fixed revision | staged/committed locator が port 経由で resolve、generation/index drift → IndexChanged、partial stage 明示 | `vcs01_snapshot.rs::vcs_staged_*`/`vcs_committed_*` | headless/L3 | Required | Pass* | ledger SNAP-04 |
| VCS-SNAP-05 old-v5 projection | v5 envelope が read-only で現 schema へ投影 | `vcs01_snapshot.rs::v5_snapshot_projects_read_only_into_current_schema` | headless/L3 | Required | Pass* | ledger SNAP-05 |
| VCS-SNAP-06 unknown/corrupt/missing | unknown schema reject、corrupt → 明示失敗、missing/unknown asset → Degraded | `vcs01_snapshot.rs` unknown/corrupt/degraded 系 | headless/L3 | Required | Pass* | ledger SNAP-06 |
| VCS-SNAP read-only oracle | resolve が controller/store/VCS port を変更しない | `resolution_never_mutates_controller_store_or_vcs_port` 他 | headless/L3 | Required | Pass* | ledger SNAP oracle |
| VCS-SNAP-REAL-* | live GUI session・RT thread・実 provider E2E | — | gui/L5 | Deferred | NotRun | #114 + device/RT lanes |

## VCS-DIFF-01..10 — semantic diff / ChangeSet（Owner #93、拡張行は #96/#97 slice）

Lane: headless compare のみ。verdict 正本: 同 §VCS-DIFF。

| Case | 期待（要約） | Fixture | Mode/Layer | Req | Result | Evidence |
|---|---|---|---|---|---|---|
| VCS-DIFF-01 Track/Lane/Clip | add/remove/modify/move が意味的に分類 | `vcs02_diff.rs` + `vcs06_diff_matrix.rs` | headless/L2 | Required | Pass | ledger DIFF-01 |
| VCS-DIFF-02 Note propagation | clip-local note 編集/追加 → typed Note change | `vcs06_diff_matrix.rs::clip_note_edit_and_addition_are_typed_changes` | headless/L2 | Required | Pass | ledger DIFF-02 |
| VCS-DIFF-03 lane ordering | 同集合 reorder → `Reordered`（entity churn なし） | `vcs06_diff_matrix.rs::lane_order_change_is_reordered_not_modified` | headless/L2 | Required | Pass | ledger DIFF-03 |
| VCS-DIFF-04 FX/routing | processor reorder → `Reordered`、connection rewire/add/remove typed | `vcs06_diff_matrix.rs` FX/routing 系 | headless/L2 | Required | Pass | ledger DIFF-04 |
| VCS-DIFF-05 Automation | region point 編集 → `Modified`、interpolation shape 変化検出 | `vcs06_diff_matrix.rs` automation 系 | headless/L2 | Required | Pass | ledger DIFF-05 |
| VCS-DIFF-06 Asset | content swap → `Replaced`、追加 → `Added` | `vcs06_diff_matrix.rs` asset 系 | headless/L2 | Required | Pass | ledger DIFF-06 |
| VCS-DIFF-07 MIDI note payload | velocity 編集 → `Modified`、追加 → `Added` | `vcs06_diff_matrix.rs` midi 系 | headless/L2 | Required | Pass | ledger DIFF-07 |
| VCS-DIFF-08 Unknown/Ambiguous | missing/unknown asset・opaque state・cross-kind collision・owner conflict・cross-project → `Unknown`/`Ambiguous`、決して `Equal` にならない | `vcs06_diff_matrix.rs` + `vcs02_diff.rs` unknown 系 | headless/L2 | Required | Pass | ledger DIFF-08 |
| VCS-DIFF determinism/cache/budget | AA equal・cache identity・budget/cancel 明示 | `vcs02_diff.rs` cache/budget 系 | headless/L2 | Required | Pass | ledger DIFF determinism |
| VCS-DIFF-09 TrackMonitorDecl（拡張） | decl upsert/edit/clear が typed change、`Recording` domain、AA=Equal | `vcs06_monitoring_matrix.rs::track_monitor_decl_add_modify_clear_are_typed_changes` | headless/L2 | Required | Pass | ledger DIFF-09 |
| VCS-DIFF-10 PunchMarks（拡張） | presence 遷移が typed change、`Recording` domain | `vcs06_monitoring_matrix.rs::punch_marks_presence_transitions_are_typed_changes` | headless/L2 | Required | Pass | ledger DIFF-10 |
| VCS-DIFF real plugin/consumer diff view | 実 plugin/consumer の diff view | — | gui/L5 | Deferred | NotRun | consumer/precision #219/#294 |

## VCS-GIT-01..06 — git read provider（Owner #94）

Lane: 実 temp git repo（git CLI）+ read-only oracle。verdict 正本:
同 §VCS-GIT。

| Case | 期待（要約） | Fixture | Mode/Layer | Req | Result | Evidence |
|---|---|---|---|---|---|---|
| VCS-GIT-01 fixed Commit/Tree/Parent | branch/tag move に対し fixed revision 不変、merge は全 parent 保持 | `vcs03_git_provider.rs` + `vcs06_git_matrix.rs` | real-git/L3 | Required | Pass | ledger GIT-01 |
| VCS-GIT-02 Index/WorkTree/Commit separation | staged snapshot が index generation を pin、drift → IndexChanged | `vcs03_git_provider.rs` staged 系 + vcs01 | real-git/L3 | Required | Pass | ledger GIT-02 |
| VCS-GIT-03 Partial stage / missing assets | partial stage 明示、untracked asset Missing（silent-equal 禁止）、missing LFS degraded | `vcs03_git_provider.rs` + `vcs06_git_matrix.rs` | real-git/L3 | Required | Pass | ledger GIT-03 |
| VCS-GIT-04 identity | repository_id vs project path 分離、SHA-1/SHA-256 object format と ContentDigest の区別 | `vcs03_git_provider.rs` identity 系 | real-git/L3 | Required | Pass | ledger GIT-04 |
| VCS-GIT-05 symlinks/gitlinks/submodule | symlink mode 保持・link target bytes 非追従、gitlink は report のみ | `vcs03_git_provider.rs::gitlink_entries_are_reported_not_fetched`、`vcs06_git_matrix.rs::symlink_index_entry_keeps_link_target_bytes` | real-git/L4 | Required | Pass | ledger GIT-05 |
| VCS-GIT-06 cancel/secret/budget | cancel が全 op を abort、credential material 非露出、history page clamp | `vcs03_git_provider.rs` + `vcs06_git_matrix.rs` | real-git/L2 | Required | Pass | ledger GIT-06 |
| VCS-GIT read-only oracle | checkout/index/worktree/HEAD・project session を変更しない | `exercise_provider`/`repo_oracle` invariance | real-git/L3 | Required | Pass | ledger GIT oracle |
| VCS-GIT-REAL-01 packed object/ref read | pack-refs/repack/gc 後も fixed revision/HEAD/tag/branch 解決と staged index read 不変 | `vcs06_git_matrix.rs::gc_pack_refs_and_repack_keep_fixed_revision_reads` | real-git/L4 | Required | Pass | ledger GIT-REAL-01 |
| VCS-GIT-REAL-02 pruned revision → typed error | 真に object 除去された revision は `RevisionGone`（silent success ではない） | `vcs06_git_matrix.rs::pruned_commit_is_revision_gone_not_silent` | real-git/L4 | Required | Pass | ledger GIT-REAL-02 |
| VCS-GIT-REAL-03 file:// remote force-push+fetch | file:// remote で pinned revision の OID/bytes 不変、origin/main のみ前進 | `vcs06_git_matrix.rs::file_remote_force_push_fetch_preserves_pinned_revision` | real-git/L4 | Required | Pass | ledger GIT-REAL-03（local transport のみ。認証 http/ssh は含まない） |
| VCS-GIT-REAL-04 linked worktree index scope | linked worktree の stage が元 worktree provider の index state に現れない | `vcs06_git_matrix.rs::linked_worktree_index_scope_is_isolated` | real-git/L4 | Required | Pass | ledger GIT-REAL-04 |
| VCS-GIT-REAL-* residual | 認証付き http/ssh remote・実 provider E2E・OS permission matrix | — | real-git/L5 | Deferred | NotRun | real remote + OS matrix |

## VCS-UI-01..05 — history/diff UI（Owner #95）

Lane: headless `syn.vcs.*` ops + screen-spec projection +
gpui-test scenario。実 GPUI/OS window/IME/keyboard は #114。
verdict 正本: 同 §VCS-UI。

| Case | 期待（要約） | Fixture | Mode/Layer | Req | Result | Evidence |
|---|---|---|---|---|---|---|
| VCS-UI-01 fixed A/B + parent selection | pinned endpoints が 1 つの共有 ChangeSet を生成、merge `after` が chosen parent を記録 | `vcs04_history.rs`（app+workspace）、`vcs06_ui_matrix.rs` | headless/L3 | Required | Pass* | ledger UI-01 |
| VCS-UI-02 SideBySide/Focus view modes | `view_mode` 切替は同一 ChangeSet/Target 上の display-only | `vcs04_history.rs`（workspace） | headless/L2 | Required | Pass* | ledger UI-02 |
| VCS-UI-03 two-window/dock | dock move/window detach が VCS view state を保持（headless）。実 OS 2 window は未計測 | `vcs04_history.rs`（workspace/app）、`vcs04_native_mount.rs` | headless/L3 | Required | Pass*（実 OS window lane NotRun） | ledger UI-03 + `vcs04_mount_gpui.rs` |
| VCS-UI-04 Unknown/Stale/Partial never zero-change | cross-project → `unknown`+issues、absent asset → `Partial`+issue rows、stale freshness、live revision 前進 → `stale` | `vcs06_ui_matrix.rs` + `vcs04_history.rs` + `vcs02_diff.rs` | headless/L3 | Required | Pass* | ledger UI-04 |
| VCS-UI-05 keyboard/IME/a11y/real GUI | 実 GPUI input・IME・a11y・native window 証拠 | — | gui/L5 | Deferred | NotRun | #114（external gate） |
| VCS-UI lifecycle/read-only | kind-scoped open/close、foreign repo/bad params 正直な error、no-backend status、ops が project/engine/transport を触らない | `vcs04_history.rs`（app+workspace） | headless/L3 | Required | Pass* | ledger UI lifecycle |

## VCS-APPLY-01..08 — apply selected change（Owner #96）

Lane: headless `apply_selected` + 実 `DirectoryApplyJournal`（temp FS）。
verdict 正本: 同 §VCS-APPLY。

| Case | 期待（要約） | Fixture | Mode/Layer | Req | Result | Evidence |
|---|---|---|---|---|---|---|
| VCS-APPLY-01 selected apply + undo | apply が 1 回 commit、ちょうど 1 undo step | `vcs05_apply.rs` + `vcs06_apply_matrix.rs` | headless/L3 | Required | Pass | ledger APPLY-01 |
| VCS-APPLY-02 approval + CAS | empty ticket → Unapproved、stale revision → StaleRevision、fingerprint drift → FingerprintMismatch、wrong source → ChangeSetMismatch、unknown change id → UnknownChange | `vcs05_apply.rs` rejection 系 | headless/L3 | Required | Pass | ledger APPLY-02 |
| VCS-APPLY-03 fail-closed content | Removed → Unsupported、missing/unknown asset → MissingAsset/AssetUnknown、unsupported field → UnsupportedField、absent target → TargetMissing、live ID collision → TargetConflict、unavailable plugin → MissingPlugin、Unknown/Partial changeset → NotApplicable | `vcs05_apply.rs` + `vcs06_apply_matrix.rs` | headless/L3 | Required | Pass | ledger APPLY-03 |
| VCS-APPLY-04 replay/crash | 同 intent → Replayed（二重 mutation なし）、Pending-without-commit retry、ACK gap reconcile、on-disk journal が restart を生き残る | `vcs05_apply.rs` + `vcs06_apply_matrix.rs::directory_journal_survives_restart_and_replays` | headless/L4 | Required | Pass | ledger APPLY-04 |
| VCS-APPLY-05 cancel | cooperative cancel → `Cancelled`、mutation なし | `vcs06_apply_matrix.rs::cancelled_intent_fails_closed_before_mutation` | headless/L3 | Required | Pass | ledger APPLY-05 |
| VCS-APPLY-06 independence | Saved HEAD / VCS index / git worktree 不変、明示 apply 前に write なし | `vcs05_apply.rs` receipt `effects.untouched.*` | headless/L3 | Required | Pass* | ledger APPLY-06 |
| VCS-APPLY-07 monitoring kinds（拡張） | `TrackMonitorDecl`/`PunchMarks` の Added/Modified が typed entity transplant、Removed は fail-closed | `vcs06_monitoring_matrix.rs` + `vcs05_apply.rs` monitor 系 | headless/L3 | Required | Pass | ledger APPLY-07 |
| VCS-APPLY-08 journal FS fault (Windows) | journal record の share-lock/readonly は mutation 前に typed `ApplyError::Journal`、解除後に元の typed failure へ回復 | `vcs06_apply_matrix.rs`（`#[cfg(windows)]`） | headless/L4 | Required | Pass（Win32 NTFS のみ。POSIX は NotRun） | ledger APPLY-08 |
| VCS-APPLY-REAL-* | real engine hot-swap・GUI approval UX | — | gui/L5 | Deferred | NotRun | engine/GUI lanes external |

## VCS-RESTORE-01..09 — restore as new copy（Owner #96）

Lane: 実 temp FS staging→manifest→readback→atomic rename。
verdict 正本: 同 §VCS-RESTORE。

| Case | 期待（要約） | Fixture | Mode/Layer | Req | Result | Evidence |
|---|---|---|---|---|---|---|
| VCS-RESTORE-01 publish+readback+replay | verified bundle publish、envelope/HEAD/blobs/receipt 可読、replay 冪等、source project/Git 不変 | `vcs05_restore.rs::restore_publishes_verified_new_copy` | headless/L4 | Required | Pass | ledger RESTORE-01 |
| VCS-RESTORE-02 missing/degraded assets | MissingAsset は fail-closed（dest なし）、AllowDegraded は marked copy を publish | `vcs05_restore.rs` asset 系 | headless/L4 | Required | Pass | ledger RESTORE-02 |
| VCS-RESTORE-03 crash/resume/readback | materialize 中断 → staging 保持、resume → Published、tampered staged envelope → Validation、foreign/orphan staging → StagingConflict | `vcs06_restore_matrix.rs` staging 系 | headless/L4 | Required | Pass | ledger RESTORE-03 |
| VCS-RESTORE-04 plugin state | processor は plugin-state source 必須（`PluginStateRequired`）、manifest+blobs を `plugin-state/` へ verbatim copy | `vcs06_restore_matrix.rs::plugin_state_manifest_and_blobs_are_copied` | headless/L4 | Required | Pass*（実 plugin host restore は NotRun） | ledger RESTORE-04 |
| VCS-RESTORE-05 destination safety | existing dest → DestinationExists、relative/`CON`/`..` → InvalidRequest、case-conflict sibling → DestinationExists、low disk → LowDisk、cancel → 無書込 | `vcs05_restore.rs` + `vcs06_restore_matrix.rs` dest 系 | headless/L4 | Required | Pass | ledger RESTORE-05 |
| VCS-RESTORE-06 independent identity | 新 ProjectId ≠ source、stable ID 保持、source Saved HEAD/Git index/worktree 不変 | `vcs05_restore.rs` receipt effects | headless/L4 | Required | Pass* | ledger RESTORE-06 |
| VCS-RESTORE-07 monitoring verbatim（拡張） | decl+punch marks が restore-as-copy で verbatim、生き残る fork `project_id` のみ再採番 | `vcs06_monitoring_matrix.rs::restore_as_copy_preserves_monitoring_verbatim` | headless/L4 | Required | Pass | ledger RESTORE-07 |
| VCS-RESTORE-08 plugin_instances verbatim（拡張） | issued `PluginInstanceId` + `project.plugin_instances` が envelope readback で verbatim | `vcs06_plugin_instance_matrix.rs::restore_as_copy_preserves_plugin_instances_verbatim` | headless/L4 | Required | Pass | ledger RESTORE-08 |
| VCS-RESTORE-09 staging FS fault (Windows)（拡張） | staging manifest/HEAD の lock/readonly は publish 前に typed `RestoreError::Io`、staging 保持・dest 不在、解除後 retry で Published | `vcs06_restore_matrix.rs`（`#[cfg(windows)]`） | headless/L4 | Required | Pass（Win32 NTFS のみ。POSIX は NotRun） | ledger RESTORE-09 |
| VCS-RESTORE-REAL-* | OS crash/power-loss durability・AV・実 plugin host・consumer open | — | gui/L5 | Deferred | NotRun | OS matrix + #219/#294 |

## VCS-COMPAT-01..04 — cross-gate compatibility（Owner #97）

| Case | 期待（要約） | Fixture/証拠 | Mode/Layer | Req | Result | Evidence |
|---|---|---|---|---|---|---|
| VCS-COMPAT-01 schema compat (v5→current) | 旧 snapshot が read-only 投影、unknown/too-old reject、source 無改変 | SNAP-05/06 rows | headless/L2 | Required | Pass* | ledger COMPAT-01 |
| VCS-COMPAT-02 Gate A / consumer precision | AudioLens/consumer cross-check | — | — | Deferred | NotRun | #219/#294 |
| VCS-COMPAT-03 CI evidence | CI run の証拠行 | covering run [43428177865](https://github.com/scarecrow173/synvelis/actions/runs/43428177865)（head `7c206289`・manual workflow_dispatch）全 leg success・vcs06 全 6 binary が ubuntu/windows/macos/msrv/rt-verify で実行・pass | ci/L5 | Required | Pass（最新 covering run。**本 branch の新規 docs 変更に対する CI run は merge 後の次回 dispatch 待ち = CI lane NotRun**） | `evidence/reports/vcs-06-compat-evidence.md` |
| VCS-COMPAT-04 other OS (macOS/Linux) | 同 matrix を POSIX FS で | hosted macOS verify leg は run 42938180676 で success 観測済みだが製品 OS matrix 実機計測ではない | gui/L5 | Deferred | NotRun | ledger COMPAT-04 |

## 明示的に Pass にしない領域（ledger §out of scope を継承）

- VCS Writer / 3-way semantic merge — 未実装、**Blocked**。
- Git 以外の VCS provider（SVN/Perforce 系）— 本 slice では
  **Unsupported**（`VcsCapabilities`/`VcsError::
  UnsupportedCapability` で誠実に宣言）。
- 実 GUI/IME/two-window（#114）、native plugin dock（#176）、
  RT/device、consumer/precision（#219/#294）、実 OS matrix —
  **NotRun**。
- Creative Branch（#277〜297 系）— VCS Case ではなく別契約。
  本 ledger は分離の不変条件のみを design に記録する。

## 集計（本 slice が転記する verdict 分布）

| family | Pass | Pass* | NotRun | Unsupported | Blocked |
|---|---|---|---|---|---|
| VCS-SNAP | 0 | 7 | 1（REAL） | 0 | 0 |
| VCS-DIFF | 11 | 0 | 1（real plugin/consumer） | 0 | 0 |
| VCS-GIT | 11 | 0 | 1（REAL residual） | 0 | 0 |
| VCS-UI | 0 | 5 | 1（UI-05） | 0 | 0 |
| VCS-APPLY | 7 | 1 | 1（REAL） | 0 | 0 |
| VCS-RESTORE | 7 | 2 | 1（REAL） | 0 | 0 |
| VCS-COMPAT | 1 | 1 | 2 | 0 | 0 |

注: `Pass`/`Pass*` は全て headless/fixture/real-git/CI lane の
実測であり、実 GUI・実 OS durability・実 provider remote・plugin
host・RT・consumer の受入を意味しない。
