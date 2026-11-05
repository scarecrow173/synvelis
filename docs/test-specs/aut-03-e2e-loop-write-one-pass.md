---
id: "TSPEC-034AUT03Zk6mVt4nQr2wXs8"
type: "test-spec"
status: "approved"
title: "AUT-03 E2E 検証仕様 — Loop 録音 1 Pass・Write Range 半開区間・境界 flush・明示 Rearm"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/133"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_03.rs"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/test-specs/pc-c1-e2e-recording-transaction.md"
    - "docs/test-specs/aut-01-e2e-touch-return-writer-state.md"
    - "docs/test-specs/aut-02-e2e-region-replace-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/20"
  defers: []
  deferred-by: []
---

# AUT-03 E2E 検証仕様

## 対象・基準

Issue #133 Acceptance が正本。headless で pin できる部分を
`crates/synvelis-workspace/tests/aut_03.rs`(+ 既存 aut_01/
pc_c1/pc_a 系テストの無退行)として実施する。実 GUI/実 Plugin/
PC-C6 oracle 依存は NotRun/Unsupported として記録する。

## Feature F1: 1 Loop Pass での Write Range 閉鎖・半開区間

- **Rule**: `record_start` で `pass`/`revision`/`epoch`/`scope`/
  `target` を fixation し、最初の wrap で `sealed=LoopWrap`・
  `coverage_end=loop_end`・`pass=2` へ遷移する。Pass 1 の
  segment point は `pos < loop_end` のみ。以後の loop では自動
  記録されない。
- **Examples**:
  - loop 中央で record_start → wrap → `pos < end` の point のみ
    収集・`coverage_end == end`・sealed=`LoopWrap`
  - `pos == end` ちょうどの wrap で `end` 位置の point は Pass 1
    segment に入らない(次 Pass 所属)
  - `transport_advance(to > end)` の block 内 wrap でも同じ閉鎖
  - wrap 後の `record_write`/gesture point は全て拒否/clip
    され、tx.segment は増えない
  - mid-loop position での record_start でも `[start_pos, end)`
    が記録され `< start_pos` の point は入らない

## Feature F2: Explicit Rearm・Pass 計数

- **Rule**: pass は play edge で 1・wrap で +1・pause/resume/
  seek/stop では不変。wrap 後の 2 周目で自動記録されず、新
  `record_start` だけが新 Pass を開く。その `record_start` は
  呼出時点の pass を fixation する。
- **Examples**:
  - wrap 後に transport が loop を続行しても新たな segment は
    作られない(sealed tx が残る限り `record_start` は拒否)
  - sealed tx を commit/abort 後の `record_start` は
    `pass = 現在の playback.pass` を固定
  - `pause` → `resume` で pass は同じ run のまま、`Stopped`
    からの `transport_play` で pass=1 へ reset
  - `Paused`/`Stopped` からの新規 play edge で open tx が
    `NewTransportPlay` seal

## Feature F3: 境界 flush — write-gated gesture の配送済み point 保全

- **Rule**: Write Range close の瞬間に write gate が開いていて
  active writer(gesture 中・LatchHeld・WriteHeld)が存在する
  lane で、in-flight gesture の配送済み point は
  `pos <= bound && pos < pass_end` で clip して transaction へ
  帰属させる。非 write-gated gesture は従来どおり破棄。
- **Examples**:
  - Write mode gesture の配送済み point が wrap で tx.segment
    へ入る(`pos < end` のみ)
  - `pos >= end` の配送分は clip される
  - Read/Unarmed 中の gesture は flush されず closure log のみ
  - gesture runtime state は従来どおり閉じ、preview 値は
    プロジェクトへ残らない

## Feature F4: 追加閉鎖イベント — coverage_end の記録

- **Rule**: Stop/Seek/Epoch/Graph/Loop/Arm/Mode/ProjectChange/
  Play edge の各経路で Write Range を閉じ、write-gated writer が
  存在した lane のみ `coverage_end` を境界位置へ記録する。
  first-seal-wins — 2 回目の close は coverage/flush を再実行
  しない。
- **Examples**:
  - `transport_stop` → `Stopped` + coverage_end=stop position
  - `transport_seek` → `Seek` + coverage_end=seek 先
  - `install_clock_anchor` で epoch 変更 → `EpochChange`
  - `note_graph_stale` → `GraphStale`
  - 録音中 `set_loop` 変更/クリア → `LoopChanged`
  - `set_write_arm(false)` → `ArmOff`、mode 変更 → `ModeChange`
  - `note_project_changed` → `ProjectChange`
  - 非 `Playing` → `Playing` edge → `NewTransportPlay`
  - 既に sealed の tx への連続境界イベントは coverage を変えない
  - `FocusLost`/`PluginCrash`/`Cancelled` は Write Range を
    閉じない(AUT-01 runtime closure のみ)

## Feature F5: coverage_end による Replace range — 区間外曲線保存

- **Rule**: `prepare_record_commit` は `coverage_end > last_pos`
  の lane で coverage_end を Replace range 終端へ採用する。
  scope resolve 失敗/`coverage_end <= last_pos` は `last_pos+ε`
  fallback。範囲外の曲線は AUT-02 の anchor で保存される。
- **Examples**:
  - held Write が `end` まで書き続けた pass で、Replace range
    が `[first, end)` へ伸びる(last+ε ではない)
  - `coverage_end` 無し lane は従来どおり `last+ε`
  - scope resolve 失敗 lane は last+ε fallback
  - commit 後に `[coverage_end, ...)` の既存曲線が anchor 経由で
    保存される(AUT-02 契約の無退行)

## Feature F6: 失敗時の点列保全・Rebase・1 Undo

- **Rule**: prepare/apply/finalize 失敗で収集済み point は
  保全され take 化しない。Conflict は `RequiresUserRebase` で
  最新 revision の silent 採用をしない。1 commit = 1 Undo。
- **Examples**:
  - Stale CAS + wrap closed tx → `Conflict` で segment 保持
  - explicit rebase 後の commit 成功、point は pass-1 区間のまま
  - commit 成功 → `undo` 1 回で pass-1 区間が元に戻る
  - commit 後の save/reload で永続 series が復元される(project
    のみ — session tx は非永続のまま)

## Feature F7: Pass 境界の誤所属ガード

- **Rule**: loop 設定中、`pos >= loop_end` の書込は Pass 1 に
  帰属しない — `record_write` は Validation 拒否、gesture
  batch は clip。`pos < loop_start` は許容。
- **Examples**:
  - `record_write(at >= loop_end)` → `Reject::Validation`
  - gesture 配送の `pos >= loop_end` は clip される
  - loop 未設定の録音では pass 境界ガード無効(wrap しない
    限り pass=1)

## 除外(NotRun/Unsupported として記録)

- 実 GUI・実 Plugin binding 経由の Pass 録音(PC-C4 #101 /
  PC-C5 #102 / SV-GUI-05 #114 未実装 — headless session の
  等価 pin で代替)
- PC-C6 oracle(#103)の同一シナリオ別 Oracle 判定 — headless
  契約 pin は本件、GUI/realtime oracle は PC-C6 側の責務
- Multi-pass Take・Punch 履歴(AUT-F01 #6 — 明示的に非対象)
- RT 経路の `eval_rt`/dispatch/publish 検証(変更面は session
  層のみで RT 非接触 — 既存 pc_b_rt 無退行で確認)
