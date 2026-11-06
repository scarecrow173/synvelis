---
id: "TSPEC-034T2c6eMscPucNt6w8iYW"
type: "test-spec"
status: "approved"
title: "SYN-IO-02 Store Publish 契約試験"
created: "2026-11-06"
updated: "2026-11-06"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
  derived-by: []
  verifies:
    - "docs/plans/syn-io-02-directory-bundle-store-stagedfile.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SYN-IO-02 Store Publish 契約試験

## Purpose

`DirectoryBundleStore` の staged publish 移行後も、Snapshot
immutability・HEAD 順序・失敗時の分類付き正直な報告・orphan/残滓・
外部変更・再 Open の契約が守られることを実 OS（Windows/Unix 共通の
`std::env::temp_dir` 実ファイル）で pin する。`InMemoryStore` は同一の
immutable name 契約（冪等再送・watermark refresh・異 byte 拒否）に
整合済みであり、それもここで pin する。controller 経由の
Saved/Live/Undo/Recovery 契約は既存 `controller_store.rs` が担保する。

## Feature

DirectoryBundleStore publishes immutable snapshots then repoints HEAD
through `ruviform-io-file` staged publish.

## Rules

- Rule 1 — Snapshot は `CreateNew` の immutable name。同一 revision
  の `save` 再実行で内容が異なる場合は `Err` を返し、既存
  snapshot byte と HEAD を変更しない。既存 byte と完全一致する
  再送は冪等に受理し（snapshot publish→HEAD publish 間の crash
  復旧で孤児 snapshot を再利用する経路）、HEAD の再指向だけを
  行う。`SnapshotEnvelope` が `id_watermark` のみ異なる同一論理
  snapshot は watermark 単調増方向に refresh を許す（新しい byte
  へ `Replace`）、古い watermark は既存 byte を保持する冪等受理と
  する。それ以外の内容差異は既存 snapshot byte を上書きしない。
- Rule 2 — `save` は snapshot publish 成功（または冪等/refresh
  受理）後にのみ HEAD を進める。snapshot publish が失敗した場合
  HEAD は不変。
- Rule 3 — HEAD publish が `NotPublished` で失敗した場合、公開済み
  snapshot は orphan として残り、旧 HEAD の内容は保持される。
  `Published`/`Indeterminate` では HEAD が既に進んでいる可能性が
  あり、`head()` の再読込が実態を決める（store は effect を偽装
  しない）。
- Rule 4 — publish 失敗は `Error::Store` として分類情報
  （effect/phase/durability/operation id）付きで伝播する。
- Rule 5 — `*.rvf-tmp-*` 残滓・外部 HEAD 書換え・HEAD 不在は
  観測可能であり、`head()`/`read_snapshot()` はそれをそのまま
  報告する（store は外部変更を隠蔽しない）。
- Rule 6 — store drop/再 Open（同一 root への `create`）は HEAD・
  snapshot を再現し、load が成功する。
- Rule 7 — snapshot の永続 byte format（SnapshotEnvelope・canonical
  byte）は移行前と同一であり、`save` 成功時 HEAD file は
  snapshot 名そのまま（改行なし）を保持する。

## Examples

- E1: 正常 save×2 — `r1.json`/`r2.json` が存在し HEAD=`r2.json`、
  両 snapshot が残る（Rule 2, 7）。既存
  `directory_bundle_save_publishes_head_last` と同等の観測。
- E2: 同一 revision 再 save — rev1 で異なる byte の 2 回目 `save`
  → `Err`、`r1.json` byte 不変・HEAD 不変。同一 byte の再送は
  `Ok` で HEAD は `r1.json` のまま（Rule 1）。
- E3: snapshot publish 失敗 — `snapshots/r2.json` を外部作成して
  rev2 `save` → `Err`（DestinationExists 分類）、HEAD=`r1.json`
  不変、外部 byte 不変（Rule 2, 4）。
- E4: HEAD publish 失敗 — `HEAD` を directory として用意して rev2
  `save` → `Err`（publish 未適用）、`snapshots/r2.json` は
  orphan として存在、`HEAD` は directory のまま（Rule 3, 4）。
- E5: 外部 HEAD 書換え — HEAD を `r1.json` に書き換えると
  `head()`=`r1.json`、存在しない名への書換えは
  `read_snapshot` が `Err`（Rule 5）。
- E6: 残滓混入 — `snapshots` 配下に `*.rvf-tmp-*`・任意 garbage
  を置いても `save`/`head`/`read_snapshot` が正常動作（Rule 5）。
- E7: 再 Open — `create` し直した store が HEAD・snapshot を
  再現し `ProjectController::load` が成功（Rule 6）。
- E8: HEAD 不在 — 空 bundle で `head()`=`None`、controller
  `load` が `Error::Store`（Rule 5, 6）。
- E9: byte-faithful round-trip — `save` に渡した byte 列が
  `read_snapshot(head)` で byte-for-byte に一致し、HEAD file
  内容は snapshot 名そのまま（Rule 7。canonical byte 維持）。
- E10: watermark refresh — 同一 revision で `id_watermark` のみ
  増加した envelope の再 save は `Ok`、snapshot byte が新しい
  watermark へ更新され HEAD が再指向される。同一内容・より古い
  watermark の byte は既存を保持して受理（Rule 1）。
- E11: stage 失敗 — `snapshots/` が file のとき `save` は
  `Error::Store`（stage create 分類）、HEAD は生成されない
  （Rule 4 の stage phase）。

## Guarantees

- 同一 revision 名の snapshot の内容（envelope）が上書きされる
  ことはない（byte 同一再送と `id_watermark` 単調増 refresh のみ
  受理、内容差異は拒否）。
- snapshot publish に失敗した save は HEAD を前進させない。
  HEAD publish 自体の `NotPublished` 失敗は旧 HEAD を保持するが、
  `Published`/`Indeterminate` 失敗は HEAD が進んだ可能性を正直に
  報告し、`head()` 再読込が実態を決める。
- publish の中途失敗は `Error::Store` のメッセージに effect/
  phase/durability/operation id を含めて伝播する。
- store は `*.rvf-tmp-*` 残滓や外部 HEAD 変更を隠蔽せず、
  `head()`/`read_snapshot()` は実ファイル状態を報告する。

## Non-goals

- OS 固有の Indeterminate/DirSync 失敗注入（ruviform-io-file
  側の fault-injection 責務。consumer 側は「snapshot publish 失敗は
  HEAD を進めない」「HEAD publish 失敗は effect 分類どおりに報告
  される」経路を pin）。
- 複数 process の同時 save・真の HEAD CAS（単一 writer モデル
  外の責務）。
- `SnapshotEnvelope` serialization 単体の検証（既存
  controller_store テストが担保。`InMemoryStore` の immutable
  name 契約は port 整合として本試験で pin）。
- recording/gateway/player の `.tmp`/rename パイプライン。

## Risk

これらを外すと、crash/外部変更/古い writer 後に HEAD が不正な
snapshot を指す・既存 snapshot が上書き破壊される・publish 失敗が
虚偽の成功として報告され、Recovery/再 Open の判断を誤らせる。
