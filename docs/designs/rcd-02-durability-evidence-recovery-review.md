---
id: "DESIGN-034Urj7kRY8rq2nngtTSuL"
type: "design"
status: "approved"
title: "RCD-02 Durability Evidence / Recovery Review / Commit Receipt"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  implements:
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-01-input-adapter-binding-arm-monitor-fake-port.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 Durability Evidence / Recovery Review / Commit Receipt

## Overview

`crates/synvelis-recording` の staging/writer/journal/recovery/commit を
拡張し、Issue #116 の durability/recovery slice を実装する。
synvelis-project/library への統合は port 境界の設計に留め、他 crate
の公開 API を壊さない（`recording-poc`/`synvelis-render`/
`synvelis-player` は無変更で build が通ること）。

## 状態・権威モデル（既存契約の確認）

- media `sync_data` → Checkpoint record 追記 → journal `sync_data` →
  dir sync（capability 時）の順序は `StreamWriter::checkpoint` が所有。
- durable frontier = intact prefix 内の最後の Checkpoint record
  （`recovery::scan`/`verify_stream` が再検証）。
- commit の冪等 authority = `applied_commits` registry
  （`synvelis-project`）+ `commit-manifest.json`（staging 側の ID
  予約正本）+ `commit.json` marker（journal 側記録）。
- 本スライスはこれらを変更せず、証拠の記録・分類・明示判定を追加
  する。

## 1. DurabilityEvidence

`writer.rs` に配置（`DurableFrontier` と同じ層）:

```rust
pub enum DirSyncState { NotAttempted, Synced, Failed, Unsupported, Unknown }
pub struct DurabilityEvidence {
    pub media_synced: bool,      // media.sync_data 成功
    pub journal_synced: bool,    // journal.sync_data 成功
    pub dir_sync: DirSyncState,
    pub frontier: Option<DurableFrontier>,
    pub fs_capability: StagingCapabilities,
}
```

- `StreamWriter::checkpoint()` は試行ごとに `DurabilityEvidence` を
  `evidence` Vec へ記録し、戻り値を `Result<DurabilityEvidence,
  WriterError>` に変更する（`.unwrap()`/`unwrap_err()`/`if let Err`
  の既存呼出しはソース互換）。sync 失敗時も失敗までの観測値を記録
  してから `Err` を返す。`Finalized` 後の no-op は最後の evidence
  を返す。
- `checkpoint_all` の戻り値型を `Vec<Result<DurabilityEvidence,
  WriterError>>` に合わせる（呼出し側なし）。
- `FinalizedSourceRef` に `#[serde(default)] durability_evidence:
  Vec<DurabilityEvidence>` を追加。writer の `finalize` は自身の
  記録を転記し、`recover_finalize` は採用 frontier について
  `DurabilityEvidence::recovered(...)`（frontier=durable、
  capability=manifest 由来、sync bool は「checkpoint record +
  byte 検証で裏付けられた durable 宣言が観測された」ことを示す
  recovered semantics として doc 明記、dir_sync は capability 写像
  で `Unknown`/`Unsupported`/`Failed`）を 1 件載せる。
- `StreamRecovery`（`scan()`）に `fs_capability: StagingCapabilities`
  を追加（manifest 由来・観測値のみ。sync 成否は scan では再観測
  しない）。

## 2. Recovery Review（`recovery.rs` に集約）

`scan()` は既に `StreamClass`（Healthy/Partial/Corrupted/
Unjournaled）を分類する。これに review 状態を追加する:

```rust
pub enum ReviewState {
    Finalized,      // finalized.json あり（commit 可能・review 不要）
    Pending,        // 未 finalize・判定記録なし → 明示判定が必要
    Adopted, Held, Discarded,   // 判定記録あり
    Unreadable,     // 判定記録が壊れている（fail-closed）
}
```

`StreamRecovery.review_state: ReviewState` として公開。
判定記録は `streams/stream-NNNN.review.json`（`publish_atomic`
で原子公開・capability 時 dir sync）。wire 型 `ReviewRecord`:

```rust
pub struct ReviewRecord {
    schema_version, session_id, stream_index,
    decision: "adopted" | "held" | "discarded",
    // adopt 時の確定参照（冪等再評価・診断用）
    finalized_digest: Option<String>, media_byte_len: Option<u64>,
    source_frames: Option<u64>, outcome: Option<FinalizedOutcome>,
    reason: Option<String>,   // hold/discard の根拠
}
```

判定 API:

```rust
pub enum ReviewAction { Adopt, Hold, Discard }
pub struct DiscardCandidate { session_id, stream_index,
    capture_stream_id, lane_id, media, journal, media_byte_len }
pub enum ReferenceCheck { Clear, Referenced(String), Unknown(String) }
pub trait MediaReferenceProbe {
    fn check(&self, c: &DiscardCandidate) -> ReferenceCheck;
}
pub fn review_stream(area, stream_index, action,
    probe: Option<&dyn MediaReferenceProbe>) -> Result<ReviewOutcome, ReviewError>
pub fn review_stream_injectable(..., faults: &mut PublishFaults) // test 用
```

- `Adopt`: `recover_stream_finalize`（既存 per-stream 実装を
  `pub(crate)` 化して再利用）で verified prefix を finalize し、
  成功後に判定記録を書く。既に finalized なら既存 ref を読み返して
  冪等成功。`Hold`/`Discarded` からの再判定も許可（記録を上書き）。
- `Hold`: 判定記録のみ書く。file は触らない。
- `Discard`: probe 必須。`Clear` のみ実行 — media/journal/
  finalized/canonical を削除してから判定記録を書く（削除→記録の
  順で crash しても未記録=Pending に戻り冪等）。`Referenced`/
  `Unknown`/probe `None` は `ReviewError::Refused` で拒否し何も
  変更しない。
- `MediaReferenceProbe` は synvelis-project/library 接続の port。
  将来の adapter が Project 参照・未保存 Undo/VCS を答える。
  本スライスでは trait 定義と fail-closed 判定のみ。

`recover_finalize`（全 stream 一括）は既存呼出し互換のため残すが、
製品経路は `scan` → review → `review_stream` を正規経路とする
（doc 明記）。`recover_finalize` 自体は明示呼出しであり「自動
finalize」ではない。

### Complete 判定の強化

`recover_stream_finalize` の outcome 決定を強化する:

```rust
let outcome_kind = match fin {
    Some(FinalizeOutcome::Complete)
        if outcome.stop == StopReason::CleanEnd
            && verified.validated_tail_bytes == 0
            && verified.unverified_tail_bytes == 0 =>
        FinalizedOutcome::Complete,
    _ => FinalizedOutcome::Partial,
};
```

Finalize record が Complete を主張しても、intact prefix が
CleanEnd でなく tail が残る場合は Partial に降格する
（unverified tail を Complete にしない受入の直接実装）。

## 3. Commit Receipt（`commit.rs`）

```rust
pub enum ReceiptStage {
    Empty,               // manifest 不在
    Reserved,            // manifest あり・registry 未適用
    CommittedConfirmed,  // registry 適用 + marker 一致
    PublishIndeterminate,// registry 適用 + marker 不在/不一致/未公開
    Conflict,            // identity 一致・内容不一致
    Invalid,             // manifest/marker 破損
}
pub struct StreamFinalizeEvidence {
    stream_index, finalized_present: bool,
    finalize_record: Option<FinalizeOutcome>, journal_stop: StopReason,
}
pub struct CommitReceipt {
    session_id, content_hash: Option<String>, stage: ReceiptStage,
    marker_outcome: Option<String>, committed_revision: Option<u64>,
    project_saved: Option<bool>,       // saved_revision 指定時
    journal_finalize: Vec<StreamFinalizeEvidence>,
}
pub fn commit_receipt(project, area,
    saved_revision: Option<ProjectRevision>) -> Result<CommitReceipt, CommitError>
pub fn reconcile_commit_receipt(controller, area,
    saved_revision: Option<ProjectRevision>) -> Result<CommitReceipt, CommitError>
```

- `commit_receipt` は file を再読込して分類するだけ（mutation なし）。
  marker の `outcome`/`revision` と registry の `commit_status`
  を照合し、`PublishIndeterminate`（registry=AlreadyApplied だが
  marker 不在・破損・identity 不一致）を独立分類する。
  `project_saved` は `saved_revision >= committed_revision` で
  Commit→Save 未完了を区別する。
- `reconcile_commit_receipt` は `PublishIndeterminate` のとき
  `write_commit_marker` を冪等に再実行する（marker 内容は
  manifest+registry から決定的に再構成されるため再書込みは安全）。
  その後 receipt を再計算して返す。`commit_session` 再実行でも
  AlreadyApplied で二重登録されないことを test で証明する。

## 4. CRC32 / content identity の分離

- `crc32.rs` に `IntegrityDigest(pub u64)` newtype を追加
  （`fold`/`to_hex` 等）。`DurableFrontier.digest_fold` を
  `IntegrityDigest` に変更（journal wire の `u64` 表現は不変、
  `CheckpointRecord`/`ChunkRecord`/`FinalizeRecord` の payload
  field は wire 互換のため `u64` のまま doc で integrity 用途を
  明記）。
- `StreamEntry` に `#[serde(default)] digest_scheme:
  DigestScheme` を追加。`DigestScheme::{IntegrityFold,
  ContentDigest}` — canonical receipt 採用時は `ContentDigest`、
  raw fold のままなら `IntegrityFold`。旧 manifest（field 無し）は
  `IntegrityFold` として decode される。
- `payload_hash`/`content_digest`（synvelis-player）は CRC32
  fold ベースの冪等・整合 hash であり content identity ではない
  ことを doc/命名で明記。#86 の SHA-256 系との接続は別 gate。

## 5. Fault injection 拡張（`faultinj.rs`）

- `Inject` に `PermissionDenied { call }`（write_all）と
  `SyncPermissionDenied { call }`（sync_data）を追加し、
  `io::ErrorKind::PermissionDenied` を返す。
- publish 経路用に `PublishInject { Denied{call}, Indeterminate{call},
  SyncFail{call} }` と `PublishFaults`（call 計数）を追加し、
  `publish_atomic_faultable(path, bytes, &mut PublishFaults)` を
  実装。`Denied`=rename 前 PermissionDenied、`SyncFail`=tmp sync
  失敗、`Indeterminate`=rename 成功後に Err を返す（file は公開
  済み・応答喪失相当）。`staging::publish_atomic` は内部で
  空の `PublishFaults` を使う同一実装に委譲する。
- review 判定記録の publish は `review_stream_injectable` 経由で
  `PublishFaults` を注入可能にする。

## File Map

- `src/crc32.rs`: `IntegrityDigest` 追加。
- `src/writer.rs`: `DirSyncState`/`DurabilityEvidence`、checkpoint
  evidence 記録・戻り値変更、`DurableFrontier.digest_fold` 型変更、
  finalize で ref へ evidence 転記。
- `src/recovery.rs`: `ReviewState`/`ReviewRecord`/`ReviewAction`/
  `DiscardCandidate`/`ReferenceCheck`/`MediaReferenceProbe`/
  `ReviewError`/`ReviewOutcome`/`review_stream(_injectable)`、
  `scan()` の review_state/fs_capability、Complete 判定強化、
  `recover_stream_finalize` の `pub(crate)` 化。
- `src/commit.rs`: `DigestScheme`/`StreamEntry.digest_scheme`、
  `CommitReceipt`/`ReceiptStage`/`StreamFinalizeEvidence`/
  `commit_receipt`/`reconcile_commit_receipt`。
- `src/faultinj.rs`: `Inject::PermissionDenied`/`SyncPermissionDenied`、
  `PublishInject`/`PublishFaults`/`publish_atomic_faultable`。
- `src/staging.rs`: `publish_atomic` を faultable 実装へ委譲。
- `src/lib.rs`: export 更新。
- `tests/recovery_review.rs`（新規）: fault-matrix + review/receipt
  冪等検証。
- 既存 `tests/vs_c.rs`/`vs_d.rs` が壊れないこと（Complete 強化の
  影響確認を含む）。

## Risk Register

| risk | 対応 |
| --- | --- |
| checkpoint 戻り値変更で外部呼出し破壊 | `Result<_, WriterError>` の Ok 型のみ変更、呼出しパターン互換を確認 |
| Complete 強化で既存テスト回帰 | vs_c/vs_d/x_e2e の finalize 経路を確認し必要なら期待値を Partial に修正（挙動は仕様通り） |
| review 判定記録の rename 上書き | 既存 record は事前 remove + publish_atomic |
| Discard の参照確認漏れ | probe 必須・Clear 以外 fail-closed・削除は判定記録の後ではなく前（冪等） |
| receipt が journal 全読みで重い | receipt/reconcile は非 RT の回復経路のみで使用 |
| digest scheme 追加で旧 manifest 破損 | `#[serde(default)]` で後方互換 |

## Verification Matrix

| 検証 | 手段 |
| --- | --- |
| fmt/lint/check | `mise run fmt-check` / `lint` / `check` |
| 単体/統合 | `mise run test`（`tests/recovery_review.rs` 追加） |
| RT noalloc | `mise run test-rt`（変更は非 RT 経路のみ） |
| build | `mise run build` |
| 実 OS/FS durability | NotRun（実機 gate） |
