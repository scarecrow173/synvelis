---
id: "TSPEC-034Th7fQpLmNvRwXk2jHs"
type: "test-spec"
status: "approved"
title: "SYN-IO-02 HEAD CAS 契約試験"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/330"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/syn-io-02-head-cas-design.md"
    - "TSPEC-034WVJHbBO657gxk3KnLP1"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-head-cas.md"
  derived-by: []
  verifies:
    - "docs/plans/syn-io-02-head-cas-plan.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SYN-IO-02 HEAD CAS 契約試験

## Purpose

`ProjectStore::save_cas` の expected-head compare-and-swap が
`DirectoryBundleStore`（`HEAD.lock` mutex + stale-break +
fencing）と `InMemoryStore`（parity）で同一契約を満たし、
`ProjectController::save` の expected 供給・`Unsupported`
fallback・journal restore の CAS 化が lost-update を防ぐことを
実ファイル（`std::env::temp_dir` の実 bundle）と in-memory の
両面で pin する。publish 効果分類・immutable snapshot・
collision 規則は既存 `store_publish.rs` の契約が担保し、本
試験は CAS 固有の追加契約に限定する。

## Feature

`ProjectStore::save_cas` commits snapshot+HEAD only while HEAD
still names the expected snapshot; cooperating writers on one
root serialize through `HEAD.lock` and the loser gets a typed
conflict.

## Rules

- Rule 1 — expected 一致時のみ commit。`expected_head` は
  `head()` の報告値そのもの（`None` = HEAD 不在 assert）。
  不一致は `HeadCasError::Conflict { expected, actual }` で、
  HEAD・snapshot とも不変（conflict は publish 前に判定
  されるため orphan も残らない）。
- Rule 2 — 同一 root の協調する 2 `save_cas` は `HEAD.lock`
  で直列化され、同じ expected HEAD を観測した 2 者が両方
  commit することはない — exactly one winner、loser は
  `Conflict`、最終 HEAD は勝者の snapshot 名。
- Rule 3 — `ProjectController::save` は `saved.head` を
  expected に供給する。HEAD が他 writer に進められた後の
  stale-baseline save は conflict で失敗し `saved` は進まず、
  divergent snapshot は publish されない。`load`→retry で
  回復する。fresh controller（`saved=None`）の save は
  HEAD 不在 assert として非空 store に対し conflict する。
- Rule 4 — `InMemoryStore::save_cas` は同一契約
  （conflict/`None` assert/parity）を filesystem 無しで
  再現する。
- Rule 5 — `save_cas` を override しない store は default
  `HeadCasError::Unsupported` を返し、`ProjectController::save`
  はその場合のみ従来の無条件 `save` へ fallback して成功する
  （後方互換）。
- Rule 6 — `HEAD.lock` は `HEAD_LOCK_STALE_AFTER` 超過で
  stale と判定され rename-aside で break され CAS が継続
  する。fresh な foreign lock は bypass されず解放まで待機
  する。
- Rule 7 — CAS critical section 内のいかなる失敗
  （conflict・publish 失敗・lock 喪失）でも `HEAD.lock` は
  解放され、次の `save_cas` が wedged しない。
- Rule 8 — lock 喪失を検出した経路（pre/post fencing）は
  `Ok` を返さない — publish 済みでも indeterminate な
  store error として報告し、caller は `head()` で
  reconcile する（成功誤報なし）。

## Examples

- E1: `save_cas` 基本 — 空 store に `expected=None` で commit
  → HEAD=`r1.json`。`expected=r1.json` で次 revision commit →
  HEAD=`r2.json`。stale expected=`r1.json` → `Conflict
  {expected: r1.json, actual: r2.json}`、HEAD・`r3.json`
  不変。`expected=None` を非空 HEAD に → `Conflict`（Rule 1）。
- E2: 同時 save 再現 — 同一 root に 2 `DirectoryBundleStore`、
  とも `expected=r1.json`・別 bytes で scoped thread 同時
  `save_cas` → `(Ok(r2.json), Conflict)` のみ（順不同）、
  `r2.json` の byte は勝者のもの、HEAD=`r2.json`、
  `HEAD.lock` 残滓なし（Rule 2）。
- E3: controller moved-HEAD — c_a が r1→r3 へ進めた後、r1
  baseline の c_b が save → `Error::Store`（`head CAS
  conflict`）、`saved.head`=`r1.json` 不変、HEAD=`r3.json`
  不変、divergent `r4.json` 非存在。`c_b.load` で r3 採用後の
  save は `r4.json` で成功（Rule 3）。
- E4: in-memory parity — `InMemoryStore` で E1 と同じ
  conflict/None assert 系列が再現する（Rule 4）。
- E5: unsupported fallback — `save_cas` 未 override の bare
  store は `Unsupported`、`controller.save` は成功して
  `saved.head`=`r1.json`（Rule 5）。
- E6: stale lock break — `created_ns` を stale 閾値より過去に
  偽装した `HEAD.lock` があっても `save_cas` は break して
  commit し、lock・aside 残滓なし（Rule 6）。
- E7: fresh lock 待機 — fresh な foreign `HEAD.lock` 存在下で
  `save_cas` は完了せず、lock 削除後に commit する（Rule 6）。
- E8: 失敗時解放 — snapshot publish が失敗（外部占有
  `r2.json`）しても `HEAD.lock` は残らず、障害除去後の retry
  が commit する（Rule 7）。

## Guarantees

- expected-head CAS は協調する `save_cas` caller 間で
  linearizable — 同じ expected を持つ 2 caller が両方
  commit することはない。
- conflict は typed（`HeadCasError::Conflict`）で、
  HEAD/snapshot のいずれも変更しない — publish 前判定の
  ため orphan も残らない。
- あらゆる失敗経路で `HEAD.lock` が解放され、次の CAS が
  wedged しない。
- lock 喪失検出時は `Ok` を返さない（post-fencing の
  indeterminate error）— 成功誤報なし。
- `save`（無条件）・HEAD/snapshot format・`ProjectController::
  save` signature は不変 — CAS は opt-in の追加契約。

## Non-goals

- 実 2-process/複数 OS での協調検証・電源断・実機 durability
  gate — 本 slice は Windows 単一 process 内 thread 協調
  までを pin した。実 2-process は後続 slice
  `syn-io-02-head-cas-e2e`（`TSPEC-034WVJHbBO657gxk3KnLP1`）
  で Windows 実測済み。macOS/Linux は CI 実走または NotRun、
  電源断・実機 durability は外部 gate（NotRun）に委譲する。
- 非協調 writer（`save` 直接・外部 HEAD 書換え）への強制
  排他 — 次回 CAS の conflict で観測される契約まで。
- journal replay restore の gateway 側統合テスト追加 —
  `commit.rs` の `save_cas` 呼出は code review + workspace
  test で担保し、専用 fault 注入は本 slice の範囲外。
- `HEAD.lock` stale 閾値超過 hold の residual race の物理的
  排除 — advisory lock の既知限界として design に記録。

## Risk

これらを外すと、複数 writer/process の同時 save で lost-update
が起き、stale-baseline の controller が新しい HEAD を検出
なしに巻き戻し、CAS 非対応 store への silent degrade が
単一 writer 前提を超えた使用を許し、失敗経路で `HEAD.lock`
が残留して後続 save を永久に塞ぐ — いずれも Recovery/
saved-state 判断を誤らせる。
