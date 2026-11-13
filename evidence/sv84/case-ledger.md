# SV-84 Case Ledger — StagedFile Migration 受入証拠台帳

Issue #84「DirectoryBundleStore・Snapshot/HEAD 保存を Ruviform
StagedFile へ移行」の受入証拠台帳。`docs/test-specs/
stagedfile-migration.md` の Rule 1–8 を Case ID に写像し、
Expected/Observed/NotRun を OS・SHA・実行環境別に記録する。

**判定規則**（POLICIES.md §3）: `Pass` は実走した結果のみ。
Job green だけでは個別 Case の Pass を示さない — 本台帳は
binary 単位の `test result` 行と fixture test 名まで降りて
記録する。`NotRun`/`Fail`/`Unsupported`/`Deferred` を Pass に
読み替えない。CI leg の Pass と Windows 実機・他 Consumer・
統合受入の証拠は分離する。

## Anchor — 対象 SHA・環境

- **対象コード SHA**: `e0fbbe8b5e14f6c7b65642b122f3e39709da550d`
  — dispatch 時点の `origin/main` HEAD（2026-11-14 実測）。
  その後 `origin/main` は docs-only 2 commit（`7c95957`
  #394・`1947623` #395）進み、現行 `origin/main` =
  `19476233b1e3f959a3973266a657db767571e82f`。
  `git diff --stat e0fbbe8..origin/main` は docs/evidence
  のみでコード差分ゼロ — 本 slice が対象とするコード状態は
  `1947623` でも同一。
- **branch**: `scarecrow173/sv84-stagedfile-migration` —
  `e0fbbe8` 起点 + 本 doc chain/evidence + `origin/main`
  `1947623` の merge（コード差分なし）。
- **Ruviform resolved SHA**: `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`
  （`Cargo.lock`・全 ruviform crate 共通単一 SHA）。
- **実装 merge 済み**: store 移行 PR #178（`b527e0e`）・
  HEAD CAS PR #342（`d653062`、Issue #330）・2-process CAS
  e2e PR #382（`285e1c4`）。
- **Windows 実機**: 本 dispatch の作業機（Windows・ローカル
  NTFS `std::env::temp_dir` 実 FS）。Project 保存先として
  実際に利用する通常ローカル FS に相当。
- **CI covering run（nearest-SHA）**:
  [43457068626](https://github.com/scarecrow173/synvelis/actions/runs/43457068626)
  — `workflow_dispatch`、2026-11-13T15:53:06Z、head_sha
  `285e1c45caf243d4723985f3e6aa62167846fd4c`、全 14 job
  success（`evidence` pr-mode gate・`ci-summary` full-mode
  gate とも green）。
- **nearest-SHA 根拠**（`git diff --stat
  285e1c45..origin/main` 実測・`origin/main`=`1947623`）:
  `crates/synvelis-project/src/store.rs`・
  `tests/{store_publish,head_cas,controller_store}.rs`・
  `crates/synvelis-project/Cargo.toml`・`Cargo.lock` は
  **byte 同一**（diff 空）。synvelis-project 内の差分は
  `host_param.rs`/`ops.rs`/`ppqn.rs`/`lib.rs`（PPQN・
  host-param 系）と `tests/{ctl_02,tim_01_ppqn}.rs` のみで、
  store・fixture・依存宣言に差分ゼロ。`lib.rs` の差分は
  re-export 追加で store module の code path に影響しない
  が、fixture binary は crate 全体を link するため byte
  同一の主張は store+fixture+manifest の範囲に限定して
  記録する。
- **対象 SHA の ci.yml run**: `gh run list --commit
  e0fbbe8`・`--commit 1947623` ともに 0 件 — **NotRun
  （pending）**。ci.yml は workflow_dispatch 専用
  （Issue #145）で、daily-ci.yml（21:29 UTC cron）の次回
  dispatch または手動 dispatch で対象 SHA が検証される。
  統合受入は merge 後の対象 SHA CI 証拠に委ねる
  （POLICIES.md §5）。

## Fixture ↔ Rule ↔ Case 対応

fixture binary: `store_publish`（16 tests）・`head_cas`
（9 tests・うち `two_process_save_cas_single_winner_typed_
conflict` は実 2-process e2e）・`controller_store`
（11 tests）。実 FS = `std::env::temp_dir` 配下の実
ディレクトリ（Mock FS 不使用）。

| Case ID | Rule | 検証内容 | fixture（binary::test） | Expected | Observed |
| --- | --- | --- | --- | --- | --- |
| SV84-C01 | R1 | 異 byte 同名 snapshot の書戻し禁止 | `store_publish::same_revision_resave_never_overwrites_snapshot` | `Err`・`r1.json` byte 不変・HEAD 不変 | Pass（下記 OS 別実走） |
| SV84-C02 | R1 | byte 同一再送の冪等受理・watermark refresh | `store_publish::same_revision_resave_advances_id_watermark`・`in_memory_store_honors_immutable_snapshot_names` | Satisfied/Refresh 受理・Conflict 拒否 | Pass |
| SV84-C03 | R2 | HEAD ordering・snapshot 失敗時 HEAD 不変 | `store_publish::saves_publish_snapshots_then_repoint_head`・`failed_snapshot_publish_keeps_head_and_destination` | snapshot→HEAD 順序・失敗時 HEAD 不変 | Pass |
| SV84-C04 | R2 | HEAD publish 失敗 → orphan+旧 HEAD | `store_publish::failed_head_publish_keeps_snapshot_orphan_and_old_head` | orphan snapshot 残存・旧 HEAD 保持・Err | Pass |
| SV84-C05 | R1 | 受理の耐久性再確立・偽成功禁止 | `store_publish::satisfied_resave_reestablishes_snapshot_durability`・`satisfied_resave_fails_instead_of_reporting_false_success` | Replace 再公開で dir-entry 耐久性再確立・不能なら Err/HEAD 不変 | Pass |
| SV84-C06 | R3 | CAS expected 一致時のみ commit・publish 前 Conflict | `head_cas::save_cas_commits_only_when_head_matches` | 一致 commit・不一致 typed Conflict（orphan 非生成） | Pass |
| SV84-C07 | R4 | Concurrent Save single winner（thread） | `head_cas::concurrent_save_cas_admits_single_winner` | exactly one winner・loser Conflict | Pass |
| SV84-C08 | R4 | Concurrent Save single winner（実 2-process） | `head_cas::two_process_save_cas_single_winner_typed_conflict` | exactly one winner・loser snapshot 非公開・lock/temp 残滓なし | Pass |
| SV84-C09 | R5 | 旧 Writer 上書き禁止 | `head_cas::controller_save_cas_rejects_moved_head` | stale baseline は Conflict・publish されず reload 回復 | Pass |
| SV84-C10 | R6 | Crash window 模擬（stale lock・fresh lock・lock 解放） | `head_cas::stale_head_lock_is_broken`・`fresh_head_lock_blocks_save_until_released`・`cas_failure_releases_the_lock` | stale break・busy 待機・失敗時 lock 解放 | Pass |
| SV84-C11 | R6 | Crash window 模擬（temp 残滓・外部 HEAD・再 Open） | `store_publish::leftover_temp_siblings_do_not_disturb_the_store`・`external_head_rewrite_is_reported_honestly`・`reopen_reproduces_head_and_snapshots`・`empty_bundle_has_no_head` | 実態の正直な報告・再 Open 再現 | Pass |
| SV84-C12 | R7 | Publish ≠ Project Commit | `store_publish::failed_head_publish_keeps_snapshot_orphan_and_old_head` + head_cas fencing tests | publish 成功≠commit・lock 喪失で Ok を返さない | Pass |
| SV84-C13 | R8 | Unsupported store の無条件 save 維持 | `head_cas::unsupported_store_keeps_unconditional_save` | `HeadCasError::Unsupported`・fallback 契約 | Pass |

## OS/実行環境別の実走結果

### Windows 実機（ローカル NTFS・実 FS）— 本 dispatch 実施

- **環境**: Windows（作業機）・`cargo test --locked -p
  synvelis-project`（mise run test synvelis-project）。
  fixture は `std::env::temp_dir`（ローカル NTFS）上の実
  ファイルで実行 — Project 保存先/通常 FS の状態遷移・
  Readback/再 Open・衝突/失敗復旧を対象機能どおり検証。
- **SHA**: `e0fbbe8` + docs/evidence 差分のみ（コード同一）。
- **結果**（2026-11-14 実測・exit 0）:
  - `store_publish`: **16/16 ok**（0.25s）
  - `head_cas`: **9/9 ok**（0.34s — 2-process e2e 含む）
  - `controller_store`: **11/11 ok**（0.27s）
  - package 全体: 41 test binaries・`test result: ok`・
    FAILED 0・Doc-tests 2/2 ok。
- **補助コマンド**: `mise run fmt-check` exit 0、
  `mise run check synvelis-project`・`mise run lint
  synvelis-project`（結果は PR 本文に記録）。
- **Case 判定**: SV84-C01〜C13 全て **Pass**（Windows 実機
  実 FS）。

### OS-L1 CI（nearest-SHA run 43457068626・head_sha `285e1c45`）

job URL:
`https://github.com/scarecrow173/synvelis/actions/runs/43457068626/job/<job-id>`。
`test result` 行は `gh run view --job <id> --log` 実測値。

| leg (job id) | `head_cas` | `store_publish` | `controller_store` | leg 結果 |
| --- | --- | --- | --- | --- |
| verify (ubuntu-latest) `132006742268` | 9/9 ok | 16/16 ok | 11/11 ok | success |
| verify (windows-latest) `132006670018` | 9/9 ok | 16/16 ok | 11/11 ok | success |
| verify-macos `132006666935` | 9/9 ok | 16/16 ok | 11/11 ok | success |
| msrv-verify `132006754106`（rust 1.89） | 9/9 ok | 16/16 ok | 11/11 ok | success |
| rt-verify `132006741582` | `rt_` filter で選択外（0 selected） | 同 | 同 | success |

- rt-verify の `rt_` contract 選択で本 fixture は対象外
  （syn-io-02 fixture は RT no-alloc/no-block 契約の対象外）
  — RT 証拠としては計上しない。workspace `rt_` contract
  suite（strict count 10）は leg success で Pass。
- gw10/ui/dep-guard legs は本 fixture を実行しない lane —
  全て success（dep-guard は ruviform 単一 SHA pin 整合を
  担保）。
- **Case 判定（nearest-SHA・OS-L1）**: SV84-C01〜C13 は
  ubuntu/windows-latest/macos-latest/msrv の 4 leg で
  **Pass**（対象コード SHA（`e0fbbe8`/`1947623` 同一コード）の run は未実施 —
  上記 Anchor どおり NotRun pending と分離）。

### macOS/Linux L1

- macOS: verify-macos leg（macos-latest）で上記 fixture
  全 Pass — **OS-L1 Pass（nearest-SHA）**。L2/L3 は
  利用要件まで **Deferred**。
- Linux: verify (ubuntu-latest) leg で上記 fixture 全
  Pass — **OS-L1 Pass（nearest-SHA）**。L2/L3 は
  **Deferred**。
- 対象コード SHA（`e0fbbe8`/`1947623`）の両 OS leg は ci.yml run 未実施の
  ため **NotRun（pending）** — nearest-SHA 証拠と分離。

### Windows L2/L3（利用中の機能のみ）

- Project 保存先/通常 FS の Snapshot/HEAD CAS/Recovery:
  **Pass** — 上記 Windows 実機実 FS 実走（状態遷移・
  Readback/再 Open・衝突/失敗復旧を fixture が網羅）。
  CI windows-latest leg は補助証拠（GitHub-hosted runner
  であり実機 Case の代替ではない）。
- ネットワーク/クラウド等未使用 FS: **NotApplicable**
  （POLICIES §2: 使わない FS 組合せは必須外）。
- GUI/実 Device/実 Plugin の L2/L3: **NotRun — 別 Owner**
  （#114/#176 系。本 Issue 範囲外）。

### Deferred / NotRun / Unsupported（機能単位）

| 項目 | 要求状態 | 結果 | 根拠・委譲先 |
| --- | --- | --- | --- |
| 対象コード SHA（`e0fbbe8`/`1947623`）の ci.yml run | Required（統合受入） | **NotRun（pending）** | dispatch-only CI。daily-ci/手動 dispatch 待ち。nearest-SHA 証拠は byte 同一範囲確認済み |
| 物理電断（true power-loss） | Deferred | NotRun | 非破壊的に自動化不能。crash window は fixture で模擬済み。real-FS probe 先例は RCD-02 `test-recording-real-fs` lane（recording 側） |
| OS 固有 Indeterminate/DirSync 失敗注入 | Deferred | NotRun | ruviform-io-file 側 fault-injection 責務。consumer は分類転写を pin（C04/C05/C12） |
| Network/Cloud/特異 FS | NotApplicable | NotRun | 未利用 capability |
| 実 GUI/実 Device/実 Plugin L2/L3 | 別 Owner | NotRun | #114/#176 系 |
| macOS/Linux L2/L3 | Deferred（policy） | NotRun | 利用要件発生まで Deferred |
| Ruviform #305 撤去候補との結合 | 統合・受入時 | NotRun | candidate-revision 組合せ検証（Issue #84 統合関係事項） |
| Ruviform #294 scratch candidate-SHA 両 Consumer 独立 Oracle | upstream gate | NotRun | ruviform 側 Issue |
| Ruviform #306 合成 E2E/Fault/OS/FS/Consumer 受入 | upstream OPEN | NotRun | ruviform 側 Issue |
| AudioLens #143 StagedFile 追従の独立受入 | 別 Consumer | NotRun | 片側 Pass を双方に転用しない |
| Synvelis #28 継続追従・#294 VCS・#86 Verified Import・#25 WAV Export | 別 Owner | NotRun | 関係 Issue（本 Issue 範囲外） |

## 責務境界・両 Consumer Golden 確認

- **Publish（ruviform-io-file）**: 単一 file の staged
  write/seal/atomic publish/dir sync/失敗分類/op id/temp/
  recovery handle のみを所有。ProjectId/Revision/Undo の
  概念を持たない。
- **Project Commit（synvelis-project）**: `SnapshotEnvelope`
  schema/canonical byte・immutable `r<N>.json`・HEAD
  pointer・ExpectedRevision/CAS（`HEAD.lock`・fencing）・
  collision policy・Saved/Live revision・Undo/AutoSave・
  Recovery 採用を所有。**Publish 成功 ≠ Project Commit** —
  HEAD repoint が commit 点（SV84-C04/C12 で pin）。
- **Synvelis 側 Golden**: 本台帳の fixture 群
  （store_publish/head_cas/controller_store）。
- **AudioLens 側**: 独自 oracle（alib-02 系）を持ち、
  片側の Pass を他方へ転用しない（AL #143 は別 Consumer
  受入として upstream で追跡）。
- **関係 Issue（参照記録のみ）**: RF #288（CLOSED・IO-06
  Gate 設計正本）・RF #306（OPEN）・SV #28（OPEN）・
  SV #294（OPEN）・RF #305（撤去候補）・AL #143 — 統合・
  受入時の関係事項として `defers` に記録し、本 slice で
  裁定しない。

## 本台帳が成立させるもの・させないもの

- **成立**: StagedFile/CAS/Recovery の実装が main に
  統合済みであること、fixture が Rule 1–8 を実 FS で pin
  していること、Windows 実機・CI 3 OS+MSRV の leg 別
  実走結果、対象 SHA CI 未実施の正直な NotRun 分類、
  責務境界と残ゲートの分類。
- **不成立（主張しない）**: 対象コード SHA 自体の CI 結果、
  物理電断・実 durability、Network/Cloud FS、実
  GUI/Device/Plugin、他 Consumer 受入、無条件の atomic
  directory transaction 保証。
