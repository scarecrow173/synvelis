---
id: "TASK-3E7SSn7u5OV3w997JoXm8f"
type: "task"
status: "done"
title: "Take Lane / Comp Editor — UI 詳細契約 canonical doc chain タスク（synvelis#12 §14/§15 後続 slice — docs-only）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  implements:
    - "docs/plans/take-lane-comp-editor.md"
  implemented-by: []
  depends-on:
    - "docs/plans/take-lane-comp-editor.md"
    - "docs/specs/take-lane-comp-editor.md"
    - "docs/designs/take-lane-comp-editor-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/take-lane-comp-editor.md"
    - "docs/impl/ir/take-lane-comp-editor.md"
    - "docs/impl/exp/take-lane-comp-editor.md"
    - "evidence/sv12/ui-contract-ledger.md"
    - "evidence/sv12/owner-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/take-lane-comp-editor.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/take-lane-comp-editor.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  defers: []
  deferred-by: []
---

# Take Lane / Comp Editor — UI 詳細契約 canonical doc chain タスク（synvelis#12 §14/§15 後続 slice — docs-only）

## Work

Issue #12 §14/§15 の合意済み UX 方針を canonical UI 契約へ固定する
docs-only slice。spec → design → plan → task → test-spec → impl/ir +
impl/exp の文書チェーンと `evidence/sv12/ui-contract-ledger.md` を作成し、
触れた dir の管理 index を再生成する。`crates/**`・実 GUI・数値
プリセット・OS 別既定は対象外。

## Classification

`doc-only`（docs-only 設計 slice — POLICIES §5 比例原則で軽量チェック）。

## タスク分解

- [x] T1: upstream 契約の調査（Issue #12 §13〜§15、recording-take-comping
  C6/C7、timeline-editing、ed-a/ed-b、ws-a、mda、sv-gui-03-06、rcd-04、
  owner-matrix の AC-28/29/30/31/32/36/38/39/41/47）
- [x] T2: `docs/specs/take-lane-comp-editor.md` 作成（REQ-TLC-01〜07・
  TL-AC-01〜12）
- [x] T3: `docs/designs/take-lane-comp-editor-design.md` 作成（契約
  C1〜C7・所有境界表・Editor Binding・HitTest/Tool matrix・Scenario・
  Decision Log）
- [x] T4: `docs/plans/take-lane-comp-editor.md` 作成
- [x] T5: `docs/tasks/take-lane-comp-editor-task.md` 作成（本書）
- [x] T6: `docs/test-specs/take-lane-comp-editor.md` 作成
  （F-LANE/F-SELECT/F-AUDITION/F-GESTURE/F-HANDLE/F-HITTEST/F-TOOL/
  F-EDITOR/F-FADE）
- [x] T7: `docs/impl/ir/take-lane-comp-editor.md` +
  `docs/impl/exp/take-lane-comp-editor.md` 作成
- [x] T8: `evidence/sv12/ui-contract-ledger.md` 作成（AC↔test-spec case
  対応表）
- [x] T9: 触れた dir の管理 index 再生成（doc-maintenance `index apply` /
  `sync_impl_index` safe sync API）
- [x] T10: 検証実行（fmt-check・prek・doc-status audit/lint・impl audit）
- [x] T11: `review-protocol` 実施（blocker 0）
- [x] T12: commit → push → PR 作成（`Refs #12`）

## 後続実装 Owner（本 slice 対象外）

- 実 GUI 実装（Take Lane / Comp Editor / Editor 統合）→ #112/#114 系
  slice。design 契約 C1〜C7・所有境界表・hit-test/tool matrix を消費。
- 数値プリセット（fade 既定・hit-test px 閾値・Snap Hysteresis）→
  #11 §9 PoC 後続決定。
- OS 別 Keyboard/Modifier/Touch 既定 → §14.8 未確定事項。
- MIDI note-level comp・chase・serialization → §15.6〜§15.9。
- Creative Branch Take/Comp 候補 → #278/#279/#288。
- 実機・実 OS・EXT-L3 受入 → #114・別受入 Gate。

## 完了条件

- 8 新規ファイルが存在し、front matter の id/type/status/relations が
  実態どおり。
- design が spec の REQ-TLC-01〜07・TL-AC-01〜12 を全てカバーし、
  upstream 正本と矛盾しない（第二正本を作っていない）。
- ui-contract-ledger が owner-matrix の UI 関連 AC 10 件全てを
  test-spec case へ対応付け、「契約 fixed / 実機 Deferred」を区別。
- 触れた dir の管理 index が再生成され in-sync。
- `review-protocol` で blocker 0。
- PR 本文に `Refs #12`（`Closes` 禁止）。

## ブランチ

`docs/12-take-lane-comp-editor`（`origin/main` = `476dd6f` 起点）。

## Verification

- [x] `mise run fmt-check` exits 0（`cargo fmt --all -- --check`、docs-only のため差分なし）
- [x] `mise exec -- prek run --all-files` exits 0（trim/eof/yaml/toml/mixed-line-ending/utf-8-bom/private-key/illegal-windows-names/actionlint 全て Passed or Skipped）
- [x] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan` が触れた全 dir で in-sync を示す（apply 後 rescan で 10 target 全て in-sync）
- [x] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply` 実行済み（specs/designs/plans/tasks/test-specs 各 `1 written, 0 blocked`）
- [x] `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir --apply` 実行済み（`docs/impl/ir/README.md` に add）。`--kind exp` は in-sync（exp index は `.jsonl` のみ管理・`.md` memo は対象外）
- [x] `node .agents/skills/doc-status/scripts/doc_status.js audit --type all --profile artifact` で新規 broken relation なし（新規分の finding は `orphan-navigation` warning のみ — repo 全体の既存慣行と同型）
- [x] `node .agents/skills/doc-status/scripts/doc_status.js lint --type spec|design|plan|task|test-spec` で新規分の error なし（pre-existing finding は範囲外）
- [x] `node .agents/skills/impl-doc/scripts/audit_impl_record.js` / `audit_experiment_log.js` で新規分の error なし（pre-existing error は他 doc・`issue-147-sccache.jsonl`）
- [x] cargo 系（check/lint/test/build/*-full/*-msrv）・実 GUI/実 OS 検証は `NotRun`/`NotApplicable`（docs-only・`crates/**` 非変更）として記録
