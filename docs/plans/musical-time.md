---
id: "PLAN-aXqZXebxtF6m7A9e6xy5S4"
type: "plan"
status: "approved"
title: "Musical Time — 設計契約文書チェーン実装計画（synvelis#19）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/19"
  changes:
    added:
      - type: "file"
        path: "docs/plans/musical-time.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/musical-time.md"
  implemented-by: []
  depends-on:
    - "docs/designs/musical-time-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/tim-01-configurable-ppqn.md"
    - "docs/plans/tim-01-ppqn-config-plan.md"
    - "docs/plans/tim-01-app-ppqn-plan.md"
    - "docs/plans/tim-01-ppqn-preview-plan.md"
    - "docs/plans/tim-01-ppqn-ui-plan.md"
    - "docs/plans/tim-02-smf-format01-core.md"
    - "docs/plans/tim-02-ppqn-smf-cross-golden-plan.md"
    - "docs/plans/tim-02-player-midi-rt-offline.md"
    - "docs/plans/tim03-process-boundary.md"
    - "docs/plans/tim-03-session-driver-wiring-plan.md"
    - "docs/plans/tim-04-plugin-state-snapshot.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/musical-time.md"
    - "docs/designs/musical-time-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/musical-time.md"
  references:
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
---

# Musical Time — 設計契約文書チェーン実装計画（synvelis#19）

## 目的

Issue #19 の設計・ADR 要求を、docs chain 7 文書 +
`evidence/sv19/` 受入証拠台帳として固定する docs/evidence
slice の実施計画。コード変更は行わない。実装は TIM-01/02/03/04
・Ruviform#189 等の Owner Issue に委譲済みであり、本計画は
それらの契約を Issue #19 の受入条件へ写像し、被覆を監査
することに限定する。

## 前提（調査済み事実）

- TIM-01 (#125): `SetProjectPpqn`・`PpqnConversionPolicy`・
  `ppqn.rs` reprojection・app gate・preview・UI snap/grid が
  main 統合済み。`tim_01_ppqn.rs`・`tim01_ppqn_*.rs` fixture
  が存在。
- TIM-02 (#126): `smf.rs` codec・`SmfDocument` provenance・
  `ImportMidiSmf`・`export_smf`+`ConversionReport`・
  immutable guard・player/recording note safety が main
  統合済み。`tim_02_smf.rs`・`tim_02_ppqn_smf.rs`・
  `tim02_midi_playback.rs`・`tim_02_midi_phrase.rs` fixture
  が存在。
- Punch mark silent retime 修正（sv#125 前提）は PR #385 で
  統合済み。
- `SCHEMA_VERSION = 13`、v5 snapshot migration・旧 byte 保持の
  fixture が存在。
- doc-driven-dev-graph の route は pre-existing broken
  relations により `blocked`/`focus-required` を返す
  （本 slice 範囲外の既存状態として記録・修復しない）。

## 実施ステップ

### Step 1 — spec/design/test-spec の契約固定

- `docs/specs/musical-time.md` — R-MT-01〜08 要件・
  時間ドメイン・受入条件写像・Deferred/Out of Scope。
- `docs/designs/musical-time-design.md` — Owner マップ・
  時間ドメイン境界・site 分類・設計決定 D1〜D6・
  失敗/回復・代替案・fixture 写像。
- `docs/test-specs/musical-time.md` — Rule MT-F01-1〜
  MT-F09-1・独立 oracle・Deferred/NotRun の扱い。

### Step 2 — 証拠台帳

- `evidence/sv19/case-ledger.md` — Issue #19 受入条件 8 項目を
  `SV19-C01`〜`SV19-C08` へ写像。各 Case に根拠 fixture
  （crate::test/file）・Expected・Observed（先行実測参照 or
  本 slice 未実走）・Status（Pass/Pass*/NotRun/Unsupported/
  Deferred）・丸め誤差/overflow 証拠を記録。

### Step 3 — plan/task/impl 記録

- `docs/plans/musical-time.md`（本書）・
  `docs/tasks/musical-time-task.md`・
  `docs/impl/ir/musical-time.md`・
  `docs/impl/exp/musical-time.md`。

### Step 4 — index 再生成と検証

- `doc-maintenance` の `index plan`/`index apply` で touched
  dir（specs/designs/plans/tasks/test-specs/impl/ir）の index
  を再生成。
- `doc_status.js lint` を影響 dir に実行。
- `mise run fmt-check`（正規検証）。

### Step 5 — review-protocol と完了手順

- review-protocol で客観チェック→独立 review→findings
  検証→severity 判定。P0/P1/blocker を解消。
- `git fetch origin main && git merge origin/main` で最新
  main を取り込み、index を再生成。
- Conventional Commits でコミット、`git push -u origin
  scarecrow173/sv19-musical-time`、`gh pr create`（日本語）。
- `worker_done` を 1 回送信。

## スコープ外（本 slice で行わない）

- 実装 crate のコード変更・test 追加・fixture 新設
  （「1/7 beat」専用 fixture 等は Ruviform#189/TIM Owner の
  別 gate — ledger で NotRun/Deferred として明記）。
- 実 MIDI device・実 plugin・実 OS/GUI 検証。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI/本番 crate。
- graph の pre-existing broken relations 修復。
- main への直接 push・force push・履歴書き換え・PR merge・
  Issue close。

## リスクと対策

| リスク | 対策 |
| --- | --- |
| fixture 被覆の誤認（存在≠実走済み） | ledger で「fixture 設定済み・先行実測 Pass 参照・本 slice 未実走」と「構成済みテストなし=NotRun」を厳密区別。SHA/環境を明記。 |
| headless Pass を実機受入に換算 | spec R-MT-08・test-spec Deferred 節・ledger Status 語彙で明示分離。 |
| docs index の手編集誘惑 | `doc-maintenance` の `index apply` のみ使用。 |
| relation target の誤記で broken-relation | 全 relation target を `test -f` で実在確認済み。lint で再検査。 |
| 「1/7 beat」fixture 不在の過大主張 | test-spec Rule MT-F03-1 に専用 fixture 未構成を明記し、性質被覆（denominator boundary）と専用 fixture を区別。 |

## 検証計画

| 検証 | コマンド | 判定 |
| --- | --- | --- |
| docs/設計スライス fmt | `mise run fmt-check` | exit 0 |
| docs index 整合 | `doc_maintenance.js index apply` + `doc_status.js lint` | touched dir で新規 finding なし |
| relation target 実在 | `test -f` 全 target | 全 OK |
| 実装 crate 検証 | — | **対象外**（docs/evidence slice） |
| 統合受入 | merge 後対象 SHA の CI | Coordinator 確認（別 gate） |
