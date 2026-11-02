---
id: "DESIGN-034QmfChvqgB632y8fAydk"
type: "design"
status: "approved"
title: "L2 Design — Render Snapshot・Preflight・Sequential Render・WAV Output・Reopen Verify・Safe Publish"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/25"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  implemented-by: []
  depends-on:
    - "docs/adr/l2-render-slice-boundaries.md"
    - "docs/designs/l1-local-wav-verified-playback-design.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
    - "docs/adr/l2-render-slice-boundaries.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# L2 Design — Master Sequential Offline PCM WAV Export

## 1. 構成と責務

新 crate `synvelis-render` (依存: `synvelis-player`, `synvelis-project`,
`synvelis-recording` staging helpers, ruviform-*)。依存方向は L1 と同じ
一方向、player への逆依存なし。

```text
ExportRequest (RenderSpec + OutputRecipe + Destination + PublishPolicy)
  → RenderSnapshot::freeze(&Project, media_root)
      → preflight (missing/unsupported/range/tail/capability/destination)
      → lower (L1 同一 semantic) + prepare clips (decode-to-memory)
      → PreparedRender::compile (offline_support gate)
      → ruviform render() (TransportScheduler Seek→Play, Exact)
      → Encoder→Drain→Muxer finalize→close/sync
      → reopen verify hook (独立 reader)
      → StagedOutput::publish (policy)
  → ExportReceipt { RenderReceipt + verify facts + snapshot ids + capability evidence }
  / ExportError { stage-typed } / Cancelled
```

## 2. Render Input Snapshot

`RenderSnapshot` (frozen non-RT): `project_revision: ProjectRevision`・
`saved_revision: Option<ProjectRevision>`・`sources: BTreeMap<SourceId, SourcePin>`
(`{asset_id, content_digest, state}`)・`tempo_revision`・clips (全 `Clip` 値
コピー + lane/track 参照)・`engine: EngineConfig`・`range [start,end)` frames・
`tail: TailSpec`・`destination`・`publish_policy`。freeze は `&Project` から
値コピー; Live Revision/Last Saved は別フィールドに併記 (区別の証跡)。
Export は snapshot のみを読み、live `ProjectController` には触れない。

## 3. Preflight (Output 開始前に一括)

`preflight(snapshot, resolved) -> Result<(), PreflightError>`:

- `MissingSource`: clip が参照する source 未登録/unresolved/blob 不在/digest 不一致
- `UnverifiedSource`: state < Verified (L1 resolver と同じゲート)
- `UnsupportedCodec`: canonical が PCM decode 不可能 / 非対応 wire format
- `RateMismatch`: source rate ≠ engine rate (implicit resample なし・L1 同様 skip ではなく明示失敗として列挙)
- `UnknownTail`: tail 指定が `None`/Known/Estimated 以外、または負値・表現不能
- `InvalidRange`: `end <= start`、range が表現不能、engine rate 0
- `InsufficientCapability`: `ruviform_io_wav::wav_capabilities()` が要求
  wire format/layout を支持しない
- `DestinationConflict`: `replace_existing=false` で destination 既存
  (`StagedOutput::dest_existed` + render preflight `DestinationExists` にも二重検出)
- `OfflineUnsupported`: processor が `Verified` 非報告 (ruviform preflight 経由 —
  mock processor で L2-04 を実測)

## 4. Sequential Engine 写像

`RenderSpec` (Synvelis) → `ruviform_audio::render::RenderSpec`:
`range_start/end` = snapshot range (position 単位, sample frames)、
`tail`、 `job_start = 0`・`job_end = range_end + tail + total_latency + preroll`。

Transport: `TransportScheduler::new(TimeBase::from_rate(rate), contract,
DeliveryPolicy::Exact, ScheduleBudget)` に `Seek(position = range_start - preroll)` +
`Play` を `due = job_start` で stage。`RenderTransport { scheduler, commands, plans }`。

- **Preroll**: `preroll` frames 分 position を range_start 前に戻して Play 開始。
  emit は `[range_start, range_end+tail)` のみ — process するが emit しない
  (issue の「指定範囲を誤って膨張させない」)。
- **PDC**: emit position = `position - plan.total_latency` (render 側が処理)。
- **Clip window**: L1 `lower` が `engine_start/engine_len/source_start` を算出
  (負 anchor clamp 済 — project 負 pre-roll は anchor の負値として表現)。
- **Frame boundaries**: `BlockContract { min: 1, max: block_frames }` で
  partition が任意 span を block 幅に割る — L2-03 は 64/128/256 で block 非変。
- **ClipRenderer**: `Vec<Vec<f32>>` (window span の canonical decode) +
  `ClipWindow`。process は `next_engine_frame` 内部 cursor で window 写像 —
  render は job_start から連続なので counter は deterministic。
- **DSP processors**: `GainProc { gain_db }`, `OnePoleProc { a0,b1,z1 }`,
  `DelayProc { ring, delay }` — 全て `offline_support() = Verified`、
  `latency()` 正直返却 (DelayProc は delay 値)。

## 5. PCM Output

`OutputRecipe { wire: SampleFormat (S16 既定), layout, rate }` →
`WavOutputSpec::stream_info()`。`make_encoder(&info)` + `open_boxed_muxer(file,
&[info])` を `RenderOutput` に接続。変換方針は io-wav 固定 (saturate/NaN→0/
±Inf→±FS/round-half-away、dither 無し) を receipt に記録。

## 6. Reopen Verify / Safe Publish

`verify` hook: `staged.temp_path()` を `ruviform_io_symphonia::open_demuxer` +
`SourceReader::open(Canonical)` で開き、`OutputFacts {frames_emitted, layout,
sample_rate}` と decoded を全 frame decode して比較 (期待 PCM は caller が
供給する比較関数 — E2E では fixture_sample 再構成)。`PublishOptions`:
`replace_existing` 既定 false。失敗時 temp 破棄・destination 不変は
`StagedOutput`/`render()` が保証。

## 7. Job / Failure

`ExportOutcome = Completed { receipt, verified, capability } |
Failed { stage: ExportStage, error: RenderError|PreflightError|ResolveError } |
Cancelled { phase }`。ExportStage: `Preflight|Prepare|Compile|Process|Drain|
Finalize|Verify|Publish`。Partial 状態は receipt を持たず
Completed としない (verify 前の publish 不可)。Cancel は `CancellationToken` —
render が phase 境界で `Cancelled` を返す。`Progress` は processed engine
frames。

## 8. Capability Evidence (L2-07)

`ExportReceipt.capability`: `ruviform_io_wav::wav_capabilities()`
(encodable wire formats/layouts)・`PreparedRender::capabilities()`
(per-node OfflineRenderSupport map)・`plan.total_latency`・
`tail_evidence`・ruviform crate versions (Cargo.lock pin `322a661`)。

## 9. 追跡

MR-012 (Local Mix→PCM WAV), MR-013 (unsupported codec), MR-014 (export range/
half-open), MR-015 (preroll/PDC/tail), MR-016 (stateful sequential, no chunk
parallel), MR-018 (int PCM conversion), MR-019 (drain/finalize failure →
recoverable, 旧 file 保護), MR-020 (render snapshot vs live edit), MR-021
(unsupported plugin → preflight), MR-023 (cancel/exit → terminal state),
MR-028 (safe reject/path);EX-001 (source→output deterministic),
EX-016 (offline vs live), EX-018 (master export);TT-005 (negative preroll),
TT-008 (requested/applied frames)。
