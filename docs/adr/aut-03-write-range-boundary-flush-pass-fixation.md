---
id: "ADR-034AUT0312NAA2Mk53Gle5"
type: "adr"
status: "accepted"
title: "AUT-03 Write Range 半開区間の表現 — 境界 flush・coverage_end・Pass fixation"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/133"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
  implemented-by:
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
    - "docs/plans/aut-03-loop-write-one-pass.md"
  depends-on:
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
    - "docs/adr/aut-02-boundary-sentinel-anchor-placement.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-03-e2e-loop-write-one-pass.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
  defers: []
  deferred-by: []
---

# AUT-03 Write Range 半開区間の表現

## Status

Accepted — Issue #133 の 2026-11-04 合意に基づく初期実装方針。

## Context

- Issue #133 の契約は「Loop Wrap 時、最初の Pass の記録区間を
  正確な半開 Project Beat 区間で閉じ」と要求する。半開
  `[start,end)` の `end` は**次 Pass 所属**で、Pass 1 の書込み
  範囲に含めない。
- 現行 `RecordingTransaction` は `sealed: Option<SealReason>` で
  write gate を閉じるだけで、**書込みが実際に覆った区間**を持た
  ない。`prepare` の Replace range は `first..last point + ε`
  から導出するため、held write が `end` まで書き続けても最後の
  `transport_advance` 位置までしか範囲が伸びず、`[last+ε, end)`
  に AUT-02 の右 anchor で旧曲線が復帰してしまう。
- in-flight gesture の `points` は `gesture_end` で初めて
  transaction へ入る。境界イベントで runtime state が閉じると
  gesture は破棄され( AUT-01 の preview 規則)、配送済み point が
  記録に残らない。
- session の `playback.position`/`loop_range` は Project Beat
  座標。lane の point は target の `AutomationTimeScope` で resolve
  済みの scope 座標 — 座標系をまたぐ境界値の保持場所を選ぶ必要が
  ある。

## Decision

### D-1: Write Range を `coverage_end`(lane ごとの半開終端、Project Beat)として transaction が保持する

- `RecordingTransaction.coverage_end: BTreeMap<AutomationLaneId,
  BeatPoint>` を追加し、境界 close の瞬間に **write gate が開いて
  いて active writer(gesture 中 / LatchHeld / WriteHeld)が存在
  した lane のみ**、境界位置(LoopWrap は `end`、それ以外は現在
  position)を格納する。loop 設定中は `min(bound, loop_end)` に
  clamp する。
- `prepare_record_commit` は lane の Replace range を
  `coverage_end`(有効なら scope resolve した値)> `last_pos` の
  とき限り採用し、それ以外は従来の `last_pos + ε` にフォール
  バックする。
- **coverage_end は Project Beat で保持し、prepare で scope
  resolve する**(seal 時に eager resolve しない)。recorded
  point は書込時点で resolve され scope 座標で保持されるが、
  coverage bound は「どこまで書いたか」の**project 位置**事実で
  あり、prepare 時の live anchor で resolve した方が point 列と
  整合する(clip 移動後の explicit rebase でも同じ anchor 系で
  評価されるため)。

### D-2: Write-gated gesture の配送済み point は境界で transaction へ flush する

- Write Range close の全経路(LoopWrap/Stop/Seek/Epoch/Graph/
  Loop/Arm/Mode/ProjectChange)で、close 瞬間に `write_gates`
  を満たしていた fixed target の in-flight gesture point を
  `pos <= bound && pos < pass_end` で clip して transaction へ
  append する。**gesture は依然として runtime 境界で閉じる**
  (closure log も記録する)が、配送済みデータは記録の一部として
  保全する。
- AUT-01 の「preview 破棄」は **write gate が開いていない
  gesture**(Read mode、disarmed、record disabled、cancel)に
  限る。Write-gated gesture の配送済み point を黙って捨てると
  Pass 境界誤所属 0 と Recorded Points 保全を両立できないため。
- flush は best-effort の salvage: append が失敗する target は
  残りを捨てて close を優先する(閉鎖が flush 失敗で止まらない
  よう)。既に収集済みの segment は失敗でも触らない。

### D-3: `LoopPassId` は run 内の wrap 計数として固定し、明示 Rearm = `record_start` とする

- `LoopPassId(u64)` を playback に持たせ、新規 play edge で 1、
  wrap ごとに `checked_add(1)`。`resume` は同一 run で不変。
  `record_start` が現在値を `tx.pass` へ固定する。
- 次 Pass の記録入口は新設 API を作らず既存の `record_start`
  (常に明示呼出)とする。gate が閉じた後も transport は続行し、
  sealed transaction は commit/abort で処分してから次の
  `record_start` が開ける — 「2 周目の自動記録なし」は gate
  契約そのもので担保される。
- pass を persisted/schema 化しない — session 内の pass 識別
  だけが必要で、`Cargo.lock`/project schema への影響を作らない。

### D-4: Pass 境界の誤所属を構造で拒否する

- loop 設定中、`pos >= loop_end` の書込は Pass 1 に帰属しない:
  `record_write` は Validation 拒否、gesture batch は clip。
  `pos < loop_start` は許容する — position は loop 範囲外に居て
  よく、その位置の書込みは Pass 1 の coverage 内に収まる。
- 同時刻 event は既存の pos last-wins dedup に委ね、`record_start`
  より前の pos は既存契約どおり収集する(record_write の `at` は
  playback position を直接は参照しない呼出側責務 — 開始前 pos を
  禁止すると既存 punch-in 系テストを破壊する)。

### D-5: 閉鎖イベントの追加は「Write Range の座標基準が失効する」場面に限る

- `EpochChange`(clock anchor の epoch 変更)、`GraphStale`
  (host 明示通知 — session は graph を持たず LastGood mismatch を
  自律検知できない)、`LoopChanged`(loop 範囲の変更 — Pass 境界
  の座標基準が変わる)、`NewTransportPlay`(paused/stopped からの
  新規 play edge — AUT-01「次の再生で暗黙再開しない」を
  transaction にも適用)。
- `FocusLost`/`PluginCrash`/`Cancelled` は Write Range を閉じない
  (AUT-01 の runtime closure のみ) — これらは write context の
  喪失ではなく、録音 gate 自体は存続する。

## Considered Options

- **Option A(採用)**: coverage_end を Project Beat で保持 →
  prepare で scope resolve(D-1)。境界位置の事実を一箇所に持ち、
  scope 解決失敗時は last+ε fallback で縮退しない設計。
- **Option B(却下)**: seal 時に eager scope resolve。clip 移動後
  の rebase で live anchor と乖離し、bound が point 列と別座標系
  で固定される。eager 化には全 seal path への `&Project` 依存も
  増やすだけで利点がない。
- **Option C(却下)**: gesture の境界 flush をせず AUT-01 どおり
  破棄。配送済み point を失い「Recorded Points 保全」に反する。
- **Option D(却下)**: `record_rearm` 専用 API を新設。
  `record_start` が常に明示呼出である既存契約と重複し、
  YAGNI に反する。
- **Option E(却下)**: `pos >= loop_end` を reject ではなく clip
  だけにする。呼出側の位置間違いを黙って潰すため、`record_write`
  の単一点経路は Validation で表に出す(gesture batch は clip)。

## Consequences

- `transport_stop`/`transport_seek`/`set_loop`/
  `install_clock_anchor`/`note_graph_stale`/`note_project_changed`
  の seal path に `&Project` が必要になる(flush/scope resolve) —
  workspace session API 内の signature 変更で、呼出側は限定的。
- `RecordingTransaction` に `pass`/`fixed_scopes`/`coverage_end`
  が増え、fingerprint payload が拡張される — session 内のみの
  型で永続化 schema には無関係。
- AUT-01 の「gesture は境界で破棄」は**非 write-gated** に限定
  される形で狭まる — write-gated gesture の配送済み point のみ
  transaction へ帰属させる変更で、closure log / WriterState の
  意味は変えない。
- `LoopChanged`/`NewTransportPlay`/`EpochChange`/`GraphStale` の
  `SealReason` が増える — `RebasePreview.sealed` 等の観測面が
  拡張されるだけで後方互換(非永続)には影響しない。
