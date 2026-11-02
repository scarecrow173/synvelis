---
id: "ADR-MSyE0rkQSGNdJNwNrTVoCy"
type: "adr"
status: "accepted"
title: "VS-D CommitIdentity・Manifest・Single Mutation・Undo 単位"
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
    - "docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md"
    - "docs/adr/vs-c-journal-checkpoint-sync-fault-finalized.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/16"
    - "https://github.com/scarecrow173/synvelis/issues/17"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision_outcome: "I-1+M-1+S-1+R-1+E-1+P-1+C-1+U-1"
    review_gates:
      research: []
      user: []
      adr: []
---

# VS-D CommitIdentity・Manifest・Single Mutation・Undo 単位

## Context and Problem Statement

VS-D（Synvelis #16）は Finalized Media を Project へ登録する冪等
Commit を実装する。固定すべき構造は (1) `RecordingCommitIdentity`
の構成と重複判定方法、(2) Stable ID を Commit 前にどこへ確保・保存
するか、(3) 単一 Project Mutation の形（op の粒度と検査境界）、
(4) Applied Commit Registry の所有場所、(5) manifest 予約 ID と
`IdGenerator`/`id_watermark` の整合、(6) Partial/Faulted Media の
commit 扱い、(7) 空 Lane 自動 Clip の粒度、(8) Undo/Redo の単位と
media 非削除の保証である。

## Decision Drivers

- #16: 「同一 CommitIdentity+同一 Content は AlreadyApplied、同一
  Identity+異内容は Integrity Conflict、Revision 競合は Recoverable
  Conflict」「AssetId/SourceId/TakeId/ClipId は Commit 前に
  Manifest へ確保・保存しておき、Restart/Retry でも再採番しない」。
- #17 §2/§8: Applied Commit Identity registry は Canonical Schema
  の構成要素。Materialized Media ≠ Project commit。Orphan は保持。
- #16: 「1 Recording Session = 1 Project Undo 単位」「Undo は
  Project 参照を外すが Staging/Finalized Media を直ちに削除しない」。
- 既存 `ProjectController` 契約: `validate` 全検査・`apply` は clone
  への適用で全 or 無・revision 単調・undo は snapshot 復元。
- 既存 Staging: session dir は atomic publish 済み（tmp→sync→
  rename）。`FinalizedSourceRef` は Project 非依存で安定。

## Considered Options

CommitIdentity の構成:

- I-1（採用）: identity = `RecordingSessionId`（1 session = 1
  commit）+ content hash = commit payload の canonical JSON への
  CRC32 fold。registry key は session_id、判定は hash 比較。
- I-2: 独立した `CommitId` 新規 stable ID を session とは別に採番。
- I-3: hash を identity の一部にし、同一 hash のみ同一 commit と
  みなす（異内容検出が型で区別できない）。

Stable ID の確保先:

- M-1（採用）: `commit-manifest.json` を staging session dir へ原子
  公開（tmp→sync→rename）。payload は新規 ID を含む全登録内容を
  持ち、既存ファイルがあれば再計算と照合して再利用する。
- M-2: Project 内に `pending_commits` を持ち、Prepare op が ID を
  採番して保存（commit と 2 つの mutation = 2 undo 単位）。
- M-3: ID を commit apply 内部で採番し、manifest には書かない
  （retry で再採番されるため要件に反する）。

Single Mutation の形:

- S-1（採用）: `RecordingOp::CommitRecording { manifest }` を一つの
  `Operation` とし、`validate` で全参照・ID 衝突・範囲・Clock
  Evidence・session 状態を全検査、`apply` で全 Entity 登録 +
  session Finalized + registry 記録を一括実行。
- S-2: 既存 RegisterX op を順次適用し、失敗時にロールバック（部分
  適用リスクと複数 undo 単位になる）。

Registry の所有場所:

- R-1（採用）: `Project.recording.applied_commits`
  （`BTreeMap<RecordingSessionId, AppliedRecordingCommit>`）。
  snapshot 永続化・undo/redo で自動復元・`#[serde(default)]` で旧
  snapshot 後方互換。
- R-2: staging の commit marker ファイルのみを registry とする
  （project reload 後に判定不能）。

ID 採番と watermark:

- E-1（採用）: manifest 作成時に `ProjectController` 経由で generator
  から raw ID を発行し、`CommitRecording::apply` 内で使用済み最大
  raw ID 以後へ `reseed` する（未 save crash・reload で watermark
  が戻っても再発行・衝突を防ぐ）。
- E-2: content-derived hash ID（generator を迂回。ID 権威を逸脱）。

Partial/Faulted Media:

- P-1（採用）: commit 対象は `FinalizedOutcome::Complete` のみ。
  Partial/未 finalize/faulted stream は `recoverable` リストとして
  report し、Project へ登録しない（Recovered→Committed の自動昇格
  なし）。Commit 対象 0 の session は mutation を発行しない。
- P-2: Partial media も部分 Source として commit（#16 の自動昇格
  禁止に反する）。

自動 Clip:

- C-1（採用）: commit 時点で `clip_ids` が空の lane へ、最低
  pass_index の take の各 segment から 1 Clip を生成
  （`TimeAnchor::Musical([project_in, project_out))` +
  `source_offset = segment.source_start`）。非空 lane は新規 clip
  を作らない（既存 Comp の暗黙置換なし）。manifest 作成後に lane が
  非空化した場合は validate が失敗し、driver は manifest 再構築を
  報告する。
- C-2: take ごとに全 segment を 1 clip に集約（punch のギャップで
  誤 placement）。

Undo 単位:

- U-1（採用）: commit は 1 command = 1 snapshot-undo 単位。undo/redo
  は Project のみで staging media/marker を物理削除しない。
- U-2: prepare+commit の 2 command（2 undo 単位で要件に反する）。

## Decision Outcome

**I-1 + M-1 + S-1 + R-1 + E-1 + P-1 + C-1 + U-1** を採用する。

- I-1: session を identity key とし、内容差分は content hash で検出。
  `commit_status` は `NotApplied | AlreadyApplied | IntegrityConflict`
  の 3 分類を返す。`StaleRevision` は既存の Revision 検査で
  Recoverable Conflict として表れる。
- M-1: staging 上の manifest が唯一の事前永続化 ID 記録。
  Project 未 commit・未 save の crash でも ID 集合は再利用される。
  全 entity 参照は payload 内 tagged ID 文字列で持ち、PCM byte を
  含まない。
- S-1: `validate` は NotFound/WrongOwner/IllegalTransition/
  Validation/AlreadyApplied を返し得る純粋検査。`apply` 成功時のみ
  revision が進み、registry entry・全 entity・session Finalized が
  同時に確定する。
- E-1: controller が `raw` ID を外部向けに発行する唯一の入口を持つ。
  `apply` で `reseed(max(peek, max_used+1))` を行い、watermark 戻り
  による衝突を防止（衝突自体は validate の ID 一意検査が捕捉）。
- P-1: `CommitReport.committed`（登録結果）と `.recoverable`
  （Project 未登録 media）を分離して返す。

## Consequences

- `applied_commits` は `#[serde(default)]` で追加するため、旧 VS-A
  snapshot は新 schema で読める（`deny_unknown_fields` でも unknown
  field 追加は読めないため default 必須）。
- undo で commit 全体が消えた場合、registry entry も消える。同一
  manifest での再 commit は合法（再採番なし・同一 ID で復帰）。
- Partial stream を含む session は complete stream のみ commit する。
  全 stream が失敗した session は commit mutation を発行しない
  （report は残る）。
- commit-manifest.json は staging の一部として Crash 後も残るため、
  Recovery Scan と同じ領域に置かれるが、media と混同しない別 file。

## Notes

- commit marker（`commit.json`）は「project 側に結果があるが
  journal 側が未記録」の判定材料であり、marker が無くても registry
  の AlreadyApplied が二重登録を防ぐ。
- `recording-poc` の crash 再現は、apply 直後に marker 未記録で
  driver を中断し同一 controller で retry する方式（in-process）と、
  save → reload → retry（process 越境相当）の両経路を検証する。
