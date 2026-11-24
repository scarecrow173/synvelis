---
id: "IMPL-Sv89Alib05ImportJobs01"
type: "impl"
status: "in-progress"
title: "ALIB-05 import jobs — preview.fetch/import.submit/import.retry の CallOutcome::Job 化 + app 所有 syn.library.jobs ledger（synvelis#89 ALIB-UX-H 続 slice）"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/89"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/alib-05-import-jobs.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/library_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/controller.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/common/library.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/library_browser.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/library_native_mount.rs"
      - type: "file"
        path: "docs/test-specs/alib-05-browser-import-ux.md"
      - type: "file"
        path: "evidence/sv89/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv89/verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/asset-library.md"
    - "docs/designs/asset-library-design.md"
  implemented-by: []
  depends-on:
    - "docs/test-specs/alib-05-browser-import-ux.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/alib-05-browser-import-ux.md"
    - "docs/test-specs/asset-library.md"
    - "docs/impl/ir/alib-05-native-mount.md"
    - "evidence/sv89/acceptance-ledger.md"
    - "evidence/sv89/verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/asset-library-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "crates/synvelis-app/tests/library_browser.rs"
    - "crates/synvelis-app/tests/library_native_mount.rs"
  references:
    - "evidence/sv89/acceptance-ledger.md"
    - "evidence/sv89/verification.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/91"
    - "https://github.com/scarecrow173/synvelis/issues/90"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  deferred-by: []
---

# ALIB-05 import jobs — `CallOutcome::Job` 化 + `syn.library.jobs` ledger（synvelis#89）

## Summary

`syn.library.action` の長時間 3 action（`preview.fetch`・
`import.submit`・`import.retry`）を service thread の同期実行から
job 化した。`Service::call` 経路では `CallOutcome::Job` を返して
ruviform が worker thread と `job` lifecycle event を所有し、
直接 `op_library_action` 経路では app が spawn する worker thread
（`DirectJobDrive`）で同じ `run_plan_job` 本体を走らせる。
どちらの経路でも terminal は app 所有の
`syn.library.jobs` ledger（`LibraryViews.jobs`・
`Arc<Mutex<LibraryJobLedger>>`）に着地し、`syn.library.jobs`
op が cross-view の進行・terminal を投影、`import.cancel`
action が共有 cancel token 経由の協調 cancel を提供する。
worker は project に一切書かず、import の commit は
`pending_commit` park → service thread 上の `reconcile_jobs`
replay に委ねる job/commit split を採った。

## Context

synvelis#89（ALIB-05 / ALIB-UX-H 続 slice）。headless slice
（前 slice・`alib-05-browser-import-ux`）では 3 つの長時間
action が `syn.vcs.*` と同じ service-thread 同期 discipline で
動いていたため、in-flight の進行・cancel・typed recovery が
wire に乗らず、`syn.library.jobs` ledger と `import.cancel` は
acceptance ledger 上「`syn.library.jobs` slice の責務」として
明示的に deferred だった。本 slice がそれを回収する。
ruviform 側では `CallOutcome::Job`/`JobSpec`/`JobContext`/
`drain_jobs_routed` の job 契約が既に存在し、本 slice はそれを
`syn.library.action` に適用する。

## Implementation

### Changes

- `crates/synvelis-workspace/src/library_ui.rs`:
  - `LibraryJobLedger`（app 所有・`LibraryViews.jobs` に
    `Arc<Mutex<>>` で同居）: `begin`（key dedup →
    `Spawn{token,epoch}`/`InFlight`）、`request_cancel`
    （`Requested`/`Terminal`/`Unknown`）、`park_commit`・
    `finish`・`abort`・`snapshot`。entry は
    `state(queued/running/cancel-requested/done)`・`stage`・
    `progress`・`cancel` token・`cancel_requested`・
    `pending_commit`・`intent`・`runtime_job`・`projected`・
    `epoch` を保持し、`LibraryJobWire`（`{job_key,kind,view,
    import_id,state,stage,progress,cancel_requested,
    pending_commit,runtime_job,epoch,terminal{kind:…}}`）に
    verbatim 投影する。
  - `LibraryJobPlan`（provider・intent・store/blobs/publisher・
    project snapshot・`live_revision`・`id_seed`・cancel cx・
    ledger を束ねる spawn 単位）と `run_plan_job`/`run_preview_job`/
    `run_import_job` — kind で分岐し preview は chunk 単位で
    cancel token を見る bounded fetch、import は dry-run scratch
    `ProjectController`（`new_at_revision` で live revision 起点・
    `JOB_ID_BAND` の id band を live generator から切り出し）
    上で `import_verified` を走らせ、manifest `Published`+ で
    `pending_commit` に park して ledger に terminal を書く。
  - `LibraryJobTerminal`: `committed-live`（receipt 全部・
    `reused` flag）/`already-applied`/`cancelled{resumable,
    stage}`/`failed{reason,detail}`/`recoverable{reason,stage,
    detail}`/`stale-revision{expected,actual}`/`preview-ready`/
    `preview-plan` — `job_terminal` が `ImportError` を durable
    manifest stage 込みで typed に写像（`Io`/`Corrupt`/
    `PublishFailed`/`JobState` は manifest が残るとき
    `Recoverable`、なければ `Failed`）。
  - `apply_job_terminal`: terminal を spawning view の typed
    surface state に materialize（`CommittedLive`→import surface、
    `preview-ready`→`PreviewUiState::Ready` 等）。provider 由来
    reason は `provider_kind_from_tag`（`kind_tag` の逆写像・
    `LibraryReason` code を除外）で `state.fault` にも着地
    させ、同期経路の `fail_provider` と parity を取る。
  - `LibraryViews::reconcile_jobs`: `pending_commit` で未
    terminal の entry を拾い、intent と `LibraryImportPorts`
    から本物の `ProjectController` で `import_verified` commit
    を replay → `notify_committed_change` 対象を返す。terminal
    の view への materialize（`projected` flag）もここで行う。
  - `LibraryAction::ImportCancel{import_id,job_key}` variant と
    spec panel / `Debug` 拡張。

- `crates/synvelis-app/src/ui_ops.rs`:
  - `LibraryActionEffect`（`Result(Value)` /
    `Job{plan: Box<LibraryJobPlan>,label}`）と
    `library_action_effect` — 全 `syn.library.action` の共有
    dispatch。reconcile 後に job action を `begin` し、
    `InFlight` なら `{status:"in-flight"}`、prepare 失敗は
    ledger `abort` + 同期結果、成功時は plan を組み立てる
    （import arm は `JOB_ID_BAND` 分 live generator を進める）。
  - `op_library_action`: `Job` effect は app thread
    （`sv-lib-job-<key>`・`DirectJobDrive`）を spawn して
    `{ok,view_id,job_key,queued:true}` を即答。
  - `library_action_call`: `Service::call` 経路用の
    `CallOutcome` variant — `Job` effect を
    `CallOutcome::Job(JobSpec)` に包み、closure 内で
    `RuntimeJobDrive`（runtime ctx + 共有 cancel token +
    ledger）を組み立てて同じ `run_plan_job` を呼ぶ。
  - `op_library_jobs`: 先ず `reconcile_jobs`（parked commit を
    先に着地させてから投影 — ledger が未 commit の commit を
    報告しない）して `{jobs:[LibraryJobWire]}` を返す。
    `job_key` filter 対応。
  - `import.cancel` arm: `import_id` → `import:<id>` key 解決
    または `job_key` で `ledger.request_cancel` →
    `{status:cancel-requested|terminal|unknown-job}`。
  - `op_library_view`/`op_library_action` の read 経路も
    先ず `reconcile_jobs` — commit replay と terminal の
    view materialize を read の前に完了させる。

- `crates/synvelis-app/src/service.rs`: `syn.library.jobs` を
  OPS に登録、`syn.library.action` の `call` を
  `library_action_call`（`CallOutcome` 経路）へ route。

- `crates/synvelis-app/src/shell.rs`: `on_tick` で
  `Event::Job` を拾い `library_dirty` を mark — run loop /
  uitest adapter（同一 handle・同一 semantics）が既存の
  dirty→spec 再読み→差分 push 契約で job 由来の変更を push。

- `crates/synvelis-project/src/controller.rs`:
  `ProjectController::new_at_revision(project,revision,generator)`
  — 下記 bug fix。

- `crates/synvelis-app/tests/common/library.rs`: job lane
  fixture — `action_json`（ack verbatim）・`jobs`/`job`
  （`op_library_jobs` read = reconcile 駆動）・`import_key`/
  `preview_key`・`wait_job`（`state=="done"` まで poll）・
  `wait_stage`・`Gate`/`GatedReader`/`GatePublisher`（cancel
  観測する mid-flight parking point）・`FailOncePublisher`/
  `PublishFailureKind`・`app_with_import_gate`・
  `FakeProvider::with_preview_gate`。

- `crates/synvelis-app/tests/library_browser.rs`: 既存 16 test
  を job 化に追随（action 後に `wait_job` で drain）+ 8 job
  lane test 追加で 24/24。`library_native_mount.rs` も drain
  追随（8/8）。

### Decisions During Implementation

- **commit は service thread に残す（job/commit split）**:
  `import_verified` の commit は `&mut ProjectController`
  （`draw_reserved`/`applied_replay`/`apply`）を要し、job
  thread から触れない。`Project`/`ProjectController` を job
  側で clone する道もあったが、CAS・id generator・notify の
  正しさを live に保つため、worker は dry-run scratch
  controller 上で stage/publish/manifest までを完了させ、
  manifest `Published`+ で `pending_commit` に park — 本物の
  commit は `reconcile_jobs` が service thread 上で
  `import_verified` を intent のまま replay する形にした。
  `applied_replay` の冪等 receipt は replay 側がそのまま
  返すため、`reused` flag も live で正確になる。
- **scratch の live revision を真似る（root-cause fix）**:
  `import_stale_revision_rebase_retry_recovers` が
  `stale-revision{expected:1,actual:0}` で落ちた。原因は
  `ProjectController::new` が `live_revision: 0` を
  hardcode し scratch の CAS が常に revision 0 と比較する
  こと — 正しい intent が worker 側で stale 判定になる実 bug。
  `new_at_revision` を追加し `LibraryJobPlan.live_revision` に
  dispatch 時の `controller().live_revision()` を載せて再現。
  `import_verified` は `manifest.expected_project_revision`
  を見、`rebase_import` が manifest の field を書き換える
  ため、rebase→retry は intent でなく manifest 経由で正しく
  解決する。
- **`in-flight` dedup は ledger key で**: `import:<id>` /
  `preview:<view>` key が view またぎで一意 — `begin` が
  `InFlight` を返した resubmit は double-register せず
  `{status:"in-flight"}` を ack。`ImportId` = idempotency
  key の契約を job 粒度でも保つ。
- **terminal の view fault parity**: provider 由来 failure は
  同期経路では `state.fault` に typed 着地していた。
  `apply_job_terminal` が `provider_kind_from_tag` で
  `ProviderErrorKind` tag のみを逆写像（`stale-revision`/
  `io`/`publish-failed` 等の `LibraryReason` code は除外）
  し、job 経路でも同じ typed fault に落とす。
- **cancel token は plan に載せる**: `begin` が返す token を
  `ProviderContext.cancel` に配線し `import.cancel`（app
  側 `ledger.request_cancel`）と runtime cancel bridge
  （`RuntimeJobDrive` が `JobContext` の cancel を共有 flag
  に forward）が同じ token に着地する。`cancel_requested`
  flag は user cancel と commit park を区別する。
- **直接 op 経路の worker は app spawn**: `op_library_action`
  は runtime を持たないため app が thread を spawn し、
  `DirectJobDrive`（progress は ledger のみ、runtime event
  なし）で同じ `run_plan_job` を走らせる。terminal・ledger・
  reconcile の semantics は runtime 経路と同一。

## Validation

`evidence/sv89/verification.md`「ALIB-05 import jobs lane」節に
コマンド・exit code・結果を記録。`cargo clippy --locked
--all-targets -p synvelis-workspace -p synvelis-app -p
synvelis-project -- -D warnings` warning 0、`cargo test
--locked` 同 3 crate 全 suite green（`library_browser` 24/24・
`library_native_mount` 8/8・`library_ui` unit 含む）、
`check-headless-deps`・`check/lint/test-app-uitest`・
`check/lint-app-native`・`prek run --all-files` も全て pass。
`cargo fmt --all` は `os error 206`（Windows コマンド長制限）
で環境的に実行不能のため、全 15 member package 個別の
`cargo fmt -p <pkg> -- --check` で rustfmt 準拠を代替確認
（全 pass）。

## Risks

- `reconcile_jobs` は `op_library_view`/`op_library_action`/
  `syn.library.jobs` の read 経路に埋め込んだ — 将来新しい
  library read op が増えたとき、先に reconcile を呼ばないと
  parked commit・未投影 terminal を取りこぼす。
- `preview.fetch` は job 化しても bytes 自体は unverified
  observation のまま — `PreviewReady` を import 相当の
  evidence に昇格しない契約は変わらない。
- ledger は app in-memory — process 再起動越しの job recovery
  は設計対象外（manifest 側の durable recovery は
  `Recoverable`/`import.retry` が担う）。

## Follow-ups

- `syn.library.jobs` ledger の永続化・cross-session recovery
  — 別 slice 候補。
- `LibraryCache` 経由 preview（ALIB-03/#87 領域）—
  `PreviewReady` は unverified のまま deferred。
- 実 OS GUI 受入（実 Window/IME/focus）— synvelis#114 GUI-L2
  gate。
- Git/network/credential provider — synvelis#90/#91 領域。
- synvelis#86 v5 実データ接合。
