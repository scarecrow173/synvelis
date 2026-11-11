---
id: "TSPEC-c33JwV8kL5uDnMRO2sV7Be"
type: "test-spec"
status: "approved"
title: "RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe test-spec"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-02-unsynced-loss-real-fs-probe.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/rcd-02-unsynced-loss-real-fs-probe.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 Unsynced-Write Loss Fixture + Real-FS Probe test-spec

## Purpose

unsynced-write 喪失（`write_all` 成功後・`sync_data` 未達の
byte が OS に捨てられる喪失形）を headless fixture として
決定的に検証し、Windows/NTFS real-FS の rename/dir sync/
unsynced tail 実挙動を opt-in lane で実測する test-spec。
fake FS/injected fault の成功を実 FS durability の証明に
しない。

## Feature

crash 後の staging media/journal 復旧（verified prefix のみ
Complete）と、実 FS 上の publish/durability capability 観測。

## Rules

- **R1 Sync-frontier truncation model**: `checkpoint()` 成功
  直後の file 長は最終 sync frontier。post-checkpoint の
  media/journal byte を `set_len` で除去した状態は、
  power-loss で OS が dirty page を捨てた状態と観測上等価で
  なければならない（file 長・内容ともに frontier へ戻る）。
- **R2 Verified prefix のみ確定**: 対称喪失・非対称喪失の
  いずれでも、`scan`/`review_stream(Adopt)`/`recover_finalize`
  は Last Valid Checkpoint の `media_len` までを確定し、
  Finalize record 不在・tail 残存では `FinalizedOutcome::
  Partial` に留める（`Complete` に昇格しない）。
- **R3 喪失分の保存/隔離**: unverified tail byte は file 上に
  保存され報告される（`unverified_tail_bytes`）。journaled
  被覆が media 実長を超える場合は fault として報告される
  （`media shorter than journaled coverage`）。recovery は
  破損・未検証 byte を削除・切詰めしない。
- **R4 実測の正直さ**: real-FS probe は観測値をそのまま出力
  し、capability 非対応を `Supported` に偽装しない。
  `detect_dir_sync` の分類は raw `File::open(dir)`/`sync_all`
  観測と整合する。
- **R5 process kill ≠ power-loss**: process kill
  （TerminateProcess 相当）後も unsynced byte は OS cache
  経由で観測可能であり、kill は unsynced 喪失を再現しない。
  この実測は power-loss 行を `NotRun` に留める根拠であり、
  power-loss 相当として `Pass` を主張しない。
- **R6 Opt-in 分離**: real-FS lane は `#[ignore]` + manual
  task で実行され、既定 `cargo test`/CI に混入しない。

## Examples

- **E1 対称喪失**: chunk 0-2 + checkpoint → chunk 3-4 pump →
  drop → media/journal を synced 長へ truncate → scan は
  `CleanEnd`/`Healthy`/`Pending`（chunk 3-4 の record も
  喪失）→ Adopt は `Partial`・`media_byte_len`=synced 長・
  spans=3 件のみ。
- **E2 非対称（media 残留）**: journal のみ truncate →
  `unverified_tail_bytes`=追加 media byte・`Partial` 分類 →
  Adopt は verified prefix のみ `Partial`。unverified byte は
  file 上に残る。
- **E3 非対称（journal 残留）**: media のみ truncate →
  `media shorter than journaled coverage` fault・`Partial`
  分類 → Adopt は checkpoint まで再検証で `Partial`
  （journaled tail は除外）。
- **E4 rename 実測**: `fs::rename` の missing-dest/replace/
  open-src/open-dst 各挙動と `create_new` の `AlreadyExists`
  を実 FS で観測・記録する。
- **E5 dir sync 実測**: `File::open(dir)`/`sync_all` の生
  観測と `detect_dir_sync` 分類が一致（Windows/NTFS では
  `Unsupported`）。
- **E6 publish 実測**: `publish_atomic`（replace）と
  `publish_atomic_create_new`（競合 `AlreadyExists`）が実 FS
  で動作し temp residue を残さない。
- **E7 kill 実測**: `write-unsynced` 子 process を ready 後に
  kill → file 実長・内容が unsynced 分を含めて観測可能
  （dirty page は process 死亡で失われない）。

## Guarantees

- unsynced 喪失の対称/非対称の全形で verified prefix 契約が
  保たれる。
- real-FS lane の全 assert は環境非依存の不変条件 +
  `cfg!(windows)` の Windows 期待に限定され、観測値自体は
  常時 fail にしない。
- ledger の実測行は lane 実測値のみ `Pass`、power-loss は
  `NotRun` 維持。

## Non-goals

- 真の power-loss 実測（自動化不能・`NotRun`）。
- 他 OS FS matrix・実 device・製品 GUI（各 Issue gate）。
- writer/recovery/staging 実装の変更検証（fixture/probe のみ。
  契約変更を伴う不具合は報告して別 slice へ）。

## Risk

- `set_len` モデルは「sync frontier へ戻る」喪失のみを表し、
  部分 sector 書込み等の物理的 torn write は表さない —
  表現範囲を文書で限定する。
- real-FS 観測値は環境依存であり、ledger 転記は本機
  （Windows/NTFS・temp volume）の値に限定する。
