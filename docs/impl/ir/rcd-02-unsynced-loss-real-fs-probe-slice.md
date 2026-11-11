---
id: "IMPL-0h9qRk2mNvPwXzYc8uJd4s"
type: "impl"
status: "completed"
title: "RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added:
      - type: "file"
        path: "crates/recording-poc/tests/real_fs_probe.rs"
      - type: "file"
        path: "docs/specs/rcd-02-unsynced-loss-real-fs-probe.md"
      - type: "file"
        path: "docs/designs/rcd-02-unsynced-loss-real-fs-probe-design.md"
      - type: "file"
        path: "docs/plans/rcd-02-unsynced-loss-real-fs-probe-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-02-task-unsynced-loss-real-fs-probe.md"
      - type: "file"
        path: "docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md"
      - type: "file"
        path: "docs/impl/ir/rcd-02-unsynced-loss-real-fs-probe-slice.md"
    modified:
      - type: "file"
        path: "crates/recording-poc/src/main.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/recovery_review.rs"
      - type: "file"
        path: "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/rcd-02-task-unsynced-loss-real-fs-probe.md"
    - "docs/specs/rcd-02-unsynced-loss-real-fs-probe.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/rcd-02-task3-stagedfile-publish-migration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-unsynced-loss-real-fs-probe-design.md"
    - "docs/plans/rcd-02-unsynced-loss-real-fs-probe-plan.md"
    - "docs/impl/ir/rcd-02-stagedfile-publish-migration.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-02-unsynced-loss-real-fs-probe-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md"
  verified-by:
    - "docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe

## Summary

`StreamWriter` の unsynced-write 喪失（`write_all` 成功後・最終
sync frontier 以降に sync されず失われる byte）を、media/journal
file を最終 checkpoint 時点の実長へ `set_len` truncate する
headless fixture として決定的に再現した。対称喪失（両 file を
frontier へ truncate → `StreamClass::Healthy` + verified prefix
のみ Adopt）と非対称喪失（media tail 残存+journal tail 喪失 →
`unverified_tail_bytes`、journal tail 残存+media tail 喪失 →
"media shorter than journaled coverage" fault）の 2 test を追加し、
いずれも `review_stream(Adopt)` が verified/物理 prefix のみの
`FinalizedOutcome::Partial` で Complete に昇格しないことを検証した。

併せて `recording-poc realfs` subcommand と
`tests/real_fs_probe.rs`（`#[ignore]`、opt-in lane
`mise run test-recording-real-fs`）を新設し、本機
Windows/NTFS 上で rename 原子性・open-handle rename・
`create_new` no-replace・dir sync capability（実測
`Unsupported`）・StagedFile publish observable semantics・
unsynced byte の post-process-kill 実長を実測して ledger
`RCD-02-REALFS` 節へ記録した。真の power-loss は非破壊的に
自動化不能であり process kill が dirty page を保持することを
実測で裏付けたため、`NotRun` のまま維持する。

## Context

- Task: `docs/tasks/rcd-02-task-unsynced-loss-real-fs-probe.md`
- Spec: `docs/specs/rcd-02-unsynced-loss-real-fs-probe.md`
- Design: `docs/designs/rcd-02-unsynced-loss-real-fs-probe-design.md`
- Plan: `docs/plans/rcd-02-unsynced-loss-real-fs-probe-plan.md`
- Test-spec: `docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md`
- Issue: <https://github.com/scarecrow173/synvelis/issues/116>
  （task1–3 統合済み残滓: unsynced-write 喪失 fixture +
  real-FS 実測 lane）

## Implementation

### Changes

- `crates/synvelis-recording/tests/recovery_review.rs`:
  - `truncate_to(path, len)` helper を追加 — power-loss-like な
    dirty byte 喪失を「最終 sync 観測時点の実長への `set_len`」で
    再現する fixture primitive。writer は userspace buffer を
    持たず `File` へ直接 write するため、file 長は write 成功
    byte の忠実な写像になる。
  - `rcd02_unsynced_loss_truncates_to_sync_frontier`: checkpoint
    （3 chunk sync 済み）後に 2 chunk を追加 write して drop、
    media/journal を checkpoint 時点の実長へ truncate → replay
    `CleanEnd` + `StreamClass::Healthy`、tail byte 0、durable
    media len = frontier。`review_stream(Adopt)` →
    `FinalizedOutcome::Partial`（3 span / `source_frames=192`
    のみ、喪失 chunk を含まない）。
  - `rcd02_unsynced_loss_asymmetric_tail`: (a) journal tail のみ
    truncate → `StreamClass::Partial` + `unverified_tail_bytes`
    = 2 chunk 分（journal coverage 外の media tail）、Adopt は
    verified prefix のみ Partial・物理 file は unverified byte
    を保持したまま（自動削除しない）。(b) media tail のみ
    truncate → `StreamClass::Partial` + `validated_tail_bytes`
    = 2 chunk 分 + fault "media shorter than journaled
    coverage"、Adopt は物理 media prefix のみ Partial。
- `crates/recording-poc/src/main.rs`: `realfs` subcommand を追加。
  - `realfs probe <dir>`: rename（missing dest / replace /
    open-handle src / open-handle dst / `create_new` no-replace）、
    dir open/sync capability + `detect_dir_sync`、StagedFile
    publish（replace / create_new / conflict 保持 / temp
    residue=0）、unsynced close 実長・`set_len` truncate を
    決定的順の `key=value` で出力（err も観測値として印字し
    偽装しない）。
  - `realfs write-unsynced <file> <synced> <unsynced>`: synced
    byte（0xA5）write → `sync_data` → unsynced byte（0x5A）
    write → `ready` を stdout へ flush → stdin 待機。test が
    ready 検出後に kill する協定点。
- `crates/recording-poc/tests/real_fs_probe.rs`（新規、`#[ignore]`
  ×2）: `realfs_probe_fs_capabilities`（環境非依存の不変条件 +
  Windows では dir sync `Unsupported` pin）、
  `realfs_unsynced_tail_survives_process_kill`（post-kill 実長 =
  synced+unsynced、prefix/tail pattern 検証 — process kill が
  dirty page を失わせないことの実測）。
- `mise.toml`: opt-in manual task `test-recording-real-fs` を 1
  件追加（既存 `test-recording-*` 系と同型、CI 対象外）。
- `evidence/reports/rcd-02-durability-recovery-review-ledger.md`:
  fault matrix に unsynced 喪失 2 行、新節 `RCD-02-REALFS` に
  実測 9 行（power-loss 行は `NotRun`）、out-of-scope を真の
  power-loss/他 OS matrix へ絞り込み、files 節に本 slice を追記。
- docs chain（本 IR を含む spec/design/plan/task/test-spec 6
  file）+ 各 `docs/*/README.md` managed index 再生成。

### Decisions During Implementation

- 喪失モデルは `set_len` truncate — writer は media/journal へ
  `File` 直接 write（userspace buffer なし）で、checkpoint の
  `sync_data` 完了時点の実長が durable frontier と一致するため、
  frontier 以降の byte を truncate することが「write 成功したが
  sync に届かなかった byte の喪失」の忠実なモデルになる。file
  handle を開いたまま truncate せず drop 後に別 open で行うのは、
  dead process が handle を閉じた状態を再現するため。
- 非対称喪失は 1 test に 2 sub-case（別 StagingArea）として
  実装 — setup が同型で、分けると fixture helper が二重になる。
  review の fail-closed 性（unverified/corrupt byte を自動削除・
  truncate しない）も両方向で assert。
- probe の観測は全て `key=value` + err-as-value — capability 非
  対応・環境依存結果を失敗にせず観測値として記録し、
  `detect_dir_sync` が生の dir sync 失敗時に `Supported` を
  主張しないことだけを不変条件として assert。
- process kill では dirty page が保持される（post-kill 実長に
  unsynced tail が残る）ことが実測されたため、power-loss 喪失の
  代替としては使えず、ledger では「process kill は unsynced
  喪失を再現しない」実測行 + 真の power-loss `NotRun` の組み
  合わせに留めた。fixture の truncate モデルが喪失シナリオの
  再現を担う。
- `realfs` は `vsc`/`vsd` と同じ main.rs 内 subcommand として
  実装 — 新 bin crate や新依存を増やさない
  （`synvelis_recording::staging` の `detect_dir_sync`/
  `publish_atomic*` は既存依存経路で到達可能）。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

- `cargo test -p synvelis-recording --test recovery_review
  rcd02_unsynced` — Pass、2 tests（補助証拠; canonical = mise）
- `mise run test-recording-real-fs` — Pass、2 tests（本機
  Windows/NTFS・rustc 1.99.0。canonical opt-in lane）
- `recording-poc realfs probe` の観測値（rename/dir sync/
  publish/set_len 全 18 key）— ledger `RCD-02-REALFS` 節へ
  実測値として転記
- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `test-msrv` — PR 本文に結果を記録

NotRun: 真の power-loss（電源断・OS crash 相当。非破壊的に自動化
不能、process kill は dirty page を保持するため代替不可）・他
OS FS matrix（Windows/NTFS のみ実測）・実 device 録音（#115）・
#118 製品 GUI E2E・#86 verified blob 実接続・#84 project 側
HEAD CAS（各 gate/別 scope）。

## Risks

- truncate fixture は「file 上の byte が frontier へ巻き戻る」
  モデルであり、実 power-loss では partial sector write や
  allocate-but-zeroed extent 等の別形態も起こり得る。本 slice
  で扱うのは「write 成功・sync 未達の tail が消える」形のみで、
  その限定を doc/ledger に明記した。
- `#[cfg(windows)]` の `detect_dir_sync == Unsupported` pin は
  現行 Windows 実測に基づく。将来 Windows で dir sync が可能に
  なった場合この assertion は更新が必要（観測値そのものは
  環境非依存 assertion で守られる）。
- `realfs probe` の residue 判定は既知 file 名との差分 — StagedFile
  temp の命名規約が変わっても検出できるよう file 名列挙方式に
  した。

## Follow-ups

- 真の power-loss 実測（実機電源断、非破壊自動化不能のため
  人間/実機 gate）。
- 他 OS（macOS/Linux）real-FS lane — 同一 probe が posix で
  どう振る舞うか（dir sync は posix で実測 Supported 相当を
  期待するが未実測）。
- #86 verified blob import/Asset 登録・#84 project 側 HEAD CAS
  参照進化（別 Issue gate）。
