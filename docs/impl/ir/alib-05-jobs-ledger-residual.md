---
id: "IMPL-Sv643Alib05JobsLedger01"
type: "impl"
status: "in-progress"
title: "ALIB-05 residual — syn.library.jobs ledger の永続化/recovery・syn.library.imports op 化・generic job.cancel wire 到達（synvelis#643）"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/643"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/alib-05-jobs-ledger-residual.md"
      - type: "file"
        path: "crates/synvelis-app/src/library_jobs.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/sv643_jobs_ledger.rs"
      - type: "file"
        path: "evidence/sv643/verification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/store_publish.rs"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/alib-05-import-jobs.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/alib-05-import-jobs.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/alib-05-import-jobs.md"
    - "docs/test-specs/alib-05-browser-import-ux.md"
    - "evidence/sv643/verification.md"
  refines:
    - "docs/impl/ir/alib-05-import-jobs.md"
  refined-by: []
  derives-from:
    - "docs/designs/asset-library-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "crates/synvelis-app/tests/sv643_jobs_ledger.rs"
    - "crates/synvelis-project/tests/store_publish.rs"
  references:
    - "evidence/sv643/verification.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/91"
    - "https://github.com/scarecrow173/synvelis/issues/90"
  deferred-by: []
---

# ALIB-05 residual — `syn.library.jobs` ledger 永続化・`syn.library.imports`・generic `job.cancel`（synvelis#643）

## Summary

PR #642 で job 化した `syn.library.jobs` ledger は process 内の
in-memory 投影であり、再起動で job の状態と receipt が消えていた。
本 slice は ledger の wire 投影（`LibraryJobWire`）を project bundle の
sidecar `library-jobs.json` へ `syn.project.save`/`new` で書き、
`syn.project.open` で復元する永続化を加えた。worker thread は再起動を
越えられないため、in-flight だった job は実行再開ではなく typed
terminal（durable manifest が残る import は `recoverable`、記録済みの
cancel は `cancelled`、それ以外は `failed`、理由は `interrupted`）と
して復元し、`done` の receipt は verbatim に保つ。
併せて `synvelis_library::inspect_imports` を `syn.library.imports`
op として job 系 op と同じ wire/ledger 経路に載せ、ruviform 汎用
`job.cancel` が `CallOutcome::Job` で起こした import/preview job の
協調 cancel token へ到達することを headless に計測した。

## Context

Issue #643 は PR #642（ALIB-05 job 化）の residual として、
ledger 永続化/recovery・`inspect_imports` の op 化・generic
`job.cancel` 到達の 3 点を要求する。ledger 本体
（`LibraryJobLedger`/`LibraryViews`/`reconcile_jobs`）は
synvelis-workspace の製品 View 契約で本 slice の lease 外であり、
restore/reinsert API は存在しない。永続化経路
（`ProjectStore`/`DirectoryBundleStore`/`InMemoryStore`）は
synvelis-project で lease 内。ruviform の `job.cancel` は pin 依存で
変更不可、`{job: <runtime job id>}` を取る wire 契約もそのまま使う。

## Implementation

### Changes

- `crates/synvelis-project/src/store.rs`: `ProjectStore` に
  `save_sidecar(name, bytes)`/`read_sidecar(name)` を追加した。
  snapshot（`snapshots/r*.json`・`CreateNew`）とは別の「bundle 直下の
  mutable document」面を用意するための API で、既定実装は
  `Error::Store`/`Ok(None)` — sidecar を実装しない store が存在を
  偽らない。`check_sidecar_name` は区切り文字・`..`・`HEAD`/
  `snapshots`/`HEAD.lock*` を拒否し、sidecar が store 自身の
  レイアウト（HEAD・snapshots ツリー）と衝突しないことを保証する。
  `DirectoryBundleStore` は `publish_staged` を `PublishMode::Replace`
  + `Durability::FileAndDirectorySync` で再利用する — snapshot の
  不可変性とは対照的に、ledger は「最新の状態」の写像なので上書きが
  正しい semantics である。`InMemoryStore` は `sidecars: BTreeMap`
  で同契約を提供する。
- `crates/synvelis-app/src/library_jobs.rs`（新規）: sidecar の
  envelope `LibraryJobsSidecar {format, project_id, jobs}` —
  `jobs` は `jobs_snapshot()` の verbatim、`format` は
  `synvelis-library-jobs/1`、`project_id` は bundle が持つ project の
  tagged id。`persist_library_jobs` は `syn.project.new`（workspace
  swap 前 — 再利用された bundle dir が旧 project の job 行を残さない
  ため）と `syn.project.save`（`controller.save` 後）で呼ぶ。
  `restore_library_jobs` は `project_open_inner` の末尾で読む。
  corrupt/unknown format は `Store` error にし、receipt を静かに
  落とさない。`project_id` 不一致は「復元しない」へ degrade する
  （bundle dir の再利用はあり得るため error にしない）。
  復元は `begin`（新規 queued entry）→ `report` → `request_cancel`
  → `finish` の公開 API 合成で行い、`done` entry は terminal を
  verbatim に、`done` 以外は `recovery_terminal` の typed terminal
  で閉じる。`pending_commit` は `recoverable{stage:"commit-pending"}`
  にし、import で durable manifest が残るものは `recoverable{stage:
  <manifest の ux stage>}`、記録済み cancel は
  `cancelled{resumable:<manifest 有無>}`、それ以外は
  `failed{reason:"interrupted"}` にする。`runtime_job`・
  `pending_commit`・`intent`・cancel token は再注入できないため、
  復元された entry の `pending_commit` は常に `false` — replay
  intent は永続化しない、という判断を wire に正直に残す。
- `crates/synvelis-app/src/app.rs`: `persist_library_jobs` を
  `project_new_inner`/`project_save_inner` に、`restore_library_jobs`
  を `project_open_inner`（`bind_library_backend` 後）に配線した。
- `crates/synvelis-app/src/ui_ops.rs` + `service.rs`:
  `syn.library.imports` op を追加。`OP` 登録・dispatch arm は
  `syn.library.jobs`/`syn.library.action` と同じ経路で、本体は
  `with_library` 内で `reconcile_jobs` → `inspect_imports(&store,
  live)` を実行し、各行に `import_id`/`stage`/`content_digest`/
  `byte_len`/`expected_project_revision`/`applied`/
  `publish_indeterminate` に加えて ledger 相関の `job_key`/`job`
  （`LibraryJobWire`、無ければ null）を付けて返す。import services
  未 bound なら `unavailable` — 「空の inspection」を「import 無し」
  に見せない。

### 設計判断と却下案

- **置き場所は bundle sidecar**: 検討した候補は (a) snapshot
  envelope への混入、(b) app 固有の別領域、(c) bundle 直下の
  sidecar。(a) は snapshot の immutability と format 契約を侵し、
  ledger だけが変わる度に snapshot を増やす設計になり不整合、(b)
  は project open/save と寿命が一致せず、project を移したとき
  ledger が残る/消える制御が利かない。(c) は store 経路（lease
  内）で完結し、`save` の HEAD と同じ publish 保証の下で書ける
  ため採用した。
- **保存タイミングは save/new のみ**: ledger 変化ごとの即時
  flush は service thread の応答性と lock discipline（lock 中に
  I/O をしない）に反するため採用しない。save した時点の ledger と
  reopen 後の投影が一致する、という不変条件を契約にする。crash
  直前の save 以降の job は復元されない — これは semantics として
  明示した制限で、receipt を捏造しない限界でもある。
- **in-flight は resume せず typed terminal**: worker thread は
  process 再起動を越えられない。`queued`/`running` のまま復元する
  見せかけの in-flight は「進行中だが誰も進めていない」嘘になる。
  そのため、起動時に `state != done` の entry は全て terminal に
  畳む。`pending_commit` の replay 再開は intent（`ImportIntent`）
  が `LibraryJobWire` に載っていないため ledger の公開 API では
  作れない — workspace 側に intent 永続化+replay API を足す
  選択肢はあったが、lease 外の製品 View 契約を変える変更は本
  slice の判断枠を超えるため、`recoverable` + `import.retry` の
  再実行導線（既存 wire）で AC を満たす方を選んだ。
- **`runtime_job` は復元しない**: runtime job id は process-local
  であり、再起動後に同じ id を持つ job は存在しない。復元 entry
  の `runtime_job` は `null` のまま — 外部が `job.cancel` に渡す
  値は live job のみが持つ。
- **`inspect_imports` の「記録」の解釈**: op の応答に ledger 相関
  （`job_key`/`job`）を乗せる形にした。ledger 自体へ inspect
  結果を書き戻す設計は、read op が project authority へ副作用を
  持つ形になり、`jobs` と `imports` の責務を混ぜるため採らない。

## Verification

`crates/synvelis-app/tests/sv643_jobs_ledger.rs`（10 test）と
`crates/synvelis-project/tests/store_publish.rs`（sidecar 2 test）
で headless に証明した。restart は test 内で store/session を再構築
する headless 計測（実 process kill・実デスクトップ再起動は
synvelis#114 gate 領域として NotRun）。`job.cancel` 計測は
`UiRuntime::dispatch_op` + `poll` の wire seam 上で行い、
`runtime_job` field の相関・`{cancelling:true}` ack・
`job_cancelled` event・ledger `cancelled` terminal の 4 点を観測
する。詳細な lane 記録は `evidence/sv643/verification.md` を参照。

## Deferred / Out of scope

- 実 OS GUI 受入（実 Window・実画面の動作）は synvelis#114
  GUI-L2 gate 領域 — NotRun。
- 実 provider・実 network・実 credential の import/preview は
  synvelis#90/#91 領域 — 本 slice の provider は scripted fake。
- `pending_commit` job の commit replay 完全復元（intent 永続化
  + `reconcile_jobs` replay 再開）は `synvelis-workspace` 側の
  restore/reinsert API を要し、本 slice は `recoverable` +
  `import.retry` で代替。Issue 側の「決裁依頼」に記録。
