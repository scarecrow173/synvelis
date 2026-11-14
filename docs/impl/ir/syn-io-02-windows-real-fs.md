---
id: "IMPL-24139Uh7nkREN2zgKcEqbY"
type: "impl"
status: "completed"
title: "SYN-IO-02 Windows NTFS 実機 Case lane（SV84-REAL-* opt-in probe）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/real_fs_probe.rs"
      - type: "file"
        path: "docs/specs/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "docs/designs/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "docs/plans/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "docs/tasks/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "docs/test-specs/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "docs/impl/ir/syn-io-02-windows-real-fs.md"
      - type: "file"
        path: "evidence/reports/syn-io-02-windows-real-fs.md"
    modified:
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "evidence/reports/syn-io-02-os-l1-evidence.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/syn-io-02-windows-real-fs.md"
    - "docs/plans/syn-io-02-windows-real-fs.md"
    - "docs/designs/syn-io-02-windows-real-fs.md"
    - "docs/specs/syn-io-02-windows-real-fs.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/syn-io-02-windows-real-fs.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/syn-io-02-store-stagedfile-migration.md"
    - "docs/impl/ir/syn-io-02-head-cas.md"
    - "docs/impl/ir/syn-io-02-os-l1-evidence.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/syn-io-02-windows-real-fs.md"
  derived-by: []
  verifies:
    - "docs/test-specs/syn-io-02-windows-real-fs.md"
  verified-by:
    - "docs/test-specs/syn-io-02-windows-real-fs.md"
  references:
    - "crates/synvelis-project/tests/real_fs_probe.rs"
    - "crates/synvelis-project/src/store.rs"
    - "crates/synvelis-project/tests/head_cas.rs"
    - "evidence/reports/syn-io-02-windows-real-fs.md"
    - "evidence/reports/syn-io-02-os-l1-evidence.md"
  defers:
    - "真の power-loss（電源断相当）実測 — 非破壊的に自動化不能、process kill は dirty page を保持するため代替不可（NotRun）"
    - "PublishEffect::Indeterminate の実 FS 誘発 — ambiguous OS error が必要で非破壊的に作れない（NotRun、ruviform-io-file fault-injection 拡張時に再検討）"
    - "Import↔Save 結合 Case — sv#86/#91 の責務（範囲外）"
    - "macOS/Linux 実機 real-FS — 各 OS 実機環境での同一 lane 実走（Windows/NTFS 観測値を一般化しない）"
    - "Issue #84 の他残ゲート行（対象 SHA CI run・upstream/consumer gate）— tracker として open 継続"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SYN-IO-02 Windows NTFS 実機 Case lane（SV84-REAL-* opt-in probe）

## Summary

Issue #84「まず Windows 実機」行の残件 —
`evidence/reports/syn-io-02-os-l1-evidence.md` 残ゲート
表の Windows NTFS 実機 Case 行（Required・NotRun）を
opt-in real-FS lane で完結させた。
`crates/synvelis-project/tests/real_fs_probe.rs`（全 test
`#[ignore]`、CI 対象外）を新設し、test binary self
re-exec + env guard + go-file barrier で子 process を
実 OS process として spawn、`HEAD.lock`/snapshot
temp/snapshot rename の trigger を spin-poll して
`Child::kill`（TerminateProcess 相当）で publish 途中に
殺す。reopen して store 契約の不変条件を assert し、
`CASE=SV84-REAL-*` 行を stdout に出す。

`mise.toml` に `[tasks.test-project-real-fs]` を 1 件
追加（`test-recording-real-fs` と同型 + `--nocapture`
で CASE 行を証拠転記可能に）。実 Windows 11/NTFS
（`SYNVELIS_REALFS_ROOT` を NTFS temp 配下に固定）で
`mise run test-project-real-fs` を実走し、
SV84-REAL-01/02/03 の 3 test 全 Pass を
`evidence/reports/syn-io-02-windows-real-fs.md` に
実測記録、残ゲート表の該当行を更新した。
`src/` 差分ゼロ・`Cargo.lock` 変更なし。

## Context

- Task: `docs/tasks/syn-io-02-windows-real-fs.md`
- Spec: `docs/specs/syn-io-02-windows-real-fs.md`
- Design: `docs/designs/syn-io-02-windows-real-fs.md`
- Plan: `docs/plans/syn-io-02-windows-real-fs.md`
- Test-spec: `docs/test-specs/syn-io-02-windows-real-fs.md`
- Issue: <https://github.com/scarecrow173/synvelis/issues/84>
  （SYN-IO-02・tracker。実装本体は PR #178・#342 で
  main 統合済み、本 slice は残ゲート表の Windows NTFS
  実機 Case 行のみ）
- Branch: `feat/84-syn-io-02-windows-real-fs`（base =
  `origin/main` `c1d485a`、code commit `dbf0820`）

## Implementation

### Changes

- `crates/synvelis-project/tests/real_fs_probe.rs`（新規、
  約 800 行）:
  - child re-exec protocol: `SYNVELIS_REALFS_*` env
    で op/dir/expected/revision/seed/len/go を渡し、
    payload は `probe_payload(seed, len)`
    （xorshift64* 4KiB table tile）で両 process が
    byte 同一に再計算 — byte 同一 retry が
    `Satisfied` collision（orphan 採用）を踏める。
    spawn args は `<test> --exact --nocapture
    --ignored`（`--ignored` 必須 — probe は `#[ignore]`
    のため）。
  - `kill_round`: attempt 毎に fresh seeded bundle
    （seed `r1.json`）を作り、go-file 書込み →
    trigger spin-poll（`is_multiple_of(256)` 毎に
    deadline 判定）→ `Child::kill` → `wait_bounded`
    （90s、store の busy 30s より大）で reap。
    clean exit（missed window）は fresh bundle で
    retry、全 miss は fail。
  - `SV84-REAL-01`（5 round = cas|save ×
    head-lock|snapshot-temp|snapshot-done）:
    `assert_crash_invariant` で HEAD ∈ {seed,crash}
    ・raw HEAD == `head()`・指す snapshot byte 完結・
    orphan byte 完結・`stale_asides==0`・`save` で
    `HEAD.lock` 非存在を assert。残滓は観測値。
  - `SV84-REAL-02`: `SnapshotDone` kill → 観測 HEAD
    分岐で `save_cas` retry（`r1` なら `Satisfied`
    collision で orphan 採用 → `r2`、`r2` なら chain
    → `r3`）。stale `HEAD.lock` は実
    `HEAD_LOCK_STALE_AFTER`(20s) path で break、
    回復後残滓 0。
  - `SV84-REAL-03`: foreign occupant で
    `HeadCasError::Store`（`SnapshotCollision::
    Conflict` → publish error, os error 183）・
    stale expected で typed `Conflict`・HEAD 不変・
    foreign byte 保持・retry で `r2` へ 1 回 commit。
- `mise.toml`: `[tasks.test-project-real-fs]` 追加
  のみ（既存 task・記述は不変。共有ファイルのため
  PR 本文に理由を明記）。
- `evidence/reports/syn-io-02-windows-real-fs.md`:
  新規。Case ID × Requirement×Result 直交記録・
  対象 SHA `dbf0820`・Expected/Observed・環境
  （Windows 11 Pro 10.0.26300・NTFS・rustc 1.99.1）・
  Indeterminate/power-loss `NotRun` を明記。
- `evidence/reports/syn-io-02-os-l1-evidence.md`:
  残ゲート表の Windows NTFS 実機 Case 行のみ更新
  （他行・他節は不変 — 「本書が成立させるもの・
  させないもの」節の記述は本書の証拠範囲を超える
  主張を含まないため未変更）。
- docs chain 6 file（本 IR を含む）+ `docs/*/README.md`
  managed index 再生成。

### Decisions During Implementation

- **`--ignored` を child spawn に明示**: probe test は
  全て `#[ignore]` で既定/CI から外れるため、child が
  無指定だと libtest が 0 tests run で clean exit し、
  kill window を永久 miss する構造的欠陥をレビューで
  検出・修正（head_cas.rs 先例は非 ignore test のため
  flag 不要だった）。
- **kill_round は attempt 毎に fresh bundle**: 同一
  bundle で retry すると、missed window で commit
  済みの HEAD が expected をずらし `Conflict` →
  clean exit の連鎖で panic する。同一 dir を使い
  回す設計を捨て、attempt 毎に seed からやり直す
  ことで missed window を pass に数えない構造にした。
- **spin-poll trigger**: Windows timer granularity
  （~15ms）より細かい publish window（ms 級）を
  sleep で取りこぼさないよう、`sleep` 無しの
  `exists()` spin + 256 spin 毎の deadline 判定。
- **残滓は観測値・不変条件のみ assert**: `HEAD.lock`
  /`*.rvf-tmp-*`/`stale-*` の有無は kill 着地点と
  回復段階で環境依存に変わるため assert せず
  `CASE=` 行に記録。assert は HEAD の値域・byte
  完結性・lock 非残滓（回復後）など store 契約の
  不変条件に限定。
- **`Satisfied` collision を retry 回復の観測点に**:
  payload を seed/len で deterministic にし、crash
  writer と byte 同一の retry が orphan `r2.json` を
  `SnapshotCollision::Satisfied` → Replace 再 publish
  で採用する実経路を踏ませた（byte が違えば
  `Conflict` で回復しない）。
- **volume pin**: workspace `G:` は ReFS/DevDrive
  のため `SYNVELIS_REALFS_ROOT` で bundle を NTFS
  temp（`C:`）へ固定し、FS 種別を証拠に記録。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

- `SYNVELIS_REALFS_ROOT=C:\\Users\\akiyoshi\\
  AppData\\Local\\Temp\\synvelis-realfs-ntfs mise run
  test-project-real-fs` — exit 0、`test result: ok.
  3 passed; 0 failed`（20.93s）。対象 SHA
  `dbf0820566694434d6f926173be4ca76d9c31286`、
  OS Windows 11 Pro 10.0.26300、FS NTFS、
  rustc 1.99.1 / cargo 1.99.1（mise 管理）、
  ruviform rev `fd1d4d5f`。canonical opt-in lane。
  実測 `CASE=` 行は evidence report に転記。
- `mise run fmt-check` — exit 0。
- `mise run check synvelis-project` — exit 0。
- `mise run lint synvelis-project` — exit 0
  （clippy `-D warnings`）。
- `mise run test synvelis-project` — exit 0
  （`#[ignore]` probe は default run で 0 selected、
  非搭載を確認）。
- `cargo check --locked --all-targets -p
  synvelis-project` — exit 0（補助証拠）。
- `mise exec -- prek run --all-files` — commit hook
  として全 file で Pass（fmt/whitespace/toml 等）。

NotRun: 真の power-loss（電源断相当・非破壊的に
自動化不能、process kill は dirty page を保持する
ため代替不可）・`PublishEffect::Indeterminate` の
実 FS 誘発（ambiguous OS error が必要）・
macOS/Linux 実機・Network/Cloud FS・Import↔Save
結合（sv#86/#91）・Issue #84 の他残ゲート行。

## Risks

- kill timing は環境依存 — trigger spin-poll +
  fresh-bundle retry で miss を fail に変換する
  設計。3 attempt 全 miss は fail（pass 偽装なし）。
- stale `HEAD.lock` 待機 ~20s は `HEAD_LOCK_STALE_
  AFTER` の契約値 — lane 所要時間に織り込み済み
  （実測 20.93s）。
- probe は `#[ignore]` のため CI では実行されない
  — 実 FS 実測は opt-in lane の手動実行に依存し、
  統合検証は merge 後対象 SHA の CI 証拠と区別する。

## Follow-ups

- 真の power-loss 実測（実機電源断環境・人間 gate）。
- `Indeterminate` 誘発の ruviform-io-file 側
  fault-injection 拡張時に consumer 側 Case を再検討。
- macOS/Linux 実機での同一 lane 実走（portable code、
  CI leg が被覆 — 実機 gate が必要になった時点で）。
- Issue #84 の残ゲート行（対象 SHA CI run・upstream
  #305/#306・consumer #294/AudioLens #143 等）は
  tracker として継続。
