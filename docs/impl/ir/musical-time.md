---
id: "IMPL-sl0Rr2E3MxV46MGYb9oin9"
type: "impl"
status: "completed"
title: "Musical Time — 設計契約文書チェーン実装記録（synvelis#19）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/19"
  changes:
    added:
      - type: "file"
        path: "docs/specs/musical-time.md"
      - type: "file"
        path: "docs/designs/musical-time-design.md"
      - type: "file"
        path: "docs/plans/musical-time.md"
      - type: "file"
        path: "docs/tasks/musical-time-task.md"
      - type: "file"
        path: "docs/test-specs/musical-time.md"
      - type: "file"
        path: "docs/impl/ir/musical-time.md"
      - type: "file"
        path: "docs/impl/exp/musical-time.md"
      - type: "dir"
        path: "evidence/sv19"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/musical-time-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/musical-time-task.md"
    - "docs/designs/musical-time-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/musical-time.md"
    - "docs/designs/musical-time-design.md"
    - "docs/plans/musical-time.md"
    - "docs/test-specs/musical-time.md"
    - "docs/impl/exp/musical-time.md"
    - "evidence/sv19/case-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/musical-time.md"
  verified-by: []
  references:
    - "docs/impl/exp/musical-time.md"
    - "evidence/sv19/case-ledger.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/129"
    - "https://github.com/scarecrow173/synvelis/issues/130"
    - "https://github.com/scarecrow173/synvelis/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/307"
    - "evidence/sv19/case-ledger.md"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/musical-time.md"
    rejected: []
---

# Musical Time — 設計契約文書チェーン実装記録（synvelis#19）

## 概要

Issue #19「Configurable PPQN・Resolution-independent
Musical Time・Native MIDI Fidelity」の設計・ADR 要求を、
docs chain 7 文書 + `evidence/sv19/case-ledger.md` として
固定した docs/evidence slice。**コード変更は一切行ってい
ない** — 実装は TIM-01 (#125)・TIM-02 (#126)・TIM-03
(#127)・TIM-04 (#128)・TIM-F01 (#129)・TIM-F02 (#130)
および Ruviform#189 等の Owner Issue に分解済みであり、
本 slice はそれらの契約を Issue #19 の受入条件へ写像し、
被覆を監査した。

## 実施内容

1. **ドメイン調査** — `ops.rs`/`ppqn.rs`/`smf.rs`/`time.rs`/
   `midi.rs`/`control.rs`/`store.rs`/`tempo.rs`/`placement.rs`
   と TIM-01/02 の spec/design/plan/task/test-spec/IR を
   精査し、実装の実在 API・fixture・schema version
   （`SCHEMA_VERSION=13`）を確認。
2. **docs chain 作成** — spec（R-MT-01〜08）・design
   （Owner マップ・時間ドメイン境界・site 分類・D1〜D6）・
   plan・task・test-spec（Rule MT-F01-1〜MT-F09-1）を作成。
3. **証拠台帳** — `evidence/sv19/case-ledger.md` で受入条件
   8 項目を `SV19-C01`〜`SV19-C08` へ写像。各 Case に根拠
   fixture・Expected/Observed・Status・丸め誤差証拠を記録。
4. **index 再生成・検証** — `doc-maintenance` `index apply`
   で touched dir の index を再生成、`doc_status.js lint`・
   `mise run fmt-check` を実行。
5. **review-protocol** — 客観チェック→独立 review→findings
   検証→severity 判定を実施し blocker を解消。
6. **merge/commit/push/PR** — `origin/main` 取り込み・
   Conventional Commit・push・日本語 PR・`worker_done`。

## 設計上の主要決定（spec/design に反映）

- **正本は exact rational beat** — `MusicalTick`（project
  PPQN grid）は投影であり、正本は ruviform `BeatPosition`
  （PPQN 非依存有理数）。1/7 beat 等の非格子位置を正確に
  保持。
- **PPQN 変更は musical site のみ再投影** — source-native
  MIDI tick/division/raw bytes・絶対 media anchor・device/
  recording/plugin frame は不変。`SetProjectPpqn` は
  `to_beat`→`from_beat` の明示経路 + `Exact`/`Round` policy。
- **時間ドメインは別 newtype** — `MusicalTick`/`MidiTick`/
  `MediaTimeValue`/device frame を型で分離し、domain を
  跨ぐ演算は `TickTimeMapping`/`to_beat`/`from_beat`/
  `tick_to_media` の明示関数のみ。
- **SMF は provenance document** — `SmfDocument` を
  `project.smf_documents` に保持、`paired_notes()` の明示
  経路のみ note 射影、immutable guard で provenance 破壊
  防止、export は `ConversionReport` で誤差を報告。
- **plugin 精度は capability class** — `Exact`/`FrameAccurate`/
  `BlockOnly`/`DegradedWithConsent`/`Unsupported`/`Unknown`。
  実機検証は TIM-03/04・ruviform#307 の別 gate。

## Validation

- `mise run fmt-check` — **Pass**（exit 0）。Windows
  (win32)、docs/markdown の正規検証として `cargo fmt
  --all -- --check` が完了。
- docs index 整合 — **Pass**。`doc_maintenance.js index
  apply` で specs/designs/plans/tasks/test-specs/impl/ir
  の index を再生成（`docs/impl/exp` は `.jsonl` のみ
  管理のため `.md` 追加で差分なし・in-sync 確認）。
  `doc_status.js lint` を影響 dir 全てに実行し、本 slice
  新規文書の finding は修正後ゼロ（pre-existing の
  repository 全体 broken relations は範囲外として記録）。
- relation target 実在確認 — **Pass**。全 relation
  （depends-on/related/derives-from/verified-by 等）の
  target を `test -f` で確認済み。
- review-protocol — 客観チェック（doc-status lint・index
  整合・relation target 実在確認・fixture 関数名の
  `grep` 実在確認）→ Reviewer A（協調: 完全性・要件写像）
  → Reviewer B（敵対: fixture path 精度・型名の正確性・
  証拠の誇張・site 分類）→ Validator（findings を実
  ファイル/ソースで検証）→ Judge（severity 判定）を
  separated pass で実施（worker 契約により subagent 不使用、
  明確に分離した pass として実行）。検出・修正した
  CONFIRMED findings: (a) `smf_phrase_boundary_virtual_off_and_panic`
  の file 帰属誤り（`tim_02_midi_phrase.rs`→`tim_02_smf.rs`）、
  (b) `assert_no_hanging_notes` が test ではなく helper で
  ある点の明記、(c) clip anchor/extent・comp range の型名を
  `TimeAnchor`/`ClipExtent`/`ProjectRange` へ修正、
  (d) automation `BeatPoint` を「再投影対象」から
  「再投影不要（既に rational beat 正本）」へ再分類、
  (e) overflow fixture 名の hedge を除去（実在確認済み）。
  いずれも P2 相当の精度修正で P0/P1/blocker なし、
  ACCEPT WITH NON-BLOCKING NOTES。
- 実装 crate の `check`/`lint`/`test` は本 slice の対象外
  （タスク指定の正規検証は `mise run fmt-check`）。

## Risks

- **対象 SHA の CI 証拠**: 本 slice は docs/evidence のみ
  でコード変更なし。merge 後の対象 SHA ci.yml run は
  daily-ci/手動 dispatch 待ち — 統合受入は Coordinator が
  対象 SHA CI 証拠で確認する（`defers`）。
- **fixture 被覆の解釈**: 多くの契約は先行 slice で実装・
  実測済みだが、本 slice は再実行していない。台帳は
  「fixture 設定済み・本 slice 未実走」と「構成済み
  テストなし=NotRun」を厳密に区別しているが、統合受入時
  に対象 SHA での fresh run が必要になれば各 Owner Issue
  の gate で実施される。
- **「1/7 beat」専用 fixture 不在**: denominator boundary
  fixture が同値の性質（grid 非表現な有理数位置の明示
  処理）を被覆するが、1/7 そのものの pin はない。generic
  rational time の完全 pin は ruviform#189 の別 gate。
- **実機・実 device/plugin 未検証**: headless fixture の
  Pass は contract 検証であり、実 MIDI device・実 plugin・
  実 OS/GUI の受入ではない（synvelis#253・TIM-03/04・
  ruviform#253/#307 へ Deferred）。
- **graph-router の既存 broken relations**: repository
  全体の pre-existing `defers`/`depends-on` 等の relation
  不整合は main の既存状態（route が `blocked`/
  `focus-required` を返す原因）。本 slice の新規文書が
  新たな broken relation を追加していないことを
  `doc-status`/index 整合で確認する。修復は範囲外。

## Follow-ups

- `evidence/sv19/case-ledger.md` の fixture 行について、
  統合受入時に対象 SHA で `mise run test <crate>` を実行し
  fresh evidence を追記するかは Coordinator/各 Owner
  Issue の判断に委譲。
- 「1/7 beat」専用 fixture・SMF SMPTE/Format2/MIDI2・
  実 MIDI device backend・実 plugin transport/state は
  TIM-F01/F02・TIM-03/04・synvelis#253・ruviform#189/
  #253/#307 の各 gate で実施。
- merge 後の対象 SHA CI 証拠が統合受入の正式証拠 —
  nearest-SHA 証拠を対象 SHA 証拠と混同しない。
