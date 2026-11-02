---
id: "DISC-034Qmb93ek3YXoxaRPDBBD"
type: "discovery"
status: "resolved"
title: "L2 Discovery — Master Sequential Offline PCM WAV Export・Reopen/Verify・Safe Publish"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/25"
    - "https://github.com/scarecrow173/ruviform/issues/191"
    - "https://github.com/scarecrow173/ruviform/issues/198"
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
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# L2 Discovery — Master Sequential Offline PCM WAV Export・Reopen/Verify・Safe Publish

## Summary

Issue #25: L1 が固定した同一 Semantic Lowering を **Render Input Snapshot** として再利用し、
Master Mix を Ruviform の Sequential Offline Driver (`ruviform-audio::render`, rf#198) で
実 PCM WAV (`ruviform-io-wav`, rf#191) へ出力、独立 Reader で再 Open して内容を検証し、
Staged Publish で Destination を保護する。結論: Ruviform の汎用ドライバを Synvelis 側
RenderSpec/Recipe/Preflight/Job で包む構成が最短かつ契約一意。確度: 高
(実 API を `322a661` checkout で検読済み)。

## Exploration Goal

E2E-L2 の実装形態を確定する: (a) Render Input Snapshot の構成要素と固定点、
(b) Sequential Offline 駆動を Ruviform `render()` に委譲するか自前化するか、
(c) Clip 再生プロセッサを offline でどう供給するか、(d) PCM 変換/出力ポリシー/
Failure 分類の責務分割、(e) `cargo test --workspace --locked` で L2-01〜08 を
実測合格させる検証構成。

## Key Issues and Assumptions

- Ruviform `render()` は `PreparedRender::compile(&ValidatedGraph, &mut dyn NodeFactory, …)`
  で孤立 GraphRuntime+RtRuntime を構築し、全 install processor に
  `GraphProcessor::offline_support() == Verified` を preflight で要求する
  (default は `Unknown` → reject)。→ 検証済み: render.rs L527-600, runtime.rs L62-90。
- `RenderSpec` (ruviform) は `range_start/range_end`・`tail`・`job_start/job_end`・
  `callback_chunk`・`output_port` を frozen で持ち、emit は
  `position + j - total_latency ∈ [range_start, range_end+tail)`。
  非 Playing span は process するが emit しない (preroll 機構)。→ render.rs L647-717。
- 駆動は `RenderTransport` に収めた `TransportScheduler` (frame_base = sample rate,
  `DeliveryPolicy::Exact`, `BlockContract`) を `Seek→Play` staged command で回す。
  → transport_schedule.rs `TransportScheduler::new/request/install_plan/run_callback`。
- 出力は `RenderOutput { staged: StagedOutput, make_muxer, encoder, publish_options,
  verify, publish }`: Encoder→Packet→Muxer→`finish`→(verify hook)→`publish`。
  失敗時は temp を除去し既存 destination はバイト不変 (`dest_existed` +
  `replace_existing=false` → `DestinationExists`)。→ staged.rs L106-280。
- `ruviform-io-wav`: `WavOutputSpec`→`StreamInfo`、`make_encoder`、`open_boxed_muxer`、
  wire format U8/S16/S24/S32/F32、PCM int は saturate/NaN→0/±Inf→±full scale/
  round-half-away、dither 無し。`WavMuxer::finalize` が checked completion。
  → io-wav/lib.rs, encoder.rs, muxer.rs。
- `ruviform-exec`: `Progress` (completed/total, listener)・`CancellationToken`。
- Clip 再生を render で供給する手段: prefetch ring は L1 同様 pump が必要だが
  render ドライバは内部 node に per-block hook を持たない
  (InputFeeder は boundary input のみ)。→ render.rs L662-672。
- L2-03 は block size 非変性: `BlockContract.partition` は span を
  `[min,max]` の均等 chunk に割るため、stateful DSP は逐次 process すれば
  chunk 非依存に一致する。

## 既存資産の棚卸し (main マージ済み)

- `synvelis-player` (L1): `import_local` (staging→verify→blob→commit)、
  `resolver::prepare` (Verified Source→SourceReader+Prefetch)、`lowering::lower`
  (Track/Clip→GraphSpec+ClipBinding, mute/rate/anchor/負 offset 処理済)、
  `HeadlessDriver` (live 側・共有禁止対象)、`fixture` (決定的 PCM16 WAV,
  `fixture_sample(ch,frame)` 再構成可能)、`content_digest`/`blob_path`。
- `synvelis-project`: `Project` 集約・`ProjectController` (live_revision/saved/
  undo/redo/draw_raw_id)・`DirectoryBundleStore`。
- `synvelis-recording`: `staging::{publish_atomic, sync_parent_dir, detect_dir_sync,
  DirSyncCapability}`・`crc32`。
- Ruviform `322a661` (main HEAD, rf#198 merge commit): `ruviform-audio::render`
  (`PreparedRender`/`render`/`RenderSpec`/`RenderTransport`/`TailSpec`/`TailEvidence`/
  `InputFeeder`/`SilentFeeder`/`RenderOutput`/`RenderReceipt`/`RenderError`/
  `PreflightError`/`OfflineRenderSupport`)、`ruviform-media-io`
  (`Encoder`/`Muxer`/`MediaSink`/`StagedOutput`/`PublishOptions`/`Error`/`ErrorKind`)、
  `ruviform-io-wav` (`WavEncoder`/`WavMuxer`/`open_boxed_muxer`/`WavOutputSpec`/
  `wav_capabilities`/`capabilities`/`WavReceipt`)、`ruviform-exec`
  (`Progress`/`CancellationToken`/`OfflineJob`)、`ruviform-media-core`
  (`TransportScheduler`/`TransportCommand`/`TransportPlanBuilder`/`DeliveryPolicy`/
  `ScheduleBudget`/`BlockContract`/`EngineFrame`/`MediaTime`/`event_engine_frame`)。

## ギャップ分析

| 要求 | 既存 | 不足 |
| --- | --- | --- |
| Render Input Snapshot | L1 `lower`/`resolver` が同一 semantic を供給 | Snapshot 型 (revision・digest 集合・tempo・range・channel/rate・destination policy) |
| Preflight | lowering `SkippedClip` 分類 + resolver typed error | Missing Source/Unsupported/Unknown Tail/Invalid Range/Output Capability/Destination Conflict を Output 前に集約検出 |
| Sequential Engine | ruviform `render()` + `PreparedRender` | Synvelis RenderSpec→ruviform RenderSpec/Transport 写像、offline Verified な clip renderer |
| Deterministic DSP (L2-03) | graph sum (master sink unbounded in) | gain/1-pole/delay processor (offline Verified、block 非変) |
| PCM Output | ruviform-io-wav | Output Recipe (wire format・channel/rate・clipping 方針) と muxer/encoder 構成 |
| Reopen/Verify | L1 import verify の reader 利用 | 独立 Reader で frame/channel/rate/PCM 比較する verify hook |
| Safe Publish | `StagedOutput`/`PublishOptions` | Overwrite policy・failure 時 temp 破棄・既存 destination/raw source/project 保護の検証 |
| Job/Failure | `Progress`/`CancellationToken`/`RenderError` | Synvelis Job 状態 (Failed/Partial/Completed 区別、reopen 前は Completed 非) |

## 依存 Revision 決定

`AGENTS.md` は Ruviform を既定ブランチ追従 git 依存 + `Cargo.lock` pin と定める。
rf#191 (`2089ca0`) と rf#198 (`322a661`, main HEAD) を含むよう lock を `1305538` から
`322a661` へ再生成済み。`ruviform-io-wav`/`ruviform-exec` を workspace 依存に追加。
(浮動 SHA の恒久的ハードコード禁止 → lock pin が正本)

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| A. Ruviform `render()` 委譲 + Synvelis RenderSpec/Recipe/Job | rf#198 契約を直接再利用・PDC/preroll/tail/segmentation が実装済・L2-07 証拠が自然 | ruviform API 形状への適合が必要 | **yes** |
| B. Synvelis 独自 sequential driver | 制御自由 | rf#198 と重複・同一 Time/Graph 契約の二重実装・保守負債 | no |
| C. Clip 再生 = `ClipPlayer`+Prefetch 再利用 | L1 資産そのまま | render 内部 node に pump hook 無し・underrun 必至・offline_support Unknown | no |
| D. Clip 再生 = boundary InputFeeder 経由 pump | feeder hook は使える | ダミー boundary input が必要・不自然な責務混同 | no |
| E. Clip 再生 = prepare 時に window decode → メモリ player (`ClipRenderer`) | `Verified` を正直に表明可・process 内 I/O 無し・決定的・実装小さい | 長尺 source でメモリ増 (L2 範囲では fixture 規模) | **yes** |

## Tentative Conclusions and Hypotheses

- **confirmed**: 新 crate `synvelis-render` (依存方向 `synvelis-render → synvelis-player/
  synvelis-project`・player 逆依存なし)。live `HeadlessDriver` と processor 実体を共有しない
  (Render Input Snapshot は値スナップショット + 新規 compile)。
- **confirmed**: Clip node は `ClipRenderer` (decode 済 canonical f32 planar を保持し
  window 外は silence)。`offline_support() = Verified`。DSP processor (gain/1-pole/delay)
  も Synvelis 側で `Verified` を返す deterministic 実装。
- **confirmed**: Preflight は Output 開始前に一括: missing source・rate mismatch・
  unsupported codec・invalid range・unknown tail・output capability (wav_capabilities
  non-支持)・destination conflict。未対応 plugin は L2 範囲外だが「bypass して Exact
  成功」は出さない設計 (plugin-capable node 型が存在しないため構造的に不可能)。
- **confirmed**: Reopen verify は独立 reader (symphonia demux→SourceReader) で
  `staged.temp_path()` を開き `OutputFacts` と decoded PCM を比較 — publish 前実行、
  失敗は `RenderError::Verify` で temp 破棄。
- **hypothesis**: Publish policy は `replace_existing` flag のみ — 既定 false
  (DestinationExists は preflight でも検出)。rename 経由の overwrite は policy=replace。
- **hypothesis**: Job 状態 = `Failed`(preflight/typed error) / `Cancelled` /
  `Partial`(render 途中失敗・staging 保持 or 破棄) / `Completed`(publish+receipt 後のみ)。

## Open Questions

- `run_callback` の span emission 順と `BlockContract` partition で、event 無し単純
  Playing render の切り分けが仕様通りか → 実装時に実測 (L2-02/L2-03)。
- `SourceReader` の direct seek が decode-to-memory prepare に使えるか →
  `read_planar` の sequential 性を確認 (L1 resolver が同 path で verify 済)。

## Promotion Candidates

- [x] spec-doc needed: L2 RenderSpec/Preflight/Sequential/PCM Output/Job 契約と L2-01〜08
- [x] adr-doc needed: render slice 境界 (新 crate)・Ruviform driver 採用・decode-to-memory・staged publish 方針

## Sources and Relations

- Synvelis #18/#19/#20/#21/#22/#23 (設計正本), #24/#25, PR #29-#33 (マージ済前提)
- Ruviform #191/#195/#196/#197/#198, PR #227/#229/#231/#235
- `docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md`
  (MR-001〜028), playback-mixer (EX-001〜020), time-transport (TT-001〜024)

## Evidence

- 2026-11-02: `ruviform-audio/src/render.rs` @322a661 全文検読 (PreparedRender/
  RenderSpec/RenderTransport/RenderSink/physical_call/emit_run/run_pipeline/
  render/preflight)。
- 2026-11-02: `ruviform-media-io/src/{staged,encode,mux,sink}.rs`・
  `ruviform-io-wav/src/{lib,encoder,muxer,convert,wav}.rs`・
  `ruviform-media-core/src/transport_schedule.rs` (Scheduler/Plan/Span/BlockContract/
  event_engine_frame)・`ruviform-exec/src/{offline,job}.rs` 検読。
- 2026-11-02: `crates/synvelis-player/src/*` 全件検読 (import/resolver/lowering/
  processor/driver/fixture/error)。
- 2026-11-02: Cargo.lock `1305538 → 322a661` へ再生成、`mise run check` PASS。
