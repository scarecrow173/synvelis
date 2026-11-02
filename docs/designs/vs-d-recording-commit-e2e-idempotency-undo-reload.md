---
id: "DESIGN-sbd87tLPXRrvP2kyru1XD2"
type: "design"
status: "approved"
title: "VS-D Recording Commit / E2E — Idempotent Commit・Undo・Reload 設計"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source: []
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
    - "docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md"
    - "docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md"
    - "docs/designs/project-asset-persistence-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/vs-d-commit-identity-manifest-single-mutation.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/adr/vs-d-commit-identity-manifest-single-mutation.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/16"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/23"
  defers: []
  deferred-by: []
---

# VS-D Recording Commit / E2E — Idempotent Commit・Undo・Reload 設計

## 1. 概要と責務

VS-D は `FinalizedSourceRef`（VS-C の Project 非依存出力）を受け取り、
**単一の `RecordingOp::CommitRecording`** として `ProjectController`
へ適用する commit 境界を実装する。責務は (a) commit 前に Stable ID
と内容を staging manifest に確定・永続化、(b) 全検査 → 全登録の atomic
mutation、(c) Applied Commit Registry による idempotent 判定、
(d) 回復可能 media と committed 状態の分離、(e) 1 session = 1 undo
単位、である。PCM byte は command に乗らない（media は staging file
が所有し、Project は参照と digest のみを保持）。

## 2. データ構造

### 2.1 `RecordingCommitManifest`（staging file）

`commit-manifest.json`（session dir 直下・`finalize` と同じ
tmp→sync→rename で原子公開）:

```json
{
  "schema_version": 1,
  "identity": { "session_id": "rsession-<hex32>", "content_hash": "<hex16>" },
  "payload": {
    "session_id": "rsession-<hex32>",
    "passes": [{ "id": "rpass-<hex32>", "index": 0 }],
    "streams": [{
      "stream_index": 0,
      "capture_stream_id": "cstream-<hex32>",
      "lane_id": "lane-<hex32>",
      "clock_epoch_id": "clock-<hex32>",
      "asset_id": "asset-<hex32>",
      "source_id": "source-<hex32>",
      "digest": "<hex16>", "media": "streams/stream-0000.pcm",
      "media_byte_len": 8192, "source_frames": 1024,
      "sample_rate": 48000, "channels": 2,
      "segments": [{
        "id": "cseg-<hex32>", "pass_id": "rpass-<hex32>",
        "session_start": 0, "session_end": 512,
        "source_start": 0, "source_end": 512,
        "project_in": 0, "project_out": 960,
        "media_offset": 0, "media_len": 4096 }],
      "takes": [{ "id": "take-<hex32>", "pass_id": "rpass-<hex32>",
                  "segment_ids": ["cseg-<hex32>"] }],
      "clips": [{ "id": "clip-<hex32>", "take_id": "take-<hex32>",
                  "segment_id": "cseg-<hex32>",
                  "tick_in": 0, "tick_out": 960, "source_offset": 0 }]
    }]
  }
}
```

- `content_hash` = payload の canonical serialization（JSON・構造体
  field 順固定）への `crc32` fold（u64 → hex16）。identity 比較と
  integrity 検査に使う。manifest 再読込時に payload を再 serialize
  して照合する。
- segment は contiguous span（`session_end == next.session_start &&
  source_end == next.source_start && same pass_index`）を merge した
  論理区間。punch は punch window ごとの segment、loop は pass ごと
  の contiguous run。
- clip は segment 単位（punch のギャップを偽の連続 placement にし
  ない）。最低 `pass_index` の take からのみ生成。

### 2.2 `applied_commits`（Project 永続化）

`RecordingRegistry` に追加（`#[serde(default)]` 後方互換）:

```rust
pub applied_commits: BTreeMap<RecordingSessionId, AppliedRecordingCommit>,

pub struct AppliedRecordingCommit {
    pub content_hash: String,
    pub source_ids: Vec<SourceId>,
    pub take_ids: Vec<TakeId>,
    pub clip_ids: Vec<ClipId>,
}
```

### 2.3 `RecordingOp::CommitRecording`

```rust
CommitRecording {
    manifest: Box<commit::RecordingCommitManifest>,
}
```

`validate(project)`（純粋・全検査）:

1. `content_hash` が payload 再計算と一致 → 不一致は `Validation`。
2. session 存在・`SessionState::Closed`。
3. `applied_commits` に session 無し（同一 identity → 上位の
   `commit_status` が AlreadyApplied を扱う。validate では防御的に
   `Validation`）。
4. 全新規 ID（asset/source/pass/segment/take/clip）が既存 entity・
   applied registry 内 ID と衝突しない。
5. 参照先（capture_stream_id・lane_id・clock_epoch_id・session_id）
   が存在し、stream→session 所有と lane 所属が整合。
6. pass index が session 内一意・既存 pass と重複しない。
7. segment: `source_end <= source_frames`、`session_start < session_end`
   `source_start < source_end`、同 pass 内 session 範囲非重複、
   `project_in < project_out`、media_offset+len <= media_byte_len。
8. take: segment_ids 非空・全てこの stream の segment・pass 整合・
   lane は stream の lane。
9. clip: lane が空（`clip_ids.is_empty()`）であることのみ（
   C-1）。anchor tick 順・segment 整合。
10. asset digest == stream digest、source の content_digest 整合。

`apply(project, ids)`:

- assets → sources（`Verified` → `Committed`）→ passes → segments →
  takes（`Verified`）→ clips を manifest payload 順に登録、
  lane.clip_ids へ clip 追加。
- session state を `Closed → Finalized`。
- `applied_commits[session] = { content_hash, ids }`。
- `ids.reseed(max(ids.peek(), max_used_raw+1))`（watermark 整合 E-1）。

## 3. Commit Driver（`commit` モジュール）

```rust
pub enum CommitStatus { NotApplied, AlreadyApplied, IntegrityConflict }
pub fn commit_status(project, session, content_hash) -> CommitStatus;
pub fn prepare_manifest(area, controller, session_id)
    -> Result<RecordingCommitManifest, CommitError>;
pub fn commit_session(controller, area, session_id)
    -> Result<CommitReport, CommitError>;
```

`commit_session` のプロトコル:

1. staging manifest + 各 stream `finalized.json` を読む
   （`FinalizedSourceRef` 収集）。`Complete` のみ commit 対象、
   `Partial`/missing/faulted は `recoverable` へ。
2. `commit-manifest.json` があれば読込・照合（identity/構造が
   再計算 payload と一致するか）。無ければ plan 構築 → controller
   から ID 発行 → hash → 原子公開。
3. `commit_status`:
   - `AlreadyApplied` → apply せず marker へ。
   - `IntegrityConflict` → report 化して終了（mutation なし）。
   - `NotApplied` → `Command { expected_revision: live_revision,
     op: CommitRecording }` を適用。`StaleRevision` は bounded retry
     （回復可能 conflict）。他 Err は report 化して終了。
4. `commit.json` marker を原子公開（identity・hash・applied flag・
   全 stream digest 記録）。
5. `CommitReport { status, committed_ids, recoverable[] }` を返す。

## 4. Idempotency / Crash Matrix

| 状態 | 判定 | 結果 |
| --- | --- | --- |
| manifest 未作成・commit 前 | NotApplied | plan→manifest→apply |
| manifest 作成済み・apply 前 crash | NotApplied | 同一 manifest を照合して apply（ID 再採番なし） |
| apply 済み・marker 前 crash | AlreadyApplied | apply せず marker のみ公開 |
| marker 済み・retry | AlreadyApplied | 何もしない |
| identity 同・hash 異 | IntegrityConflict | mutation なし・報告 |
| expected_revision 古い | StaleRevision | recoverable・最新 revision で再試行 |

save→reload 経路は `applied_commits` の永続化により同じ 3 分類を
維持する。undo で registry entry が消えた場合の再 commit は、同一
manifest（同一 ID・同一 hash）で正当に再適用できる（`apply` の
reseed が watermark 衝突を防ぐ）。

## 5. Undo/Redo と media 境界

- commit = 1 command = 1 undo snapshot。undo は commit 前 project を
  復元し、staging の `.pcm`/`.journal`/`finalized.json`/
  `commit-manifest.json`/`commit.json` は一切触れない。
- redo は commit 後 snapshot を復元し、同一 Stable ID・source
  mapping・clip placement を戻す。
- media の物理削除は行わない（自動 GC なし）。

## 6. 既存契約への影響

- `synvelis-project`: `RecordingRegistry.applied_commits` 追加
  （serde default・snapshot schema 1 のまま読める）。
  `ProjectController` に `draw_raw_id`（generator から raw u128
  発行・外部 manifest 用）を追加。既存 op/API は不変。
- `synvelis-recording`: `commit.rs` 新規 + `RecordingOp` variant
  追加 + lib re-export。既存 op/RT/writer/recovery 不変。
- `recording-poc`: `vsd` サブコマンド追加（既存出力不変・
  `poc_determinism` の期待値に影響しないよう別出力系で構成）。

## 7. 検証方針（Acceptance Matrix 対応）

- VS-001/002/003/004/005: gate シナリオを staging 経由で finalize
  し、commit 後の Source/Take/Clip/Segment/Placement/digest と
  staging media の bit/範囲一致を `tests/vs_d.rs` で検算。
- VS-006/007/008/013: fault/queue/journal 破壊/epoch 断絶を既存
  faultinj・handoff 契約で誘発し、recoverable 分離と非 commit を
  確認（Partial→Committed 昇格なし）。
- VS-009/010: marker 未記録の crash 再現（in-process + save/reload）
  で AlreadyApplied/再適用を実測。
- VS-011/012: undo/redo/save/reload の参照・Stable ID・placement
  復元と media 非削除を実測。
- VS-014: revision 競合を人工注入し、部分適用なし + retry 成功を
  実測。
- VS-015: vs_b の RT 計測試験を維持し、commit/driver 経路が非 RT
  で完結することをコードレビュー + 試験で確認。

## 8. 制約・非対象

- Partial media の commit、session への追記 commit、media GC、
  Playback/Export 統合は非対象（後続 Issue）。
- 実 Device/実 OS fault/power-loss の保証は capability 記録に留める。
- RT パスの変更は一切行わない。
