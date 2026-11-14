---
id: "IMPL-034Xbn2n8xBopZHuY8g53Y"
type: "impl"
status: "completed"
title: "PC-C6 acceptance ledger"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added:
      - type: "file"
        path: "evidence/sv103/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv103/coverage.md"
      - type: "file"
        path: "evidence/sv103/verification.md"
      - type: "file"
        path: "evidence/sv103/external-gates.md"
      - type: "file"
        path: "docs/impl/ir/pc-c6-acceptance-ledger.md"
      - type: "file"
        path: "docs/impl/exp/pc-c6-acceptance-ledger.md"
    modified:
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/pc-c6-oracle-rows-slice.md"
    - "docs/impl/ir/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/impl/ir/pc-c6-tr-ledger-oracle.md"
    - "docs/impl/ir/pc-c6-tr02-tr06-realpath-oracle.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
    - "docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
    - "docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md"
  verified-by: []
  references:
    - "evidence/sv103/acceptance-ledger.md"
    - "evidence/sv103/coverage.md"
    - "evidence/sv103/verification.md"
    - "evidence/sv103/external-gates.md"
    - "docs/impl/ir/pc-c6-oracle-rows-slice.md"
    - "docs/impl/ir/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/impl/ir/pc-c6-tr-ledger-oracle.md"
    - "docs/impl/ir/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/test-specs/pc-c6-e2e-oracle-rel-tr.md"
    - "docs/test-specs/pc-c6-tr-ledger-oracle.md"
    - "docs/test-specs/pc-c6-tr02-tr06-realpath-oracle.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/28"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/176"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/pc-c6-acceptance-ledger.md"
    rejected: []
---

# PC-C6 acceptance ledger

## Summary

Issue #103 固定 Test Matrix 全 53 行（PC-C1-R01〜R08・
PC-C2-REL01〜REL07・PC-C3-TR01〜TR08・PC-C4-PL01〜PL08・
PC-C5-UI01〜UI07・PC-C6-COMP01〜COMP05・PC-X1〜X6・
GUI-PC-01〜04）を統合済み fixture/docs へ写像する audit を実施し、
`evidence/sv103/` に acceptance ledger・coverage・verification・
external-gates の 4 証拠 file を作成した。headless で実施可能な
部分は既存 fixture で全行被覆済みと判定し、新規 test fixture の
追加は行わなかった。実 OS/実 GUI/実 plugin/実 device/実 RT/両
consumer gate は `NotRun`/`external owner` として分離記録し、
headless の Pass を実機 Pass に昇格しない。

## Context

Issue #103 の acceptance-ledger residual slice。PC-C1〜C6 の
実装 slice（`pc-c6-oracle-rows`・`pc-c6-oracle-rel-tr`・
`pc-c6-tr-ledger-oracle`・`pc-c6-tr02-tr06-realpath-oracle`）は
main 統合済みで、各 slice は自らの行の headless 証拠を持つ。
本 slice は新規実装ではなく、全受入行を 1 つの台帳に統合し、
未実行・外部 Owner・headless 境界を正直に分類する残件を閉じる。
spec/design/plan/task/test-spec の文書チェーンは統合済みのため
新規作成せず、既存鎖を参照する（Coordinator 判断どおり）。

## Implementation

### Changes

- `evidence/sv103/acceptance-ledger.md`（新規）: 53 行全ての
  受入内容 → 証拠 pointer（`file::test` または doc）→ 判定
  （Pass-measured (headless) / NotRun / Unsupported /
  external owner）の対応表。計測環境（Synvelis SHA・Ruviform
  resolved SHA・OS/CPU・toolchain）・判定語彙・監査方法・
  fixture 補完判定・集計・判定ルールを収録。
- `evidence/sv103/coverage.md`（新規）: 行ごとの被覆 fixture
  棚卸し（✅/◐/⛔ 凡例付き）。PC-C1-R08 の note-scope 部分被覆、
  PC-C2-REL07・PC-X5 の絶対性能閾値 NotRun、PC-C5-UI01 の
  3 view 単一 fixture 非存在などの ◐ を明示。
- `evidence/sv103/verification.md`（新規）: 実行した canonical
  検証（`mise run fmt-check`/`check`/`lint`/`test` for
  synvelis-project・synvelis-player + 補助として
  synvelis-workspace test）のコマンド・OS・toolchain・exit・
  結果。Expected/Observed 代表例・未検証項目を記録。
- `evidence/sv103/external-gates.md`（新規）: 実 GUI（#114）・
  実 plugin（#176）・実 device/RT・製品 Gate（#23）・両
  consumer（ruviform#219/#294・synvelis#28）・Bridge Clock
  （ruviform#307）・絶対性能閾値・CI covering run の
  NotRun/external owner 対応表。
- `docs/impl/exp/pc-c6-acceptance-ledger.md`（新規）: 監査過程の
  実験記録（fixture 欠落判定の方法・発見した留意点）。
- `docs/impl/ir/README.md`・`docs/impl/exp/README.md`: doc-maintenance
  の index 再生成（手編集なし）。
- fixture/test 追加: **なし**（後述 Decisions）。

### Decisions During Implementation

- **新規 fixture 追加なし**: 監査の結果、PC-C1〜C6・PC-X・
  GUI-PC の headless 実施可能部分は全て既存 fixture
  （`pc_c6_oracle_*`・`pc_c1`・`aut_0*`・`pc_a`・`pc_b`・
  `pc_c2_*`・`pc_c4_*`・`trace_c3*`・`tim03_*`・`tim04_*`・
  `uic5*`・`rel_c5`・`sv47_controls`・`sf_05*`・
  `controller_store`・`plugin_state`・`gw10_system`）で被覆
  済み。「fixture が既存行を既に被覆している場合は新規 test を
  追加しない」の条件に合致。残る未被覆は全て実機依存で
  headless では原理的に埋められないため、external-gates.md の
  NotRun/external owner 記録が正しい処理。
- **Pass-measured (headless) の語彙**: 「`mise run test` が当該
  SHA で green だった」ことのみを意味し、case 全体の受入 Pass
  ではないと明記。`Unsupported` は 0 行（捏造しない）。
- **PL 行コメント番号の差異を記録**: `pc_c6_oracle_pl.rs` 内の
  PL03/PL04/PL06 コメントは test-spec Rule の semantic と一部
  入れ替わっている（コメント PL03=QueueFull は Rule(PL04) 等）。
  fixture 内容は全行被覆するため本 ledger は test-spec（Issue
  受入条件）の semantic で写像し、差異を ledger に明記した。
- **部分被覆の正直な記録**: PC-C1-R08（note-scope は eval 層
  `ctl_01` + record transaction 単一経路 `pc_b_02_note_scope_
  records_note_relative` まで。R08 の Note/Clip/Tempo×drift
  全組合せ fixture は無し）、PC-C5-UI01（3 view 同時照合
  fixture 無し）、
  PC-C3-TR07（実 2 window は #114/#102 責務）、PC-X2
  （mode×transport 直積 fixture 無し）、PC-C6-COMP04
  （Synvelis 側のみ）を ◐/部分被覆として明示し、完全被覆を
  装わない。
- **synvelis-workspace test を補助証拠として実行**: ledger が
  workspace fixture を引用するため引用の正直性のために実行
  （変更範囲に比例 — POLICIES.md §5）。`check`/`lint` は同
  crate 未変更のため未実行と明記。

## Related Experiments

### Adopted

- `docs/impl/exp/pc-c6-acceptance-ledger.md`: 監査方法
  （行→fixture 写像の手順）・被覆判定の観察・発見した留意点
  （PL コメント番号差異・部分被覆箇所）を記録。台帳の判定根拠
  として採用。

### Rejected

<!-- なし。 -->

## Validation

canonical 検証（2026-11-14、OS: Windows 11 Pro 10.0.26300、
CPU: i9-9900K、toolchain: rust 1.99.1 via `mise run`、
Synvelis base SHA `eac70ac` = origin/main HEAD、Ruviform
`fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`）:

- `mise run fmt-check` → exit 0
- `mise run check synvelis-project` → exit 0
- `mise run check synvelis-player` → exit 0
- `mise run lint synvelis-project` → exit 0
- `mise run lint synvelis-player` → exit 0
- `mise run test synvelis-project` → exit 0
- `mise run test synvelis-player` → exit 0
- `mise run test synvelis-workspace` → exit 0（補助 — ledger 引用
  fixture の実測確認）
- `node .claude/skills/impl-doc/scripts/audit_impl_record.js --json`
  → 本記録の front matter/section 監査
- `node .claude/skills/impl-doc/scripts/audit_experiment_log.js
  --json` → exp 記録の監査
- `node .claude/skills/impl-doc/scripts/sync_impl_index.js
  --kind ir --apply` / `--kind exp --apply` → index 再生成

個別 oracle 実測: `pc_c6_oracle_rec` 8・`pc_c6_oracle_ui` 7・
`pc_c6_oracle_rel` 7・`pc_c6_oracle_tr` 8・
`pc_c6_oracle_tr_dispatch` 8・`pc_c6_oracle_pl` 8・
`pc_c6_oracle_pl_rt` 1 — 全 pass（`mise run test` 出力より）。

review-protocol: cooperative（Reviewer A）+ adversarial
（Reviewer B）の独立 2 pass で evidence/impl 文書をレビュー。
Reviewer A は行網羅（53/53 行の table row 確認）・test 名・
件数の実測照合を実施。Reviewer B は adversarial に pointer 精度・
過剰主張・scope 違反を探索。CONFIRMED findings（全て P2
文書精度、P0/P1 blocker なし）: (1) 集計の行数算定誤り —
coverage.md の ◐ 11 行と整合する 42/11 分類に修正、(2)
`pc_b.rs`/`ctl_01.rs`/`tim04_plugin_state.rs`/`plugin_state.rs`
の crate 間同名 file による pointer 曖昧性 — crate 修飾付き
path に修正、(3) PC-C1-R08 の note-scope 被覆過小評価 —
workspace `pc_b.rs::pc_b_02_note_scope_records_note_relative`
（record transaction 層の note-relative 記録）を発見し証拠に
追加・部分被覆の記述を修正、(4) `aut_01.rs` mode 系「8 tests」
の未検証件数 — 実測不能な精密件数を削除。修正後に Validator
相当の再照合（grep による test 関数実在確認・件数再集計）で
全 pointer の実在を確認。blocker なし。

## Risks

- 台帳の行→証拠写像は人手 audit に基づく — fixture 名・行番号の
  転記ミスは test 失敗では顕在化しない。`coverage.md` の
  file::test pointer を二次確認するレビューで担保する。
- `Pass-measured (headless)` を受入 Pass と誤読するリスク —
  ledger 冒頭の判定語彙と各行の NotRun/external owner 併記で
  緩和。POLICIES.md §3 の Required/実行結果分離に従う。
- `synvelis-workspace` fixture は本 slice の編集範囲外のため、
  将来の fixture 変更で台帳 pointer が陳腐化し得る — pointer は
  test 名まで記録し file 構造への依存を最小化した。
- PC-C6-COMP 行は「判定記録」行であり製品 Gate 受入ではない —
  external-gates.md で #23 Owner を明示し、headless 構成証拠を
  製品判定と混同しない。

## Follow-ups

- 実 GUI Scenario（GUI-PC-01〜04）・実 plugin・実 device・両
  consumer・絶対性能閾値・CI covering run: `external-gates.md`
  の NotRun/external owner に従い、#114・#176・#23・#28・
  ruviform#219/#294/#307 の各 Owner で実施。
- PC-C1-R08 の note-scope drift 組合せ fixture・PC-C5-UI01 の
  3 view 同時照合 fixture・PC-X2 の mode×transport 直積 fixture
  は将来の補完候補（現状は部分被覆として台帳に明示）。
- merge 後対象 SHA の Daily/Manual Full CI 証拠は Coordinator
  が確認。Issue #103 は Open 継続（`Refs` のみ、Close しない）。
