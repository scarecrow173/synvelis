---
id: "IMPL-034X9NMZPZYBp9grVPuUZ4"
type: "impl"
status: "completed"
title: "Issue #8 DAW ワークフロー UX 文書チェーン確定"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/8"
  changes:
    added:
      - type: "file"
        path: "docs/specs/daw-workflow.md"
      - type: "file"
        path: "docs/designs/daw-workflow-design.md"
      - type: "file"
        path: "docs/plans/daw-workflow-plan.md"
      - type: "file"
        path: "docs/tasks/daw-workflow-task.md"
      - type: "file"
        path: "docs/test-specs/daw-workflow.md"
      - type: "file"
        path: "docs/impl/exp/daw-workflow.md"
      - type: "file"
        path: "evidence/sv8/daw-workflow-traceability.md"
      - type: "file"
        path: "evidence/sv8/daw-workflow-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/daw-workflow-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/daw-workflow-plan.md"
    - "docs/specs/daw-workflow.md"
    - "docs/designs/daw-workflow-design.md"
    - "docs/test-specs/daw-workflow.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/daw-workflow-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/exp/daw-workflow.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/daw-workflow.md"
    rejected: []
---

# Issue #8 DAW ワークフロー UX 文書チェーン確定

## Summary

Issue #8 の UX 仕様を canonical 文書チェーン
（spec→design→plan→task→test-spec→impl ir/exp）と
`evidence/sv8/` 台帳として確定した。**製品コードの実装は
一切行っていない** — `crates/`・`Cargo.toml`/`Cargo.lock`・
`mise.toml`・CI・Ruviform への変更はゼロであり、成果物は
文書のみ。実 OS Window・実 Plugin GUI・実機/実 Device の
検証は実施しておらず、各レーンは NotRun/Deferred として
明示した（POLICIES.md §2/§3）。実装は #10/#11/#12/#9/
#112/#114/#176 等の Owner Issue に委譲する。

## Context

- Task: `docs/tasks/daw-workflow-task.md`
- Plan: `docs/plans/daw-workflow-plan.md`
- Spec: `docs/specs/daw-workflow.md`（WF-R-01〜10・
  WF-AC-01〜10・制作領域と View 対応・P0/P1/P2 写像）
- Design: `docs/designs/daw-workflow-design.md`（Gate A
  backbone GA-01〜08・WF-SCN-MIDI/AUDIO/MIX・Keyboard/
  Gesture/Tool 契約・共通操作体系・Recording 状態・非依存
  境界・Decision Log D-1〜D-7）
- Issue: https://github.com/scarecrow173/synvelis/issues/8
  （2026-11-10 実行判断: Worker が設計ドラフト・契約/状態図・
  Case 表・review-protocol・PR まで担当。新規トレードオフ・
  契約破壊・定量 SLO・OS サポート宣言のみ「提案/決定待ち」
  に分離）

## Implementation

### Changes

#### Added

- `docs/specs/daw-workflow.md` — WF-R-01〜10・WF-AC-01〜10・
  制作領域と View 対応表・P0/P1/P2 ロードマップ写像。
- `docs/designs/daw-workflow-design.md` — Gate A backbone
  GA-01〜08・WF-SCN-MIDI/AUDIO/MIX（工程・Oracle・失敗系・
  検証レーン）・Keyboard/Gesture/Tool 契約（§7）・共通操作
  体系（§8）・Recording 状態（§9）・Remote/VCS 非依存（§10）・
  検証レーン（§11）・Decision Log D-1〜D-7。
- `docs/plans/daw-workflow-plan.md` — 文書作業の plan
  （File Map・Sequence・Verification）。
- `docs/tasks/daw-workflow-task.md` — 実装スライスと検証手順
  （`## Verification` に機械検査コマンドを記載）。
- `docs/test-specs/daw-workflow.md` — TS-R-01〜10・EX-1〜7・
  Guarantees/Non-goals/Risk。
- `docs/impl/exp/daw-workflow.md` — 設計判断ログ（既存契約
  再利用アプローチ・review-protocol・生成物命名の試行記録）。
- `evidence/sv8/daw-workflow-traceability.md` — WF-AC→要件→
  design 節→Scenario→Issue #8 対応・Issue §別カバレッジ・
  既存 docs 整合確認・Deferred 所在・Owner 委譲マップ。
- `evidence/sv8/daw-workflow-verification.md` — 実行した検証・
  baseline finding・NotRun/Deferred レーン・判定まとめ。

#### Generated

- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/README.md` — `doc_maintenance.js index apply`
  による再生成（手編集なし）。

### Decisions During Implementation

- **spec/design を approved 化**: タスク契約により
  review-protocol（2 pass + validate + judge）で blocker
  なしを確認した時点を承認代替として扱い、plan-doc の
  approved-design ゲートを通過させた。人間承認の捏造では
  なく、coordinator 契約に基づく手続き。
- **Gate A backbone を GA-01〜08 に固定**: Issue #8 の必須
  最小タスクを工程化し、全 WF-SCN で共通 Oracle を適用
  できるようにした（design D-1）。
- **WF-SCN-MIX の調査部分を detour 化**: Signal Flow/
  Inspector/Version Diff は spine から外し文脈保持の任意
  遷移とした（design D-2）。
- **Keyboard は操作→Command のみ固定**: キー割当は #112 に
  委譲し Deferred とした（design D-3）。
- **impl exp は repo 慣行の `.md`**: impl-doc CLI の JSONL
  経路ではなく `docs/impl/exp/daw-workflow.md`
  （`type: "experiment"`）を採用。script が生成した
  `issue-8-daw-ux.jsonl` は許可範囲外のため削除し、
  許可パス `daw-workflow.md` で作成した（sv3 先例と同じ
  判断）。
- **evidence は traceability + verification の 2 ファイル**:
  sv3 慣行に倣い、受入項目写像と実行検証記録を分離した。

## Related Experiments

### Adopted

- `docs/impl/exp/daw-workflow.md` — 「既存 headless 契約を
  前提参照し UX 規範のみ新規に書く」アプローチを採用。
  個別契約（#10/#11/#12/#18/#20/#21）は正本に残し、本書は
  ワークフロー上の位置づけと横断安全契約のみを定義した。

### Rejected

- なし。

## Validation

`evidence/sv8/daw-workflow-verification.md` に全記録。
概要: `doc_status.js lint`（spec/design/plan/task/test-spec/
impl）・`doc_maintenance.js index plan`→`apply`・
`mise run fmt-check`・`git diff --check`・`git status` 範囲
確認。実機/実 GUI/実 Device 検証は NotRun（#114/#176 lane）。

## Risks

- 記述した Oracle が実装時に検証不能になる可能性 → Oracle
  は独立検証可能な形に限定済み。実機検証は #114/#176。
- Keyboard/Gesture/Tool の最小操作集合が #112 で不足 →
  Deferred として明記、実装時に design 改訂。
- doc-driven-dev-graph router が既存 broken relation で
  fail-closed する可能性（sv3 先例）→ 発生時は blocker を
  記録し宣言済み文書順で継続。router 成功を偽装しない。

## Follow-ups

- #112: Keyboard 具体的キー割当・Tool UI の確定。
- #11/#12: Tool/Snap 数値・Take/Comp Editor UI の確定に応じ
  て design §7/§6.2 を照合。
- #114/#176: GUI-L2/EXT-L3 lane で WF-AC の実機検証。
- #23: Gate A node Acceptance 判定に本書の WF-SCN/GA
  backbone を使用。
