---
id: "IR-SV-RF305-export"
type: "impl"
status: "completed"
title: "SYN-RF305 render/player WAV Export StagedFile 移行 実装記録・検証証跡"
created: "2026-11-07"
updated: "2026-11-07"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SYN-RF305 render/player WAV Export StagedFile 移行 — 実装記録・検証証跡

## 対象

- Issue: https://github.com/scarecrow173/synvelis/issues/82
  （関連: Closed https://github.com/scarecrow173/synvelis/issues/25）
- Upstream: https://github.com/scarecrow173/ruviform/issues/305 /
  https://github.com/scarecrow173/ruviform/pull/391
- Branch: `scarecrow173/sv-rf305-export`（PR181。origin/main
  `2c0711e` merge 済み）
- Ruviform 解決 SHA: `27698cb108c672c5758e724d8623663983db1224`
  （RF391 最終 combined 候補 = RF main `5d48298b` + staged-file
  cutover + RF407 bounded bridge `4c675193` + docs 追記。
  `3473a964 → 27698cb1` は docs/index のみでコード差分ゼロを
  本 worker が `git diff --stat` で独立確認済み。
  `Cargo.lock` 内の全 ruviform crate が一致・manifest は既定
  ブランチ追従の git 依存のまま・恒久 pin なし。
  解除条件: RF391/RF407 が main へ merge され `cargo update` で新
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
  `open_readback` → `ruviform_io_symphonia::with_borrowed_demuxer`
  （RF407/ruviform #400 の正本 bounded bridge）→ callback 内で
  `SourceReader::open(Box::new(&mut dyn reborrow), stream id,
  DecodeMode::Canonical)` の canonical decode。`MediaSource: 'static`
  と borrowed `!Send` `ReadbackFile` の境界は RF 側 bridge が担い、
  SV 独自の `ReadProxy`/`serve_readback`/channel/scoped-thread は
  完全撤去した。in-flight encoded byte は bridge 契約どおり方向
  あたり ≤64KiB に bounded で、staged bytes を全量 RAM に保持しない
  （peak ≈ bridge chunk + decoded planes、移行前 `File` source と
  同等）。全量 `Vec` 複写を行った初期移行リビジョンの追加
  encoded-RAM 退行は構造的に解消（root P1 finding への対応）。
  `expect_pcm` は callback に capture せず、`DecodedPlanes` を
  `Send` な戻り値で呼出し thread 側比較。worker panic は
  `catch_unwind` で typed `MediaError` 化し分類を保持。
  rate/channel/frame 数/PCM 検証は従来どおり。
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
  crate を `27698cb1` へ更新。`ui/vendor` 再同期。

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

**実装の遷移**: 当初は SV 側の一時的 workaround として
`std::thread::scope` + request/response channel proxy（`ReadProxy`/
`serve_readback`）で同構造を実装していたが、汎用 Read+Seek →
`'static` decoder 接続の正本は Ruviform が担う方針となり、RF407
（ruviform PR407・`4c675193`）が `with_borrowed_demuxer` と
`impl Demuxer for &mut D` / `SourceReader<'a>` を提供。最終 head
では SV 側 proxy を完全撤去し、bridge を消費する形へ置換した。
`expect_pcm` は `&dyn Fn+Send`（非 Sync）のため worker へ渡せず、
`DecodedPlanes` を `R: Send` で返して呼出し thread で実行する
所有境界は同一。worker panic は bridge の `resume_unwind` を
`catch_unwind` で受け従来どおり `MediaError::Io` 化。
raw handle 漏れ・unsafe lifetime 延長・temp path reopen・独自
WAV parser・受入制限の追加は一切行っていない。

**RF407 `SourceReader<'a>` 追随**: named 位置3箇所へ lifetime 注釈
（`synvelis-render/src/prepare.rs` `OpenedSource.reader` フィールド
`SourceReader<'static>`、`decode_window` 引数 `&mut SourceReader<'_>`、
`synvelis-player/src/import.rs` `demuxer_container_label` 引数
`&SourceReader<'_>`）。式位置 `SourceReader::open`
（`prepare.rs`/`import.rs`/`resolver.rs`/`verify.rs`）は型推論で
無変更。他 named 使用は workspace 走査の結果ゼロ。

**修正後実測（`27698cb1`・Windows・最終 head）**: `fmt-check`/
`check`/`lint` pass、`mise run test` 89 binaries 0 fail、
`test-rt` 0 fail、`check-headless-deps` pass（9 crates）、
`check-ruviform-rev` pass（`27698cb1`・26 sources・REV match）、
`check-app-native`/`lint-app-native` pass、
`l2_e2e` 17/17・`x_e2e` 5/5（`l2_01` reopen + `md_b` PCM-exact が
bridge 経路を実測）。`test-msrv` は `cargo-platform@0.3.3`
（rustc 1.91 要求）により失敗 — 同 package は origin/main
`2c0711e`（SV175）が lockfile へ持ち込んだもので main 既存破壊、
本変更非起因として coordinator へ報告済み。

**残存 note**: decoded planes の O(output) 保持は移行前からの
`expect_pcm(&[Vec<f32>])` 契約由来で本修正の対象外（増分のみ
解消）。`expect_pcm` を増分 callback 化する選択肢は test 契約変更を
伴うため別途検討事項。

## Verification（Windows・実測・最終 head・候補 SHA `27698cb1`）

**候補 SHA**: 最終 candidate は
`27698cb108c672c5758e724d8623663983db1224`
（`feature/media-io-staged-file-cutover` head = RF main `5d48298b`
+ RF391 staged-file cutover + RF407 `4c675193` + docs 追記）。
`3473a964 → 27698cb1` の差分は `git diff --stat` で **docs/index
のみ7ファイル**・コード差分ゼロを本 worker が独立確認済み。
全 compile/test 証拠は `27698cb1` の lockfile で再実測した。

| コマンド | 実内容 | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | `cargo fmt --all -- --check` | pass |
| `mise run check` | `cargo check --workspace --all-targets` | pass |
| `mise run lint` | `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `mise run test` | `cargo test --workspace --locked` | pass（89 test binary・0 fail） |
| `mise run test-rt` | RT lane test | pass（0 fail） |
| `mise run check-headless-deps` | headless crate GUI dep 検査 | pass（9 crates, no GUI deps） |
| `mise run check-ruviform-rev` | lockfile SHA 整合 | pass（`27698cb1`・26 sources・REV match） |
| `mise run check-app-native` | `cargo check -p synvelis-app --features native --all-targets` | pass |
| `mise run lint-app-native` | `cargo clippy -p synvelis-app --all-targets --features native -- -D warnings` | pass |
| `mise run test-msrv` | rust 1.89 `cargo test --workspace --locked` | **fail — main 既存破壊**（`cargo-platform@0.3.3` が rustc 1.91 要求。SV175 `2c0711e` が lockfile へ持ち込んだ `synvelis-app` dev-dep → `ruviform-ui-test` → `cargo_metadata` 由来で本変更非起因。origin/main の lockfile にも同一 package が存在） |

- `l2_e2e` 17/17・`x_e2e` 5/5・`pc_b` 7/7 全 pass（`27698cb1` 実測）。
  `l2_05_fault_injection_partial_never_completed` は実
  `PublishFailure` 注入（destination directory）で
  `ExportStage::Publish` + `effect()==NotPublished` + destination
  保全 + temp 残滓なしを pin。
- 旧 API 検索: `crates/` 配下に `StagedOutput`・旧
  `PublishOptions`・`open_boxed_muxer` 参照 0 件。`verify.rs` に
  `read_to_end`/`Vec<u8>`/`Cursor`/`unsafe`/raw handle/`mpsc`/
  scoped-thread/proxy 参照 0 件（app 独自接続の完全撤去）。
- ドキュメント監査（`doc_maintenance.js plan --type all`）:
  RF305 チェーン 6 文書 + l2 spec `derived-by` 修正後、
  RF305/l2 関連 finding 0 件（残 247 件は本変更と無関係の
  既存 repo-wide finding）。index は `apply` で正規再生成済み。
- 独立レビュー: Reviewer A/B → Validator → Judge（実施結果は
  下記レビュー節を参照）。

## Independent Review

### 第1ラウンド（移行 head `5fa1ef9`・候補 `d68b14d`）

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

### 第2ラウンド（最終 head・combined 候補 `27698cb1` + origin/main
`2c0711e` merge + proxy 撤去 → RF407 bridge 消費）

review-protocol 完走（A 協調 / B 敵対 → Validator → Judge、
すべて独立 subagent。upstream 証拠は pinned cargo checkout
`27698cb` の file:line で検証）。

- **Reviewer A（cooperative）: ACCEPT WITH NON-BLOCKING NOTES —
  P0/P1 ゼロ**。8 不変条件を file:line で全検証:
  encoded 全量 RAM 複写なし（bridge ≤64KiB/方向・`sync_channel(1)`）
  ・`expect_pcm` は `&dyn Fn+Send`（非 Sync）で callback へ compile
  不可・Copy facts のみ capture・caller thread 同期 wait なし・
  panic → `resume_unwind` → `catch_unwind` → typed `MediaError::Io`
  → `ExportStage::Verify`・`Box<dyn Demuxer + '_>` reborrow は
  callback scope 内（escape 不可・upstream compile_fail doctest が
  pin）・driver が finalize→drop→seal→verify→publish 順序を型強制・
  lifetime 注釈は指定3箇所のみで完全・manifest は既定ブランチ git
  依存で恒久 pin なし・`l2_01`/`md_b`/fault-injection 全て default
  verify 経由で bridge 実測。N1（panic 注入 test なし）/N2（pc_b
  自体は verify 非経路、coverage は l2_e2e）/N3（vendor REV は
  gitignored 生成物）/N4（F: 旧 checkout は情報）は全て P3。
- **Reviewer B（adversarial）: ACCEPT WITH NOTES — P0/P1 ゼロ**。
  11 attack-surface 検証 clean: panic mapping 拡大は fail-safe
  （serve 側 panic も同型 `MediaError` で temp 破棄・publish 不可）、
  `AssertUnwindSafe` sound（catch 後 `readback`/`sealed` 不参照）、
  deadlock/liveness clean、メモリ境界主張は正直（encoded のみ
  bounded・decoded planes は既存 `expect_pcm` 契約由来 O(output)）、
  error kind 維持、seek 意味論は uncached で改善側、`!Send` 検証、
  reborrow 非 escape、全 export test が default verify 経由で bridge
  非 bypass、manifest clean。F1 panic msg の serve-side 誤帰属
  （P3 cosmetic、分類不変）/F3 旧候補 SHA・byte-copy 記述残存
  （P3・全て修正済み）/F4 `ErrorKind::Io` panic 分類は既存挙動
  （観察）。F2（IR の `md_b` bridge 主張の誤同定）は Validator が
  却下。
- **Validator: 6/6 adjudicated — P0/P1 なし**。F2 は
  `md_b_comp_export_pcm_exact`（`l2_e2e.rs:1648`）が `expect_pcm` +
  default verify → bridge を実測することを確認し B の誤同定として
  REJECTED（player `tests/md_b.rs` と混同）。F1/N1 を CONFIRMED
  （P3）。F3 修正済みを確認。verify.rs sanity 4点（
  `AssertUnwindSafe` 健全・Copy capture・reborrow 非 escape・
  callback 内 caller wait なし）全 CONFIRMED。lockfile SHA は
  親側 `check-ruviform-rev` 実測で担保。
- **Judge: ACCEPT — P0/P1/P2 ゼロ・merge-eligible・条件なし**。
  3 高リスク主張を自ら spot-check 再検証（panic mapping +
  `AssertUnwindSafe` soundness、reborrow containment + 3 注釈、
  F2 却下の妥当性）。`test-msrv` 失敗は main 既存破壊として
  non-blocking。残る P3（F1 メッセージ表現・`d68b14d` は歴史的
  記録として残置）は follow-up 任意、fix 義務なし。

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
- `test-msrv`: `cargo-platform@0.3.3`（rustc 1.91 要求）が
  `synvelis-app` dev-dep → `ruviform-ui-test` → `cargo_metadata`
  経由で SV175 `2c0711e` 以降の lockfile に存在 — **main 既存破壊**
  で本変更非起因。修正は SV175 所有範囲（依存 pin または代替）。
- RF391/RF407 本 merge 後の main HEAD 再解決・両 Consumer exact
  tuple 判定・候補 lock 解除: coordinator 所有ゲート。
