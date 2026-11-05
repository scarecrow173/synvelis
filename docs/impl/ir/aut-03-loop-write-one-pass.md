---
id: "IMPL-AUT03LoopWriteOnePass"
type: "impl"
status: "completed"
title: "AUT-03 Loop 録音 1 Pass・Write Range 半開区間閉鎖・境界 flush・明示 Rearm 実装記録"
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
      - type: "file"
        path: "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
      - type: "file"
        path: "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
      - type: "file"
        path: "docs/designs/aut-03-loop-write-one-pass-design.md"
      - type: "file"
        path: "docs/plans/aut-03-loop-write-one-pass.md"
      - type: "file"
        path: "docs/tasks/aut-03-task1-loop-write-one-pass.md"
      - type: "file"
        path: "docs/test-specs/aut-03-e2e-loop-write-one-pass.md"
      - type: "file"
        path: "docs/impl/ir/aut-03-loop-write-one-pass.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/controls_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_01.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_a.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_b.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_c1.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/aut-03-task1-loop-write-one-pass.md"
  implemented-by: []
  depends-on:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
    - "docs/plans/aut-03-loop-write-one-pass.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/aut-01-touch-return-writer-state.md"
    - "docs/impl/ir/aut-02-region-replace-boundary.md"
    - "docs/impl/ir/pc-c1-recording-transaction-prepare-apply-finalize.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-03-e2e-loop-write-one-pass.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUT-03 実装記録: Loop 録音 1 Pass・Write Transaction・明示 Rearm

## Summary

Issue #133。Automation Write を「1 Loop Pass の半開区間で
終了・Transport は続行可・後続 pass の自動書込みなし・明示
Rearm」の製品契約として実装した。録音経路の境界処理を
`close_write_range` 一本の共通経路に集約し、境界 flush・
`coverage_end`・pass/lane/scope/revision/epoch fixation を
PC-C1 の prepare/apply/finalize 契約を壊さずに追加した。

## Context

- Spec: `docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md`
- ADR: `docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md`
- Design: `docs/designs/aut-03-loop-write-one-pass-design.md`
- Plan: `docs/plans/aut-03-loop-write-one-pass.md`
- Test-spec: `docs/test-specs/aut-03-e2e-loop-write-one-pass.md`
- 依存: AUT-01 writer state(#131)・AUT-02 Region Replace
  境界保存(#132)・PC-C1 RecordingTransaction(#98)。
- Multi-pass Take は AUT-F01(issue #6)に分離済み・本件の
  Gate にはしない。

## Implementation

### Write Range の半開区間化

`ControlSession` の録音経路を「seal だけ立てる」ものから
**`close_write_range(project, reason, bound)` 一本の共通経路**へ
再編した(既存 `seal_recording` は廃止)。境界で以下を 1 度だけ
行う(first-seal-wins、2 回目以降の境界は reason・列ともに不変):

1. **Flush** — fixed target の in-flight gesture の delivered
   points を `pos <= bound && pos < loop_end` で clip して append。
   記録済み点は pass の一部であって preview ではない。
2. **Coverage** — gesture / Latch held / Write held の active
   writer が居た fixed lane へ `coverage_end[lane] = bound` を
   project beats で記録。held write が連続被覆した末尾は
   `last point + ε` では表現できないため、ADR のとおり別列で保持。
3. **Seal** — `tx.sealed = Some(reason)`。列は inspect/commit/
   abort に残る。

`bound` は wrap 時のみ loop `end`、他の全境界では「閉鎖時点の
transport position」(loop 設定時は `min(bound, loop_end)` に
clamp)。これで半開区間 `[first point, coverage)` が確定する。

### 新規 seal 経路(全て同じ close_write_range)

- `LoopWrap` — wrap 時(既存 reason、従来は seal のみ)。
- `Stopped` / `Seek` — stop は現在位置、seek は **pre-seek 位置**で
  閉じる(記録区間の contiguity を偽らない)。
- `EpochChange` — `install_clock_anchor` が tx fixation と異なる
  epoch を載せた時。
- `GraphStale` — 新設 `note_graph_stale(project)`。runtime gesture
  state は閉じない(preview は session 側の問題で project 不変)。
- `LoopChanged` — `set_loop` の range 変更(同一 range の再設定は
  境界にしない)。
- `NewTransportPlay` — 非 Playing → Playing edge(AUT-01 の run
  境界契約を recording gate に適用)。

### Pass fixation・Rearm

- `LoopPassId(u64)` 新設・`PlaybackState.pass` 追加: play edge で
  `1`、wrap 毎に `+1`(saturating)、pause/resume/seek/stop では
  不変。stop → play は新 run で `1` に戻る。
- `RecordingTransaction.{pass, fixed_scopes, coverage_end}` を
  `record_start` で fixation。`fixed_scopes` は
  `(ControlOwner, AutomationTimeScope)` — clip move/scope 変更で
  記録位置を再解釈しない。
- 後続 pass での自動再書込みはしない(sealed tx は write gate を
  閉じ続ける)。明示 Rearm = 新しい `record_start` が「その時点の
  pass」を fixation する — same frame でも pass は正しく分かれる。
- `record_write` は `pos >= loop_end` を Validation 拒否、
  gesture batch 経由の `record_points` でも同境界で clip。
- `bind_write_lane` は open tx 中の fixed target rebind を拒否
  (mapping は record_start の写しが不変の正本)。
- `txn_fingerprint` を pass/fixed_scopes/coverage_end に拡張 —
  prepared commit の identity に AUT-03 の context を含める。

### Commit 経路(PC-C1 不変 + coverage 反映)

`prepare_record_commit` は lane 毎に `coverage_end` を fixed
scope で scope 座標へ resolve し、`> last_pos` の時だけ
`Replace` range の exclusive end に採用(resolve 不能な scope
では last+ε に fallback)。Replace 衝突解決は AUT-02 の
`WriteAutomationRegion{Replace}` そのまま — Composite 1 op =
1 undo。失敗時の列保持・`Conflict` → 明示 `approve_rebase` のみ
という PC-C1 契約は不変。

### set_mode の seal 順序(review で確定した修正)

`set_mode` で `modes.insert` → seal の順だと coverage 判定が
**新 mode** を見てしまい、境界直前まで held だった Write writer
の `coverage_end` が付かない。seal → insert に並べ替え、
「境界時点で active だった writer」に coverage を正しく付与。

### 設計からの差分

- design は `SealOutcome.write_range_closed` を構想したが、実装は
  `close_write_range() -> bool` + 既存 `TransportEffect.sealed` の
  2 値で十分(重複 outcome 型は導入しない)。
- `set_loop` の validation(`end <= start`)は close 前に評価 —
  不正 range で recording を閉じないよう validation 先行。

### 監査 — seal/close 経路の全 call 監査

- `close_write_range` 呼出: wrap/stop/seek/play-edge/loop-change/
  epoch-change/graph-stale/project-change/mode/arm(fixed target)
  の 10 経路 — 全て共通関数を通る。
- `record_write`/`record_points`/`gesture_end` flush は全て
  fixed lane 経由・pass 境界 clip 済み位置のみ採用。
- `seal_recording` 旧関数は削除済み(grep 監査: 残参照なし)。

## Related Experiments

実験ログなし — 方針は design/ADR で事前確定済みの確定実装。

## Validation

### 検証結果(実施済み・全緑)

- `mise run fmt` / `fmt-check`: pass
- `mise run check`: pass
- `mise run lint` (`cargo clippy --workspace --all-targets -D
  warnings`): pass
- `mise run test` (`cargo test --workspace --locked`): 全 binary
  緑(`tests/aut_03.rs` 19 件を含む。aut_01/pc_a/pc_b/pc_c1/
  sv47/sv48/ws_a 等無退行)
- `mise run test-rt` (`cargo test --workspace --locked rt_`): 全緑
- `mise run test-msrv` (rust 1.89 `cargo test --workspace --locked`):
  全緑

### `tests/aut_03.rs` が pin するもの(test-spec F1–F7)

- F1: wrap が loop end で正確に seal・`< end` のみ記録・transport
  続行・pass +1・`pos >= end` は record_write 拒否と gesture
  flush clip の双方で次 pass 扱い。
- F2: 後続 pass は seal を維持し自動書込みしない・明示
  `record_start` だけが現 pass で新 tx を開く・pass lifecycle
  (wrap=+1/pause-resume 不変/new run=1)。
- F3: fixed target/lane/revision/scope/epoch の fixation —
  lane rebind 拒否・非 fixed target の mode 変更は無関係・
  arm 除去は `ArmChanged` seal・再 arm で gate は復帰しない。
- F4: wrap/stop/seek での gesture flush(in-flight delivered
  points は記録される)・Write held の coverage_end が Replace
  range を loop end まで延長。
- F5: Stopped/Seek/ProjectChange/EpochChange/GraphStale/
  LoopChanged/NewTransportPlay/ModeChanged/ArmChanged の
  first-seal-wins。
- F6: multi-lane 1 tx = 1 Composite = 1 undo・conflict で列保持・
  明示 rebase のみ commit 可・pre-modulation 値 verbatim・
  commit≠save・save/reload で region 一致・Replace の範囲外曲線
  保存(AUT-02 経由)。
- F7: empty pass の seal+commit(nothing applied, was_sealed
  報告)・wrap と同 frame の rearm が新 pass を fixation・
  prepared commit が fixed_revision/was_sealed を pin。

## Risks

- `note_graph_stale` は host 通知境界として新設 — 現状の
  in-crate 呼出は test のみ。実 host の配線責務は呼出側にある
  (`install_clock_anchor` と同じ構造)。
- `pos < loop.start` の点は permissive な lead-in として記録
  される(non-blocking な review note として記録済み)。
  半開区間契約は loop end 側で厳密に保証される。
- `coverage_end` が scope resolve 不能な lane は last+ε に
  fallback — 契約上の degenerate であり silent な範囲捏造は
  しない。

## Follow-ups

- 実 GUI Inspector/Window 操作経由のリダイレクト防止 —
  headless 契約では「fixed_targets/fixed_lanes が唯一の write
  対象」と「rebind 拒否」で pin。実 GUI 経路は SV-GUI-05 未実装
  (NotRun)。
- 実 plugin dispatch・実機 transport — headless の
  `transport_*`/`note_graph_stale` API で pin。実 host 経路は
  未検証(NotRun)。
- AUT-F01 multi-pass take — 本件の対象外(issue #6、
  Unsupported)。
- 実機 RT 録音検証 — headless contract のみ(NotRun)。
- `docs/designs/overview.md` 不在は既存の manual finding —
  本件では非対処。
