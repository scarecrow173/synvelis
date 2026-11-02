---
id: "IMPL-L2MasterSequentialOfflineWavExport"
type: "impl"
status: "completed"
title: "E2E-L2 Master Sequential Offline PCM WAV Export・Reopen/Verify・Safe Publish"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-render/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-render/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-render/src/spec.rs"
      - type: "file"
        path: "crates/synvelis-render/src/snapshot.rs"
      - type: "file"
        path: "crates/synvelis-render/src/error.rs"
      - type: "file"
        path: "crates/synvelis-render/src/preflight.rs"
      - type: "file"
        path: "crates/synvelis-render/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-render/src/processor.rs"
      - type: "file"
        path: "crates/synvelis-render/src/export.rs"
      - type: "file"
        path: "crates/synvelis-render/src/verify.rs"
      - type: "file"
        path: "crates/synvelis-render/src/job.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/l2_e2e.rs"
      - type: "file"
        path: "docs/discovery/l2-master-sequential-offline-wav-export-discovery.md"
      - type: "file"
        path: "docs/specs/l2-master-sequential-offline-wav-export.md"
      - type: "file"
        path: "docs/adr/l2-render-slice-boundaries.md"
      - type: "file"
        path: "docs/designs/l2-master-sequential-offline-wav-export-design.md"
      - type: "file"
        path: "docs/plans/l2-master-sequential-offline-wav-export.md"
      - type: "file"
        path: "docs/test-specs/l2-master-sequential-offline-wav-export.md"
      - type: "file"
        path: "docs/tasks/l2-task1-workspace-crate-deps.md"
      - type: "file"
        path: "docs/tasks/l2-task2-snapshot-preflight.md"
      - type: "file"
        path: "docs/tasks/l2-task3-render-core.md"
      - type: "file"
        path: "docs/tasks/l2-task4-verify-publish-job.md"
      - type: "file"
        path: "docs/tasks/l2-task5-acceptance-tests.md"
      - type: "file"
        path: "docs/impl/ir/l2-master-sequential-offline-wav-export.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-player/src/lowering.rs"
      - type: "file"
        path: "crates/synvelis-player/src/import.rs"
      - type: "file"
        path: "crates/synvelis-player/src/error.rs"
      - type: "file"
        path: "crates/synvelis-player/src/resolver.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/l1_e2e.rs"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/discovery/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/l2-master-sequential-offline-wav-export.md"
    - "docs/designs/l2-master-sequential-offline-wav-export-design.md"
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
    - "docs/adr/l2-render-slice-boundaries.md"
    - "docs/discovery/l2-master-sequential-offline-wav-export-discovery.md"
    - "docs/test-specs/l2-master-sequential-offline-wav-export.md"
    - "docs/tasks/l2-task1-workspace-crate-deps.md"
    - "docs/tasks/l2-task2-snapshot-preflight.md"
    - "docs/tasks/l2-task3-render-core.md"
    - "docs/tasks/l2-task4-verify-publish-job.md"
    - "docs/tasks/l2-task5-acceptance-tests.md"
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

# E2E-L2 Master Sequential Offline PCM WAV Export・Reopen/Verify・Safe Publish

## Summary

Frozen `RenderSnapshot`（live/saved revision・tempo revision・source digest
を記録した clone）→ static/canonical preflight（range/tail/destination/
capability/geometry/source registration・PCM・rate）→ L1 semantic
lowering 再利用＋master-bus insert chain → fresh render processors →
Ruviform `PreparedRender` + 実 sequential `render()` + `StagedOutput` →
実 io-wav encoder/muxer → 独立 `SourceReader` reopen verify（rate/
channel/frame count/optional PCM）→ safe publish → typed outcome
（Completed/Failed/Cancelled）の縦断 slice を実装した。Completed は
verify+publish 後にのみ構成され、partial/cancelled は決して publish
しない。Live playback の processor/plugin instance とは一切共有しない。

## Context

Issue #25。前提: L1（PR #33 `synvelis-player`）、Ruviform PR #229
rf#191（io-wav encoder/muxer・MediaSink・StagedOutput）、PR #235 rf#198
（`ruviform-audio::render` sequential offline・PreparedRender・
offline_support gate）。依存 rev は `1305538c` →
`322a661ac0177fc98f799b07ae591c98eb2ec845`（rf#198 含む default HEAD）
へ bump し `Cargo.lock` に pin。

## Implementation

### Changes

- `crates/synvelis-render`（新規 crate、workspace member）:
  - `snapshot.rs`: `RenderSnapshot::freeze` — project を clone し
    live/saved/tempo revision と `source_id → content_digest` を記録。
    controller・undo/redo に触れない。
  - `spec.rs`: `ExportRequest`（engine geometry・range・tail・preroll・
    destination・overwrite policy・wire format・inserts・max_block・
    clip_support）・`TailPolicy`・`Insert`（`Gain`/`OnePole`/`Delay`、
    `tail_frames`/`declared_latency` helper）・`OutputHooks`（fault-
    injection seams）。
  - `preflight.rs`: `preflight_static`（range・tail 分類・destination
    conflict・WAV capability・mono/stereo geometry・`max_block`/insert
    magnitude bound 1<<20・tail-carrying insert vs `TailPolicy::None`
    の整合・wire-format 幅による RIFF 上限・source registration/
    Verified/asset）と `preflight_canonical`（source rate == engine
    rate）。
  - `prepare.rs`: `open_sources` — resolver と同じ verified gate を
    同一 fd で streaming digest（新 `content_digest_stream`、fwd+bwd
    CRC32 を 64KiB chunk で）+ symphonia demux + codec 先出し check
    + `SourceReader::open`（digest↔decode TOCTOU なし、全 blob の
    二重読込なし）。`decode_window` は render が実際に処理する span
    `[play_pos, end+tail+latency)` ∩ window だけを decode-to-memory
    （`base` 付きで返し、range 外の長尺 clip を materialize しない）。
    `cancel` は per-source・per-chunk で確認。
  - `processor.rs`: `ClipRenderer`（`pcm_base` offset 付き window
    server、call counter = sequential drive の position 契約）と
    決定的 DSP `GainProc`/`OnePoleProc`（per-channel state + declared
    latency）/`DelayProc`（per-channel ring、`frames=0` は identity）。
  - `export.rs`: orchestration — skipped clip は `Muted` 以外全て
    `UnrenderableClip` で refuse、zero clip node は `NothingToRender`、
    codec 非 PCM は `ResolveError::UnsupportedCodec` を Preflight
    stage の `UnsupportedCodec` に remap（decodable かどうかで
    class が揺れない）、`total_latency > declared` は refuse、
    transport = `Seek(play_pos)+Play` 一段、emit `[start, end+tail)`、
    `StagedOutput` + verify-before-publish。
  - `verify.rs`: 独立 reopen は spec 記載どおり symphonia demux +
    `SourceReader`(Canonical) で全 frame decode、rate/channel count/
    frame count を `OutputFacts` と照合、`expect_pcm` で全 sample
    比較（layout variant は跨いでよい — unpositioned WAV は
    `Custom` として reopen される）。
  - `job.rs`: `ExportJob`（ruviform `Progress`/`CancellationToken`）、
    `ExportOutcome`、`ExportStage`、`ExportReceipt`（render receipt +
    snapshot ids + capability evidence + `VerifiedOutput`）。
- `synvelis-player` 側の変更（L1 共有層の拡張・全て後方互換）:
  - `lowering.rs`: `lower_with_inserts(project, canonical_of, engine,
    insert_count)` — master-bus insert 分を sink の前に挿入。
    `lower` は `insert_count=0` に delegate。layout variant が異なる
    場合は必ず `ChannelMap` を明示（count 一致でも `Custom` なら
    必須）。`lead` の `i64::MIN` overflow を checked_neg で解消。
    stereo→mono が sum ではなく channel-0 select であることを
    doc/comment で正直に記述。
  - `import.rs`: `content_digest_stream(&mut File)` — streaming 版
    digest（同一結果、bounded memory、同一 fd で decode まで保持）。
  - `error.rs`: `ResolveError::UnsupportedCodec` に `source_id`、
    `ResolveError::Cancelled` を追加。
  - `resolver.rs`: `UnsupportedCodec` 構築 2 箇所に `source_id`。
  - `tests/l1_e2e.rs`: `Lowered` の新 `inserts` field に `Vec::new()`
    を追記（構造体 literal 追従のみ）。
- `Cargo.toml`/`Cargo.lock`: `synvelis-render` member 追加、ruviform
  git deps を `322a661` へ、`ruviform-io-wav`/`ruviform-exec` dep 追加。

### Decisions During Implementation

- **decode-to-memory は rendered span に限定**: clip の全 window では
  なく `[play_pos, end+tail+declared_latency) ∩ window` のみ decode。
  fixture-scale 前提の L2 設計意図（decode-to-memory canonical PCM）
  を守りつつ、長尺 source + 短い range での memory blowup を防ぐ。
  `total_latency > declared` は compile 後に refuse するので bound
  が足りないまま silent output にはならない。
- **`Insert::Delay{frames:0}` は identity**: 従来 `max(1)` ring が
  1-sample shift を量産していた — spec の `y[n]=x[n−frames]` どおり
  frames=0 は pass-through に修正し model も同じく修正。
- **`TailPolicy::None` の整合 check**: `tail_frames`（Delay のみ
  provable bounded tail — OnePole の declared latency は PDC 補正で
  range 内に drain され、自然減衰は unbounded）が >0 の chain に
  None を許すと「provably no tail」evidence が嘘になるため
  `InvalidRange` で refuse。
- **codec gate は open 時に一本化**: 非 PCM は `SourceReader::open`
  前に `ResolveError::UnsupportedCodec`（`source_id` 付き）とし、
  export が Preflight stage の `UnsupportedCodec` に remap —
  decoder 有無で `Corrupt` と `UnsupportedCodec` が揺れない。
- **独立 reopen は `SourceReader`**: spec が名指す reader API に揃え
  packet 手動 pump を廃止（同時に code が単純化）。
- **zero clip node は refuse**: all-muted/clip-free は ruviform の
  opaque graph error に落ちる前に `NothingToRender` で明示。
- **cancel は prepare でも**: `open_sources`/`decode_window` に
  `CancellationToken` を通し、最も長い pre-render phase でも即座に
  `Cancelled{phase:"prepare"}` へ落ちる。

### Review Outcomes (two independent reviewers)

Reviewer A の P1 2件（`lowered.skipped` 未検査で silent drop 可能、
impl record 不在）と P3 6件（codec stage、channels coercion、
SourceReader 未使用、doc lie、unused dep）を修正した。Reviewer B の
adversarial 15件のうち、確認済みの実害（decode 無制限、Delay{0}、
tail 整合、TOCTOU、zero-clip、RIFF 8B 固定、magnitude bound、
lead overflow、inline comment、codec class 揺れ、cancel gap）は全て
修正済み。以下は検証済みのうえ修正しない判断：

- `ClipRenderer` の call-counter 位置導出（B#5）: `RtContext` は空
  token で transport position API が存在しない。export が stage する
  program は単一 `Seek`+`Play` span に限定されるため契約内 — 制約と
  して記録。
- default verify が structural のみ（B#13）: `expect_pcm` opt-in は
  設計意図（hook seam）、receipt の `pcm_checked` flag が正直に区別。
- publish `InvalidValue → DestinationExists` remap（B#14）: ruviform
  render.rs の variant 選択 — Synvelis 側で重複実装せず報告対象に。
- 解析 model の impl 式との結合（B#15）: model は documented
  semantic の独立再導出 + wire-level reopened PCM との直接比較があり、
  Delay{0} mirror のみ実害として修正。

## Acceptance Matrix Traceability

| AC | 検証 |
| --- | --- |
| L2-01 master export・独立 reopen・PCM 一致・publish | `l2_01_master_export_reopen_pcm_match` |
| L2-02 clip 境界/gap/muted/負 anchor/range edge/preroll clamp・skip 明示 | `l2_02_clip_boundaries_gaps_muted`・`l2_02_range_edges_and_preroll_clamp`・`l2_02_unresolvable_anchor_refused`・`l2_02_all_muted_refused` |
| L2-03 insert chain・stateful DSP・block 不変 | `l2_03_stateful_inserts_block_invariant`・`l2_03_delay_zero_is_identity` |
| L2-04 PDC・preroll state・known/estimated tail・unknown tail/unsupported support 明示拒否 | `l2_04_pdc_realigns_not_inflates`・`l2_04_preroll_is_state_semantics`・`l2_04_known_tail_captures_delay`・`l2_04_unknown_tail_and_support_rejected` |
| L2-05 destination policy・sink/writer/flush/verify/publish/cancel で partial≠Completed・temp cleanup・旧 destination 保持 | `l2_05_destination_policy`・`l2_05_fault_injection_partial_never_completed` |
| L2-06 snapshot freeze・live/saved/digest/undo 不変 | `l2_06_snapshot_freeze_and_immutability` |
| L2-07 backend capability・node support・latency・tail evidence receipt | `l2_07_capability_evidence_on_receipt` |
| L2-08 deterministic fixture/output・toolchain/exit code/hash/未計測記録 | 本書 Validation 節 + `l2_01`/`l2_07` の receipt 検査 |

MR/EX/TT 系は test-spec の cross-reference 表に従い対応（未実装を
合格とは表記しない）。

## Validation

全て exit code 0 で実測（mise 経由ではなく同等 cargo lane — 本
worktree の `mise.toml` は同一 task を定義していないため直接実行を
記録する）:

- `cargo fmt --all -- --check`: clean。
- `cargo check --workspace --all-targets`: clean。
- `cargo clippy --workspace --all-targets -- -D warnings`: clean。
- `cargo test --workspace --locked`: **121 pass / 0 fail**（L2 suite
  **15/15**、L1 suite 10/10、vs_a–d 全 suite 維持）。
- `cargo build --workspace --locked`: clean。
- `git fetch origin main` → `git merge origin/main`: Already up to date。
- Toolchain: `rustc 1.99.0 (b940084d7 2026-09-28)`（workspace
  `rust-version = "1.89"` 要件を満たす）。
- Ruviform lock: `git+…ruviform.git#322a661ac0177fc98f799b07ae591c98eb2ec845`。
- Headless E2E: `cargo test -p synvelis-render --test l2_e2e`（実
  symphonia/io-wav 経路・GUI 不要）。

Deterministic fixture / output evidence（同一入力・反復実行で同一
sha256 — `wav_pcm16` fixture bytes と published WAV）:

| file | channels/frames | sha256 |
| --- | --- | --- |
| fixture mono 4800 | 1ch | `23b5e3f4112c32969a25a36f76bce0a6d462069c690e6c8653464130bb708e34` |
| fixture stereo 4800 | 2ch | `9ffc2f7b2f5c607d6951da638e4ee7a4ae8801878e165b899e226c5022b9b08e` |
| export mono 512+64tail | 1ch | `6dcca5e8ae158324f30655fd68cf6e7900e2f2f9c1e3d556ef26d059b9317ec1` |
| export stereo | 2ch | `3409ae06801237f2e09679df9ae4107c39b8c1bd2b453a8662e7df864c9e3eb9` |
| export（追加 fixture） | — | `6c46aef08c565171254e56a1c9bf99a818a2c7408376f0f14d5b388451bf7648` |

未計測・未実施: cancellation は `prepare`/`process` phase のみ実測
（upstream の drain/finalize/verify/publish cancel point は本層では
未注入）、publish-stage の `DestinationExists` 競合は upstream
`staged.rs` の再検査に委譲（preflight 側は実測済み）、`wire_apply`
model の S16 quantize は clamp のみ（全 S16 テストは i16-grid 上で
exact）、`Estimated` tail は evidence 記述まで（PCM 内容は Known
と同型のため未別検）。

## Related Experiments

なし（探索的 experiment log 対象の試行は発生しなかった）。

## Risks

- `ClipRenderer` の位置は call counter 由来（`RtContext` に位置 API
  がない）。単一 `Seek`+`Play` 以外の transport program を将来 stage
  する場合は position feed が必要 — 現 contract 内の制約として明記。
- decode-to-memory は rendered span ∩ window まで縮小済みだが、
  range 自体が巨大な request は依然 O(range) — fixture-scale L2 の
  想定内。`max_block`/insert bound は 1<<20 で refuse 済み。
- `OnePole` の自然減衰 tail は unbounded — `TailPolicy::None` 選択で
  truncate されるのは caller の明示意思（`tail_frames` は
  provable bounded tail のみ数える）。

## Known Limitations / Report（Ruviform 側の観測・起票候補）

Synvelis 側の変更は本 repo に閉じ込め、以下は回避・重複実装せず
報告対象として記録する。

- `StagedOutput::publish` 系の `ErrorKind::InvalidValue` が
  `RenderError::DestinationExists` に remap される（render.rs
  publish stage の kind→variant 対応）— 独自 publish hook が
  InvalidValue を返すと conflict と誤記録される（variant 分離の
  upstream 提案候補）。
- `GraphProcessor`/`RtContext` に transport position の取得経路が
  なく、processor は自前の call counter で位置を導出するしかない。
  non-contiguous drive（pause span・mid-render seek）では
  authoritative position と乖離し得る — position feed を ctx 経由で
  渡す upstream 提案候補。
- `SourceReader::hold_frame` は seek target が packet gap に落ちると
  landing frame 長に clamp する — 連続 PCM-WAV では未到達だが
  gap ある container では内容が早く着地し得る（upstream 観測）。

## Follow-ups

- L2 の `declared_latency` 事前一致と `total_latency` の事後検証は
  現状 `total > declared → refuse` — OnePole 以外の latency 宣言
  processor が増えた場合は bound 計算を再確認。
- cancellation の drain/finalize/verify/publish 各 phase の
  fault-injection coverage（upstream に hook は存在）。
- stereo→mono downmix（現状 channel-0 select、sum 不可）を正式な
  mix semantic として扱う issue（map 表現力の upstream 制約込み）。

## Rollback

`crates/synvelis-render` 削除・workspace `Cargo.toml` member 除去・
ruviform dep を `1305538c` へ revert・`Cargo.lock` 再生成・
`synvelis-player` への additive 変更（`lower_with_inserts`/
`content_digest_stream`/`ResolveError` variant/`inserts` field/
l1_e2e literal）を revert すれば L2 slice を除去できる。L1 機能は
`lower` 経路が `insert_count=0` delegate のため additive のみで影響
なし（既存 test 全 pass を実測済み）。
