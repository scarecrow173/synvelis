---
id: "DESIGN-034T2RF305WAVSTAGED00Y"
type: "design"
status: "approved"
title: "SYN-RF305 WAV Export StagedFile 移行設計"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/82"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/syn-rf305-wav-export-staged-file.md"
    - "docs/adr/syn-rf305-consumer-first-staged-file-cutover.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-rf305-wav-export-staged-file.md"
    - "docs/adr/syn-rf305-consumer-first-staged-file-cutover.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-rf305-export-staged-publish-contract.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/25"
    - "https://github.com/scarecrow173/ruviform/issues/305"
    - "https://github.com/scarecrow173/ruviform/pull/391"
  defers: []
  deferred-by: []
---

# SYN-RF305 WAV Export StagedFile 移行設計

## 1. 背景と制約

Ruviform `render` ドライバの出力束は、候補 `d68b14d`（= `7ead144` + docs-only; `7ead144` = RF main
`e8adcc0` + PR391 cutover の merge）で以下の型状態契約になる:

```rust
pub struct RenderOutput<Mk, V, Pb> {
    pub staged: StagedFile,
    pub make_muxer: Mk,      // for<'a> FnOnce(&'a mut StagedFile)
                             //   -> Result<Box<dyn Muxer + 'a>, MediaError>
    pub encoder: Box<dyn Encoder>,
    pub publish_options: PublishOptions,
    pub verify: Option<V>,   // FnOnce(&mut SealedFile, &OutputFacts)
                             //   -> Result<(), MediaError>
    pub publish: Pb,         // FnOnce(SealedFile, PublishOptions)
                             //   -> Result<PublishReceipt, PublishFailure>
}
```

driver が強制する順序は `make_muxer(&mut staged)` → drive →
`muxer.finish()` → `drop(muxer)`（借用解除）→ `staged.seal()` →
`verify(&mut sealed, &facts)`（`operation_id` identity guard 付）→
`publish(sealed, publish_options)`。`RenderReceipt` は
`publish: PublishReceipt` を保持し `Clone` を持たない。

旧 API（RF main 現行）との差分:

| 旧 (`StagedOutput`) | 新 (`StagedFile`/`SealedFile`) |
| --- | --- |
| muxer へ clone した `File` を渡す | `&mut StagedFile` 借用で渡す |
| verify hook が `&Path` を受ける | `&mut SealedFile` + `open_readback` |
| publish が `Result<(), MediaError>` | `Result<PublishReceipt, PublishFailure>` |
| `PublishOptions{replace_existing,sync_before_publish}` | `PublishOptions{mode,visibility,durability}` |
| `RenderError::Publish(MediaError)` | `Publish{source: PublishFailure, cleanup}` |

## 2. 写像設計

### 2.1 `OutputHooks` seam（`spec.rs`）

- `WrapSink`:
  `Box<dyn for<'a> FnOnce(&'a mut StagedFile) -> Box<dyn MediaSink + 'a> + Send>`。
  `&mut StagedFile` は `Write+Seek+Send` なので `MediaSink` を満たす。
  fault-injection sink は借用を包む新しい sink として muxer へ渡す。
  `open_boxed_muxer` は `Box<dyn MediaSink>`（`'static`）を要求するため
  借用 sink では使えず、wrap 経路も `open_muxer`（generic `S:
  MediaSink`）へ統一する。
- `DynVerify`:
  `Box<dyn FnOnce(&mut SealedFile, &OutputFacts) -> Result<(), MediaError> + Send>`。
- `DynPublish`:
  `Box<dyn FnOnce(SealedFile, PublishOptions) -> Result<PublishReceipt, PublishFailure> + Send>`。
- `DynMuxerCtor`（crate-private）:
  `Box<dyn for<'a> FnOnce(&'a mut StagedFile) -> Result<Box<dyn Muxer + 'a>, MediaError> + Send>`。

### 2.2 `export.rs` orchestration

- `StagedFile::create(&request.destination)` —
  `FileError` は従来どおり `ExportError::Io`（`ExportStage::Preflight`）
  として写像。
- `publish_options` —
  `replace_existing: bool` を `PublishMode::{Replace,CreateNew}` へ、
  旧 `sync_before_publish: true` を `Durability::FileSync` へ写像。
  `VisibilityRequirement::RequireAtomic` は `PublishOptions::new`
  既定。DirSync 強化は行わない（旧契約の忠実維持が本移行の境界）。
- 既定 verify = `reopen_check` + `VerifiedOutput` capture
  （`verified_slot` Mutex 機構は不変）。既定 publish =
  `SealedFile::publish`。
- receipt: `RenderReceipt` は非 Clone 化されたため、
  `frames_emitted`・`tail_evidence` を先に取り出してから
  `ExportReceipt.render` へ move する。

### 2.3 `verify.rs` — readback 独立 decode

`reopen_check(sealed: &mut SealedFile, facts, expect_pcm)`:

1. `sealed.open_readback()` → `ReadbackFile<'_>`（`Read+Seek`、
   sealed への `&mut` 借用・`!Send`）。
2. `ruviform_io_symphonia::with_borrowed_demuxer(&mut readback, f)`
   — RF407（ruviform #400）の bounded bridge。呼出し thread が
   serve loop で `&mut readback` の `read`/`seek` を配給し、
   worker thread 上の channel-backed proxy（`MediaSource` 適合）
   経由で probe 済み `&mut dyn Demuxer` を callback `f` へ貸す。
   借用は scope 内に閉じ、in-flight encoded byte は方向あたり
   ≤64KiB に bounded — 全量 `Vec` 複写・独自 WAV parser・
   decoder pump 複製・path reopen は使わない。
3. callback 内で `SourceReader::open(Box::new(demuxer), stream id,
   DecodeMode::Canonical)`（ruviform-media-io の
   `impl Demuxer for &mut D` reborrow port で `Box<dyn Demuxer + '_>`
   を構成）→ 従来どおり stream 数・rate・channel 数・frame 数を
   検証し、decoded planes + facts を `Send` な戻り値で呼出し側へ
   返す。`expect_pcm`（`&dyn Fn + Send`、非 Sync 参照）は
   callback に capture せず、呼出し thread で planes と比較する。
4. callback は caller-thread の仕事を同期 wait しない（bridge が
   caller を serve に占有させる設計制約）。worker panic は
   bridge の `resume_unwind` を `catch_unwind` で受け、従来どおり
   typed `MediaError`（`ErrorKind::Io`）へ写像する。

失敗は `RenderError::Verify` → `ExportStage::Verify` →
`ExportError::Render` として publish を起こさない（driver が
verify 前に publish しない型契約を強制）。

### 2.4 publish 失敗の typed 保全

`RenderError::Publish { source: Box<PublishFailure>, cleanup:
Option<Box<CleanupFailure>> }` は `ExportError::Render` で全体を
保持し、`stage_of` が `ExportStage::Publish` を返す。
`PublishFailureKind::DestinationExists` は driver が
`RenderError::DestinationExists` へ正規化 — SV preflight の
`DestinationConflict`（事前拒否）と `Publish` stage（publish 時
失敗）を混同しない。`Cancelled`/`Failed`/`Partial`/`Completed`、
`NotPublished`/`Published`/`Indeterminate` の分類は
`ExportOutcome`/`PublishEffect` がそのまま担う。

### 2.5 fault injection（`l2_e2e.rs`）

- `BoundedSink<'a>`/`FailFlushSink<'a>` は `&'a mut StagedFile` を
  保持し `Write`/`Seek` を delegate。`wrap_sink` closure は
  `Box<dyn MediaSink + 'a>` を返す。
- publish 失敗注入: `PublishFailure` に公開 constructor がなく、
  RF `mock` は `#[cfg(test)]` internal。fabricated `MediaError` は
  新 publish 契約では型として表現不能 — そのため destination を
  **directory** として `PublishMode::Replace` の atomic rename を
  実際に失敗させ、OS adapter 発行の実 `PublishFailure`
  （`NotPublished`）を使う。`SealedFile::publish` 委譲 hook 経由で
  `RenderError::Publish`・`effect() == NotPublished`・temp cleanup・
  dest directory 保全を pin する。
- `Indeterminate`: Consumer 側で誠実に生成不能（上記と同理由）。
  typed 分類・evidence 保全経路（`RenderError::Publish` 保持）は
  本試験で pin し、Indeterminate 自体の分岐は RF 側 mock 試験の
  責務として test-spec に明記する。

### 2.6 `pc_b.rs`（player parity test）

`RenderOutput` 直接構築を同契約へ更新。`DynMuxerCtor` 相当の
`for<'a>` boxed alias を file scope に定義し、
`PublishOptions::new(PublishMode::Replace, Durability::BestEffort)`
（旧 `sync_before_publish: false` の忠実写像）、
`publish: Box::new(SealedFile::publish)`、verify `None` を
`&mut SealedFile` 型で明示する。

### 2.7 依存・解決 SHA

- `synvelis-render` dependencies と `synvelis-player`
  dev-dependencies に `ruviform-io-file.workspace = true`。
- manifest は既定ブランチ追従の git 依存のまま、`Cargo.lock` の
  全 ruviform crate を combined 候補 SHA `27698cb1`（RF391 +
  RF407 merge）へ更新（Consumer 検証 tuple）。`sync-ruviform-ui`
  で vendor 再同期し `check-ruviform-rev` が lock 値を証明。
  恒久 pin・vendor コピーは置かない — 解除条件は RF391/RF407 が
  main へ merge され通常の `cargo update` で新 main HEAD へ
  再解決される時点。

## 3. 失敗境界の整理

| 事象 | 旧写像 | 新写像 |
| --- | --- | --- |
| staged create 失敗 | `ExportError::Io` (Preflight) | 同じ（`FileError`） |
| muxer 構築失敗 | `RenderError::Output` | 同じ |
| finalize 失敗 | `RenderError::Finalize` | 同じ |
| verify 失敗 | `RenderError::Verify` | 同じ（publish 不実行） |
| publish 失敗 | `RenderError::Publish(MediaError)` | `Publish{PublishFailure, cleanup}` typed 保持 |
| 既存 destination | Preflight `DestinationConflict` / Publish `DestinationExists` | 同じ 2 層 |
| cancel | `RenderError::Cancelled{phase}` → `Cancelled` outcome | 同じ |

## 4. RT/精度の非回帰

- 変更は output plumbing seam のみ。process loop・PCM 精度・
  tail/preroll/PDC 計算・`ExportPolicy`/`Project`/`Playback`/
  `Receipt`/`Frame`/保存済 Revision は untouched。
- `ruviform-io-file` は non-RT 契約 — render/export 経路も non-RT
  であり、RT context からの呼出し契約は不変。

## 5. 未決・引継ぎ

- `Durability::FileAndDirectorySync` への強化要否（本移行では
  `FileSync` 維持。Export の durability 要件は別 Issue で決定）。
- `Indeterminate` 決定的注入は RF mock 側の責務。
