---
id: "TSPEC-1IX4OipTK7jb8iO85siVKU"
type: "test-spec"
status: "approved"
title: "SYN-IO-02 Windows NTFS 実機 Case lane test-spec"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/syn-io-02-windows-real-fs.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/syn-io-02-store-publish-contract.md"
    - "docs/test-specs/syn-io-02-head-cas.md"
    - "docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-windows-real-fs.md"
  derived-by: []
  verifies:
    - "SPEC-497QYMVthAP9s510aXzuVO"
  verified-by: []
  references:
    - "crates/synvelis-project/tests/real_fs_probe.rs"
    - "crates/synvelis-project/src/store.rs"
  defers: []
  deferred-by: []
---

# SYN-IO-02 Windows NTFS 実機 Case lane test-spec

## Purpose

`DirectoryBundleStore` の Snapshot `CreateNew` publish →
HEAD CAS/`Replace` publish を実 Windows FS 上で実 OS
process kill により中断し、reopen/Recovery/Failed の
契約を Case ID `SV84-REAL-*` で pin する test-spec。
headless fixture・fake FS の成功を実 FS durability の
証明にしない。process kill を power-loss・
Indeterminate の代替と主張しない。

## Feature

Project bundle（`<root>/snapshots/r<N>.json` +
`<root>/HEAD` + `HEAD.lock`）の publish 中断・回復・
失敗時 HEAD 非進行の実 FS 検証。

## Rules

- **R1 kill-mid-publish 不変条件**: process kill 後の
  reopen で、HEAD は seed snapshot 名か crash
  snapshot 名のいずれかのみを指し、raw `HEAD` 内容と
  `head()` が一致し（torn HEAD なし）、HEAD が指す
  snapshot は byte 完結でなければならない。旧 HEAD
  保持時に残る crash snapshot orphan は存在すれば
  byte 完結（`CreateNew` atomic rename）でなければ
  ならない。directory 全体の atomicity は主張しない。
- **R2 Crash→Recovery retry**: kill 後の `save_cas`
  retry は、観測 HEAD に応じて `Satisfied` collision
  （byte 同一 orphan の再 publish 採用）または次
  revision への chain で一貫 HEAD へ回復しなければ
  ならない。残存 `HEAD.lock` は手動削除ではなく実
  stale path（`HEAD_LOCK_STALE_AFTER` の
  rename-aside break）で処理され、回復後に
  `HEAD.lock*` 残滓を残してはならない。
- **R3 Failed で HEAD 非進行**: publish 失敗
  （immutable snapshot 名の foreign occupant）と
  stale-expected CAS `Conflict` は typed error
  （`HeadCasError::Store`/`Conflict`）を返し、
  HEAD を進めず、foreign byte を破壊せず、lock を
  解放しなければならない。clean retry は 1 回だけ
  commit しなければならない。
- **R4 process kill ≠ power-loss/Indeterminate**:
  process kill は OS dirty page を失わせない。
  `Indeterminate`（ambiguous OS error）と真の
  power-loss は非破壊的に自動化不能であり `NotRun`
  のまま記録し、process kill の観測をこれらの
  代替 Pass として主張してはならない。
- **R5 Opt-in 分離と bounded 実行**: lane は全 test
  `#[ignore]` + 専用 manual task で実行され、既定
  `cargo test`/CI に混入しない。child process は
  go-file barrier・marker・bounded wait を持ち、
  wedged child は fail、missed kill window（child
  clean exit）は fresh bundle で retry して pass に
  数えない。
- **R6 実測の正直さ**: `CASE=SV84-REAL-*` 行は
  観測値（trigger・status・head・残滓）をそのまま
  出力し、環境依存の残滓を assert で偽装しない。
  契約不変条件のみを assert する。

## Examples

- **E1 cas-lock kill**: `save_cas` が `HEAD.lock`
  取得直後に kill → reopen で `head=r1.json`・
  `HEAD.lock` 残滓・snapshot 未生成。
- **E2 cas-snaptemp kill**: snapshot staging 中に
  kill → `head=r1.json`・`*.rvf-tmp-*` 残滓・
  `HEAD.lock` 残滓。
- **E3 cas-snapdone kill**: snapshot rename 後・
  HEAD repoint 前に kill → `head=r1.json`・orphan
  `r2.json` 16MiB byte 完結・`HEAD.lock` 残滓。
- **E4 save kill**: 無条件 `save` で同 window を kill
  → `HEAD.lock` 非存在（save は lock を取らない）・
  同じ reopen 不変条件。
- **E5 retry 回復**: kill 後 `observed_head=r1.json`
  + lock 残滓 → `save_cas` retry が stale lock を
  ~20s 待機 break し `head=r2.json`・残滓 0。
- **E6 Failed/Conflict**: foreign occupant で
  `HeadCasError::Store`・stale expected で typed
  `Conflict` → HEAD 不変・foreign byte 保持・retry
  で `head=r2.json`。

## Guarantees

- 全 assert は store 契約の不変条件に限定され、
  実 FS の環境差（temp residue・lock 残滓の有無）
  を fail にしない — 観測値は `CASE=` 行に記録。
- kill window の miss は pass に数えない
  （fresh bundle retry・全 miss は fail）。
- power-loss/Indeterminate は `NotRun` のまま
  evidence に残る。

## Non-goals

- 真の power-loss・`Indeterminate` 誘発・Network/
  Cloud FS・macOS/Linux 実機・Import↔Save 結合・
  directory-wide atomicity の主張。

## Risk

- kill timing は環境依存 — trigger spin-poll +
  retry で吸収し、miss を pass にしない設計で
  flaky を fail に変換する。
- stale lock 待機（~20s）は契約上の実測値 — lane
  所要時間に織り込み、証拠に記録する。
