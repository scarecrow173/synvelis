---
id: "DSN-alib07-import-cancel"
type: "design"
status: "approved"
title: "ALIB-07 slice 2 設計 — LIB-IMPORT-04 cancel checkpoint・subprocess crash fixture"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/alib-07-import-cancel.md"
  changes:
    added:
      - type: "file"
        path: "docs/specs/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/designs/alib-07-import-cancel-design.md"
      - type: "file"
        path: "docs/plans/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/tasks/alib-07-import-cancel.md"
      - type: "file"
        path: "docs/impl/ir/alib-07-import-cancel.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/import.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import.rs"
      - type: "file"
        path: "docs/test-specs/alib-07-fault-matrix.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-07-import-cancel.md"
  implemented-by: []
  depends-on:
    - "docs/designs/alib-07-fault-matrix-design.md"
    - "docs/designs/alib-02-streaming-verified-import-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/alib-07-fault-matrix.md"
---

# ALIB-07 slice 2 Design — import cancel checkpoint + crash fixture

## 方針

spec の決定（pre-commit checkpoint 追加）を最小の整合的変更で実装
する。pipeline の stage 順序・manifest 形式・resume 経路は変更し
ない。test 側は既存の scripted provider/publisher 基盤を再利用し、
subprocess crash のみ self-exec 方式で追加する（`Cargo.toml` への
bin 追加は行わない）。

## 変更点

### 1. `src/import.rs` — pre-commit checkpoint（1 行）

`import_verified` の「Live commit」区間、blob 再 stat ブロックの
直後・`commit_op` の直前に `cx.check_interrupts()?;` を追加する。
これが publish tail（seal/publish/dirsync — publisher 内部に
checkpoint を持たない区間）と pre-commit 区間で着地した cancel の
唯一の観測点になる。

- 観測時: `ImportError::Provider(ProviderErrorKind::Cancelled)`、
  manifest は `Published` のまま、blob は manifest 保護下の
  published-but-uncommitted 状態で温存される。
- `controller.apply` 成功後には置かない: commit は既に成立しており、
  `Cancelled` を返すと committed import を失敗と誤報する。残る
  manifest 書込みは local atomic bookkeeping であり、apply〜書込み
  間の crash は `applied_replay` が次回呼出で `Committed` へ
  reconcile する既存設計に委ねる。

### 2. `tests/import.rs` — 3 case

- `cancel_during_publish_is_typed_and_same_import_resumes`:
  `ScriptedPublisher` に `BlobPublishError::Interrupted(
  ProviderError(Cancelled))` を 1 発 script し、publish 相 cancel が
  `ImportError::Provider(Cancelled)`・manifest `Verified` 保持・
  未 commit であることを固定。同一 publisher で retry すると
  queue 消費後に実 publish へ delegate し、commit まで収束する。
- `cancel_at_commit_boundary_parks_at_published_and_resumes`:
  `CancelAfterPublish` publisher（実 `StagedBlobPublisher` へ
  delegate 後、receipt 返却前に `cx.cancel.cancel()`）で、publish
  完了後〜commit 前に着地した cancel を再現。pre-commit checkpoint
  が `Cancelled` を返し、manifest `Published`・blob 実在・
  `applied_imports` 空を assert。`ScriptedPublisher`（calls 計数）
  で retry し、publish が再実行されない（`calls == 0`）ことと
  commit 収束を固定する。
- `subprocess_crash_during_publish_or_commit_resumes_across_processes`:
  self-exec 方式（`std::env::current_exe()` + env guard）で test
  binary を子 process として起動し、2 つの crash point を検証する。
  - `ack-lost`: publisher が実 publish 成功後・receipt 返却前に
    `process::exit(1)` → manifest `Verified` + blob 実在
    （published-unrecorded）。親 process が fresh
    `ProjectController` で resume → stat 照合で `Reused` として
    blob を再利用し commit 収束。
  - `pre-commit`: `stat` 2 回目（publish_blob 内の事前確認=1 回目、
    commit 前 re-stat=2 回目）で `process::exit(1)` → manifest
    `Published` + blob 実在。親が resume → publish を skip して
    commit 収束。
  - 両 case で `ImportManifest` を直接 deserialize し、子 process
    が予約した asset/source ID が親の commit receipt と一致する
    こと（reservation floor・reserved ID の process 間安定性）を
    assert する。

### 3. `docs/test-specs/alib-07-fault-matrix.md` — 簿記更新

- Baseline を現行 SHA（branch `feature/91-alib07-cancel-slice`、
  `origin/main` HEAD）へ更新し、PR #236/#253 への参照と
  cross-link（#84/#97/#114・Ruviform #219/#294・#83）を明記する。
- LIB-IMPORT-04 行を publish/commit 相 Pass へ更新、LIB-IMPORT-08
  行を subprocess crash Pass へ更新（実 durability ではなく
  manifest 駆動 resume の証拠と明記）。
- LIB-MIG-02 行に #86（PR #253）main 済みの参照を追記する。

## 設計上の却下案

- **apply 成功後の checkpoint**: commit 済み import を `Cancelled`
  と誤報する。resume は `applied_replay` で収束するが、第 1 報告が
  不正直になるため却下。
- **`controller.apply` 内への cancel 注入**: `synvelis-project` は
  編集範囲外かつ in-memory atomic op であり、checkpoint を
  持ち込む必然性がない。
- **`[[bin]]`/example による subprocess fixture**: `Cargo.toml`
  変更が必要で編集範囲外。self-exec（current_exe + env guard）は
  test target 内で完結し、Cargo manifest を触らない。
- **in-process のみでの crash 代替**: manifest 駆動 resume は
  既存 test が証明済みだが、process 境界を越えた ID 予約・blob
  store・manifest parse の独立性は subprocess でのみ実測できる。

## 検証

`mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
`test-msrv`。docs は `doc_status.js lint/audit` +
`doc_maintenance.js apply`（index 再生成）。cargo 直接実行は補助
証拠として区別する。
