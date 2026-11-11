---
id: "IR-SV330-syn-io-02-head-cas"
type: "ir"
status: "final"
title: "SYN-IO-02 HEAD CAS expected-head store port 実装記録・検証証跡"
created: "2026-11-12"
owners: ["scarecrow173"]
---

# SYN-IO-02 HEAD CAS expected-head store port — 実装記録・検証証跡

## 対象

- Issue: https://github.com/scarecrow173/synvelis/issues/330
- Branch: `scarecrow173/sv330-head-cas`
- Ruviform 解決 SHA: `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`
  （`Cargo.lock` 不変 — ruviform 側変更なし）

## 実施内容

- `ProjectStore` に additive な `save_cas(project_id,
  expected_head, revision, bytes) -> Result<String,
  HeadCasError>` を追加。default 実装は typed
  `HeadCasError::Unsupported` — unconditional `save` への
  silent degrade を禁止し、required method 化による範囲外
  Fake 実装（`vcs01_snapshot.rs`・`synvelis-library`
  `FailOnceStore`）の破壊を回避。
- `HeadCasError`: `Conflict { expected, actual }`（lock 内で
  観測した HEAD が expected と不一致）/ `Unsupported` /
  `Store(Error)`（既存 publish 分類を保持）。`From` で
  `Error` へ写像（`error.rs` は範囲外のため typed channel は
  port 境界に配置）。
- `DirectoryBundleStore::save_cas`: `HEAD.lock` を
  `StagedFile`+`CreateNew`+`BestEffort` で atomic 取得する
  advisory mutex。内容は `created_ns`+一意 `token`。stale
  （`HEAD_LOCK_STALE_AFTER`=20s 超過・parse 不能・読取不能）
  は `fs::rename` の一意 aside 名への atomic 退避で break、
  fresh は 10ms poll・`HEAD_LOCK_BUSY_TIMEOUT`=30s で待機。
  critical section: lock 取得 → `head()` 読取 → expected 比較
  → `publish_snapshot`（既存 collision/durability 規則を
  `save` と共有する helper に分解）→ pre-publish fencing
  （`still_ours`）→ `publish_head`（既存 `Replace`+
  `FileAndDirectorySync`）→ post-publish fencing → `Ok`。
  `HeadLock::drop` は token 照合後に削除し、break されて
  他 writer が再取得した lock を消さない。
- post-publish fencing: pre-check と HEAD rename の間に
  lock が break される residual window で `Ok` を返さない
  — `HeadCasError::Store`（indeterminate 相当）で `head()`
  再読込へ委ねる（成功誤報なし）。
- `InMemoryStore::save_cas`: `self.head` と expected を比較
  し `Conflict`/一致時は既存 `save` へ委譲 — port 契約の
  parity を filesystem 無しで再現。
- `ProjectController::save`: `self.saved.head` を expected
  に供給（未 save/load は `None` = absent assert）。
  `Ok` → `SavedState` 更新、`Conflict`/`Store` → `Error`
  写像で `saved` 不変、`Unsupported` のみ legacy
  `store.save` へ fallback。公開 signature 不変。
- `synvelis-gateway/src/commit.rs` journal replay restore:
  `controller.load` が採用した HEAD を expected に
  `save_cas` で復元 publish。force-save fallback 無し —
  `Unsupported`/競合/失敗はすべて `io::Error` で loud
  failure（load→restore 間の HEAD 移動を silent rewind
  しない）。
- `tests/store_publish.rs` E4 整合: `controller.save` が
  CAS 経由になったため HEAD 不可読（directory 化）では
  publish 前に失敗し orphan が残らない — publish 契約は
  `store.save` 直接経路（手組み envelope）で pin し直し。

## 変更ファイル

- `crates/synvelis-project/src/store.rs`
- `crates/synvelis-project/src/controller.rs`
- `crates/synvelis-gateway/src/commit.rs`
- `crates/synvelis-project/tests/head_cas.rs`（新規・8 tests）
- `crates/synvelis-project/tests/store_publish.rs`
- `docs/specs/syn-io-02-head-cas.md`（新規）
- `docs/designs/syn-io-02-head-cas-design.md`（新規）
- `docs/plans/syn-io-02-head-cas-plan.md`（新規）
- `docs/tasks/syn-io-02-head-cas-task.md`（新規）
- `docs/test-specs/syn-io-02-head-cas.md`（新規）
- `docs/impl/ir/syn-io-02-head-cas.md`（本書）
- `docs/specs/syn-io-02-snapshot-head-staged-publish.md`
  （Out of Scope/HEAD 記述を実装状態へ整合）
- `docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md`
  （同上）

## 検証証跡（実行環境 Windows・全 pass）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | pass |
| `mise run check`（cargo check --workspace --all-targets） | pass |
| `mise run lint`（cargo clippy --workspace --all-targets -- -D warnings） | pass |
| `mise run test`（cargo test --workspace --locked） | pass・206 test binary・0 fail |
| `mise run test-rt`（rt_ contract tests） | pass |
| `mise run check-app-native` | pass |
| `mise run lint-app-native` | pass |
| `mise run test-msrv`（rust 1.89・cargo test --workspace --locked） | pass・206 test binary・0 fail |
| `mise run build`（cargo build --workspace --locked） | pass |

契約テスト（`tests/head_cas.rs`・test-spec Rules 1–8 対応）:
save_cas の expected 一致 commit・stale/absent-assert conflict
（conflict は publish 前判定で snapshot/HEAD/orphan 不変）、
同一 root 2 store 同時 `save_cas` の single-winner + typed
`Conflict` + 最終 HEAD=勝者、`InMemoryStore` parity、
`Unsupported` fallback（bare store で `controller.save` 成功）、
stale `HEAD.lock` の rename-aside break、fresh foreign lock の
待機→解放後 commit、snapshot publish 失敗時の lock 解放と
retry 回復、controller moved-HEAD conflict（`saved` 非前進・
divergent snapshot 非 publish・reload→retry 回復）。

## NotRun / Unsupported（外部 gate・正直記録）

- 実 2-process/複数 process 間の `HEAD.lock` 協調 — 本 slice
  は同一 process 内 thread 協調までを pin。実 process 検証は
  別 Issue/外部 gate へ委譲。
- macOS/Linux 実機での `HEAD.lock`/stale-break 挙動 —
  NotRun（Windows 実測のみ。`fs::rename`/read の OS 差は
  ruviform publish と同じ portability 前提）。
- 電源断・実 durability fault 注入 — ruviform-io-file 側
  責務（既存 SYN-IO-02 と同じ委譲先）。
- 非協調 writer（`save` 直接・外部 HEAD 書換え）への強制排他
  — advisory lock の設計限界として design に記録。検出は
  次回 CAS の conflict で行われる。
- `HEAD_LOCK_STALE_AFTER`（20s）超過 hold の residual race —
  post-publish fencing で誤成功は塞ぐが物理的余地は残存
  （kernel file lock なしの advisory mutex の限界）。

## 残件

- docs index（`docs/*/README.md`）の再生成は範囲外 —
  coordinator/統合側で実施。
- 実 multi-process gate・macOS/Linux・電源断は上記 NotRun
  のとおり外部検証事項。
