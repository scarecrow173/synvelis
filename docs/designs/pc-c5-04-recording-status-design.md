---
id: "DESIGN-034WaOyEzrP97e8ooaGm2W"
type: "design"
status: "approved"
title: "PC-C5 slice4 Recording status read model 設計"
created: "2026-11-12"
updated: "2026-11-12"
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
  implements:
    - "docs/specs/pc-c5-04-recording-status.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/pc-c5-04-recording-status.md"
    - "docs/tasks/pc-c5-04-recording-status.md"
    - "docs/test-specs/pc-c5-04-recording-status.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c5-04-recording-status.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice4 Recording status read model 設計

## 概要

`crates/synvelis-workspace/src/recording_status.rs` を新設し、
`ProjectSession` の recording 証拠を UI-C5-04 の 5 表示状態
（`Conflict`/`Prepare`/`Pending`/`Applied`/`Unknown`）へ写像する
pure read fn → DTO を提供する。`relation_ui.rs`（`relation_rows` /
`relation_rows_with`）と同じく read-side only で、project・controller・
undo・session 状態を変更しない。domain 側（`control.rs`/`session.rs`）は
一切変更しない — 本 slice は消費のみ。

## 詳細設計

### 1. 公開 API

```rust
/// 既定の notice tail で `recording_status_with` を呼ぶ。
pub fn recording_status(session: &ProjectSession) -> RecordingStatusView;

/// caller 保持の prepared payload と表示 bounds を明示する版。
pub fn recording_status_with(
    session: &ProjectSession,
    prepared: Option<&PreparedRecordingCommit>,
    options: RecordingStatusOptions,
) -> RecordingStatusView;

pub const RECORDING_STATUS_NOTICE_TAIL: usize = 8;

pub struct RecordingStatusOptions {
    /// 末尾から採用する record notification 件数。
    pub notice_tail: usize,
}
```

`prepared` は caller（app/ControlsUi host）が `prepare_record_commit` で
得て保持している payload への参照。session はこれを観測できないため、
read model は「session が見ない証拠」を引数として明示的に受け取る。
`None` なら `Prepare` 状態は原理的に到達不能 — 推測で補わない。

### 2. 表示状態の写像（`RecordingDisplayState`）

証拠の優先順位は以下の通り（open transaction の有無で二分）。

| 条件 | 表示状態 |
| --- | --- |
| open txn なし + `last_disposition == Committed` | `Applied` |
| open txn なし + それ以外（idle / `Aborted` / 証拠なし） | `Unknown` |
| open txn あり + drift（`tx.state == Conflict` ∨ `fixed_revision != live`） | `Conflict` |
| open txn あり + drift なし + prepared verdict `Live` | `Prepare` |
| open txn あり + 上記以外 | `Pending` |

**Conflict の drift 判定**: `tx.state == RecordingTxnState::Conflict`
（prepare が実際に conflict を報告した後）だけでなく、
`tx.fixed_revision != session.revision()` も Conflict とする。
後者は `prepare_record_commit` が最初に行う検査そのものであり、
revision は単調増加で同じ値に戻らないため「次の commit が conflict する」
ことが既に決定している — 推測ではなく同じ証拠による早期表示。
`approve_rebase` は fixed_revision を live に再 pin し state を Pending に
戻すので、rebase 後は自然に Pending へ戻る。

**Prepare の写像定義（本 slice の設計判断）**: domain に永続 "Prepared"
txn state は存在しない — `PreparedRecordingCommit` は caller 保持の
snapshot（`control.rs` の `PreparedRecordingCommit`）であり、
session は「prepared が存在する」ことを一切観測できない。
「prepare 実行中」という中間状態も存在しない（prepare は
`control_record_commit`/`prepare_record_commit` 内で同期実行され、
失敗しても `PreparedRecordingCommit` は生成されない）。
したがって `Prepare` は **awaiting-apply** — caller が保持する prepared
payload が `control_record_finish` を待っている状態 — と定義し、
read fn が `Option<&PreparedRecordingCommit>` を明示入力として受け、
`ControlSession::preflight_record_finish`（`&self`・副作用なし）で
session 証拠に対して検証する。caller の主張は無検証で信用しない。

**verdict**（`PreparedVerdict`）:

- `Live`: preflight が `Ok` かつ `expected_revision == live`
  → `control_record_finish` が適用し得る。
- `StaleRevision { expected, current }`: preflight は `Ok` だが
  `expected_revision != live`（prepare 後に rebase/foreign 変更が入った）
  → finish は CAS で reject される。
- `Rejected(RecordCommitError)`: preflight が型付きエラーを返した
  （`AlreadyApplied`/`IntegrityConflict`/`UnknownTransaction`）
  → payload は finish に使えない。エラーを verbatim に保持。

`Pending` は残り全て: 蓄積中・seal 済み（write gate 閉鎖のみ、
commit/abort は合法）・`RecordingTxnState::Rejected`（構造 reject、
列は保持）。`Rejected` を独立した表示状態にしないのは、UI-C5-04 の
5 状態が仕様で固定されており、かつ Rejected txn は project 修復 +
retry で回復し得る「open transaction の一状態」であるため —
verbatim `RecordingTxnState` は provenance が運ぶ。

### 3. DTO 構成

```rust
pub struct RecordingStatusView {
    pub state: RecordingDisplayState,
    /// view を計算した live revision — `is_current(session)` で staleness 判定。
    pub revision: ProjectRevision,
    pub record_enabled: bool,
    pub transport: PlaybackStatus,
    pub transaction: Option<RecordingTxnView>,
    pub prepared: Option<PreparedCommitView>,
    pub outcome: Option<RecordingOutcomeView>,
    /// bounded tail（newest last）の record notification。
    pub notices: Vec<RecordNotification>,
    /// log 溢れで捨てられた notification 数。
    pub notices_dropped: u64,
    /// 状態・証拠・action の説明行。各行は typed provenance を持つ。
    pub explanations: Vec<RecordingExplanation>,
    /// `state` で合法な action（決定的順序）。
    pub actions: Vec<RecordingAction>,
}
```

- `RecordingTxnView`: id・`RecordingTxnState` verbatim・`SealReason`・
  fixed/current revision・`revision_stale`・started_at・`LoopPassId`・
  fixed targets・lane 行・合計記録点数・`record_dropped`/
  `deliveries_dropped`/`record_gaps_merged`。
- `RecordingLaneView`: `fixed_lanes` の各 (target, lane) について、
  `segments`/`record_gaps`/`deliveries` のいずれかに実データがある lane
  のみ投影（`rebase_preview` と同じ join）。target/lane の live 存在、
  記録点数、`(first,last)` 範囲、`RecordGap` 列、配送 interval 数。
- `RecordingOutcomeView`: `RecordDisposition` verbatim +
  committed かつ revision ありのとき `applied_record(id)` の receipt。
- `PreparedCommitView`: id・fingerprint・expected_revision・was_sealed・
  `op.is_some()`（書込みが発生するか）・`PreparedVerdict`。
- `RecordingExplanation { text: String, provenance: RecordingProvenance }`:
  text は英語の表示用文字列（gateway_ui の `notes` と同じ方針）、
  provenance は型付き証拠。

### 4. `RecordingProvenance`（typed provenance）

```rust
pub enum RecordingProvenance {
    /// open transaction 自身のフィールド（verbatim）。
    Transaction { id, state: RecordingTxnState, sealed: Option<SealReason> },
    /// fixed/live revision の比較 — prepare が行う検査と同じ証拠。
    RevisionDrift { fixed: ProjectRevision, current: ProjectRevision },
    /// caller 保持 prepared payload とその検証 verdict。
    Prepared { id, expected_revision, was_sealed, verdict: PreparedVerdict },
    /// 最後の lifecycle disposition（commit/abort）。
    Disposition(RecordDisposition),
    /// applied registry の receipt。
    AppliedReceipt { id: RecordingTransactionId, receipt: AppliedRecord },
    /// bounded log の 1 entry（kind は variant が保持）。
    Notification(RecordNotification),
    /// 対象 target の record health counter。
    Health(RecordHealthSummary),
    /// session gate（record enabled + transport status）。
    Gates { record_enabled: bool, status: PlaybackStatus },
}
```

`RecordCommitError`/`RecordNotification`/`RecordDisposition`/
`AppliedRecord`/`RecordHealthSummary`/`SealReason` は全て
Clone + PartialEq で、clone して view に所有させる
（`RelationView` が reply を所有 DTO に写すのと同じ）。

### 5. 合法 action（`RecordingAction`）

```rust
pub enum RecordingAction {
    /// `control_record_rebase_preview` — rebase が書く内容の inspect。
    Inspect,
    /// 冪等 commit 経路 — prepared 無しは `control_record_commit`、
    /// live prepared 有りは `control_record_finish`。
    /// `AlreadyApplied`/`Conflict` は型付き応答であり二重書込みしない。
    Retry,
    /// `control_record_approve_rebase` — Inspect 後の明示 rebase 承認。
    ExplicitRebase,
    /// `control_record_abort` — open transaction を破棄。
    Abort,
}
```

| state | actions |
| --- | --- |
| Conflict | Inspect, ExplicitRebase, Abort |
| Prepare | Inspect, Retry, Abort |
| Pending | Inspect, Retry, Abort |
| Applied | （なし） |
| Unknown | （なし） |

- Conflict で `Retry` を出さない: revision drift が残る限り
  `control_record_commit` は決定的に `Conflict` を返すだけで進まない。
  `ExplicitRebase` の前に `Inspect`（rebase_preview）が必須なのは
  domain 側の「never an automatic merge」契約と一致。
- Prepare/Pending の `Retry` は「commit を試みる」意味で統一 —
  初回 commit も rejected txn の retry も同じ冪等経路を通る。
- `Inspect` は open transaction がある全状態で合法
  （`rebase_preview` は `NoTransaction` 以外で動く）。

### 6. explanation 行の構成

`explanations` は以下の順で構築（上限は notice_tail + lane 数 + 固定行）:

1. state headline（provenance は状態の主証拠: Conflict→`RevisionDrift`、
   Prepare→`Prepared`、Applied→`Disposition`、Unknown→`Disposition` or `Gates`）。
2. transaction summary（`Transaction`）— open txn があるとき。
3. seal 行（`Transaction`）— `sealed.is_some()` のとき。
4. lane 行（`Transaction`）— lane ごと。
5. prepared verdict 行（`Prepared`）— caller が prepared を渡したとき
   （Prepare 以外の状態でも stale/foreign を報告するため必ず出す）。
6. outcome 行（`Disposition`）+ receipt 行（`AppliedReceipt`）。
7. notice 行（`Notification`）— bounded tail の各 entry。
8. health 行（`Health`）— open txn の fixed target ごとに
   `record_health(target)` が `Some` のとき。

health は `record_health(target)` の per-target lookup のみ公開のため、
open txn の `fixed_targets` に限定する（health table 全件列挙は
session API に無く、本 slice では追加しない — spec の Deferred 参照）。

### 7. ファイル変更一覧

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-workspace/src/recording_status.rs` | 新規: DTO + `recording_status`/`recording_status_with` |
| `crates/synvelis-workspace/src/lib.rs` | `pub mod recording_status;` + re-export（小変更） |
| `crates/synvelis-workspace/tests/uic5_04.rs` | 新規 fixture（8〜12 test） |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/pc-c5-04-recording-status*.md` | 本チェーン |

`control.rs`/`session.rs`/`controls_ui.rs`/`state.rs`/`snap.rs`/
`synvelis-app/**`/`synvelis-project/**`/`synvelis-recording/**`/
`synvelis-player/**` は変更しない。

## リスクと対策

- **predictive Conflict の誤表示**: `fixed != live` は prepare と同じ
  検査なので誤りではないが、「ユーザーが rebase するつもりでも
  Conflict と出る」ことを spec/impl-ir に明記し、provenance で
  `tx.state` verbatim（Pending のままか Conflict 報告済みか）を
  区別可能にする。
- **prepared payload の stale 判定漏れ**: `preflight_record_finish` は
  id+fingerprint のみ検査し revision CAS は apply 時 — `expected_revision
  == live` を別途確認して `StaleRevision` を出す。approve_rebase は
  fingerprint（fixed_revision を含む）を変えるため IntegrityConflict
  としても捕捉される — 二重に安全。
- **`RecordingTxnState::Committed` が観測不能**: `finish_record_commit`
  は txn を `take()` するため `recording()` は Committed を返さない。
  Applied は `last_disposition`/`applied_record` で判定し、
  `RecordingTxnState::Committed` variant には依存しない。
- **notification log の肥大**: `RECORD_NOTIFY_CAP=256` まで育つため
  `notice_tail`（既定 8）で bounded にし、溢れは `notices_dropped` で報告。
