---
id: "IMPL-Sv646Alib05PendingCommit01"
type: "impl"
status: "in-progress"
title: "ALIB-05 residual 2 — pending_commit job の intent 永続化 + commit replay（workspace restore/reinsert API）（synvelis#646）"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/646"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/alib-05-pending-commit-replay.md"
      - type: "file"
        path: "crates/synvelis-app/tests/sv646_pending_commit_replay.rs"
      - type: "file"
        path: "evidence/sv646/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/library_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/library_jobs.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/sv643_jobs_ledger.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/alib-05-jobs-ledger-residual.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/alib-05-jobs-ledger-residual.md"
    - "docs/impl/ir/alib-05-import-jobs.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/alib-05-jobs-ledger-residual.md"
    - "docs/impl/ir/alib-05-import-jobs.md"
    - "docs/test-specs/alib-05-browser-import-ux.md"
    - "evidence/sv646/verification.md"
  refines:
    - "docs/impl/ir/alib-05-jobs-ledger-residual.md"
  refined-by: []
  derives-from:
    - "docs/designs/asset-library-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "crates/synvelis-app/tests/sv646_pending_commit_replay.rs"
    - "crates/synvelis-app/tests/sv643_jobs_ledger.rs"
    - "crates/synvelis-workspace/src/library_ui.rs"
  references:
    - "evidence/sv646/verification.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/91"
    - "https://github.com/scarecrow173/synvelis/issues/90"
  deferred-by: []
---

# ALIB-05 residual 2 — `pending_commit` job の intent 永続化 + commit replay（synvelis#646）

## Summary

PR #645（synvelis#643）が ledger の永続化を導入したとき、
`pending_commit` の job は `recoverable{stage:"commit-pending"}`
として復元し、再起動後の commit replay は intent が wire に
載らないため `import.retry` の再実行に委ねられていた。本 slice は
synvelis-workspace に restore/reinsert API（`LibraryJobRestore`
enum + `LibraryJobLedger::restore_pending_commit`）を追加し、
app 側の `restore_library_jobs` が durable `ImportManifest` から
`ImportIntent` を再構成して entry を live `pending_commit` として
再注入する経路を実装した。再注入された entry は次の
`syn.library.*` op 先頭の `reconcile_jobs` が拾い、
`import_verified` の終端 commit を live controller に replay
して `committed-live`（live revision が動いていれば
`stale-revision`）の typed terminal に落ちる。

## Context

`LibraryJob.intent`（`ImportIntent`）は in-memory 専用で、
`LibraryJobWire` は `pending_commit: bool` だけを持つ。永続化済み
の写しは durable `ImportManifest` が唯一の供給元で、
`staging_dir(<import_id>)/import-manifest.json` に
`import_id`・`expected_project_revision`・`source`・`target`・
`stage`・`format_version` をすべて保持する
（`manifest_path`/`read_manifest` は private。filename の直 join
は rustdoc-documented layout で、test に先例がある）。
`reconcile_jobs` の filter（`pending_commit && terminal.is_none()`
→ `e.intent.clone()`）は既存で、不足していたのは ledger への
再注入 API と intent の再構成だけである。

## Implementation

### Changes

- `crates/synvelis-workspace/src/library_ui.rs`:
  `LibraryJobRestore{Reinserted{token,epoch}, InFlight}` enum と
  `LibraryJobLedger::restore_pending_commit(key, kind, view,
  import_id, intent, progress)` を追加した。entry は live の
  `park_commit` が残す状態（`Running` + stage `commit-pending` +
  `pending_commit:true` + `intent:Some(..)` + `runtime_job:None`
  + `terminal:None`）そのものに建てる — 「worker は再起動で消えた
  が commit を owe している」ことを `jobs_snapshot` が正直に投影
  し、fake の `queued`/`running` にはしない。dedup は `begin` と
  同じ契約（同一 key の non-Done entry は `InFlight` で上書き
  しない）で、epoch と cancel token は `begin` 同様に新規 mint
  する。`mod tests` の `FakeProvider` に `open_pinned` 対応
  （`pinned_supported` + `with_pinned_bytes` + `PINNED_BYTES`
  fixture）を足し、unit test 4 件を追加した。
- `crates/synvelis-workspace/src/lib.rs`: `LibraryJobRestore` を
  re-export に追加。
- `crates/synvelis-app/src/library_jobs.rs`: `restore_library_jobs`
  が `pending_commit && state != Done` の wire について ledger
  lock の前に `pending_commit_intent` で manifest を読み
  （lock 中に I/O しない discipline を守る）、
  `staging_dir(import_id).join("import-manifest.json")` を検証する
  — `import_id` 一致・`format_version == 1`・`stage >= Published`
  のすべてを満たす manifest だけが `ImportIntent` を再構成する。
  `restore_job_entry` は intent が得られた entry を
  `restore_pending_commit` で live 再注入し、記録済みの
  `cancel_requested` は `request_cancel` で replay する。
  intent が得られない entry は sv643 の `recovery_terminal` 経路
  （`recoverable{commit-pending}` 等）にそのまま落ちる。新規定数
  `IMPORT_MANIFEST_NAME` と `IMPORT_MANIFEST_FORMAT`（library の
  private `MANIFEST_FORMAT` の mirror — unknown format は
  fail-closed で degrade）を追加。
- `crates/synvelis-app/tests/sv643_jobs_ledger.rs`:
  `restart_restores_pending_commit_as_recoverable` を
  `restart_replays_pending_commit_to_committed_live` に更新
  — manifest が intent を証明できる限り replay が走る新 semantics
  を固定する（`recoverable` degrade は manifest が証明できない
  場合だけに残る）。
- `crates/synvelis-app/tests/sv646_pending_commit_replay.rs`
  （新規、5 test）: reopen 後の `committed-live` replay・
  `set_ppqn` で live revision を動かした `stale-revision`
  terminal・manifest 欠落/破損での `recoverable` degrade・
  sidecar 上の `cancel_requested` 復元 + replay。

### 設計判断と却下案

- **intent の供給元は durable `ImportManifest`**: 候補は (a)
  durable manifest、(b) sidecar envelope（`LibraryJobsSidecar`）
  への job_key→intent map、(c) `LibraryJobWire` への intent
  field。(a) は intent の全 field が既に永続化済みで serde/
  re-export 済み、`staging_dir` は public — wire 契約も sidecar
  format（`synvelis-library-jobs/1`）も変えずに済み、replay が
  実際に読む authority と同じ値から intent を再構成できる。
  (b) は `ImportIntent` が serde 非対応なため mirror 型が必要で、
  manifest と並走する別の真実を持つと記録と実体がずれうる。
  (c) は `syn.library.jobs` の外部契約を広げる。いずれも (a)
  で賄える不足が確定しなかったため採用しない。
- **replay のトリガは既存 `reconcile_jobs`**: 「明示的再開導線」
  を新規 op/`LibraryAction` にする設計は外部契約の追加になる。
  再注入 entry が `pending_commit && terminal.is_none()` であれば
  `syn.library.view`/`action`/`jobs` の各 op 先頭で走る既存の
  reconcile がそのまま replay するため、op 追加は要らない。
  service thread 単一 writer の構図（`&mut ProjectController`
  を job thread に渡さない）も既存経路を踏襲する。
- **degrade は fail-closed**: manifest 未検出・読み取り不可・
  `import_id` 不一致・`format_version != 1`・`stage < Published`
  の `pending_commit` entry は、sv643 と同じ `recoverable{stage:
  "commit-pending"}`（cancel 記録があれば `cancelled`）に degrade
  し、receipt を捏造しない。kind が `ImportSubmit`/`ImportRetry`
  以外の `pending_commit` wire も replay 対象にしない — import
  lane 以外が commit を owe することは現状の契約上あり得ない。
- **cancel token・epoch・cancel_requested**: epoch と token は
  `begin` と同じく process-local に mint する（旧 token は死んだ
  process のものであり復元しない）。wire の `cancel_requested`
  は `request_cancel` で replay し、flag は entry に残るが
  `reconcile_jobs` は cancel で filter しない — parked commit は
  協調 cancel の最終 checkpoint を過ぎているため、live と同じく
  flag が記録されても commit は replay される。
- **永続化タイミングの制限（sv643 継承・変更しない）**: sidecar
  は `syn.project.save`/`new` 時点の snapshot。最後の save 以降に
  park された job は sidecar に載らず restore 対象外 — durable
  manifest は残るので `import.retry`/`syn.library.imports` の
  既存導線が残る。

## Verification

workspace 側は `library_ui.rs` 内 unit test 4 件で固定:
`restore_pending_commit_projects_commit_owed_state`（wire 投影 +
InFlight dedup）、`restore_pending_commit_cancel_reaches_the_entry`
（`request_cancel` 到達 + flag）、`reconcile_replays_a_reinserted_
pending_commit`（scratch を `Committed` まで駆動した durable
manifest + live controller への replay → `committed-live` +
`applied_imports` 登録 + live revision 前進）、
`reconcile_reinserted_without_ports_lands_unavailable`（import
ports 未 bound でも filter に拾われ `unavailable` terminal）。
app 側は `sv646_pending_commit_replay.rs`（5 test）と更新した
sv643 replay test で、restart 立ち合い（store/session 再構築の
headless 計測 — 実 process kill・実デスクトップ再起動は NotRun）
に全経路を観測する。lane 記録は `evidence/sv646/verification.md`
を参照。

## Deferred / Out of scope

- 実 OS GUI 受入（実 Window・実画面）は synvelis#114 GUI-L2 gate
  領域 — NotRun。
- 実 provider・実 network・実 credential — synvelis#90/#91 領域
  （本 slice の provider は scripted fake）。
- `queued`/`running`/`cancel-requested` で再起動を迎えた job の
  実行 resume — worker thread は再起動を越えられない（sv643 の
  typed-terminal 復元 semantics を維持）。
- 最後の save 以降に park された job の restore — sidecar の
  save 時点 snapshot 契約上の既知制限（上記のとおり manifest
  経由の `import.retry` 導線が残る）。
- `manifest_path`/`read_manifest` の public API 化 —
  `crates/synvelis-library/` は lease 外（別 Worker 占有）。
  documented path の直 join で足りたため申請しなかった。
- docs index（docs 配下の README.md）の更新 — Coordinator が
  統合時に整備する。
