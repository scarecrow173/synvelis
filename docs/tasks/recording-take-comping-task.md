---
id: "TASK-fQ5GG20T5UOhj5Xy7za6XD"
type: "task"
status: "in-progress"
title: "Recording / Take / Comping — 文書チェーン・受入 Owner Matrix タスク（synvelis#12）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/recording-take-comping.md"
  implemented-by: []
  depends-on:
    - "docs/plans/recording-take-comping.md"
    - "docs/specs/recording-take-comping.md"
    - "docs/designs/recording-take-comping-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/recording-take-comping.md"
    - "docs/impl/ir/recording-take-comping.md"
    - "docs/impl/exp/recording-take-comping.md"
    - "evidence/sv12/owner-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/recording-take-comping.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/synvelis/issues/116"
    - "https://github.com/scarecrow173/synvelis/issues/117"
    - "https://github.com/scarecrow173/synvelis/issues/118"
---

# Recording / Take / Comping — 文書チェーン・Owner Matrix タスク（synvelis#12）

## 目的

Issue #12 の基本アーキテクチャ合意を canonical 文書チェーンと 69 受入条件の Owner Matrix として固定する docs-only slice。実装は #115〜#118 が Owner。

## タスク分解

- [x] **T1 spec**: `docs/specs/recording-take-comping.md` — What/Why・AC-01〜69・Non-goals・Sources
- [x] **T2 design**: `docs/designs/recording-take-comping-design.md` — Owner 境界・C1〜C7 契約・状態モデル・Scenario/Oracle・Decision Log
- [x] **T3 plan**: `docs/plans/recording-take-comping.md` — scope・file map・dependencies・risks・verification・handoff・rollback
- [x] **T4 task**: 本書 — task 分解・完了条件・実装 Owner
- [x] **T5 test-spec**: `docs/test-specs/recording-take-comping.md` — Feature/Rule/Example/Guarantee・fixture 契約・状態語彙
- [x] **T6 owner-matrix**: `evidence/sv12/owner-matrix.md` — 69 AC × Owner Issue × fixture × OS Gate × 状態分類
- [x] **T7 ir**: `docs/impl/ir/recording-take-comping.md` — 実施記録（docs-only・handoff）
- [x] **T8 exp**: `docs/impl/exp/recording-take-comping.md` — graph route・ID 生成・判断記録
- [x] **T9 index**: touched dir（specs/designs/plans/tasks/test-specs/impl-ir/impl-exp）の管理 index を doc-maintenance `index apply` で再生成し `in-sync` 確認
- [x] **T10 verify**: `mise run fmt-check`・doc-status audit・review-protocol（blocker 0）
- [ ] **T11 ship**: `git fetch origin main && git merge origin/main` → commit → `git push -u origin docs/12-recording-take-comping` → `gh pr create`（日本語本文）→ `worker_done`

## 実装 Owner（後続 slice、本 slice 対象外）

| Owner | Issue | 範囲 |
| --- | --- | --- |
| RCD-01 | #115 | 実 Device Capture・同 Clock 複数入力・Raw Fidelity・fail-stop |
| RCD-02 | #116 | Writer/Staging/Journal/Checkpoint/Finalize/Recovery・Asset Manifest |
| RCD-03 | #117 | Monitor/Punch/Loop/Latency・Raw Capture 不変性 |
| RCD-04 | #118 | Comp/Take Group/Effective Comp・MIDI Phrase・Recovery E2E・Stable ID |
| 製品 Gate | #114 | 実 window/OS/Device/Plugin 受入（GUI-L2/EXT-L3） |

## 完了条件

- 7 文書 + `evidence/sv12/` が canonical front matter・relations・status で揃う。
- 69 AC が spec・owner-matrix・test-spec で同一 ID（AC-01〜69）に対応し、集計件数が一致する。
- `Pass (headless)`/`Partial`/`NotRun`/`Deferred`/`Unsupported` が POLICIES.md 語彙で厳密分離され、headless 成功を実機 Pass に換算していない。
- 触れた dir の管理 index が `in-sync`。
- `mise run fmt-check` exit 0、review-protocol blocker 0。
- 差分が占有範囲 + 生成 index のみ。

## ブランチ

`docs/12-recording-take-comping`（worktree は `scarecrow173/sv12-recording-take` で作業し、push 時にブランチ名を合わせるか coordinator 指定に従う）。
