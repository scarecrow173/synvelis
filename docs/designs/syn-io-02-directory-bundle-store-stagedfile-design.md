---
id: "DESIGN-034T2aGDbNinkumWeftaiP"
type: "design"
status: "approved"
title: "SYN-IO-02 DirectoryBundleStore StagedFile 移行設計"
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
  implements:
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/282"
    - "https://github.com/scarecrow173/ruviform/issues/296"
  defers: []
  deferred-by: []
---

# SYN-IO-02 DirectoryBundleStore StagedFile 移行設計

## Context

`DirectoryBundleStore` は `<root>/snapshots/r<N>.json` +
`<root>/HEAD` の directory bundle を永続化する。現行は
`write_atomic` が `path.with_extension("tmp")` の固定名 temp に
`File::create` → `write_all` → `sync_all` → `fs::rename` を行う。

この実装の限界:

- 固定 temp 名は crash 残滓・衝突の診断性が低い。
- `sync_all` は file sync のみで directory sync を持たず、
  rename 後の name durability を保証しない。
- `fs::rename` は destination 既存でも置換する — immutable
  snapshot 名への CreateNew 保証がない。
- publish 結果が「適用/未適用/不明」のどれかを分類しない。
  失敗は一律 `Error::Store` であり、旧 HEAD 維持・orphan
  snapshot・indeterminate leftover の区別が呼び出し側に
  伝わらない。

Ruviform `ruviform-io-file`（IO-IMP-01/02、ruviform #295/#296）は
`StagedFile`→`SealedFile`→`publish` の型状態 lifecycle と、
`PublishEffect`（`NotPublished`/`Published`/`Indeterminate`）・
`PublishPhase`・`DurabilityEvidence`・`RecoveryHandle` を持つ
分類済み失敗を提供する。本設計は store の publish 段のみを同
API へ移管し、Synvelis 固有の意味論（Schema/Revision/HEAD/
Recovery/Undo）は Consumer 側に残す。

## Scope

### In Scope

- `write_atomic` 撤去、`publish_staged(path, bytes, mode)` 相当の
  内部 helper への置換。
- snapshot `CreateNew`・HEAD `Replace`、双方
  `Durability::FileAndDirectorySync` /
  `VisibilityRequirement::RequireAtomic`。
- `PublishFailure` → `Error::Store` 写像（分類情報の転写）と
  recovery handle の方針。
- `ruviform-io-file` 依存宣言と `Cargo.lock` 再生成。

### Out of Scope

- `ProjectStore` port 署名変更、`SnapshotEnvelope`/canonical byte/
  schema version の変更（`InMemoryStore` は port の immutable name
  契約への整合のみ行い、serialization・API は不変）。
- 他 crate の `.tmp`/rename（recording/gateway/player）。
- HEAD CAS 機構の新設（単一 file replace は CAS ではない。
  複数 writer 競合は単一 writer モデル＋呼び出し側の
  直列化が担う既存判断を維持）。
- Autosave/Backup への展開。

## Components and Boundaries

- `DirectoryBundleStore`（synvelis-project 所有）: publish 順序
  （snapshot → HEAD）・immutable name・orphan 方針・error 写像を
  所有。OS primitive の詳細は ruviform に委譲。
- `ruviform_io_file::StagedFile`/`SealedFile`（ruviform 所有）:
  temp sibling の排他作成（`*.rvf-tmp-<token>`）、Write+Seek、
  seal、publish pipeline（validate→file sync→close→atomic
  publish→dir sync→cleanup）、失敗分類、recovery handle。
- `ProjectController`（synvelis-project 所有・変更なし）:
  `Command::expected_revision` の CAS、単一 writer、
  SavedState/Undo/Recovery 判断。store 失敗はそのまま伝播し、
  `saved` は更新されない。

## Data and Control Flow

`save(project_id, revision, bytes)`:

1. `name = snapshot_name(revision)`（`r<N>.json`、immutable）。
2. snapshot publish: `StagedFile::create(snapshots/name)` →
   `write_all(bytes)` → `seal()` →
   `publish(CreateNew, FileAndDirectorySync, RequireAtomic)`。
   - `Ok(receipt)`: 次へ。receipt に recovery handle が残る
     （temp 残留）は best-effort cleanup。
   - `Err(failure)`: HEAD へ進まず `Error::Store` を返す。
     `Unsupported`・`Io`・`Published`/`Indeterminate` は分類込み
     で転写する — snapshot は orphan/不明 leftover として残り、
     呼び出し側は再読込/Recovery を判断する。
   - `DestinationExists` の例外（`snapshot_collision` で判定）:
     - byte 完全一致、または envelope が `id_watermark` のみ
       異なり既存側が新しい場合 → 公開済み snapshot がこの save
       を冪等に満たすものとして継続（snapshot→HEAD publish 間
       crash の孤児を journal replay が再 save する gateway
       回復経路を維持）。
     - envelope が `id_watermark` のみ異なり *incoming* が新しい
       場合 → `Replace` で新 byte に refresh して継続。
       `controller.save` は `draw_raw_id` による外部 ID 予約を
       save 時に `id_watermark` へ刻むため、apply なしの予約で
       同一 revision の canonical byte が変わり得る — 単調増の
       bookkeeping 前進であり内容は同一論理 snapshot。
     - 上記以外（parse 不能・内容差異）は `Error::Store` を返し、
       既存 byte も HEAD も変更しない。
3. HEAD publish: `StagedFile::create(root/HEAD)` →
   `write_all(name)` → `seal()` →
   `publish(Replace, FileAndDirectorySync, RequireAtomic)`。
   - `Err(failure)`: snapshot は公開済み orphan として残り、旧
     HEAD file は publish 未適用なら不変（`NotPublished`）。
     `Published`/`Indeterminate` は再 Open 時の `head()` 読値が
     真実を決める（設計正本の ACK-lost 契約）。
4. 両方 `Ok` で `Ok(name)` を返す。

`head()`/`read_snapshot()`/`create()` は変更しない。
`InMemoryStore` は port の immutable name 契約に整合させる —
同名 snapshot の byte 一致再送は冪等受理、異 byte は `Error::Store`
を返す（directory store と同一セマンティクスで、port 利用側が
テスト double で実装と乖離した挙動を観測しないため）。

### 失敗写像

`PublishFailure` → `Error::Store(format!("{failure}"))` に
temp path を付記する。`PublishFailure` の Display は
phase/kind/operation id/effect/durability を含む。

- `PublishEffect::Indeterminate`: recovery handle・temp を
  **削除しない**（API 契約）。temp path を error message に
  含め、手動/将来の recovery で検査・削除可能にする。
- `NotPublished`/`Published` の失敗: `take_recovery_handle()` →
  `handle.cleanup()` を best-effort で試みる（結果は無視。
  削除不能でも publish 失敗報告を変えない）。
- `StagedFile::create`/`write_all` の失敗: 所有権が drop へ
  戻り `cleanup_on_drop` で temp が best-effort 削除される
  （旧実装の `<name>.tmp` 残留を解消）。

## Non-Functional Constraints

- **互換性**: `ProjectStore` trait・`Error` enum・永続 byte
  format（snapshot JSON・HEAD 内容）は不変。`HEAD` ファイルの
  内容は snapshot 名そのまま（改行なし）を維持。
- **Durability 方針の明示的変更**: 旧 `File::sync_all` のみ
  → `FileAndDirectorySync` 要求。アーキテクチャ設計
  （project-asset-persistence、HEAD atomic replace + dir sync）
  に整合する意図的な引き上げであり、dir sync を提供できない
  FS/backend では silent fallback せず `Published` 失敗として
  正直に報告する。
- **RT**: `ruviform-io-file` は non-RT のみ。store は非 RT 文脈
  （save/load/recovery）でのみ使用される契約を維持。
- **依存**: 新規依存は同一 git repo `ruviform-io-file` のみ
  （既存承認の ruviform git dep 群に追加。libc/windows-sys は
  ruviform 側 target dep で consumer へは波及しない）。

## Risks and Trade-offs

- **DirSync 非対応 FS で save が失敗する**: 設計正本の要求であり
  silent downgrade より正直な失敗を取る（PAP-15: 保証低下・
  Reject 等の policy 対象。`Published` effect で「中身は書けた
  が name durability 不明」を区別できる）。
- **同一 revision 再 save の衝突**: 旧実装は無条件上書き成功。
  厳密な CreateNew 拒否は 2 経路を壊す — (a) gateway の crash
  回復（`commit.rs` journal replay がコミット済み revision を
  `store.save` で復元 — snapshot 公開済み・HEAD 未指向の孤児を
  抱えて再起動した場合）、(b) `draw_raw_id` の外部 ID 予約
  （import/recording manifest が apply なしで ID を消費すると
  `controller.save` の `id_watermark` stamp が同一 revision の
  byte を変える）。そこで `DestinationExists` を
  `snapshot_collision` で Satisfied/Refresh/Conflict に分類する
  immutable name 契約とした — envelope 内容が `id_watermark`
  のみ異なる同一論理 snapshot は単調増方向に refresh 受理し、
  それ以外の内容差異は拒否する。controller 経由の通常 save は
  revision 単調増で衝突しない。
- **`*.rvf-tmp-*` 残留（Indeterminate）**: 自動削除しない設計。
  `load`/`save` は temp 残滓に依存しないため機能影響なし。
  orphan cleanup は別ポリシー（設計正本「後日 Cleanup 候補」）。
- **Windows `MoveFileExW(temp,dest,0)` の CreateNew**: ruviform
  が検証済みの atomic no-replace primitive。ACCESS_DENIED の
  診断 probe は ruviform 内部に閉じる。

## Deferred Design Concerns

- HEAD への expected-generation 書き込み（真の CAS）— 単一
  file では実現不能。複数 writer 必要時に ProjectStore 拡張
  として再検討。
- store レベルの orphan GC・定期 temp cleanup — GC policy は
  project-asset-persistence の別責務。

## Verification Notes

- [ ] snapshot `CreateNew` が内容差異のある同一 revision 再 save
  を拒否し既存 byte・HEAD が不変であること、同一 byte 再送が
  冪等受理されること、`id_watermark` 単調増の同一論理 snapshot
  が refresh 受理されること。
- [ ] snapshot publish 失敗時に HEAD が進まず、HEAD publish
  失敗時に snapshot が orphan として残る。
- [ ] `Published`/`Indeterminate`/`DirSync` 失敗が `Error::Store`
  として分類情報付きで伝播する（OS 注入不能な分岐は
  ruviform 側検証に委譲し、consumer は「任意の publish 失敗で
  HEAD を進めない」経路を pin）。
- [ ] `*.rvf-tmp-*` 残滓・外部 HEAD 書換え・HEAD 不在・再 Open
  が観測できる。
- [ ] `cargo fmt --check` / `clippy -D warnings` /
  `cargo test --workspace --locked` が pass。
