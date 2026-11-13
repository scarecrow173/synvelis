---
id: "DESIGN-034Tk3mXw7LpQ2vRnB8y"
type: "design"
status: "approved"
title: "SYN-IO-02 HEAD CAS consumer-side mutex 設計"
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
  implements:
    - "docs/specs/syn-io-02-head-cas.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
    - "docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md"
    - "DESIGN-034WVJwLjBMYR3fBrNJD2M"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-head-cas.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/288"
  defers: []
  deferred-by: []
---

# SYN-IO-02 HEAD CAS consumer-side mutex 設計

## Context

`DirectoryBundleStore::save` は `snapshots/r<N>.json` を
`PublishMode::CreateNew` で公開した後、`HEAD` を無条件
`PublishMode::Replace` で差し替える。rename は原子的だが
compare-and-swap ではない — 2 つの writer が同じ HEAD を
観測してから各々 save すると、後の repoint が先の commit を
検出なしに上書きする（lost-update）。`Command::expected_revision`
は live revision の楽観ロックであり store HEAD への検査では
ないため、別 process・別 controller・journal restore は防御の
外にいる。

制約:

- ruviform `ruviform-io-file` の `PublishMode` は
  `CreateNew`/`Replace` のみで CAS プリミティブを持たず、
  ruviform 側の変更は禁止（syn-io-01 境界 spec で HEAD CAS は
  Synvelis 責務と確定済み）。
- `ProjectStore` の既存 `save`/`head`/`read_snapshot` と HEAD
  file content（snapshot 名 verbatim・改行なし）は不変。
- 範囲外の `ProjectStore` 実装（test Fake 等）を壊さないため、
  port 追加は required method にできない。

## 検討した代替案

1. **ruviform への CAS primitive 追加** — `PublishMode::Compare`
   や expected-content rename 等。ruviform 非変更制約に反し、
   File API の一般性を DAW 固有の HEAD 意味論で汚す。棄却。
2. **generation-named HEAD entry + directory scan による
   linearizable CAS** — `HEAD.d/<gen>` を `CreateNew` で連番
   確保し、常に最新 generation を HEAD とみなす方式。read 側が
   dir scan で最新を選ぶ必要があり、`head()` 契約・外部ツール
   との HEAD file 互換を壊す。snapshot 名と同様 immutable だが
   「HEAD は 1 file」という既存 format 契約に反する。棄却。
3. **`HEAD.lock` advisory mutex + consumer 側 compare（採用）** —
   lock file を `CreateNew` で atomic 取得し、lock 内で
   HEAD read→compare→publish を直列化する。read-compare-
   publish が 1 つの相互排除区間に入るため、協調する
   `save_cas` caller 間では linearizable。既存 format・port・
   ruviform primitive のみで実現できる最小の正しい構成。

## 構成

### Port（`ProjectStore`）

```rust
fn save_cas(
    &mut self,
    project_id: &str,
    expected_head: Option<&str>,
    revision: ProjectRevision,
    bytes: &[u8],
) -> Result<String, HeadCasError>
```

- additive・default 実装は `Err(HeadCasError::Unsupported)`。
  required method にしない理由: 範囲外の Fake 実装
  （`vcs01_snapshot.rs`、`synvelis-library` `FailOnceStore`）
  を変更せずに port を拡張するため。default が unconditional
  `save` へ落ちないことが契約 — CAS を要求する caller は
  `Unsupported` を検出して明示的に対処する。
- `expected_head` は `head()` の報告値そのもの
  （`Option<&str>`、`None` = HEAD 不在の assert）。

### `HeadCasError`

| variant | 意味 | caller の対処 |
| --- | --- | --- |
| `Conflict { expected, actual }` | lock 内で観測した HEAD が expected と不一致 | `load` で再採用して retry。HEAD/snapshot 不変 |
| `Unsupported` | store が CAS 非対応（default body） | CAS 必須なら store を替える。`ProjectController::save` は legacy `save` へ fallback |
| `Store(Error)` | lock 取得・snapshot/HEAD publish・I/O 失敗 | 既存 publish 分類を保持。`head()` 再読込で reconcile |

`From<HeadCasError> for Error` は `Conflict`/`Unsupported` を
`Error::Store` の説明文字列へ写像する（`error.rs` は本 slice
の編集範囲外のため typed channel は port 境界の
`HeadCasError` に置く — store 利用者は variant を判別でき、
controller 経由の caller は message の `head CAS conflict:`
接頭辞で識別する）。

### `DirectoryBundleStore::save_cas` プロトコル

```
acquire HEAD.lock ──► read HEAD ──► compare expected
                                        │ mismatch → Conflict（snapshot 未公開・orphan 無し）
                                        ▼ match
                              publish_snapshot (CreateNew + collision 既存規則)
                                        │
                              fencing: lock.still_ours()? ── no → Store error（orphan snapshot 残存・HEAD 不変）
                                        ▼ yes
                              publish_head (Replace + FileAndDirectorySync)
                                        │
                              post-fencing: lock.still_ours()? ── no → Store error（indeterminate・head() で reconcile）
                                        ▼ yes
                                    Ok(name) → Drop で token 照合付き解放
```

- **lock 取得**: `HEAD.lock` を `StagedFile` + `PublishMode::
  CreateNew` + `Durability::BestEffort` で publish。lock は
  advisory 協調状態であり durable project state ではない —
  crash で name が消えても stale-lock 経路が回復する。
  内容は `synvelis-head-lock/1\ncreated_ns=<unix ns>\ntoken=<pid>-<counter>-<ns>`。
- **stale 判定**: `created_ns` が `HEAD_LOCK_STALE_AFTER`
  （20 秒）より古い・parse 不能・読取不能な lock は stale と
  みなし、`fs::rename` で一意な `HEAD.lock.stale-*` aside 名へ
  atomic に退避してから削除する。rename が唯一の仲裁 — 成功
  した contender のみが lock を解体し、他は `CreateNew` を
  retry する。fresh lock は `HEAD_LOCK_POLL_INTERVAL`（10ms）
  刻みで `HEAD_LOCK_BUSY_TIMEOUT`（30 秒、stale 閾値より長く
  wedge lock を待ち続けずに break へ進める設定）まで待機し、
  超過は `HeadCasError::Store`。
- **fencing（pre-publish）**: snapshot publish 後・HEAD publish
  前に `still_ours()`（lock file が自 token を保持するか）を
  確認。lock が stale 扱いで break 済みなら HEAD を repoint
  せず abort — publish 済み snapshot は `save` と同じ crash
  window の orphan として残り、HEAD は不変。
- **post-publish fencing**: HEAD publish 後にも `still_ours()`
  を再確認。pre-check と HEAD rename の間に lock が break され
  並行 writer が commit し得る residual window を塞ぐため、
  ここで lock 喪失を検出したら `Ok` を返さず
  `HeadCasError::Store`（indeterminate 相当）を返す — publish
  は着地したが他 writer の commit を上書き/上書きされた可能性
  があり、`head()` 再読込での reconcile を caller に委ねる。
  「成功誤報なし」の要件をこの経路でも守る。
- **解放**: `HeadLock` の `Drop` が `still_ours()` 照合後に
  `HEAD.lock` を削除 — break されて他 writer が再取得した
  lock を loser が消さない。

### `ProjectController::save`

- expected = `self.saved.as_ref().map(|s| s.head.clone())`
  （初回 save/load 前は `None` = HEAD 不在 assert）。
- `Ok` → `SavedState` 更新（従来どおり）。
- `Conflict`/`Store` → `Error` へ写像して return、`saved` 不変。
- `Unsupported` → 従来の無条件 `store.save`（単一 writer 前提の
  既存 store 向け後方互換。signature・戻り値は不変）。

### journal replay restore（`synvelis-gateway/src/commit.rs`）

- `controller.load` が採用した HEAD（`controller.saved().head`）
  を expected に `store.save_cas` で journaled state を復元
  publish する。
- force-save（無条件 `save`）は採用しない — restore は recovery
  意味論だが、load→restore の間に割り込んだ writer の HEAD を
  journaled state で silent に巻き戻すのはまさに本 Issue が
  防ぐ lost-update と同型。CAS 非対応 store でも fallback せず
  `io::Error` で loud failure とする（同梱の
  `DirectoryBundleStore`/`InMemoryStore` はいずれも CAS 実装
  済みであり、実運用経路での非対応は発生しない）。
- restore 成功後は従来どおり `controller.load` で再採用する。

### `InMemoryStore::save_cas`

`self.head` と expected を比較し、不一致は
`HeadCasError::Conflict`、一致は既存 `save`（immutable name・
collision 分類は port 共通）へ委譲する。`&mut self` が caller
を直列化するため lock 概念は不要 — port 契約の parity を
filesystem 無しで検証できる。

## 保証境界（guarantee boundaries）

- **原子性の単位は 1 publish**: 各 `publish_staged` は
  rename 原子性 + `FileAndDirectorySync` を要求する。snapshot
  と HEAD は別 publish であり multi-file transaction ではない
  — snapshot publish 後・HEAD publish 前の crash/publish 失敗
  は orphan snapshot を残す（既存 `save` と同一の crash
  window。`snapshot_collision` の冪等受理が recovery 経路）。
- **相互排除は協調 writer 間のみ**: `save_cas` を呼ぶ writer
  同士は `HEAD.lock` で直列化される。`save`（無条件）・外部
  プロセスの HEAD 書換え・lock を取らない書き込みは排他の外
  — 防げないが、次回 `save_cas` の expected 比較で conflict
  として観測される（silent ではない）。
- **stale lock**: `HEAD_LOCK_STALE_AFTER`（20s）超過の lock は
  crashed writer の残滓とみなし rename-aside で break する。
  正常な critical section（小 file の staged publish 数回）は
  この閾値を大きく下回る。
- **長時間 hold の residual race**: critical section が stale
  閾値を超えて継続した場合、lock は break 可能になり得る。
  pre/post fencing が「lock 喪失後に `Ok` を返さない」ことを
  保証するが、break→相手 commit→自分の HEAD rename が微小
  window で競合する物理的余地は advisory lock の限界として
  残る（kernel file lock ではない）。発生時は post-fencing が
  indeterminate error を返し、HEAD 内容と `head()` 再読込が
  実態を決める — 誤った成功報告はしない。
- **crash window**: lock file は `BestEffort` durability —
  crash で `HEAD.lock` が残っても stale break が回収し、
  消えても排他が一時的に緩むだけで正しさは expected 比較が
  守る。`HEAD.lock.stale-*` aside の残滓は無害（`head()`・
  `read_snapshot` は関与しない）。
- **非対象**: 実電源断・fsync 挙動の実機検証、macOS/Linux 実測、
  全 filesystem での rename/dirsync 等価性、非協調 writer への
  強制排他。これらは外部 gate として NotRun 記録する。

## Non-Functional Constraints

- **互換性**: `ProjectStore::save`/`head`/`read_snapshot`、
  `ProjectController::save` signature、HEAD/snapshot の永続
  byte format は不変。`HEAD.lock` は新規の追加 file であり、
  既存 bundle には存在しない（初回 `save_cas` で生成・
  正常終了時に削除）。
- **RT**: store は非 RT 文脈契約を維持（`HEAD.lock` の
  `std::thread::sleep` 待機も save 経路のみ）。
- **依存追加なし**: ruviform-io-file の既存 primitive
  （`StagedFile`/`PublishMode::CreateNew`/`PublishFailureKind::
  DestinationExists`）のみで実装。`std::fs` の rename/read のみ
  新規利用。

## Risks and Trade-offs

- **advisory lock は強制力を持たない**: kernel lock でないため
  非協調 writer を防げない — ただし検出は次回 CAS で必ず
  起きる（conflict）し、成功誤報は post-fencing が塞ぐ。
  Git `index.lock` と同クラスの保証として受容する。
- **stale break の誤判定**: clock skew・NFS 等で `created_ns`
  が信頼できない環境では fresh lock を stale と誤 break し得る
  — その場合も token 照合 fencing が repoint 前に検出し、
  最悪でも indeterminate error であって silent lost-update
  にはならない。
- **`HEAD.lock` 残滓**: 異常終了で残っても次回 `save_cas` が
  stale break で回収。`save`（lock 非取得）には影響しない。
- **待機時間**: fresh lock 保持者の critical section が長いと
  `save_cas` は最大 30 秒待つ — store 呼び出しは非 RT の
  save/load 経路であり許容範囲。

## Deferred Design Concerns

- kernel-level file lock（`flock`/`LockFileEx`/`fcntl`）や
  ruviform への CAS primitive — 境界制約により対象外。真の
  multi-process 強制排他が要件化した場合は ruviform Issue
  として再検討する。
- `HEAD.lock` の定期 GC・orphan snapshot GC — store の
  recovery/GC policy は別責務（project-asset-persistence）。
- `save_cas` の batch/transaction 化（複数 revision を
  1 lock で）— 要件なし（YAGNI）。

## Verification Notes

- [x] 同一 expected HEAD の 2 store 同時 `save_cas` が
  single-winner（loser は typed `Conflict`、最終 HEAD は
  勝者 snapshot）。
- [x] `expected=None` の absent-assert、stale expected の
  conflict、conflict 時に snapshot が publish されないこと。
- [x] `InMemoryStore` parity、`Unsupported` fallback、
  stale/fresh `HEAD.lock`、失敗時 lock 解放。
- [x] `controller.save` が moved HEAD を conflict で拒否し
  `saved` を進めないこと、reload→retry で回復すること。
- [x] 実 2-process — 後続 slice `syn-io-02-head-cas-e2e`
  （`docs/designs/syn-io-02-head-cas-e2e-design.md`）で
  Windows 実測済み: test binary self re-exec の 2 process
  が同一 root で `save_cas` し、exactly one winner + loser
  typed `Conflict` + 最終 HEAD=勝者 + `HEAD.lock` 非残滓を
  複数 round で pin。macOS/Linux は portable code のため
  CI 実走または NotRun。
- [ ] macOS/Linux 実機・電源断 — NotRun（外部 gate）。
