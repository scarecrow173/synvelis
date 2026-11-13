---
id: "DESIGN-034WVJwLjBMYR3fBrNJD2M"
type: "design"
status: "approved"
title: "SYN-IO-02 HEAD CAS 実 2-process E2E 検証 — design"
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
  implements:
    - "SPEC-034WVJji6Ejtn5EnEV5RkC"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "DESIGN-034Tk3mXw7LpQ2vRnB8y"
    - "SPEC-034T7fQpLmNvRwXk2jHs9"
  refines:
    - "docs/specs/syn-io-02-head-cas-e2e.md"
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-head-cas-e2e.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-project/src/store.rs"
    - "crates/synvelis-project/tests/head_cas.rs"
    - "crates/synvelis-library/tests/import.rs"
  defers: []
  deferred-by: []
---

# SYN-IO-02 HEAD CAS 実 2-process E2E 検証 — design

## Context

PR #342 で `save_cas` / `HeadCasError` / `HEAD.lock` advisory
mutex CAS は統合済みで、同一 process 内 thread 協調の契約
テスト（`tests/head_cas.rs`）も pass している。残件は
Issue #330 の受入条件のうち「実 2-process 協調での
lost-update なし」と「実 process 経路での typed error
観測」「保証境界の確定記録」。本 design はこの検証 slice
の構成を定める — store 実装・port 契約・`HEAD.lock` 機構は
変更しない。

制約:

- `save_cas` API・`HeadCasError`・`HEAD.lock` 機構は確定済み
  （契約再設計は対象外）。
- テストは `mise run test synvelis-project` で自動実行可能
  でなければならない — 手動 lane・外部バイナリ・opt-in
  feature 依存は不可。
- 新規 dependency 追加なし（dev-dependency も増やさない）。

## Scope

### In Scope

- `crates/synvelis-project/tests/head_cas.rs` への実 2-process
  協調テスト（self re-exec 方式）。
- 本 chain の docs と、親 chain の残件記述の実測整合。

### Out of Scope

- `crates/*/src/` の変更、CAS 契約の再設計。
- macOS/Linux 手動 lane・電源断・非協調 writer 排他。

## Components and Boundaries

- **親 test（orchestrator）**:
  `two_process_save_cas_single_winner_typed_conflict`。
  bundle root を `workdir`/`BundleGuard` で用意し、各 round
  で現在 HEAD を `head()` から読み、2 つの子 process を
  `current_exe()` の self re-exec で spawn、go-file で同時
  開始を合わせ、結果を検証する。
- **子 process（worker）**: 同じ test 関数が env guard
  （`SYNVELIS_HEAD_CAS_E2E_*`）で worker 分岐し、go-file
  出現を待って `DirectoryBundleStore::create(dir)` →
  `save_cas(project, expected, revision, bytes)` を 1 回
  実行、結果を marker 行で stdout に出す。
- **marker protocol**: `HEAD_CAS_CHILD_RESULT ok
  name=<snapshot>` / `HEAD_CAS_CHILD_RESULT conflict
  expected=<opt> actual=<opt>` / `HEAD_CAS_CHILD_RESULT
  error <debug>`。`<opt>` は `none` または snapshot 名。
  typed outcome（ok/conflict）は exit 0、worker 到達前の
  panic/IO 失敗は非 0 + marker 無しで親が失敗と判定する。

## Data and Control Flow

1. 親: bundle root 作成 → round 毎に `head()` で expected
   を取得（round 1 は `None` = absent assert）。
2. 親: 2 子を `current_exe() <test fn> --exact --nocapture`
   + env（dir・expected・revision・tag・go-file path）で
   spawn。revision は子ごと・round ごとに一意
   （snapshot 名衝突を避ける）。
3. 親: go-file を作成 → 両子がほぼ同時に `save_cas` 試行。
4. 子: `HEAD.lock` `CreateNew` で直列化 — 勝者が lock 内で
   expected 一致を確認し snapshot+HEAD を commit、敗者は
   解放後に移動済み HEAD を観測して `Conflict`。
5. 親: 両子の終了を timeout 付きで待ち、marker を parse。
   exactly one `ok`（name = 勝者 revision の snapshot）+
   one `conflict`（expected = seed、actual = 勝者名）を
   assert。HEAD file・snapshot bytes（勝者 tag）・敗者
   snapshot 非存在・`HEAD.lock` 非残滓を検証。
6. round を繰り返す（各 round の expected は前 round の
   勝者 HEAD）。

### 検討した代替案

1. **専用 fixture バイナリ（`[[bin]]`/examples）** —
   `recording-poc` の `vsc_process.rs` が先例だが、
   synvelis-project は lib crate で bin を持たず、fixture
   専用 bin 追加は crate 構成を変える。棄却。
2. **test binary self re-exec（採用）** —
   `synvelis-library/tests/import.rs` の
   `subprocess_crash_during_publish_or_commit_resumes_across_processes`
   と同一 pattern。追加物は test code のみ。
3. **shell script / 外部 orchestrator** — `mise run test`
   の正規経路から外れ CI 自動実行を保証できない。棄却。

## 保証境界の確定記録（本 slice の実測範囲）

親 design（`DESIGN-034Tk3mXw7LpQ2vRnB8y`）の保証境界節を
正本とし、本 slice の実 2-process 実測で確認した範囲を
対応付ける:

- **実測で確認（Windows/NTFS・CI では各 OS）**:
  `HEAD.lock` の `CreateNew` acquire・token 照合 release が
  process をまたいで機能し、協調する `save_cas` caller 間で
  exactly one winner + loser typed `Conflict` が成立する。
  conflict は publish 前判定のため敗者 snapshot は残らない。
- **実測していないが契約上保証（既存 test が担保）**:
  各 publish の rename atomicity と `FileAndDirectorySync`
  は ruviform-io-file の既存契約 — `store_publish.rs` /
  `head_cas.rs` の既存 test が publish 単位の保証を pin
  しており、本 slice は CAS 協調境界に限定する。
- **設計上の限界・未検証（親 design 記載どおり）**:
  FS 依存の限界（network FS・case 規則差・dirsync 非対応
  FS での等価性未実測）、advisory mutex の範囲（非協調
  writer・`save` 直接呼出は排他の外、stale 閾値超過 hold の
  residual race は post-fencing が誤成功を塞ぐが物理的余地
  は残る）、電源断・実 durability fault。

## Non-Functional Constraints

- 新規 dependency なし（`std::process`/`std::env`/
  `std::thread`/`std::fs`/`std::time` のみ）。
- portable — Windows/macOS/Linux で同一 code が動作
  （OS 固有 API・`#[cfg]` 分岐なし）。
- 実行時間は数秒以内 — 子 spawn × round 数が支配的。
- hang 防止: 子の go-file 待機と親の子待機に上限を設け、
  timeout は test failure として報告する（無期限 wait しない）。

## Risks and Trade-offs

- self re-exec は libtest の `--exact` filter 仕様に依存 —
  安定した CLI であり `import.rs` 先例あり。
- go-file barrier は「同時 critical section 突入」の保証では
  なく best-effort の同時試行 — ただし `HEAD.lock` 直列化に
  より結果（exactly one winner）は timing 非依存で成立する。
- stdout marker parse は `--nocapture` 出力に依存 —
  `HEAD_CAS_CHILD_RESULT ` prefix で他出力と区別する。

## Deferred Design Concerns

- kernel file lock・真の強制排他 — 親 design Deferred
  どおり（ruviform 境界事項）。
- 電源断・実 durability fault — 外部 gate。
- macOS/Linux での CI 実走確認 — merge 後の対象 SHA の CI
  証拠（本 slice では Windows ローカル実測 + portable code
  のため CI で自動実走見込み）。

## Verification Notes

- [x] 実 2-process 同時 `save_cas` で exactly one winner・
  敗者 typed `Conflict`・最終 HEAD=勝者・敗者 snapshot
  非 publish・`HEAD.lock` 非残滓（3 round、Windows 実測）。
- [x] `mise run fmt-check` / `check synvelis-project` /
  `lint synvelis-project` / `test synvelis-project` — 全て
  exit 0（Windows・rustc 1.99.1、impl-ir に記録）。
