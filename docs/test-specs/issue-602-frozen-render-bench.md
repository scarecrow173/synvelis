---
id: "TSPEC-v6mGcY3cvf6QTIDJnMSZ"
type: "test-spec"
status: "approved"
title: "Issue #602 検証仕様 — frozen 2-arm benchmark harness（render/playback E 層 slice 1）"
created: "2026-11-20"
updated: "2026-11-20"
owners: []
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
  related:
    - "scarecrow173/ruviform#823"
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

# Issue #602 Test Spec — frozen 2-arm benchmark harness

検証本体: `crates/synvelis-render/benches/frozen_render.rs`（`harness = false`
の custom main）。実行入口は `mise run bench-frozen`（=
`cargo bench --locked -p synvelis-render --bench frozen_render --`、
引数はハーネスへそのまま転送）。baseline の正本は
`crates/synvelis-render/benches/frozen/baseline.sv602.json`。
evidence は `evidence/sv602/`。

本 harness は「凍結契約」である。naive mixer の意味論、セル行列、
計測手順、gate 規則のいずれかを変更した場合、pin 済み baseline は
無効化される（再 pin は後述の candidate → 手動 promote 手順のみ）。

## Feature: Arm K — naive mixer vs 本番 HeadlessDriver（`mix/<clips>x<block>`）

- 被測定シーケンス（B 側・本番）: `write_fixture WAV → import_local →
  resolver::prepare → lowering::lower → HeadlessDriver::activate →
  run(blocks, PumpPolicy::Every(1)) → PlayReport`。1 回の timed call =
  `HeadlessDriver::new → prepare×N → activate → run` 全体。
- 比較対象（A 側・凍結参照）: harness 内の naive mixer。各 playback
  binding の engine window `[engine_start, engine_start+engine_len)` を
  source `[source_start, source_start+engine_len)` に 1:1 写像し、
  `fixture_sample_f32(src_ch, src_pos)` を合算（`src_ch =
  ch.min(src_channels-1)`、fade は `FadeWindow::gain_at`）。
- セル行列: `mix/4x256`（4 clips × block 256 × window 24576）、
  `mix/8x512`（8 × 512 × 49152）、`mix/16x256`（16 × 256 × 24576）。
  fixture は stereo/48 kHz、clip i の source offset は `i*64`。
- 計測: A/B を同一 process/session で交互（rep ごと A→B）。rep 数 K=7。
  rep 内 `inner` は 1 call 見積もりから ~0.25 s になるよう calibrate
  （上限 64）。報告値は rep 中央値 / inner の per-call ns。
- gate 指標: `ratio = median(A)/median(B)` per-call。本番側が遅くなるほど
  ratio は下がるので、回帰検出は `ratio < min_ratio`（baseline pin）で
  fail。改善（ratio 上昇）は fail にしない。
- 正当性ゲート（perf と独立・常時）: A/B の全 sample を bit 比較し
  `errors > 0` または call 失敗 `call_failures > 0` で cell fail
  （`--test` でも exit 1）。

## Feature: Arm P — 実 `export()` パイプライン E2E（`export/<clips>x<range>`）

- 被測定シーケンス: `ProjectController` ops で決定的 project 構築 →
  `RenderSnapshot::freeze` → `synvelis_render::export(snapshot,
  request, hooks, job)` → staged file publish → 出力 WAV readback。
- セル行列: `export/4x48000`（4 clips × range 48000）、
  `export/8x96000`（8 × 96000）。insert chain は信号順に
  `Gain(-6 dB) → OnePole(a=0.5) → Delay(64)`、
  `tail = TailPolicy::Known(64)`、`wire_format = S16`、
  `replace_existing = true`、`clip_support = Verified`。
- 正当性ゲート: publish された WAV bytes の sha256 が baseline pin と
  一致すること。rep 内で fingerprint が揺れた場合も
  `fingerprint_unstable` fail（決定性自体の検査）。
- gate 指標: `normalized = median(wall)/median(同一 session 先頭 Arm K
  本番 per-call)`。`normalized > max_normalized` で fail。絶対 wall は
  informational のみ（`wall.median_ns` 等として記録されるが gate しない）。
- `ExportOutcome::Completed`（reopen verify + publish 通過）のみ正常;
  `Failed`/`Cancelled`/readback 失敗は `export_failed` として cell fail。

## Feature: CLI・記録・verdict 契約

- 出力: stdout に JSONL。`type:"run"`（schema `syn.bench.run/v1`、mode・
  reps・env〔synvelis_sha/ruviform_sha/rustc/os/arch/cpu/cores〕・
  injected）、`type:"cell"`（arm・reps・median/min/max・per_call・
  ratio または normalized・errors・work・limiter）、`type:"gate"`
  （verdict・baseline_state・cell 別 verdict+reasons）。
- verdict 語彙: `pass` / `fail` / `not_run` / `unsupported`。
- mode: 既定 run（実計測 + gate + `evidence/sv602/decision.tsv` 追記）、
  `--test`（1 rep smoke・gate `not_run`・正当性 fail のみ exit 1）、
  `--selftest`（回帰注入・gate fail 実証・exit 1 を期待）、
  `--write-baseline <path>`（candidate 出力・`approval:null`・
  gate 非参照）、`--baseline <path>`（pin 差し替え）、`--help`。
- 位置引数はセル名 substring filter。0 件 match は `gate not_run` +
  exit 0（エラーにしない）。
- exit: `0` pass / `1` gate または正当性 fail / `2` CLI misuse。
- `cargo bench` が付与する `--bench`、`cargo test` が付与する
  `--nocapture` 等は受理して無視する。

## Feature: baseline ライフサイクル（fail-closed）

- `--write-baseline <path>` は schema `sv.bench.baseline/v1`・
  `"approval": null` の candidate を書くのみ。自動 promote しない。
- promote は人手で `approval` を object（`by`/`at`/`note`）に置換し、
  `benches/frozen/baseline.sv602.json` に配置する。pin は commit 対象。
- gate 参照は run/selftest mode のみ。baseline が存在しない・JSON として
  不正・schema 不一致・`approval` が object でない（未 promote 含む）
  場合は unreadable として **gate fail（exit 1）** に倒す。cell ごとの
  verdict は `not_run` + `baseline_unreadable`。
- baseline に cell エントリが無い cell は `not_run` +
  `baseline_incomplete`（gate 全体も fail）。
- 環境差（`env` block）は記録のみ。ratio/normalized は同一 session の
  A/B 比なので機種差を相殺する設計であり、sha256 fingerprint は
  決定的なので跨機種で一致するはず。

## Feature: selftest（回帰注入の実証）

- Arm K: B 側 timed call 内に推定 per-call × 4 の busy spin を注入 →
  全 K cell が `ratio_below_threshold` で fail すること。
- Arm P: `OutputHooks.wrap_sink` で WAV data byte@100 を 1 byte 反転
  （header 44 byte は温存するため reopen verify/publish は通る） →
  全 P cell が `fingerprint_mismatch` で fail すること。
- `--selftest` は gate `fail`・exit 1 が正しい結果。証跡は
  `evidence/sv602/selftest-gate-fail.jsonl`。

## Feature: 既知の依存不具合と workaround

- `scarecrow173/ruviform#823`: `ruviform_media_reader::Prefetch::manual`
  が free-block pool を `debug_assert!` 内で初期化するため、release
  profile では pool が空になり producer が `Backpressured` を返し続け、
  consumer が永続 underrun（無音）になる。debug build（`cargo test`）
  では顕在化しない。
- workaround: root `Cargo.toml` の
  `[profile.bench.package.ruviform-media-reader] debug-assertions = true`。
  対象 crate を bench profile のみ debug-assertions 付きでコンパイルし
  pool 初期化を有効化する。撤去条件: upstream 修正を取り込み
  ruviform rev を bump したとき（comment に明記済み）。

## Feature: evidence

- `evidence/sv602/decision.tsv` — run/selftest/write-baseline の各実行が
  1 行追記（hillclimb schema）。
- `evidence/sv602/runs.jsonl` — 検証 lane 実行記録。
- `evidence/sv602/selftest-gate-fail.jsonl` — selftest 全 JSONL。
- `evidence/sv602/baseline-candidate.sv602.json` — 生成候補
  （`approval:null` のまま保存）。
- `evidence/sv602/verification.md` — lane 実行一覧と判定。
