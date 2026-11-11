---
id: "DESIGN-034VplpQjplSqKKaNy9r1G"
type: "design"
status: "approved"
title: "RCD-02 StagedFile Publish Migration Design"
created: "2026-11-10"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  defers: []
  deferred-by: []
---

# RCD-02 StagedFile Publish Migration Design

## Context

`crates/synvelis-recording` の staging publish は `staging.rs` の
`publish_atomic`/`publish_atomic_faultable` が担い、内部は
`path.with_extension("tmp")` への `File::create` → `write_all` →
`sync_all` → `fs::rename` の手書き pipeline である。temp 命名・
排他作成・no-replace primitive・sync/dir-sync の段階分類は全て
synvelis 側の重複実装であり、#84 で synvelis-project が採用した
`ruviform_io_file::StagedFile`/`SealedFile::publish` と契約が
分岐している。

upstream（ruviform `ruviform-io-file`、Cargo.lock rev
`b02942c2f9e6e195d0d752803b077866468fe58e`）の公開面:

- `StagedFile::create(destination)` — destination の sibling に
  排他作成の temp（`<name>.rvf-tmp-<token>`）を開く。`Write +
  Seek` を実装し、内部 `File` は公開しない。
- `StagedFile::seal()` → `SealedFile`（write phase 終了の型証明。
  `Write`/`Seek` を実装しない）。
- `SealedFile::publish(PublishOptions)` →
  `Result<PublishReceipt, PublishFailure>`。pipeline は
  validate → file sync（`Durability::FileSync` 以上）→
  handle prepare → atomic publish（`PublishMode::CreateNew` は
  `MoveFileExW(flags=0)`/`renameat2(RENAME_NOREPLACE)`/
  `renamex_np(RENAME_EXCL)` 等の真の no-replace、
  `Replace` は `fs::rename`）→ dir sync（`FileAndDirectorySync`
  のみ）→ temp cleanup。
- `PublishFailure` は `kind`（`DestinationExists`/`Unsupported`/
  `Io`）・`phase`（`Validate`/`FileSync`/`Close`/`Publish`/
  `DirectorySync`/`Cleanup`）・`effect`
  （`NotPublished`/`Published`/`Indeterminate`）・達成
  `durability`（`None`/`FileSynced`/`FileAndDirectorySynced`）・
  `RecoveryInfo`/`RecoveryHandle` を保持する `std::error::Error`。
- `PublishOptions::new(mode, durability)` は
  `VisibilityRequirement::RequireAtomic` を既定とする。

先例: `synvelis-project/src/store.rs` の `publish_staged` +
`PublishStepError` + `publish_error`（#84 実装済み）は、stage/
write/publish の 3 段失敗を分類保存し、非 Indeterminate leftover
を best-effort cleanup、Indeterminate leftover を保持する規約を
確立している。本設計はこの規約に倣う。

## Scope

### In Scope

- `staging.rs` の publish 内部を StagedFile pipeline へ差し替え、
  公開 signature を維持した薄い wrapper とする。
- manifest publish の `PublishMode::CreateNew` 化と
  `AlreadyExists` 分類。
- `PublishInject`/`PublishFaults` seam の StagedFile pipeline への
  写像維持。
- `synvelis-recording/Cargo.toml` への `ruviform-io-file` 追加と
  `Cargo.lock` 再生成。
- fault matrix 回帰 + 任意の unsynced-write 喪失 fixture、
  evidence ledger 更新。

### Out of Scope

- `synvelis-project` 側 HEAD CAS/migration（#84 残件）。
- #86 verified-blob 実接続。
- media/journal の Raw 追記 Writer 自体の atomic write 化
  （issue 契約どおり対象外）。
- `synvelis-player` 側 publish 経路の CreateNew 化等の挙動変更
  （signature 維持の範囲で無変更）。

## Design

### 1. Publish port seam（`staging.rs`）

公開 API は signature 維持の wrapper とし、実体を private な
`publish_staged` に集約する:

```rust
fn publish_staged(
    path: &Path,
    bytes: &[u8],
    mode: PublishMode,          // ruviform_io_file::PublishMode（内部型）
    faults: &mut PublishFaults,
) -> Result<(), PublishStepError>
```

`PublishStepError` は synvelis-project/store.rs と同型の 3 分類:

- `Stage(FileError)` — `StagedFile::create` 失敗（bytes 未書込み、
  destination 非接触）。
- `Write { source: io::Error }` — `write_all` 失敗（temp は
  `StagedFile` の Drop が best-effort cleanup）。
- `Publish(PublishFailure)` — `SealedFile::publish` 失敗
  （effect/phase/kind/durability/recovery handle を保持）。

公開 wrapper（全て `io::Result<()>`、signature 不変）:

- `publish_atomic(path, bytes)` = `publish_staged(path, bytes,
  PublishMode::Replace, &mut PublishFaults::new())` を io error へ
  写像。**Replace 維持** — 現行 `fs::rename` の create-or-replace
  意味論と一致し、player の blob/receipt 再公開・commit marker
  冪等再公開・review record 再公開を壊さない。
- `publish_atomic_faultable(path, bytes, faults)` = 同上 +
  注入 seam。finalized.json/review.json の冪等再公開が
  `Replace` を必要とするため faultable 側も `Replace`。
- `publish_atomic_create_new(path, bytes)`（新規 pub）=
  `PublishMode::CreateNew`。manifest publish 専用。no-replace
  primitive で `exists()`→publish の TOCTOU を閉じる。

### 2. `PublishOptions` の選択

- `mode`: 呼出し別（上記）。公開 API には ruviform 型を出さず、
  wrapper 関数で固定する。
- `durability`: **`Durability::FileSync`**。現行実装は tmp file の
  `sync_all`（file sync 相当）+ rename で、dir sync は publish 内
  では行わず caller が `detect_dir_sync` capability 検出済みの
  場合のみ `sync_dir`/`sync_parent_dir` を別途呼ぶ設計。この分離
  を維持し `FileAndDirectorySync` へは上げない — dir sync 非対応
  FS で publish が `PublishEffect::Published` エラー化する退行を
  避け、`StagingCapabilities.directory_sync` の正直な記録と
  整合させる。
- `visibility`: `PublishOptions::new` 既定の `RequireAtomic`
  （唯一サポートされる level。現行 rename pipeline と同等の
  atomic visibility を要求）。

### 3. Fault injection seam の写像（`faultinj.rs`/`staging.rs`）

`PublishInject`/`PublishFaults` の signature・call カウンタ・
注入評価順は不変。注入位置を新 pipeline の対応 stage へ写像する:

| Inject | 旧 pipeline での位置 | 新 pipeline での位置 | 観測結果（不変条件） |
| --- | --- | --- | --- |
| `Denied { call }` | tmp create 前に PermissionDenied | `StagedFile::create` 前に PermissionDenied を返す | destination 非接触・Err |
| `SyncFail { call }` | tmp `write_all` 後・`sync_all` 前に Err（rename 不実施、tmp 残留し得る） | `seal` 後・`publish` 前に `sealed.abort()` して Err（temp は cleanup、publish 不実施） | destination 非接触・Err。temp 残留は「残留し得る」記述どおり必須でない |
| `Indeterminate { call }` | `fs::rename` 成功後に Err | `publish` が `Ok` を返した後に Err（file は公開済み） | destination 公開済み + Err（応答喪失相当） |

`Indeterminate` は publish 成功後にのみ発火する（旧実装が rename
失敗時に注入を評価しないのと同じ順序）。publish 自体の失敗は
注入に関わらず `PublishStepError::Publish` として分類される。

### 4. `io::Result` 境界での分類保存

`publish_staged` の `PublishStepError` を `io::Error` へ写像する
際、分類を失わない:

- `PublishFailureKind::DestinationExists` →
  `io::ErrorKind::AlreadyExists`（manifest の
  `StagingError::AlreadyExists` 分類に必要）。
- その他 → `io::Error::other(failure)` 相当で `PublishFailure`
  を source として保持。`Display` が phase/kind/effect/
  durability/operation id を含むため message に分類が残る。
- `Stage(FileError)` → source の `io::Error` を downcast で回収
  して kind を保存（回収不能時は `Other`）。
- `Write { source }` → そのまま `io::Error` として返す。

`PublishFailure.effect() == Indeterminate` の leftover は
ruviform 契約どおり自動削除しない。それ以外
（`NotPublished`/`Published`）の recovery handle は
`synvelis-project` 先例どおり best-effort `cleanup()` する。
publish 成功時に temp retained の recovery handle が残った場合も
同様に best-effort cleanup する（publish 結果は変えない）。

### 5. `StagingArea::create` の manifest publish

```text
manifest_path.exists() → AlreadyExists（既存の早期判定は維持）
↓ streams/stream file 配置（不変）
manifest bytes encode
↓
publish_atomic_create_new(manifest_path, bytes)
  ├─ Ok → 既存どおり dir sync capability 時 sync_dir
  ├─ AlreadyExists io error → StagingError::AlreadyExists
  │   （exists()→publish 間に manifest が出現した競合を正直に分類）
  └─ その他 → StagingError::Io（既存どおり）
```

`exists()` 早期判定は残す（通常経路の高速拒否）が、正しさの
authority は `CreateNew` の atomic no-replace に移る。

### 6. 変更しないもの

- `writer.rs` の media/journal `FaultWriter` append・
  `checkpoint()` の sync 順序・`finalize()` の finalized publish
  呼出し（`publish_atomic` 経由のまま）。
- `recovery.rs` の `recover_stream_finalize_faultable`/
  `publish_review` の呼出し形（`publish_atomic_faultable` 経由の
  まま）。
- `commit.rs` の commit marker/commit manifest publish
  （`publish_atomic` 経由のまま。stale manifest の remove-first
  workaround は現行意味論を維持するため残す）。
- `faultinj.rs` の `FaultWriter`/`Inject`/`corrupt_byte`（media/
  journal 注入系は無変更）。
- `synvelis-player`/`synvelis-project`/他 crate — `publish_atomic`
  signature 維持で無変更。

### 7. Optional fixture（unsynced-write 喪失）

crash で最終 sync frontier より後の media/journal byte が失われた
状態を、ファイル truncate で再現する test を `tests/` に 1 件追加
する（採用は実装時判断）: checkpoint 確定後の追記分を media/journal
双方から切り捨て、`scan`/`review_stream(Adopt)` が frontier までの
verified prefix のみを `Partial` として確定し、失われた byte を
推測復元しないことを確認する。

## File Map

- `crates/synvelis-recording/Cargo.toml` — `ruviform-io-file.workspace = true` 追加
- `crates/synvelis-recording/src/staging.rs` — `publish_staged`/
  `PublishStepError`/error 写像、3 wrapper、manifest CreateNew 化
- `crates/synvelis-recording/src/faultinj.rs` — `PublishInject`/
  `PublishFaults` doc を新 pipeline 写像に更新（API 不変）
- `crates/synvelis-recording/src/lib.rs` — `publish_atomic_create_new` export 追加
- `crates/synvelis-recording/src/writer.rs`/`recovery.rs`/`commit.rs`
  — 呼出し整合（原則無変更、必要時のみ import 調整）
- `crates/synvelis-recording/tests/` — 回帰確認 + optional fixture
- `Cargo.lock` — `synvelis-recording` の依存 edge 追加に伴う再生成
- `docs/` チェーン・`evidence/reports/` ledger — 本 slice の記録

## Risk Register

- **R1 player 互換**: `publish_atomic` の意味論差（temp 命名・
  write 失敗時 residue・dir sync 非実施）で player が壊れる危険。
  → signature 維持 + `Replace`/`FileSync` parity + workspace 全体の
  `check`/`test` で検出。player test も `mise run test` に含まれる。
- **R2 fault seam の観測差**: `SyncFail` で旧実装は `*.tmp` residue
  を残したが新実装は temp を cleanup する。「残留し得る」は
  必須でない記述のため契約違反ではないが、test-spec/ledger に
  差分を明記する。
- **R3 CreateNew 対応 platform**: `PublishMode::CreateNew` は
  unix/Windows で真の no-replace を提供。その他 target は
  `Unsupported` を返し得る — recording は Windows/macOS/Linux
  想定で、`Unsupported` は `io::Error` として fail-closed に
  表面化する（silent fallback なし）。
- **R4 Indeterminate leftover**: ruviform 契約で Indeterminate の
  temp は自動削除されない。`.rvf-tmp-*` orphan が streams dir に
  残り得るが、recovery scan は manifest の既知 filename のみを
  読むため分類に影響しない。ledger/test-spec に明記。
- **R5 MSRV**: `let ... else`/`if let ... && let` 等の edition 2024
  構文は rust-version 1.89 で使用可能な範囲に留める
  （store.rs 先例と同等）。

## Verification Matrix

- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `test-msrv` — canonical 検証。
- `cargo test -p synvelis-recording` 系は `mise run test` に包含
  （補助証拠として個別実行も可・明記する）。
- `synvelis-player` の `import.rs`/`publish.rs` が無変更で
  build/test Pass（workspace test に包含）。
- Windows/NTFS 実測: `detect_dir_sync` の観測値・publish 後の
  dir 実状態を ledger へ記録（`StagingArea::create` 経路の
  `directory_sync` capability 行）。
- 実 OS power-loss・他 OS FS matrix は NotRun 継続。

## Self-Review

- spec の全 requirement が file map の変更へ対応する。
- signature 維持・fault seam 保全・CreateNew TOCTOU・正直分類・
  回帰不変条件を design 判断として明示した。
- placeholder なし。ruviform 型は公開 API に出さない境界を明記。
