---
id: "DESIGN-0948qgqpuG8j47FSJjhaHR"
type: "design"
status: "approved"
title: "SYN-IO-02 Windows NTFS 実機 Case lane — design"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "SPEC-497QYMVthAP9s510aXzuVO"
  implemented-by: []
  depends-on:
    - "docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md"
    - "docs/designs/syn-io-02-head-cas-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-unsynced-loss-real-fs-probe-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-windows-real-fs.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-project/src/store.rs"
    - "crates/synvelis-project/tests/head_cas.rs"
    - "crates/synvelis-project/tests/store_publish.rs"
    - "crates/synvelis-library/tests/import.rs"
    - "crates/recording-poc/tests/real_fs_probe.rs"
  defers: []
  deferred-by: []
---

# SYN-IO-02 Windows NTFS 実機 Case lane — design

## Context

`DirectoryBundleStore`（`crates/synvelis-project/src/
store.rs`）は main 統合済み: snapshot は
`StagedFile` + `PublishMode::CreateNew` +
`Durability::FileAndDirectorySync`、HEAD は `Replace` +
同 durability。`save_cas` は `HEAD.lock`
（`CreateNew` + `BestEffort`、stale 20s・busy 30s）内で
expected-head を照合し、pre/post publish fencing を持つ。
headless fixture（`store_publish.rs`/`head_cas.rs`）は
CI で 3 OS + MSRV Pass 済みだが、実 Windows FS 上の
kill→reopen→retry は未実測 — 本 slice がそれを埋める。

関連既存構造:

- publish window の構成: `save_cas` = acquire
  `HEAD.lock` → `head()` 照合 → `publish_snapshot`
  （16MB 級 payload で広い window）→ fencing →
  `publish_head`（`Replace`）→ release。`save` は lock
  を取らず snapshot→HEAD を直列 publish。
- temp 命名: `<dest>.rvf-tmp-<token>`（ruviform-io-file
  `os.rs`）。`HEAD.lock` 残滓は `created_ns=` 内容で
  fresh/stale を分類し、stale は
  `HEAD.lock.stale-*` へ rename-aside break。
- self re-exec 先例: `head_cas.rs` e2e（env guard +
  go-file barrier + marker + bounded wait）と
  `import.rs` LIB-IMPORT-08（kill_on crash child）。
  synvelis-project には bin が無いため test binary
  self re-exec 型を採る。

## Scope

### In Scope

- `crates/synvelis-project/tests/real_fs_probe.rs` 新規
  （`#[ignore]` ×3 + child protocol）。
- `mise.toml` `[tasks.test-project-real-fs]` 追加。
- evidence 2 file（新規 report + 残ゲート表の行更新）。
- docs chain + index 再生成。

### Out of Scope

- `src/` 変更・fault injection・power-loss/Indeterminate
  誘発・他 OS/他 FS・Import↔Save 結合・workflow 変更
  （spec Out of Scope と同一）。

## Design

### 1. Child re-exec protocol（head_cas.rs 型）

- 親 test が `std::env::current_exe()` で自 binary を
  `<test_name> --exact --nocapture --ignored` で spawn。
  `--ignored` は必須 — probe test は全て `#[ignore]` で
  既定/CI から外れており、子が同じ ignore 状態で動く
  必要がある（無指定だと 0 tests run で clean exit し、
  kill window を永久に miss する）。
- env で `SYNVELIS_REALFS_{CHILD,DIR,OP,EXPECTED,
  REVISION,SEED,LEN,GO}` を渡し、payload は
  `probe_payload(seed, len)`（xorshift64* 由来 4KiB
  table の tile）で両 process が byte 同一に再計算
  する — byte 同一 retry が `Satisfied` collision 経路
  （orphan 採用）を踏めるようにするため。
- go-file barrier: 子は `SYNVELIS_REALFS_GO` path の
  出現を `CHILD_GO_TIMEOUT`(60s) まで待ってから store
  を呼ぶ。親は spawn 直後に go を書き、trigger 観測と
  kill のタイミングを完全に制御する。
- child は `REALFS_CHILD_ENTERED`/`REALFS_CHILD_RESULT`
  marker を stdout に出し flush。親は `CASE=` 行に
  status/marker/stderr tail を転記する（libtest summary
  行に RESULT が隠れないよう marker 行を抽出）。

### 2. Trigger window と kill_round

- `Trigger::{HeadLock, SnapshotTemp, SnapshotDone}`:
  それぞれ `HEAD.lock` 存在 / `snapshots/
  r<N>.json.rvf-tmp-*` 存在 / `snapshots/r<N>.json`
  存在を spin-poll（`is_multiple_of(256)` 毎に deadline
  判定 — Windows timer granularity ~15ms より細かい
  publish window を sleep 無しで捉える）。
- `kill_round` は **attempt 毎に fresh seeded bundle**
  （seed `r1.json`）を作る。missed window（trigger 後・
  kill 前に child が commit して clean exit）で同一
  bundle を retry すると expected-head がずれて
  `Conflict`→clean exit の連鎖で panic するため、
  attempt 間の状態混入を構造的に排除する。clean exit
  は `KILL_ATTEMPTS`(3) まで retry し、全 miss なら
  fail — missed window を pass に数えない。
- `Child::kill` は既終了との race を `let _ =` で許容
  し、`wait_bounded`（`CHILD_WAIT_TIMEOUT`=90s、
  store の `HEAD_LOCK_BUSY_TIMEOUT`=30s より十分大）
  で wedged child を fail にする（無制限 hang を
  作らない）。

### 3. Case 構成

- **SV84-REAL-01** `sv84_real_01_kill_mid_publish_
  reopen_invariant`: 5 round = (cas|save) ×
  (head-lock|snapshot-temp|snapshot-done) の publish
  window をカバー（`save` は lock を取らないため
  head-lock round は cas のみ）。各 round で
  `assert_crash_invariant`: HEAD ∈ {seed, crash}
  snapshot 名・raw HEAD と `head()` 一致・HEAD 指す
  snapshot は byte 完結・旧 HEAD 時の orphan は byte
  完結 or 不存在・`stale_asides==0`・`save` で
  `HEAD.lock` 非存在。`temp_residue`/`lock_leftover`
  は assert せず観測値として `CASE=` 行に記録
  （環境依存の残滓を偽装しない）。
- **SV84-REAL-02** `sv84_real_02_save_cas_retry_
  recovers_after_process_kill`: `SnapshotDone`
  trigger で kill → reopen で観測 HEAD に分岐。
  `r1.json`（旧 HEAD + orphan）なら同一 payload の
  `save_cas` retry が `Satisfied` collision で orphan
  を再 publish 採用して `r2.json` へ回復。`r2.json`
  （commit 済み）なら byte 検証後に `r3.json` へ
  chain。stale `HEAD.lock` 残滓は retry が実
  `HEAD_LOCK_STALE_AFTER`(20s) path で break する
  ことを待機時間として観測（手動削除しない）。
  回復後 `HEAD.lock*` 残滓 0 を assert。
- **SV84-REAL-03** `sv84_real_03_failed_publish_and_
  conflict_never_advance_head`: `snapshots/r2.json`
  に foreign occupant を置き `save_cas` →
  `HeadCasError::Store`（`SnapshotCollision::
  Conflict` → publish error、os error 183）で
  HEAD 不変・foreign byte 保持・lock 解放を assert。
  stale expected `r9.json` の CAS は publish 前に
  typed `Conflict`（`r3.json` 非生成）。foreign
  除去後の同一 CAS retry が `r2.json` へ 1 回だけ
  commit。
- **Indeterminate/power-loss**: 誘発手段が無いため
  test を置かず、file header・evidence・本 design で
  `NotRun` を明記（process kill ≠ power-loss）。

### 4. Volume pin

`workdir` は `SYNVELIS_REALFS_ROOT` があればその配下、
無ければ `std::env::temp_dir()`。NTFS 実測を主張する
lane なので実行者が volume を固定し、FS 種別を証拠に
記録する（本機では `C:` = NTFS、workspace `G:` =
ReFS/DevDrive を対象外とした）。

### 5. 変更しないもの

- `crates/*/src/**`・`Cargo.toml`/`Cargo.lock`・
  既存 test・workflow・`POLICIES.md`/`AGENTS.md`・
  他 slug docs・他 evidence file。
- `mise.toml` は `[tasks.test-project-real-fs]` 追加
  のみ（既存 task・記述は不変）。

## File Map

| File | Change |
| --- | --- |
| `crates/synvelis-project/tests/real_fs_probe.rs` | 新規 — `#[ignore]` ×3 + child protocol |
| `mise.toml` | `[tasks.test-project-real-fs]` 追加 |
| `evidence/reports/syn-io-02-windows-real-fs.md` | 新規 — Case ID×Result・実測記録 |
| `evidence/reports/syn-io-02-os-l1-evidence.md` | 残ゲート表の Windows NTFS 行のみ更新 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/syn-io-02-windows-real-fs.md` | doc chain 6 file |
| `docs/*/README.md` | 管理 index 再生成 |

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| kill window を継続的に miss して flaky | low | med | fresh bundle per attempt + `KILL_ATTEMPTS`=3 + 16MB payload で window を広く。全 miss は fail（pass 偽装しない） |
| stale `HEAD.lock` 待機で lane が遅い | med | low | `HEAD_LOCK_STALE_AFTER`=20s が契約上の実測値 — 証拠に待機を記録。30s busy timeout 内に break される設計を store 側が保証 |
| trigger predicate が実 temp 命名とずれる | low | med | `<name>.rvf-tmp-<token>` を ruviform-io-file `os.rs` で確認済み。trigger 未観測は 30s で fail |
| `--ignored` 無し child が 0 tests run で clean exit | med | high | spawn args に `--ignored` を明示（レビューで検出・修正済み） |
| 実 FS の環境差で assert が誤 fail | low | med | assert は store 契約の不変条件のみ。残滓・観測値は print に留める |

## Verification Matrix

| Acceptance | Verification |
| --- | --- |
| SV84-REAL-01/02/03 の pin | `mise run test-project-real-fs`（opt-in・実 NTFS） |
| 既定/CI 非搭載 | 全 test `#[ignore]` + `cargo test` で 0 selected を確認 |
| Failed/Conflict の HEAD 非進行 | SV84-REAL-03 の typed error + head assert |
| 回帰不変 | `mise run fmt-check`/`check`/`lint`/`test` synvelis-project |
| 証拠の正直さ | power-loss/Indeterminate を `NotRun` のまま記録 |

## Self-Review

- kill_round が attempt 毎に fresh bundle を持つ設計に
  した理由: 同一 bundle retry では commit 済み HEAD が
  expected をずらし、missed window が永久 retry に
  なる構造的欠陥を事前に排除（実装中に検出・修正）。
- `wait_for_trigger` は sleep ではなく spin + 256 spin
  毎の deadline 判定 — publish window（ms 級）を
  Windows timer granularity で取りこぼさないため。
- `print_child_out` は libtest summary に隠れないよう
  `REALFS_CHILD_*` marker 行を抽出 — 証拠転記の
  再現性のため。
- `assert_crash_invariant` は「HEAD ∈ {seed,crash}」
  「raw HEAD == head()」「指す snapshot は byte 完結」
  のみを assert し、残滓の有無は観測値 — 環境差を
  fail にしない。
