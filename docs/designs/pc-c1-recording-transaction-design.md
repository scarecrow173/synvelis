---
id: "DESIGN-034RoTLCvNiYIgUG0JDqVa"
type: "design"
status: "approved"
title: "PC-C1 Recording Transaction prepare/apply/finalize 設計"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/98"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
  implemented-by: []
  depends-on:
    - "docs/designs/pc-a-parameter-control-design.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/133"
  defers: []
  deferred-by: []
---

# PC-C1 Recording Transaction prepare/apply/finalize 設計

## 1. Transaction のライフサイクルと保持データ(`synvelis-workspace`)

```text
pub struct RecordingTransactionId(pub u64);     // session 内単調/一意

pub enum RecordingTxnState {
    Pending,     // 蓄積中 / commit 可能
    Conflict,    // prepare が fixed != current を検出
    Rejected,    // prepare 構造検証 or apply が拒否
    Committed,   // finish で消費済み(recording からは外れる)
}

pub struct RecordingTransaction {
    pub id: RecordingTransactionId,
    pub started_at: BeatPoint,                          // 開始位置(project beats)
    pub epoch: ClockEpochId,                            // transport epoch context(開始時)
    pub fixed_targets: Vec<ParameterTargetId>,          // armed+write-mode(開始時)
    pub fixed_lanes: BTreeMap<ParameterTargetId, AutomationLaneId>, // 開始時写像
    pub fixed_revision: ProjectRevision,                // 開始時 revision
    pub segments: BTreeMap<AutomationLaneId, Vec<AutomationPoint>>,
    pub sealed: Option<SealReason>,
    pub state: RecordingTxnState,
}
```

`ControlSession` に追加する session state:

```text
next_txn_id: u64                          // 単調 counter(1 から発行)
applied: BTreeMap<RecordingTransactionId, AppliedRecord>
    // AppliedRecord { fingerprint: u64, revision: ProjectRevision }
    // 上限 APPLIED_REGISTRY_CAP = 64、超過時は最小 id から eviction。
last_disposition: Option<RecordDisposition>
    // Committed{id,revision} | Aborted{id} — id を持たない
    // session-level retry を AlreadyApplied / NoTransaction に写像。
```

- `record_start(&mut self, revision) -> Result<RecordingTransactionId,
  Error>`: 固定対象の write-lane 写像を `fixed_lanes` へ snapshot し、
  `epoch`/`id`/`state=Pending` を採番する。既存 gate(armed +
  write-mode + bound lane + record-enabled + rolling)は変更しない。
- `record_points` は `fixed_lanes` を引く(live `write_lanes` ではない)。
  `bind_write_lane` は固定対象への再 bind を `Validation` で拒否する
  (非固定対象は次回 transaction へ効く)。
- `SealReason` に `Stopped`/`Seek`/`ModeChanged`/`ArmChanged`/
  `ProjectChange` を追加。seal は常に「write gate のみ」を閉じ、
  commit/abort/prepare は可能(LoopWrap の既存契約どおり)。
- `RECORD_POINT_BUDGET: usize = 262_144`(全 segment 合計点の NonRT
  budget)。`record_points` は超過する新規書き込みを `Validation`
  で拒否(既録点は失わない)。prepare 側でも防御的に再確認する。

## 2. prepare / finish / rebase

```text
pub struct PreparedRecordingCommit {
    pub id: RecordingTransactionId,
    pub fingerprint: u64,
    pub expected_revision: ProjectRevision,   // == tx.fixed_revision
    pub op: Option<ProjectOp>,                // 空 transaction は None
    pub was_sealed: bool,
}

pub enum RecordCommitError {
    NoTransaction,                                     // open txn なし
    UnknownTransaction { id: RecordingTransactionId }, // id が open/applied に非一致
    Conflict { fixed: ProjectRevision, current: ProjectRevision,
               affected_targets: Vec<ParameterTargetId> },
    Rejected { reason: Error },          // controller apply 側の拒否
    AlreadyApplied { id: RecordingTransactionId, revision: ProjectRevision },
    IntegrityConflict { id: RecordingTransactionId },
    Invalid(String),                     // prepare 側の構造検証失敗
}
```

- `prepare_record_commit(&mut self, project, current_revision)
  -> Result<PreparedRecordingCommit, RecordCommitError>`:
  1. open txn なし → `NoTransaction`。
  2. `tx.fixed_revision != current_revision` → state=Conflict を記録し
     `Conflict{fixed,current,affected_targets: fixed_targets}`。
     無関係 edit も区別しない(既定 Conflict)。
  3. 構造検証: 全 fixed target が project に存在、全 segment lane が
     存在し `lane.target == 記録した fixed target`、各 segment の
     range が構成可能、budget 未超過。失敗は state=Rejected を記録し
     `Invalid`。
  4. snapshot(各 lane の点列を clone)から
     `WriteAutomationRegion{policy: Replace}` per lane を構成し、
     `ProjectOp::Composite` を `op` に入れる。points なしは `None`。
     fingerprint = FNV-1a 64(id/started_at/epoch/fixed_revision/
     fixed_targets/fixed_lanes/segments の canonical byte 列)。
  5. state=Pending のまま prepared を返す(点列は消費しない)。
- `finish_record_commit(&mut self, id, fingerprint, applied_revision)`:
  - open txn.id == id: fingerprint 不一致 → `IntegrityConflict`。
    一致 → txn を取り出し `applied[id] = {fingerprint,
    applied_revision}`、disposition=Committed、`Ok`。
  - open txn なし or id 不一致: applied registry を引き
    fingerprint 一致 → `AlreadyApplied{id,revision}`、不一致 →
    `IntegrityConflict{id}`、未登録 → `UnknownTransaction{id}`。
- `note_commit_rejected(&mut self, id)`: apply 失敗時に open txn の
  state=Rejected を記録する(session orchestrator 用)。
- `rebase_preview(&self, project, current_revision)
  -> Result<RebasePreview, RecordCommitError>`:
  ユーザー明示要求のみ。`lanes: Vec<RebaseLaneReport>` —
  lane/fixed_target の存在、記録点数・記録 range、target の全 lane
  上で記録 range と重なる既存 region 数(Replace が上書きする
  区間の見通し)、fixed/current revision、seal 状態を返す。
- `approve_rebase(&mut self, current_revision)`:
  `tx.fixed_revision = current_revision`、`state = Pending`。
  seal は維持する(write gate は再開しない — commit の再承認のみ)。

## 3. Gate 閉鎖(`synvelis-workspace`)

- `transport_stop` → seal(Stopped)。`transport_seek` → open txn が
  あれば seal(Seek)。`set_mode`/`set_arm` が固定対象に触れたら
  seal(ModeChanged/ArmChanged)。`transport_advance` の loop wrap は
  既存の seal(LoopWrap)。pause は seal しない(write_gates が
  Playing を要求するため paused 中は書き込みが着地しない)。
- `ProjectSession::apply`/`undo`/`redo` 成功時に
  `control.note_project_changed()` → open txn を seal(ProjectChange)。
  `control_record_commit` 内部の apply はこの通知を通さない
  (自身の commit は foreign change ではない)。
  `controller_mut` 直叩き(load 等)は funnel 外として文書化する。
- `write_gates` は従来どおり(armed + record_enabled + Playing +
  unsealed + fixed_targets contains) — seal はすべてこの条件に落ちる。

## 4. Session orchestration(`ProjectSession`)

```text
pub fn control_record_commit(&mut self)
    -> Result<RecordingOutcome, RecordCommitError> {
    let current = self.revision();
    let prepared = self.control.prepare_record_commit(project, current)?;
        // NoTransaction 時: last_disposition == Committed なら
        // AlreadyApplied{id,revision} へ写像、それ以外は NoTransaction。
    match prepared.op {
        None => finish(prepared.id, fp, current)  // 空 commit も registry に載せ
            -> RecordingOutcome{applied:false,..},
        Some(op) => {
            let revision = self.apply_commit(op, prepared.expected_revision)
                // controller.apply + engine rebuild(外部 apply 通知なし)
                .map_err(|e| { note_commit_rejected(id);
                               Rejected{reason: e} })?;
            self.control.finish_record_commit(id, fp, revision)?;
            RecordingOutcome{applied:true, revision:Some(revision),..}
        }
    }
}
```

- `RecordingOutcome` に `id: RecordingTransactionId` を追加。
- `apply_commit`(private)は `controller.apply` + engine rebuild のみ
  — `note_project_changed` は呼ばない(commit 自身の apply が
  transaction を seal する必要はない)。
- 公開 `apply`/`undo`/`redo` は成功後に `note_project_changed` を
  呼ぶ — 外部の全 project 変更が recording gate を閉じる唯一の道。
- `controls_ui::record_commit` の error 型を `RecordCommitError` に
  揃え、`record_rebase_preview`/`record_approve_rebase` を UI 面に
  追加(Inspect/Retry/Explicit Rebase/Abort の headless 対応物)。

## 5. 冪等・整合性の保証水準

- applied registry は session 生存期間のみ有効な in-memory 有限
  窓(64 件)。process crash・session 再構築後の dedupe は保証しない
  — persisted registry が必要になれば #12 schema owner との別契約。
- fingerprint は deterministic かつ payload-canonical: 同じ transaction
  からの prepare は常に同じ fingerprint を返し、別 transaction の
  同一点列は別 id で区別される。
- MultiWindow の duplicate commit は single-writer funnel 上で
  id/fingerprint に解決される(実 GUI 経路は #103/#114 の範囲)。

## 6. 非対象

- persisted commit registry / journal 連携(#12 別契約)。
- Region/Loop semantics(#132/#133)、MultiWindow GUI Oracle(#103)。
- RT callback 内処理 — 全経路は non-RT session 層の構造的不変条件。
