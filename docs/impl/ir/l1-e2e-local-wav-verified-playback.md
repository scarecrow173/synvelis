---
id: "IMPL-L1LocalWavVerifiedPlayback"
type: "impl"
status: "completed"
title: "E2E-L1 Local WAV Safe Import・Verified Media・Track/Clip・Headless Generic Graph Playback"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/error.rs"
      - type: "file"
        path: "crates/synvelis-player/src/fixture.rs"
      - type: "file"
        path: "crates/synvelis-player/src/import.rs"
      - type: "file"
        path: "crates/synvelis-player/src/resolver.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lowering.rs"
      - type: "file"
        path: "crates/synvelis-player/src/processor.rs"
      - type: "file"
        path: "crates/synvelis-player/src/driver.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/l1_e2e.rs"
      - type: "file"
        path: "docs/discovery/l1-local-wav-verified-playback-discovery.md"
      - type: "file"
        path: "docs/specs/l1-local-wav-verified-media-graph-playback.md"
      - type: "file"
        path: "docs/adr/l1-player-slice-boundaries.md"
      - type: "file"
        path: "docs/designs/l1-local-wav-verified-playback-design.md"
      - type: "file"
        path: "docs/plans/l1-e2e-local-wav-playback.md"
      - type: "file"
        path: "docs/test-specs/l1-e2e-local-wav-verified-playback.md"
      - type: "file"
        path: "docs/tasks/l1-task1-project-model-extensions.md"
      - type: "file"
        path: "docs/tasks/l1-task2-player-crate-fixture.md"
      - type: "file"
        path: "docs/tasks/l1-task3-import-resolver.md"
      - type: "file"
        path: "docs/tasks/l1-task4-lowering-processor-driver.md"
      - type: "file"
        path: "docs/tasks/l1-task5-acceptance-tests-verification.md"
      - type: "file"
        path: "docs/impl/ir/l1-e2e-local-wav-verified-playback.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-project/src/id.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/tempo.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/commit.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/l1-e2e-local-wav-playback.md"
    - "docs/designs/l1-local-wav-verified-playback-design.md"
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
    - "docs/adr/l1-player-slice-boundaries.md"
    - "docs/discovery/l1-local-wav-verified-playback-discovery.md"
    - "docs/test-specs/l1-e2e-local-wav-verified-playback.md"
    - "docs/tasks/l1-task1-project-model-extensions.md"
    - "docs/tasks/l1-task2-player-crate-fixture.md"
    - "docs/tasks/l1-task3-import-resolver.md"
    - "docs/tasks/l1-task4-lowering-processor-driver.md"
    - "docs/tasks/l1-task5-acceptance-tests-verification.md"
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

# E2E-L1 Local WAV Safe Import・Verified Media・Track/Clip・Headless Generic Graph Playback

## Summary

Local PCM WAV → nonRT staging → size/hash/container/codec/frame/channel
verify → measured durability → digest-addressed Verified Blob →
ExpectedRevision commit → save/reload → stable Source/Asset/Clip
identity → verified resolver → timeline lowering → generic typed
graph → deterministic headless playback の縦断 slice を実装した。
未検証の Path は Source になれず、全 Project mutation は
`ProjectController` の検証済 Command のみで、Graph 層に
Project/Track/Clip 公開型は一切導入していない。

## Context

Issue #24。前提: VS-A #13（PR #29）・VS-B/C/D #14–#16（PR #30–#32）、
Ruviform PR #225/#227/#231（Typed Graph IR / PreparedPlan+RtRuntime /
media-reader）。doc chain: discovery → spec → ADR → design → plan →
test-spec → 5 task。依存は `ruviform` git dep・Cargo.lock pin
`1305538caeab62e6f491a1d9a2497a1f76909d04`（PR #231 以降を含む
default branch 追従ポリシーの解決結果）。

## Implementation

### Changes

- `synvelis-project/src/id.rs`: `ImportId`（import manifest の安定
  identity）を追加。
- `synvelis-project/src/project.rs`: `applied_imports` registry
  （`BTreeMap<ImportId, AppliedImport { source_id, asset_id, digest }>`、
  serde 永続化対象）を追加し、同一 manifest の再 commit を
  `AlreadyApplied` に分類。
- `synvelis-project/src/ops.rs`: `Clip.muted`・`ProjectOp::SetClipMuted`
  を追加。全 state write は引き続き validated Command のみ。
- `synvelis-project/src/tempo.rs`: Ruviform 新 music API へ追従
  （`TempoEvent::at_tick`/`TimeSignatureEvent::at_tick`/
  `tick_to_media(.., TimeBase)`/`media_to_tick(.., Ppqn)`/
  `tick.tick().value()`）。
- `synvelis-recording/src/commit.rs`: `Clip` initializer に
  `muted: false` を追加（既存録音経路の挙動は不変）。
- `crates/synvelis-player`（新規 crate）:
  - `fixture.rs`: 決定的 PCM16 WAV 生成（44-byte canonical header +
    `((frame*31 + ch*7) % 4096 - 2048) * 16` の i16 パターン、
    期待 canonical f32 は `v / 32768`）。
  - `import.rs`: `import_local` — staging 外配置 → bytes 完全読取 →
    size/hash/container/codec/frame/channel verify → atomic publish
    → `detect_dir_sync == Supported` のときのみ parent dir sync
    （Windows で `Access is denied` にならず durability receipt が
    実測保証を記録）→ `import-manifest.json` stage 記録
    （`Staged → Verified → BlobPublished`）→ ExpectedRevision
    commit → `applied_imports` registry で `Applied`/
    `AlreadyApplied`/`IntegrityConflict` 分類。typed error:
    `StagingIo`/`Corrupt`/`HashMismatch`/`UnsupportedCodec`/
    `MissingMedia`/`Store`/`RevisionConflict`/`Cancelled`。失敗時
    manifest は残り、部分 Project mutation は起きない。
  - `resolver.rs`: `prepare` — `SourceState >= Verified` 必須、
    asset/digest blob 探索 → blob hash 再検算 → probe → PCM
    canonical `SourceReader` → `Prefetch::manual`。typed error:
    `UnresolvedSource`/`UnverifiedSource`/`MissingAsset`/
    `MissingMedia`/`Corrupt`/`HashMismatch`/`UnsupportedCodec`/`Io`。
  - `lowering.rs`: `lower` — Clip bindings を `ClipWindow`
    （`engine_start`/`engine_len`/`source_start`/`source_len`）へ
    lower し generic `NodeSpec`/ports + master sink を構築。
    musical anchor は `TempoMap` で media 変換、source offset /
    mute / channel layout / rate 不一致を処理し、skip は
    `SkippedClip` diagnostic として明示。SourceFrame/EngineFrame/
    tick を混同せず、tick→frame は最終 frame 境界で丸める。
  - `processor.rs`: `ClipPlayer` — RT `PrefetchConsumer` 所有、
    `PlaybackStats`（written/underrun/faulted/eof_blocks/mismatch/
    silenced/fills）を atomic に集計。位置プロトコルは
    `expected_first = consumer.position()`（ruviform 自身の
    テストと同じ canonical 契約）。steady state は bulk fill、
    seek Boundary が in-flight の可能性や位置不一致時は
    single-frame probe fill（boundary を跨いでも served 位置が
    一意に帰属できる唯一の fill）で position-true に resync。
    `pending_seek`/`drain` nonce は `ClipSync` で共有。
  - `driver.rs`: `HeadlessDriver` — prepare/compile/publish、
    activation 失敗時は last-good plan を保持（producers/handles/
    stats は publish 成功後にのみ差し替え）、manual pump（1 pump =
    ≤1 item のため Pushed 中は反復）、`run`/`collect`/`stop`/
    `seek`/`stats_of`/`total_latency`。`seek` は先に全 clip の
    in-flight ring を drain（quiesce process で discard 出力）して
    から `handle.seek` するため、Boundary が consumer の次 item
    になり sample-exact に再 anchor できる。
- `Cargo.toml` / `crates/synvelis-player/Cargo.toml`:
  workspace member 追加 + ruviform 系依存解決を `1305538c` に
  更新（`Cargo.lock` 記録）。

### Decisions During Implementation

- **Prefetch `pump()` は 1 呼出 ≤1 item**（seek boundary、1 decode
  block、marker のいずれか）。1 block あたり 1 pump では boundary が
  consumer に届かず全無音になったため、`Pushed` の間は反復する
  bounded pump loop にした。
- **`PrefetchConsumer::fill` の `expected_first` は stream 位置の
  resync assertion** であり、engine 側の期待値ではない（ruviform
  自身の `prefetch.rs` テストも `consumer.position()` を渡す）。
  boundary は fill 内でのみ adopt され、adopt すると
  `position()` が seek target に reset される — bulk fill が
  boundary を跨ぐと per-frame 帰属が失われる。そこで
  「`consumer.generation() < base + issued_seeks` または
  `position() != want` の間は single-frame probe、整合後のみ
  bulk」とし、probe の served 位置を `after - span` で正確に
  帰属させた。seek target は常に window 写像由来なので boundary
  が adopt した位置は必ず `want` であり、probe が served frame を
  `served == want` のときだけ `scratch` に置く設計は play-through
  も hole も off-by-one も生じない。
- **seek の sample-exact 性は「先に drain、後に seek」で担保**:
  `exec_cmd` は boundary を pending の末尾に積む（stale item が
  boundary より先に ring へ出る documented play-through 契約）。
  stale play-through を捨てるのではなく `driver.seek` が quiesce
  process で ring を drain（discard、stats.mismatch に記録）して
  から `handle.seek` を発行するため、boundary が consumer の
  最初の item になり in-flight play-through は構造的に発生しない。
- **Windows dir-sync**: `sync_parent_dir` は POSIX 前提で、
  Windows では `Access is denied` になる。durability は「保証の
  測定」が契約なので `detect_dir_sync` capability 判定を必須化し、
  `Supported` のときのみ sync、それ以外は receipt に unsupported
  を記録する。
- **Ruviform music API 変更追従**: 依存 rev 更新で `TickPosition`/
  `Ppqn`/`at_tick` 系に置き換わったため `tempo.rs` を新 API に
  適合（PPQN は validated `Ppqn` として保持し reverse 変換にも
  渡す）。
- **activation の last-good**: `activate` で producers/handles/stats
  を validation/compile/publish 前に差し替えないよう、成功後の
  commit ブロックに移動（失敗時に旧 plan が残る）。

## Acceptance Matrix Traceability

| AC | 検証 |
| --- | --- |
| L1-01 WAV verify・Verified Source materialize・未検証 Source 再生不可 | `l1_01_import_verifies_and_materializes` |
| L1-02 SourceId/AssetId/ClipId 分離・1 Source→2 Clip・Undo/Redo/Save/Reload | `l1_02_id_separation_shared_source_undo_save` |
| L1-03 headless fixture・clip start/end・`[start,end)`・seek/loop・block 跨ぎ sample 一致 | `l1_03_headless_clip_playback_exact`・`l1_03_clip_end_past_source_eof` |
| L1-04 切断/破損/不正 hash/unsupported/conflict/cancel/store error で部分 mutation なし・orphan 明示 | `l1_04_failure_classes_are_typed_and_atomic`・`l1_04_import_idempotency_and_orphan` |
| L1-05 Source-frame/Placement off-by-one なし・変換条件記録・暗黙 resample/quantize なし | `l1_05_time_conversion_is_exact_and_documented` |
| L1-06 Ruviform version/capability/latency・underrun/EOF/plan failure を成功音声と混同しない | `l1_06_capability_and_latency_evidence`・`l1_compat_prepare_failure_keeps_last_good` |
| L1-07 lock 再現・`cargo test --workspace --locked`・新 fixture テスト結果/hash/exit code | `l1_07_fixture_is_deterministic` + 本書 Verification 節 |
| L1-08 discovery/spec/ADR/design 系の相互参照・evidence 記録 | 本書 + 各 doc の `implements`/`relations` |

EX/PAP/MR 系の該当要件は spec/test-spec の cross-reference 表に従い、
上記の実測 test と対応付けた（未実装項目を合格とは表記しない）。

## Validation

- `cargo fmt --all -- --check`: clean（exit 0）。
- `cargo check --workspace --all-targets`: clean（exit 0）。
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
  （exit 0）。
- `cargo test --workspace --locked`: 全 19 suite pass、0 fail
  （exit 0）。既存 vs_a/b/c/d テストを含め全て維持。
- `cargo test -p synvelis-player --test l1_e2e`: **10/10 pass**
  （exit 0）。
- Ruviform lock: `ruviform-*` 系は全て
  `git+…ruviform.git#1305538caeab62e6f491a1d9a2497a1f76909d04`。
- Fixture determinism（`wav_pcm16` canonical bytes）:

  | channels | frames | bytes | sha256 |
  | --- | --- | --- | --- |
  | 1 | 960 | 1964 | `b39236c4064dfddfe2ab92b2486fe3e417428a6cb80887c28e63194016d217ae` |
  | 2 | 3000 | 12044 | `25225bcbb635702e1647154d59b7de773907ffe965a4467b07152ab0c2f1c76f` |
  | 2 | 1024 | 4140 | `aba3fb1fdcb6534809e89964f856cd23d8ad44074e703a720d16493c8c81d042` |
  | 1 | 300 | 644 | `6913deafb2282b7dc17f44d04651fc5f473281c7f1338f266778fc860fdf5799` |

- Seek/replay determinism: `driver.seek(0)` 後の replay `pcm`/`crc`
  が first run と byte 一致（`l1_03_headless_clip_playback_exact`）。
- Failure atomicity: 部分 mutation がないこと・blob/orphan が
  明示分類されることを `l1_04_*` で検証。

## Related Experiments

なし（本 Issue では experiment log 記録対象の探索的実験は発生しなかった）。

## Risks

- `PrefetchConsumer::fill` の `expected_first` が engine frame 期待値
  ではなく stream position の assertion である点は、本実装で probe 充填
  により吸収した。resync 中は 1 frame probe のため block あたりの
  充填効率が落ちるが、ring=8・block=256 の headless 条件では実害なし。
- seek 時の quiesce drain は 1 block 分の追加 process を要する。
  RT 適用時は seek を非RT文脈で先行させる設計が必要。
- `DirSyncCapability` が `Unsupported` の環境（Windows）では durability
  receipt は `unsupported` として記録し、fsync 省略ではなく明示分類する。

## Known Limitations / Report（Ruviform 側の観測・起票候補）

Synvelis 側の変更は Synvelis repo に閉じ込め、以下は回避や重複
実装で隠さず報告対象として記録する。

- `PrefetchConsumer::fill` の `expected_first` は「stream 位置の
  assertion」であり engine の frame 期待値とは別物 — この意味論は
  テストからしか読み取れず、API doc 上の契約が薄い
  （media-reader doc improvement 候補）。
- `handle.seek` の boundary は pending 末尾に in-order で積まれ
  stale content は documented に play-through する。sample-exact
  re-anchor を求める利用側は「quiesce drain → seek」が必要であり、
  seek に optional `discard_pending` フラグや drain 補助があれば
  簡潔になる（media-reader feature 候補）。
- `pump()` は 1 呼出 ≤1 item — boundary を含む marker が
  backpressure 下で producer pending に留まり得るため、利用側は
  `Pushed` 反復を明示する必要がある（doc 上は non-blocking step
  とのみある；`PumpOutcome` 契約の明確化候補）。
- `PrefetchHandle::reopen` は position 0 に再 anchor するが seek
  target への direct reopen はなく、mid-stream source の差し替え
  には reopen→seek の 2 段が必要（feature 候補・現状は
  Synvelis 側では不要）。

## Follow-ups

- Ruviform 起票候補: `PrefetchConsumer::fill` の `expected_first` 意味論と
  `PumpOutcome`/`seek` play-through 契約の API doc 明確化（本 IR の
  Known Limitations / Report 節参照）。
- `PrefetchHandle` への `discard_pending`/`seek`-atomic drain 補助
  （feature 提案・現状は Synvelis 側 quiesce で充足）。
- seek の非RT先行スケジューリング（RT driver 適用時の別 Issue）。

## Rollback

`crates/synvelis-player`・docs の新規ファイル削除 +
`Cargo.toml`/`Cargo.lock`・`synvelis-project`/`synvelis-recording`
の変更 revert で L1 slice 全体を除去できる（Project model の追加
field は serde default を持つため旧 snapshot との相互互換あり）。
