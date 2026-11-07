---
id: "IR-SV-RF305-export"
type: "ir"
status: "final"
title: "SYN-RF305 render/player WAV Export StagedFile 移行 実装記録・検証証跡"
created: "2026-11-07"
owners: ["scarecrow173"]
---

# SYN-RF305 render/player WAV Export StagedFile 移行 — 実装記録・検証証跡

## 対象

- Issue: https://github.com/scarecrow173/synvelis/issues/82
  （関連: Closed https://github.com/scarecrow173/synvelis/issues/25）
- Upstream: https://github.com/scarecrow173/ruviform/issues/305 /
  https://github.com/scarecrow173/ruviform/pull/391
- Branch: `feature/render-staged-file-rf305`
- Ruviform 解決 SHA: `7ead1441aaf31aac43f9cc454aaa30b03c4ce7f1`
  （refreshed PR391 候補 = RF main `e8adcc0` + staged-file cutover。
  `Cargo.lock` 内の全 ruviform crate が一致・manifest は既定
  ブランチ追従の git 依存のまま・恒久 pin なし。
  解除条件: RF391 が main へ merge され `cargo update` で新
  main HEAD へ再解決される時点）

## 実施内容

- `spec.rs`: `WrapSink`/`DynVerify`/`DynPublish`/`DynMuxerCtor` を
  新契約へ更新 — `&'a mut StagedFile` 借用、`&mut SealedFile`
  verify、`Result<PublishReceipt, PublishFailure>` publish。
  `OutputHooks` ドキュメントも `StagedFile`/`SealedFile` 契約へ。
- `export.rs`: `StagedFile::create`（`FileError` → `ExportError::Io`
  preflight）、`WavMuxer<&mut StagedFile>`（wrap 経路も
  `open_muxer` へ統一 — `open_boxed_muxer` は `'static` sink を
  要求し借用 sink では使えない）、`PublishOptions::new(mode,
  Durability::FileSync)`（`replace_existing` → `PublishMode`、旧
  `sync_before_publish: true` → `FileSync` の忠実写像）、既定
  publish は `SealedFile::publish`、非 Clone `RenderReceipt` は
  `frames_emitted`/`tail_evidence` 取得後に `ExportReceipt.render`
  へ move。
- `verify.rs`: `reopen_check` を `&mut SealedFile` 型へ。
  `open_readback` → **bounded channel proxy**（`ReadProxy` +
  `serve_readback`）→ symphonia demux → `SourceReader` canonical
  decode。`MediaSource: 'static` と borrowed `!Send` `ReadbackFile`
  の境界は scoped worker + request/response channel で橋渡しし、
  staged bytes を全量 RAM に保持しない（peak ≈ proxy chunk +
  decoded planes、移行前 `File` source と同等）。全量 `Vec` 複写を
  行った初期移行リビジョンの追加 encoded-RAM 退行は本修正で解消
  （root P1 finding への対応）。rate/channel/frame 数/PCM 検証は
  従来どおり。
- `l2_e2e.rs`: `BoundedSink`/`FailFlushSink` を `&'a mut StagedFile`
  借用型へ。publish 失敗注入は destination directory で実 OS
  adapter の `PublishFailure`（`NotPublished`）を発生させ、
  `source.effect()` を pin。fabricated error なし。
- `pc_b.rs`: `RenderOutput` 直接構築を新契約へ
  （`DynMuxerCtor` alias、`PublishMode::Replace` +
  `Durability::BestEffort` で旧 `sync_before_publish:false` を
  忠実写像、`SealedFile::publish` 委譲）。
- `Cargo.toml`（render dependencies / player dev-dependencies）に
  `ruviform-io-file.workspace = true`。`Cargo.lock` 全 ruviform
  crate を `d68b14d` へ更新。`ui/vendor` 再同期。

## 不変条件の対応

- finalize → muxer drop（借用解除）→ `seal()` → readback 検証 →
  publish の順序は Ruviform driver 契約が強制。finalize/Drop
  だけで `Completed` にならない構造は不変（`Completed` は
  `Ok(RenderReceipt)` 取得時のみ）。
- readback/verify 失敗は publish 不実行（driver が verify を
  publish 前に置く型契約 + `ExportStage::Verify` 分類を pin）。
- `RenderError::Publish{source: PublishFailure, cleanup}` は
  `ExportError::Render` で全体保持 → `stage_of` →
  `ExportStage::Publish`。effect/phase/kind/durability/recovery
  は flatten しない。`DestinationExists` kind は driver が
  `RenderError::DestinationExists` へ正規化。
- `Cancelled`/`Failed`/`Partial`/`Completed` と
  `NotPublished`/`Published`/`Indeterminate` は混同なし
  （`ExportOutcome`/`PublishEffect` が分類を担う）。

## 追加修正 — verify readback の encoded 全量 RAM 退行（root P1）

初期移行リビジョンの `verify.rs` は `read_to_end → Vec<u8> →
Cursor` で demux しており、decode planes（従来どおり O(output)）に
加え encoded bytes 全量も同時保持する新ピークメモリ退行があった
（例: 48k stereo f32・2h で旧 ~2.575GiB → 新 ~5.15GiB。コード評価、
OOM 未実測）。root の独立 Validator も CONFIRMED/P1。

**所有境界の診断**: `open_demuxer(impl MediaSource + 'static)`
（`MediaSource = Read + Seek + Send`、symphonia `MediaSourceStream`
も 'static 所有）に対し `SealedFile::open_readback` は借用・
`!Send` な `ReadbackFile<'a>`（handle 非露出・`try_clone` なし —
RF の型状態設計上の意図的制約）しか貸さない。借用を保持する限り
`'static` 所有 source は作れず、全量 copy が強制される構造。

**実装（SV 所有・std のみ・新依存なし）**: `std::thread::scope` で
scoped worker 上で `open_demuxer(ReadProxy)` + `SourceReader`
decode を駆動し、呼出し thread 側が `serve_readback` で
`&mut ReadbackFile` の positioned read を配給する request/response
channel proxy。`ReadProxy` は channel endpoint のみ所有するため
`'static + Send`、任意 seek（`SeekFrom::End` は `Len` 要求で解決）
を正直に実装し、peak は O(proxy chunk ≤1MiB × in-flight 2) +
decoded planes に縮退 — 移行前 `File` source と同等 profile。
`expect_pcm` は `&dyn Fn+Send`（非 Sync）のため worker へ渡せず、
planes を join で返し呼出し thread で実行。worker panic は
`MediaError::Io` 化して失敗経路を保持。raw handle 漏れ・unsafe
lifetime 延長・temp path reopen・独自 WAV parser・受入制限の追加は
一切行っていない（channel は実 readback 経由、decode は symphonia
のまま）。

**修正後実測（`d68b14d`・Windows）**: `fmt-check`/`check`/`lint`
pass、`mise run test` 87 binaries 0 fail、`test-rt` 0 fail、
`check-headless-deps` pass、`check-ruviform-rev` pass、
`l2_e2e` 17/17・`x_e2e` 5/5（`l2_01` reopen + `md_b` PCM-exact が
proxy 経路を実測）。

**残存 note**: decoded planes の O(output) 保持は移行前からの
`expect_pcm(&[Vec<f32>])` 契約由来で本修正の対象外（増分のみ
解消）。`expect_pcm` を増分 callback 化する選択肢は test 契約変更を
伴うため別途検討事項。

## Verification（Windows・実測・候補 SHA `d68b14d` / `7ead144`）

**候補 SHA**: 最終 candidate は
`d68b14d082128b66ec1dd1328f82e8251dfba694`
（`feature/media-io-staged-file-cutover` head）。`7ead144 → d68b14d`
の差分は `git diff --stat` で **docs のみ2ファイル**
（`docs/impl/ir/issue-305-…` +25, `docs/tasks/issue-305-t1-…` ±1）
・コード差分ゼロを本 worker が独立確認済み。全 compile/test 証拠は
コード同一の `7ead144` で取得したものを含み、`d68b14d` で
`check` / `check-ruviform-rev` / export 3 test binary を再実測した。

| コマンド | 実内容 | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | `cargo fmt --all -- --check` | pass |
| `mise run check` | `cargo check --workspace --all-targets` | pass |
| `mise run lint` | `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `mise run test` | `cargo test --workspace --locked` | pass（87 test binary・0 fail） |
| `mise run test-rt` | RT lane test | pass（0 fail） |
| `mise run check-headless-deps` | headless crate GUI dep 検査 | pass（9 crates, no GUI deps） |
| `mise run check-ruviform-rev` | lockfile SHA 整合 | pass（`d68b14d…`・24 sources・REV match） |
| `mise run test-msrv` | rust 1.89 `cargo test --workspace --locked` | pass（87 test binary・0 fail） |
| `mise run check`（`d68b14d` 再実測） | `cargo check --workspace --all-targets` | pass |
| `cargo test --locked`（`d68b14d` 再実測） | `l2_e2e` / `x_e2e` / `pc_b` | pass（17/17・5/5・8/8） |

- `l2_e2e` 17/17・`x_e2e` 5/5・`pc_b` 8/8 全 pass（`d68b14d` 実測）。
  `l2_05_fault_injection_partial_never_completed` は実
  `PublishFailure` 注入（destination directory）で
  `ExportStage::Publish` + `effect()==NotPublished` + destination
  保全 + temp 残滓なしを pin。
- 旧 API 検索: `crates/` 配下に `StagedOutput`・旧
  `PublishOptions`・`open_boxed_muxer` 参照 0 件。
- ドキュメント監査（`doc_maintenance.js plan --type all`）:
  RF305 チェーン 6 文書 + l2 spec `derived-by` 修正後、
  RF305/l2 関連 finding 0 件（残 247 件は本変更と無関係の
  既存 repo-wide finding）。index は `apply` で正規再生成済み。
- 独立レビュー: Reviewer A/B → Validator → Judge（実施結果は
  下記レビュー節を参照）。

## Independent Review

review-protocol 完走（A 協調 / B 敵対 → Validator → Judge、
すべて独立 subagent、相互の finding 非共有で並行）。

- **Reviewer A（cooperative）: ACCEPT（non-blocking notes）**。
  5 不変条件を候補 `7ead144`/`d68b14d` ソースの file:line で検証:
  driver が finalize→muxer drop→seal→verify（operation_id swap
  guard）→publish を強制、verify 失敗は publish 不可、
  `PublishFailure` typed 証拠は `RenderError::Publish{source}`→
  `ExportError::Render` 経路で非 flatten 保全、destination/project
  保護は preflight + `CreateNew` atomic + RecoveryHandle cleanup の
  3 層。mapping fidelity と readback 独立性を確認。
- **Reviewer B（adversarial）: ACCEPT（non-blocking notes）**。
  `PublishOptions` 写像を候補ソースで実検証:
  `replace_existing` → `PublishMode::{Replace,CreateNew}`、
  `sync_before_publish: true` → `Durability::FileSync`
  （dir-sync 非追加、旧 `File::sync_all`+rename と同順序）、
  `pc_b` の `BestEffort` は旧 `sync_before_publish:false` 相当。
  directory-destination 失敗注入は実 OS adapter 経由の genuine
  `PublishFailure`（fabricated error なし）、verify の readback→
  `Cursor` 経路は真に独立 decode。evidence の under-claim は
  保守的で over-claim 経路なし。
- **Validator: 12/12 CONFIRMED・0 REJECTED・0 UNVERIFIED**。
  全 candidate は正確な observation であり factual defect ゼロ。
  `7ead144→d68b14d` docs-only 差分を cargo checkout の
  木比較（2,076 ファイル、全 non-docs subtree 完全一致）で
  独立再検証済み。
- **Judge: ACCEPT WITH NON-BLOCKING NOTES — P0/P1 ゼロ**。
  #7（custom verify hook 未カバー, P2）と
  #9（`synvelis-recording::staging::publish_atomic` の
  手組 staged publish, P2・範囲外）は follow-up issue 候補。
  残りは P3 の report-only 観察。fix 義務なし。

## NotRun / Unsupported

- `PublishEffect::Indeterminate` の Consumer 側決定的注入:
  `PublishFailure` 非公開 constructor + RF mock は `#[cfg(test)]`
  internal のため生成不能。typed 分類・evidence 保全経路は
  `NotPublished` leg で pin し、`Indeterminate`/`Published` も
  同一経路を通る（RF 側 mock/driver 試験の責務）。
- macOS/Linux 実測: NotRun（実装は OS 中立 API のみ使用、
  Windows 実測済み）。実電源断・クラッシュ中回復は RF adapter
  契約。
- GUI/Native Theme/Asset Provider/RF398: 所有範囲外・未変更。
- RF391 本 merge 後の main HEAD 再解決: RF 側 merge 待ちの
  coordinator 所有ゲート。
