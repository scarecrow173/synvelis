---
id: "TASK-rlRe32Ahahq9E7GRDaytrp"
type: "task"
status: "done"
title: "Musical Time — 設計契約文書チェーン実装タスク（synvelis#19）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/musical-time.md"
  depends-on:
    - "docs/designs/musical-time-design.md"
  verifies:
    - "docs/test-specs/musical-time.md"
  source:
    - "https://github.com/scarecrow173/synvelis/issues/19"
---

# Musical Time — Task

## Task 1 — docs chain + evidence ledger（本 slice 全体）

Issue #19 の設計契約を docs chain 7 文書と
`evidence/sv19/` 受入証拠台帳として固定する docs/evidence
slice。コード変更は行わない。

- [x] `docs/specs/musical-time.md` — canonical spec
  （R-MT-01〜08 要件・時間ドメイン・受入条件写像
  SV19-C01〜C08・Deferred/Out of Scope・evidence 分類）。
- [x] `docs/designs/musical-time-design.md` — Owner マップ・
  時間ドメイン境界・site 分類・設計決定 D1〜D6・
  失敗/回復・代替案・fixture 写像。
- [x] `docs/plans/musical-time.md` — 実施計画。
- [x] `docs/test-specs/musical-time.md` — Rule
  MT-F01-1〜MT-F09-1 の受入検証契約（独立 oracle）。
- [x] `evidence/sv19/case-ledger.md` — Issue #19 受入条件
  8 項目を `SV19-C01`〜`SV19-C08` へ写像し、根拠 fixture・
  Expected/Observed・環境・SHA・Pass/Fail/NotRun/
  Unsupported/Deferred・丸め誤差を記録。
- [x] `docs/impl/ir/musical-time.md` — 実装/検証記録。
- [x] `docs/impl/exp/musical-time.md` — 実験/監査記録。
- [x] docs index 再生成（`doc-maintenance` `index apply`）と
  `doc_status.js lint`・`mise run fmt-check` の実行・記録。
- [x] review-protocol によるレビューと blocker 解消。
- [x] `git fetch origin main && git merge origin/main`・
  index 再生成・Conventional Commit・push・PR 作成・
  `worker_done` 送信。

## Task 2 — 以降（別 Owner Issue・本 slice 対象外）

以下は本 slice では実施しない。契約としてのみ参照する。

- [ ] TIM-03 (#127) plugin process/transport precision の
  実機検証 — capability 分類契約のみ本 slice が固定。
- [ ] TIM-04 (#128) plugin state snapshot 品質。
- [ ] TIM-F01 (#129) SMF SMPTE/Format 2、TIM-F02 (#130)
  MIDI 2/MPE。
- [ ] 実 MIDI device backend 検証（synvelis#253/
  ruviform#253）。
- [ ] Ruviform#189 generic configurable rational musical
  time（Synvelis は consumer）。
- [ ] 「1/7 beat」専用 fixture・その他未構成 fixture の新設
  — 必要なら Ruviform#189/TIM Owner の別 gate。
