---
id: "TSPEC-034WVJHbBO657gxk3KnLP1"
type: "test-spec"
status: "approved"
title: "SYN-IO-02 HEAD CAS 実 2-process E2E 試験"
created: "2026-11-13"
updated: "2026-11-13"
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
  depends-on:
    - "docs/designs/syn-io-02-head-cas-e2e-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "TSPEC-034Th7fQpLmNvRwXk2jHs"
    - "SPEC-034T7fQpLmNvRwXk2jHs9"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-head-cas-e2e.md"
  derived-by: []
  verifies:
    - "SPEC-034WVJji6Ejtn5EnEV5RkC"
  verified-by: []
  references:
    - "crates/synvelis-project/tests/head_cas.rs"
    - "crates/synvelis-project/src/store.rs"
  defers: []
  deferred-by: []
---

# SYN-IO-02 HEAD CAS 実 2-process E2E 試験

## Purpose

`ProjectStore::save_cas` の expected-head CAS が、同一
process 内 thread ではなく**実 OS process をまたいだ**
協調 writer 間でも lost-update を起こさないこと、および
敗者が typed `HeadCasError::Conflict`（成功誤報なし）を
受け取ることを、`mise run test synvelis-project` の正規
経路で自動検証する。`syn-io-02-head-cas` test-spec の
Rule 2（同一 process thread 協調）の process 境界版で
あり、publish 効果分類・immutable snapshot・collision
規則は既存 `store_publish.rs`/`head_cas.rs` の契約が担保
するため、本試験は 2-process 協調の追加契約に限定する。

## Feature

Cooperating `save_cas` callers in separate OS processes on
one bundle root serialize through `HEAD.lock`; exactly one
commits and the loser observes the moved HEAD as a typed
conflict.

## Rules

- Rule 1 — 実 2-process 同時 `save_cas` は exactly one
  winner。同一 bundle root に対し、同じ expected HEAD を
  assert した 2 つの実 process が `save_cas` を試行した
  とき、勝者は `Ok(snapshot_name)`、敗者は
  `HeadCasError::Conflict { expected, actual }` を受け取る。
  両者が `Ok` を返すことはない。
- Rule 2 — 最終状態は勝者の commit のみを反映する。HEAD
  file は勝者の snapshot 名を指し、snapshot bytes は勝者
  process の payload と一致し、敗者の snapshot は publish
  されず、`HEAD.lock`（および `HEAD.lock.stale-*` aside）
  の残滓がない。
- Rule 3 — 敗者の conflict は typed で成功誤報がない。
  `expected`/`actual` は lock 内で観測した実値（round 1
  は `expected=none` absent assert、以降は前 round の
  勝者 HEAD）を反映し、`Ok`・`Unsupported`・generic
  error へのすり替えは失敗とする。
- Rule 4 — 検証は portable で正規経路実行可能。test
  binary の self re-exec（`current_exe()` + env guard +
  `--exact` filter）で子 process を起こし、OS 固有 API・
  `#[cfg]` 分岐・外部バイナリ・手動手順に依存しない。
  go-file barrier と timeout で hang を failure に変換
  する。

## Examples

- E1: round 1（absent assert）— 空 bundle に 2 process が
  `expected=None` で `save_cas` → `(Ok(r<a>.json),
  Conflict{expected: none, actual: r<a>.json})`。HEAD =
  勝者 snapshot、敗者 revision の snapshot 非存在、
  `HEAD.lock` 非残滓。
- E2: round 2 以降（moved-head assert）— expected = 前
  round の勝者 HEAD に対し同様に exactly one winner、
  敗者 conflict の `actual` は今回の勝者名。
- E3: marker 欠落・子の非 0 終了・`error` marker・timeout
  は全て test failure — typed outcome のみを pass 条件と
  する。

## Guarantees

- 協調する `save_cas` caller が別 OS process に分かれても、
  `HEAD.lock` の process 間 mutex により exactly one
  winner が保証される（実測: Windows、portable code の
  ため CI で各 OS 実走）。
- 敗者は publish 前判定の typed `Conflict` を受け取り、
  HEAD・snapshot・`HEAD.lock` のいずれにも副作用を残さない。
- テストは `mise run test synvelis-project` で他 suite と
  同列に自動実行され、手動 lane を必要としない。

## Non-goals

- `save`（無条件）・`HEAD` 直接書換え等の非協調 writer
  への強制排他 — 親 test-spec/design の契約どおり対象外。
- 電源断・実 durability fault・stale 閾値超過 hold の
  residual race の物理的排除 — 外部 gate / advisory
  mutex の既知限界。
- macOS/Linux の手動実機レーン — CI 実走または NotRun
  記録（POLICIES.md OS-L1 方針）。
- `save_cas` 契約・`HeadCasError`・`HEAD.lock` 機構自体の
  変更 — PR #342 で確定済み。

## Risk

これらを外すと、process をまたいだ同時 save での
lost-update 防止が「同一 process thread 協調」という
狭い条件でしか実測されていないまま残り、`HEAD.lock` の
process 間 mutex・token 照合・stale break が実 process で
機能するかが未検証のままになる — 実運用の複数 writer
構成での信頼性判断を誤らせる。
