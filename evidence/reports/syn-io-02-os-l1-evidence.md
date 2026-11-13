# SYN-IO-02 OS-L1 Evidence — StagedFile/CAS/Recovery CI照合 (synvelis#84)

Issue #84「DirectoryBundleStore Snapshot/HEAD → Ruviform
StagedFile 移行」の OS-L1 証拠照合記録。**CI leg の green を
実 OS 受入・実機 durability・consumer 受入に換算しない** —
本書は既存 CI で実走した headless fixture の SHA/Run URL 付き
証跡と、残ゲートの Required/Deferred/NotRun 分類のみを記録
する（POLICIES.md §3: Job green だけでは個別 Case の Pass を
示さない — 本書は binary 単位の `test result` 行まで降りて
記録する）。

収集日時: 2026-11-13T13:41Z（`gh run list` / `gh run view
--log` / `git fetch origin main` 実測）。

## Anchor — 対象 SHA と実装状態

- **対象 SHA**: `c0b000b3bdbdd31532e3f26805326fa0240e8f65`
  — 収集時点の `origin/main` HEAD（`git rev-parse
  origin/main` で再確認）。PR #380（VCS-06 real-git/FS fault
  slice）の merge commit、merged 2026-11-13T13:37:02Z。
- **実装は main 統合済み**（コード確認・読み取りのみ）:
  - StagedFile 移行 — PR #178 `b527e0e`（merged
    2026-11-07T03:18:12Z）: `DirectoryBundleStore::
    write_atomic` 撤去、`publish_staged` が
    `ruviform_io_file::{StagedFile, seal→publish}` を使用。
    snapshot = `PublishMode::CreateNew` +
    `Durability::FileAndDirectorySync`、HEAD = `Replace` +
    同 durability。`snapshot_collision` が
    Satisfied/Refresh/Conflict を分類し、衝突受理は勝者 byte
    の完全 publish 再実行（dir-entry 耐久性の再確立）。
    `PublishFailure` の effect/phase/durability を
    `Error::Store` へ転写、`Indeterminate` の recovery
    handle は自動削除しない。
  - HEAD CAS — PR #342 `d653062`（Issue #330、merged
    2026-11-11T22:32:41Z）: `ProjectStore::save_cas` +
    typed `HeadCasError`。`DirectoryBundleStore` は
    `HEAD.lock`（`CreateNew` + `BestEffort`、stale 20s で
    rename-aside break、busy 30s）内で expected-head を照合、
    pre/post publish fencing で lock 喪失時に `Ok` を返さない。
    `InMemoryStore` parity、`ProjectController::save` が
    `saved.head` を expected に供給し `Unsupported` のみ
    無条件 `save` へ fallback、`synvelis-gateway` journal
    restore も `save_cas` 化。
  - `Cargo.lock` の ruviform 解決 rev:
    `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`（全 ruviform
    crate 共通・単一 SHA — `dep-guard` leg green）。
  - StagedFile の他 consumer（同 crate 内・本 Issue 範囲外）:
    `apply/journal.rs`、`plugin_state.rs`（
    `DirectoryPluginStateStore`）、`restore/mod.rs` も同一
    `ruviform-io-file` API を使用 — Project Commit CAS は
    `store.rs` 固有、publish 基盤は共有（後述の責務境界）。
- **doc chain（実装済み・本 slice で再利用）**:
  `docs/specs|designs|plans|tasks|test-specs/syn-io-02-*`、
  `docs/impl/ir/syn-io-02-store-stagedfile-migration.md`、
  `docs/impl/ir/syn-io-02-head-cas.md`。

## CI run 選定

`gh run list --workflow ci.yml`（2026-11-13T13:41Z 実測）:

- **対象 SHA `c0b000b3` の ci.yml run は未実施** — merge
  （13:37:02Z）が最新 ci.yml run（12:47:38Z）より後で、
  `daily-ci.yml` の次回 cron dispatch（21:29 UTC）も未到来。
  正確な SHA の CI 証拠は **NotRun（pending）** として記録し、
  推測で補わない。
- **採用した covering run**:
  [43428177865](https://github.com/scarecrow173/synvelis/actions/runs/43428177865)
  — `workflow_dispatch`（actor `scarecrow173`）、
  2026-11-13T12:47:38Z、head_sha
  `7c206289b0d72af54aaf753ca5d94b9786a58c45`、**全 14 job
  success**（`evidence` pr-mode gate・`ci-summary`
  full-mode gate とも green）。
- **SHA 差分の照合**（`git merge-base --is-ancestor` +
  `git diff --stat 7c206289..c0b000b3` 実測）:
  `7c206289` は対象 SHA の祖先。差分は PR #379
  （`.pre-commit-config.yaml`）と PR #380（VCS-06
  `vcs06_{git,apply,restore}_matrix.rs` の test 行追加、
  docs chain、evidence reports）のみで、**`crates/
  synvelis-project/src/**` と `tests/{head_cas,
  store_publish,controller_store}.rs` に差分ゼロ** —
  本書が証拠とする fixture binary と被検査の store 実装は
  run SHA と対象 SHA で byte 同一。したがって本 run の
  該当 leg 結果は対象 SHA のコードへそのまま適用できるが、
  **c0b000b3 自体の CI run ではない**点を明記して使う
  （nearest-SHA evidence）。

## Per-leg fixture 結果（run 43428177865・head_sha `7c206289`）

job URL は
`https://github.com/scarecrow173/synvelis/actions/runs/43428177865/job/<job-id>`。
`test result` 行は `gh run view --job <id> --log` の実測値
（binary 単位・全 leg で workspace `cargo test --locked` 系）。

| leg (job id) | `head_cas` | `store_publish` | `controller_store` | leg 結果 |
| --- | --- | --- | --- | --- |
| verify (ubuntu-latest) `131894707867` | 8/8 ok | 16/16 ok | 11/11 ok | success |
| verify (windows-latest) `131894707855` | 8/8 ok | 16/16 ok | 11/11 ok | success |
| verify-macos `131894707388` | 8/8 ok | 16/16 ok | 11/11 ok | success |
| msrv-verify `131894707856`（rust 1.89） | 8/8 ok | 16/16 ok | 11/11 ok | success |
| rt-verify `131894707753` | binary 実行・`rt_` filter で 8 filtered / 0 selected | 同・16 filtered / 0 selected | 同 | success |

- `store_publish.rs` の
  `satisfied_resave_fails_instead_of_reporting_false_success`
  は `#[cfg(any(unix, windows))]` — Windows leg でも実走
  （16/16 に含む）。macOS leg も unix として実走。
- rt-verify は `rt_` contract test 選択 — syn-io-02 fixture
  は RT no-alloc/no-block 契約の対象外であり、binary が
  filter 下で実行され 0 selected となるのは期待どおり
  （RT 証拠としては計上しない）。
- gw10-verify（ubuntu/windows/macos）・ui-verify（同）・
  dep-guard は本 fixture を実行しない lane（gw10 = gateway
  harness、ui = frontend typecheck/build、dep-guard =
  lockfile/dep guard）。全て success — dep-guard は ruviform
  単一 SHA pin の整合を担保。
- **workspace coverage の範囲**（Recording/WAV/RT/Feature
  の受入行に対応）: verify legs の `mise run test-full` は
  workspace 全 package の `cargo test` を実行し、ubuntu leg
  のみで 258 binary の `test result: ok`・FAILED 0 を確認
  （synvelis-project 以外の `recording.rs`、
  `staged_publish.rs`（7 tests）、`l2_e2e.rs`、
  `sf03_render.rs`、`rcd_04_rt_comp_pcm.rs` 等の
  Recording/WAV/Render 系 binary も同一 leg で実行 —
  `cargo test` は fail-closed のため leg success = 全
  binary Pass）。rt-verify は `mise run test-rt` の `rt_`
  contract 選択を実行し leg success（選択集合の RT 契約は
  Pass — syn-io-02 fixture は選択外・上記どおり）。
  **Feature**: `synvelis-project` の `Cargo.toml` に
  `[features]` 節はなく非既定 feature を持たない — 本
  fixture への Feature Matrix 適用は非該当（default
  features のみで上記結果）。

## Fixture ↔ 受入照合（Issue #84 該当行）

「Snapshot/HEAD/CAS/ExpectedRevision/Concurrent Save/Crash
Simulation/Recovery の Headless Fixture で書戻り・二重
Publish・旧 Writer 上書き・Orphan の禁止」を test-spec
Rule ↔ test ↔ 実走 leg で照合:

| 要求 | test-spec Rule | fixture（binary::test） | 実走 leg | 結果 |
| --- | --- | --- | --- | --- |
| Snapshot immutable name・異 byte 書戻し禁止 | store-publish R1 | `store_publish::same_revision_resave_never_overwrites_snapshot` | ubuntu/win/mac/msrv | Pass |
| 同名 CreateNew の二重 Publish 禁止（single winner） | store-publish 末尾 Rule | `store_publish::concurrent_same_name_publish_admits_one_winner` | 同上 | Pass |
| HEAD ordering（snapshot publish→HEAD repoint） | store-publish R2 | `store_publish::saves_publish_snapshots_then_repoint_head`、`failed_snapshot_publish_keeps_head_and_destination` | 同上 | Pass |
| HEAD publish 失敗 → orphan + 旧 HEAD 保持 | store-publish R3 | `store_publish::failed_head_publish_keeps_snapshot_orphan_and_old_head` | 同上 | Pass |
| CAS/ExpectedRevision 一致時のみ commit・stale は typed Conflict で publish 前判定（orphan 非生成） | head-cas R1 | `head_cas::save_cas_commits_only_when_head_matches` | 同上 | Pass |
| Concurrent Save — 同 expected の 2 writer で exactly one winner | head-cas R2 | `head_cas::concurrent_save_cas_admits_single_winner` | 同上 | Pass |
| 旧 Writer 上書き禁止 — stale-baseline controller の divergent snapshot は publish されず HEAD 不変・reload で回復 | head-cas R3 | `head_cas::controller_save_cas_rejects_moved_head` | 同上 | Pass |
| Crash window orphan の再利用（Satisfied 再 save で HEAD 再指向・load 回復） | store-publish R1/R8 | `store_publish::satisfied_resave_repoints_head_onto_orphan_snapshot` | 同上 | Pass |
| DirectorySync 未証明の受理は耐久性を再確立（rename で物理更新）・不能なら Err で HEAD 不変 | store-publish R8 | `store_publish::satisfied_resave_reestablishes_snapshot_durability`、`satisfied_resave_fails_instead_of_reporting_false_success` | 同上（unix+windows cfg） | Pass |
| 外部 HEAD 書換え・dangling name の正直報告 | store-publish R5 | `store_publish::external_head_rewrite_is_reported_honestly` | 同上 | Pass |
| `*.rvf-tmp-*`/`HEAD.lock.stale-*` 残滓は保存データとして保持・非干渉 | store-publish R5、head-cas R6/R7 | `store_publish::leftover_temp_siblings_do_not_disturb_the_store`、`head_cas::stale_head_lock_is_broken`、`fresh_head_lock_blocks_save_until_released`、`cas_failure_releases_the_lock` | 同上 | Pass |
| Reopen/空 bundle/byte-faithful/InMemory parity/Unsupported fallback | store-publish R6/R7、head-cas R4/R5 | `store_publish::{reopen_reproduces_head_and_snapshots,empty_bundle_has_no_head,save_round_trips_bytes_verbatim,in_memory_store_honors_immutable_snapshot_names}`、`head_cas::{in_memory_store_save_cas_parity,unsupported_store_keeps_unconditional_save}` | 同上 | Pass |
| Saved/Live Revision・Undo・load/migration の非退行 | 既存契約 | `controller_store` 11 tests（save/reload・undo/redo・schema reject・directory_bundle HEAD-last） | 同上 | Pass |

補足: 「Orphan の禁止」の正確な契約は「orphan を残さない」
ではなく — CAS conflict は publish 前判定で snapshot を生成
しない（orphan 非生成）が、crash window で生じた orphan
snapshot は削除せず recovery データとして保持し、Satisfied
再 save で HEAD へ再採用する（store.rs の設計・test-spec
R1/R8 どおり）。`Indeterminate` publish の recovery handle も
自動削除しない（ruviform-io-file 契約）。

## 責務境界と Consumer Golden の関係

- **StagedFile Publish（ruviform-io-file @ `fd1d4d5f`）**:
  generic な stage→seal→classified publish。effect
  （NotPublished/Published/Indeterminate）・phase・durability・
  operation id の分類と recovery handle/temp 残滓 semantics
  のみを所有し、Project/Revision/Undo の概念を持たない。
- **Project Commit（synvelis-project）**: `SnapshotEnvelope`
  （schema version・canonical `BTreeMap` byte）・immutable
  snapshot 命名 `r<N>.json`・HEAD pointer・ExpectedRevision/CAS
  判断（`HEAD.lock` mutex・stale-break・fencing）・collision
  policy（`id_watermark`）・Saved/Live revision・Undo/AutoSave・
  Recovery 採用を所有。**Publish 成功 ≠ Project Commit** —
  HEAD repoint が commit 点であり、`Published`/
  `Indeterminate`/DirSync 未達では HEAD を勝手に進めず
  `head()` 再読込・Recovery へ接続する（store.rs rustdoc・
  test-spec Guarantees どおり）。
- **両 Consumer 独立 Golden**: Synvelis 側の oracle は本書の
  fixture 群（`store_publish`/`head_cas`/`controller_store`）。
  AudioLens 側は独自 oracle（alib-02 系）を持ち、片側の Pass
  を他方へ転用しない。関係 Issue（参照記録のみ・本 slice で
  裁定しない）:
  - Ruviform #288（CLOSED）— IO-06 Cross-Consumer・OS/FS・
    Security・性能・互換性 Gate の設計正本。
  - Ruviform #306（OPEN）— IO-IMP-12 共通 I/O 合成
    E2E・Fault/Fuzz/OS/FS・Consumer 受入 Gate。
  - Synvelis #28（OPEN）— Ruviform 継続追従・DAW Consumer
    回帰・更新対応 Issue 運用。
  - Synvelis #294（OPEN）— RCP-05 Creative Branch/Adopt・
    VCS Revision と Render 参照・Late Attach の一貫性。
  - Ruviform #305（撤去候補）・AudioLens #143 — 候補
    revision での結合検証は統合・受入時の関係事項。

## 残ゲート分類（Required / Deferred / NotRun）

| gate | 要求状態 | 結果 | 根拠・委譲先 |
| --- | --- | --- | --- |
| 対象 SHA `c0b000b3` の ci.yml run | Required（統合受入） | **NotRun（pending）** | merge 直後で未 dispatch。次回 daily-ci（21:29 UTC cron）または手動 `workflow_dispatch` で実走予定。nearest-SHA 証拠は上記のとおり関連 code byte 同一を確認済み |
| Linux/macOS 通常 FS の syn-io-02 fixture | Required（OS-L1） | **Pass** | ubuntu-latest / macos-latest leg で 8/16/11 tests 全て `ok`（`std::env::temp_dir` 実 FS） |
| Windows CI leg（windows-latest runner）の同 fixture | Required（OS-L1） | **Pass（CI 証拠）** | windows-latest leg で 8/16/11 tests 全て `ok`。ただし GitHub-hosted runner であり下記実機 Case の代替ではない |
| Windows NTFS 実機 Case（Snapshot CreateNew→HEAD CAS/Crash→Recovery、Failed/Indeterminate で HEAD 誤進行なし） | Required（Windows L2/L3・提供機能の最終受入） | **NotRun** | 実機 FS lane 未整備。Issue #84「まず Windows 実機」条件の残件。CI leg の Pass を実機 Case に読み替えない |
| Network/Cloud/特異 FS | Deferred | NotRun | 未利用 capability（POLICIES §2: 使わない FS 組合せは必須外） |
| 物理電断（true power-loss） | Deferred | NotRun | 非破壊的に自動化不能。OS 固有 Indeterminate/DirSync 失敗注入は ruviform-io-file 側 fault-injection 責務（consumer は分類転写を pin 済み）。real-FS probe の先例は RCD-02 `test-recording-real-fs` lane |
| 実 GUI / 実 Device / 実 Plugin | —（他 Owner） | NotRun | #114/#176 系の別 Issue 責務。本 Issue 範囲外 |
| macOS/Linux L2/L3 | Deferred（policy） | NotRun | 利用要件発生まで Deferred |
| Ruviform #305 撤去候補との結合 | 統合・受入時 | NotRun | candidate-revision での組合せ検証（Issue #84 統合関係事項） |
| Ruviform #294 scratch candidate-SHA 両 Consumer 独立 Oracle | 統合受入 | NotRun | upstream gate |
| Ruviform #306 合成 E2E/Fault/OS/FS/Consumer 受入 | upstream OPEN | NotRun | ruviform 側 Issue |
| AudioLens #143 StagedFile 追従の独立受入 | 別 Consumer | NotRun | 片側 Pass を双方に転用しない |

## 本書が成立させるもの・させないもの

- **成立**: StagedFile/CAS/Recovery の実装が main（PR
  #178・#342）に統合済みであること、対象 SHA の祖先 run
  で syn-io-02 fixture が 3 OS + MSRV の CI leg で実走・
  全 Pass したこと（binary・test 数・leg 単位で記録）、
  責務境界と残ゲートの正直な分類。
- **不成立（主張しない）**: `c0b000b3` 自体の CI 結果
  （未実走）、Windows 実機 NTFS Case、物理電断・実
  durability、Network/Cloud FS、実 GUI/Device/Plugin、
  AudioLens 等他 Consumer の受入、**無条件の atomic
  directory transaction 保証**（本 store は単一 file の
  staged publish + HEAD repoint であり directory 全体の
  atomicity は主張しない — Issue #84・test-spec Non-goals
  どおり）。
