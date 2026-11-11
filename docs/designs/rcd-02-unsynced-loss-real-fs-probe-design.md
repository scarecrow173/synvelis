---
id: "DESIGN-VClnKX5mXMKj7CQFWgFYAE"
type: "design"
status: "approved"
title: "RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe Design"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-02-unsynced-loss-real-fs-probe.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/designs/rcd-02-stagedfile-publish-migration-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/designs/rcd-02-stagedfile-publish-migration-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-unsynced-loss-real-fs-probe.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  defers: []
  deferred-by: []
---

# RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe Design

## Context

Issue #116 task1-3 で `DurabilityEvidence`・Recovery Review・
fault matrix・`StagedFile` publish が統合済み。残存する本
slice の対象は 2 つ。

- **(a) unsynced-write 喪失 fixture**: 既存 fault matrix は
  `FaultWriter`/`Inject`（write/sync の呼出し失敗）と journal
  byte 編集（torn tail/corrupt）で再現する。これらは「writer
  が失敗を観測した」fault だが、power-loss 固有の「`write_all`
  が**成功**し file に見えていた byte が、`sync_data` 未達の
  ため OS dirty page と共に失われる」喪失形は未再現だった。
- **(b) Windows/NTFS real-FS probe**: ledger の実測行は
  `detect_dir_sync` の manifest 記録値のみ。rename 原子性・
  publish pipeline・process kill 後の unsynced byte 実挙動を
  独立に実測する opt-in lane が無い。

関連既存構造:

- `StreamWriter::checkpoint()` = media `sync_data` → Checkpoint
  record append → journal `sync_data` → dir sync（capability
  時）。checkpoint 成功直後の media/journal 実 file 長が、
  その時点で durable が保証された最終 sync frontier。
- `process_chunk` は media `write_all` → journal `write_all`
  を File へ直接行い、userspace buffer を持たない。
  `write_all` 成功 = OS cache まで到達（≠ durable）。
- `recovery::scan` は journal intact prefix の Last Valid
  Checkpoint を durable frontier として採用し、
  `journaled_end`（intact chunk record が被覆する media 末尾）
  との差分を `validated_tail_bytes`、media 実長超過分を
  `unverified_tail_bytes`、journaled 被覆未満を fault として
  分類する。`review_stream(Adopt)` /
  `recover_stream_finalize_faultable` は verified prefix のみ
  を finalize し、Finalize record 不備・tail 残存時は
  `Partial` に留める。
- `recording-poc vsc record --crash`（exit 3）→ `vsc recover`
  の process 越境回復先例が `tests/vsc_process.rs` にある。

## Scope

### In Scope

- `crates/synvelis-recording/tests/recovery_review.rs` —
  unsynced-loss fixture test 2 件。
- `crates/recording-poc/src/main.rs` — `realfs` サブコマンド
  （`probe` / `write-unsynced`）。
- `crates/recording-poc/tests/real_fs_probe.rs` — `#[ignore]`
  opt-in test。
- `mise.toml` — `test-recording-real-fs` manual task 1 件
  （spec が許可する例外。既存 `test-recording-real-input` と
  同型）。
- `evidence/reports/rcd-02-durability-recovery-review-ledger.md`
  — 実測行追記。
- docs チェーン + 管理 index 再生成。

### Out of Scope

- `crates/synvelis-recording/src/` 一切（fixture/probe のみ。
  契約そのものは変更しない）。
- 真の power-loss、他 OS matrix、実 device、製品 GUI、
  #84/#86/consumer 接続（各 Issue gate・`NotRun` 維持）。

## Design

### 1. Unsynced-write 喪失のモデル化

power-loss で失われるのは「`write_all` 成功済みだが該当 file
の `sync_data` が一度も完了していない byte」＋「同様に
sync 未達の file 長 meta data」。本 fixture は、
`checkpoint()` 成功直後の file 長を最終 sync frontier とし、
その後に pump した chunk 群を `File::set_len(frontier)` で
除去する。`set_len` は実 file 長を即座に戻すため、OS が
dirty page を捨てた post-crash 状態と観測上等価（file 長・
内容ともに frontier へ戻る）のを決定的に作る。

- `checkpoint()` 直後に `fs::metadata(path).len()` を
  media/journal それぞれ採取（その時点で全 write は sync
  済み）。これを `synced_media_len`/`synced_journal_len` と
  する。
- 追加 `push_and_pump` で unsynced byte を発生させてから
  writer を `drop`（finalize しない crash 相当）。
- `OpenOptions::new().write(true).open(path)?.set_len(len)`
  で truncate（append open では truncate できないため別
  handle）。

### 2. Fixture test 構成（`recovery_review.rs` 追補）

- **`rcd02_unsynced_loss_truncates_to_sync_frontier`**
  （対称喪失）: chunk 0-2 → checkpoint → chunk 3-4 pump →
  drop → media/journal を synced 長へ truncate。
  - scan: journal は checkpoint record で clean に終わる
    （chunk 3-4 の record も喪失）→ `StopReason::CleanEnd`、
    `journaled_end == synced_media_len == media_len` で
    tail なし → `StreamClass::Healthy`、
    `ReviewState::Pending`。
  - Adopt: verify は checkpoint まで →
    `FinalizedOutcome::Partial`（Finalize record 不在のため
    Complete に昇格しない）、`media_byte_len == synced_media_len`、
    `source_frames`/`spans` は chunk 0-2 分のみ。
    喪失 chunk は finalized ref・digest に一切混入しない。
- **`rcd02_unsynced_loss_asymmetric_tail`**（非対称喪失・
  2 sub-case）:
  - media 残留 / journal 喪失: journal のみ truncate。
    `journaled_end == synced_media_len` のまま media が伸長
    → `unverified_tail_bytes == unsynced media bytes`、
    `StreamClass::Partial`。Adopt は verified prefix のみ
    Partial、unverified byte は file 上に保存される
    （truncate/削除しない）。
  - journal 残留 / media 喪失: media のみ truncate。
    intact chunk record が被覆する `journaled_end` より
    media が短い → fault「media shorter than journaled
    coverage」が記録され `StreamClass::Partial`。Adopt は
    checkpoint まで再検証が通り、journaled tail を除外して
    `Partial`（`validated_tail_bytes > 0` で clean/no_tail
    不成立）。

### 3. Real-FS probe（`recording-poc realfs`）

`vsc`/`vsd` と同じく main.rs に `realfs` dispatch を追加する
（依存追加なし・std fs + `synvelis_recording::staging` API）。

- `realfs probe <dir>`: in-process の FS 観測を決定的順で
  `key=value` 出力する。全て観測値であり、err も計測結果
  （失敗で偽装しない）。
  - `rename_missing_dest`: `fs::rename` a→b（b 不在）。
  - `rename_replace` + `rename_replace_content_ok`: 既存 b への
    rename と内容一致（`publish_atomic` が前提とする atomic
    replace）。
  - `rename_src_open` / `rename_dst_open`: open handle を
    持つ file の rename / open 中 destination への rename
    （Windows の sharing semantics 観測）。
  - `create_new_existing`: `create_new` no-replace の
    `AlreadyExists`（`PublishMode::CreateNew` の基礎）。
  - `dir_open` / `dir_sync`: `File::open(dir)` /
    `dir.sync_all()` の生観測。
  - `detect_dir_sync`: `staging::detect_dir_sync` の分類値
    （raw 観測との一致確認）。
  - `publish_atomic_replace` + `publish_atomic_content_ok` +
    `publish_residue`: StagedFile pipeline が実 FS で replace
    公開し temp residue を残さないこと。
  - `publish_create_new_conflict`: `publish_atomic_create_new`
    の競合 `AlreadyExists`。
  - `file_nosync_len`: write → drop（no sync）→ 再 open で
    実長（in-process 観測）。
  - `set_len`: truncate 自体の動作（fixture モデルの実 FS
    裏付け）。
- `realfs write-unsynced <file> <synced> <unsynced>`:
  `<synced>` byte（`0xA5`）write → `sync_data` →
  `<unsynced>` byte（`0x5A`）write → `ready …` を stdout
  flush → stdin 1 byte 待ちで block。test は ready 検出後に
  `Child::kill`（TerminateProcess 相当・graceful close なし）
  で殺し、file 実長・内容を別 process から観測する。

### 4. Opt-in lane（`tests/real_fs_probe.rs` + mise task）

- `#[ignore = "real-FS lane: requires a real local filesystem (run `mise run test-recording-real-fs`)"]`
  で既定 `cargo test` / CI に混入しない（`real_input.rs` と
  同じ opt-in 運用）。
- `realfs_probe_fs_capabilities`: `probe` の全 key=value を
  解析して print（ledger 転記用）し、不変条件を assert:
  `rename_replace=ok` かつ content 一致、`create_new`/
  `publish_create_new` が `AlreadyExists`、
  `publish_atomic` 成功 + residue 0、`detect_dir_sync` が
  raw `dir_open`/`dir_sync` 観測と整合（open/sync が
  非対応なら `Supported` を返さない）。dir sync の
  `Supported`/`Unsupported` 判定値自体は環境差として記録する
  （Windows では `Unsupported` 期待を `cfg!(windows)` で
  assert、他 OS は記録のみ）。
- `realfs_unsynced_tail_survives_process_kill`: kill 後の
  file 実長 == synced+unsynced（Windows/NTFS で process kill
  は dirty page を失わない実測）、synced 領域の内容一致、
  tail byte も観測可能であることを assert。これは
  「process kill ≠ power-loss」の実測であり、power-loss 行を
  `NotRun` に留める根拠。
- `mise.toml` 追加は `test-recording-real-input` と同型の
  manual task 1 件（`cargo test -p recording-poc --locked
  --test real_fs_probe -- --ignored --test-threads=1`）。
  `--test-threads=1` は temp dir 共有を避けるため。

### 5. Ledger 実測行

unsynced-loss fixture の検証行は `RCD-02-FAULT` セクションへ
追加し、real-FS 観測値は専用の `RCD-02-REALFS` セクションへ
実測行として分離する（injected fault の fixture matrix と、
実 FS 上の観測結果を混在させない）。status 語彙は既存
ledger と同じ（`Pass`=実測 lane で測定、`NotRun`= lane
存在・未計測、`Unsupported`=能力欠如）。真の power-loss
行は `NotRun` のまま新設し、process kill 実測と区別する
（kill で代替可能な範囲のみ Pass）。

### 6. 変更しないもの

- `synvelis-recording/src/` の writer/recovery/staging/
  journal/faultinj — 契約変更なし。
- fault 注入 seam（`FaultWriter`/`PublishInject`）の API。
- `vsc`/`vsd` 既存サブコマンドの引数・挙動。
- `Cargo.toml`/`Cargo.lock`（依存追加なし）。

## File Map

| File | Change |
| --- | --- |
| `crates/synvelis-recording/tests/recovery_review.rs` | unsynced-loss fixture test 2 件追加 |
| `crates/recording-poc/src/main.rs` | `realfs` dispatch + `probe`/`write-unsynced` 実装 |
| `crates/recording-poc/tests/real_fs_probe.rs` | `#[ignore]` opt-in test 新規 |
| `mise.toml` | `test-recording-real-fs` manual task 追加 |
| `evidence/reports/rcd-02-durability-recovery-review-ledger.md` | 実測行・fixture 追記 |
| `docs/specs|designs|plans|tasks|test-specs|impl/ir` | 本 slice の doc チェーン |
| `docs/*/README.md` | 管理 index 再生成 |

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| `set_len` truncation が power-loss 喪失のモデルとして過剰/過少 | low | med | 「sync frontier へ戻る」喪失形のみを主張。file 内容破損・meta 消失は別 fault（既存 torn/corrupt matrix が被覆）と文書で区別 |
| real-FS lane が他 OS/環境で異なる観測値を返す | med | low | opt-in `#[ignore]` + Windows 期待は `cfg!(windows)` 限定 assert。観測値は print で ledger 転記 |
| probe が temp residue を残し test 間で干渉 | low | low | pid 付き temp dir + `--test-threads=1` + 後始末 |
| 既存 test の回帰 | low | med | `mise run test` 全件。新規は追補のみで既存 test 不変 |

## Verification Matrix

| Acceptance | Verification |
| --- | --- |
| unsynced-loss fixture + verified prefix 契約 | `recovery_review.rs` 新規 2 test（`mise run test`） |
| Windows/NTFS 実測 + ledger 実測行 | `mise run test-recording-real-fs` 実行 → 観測値を ledger 転記 |
| NotRun/Unsupported を Pass 扱いしない | ledger 行 status の目視 + power-loss 行 `NotRun` 維持 |
| 回帰不変 | `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv` |

## Self-Review

- `set_len` を append handle でなく write handle で開く点を
  実装詳細として固定した（append では truncate 不能）。
- 「journal が clean に終わる」は chunk record 喪失の結果で
  あり、scan が Healthy を返しても Adopt の outcome は
  Finalize record 不在で Partial — 「見た目 clean でも
  Complete にならない」を assertion の核にした。
- probe の assert は「環境非依存の不変条件 + Windows 期待値
  （`cfg!(windows)`）」に限定し、観測値そのものを常時 fail
  にしない（他 OS で誤 fail させない）。
